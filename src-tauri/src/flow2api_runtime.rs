use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::watch;
use tokio::time::{sleep, timeout, Instant};
use uuid::Uuid;

const EVENT_NAME: &str = "event:flow2api";
const READY_PREFIX: &str = "BANG_FLOW_READY ";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Flow2ApiStatus {
    pub state: String,
    pub base_url: Option<String>,
    pub version: Option<String>,
    pub message: Option<String>,
}

impl Default for Flow2ApiStatus {
    fn default() -> Self {
        Self { state: "starting".into(), base_url: None, version: None, message: None }
    }
}

#[derive(Debug, Deserialize)]
struct Manifest {
    protocol: u32,
    target: String,
    python: String,
    entrypoint: String,
    browser_executable: Option<String>,
    version: String,
}

#[derive(Debug, Deserialize)]
struct ReadyLine {
    protocol: u32,
    instance: String,
    port: u16,
}

#[derive(Clone)]
pub struct Flow2ApiManager {
    state: Arc<Mutex<Flow2ApiStatus>>,
    endpoint: Arc<Mutex<Option<String>>>,
    auth_token: Arc<Mutex<Option<String>>>,
    shutdown_tx: Arc<Mutex<watch::Sender<bool>>>,
    generation: Arc<Mutex<u64>>,
    /// Opt-in fixed loopback port for the child; 0 keeps the ephemeral default.
    /// Only needed so a client that must know the address in advance (a Chrome
    /// extension worker) can be configured once. Never anything but loopback.
    fixed_port: Arc<Mutex<u16>>,
    /// Windows job object keeping the child tree owned; held open on purpose.
    job_handle: Arc<Mutex<isize>>,
}

impl Flow2ApiManager {
    pub fn new() -> Self {
        let (shutdown_tx, _) = watch::channel(false);
        Self {
            state: Arc::new(Mutex::new(Flow2ApiStatus::default())),
            endpoint: Arc::new(Mutex::new(None)),
            auth_token: Arc::new(Mutex::new(None)),
            shutdown_tx: Arc::new(Mutex::new(shutdown_tx)),
            generation: Arc::new(Mutex::new(0)),
            fixed_port: Arc::new(Mutex::new(0)),
            job_handle: Arc::new(Mutex::new(0)),
        }
    }

    pub fn set_fixed_port(&self, port: u16) {
        *self.fixed_port.lock().unwrap() = port;
    }

    pub fn fixed_port(&self) -> u16 {
        *self.fixed_port.lock().unwrap()
    }

    /// Start (or restart) the supervisor. A supervisor already running is
    /// cancelled first so `flow2api_restart` cannot leave two children behind.
    pub fn start(&self, app: AppHandle) {
        let manager = self.clone();
        let (tx, mut shutdown) = watch::channel(false);
        let gen = {
            let mut current = self.generation.lock().unwrap();
            *current = current.wrapping_add(1);
            // Cancel any supervisor from an earlier generation before the new
            // one takes over, otherwise a restart leaves two children behind.
            let mut slot = self.shutdown_tx.lock().unwrap();
            let _ = slot.send(true);
            *slot = tx;
            *current
        };
        *self.endpoint.lock().unwrap() = None;
        *self.auth_token.lock().unwrap() = None;
        tauri::async_runtime::spawn(async move {
            let _ = manager.run_supervisor(app, gen, &mut shutdown).await;
        });
    }

    pub fn status(&self) -> Flow2ApiStatus {
        self.state.lock().unwrap().clone()
    }

    pub fn endpoint(&self) -> Result<(String, String), String> {
        let url = self.endpoint.lock().unwrap().clone()
            .ok_or_else(|| "Flow2API internal belum siap. Tunggu beberapa saat lalu coba lagi.".to_string())?;
        let token = self.auth_token.lock().unwrap().clone()
            .ok_or_else(|| "Kredensial Flow2API internal belum siap.".to_string())?;
        Ok((url, token))
    }

    /// Bundled-mode credential for the backend: the launch token is the API key.
    pub fn bundled_credential(&self) -> Option<String> {
        self.auth_token.lock().unwrap().clone()
    }

    /// Wait briefly for a ready endpoint so a generation click right after
    /// launch does not fail on a still-starting backend.
    pub async fn wait_ready(&self, budget: Duration) -> Result<(String, String), String> {
        let deadline = Instant::now() + budget;
        loop {
            if let Ok(pair) = self.endpoint() {
                return Ok(pair);
            }
            let status = self.status();
            if status.state == "failed" || status.state == "stopped" {
                return Err(status.message.unwrap_or_else(|| "Flow2API internal gagal dijalankan.".into()));
            }
            if Instant::now() >= deadline {
                return Err("Flow2API internal belum siap. Coba lagi sebentar lagi dari Pengaturan.".into());
            }
            sleep(Duration::from_millis(250)).await;
        }
    }

    pub fn stop(&self) {
        let sender = self.shutdown_tx.lock().unwrap().clone();
        let _ = sender.send(true);
    }

    async fn run_supervisor(&self, app: AppHandle, gen: u64, shutdown: &mut watch::Receiver<bool>) -> Result<(), String> {
        let mut attempt = 0u32;
        loop {
            if *shutdown.borrow() { break; }
            attempt = attempt.saturating_add(1);
            self.publish(&app, gen, Flow2ApiStatus {
                state: if attempt == 1 { "starting" } else { "restarting" }.into(),
                base_url: None, version: None,
                message: (attempt > 1).then(|| format!("Flow2API mulai ulang (percobaan {attempt}).")),
            });
            match self.run_child(&app, gen, shutdown).await {
                Ok(()) => break,
                Err(_) if *shutdown.borrow() => break,
                Err(error) => {
                    crate::diag::log_line(&format!("Flow2API supervisor: {error}"));
                    *self.endpoint.lock().unwrap() = None;
                    *self.auth_token.lock().unwrap() = None;
                    self.publish(&app, gen, Flow2ApiStatus { state: "failed".into(), base_url: None, version: None, message: Some(error) });
                    if attempt >= 6 { break; }
                    let delay = Duration::from_secs((attempt.min(5) * 2) as u64);
                    tokio::select! { _ = sleep(delay) => {}, _ = shutdown.changed() => break }
                }
            }
        }
        self.publish(&app, gen, Flow2ApiStatus { state: "stopped".into(), base_url: None, version: None, message: None });
        Ok(())
    }

    async fn run_child(&self, app: &AppHandle, gen: u64, shutdown: &mut watch::Receiver<bool>) -> Result<(), String> {
        let resource_dir = app.path().resource_dir().map_err(|e| format!("Folder resource app tidak tersedia: {e}"))?;
        let runtime = resource_dir.join("flow2api");
        let manifest_path = runtime.join("manifest.json");
        let raw = tokio::fs::read(&manifest_path).await
            .map_err(|e| format!("Runtime Flow2API tidak ditemukan di {}: {e}", manifest_path.display()))?;
        let manifest: Manifest = serde_json::from_slice(&raw).map_err(|e| format!("Manifest runtime Flow2API tidak valid: {e}"))?;
        if manifest.protocol != 1 || manifest.target != current_target() {
            return Err(format!("Runtime Flow2API tidak cocok (protocol {}, target {}).", manifest.protocol, manifest.target));
        }
        let python = runtime.join(&manifest.python);
        let entrypoint = runtime.join(&manifest.entrypoint);
        if !python.is_file() || !entrypoint.is_file() {
            return Err("Interpreter atau launcher Flow2API tidak ada di paket.".into());
        }
        let data_dir = crate::paths::user_data_dir().join("flow2api");
        tokio::fs::create_dir_all(&data_dir).await.map_err(|e| format!("Tidak dapat membuat data Flow2API: {e}"))?;
        let migration = crate::flow2api_migration::prepare(&crate::paths::user_data_dir(), crate::flow2api_migration::discover_source().as_deref())?;
        crate::diag::log_line(&format!("Flow2API migration: {migration}"));

        let fixed_port = self.fixed_port();
        if fixed_port != 0 {
            // Fail fast and legibly instead of waiting out the 90 s readiness
            // timeout when something else already holds the port.
            if let Err(error) = check_loopback_port_free(fixed_port) {
                return Err(error);
            }
        }

        let token = Uuid::new_v4().to_string();
        let instance = Uuid::new_v4().to_string();
        let mut command = Command::new(&python);
        command.arg("-I").arg("-u").arg(&entrypoint)
            .arg("--data-dir").arg(&data_dir)
            .current_dir(&data_dir)
            .env("FLOW2API_DATA_DIR", &data_dir)
            .env("FLOW2API_EMBEDDED", "1")
            .env("PYTHONUNBUFFERED", "1")
            .env("PYTHONIOENCODING", "utf-8")
            .env("PLAYWRIGHT_BROWSERS_PATH", runtime.join("browsers"))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if fixed_port != 0 {
            // Opt-in: a client that cannot follow an ephemeral port (the Chrome
            // extension worker) needs an address known in advance. Still loopback.
            command.arg("--port").arg(fixed_port.to_string());
        }
        if let Some(browser) = manifest.browser_executable.as_ref() {
            let path = runtime.join(browser);
            if path.exists() { command.env("BROWSER_EXECUTABLE_PATH", path); }
        }
        #[cfg(target_os = "windows")]
        command.creation_flags(CREATE_NO_WINDOW);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
        }

        let mut child = command.spawn().map_err(|e| format!("Gagal menjalankan Python Flow2API: {e}"))?;
        match own_process_tree(&child) {
            Ok(handle) => {
                // Close the previous job handle (if any) before keeping this
                // one, so a retry cannot leak handles for the whole session.
                let previous = std::mem::replace(&mut *self.job_handle.lock().unwrap(), handle);
                close_job_handle(previous);
            }
            Err(error) => crate::diag::log_line(&format!("Flow2API job object: {error}")),
        }
        let stdin = child.stdin.take().ok_or("Pipe stdin Flow2API tidak tersedia")?;
        let stdout = child.stdout.take().ok_or("Pipe stdout Flow2API tidak tersedia")?;
        let stderr = child.stderr.take().ok_or("Pipe stderr Flow2API tidak tersedia")?;
        let (ready_tx, mut ready_rx) = tokio::sync::oneshot::channel::<Result<ReadyLine, String>>();
        let expected_instance = instance.clone();
        let mut ready_tx = Some(ready_tx);
        let stdout_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(payload) = line.strip_prefix(READY_PREFIX) {
                    if let Some(tx) = ready_tx.take() {
                        let parsed = serde_json::from_str::<ReadyLine>(payload).map_err(|e| e.to_string());
                        let _ = tx.send(parsed);
                    }
                }
            }
        });
        let app_for_stderr = app.clone();
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let safe: String = line.chars().take(500).collect();
                crate::diag::log_line(&format!("Flow2API: {safe}"));
            }
            drop(app_for_stderr);
        });

        let mut stdin = stdin;
        let initial = serde_json::json!({"token": token, "instance": instance});
        stdin.write_all(format!("{initial}\n").as_bytes()).await.map_err(|e| e.to_string())?;
        stdin.flush().await.map_err(|e| e.to_string())?;

        let ready = tokio::select! {
            result = timeout(Duration::from_secs(90), &mut ready_rx) => result.map_err(|_| "Flow2API tidak siap setelah 90 detik.".to_string())?
                .map_err(|_| "Flow2API readiness pipe terputus.".to_string())??,
            _ = shutdown.changed() => {
                let _ = stdin.write_all(b"{\"command\":\"shutdown\"}\n").await;
                terminate_owned(&mut child).await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Ok(());
            }
        };
        if ready.protocol != 1 || ready.instance != expected_instance || ready.port == 0 {
            terminate_owned(&mut child).await;
            return Err("Handshake identitas Flow2API tidak valid.".into());
        }
        let base = format!("http://127.0.0.1:{}", ready.port);
        if let Err(error) = verify_ready(&base, &token, &expected_instance).await {
            // The child answered on stdout but not at its endpoint; do not
            // leave it running while the supervisor retries.
            let _ = stdin.write_all(b"{\"command\":\"shutdown\"}\n").await;
            let exited = timeout(Duration::from_secs(8), child.wait()).await;
            if exited.is_err() { terminate_owned(&mut child).await; }
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(error);
        }
        *self.endpoint.lock().unwrap() = Some(base.clone());
        *self.auth_token.lock().unwrap() = Some(token);
        self.publish(app, gen, Flow2ApiStatus { state: "ready".into(), base_url: Some(base), version: Some(manifest.version), message: Some("Backend hidup; captcha/token tetap perlu dikonfigurasi.".into()) });

        tokio::select! {
            status = child.wait() => {
                let status = status.map_err(|e| format!("Gagal menunggu Flow2API: {e}"))?;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                if *shutdown.borrow() { Ok(()) } else { Err(format!("Flow2API berhenti dengan status {status}.")) }
            }
            _ = shutdown.changed() => {
                let _ = stdin.write_all(b"{\"command\":\"shutdown\"}\n").await;
                let exited = timeout(Duration::from_secs(8), child.wait()).await;
                if exited.is_err() { terminate_owned(&mut child).await; }
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                Ok(())
            }
        }
    }

    fn publish(&self, app: &AppHandle, gen: u64, status: Flow2ApiStatus) {
        if *self.generation.lock().unwrap() != gen {
            // A newer supervisor generation already owns the state.
            return;
        }
        *self.state.lock().unwrap() = status.clone();
        let _ = app.emit(EVENT_NAME, status);
    }
}

/// Check that a loopback port is free before we ask the child to bind it.
///
/// This is only a pre-flight courtesy so the user gets an immediate, readable
/// message instead of waiting out the readiness timeout; the authoritative
/// check is still the child's own bind.
fn check_loopback_port_free(port: u16) -> Result<(), String> {
    match std::net::TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => {
            drop(listener); // release immediately; the child binds it for real
            Ok(())
        }
        Err(_) => Err(format!(
            "Port {port} sedang dipakai proses lain. Hentikan proses itu atau pilih port lain."
        )),
    }
}

async fn verify_ready(base: &str, token: &str, instance: &str) -> Result<(), String> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(3)).build().map_err(|e| e.to_string())?;
    let response = client.get(format!("{base}/_bang/ready"))
        .bearer_auth(token)
        .send().await.map_err(|e| format!("Flow2API readiness check gagal: {e}"))?;
    if !response.status().is_success() { return Err(format!("Flow2API readiness check mengembalikan {}", response.status())); }
    let value: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    if value.get("protocol").and_then(|v| v.as_u64()) != Some(1)
        || value.get("instance").and_then(|v| v.as_str()) != Some(instance) {
        return Err("Identitas Flow2API pada endpoint tidak cocok.".into());
    }
    Ok(())
}

/// Put the child in a job object that kills the whole tree when De Story
/// exits, so a crashed app cannot leave a stray Python/browser behind. The
/// returned handle must stay open for the app's lifetime.
#[cfg(windows)]
fn own_process_tree(child: &Child) -> Result<isize, String> {
    use std::mem::zeroed;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

    let Some(pid) = child.id() else { return Err("PID Flow2API tidak tersedia".into()) };
    unsafe {
        let job: HANDLE = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() { return Err("Gagal membuat job object Flow2API".into()); }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) == 0
        {
            CloseHandle(job);
            return Err("Gagal mengatur job object Flow2API".into());
        }
        let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
        if process.is_null() {
            CloseHandle(job);
            return Err("Gagal membuka proses Flow2API".into());
        }
        let assigned = AssignProcessToJobObject(job, process);
        CloseHandle(process);
        if assigned == 0 {
            CloseHandle(job);
            return Err("Gagal menautkan proses Flow2API ke job object".into());
        }
        Ok(job as isize)
    }
}

#[cfg(not(windows))]
fn own_process_tree(_child: &Child) -> Result<isize, String> {
    Ok(0)
}

/// Release a job object handle once its child tree is being torn down. On
/// non-Windows there is nothing to close (the id 0 is a placeholder).
#[cfg(windows)]
fn close_job_handle(handle: isize) {
    if handle != 0 {
        use windows_sys::Win32::Foundation::CloseHandle;
        unsafe { CloseHandle(handle as _) };
    }
}
#[cfg(not(windows))]
fn close_job_handle(_handle: isize) {}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

async fn terminate_owned(child: &mut Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        unsafe { libc::kill(-(pid as i32), libc::SIGTERM); }
        if timeout(Duration::from_secs(3), child.wait()).await.is_err() {
            unsafe { libc::kill(-(pid as i32), libc::SIGKILL); }
            let _ = child.wait().await;
        }
        return;
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

fn current_target() -> &'static str {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "win-x64"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "mac-x64"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "mac-arm64"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux-x64"
    } else {
        "unsupported"
    }
}

/// The endpoint a Flow2API call should actually use.
///
/// Bundled mode uses the supervised child and its per-launch token, which is
/// never written to settings; external mode uses the saved URL and credential.
/// A remote operator's own `localhost:8000` server keeps working because the
/// mode is explicit — the URL alone never decides.
pub async fn resolve_target(
    mode: &str,
    external_base: &str,
    external_key: Option<&str>,
    manager: Option<&Flow2ApiManager>,
) -> Result<(String, Option<String>), String> {
    if mode == "external" {
        let base = if external_base.trim().is_empty() {
            "http://localhost:8000".to_string()
        } else {
            external_base.trim().trim_end_matches('/').to_string()
        };
        return Ok((base, external_key.map(|k| k.to_string())));
    }
    let manager = manager.ok_or("Flow2API internal belum siap.")?;
    let (base, token) = manager.wait_ready(Duration::from_secs(60)).await?;
    Ok((base, Some(token)))
}

/// Whether a generation call should wait for the bundled backend to come up.
pub fn is_bundled(mode: &str) -> bool {
    mode != "external"
}

/// Where the whole Flow2API data tree lives for this user
/// (`%APPDATA%\De Story\flow2api`): DB, config and the private key files.
pub fn data_dir() -> std::path::PathBuf {
    crate::paths::user_data_dir().join("flow2api")
}

/// Read a small private JSON file the backend generated in the data dir.
///
/// Only ever called for `admin-credentials.json` and `extension-key.json`.
/// Contents are handed to the app's own window on request and are never logged.
fn read_private_json(path: &std::path::Path) -> Option<serde_json::Value> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    let text = value.get(key)?.as_str()?.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Details needed to open Flow2API's own admin panel and sign in.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Flow2ApiPanelInfo {
    /// Loopback base URL of the running backend.
    pub base_url: String,
    pub login_url: String,
    pub username: String,
    /// Generated once by the backend, stored in the data dir, never logged.
    pub password: String,
}

/// Connection details for the bundled Chrome extension CAPTCHA worker.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Flow2ApiExtensionInfo {
    /// `ws://127.0.0.1:<port>/captcha_ws`.
    pub ws_url: String,
    /// The stable extension credential; `None` until the backend has been
    /// started at least once in embedded mode (it writes the file at startup).
    pub key: Option<String>,
    /// Absolute path of the bundled `extension/` folder to load unpacked.
    pub extension_dir: Option<String>,
    /// Whether the address is guaranteed to survive a restart.
    pub fixed_port: bool,
}

/// The loopback port embedded in a `http://127.0.0.1:<port>` base URL.
fn port_of(base: &str) -> Option<u16> {
    base.rsplit(':').next()?.trim().parse().ok()
}

impl Flow2ApiManager {
    /// The live loopback address, once the child has reported one.
    ///
    /// Unlike [`Self::endpoint`] this does not need the launch token, because
    /// the address is knowable the moment the child binds it.
    fn base_url(&self) -> Option<String> {
        self.endpoint.lock().unwrap().clone()
    }

    /// Address of the CAPTCHA WebSocket worker endpoint.
    ///
    /// Prefers the live port; falls back to the opt-in fixed port while the
    /// backend is still starting, since that is the address it will bind.
    pub fn captcha_ws_url(&self) -> Result<String, String> {
        let port = self.base_url().as_deref().and_then(port_of).or_else(|| {
            let fixed = self.fixed_port();
            (fixed != 0).then_some(fixed)
        });
        port.map(|port| format!("ws://127.0.0.1:{port}/captcha_ws"))
            .ok_or_else(|| "Flow2API internal belum siap; alamat WebSocket belum tersedia.".to_string())
    }

    /// Admin-panel address and credentials, as the backend stored them.
    pub fn panel_info(&self) -> Result<Flow2ApiPanelInfo, String> {
        let base = self.base_url().ok_or(
            "Flow2API internal belum siap. Tunggu beberapa saat lalu coba lagi.",
        )?;
        let credentials = read_private_json(&data_dir().join("admin-credentials.json"))
            .ok_or("Kredensial panel admin Flow2API belum ada. Jalankan Flow2API internal sekali lagi.")?;
        let username = json_string(&credentials, "username").unwrap_or_else(|| "admin".into());
        let password = json_string(&credentials, "password")
            .ok_or("Berkas kredensial panel admin Flow2API tidak berisi sandi.")?;
        Ok(Flow2ApiPanelInfo {
            login_url: format!("{base}/login"),
            base_url: base,
            username,
            password,
        })
    }

    /// Stable extension credential, if the backend has provisioned it.
    pub fn extension_key(&self) -> Option<String> {
        let value = read_private_json(&data_dir().join("extension-key.json"))?;
        json_string(&value, "key")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn external_mode_uses_the_saved_url_and_credential() {
        let (base, key) = resolve_target("external", "https://flow.example.com/", Some("k1"), None)
            .await
            .unwrap();
        assert_eq!(base, "https://flow.example.com");
        assert_eq!(key.as_deref(), Some("k1"));
    }

    #[tokio::test]
    async fn external_mode_without_a_url_falls_back_to_the_default() {
        let (base, key) = resolve_target("external", "  ", None, None).await.unwrap();
        assert_eq!(base, "http://localhost:8000");
        assert!(key.is_none());
    }

    #[tokio::test]
    async fn bundled_mode_without_a_manager_fails_with_an_actionable_error() {
        let error = resolve_target("bundled", "ignored", None, None).await.unwrap_err();
        assert!(error.contains("belum siap"), "unexpected: {error}");
    }

    #[tokio::test]
    async fn bundled_mode_never_returns_the_external_url() {
        // Even with a saved external URL configured, bundled mode must only
        // ever answer with the supervised loopback endpoint — here it waits for
        // a child that never reports ready, rather than handing back the URL.
        let manager = Flow2ApiManager::new();
        let outcome = tokio::time::timeout(
            Duration::from_secs(3),
            resolve_target("bundled", "http://10.0.0.9:8000", None, Some(&manager)),
        )
        .await;
        assert!(outcome.is_err(), "resolver returned instead of waiting for the bundled backend");
        assert_eq!(
            manager.endpoint().unwrap_err(),
            "Flow2API internal belum siap. Tunggu beberapa saat lalu coba lagi."
        );
    }

    #[test]
    fn only_the_literal_external_string_selects_external_mode() {
        assert!(is_bundled("bundled"));
        assert!(is_bundled(""));
        assert!(is_bundled("Bundled"));
        assert!(!is_bundled("external"));
    }

    #[tokio::test]
    async fn wait_ready_reports_failed_state_immediately() {
        let manager = Flow2ApiManager::new();
        // Force a failed state as the supervisor would publish it.
        *manager.state.lock().unwrap() = Flow2ApiStatus {
            state: "failed".into(),
            base_url: None,
            version: None,
            message: Some("Runtime Flow2API tidak cocok.".into()),
        };
        let error = manager.wait_ready(Duration::from_secs(5)).await.unwrap_err();
        assert!(error.contains("tidak cocok"), "unexpected: {error}");
    }

    #[test]
    fn a_bound_loopback_port_is_reported_as_taken() {
        let held = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = held.local_addr().unwrap().port();

        let error = check_loopback_port_free(port).unwrap_err();
        assert!(error.contains(&port.to_string()), "unexpected: {error}");
        assert!(error.contains("Port"), "unexpected: {error}");

        // Releasing it must make the port available again, otherwise the check
        // would be a one-way latch that blocks every retry.
        drop(held);
        assert!(check_loopback_port_free(port).is_ok());
    }

    #[test]
    fn the_ws_url_falls_back_to_the_fixed_port_before_the_backend_is_ready() {
        let manager = Flow2ApiManager::new();
        // Nothing running and no fixed port: no address is knowable yet.
        assert!(manager.captcha_ws_url().is_err());

        // With a fixed port the extension can be configured before startup.
        manager.set_fixed_port(8765);
        assert_eq!(manager.captcha_ws_url().unwrap(), "ws://127.0.0.1:8765/captcha_ws");

        // A live endpoint wins over the configured port.
        *manager.endpoint.lock().unwrap() = Some("http://127.0.0.1:54321".into());
        assert_eq!(manager.captcha_ws_url().unwrap(), "ws://127.0.0.1:54321/captcha_ws");
    }

    #[test]
    fn panel_info_without_a_running_backend_fails_instead_of_inventing_an_address() {
        // No endpoint is known yet, and panel_info must not fall back to the
        // configured external URL: that server has its own credentials.
        let manager = Flow2ApiManager::new();
        assert!(manager.panel_info().is_err());
    }

    #[test]
    fn private_json_helpers_read_what_the_backend_writes() {
        // The extension key file is `{"schema":1,"key":"..."}`; a blank or
        // absent key must read as None rather than an empty credential.
        let value: serde_json::Value = serde_json::from_str(r#"{"key":"abc123"}"#).unwrap();
        assert_eq!(json_string(&value, "key").as_deref(), Some("abc123"));
        assert_eq!(json_string(&value, "missing"), None);

        let blank: serde_json::Value = serde_json::from_str(r#"{"key":"   "}"#).unwrap();
        assert_eq!(json_string(&blank, "key"), None);

        let missing = std::path::Path::new("tidak-ada-file-ini.json");
        assert!(read_private_json(missing).is_none());
    }

    #[test]
    fn a_port_is_read_back_out_of_a_loopback_base_url() {
        assert_eq!(port_of("http://127.0.0.1:8000"), Some(8000));
        assert_eq!(port_of("http://127.0.0.1:54321"), Some(54321));
        assert_eq!(port_of("http://127.0.0.1"), None);
        assert_eq!(port_of(""), None);
    }
}


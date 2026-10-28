use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use serde_json::json;
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use crate::db::repo::{get_job, update_job};
use crate::db::DbPool;
use crate::services::higgsfield::cancel_request;

pub enum QueueName {
    Higgsfield,
    /// Character sheets. A queue of their own so sheet jobs are never starved
    /// by clip jobs that hold Higgsfield permits while waiting for sheets.
    Sheet,
    Tts,
    Whisper,
    Export,
}

pub struct TaskCtx {
    pub job_id: String,
    pub token: CancellationToken,
    db: DbPool,
    app: AppHandle,
    last_emit: Arc<Mutex<Instant>>,
}

impl TaskCtx {
    pub fn progress(&self, p: f64, message: Option<&str>) {
        let p_clamped = p.clamp(0.0, 0.99);
        let patch = json!({
            "progress": p_clamped,
            "message": message
        });

        let conn = self.db.lock().unwrap();
        if let Ok(j) = update_job(&conn, &self.job_id, patch) {
            let mut last = self.last_emit.lock().unwrap();
            if last.elapsed() > Duration::from_millis(250) || p >= 0.99 {
                *last = Instant::now();
                let _ = self.app.emit("event:job", j);
            }
        }
    }

    pub fn set_remote(&self, id: &str) {
        let conn = self.db.lock().unwrap();
        let _ = update_job(&conn, &self.job_id, json!({ "remoteId": id }));
    }
}

/// The text a panicking task carried, so a failed job can say what broke
/// instead of only "task failed". A non-panic join failure is a task that was
/// stopped, which keeps its own wording.
fn panic_text(join_err: tokio::task::JoinError) -> String {
    if !join_err.is_panic() {
        return "Tugas dihentikan".to_string();
    }
    if let Ok(payload) = join_err.try_into_panic() {
        if let Some(s) = payload.downcast_ref::<&str>() {
            return (*s).to_string();
        }
        if let Some(s) = payload.downcast_ref::<String>() {
            return s.clone();
        }
    }
    "Tugas berhenti karena kesalahan tak terduga".to_string()
}

pub struct JobManager {
    sem_hf: Arc<Semaphore>,
    sem_sheet: Arc<Semaphore>,
    sem_tts: Arc<Semaphore>,
    sem_whisper: Arc<Semaphore>,
    sem_export: Arc<Semaphore>,
    tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,
    db: DbPool,
    app: AppHandle,
}

impl JobManager {
    pub fn new(db: DbPool, app: AppHandle) -> Self {
        Self {
            sem_hf: Arc::new(Semaphore::new(3)),
            sem_sheet: Arc::new(Semaphore::new(2)),
            sem_tts: Arc::new(Semaphore::new(2)),
            sem_whisper: Arc::new(Semaphore::new(1)),
            sem_export: Arc::new(Semaphore::new(1)),
            tokens: Arc::new(Mutex::new(HashMap::new())),
            db,
            app,
        }
    }

    pub fn spawn_task<F, Fut>(&self, job_id: &str, queue: QueueName, f: F)
    where
        F: FnOnce(TaskCtx) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<Option<String>, String>> + Send + 'static,
    {
        let token = CancellationToken::new();
        self.tokens.lock().unwrap().insert(job_id.to_string(), token.clone());

        let sem = match queue {
            QueueName::Higgsfield => self.sem_hf.clone(),
            QueueName::Sheet => self.sem_sheet.clone(),
            QueueName::Tts => self.sem_tts.clone(),
            QueueName::Whisper => self.sem_whisper.clone(),
            QueueName::Export => self.sem_export.clone(),
        };

        let j_id = job_id.to_string();
        let tokens_map = self.tokens.clone();
        let db = self.db.clone();
        let app = self.app.clone();

        let ctx = TaskCtx {
            job_id: j_id.clone(),
            token: token.clone(),
            db: db.clone(),
            app: app.clone(),
            last_emit: Arc::new(Mutex::new(Instant::now())),
        };

        // Sync Tauri commands run on the UI thread, outside Tokio's runtime context.
        tauri::async_runtime::spawn(async move {
            let permit = sem.acquire().await;
            if token.is_cancelled() {
                drop(permit);
                tokens_map.lock().unwrap().remove(&j_id);
                let conn = db.lock().unwrap();
                if let Ok(j) = update_job(&conn, &j_id, json!({ "status": "canceled", "message": null })) {
                    let _ = app.emit("event:job", j);
                }
                return;
            }

            // Running
            {
                let conn = db.lock().unwrap();
                if let Ok(j) = update_job(&conn, &j_id, json!({ "status": "running", "progress": 0.02 })) {
                    let _ = app.emit("event:job", j);
                }
            }

            let res = match tauri::async_runtime::spawn(f(ctx)).await {
                Ok(res) => res,
                // A task that panics used to skip everything below, leaving the
                // job "running" forever and never returning the queue permit, so
                // the next two narration jobs queued behind it waited forever too.
                Err(join_err) => match join_err {
                    tauri::Error::JoinError(je) => {
                        Err(format!("Kesalahan internal: {}", panic_text(je)))
                    }
                    other => Err(other.to_string()),
                },
            };
            drop(permit);
            tokens_map.lock().unwrap().remove(&j_id);

            let conn = db.lock().unwrap();
            match res {
                Ok(asset_id) => {
                    let mut patch = json!({ "status": "done", "progress": 1.0 });
                    if let Some(aid) = asset_id {
                        patch["resultAssetId"] = json!(aid);
                    }
                    if let Ok(j) = update_job(&conn, &j_id, patch) {
                        let _ = app.emit("event:job", j);
                    }
                }
                Err(err) => {
                    let is_canceled = token.is_cancelled();
                    let patch = if is_canceled {
                        json!({ "status": "canceled", "message": null })
                    } else {
                        json!({ "status": "failed", "error": err, "message": null })
                    };
                    if let Ok(j) = update_job(&conn, &j_id, patch) {
                        let _ = app.emit("event:job", j);
                    }
                }
            }
        });
    }

    pub async fn cancel_job(&self, job_id: &str, hf_key: Option<&str>) {
        let job = {
            let conn = self.db.lock().unwrap();
            get_job(&conn, job_id)
        };

        if let Some(j) = job {
            let remote_id = j.get("remoteId").and_then(|x| x.as_str());
            let provider = j.get("provider").and_then(|x| x.as_str()).unwrap_or("");

            if provider == "higgsfield" {
                if let (Some(rid), Some(key)) = (remote_id, hf_key) {
                    cancel_request(key, rid).await;
                }
            }
        }

        let token = self.tokens.lock().unwrap().remove(job_id);
        if let Some(t) = token {
            t.cancel();
        } else {
            let conn = self.db.lock().unwrap();
            if let Ok(j) = update_job(&conn, job_id, json!({ "status": "canceled", "message": null })) {
                let _ = self.app.emit("event:job", j);
            }
        }
    }
}

//! Persistence for diagnosing silent exits.
//!
//! Release builds set `windows_subsystem = "windows"` (see `main.rs`), so a panic
//! message has nowhere to go and the process just disappears. Every panic and
//! every app-lifecycle transition is appended to `de-story.log` in the user
//! data dir instead, so a "force close" can be told apart from a normal quit.

use std::io::Write;

pub fn log_path() -> std::path::PathBuf {
    crate::paths::user_data_dir().join("de-story.log")
}

pub fn log_line(msg: &str) {
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let thread = std::thread::current()
        .name()
        .unwrap_or("unnamed")
        .to_string();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = writeln!(f, "[{stamp}] [{thread}] {msg}");
    }
}

pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic payload>".to_string()
        };
        log_line(&format!("PANIC at {loc}: {msg}"));
        default_hook(info);
    }));
}

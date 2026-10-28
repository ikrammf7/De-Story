use rusqlite::{backup::Backup, Connection, OpenFlags};
use std::fs;
use std::path::{Path, PathBuf};

/// Prepare the private Flow2API data root. `source` is the installation to
/// import from, chosen by the caller (see [`discover_source`]); `None` means
/// start empty. The source is always treated as read-only.
pub fn prepare(data_dir: &Path, source: Option<&Path>) -> Result<String, String> {
    let target = data_dir.join("flow2api");
    fs::create_dir_all(&target).map_err(|e| format!("Tidak dapat membuat data Flow2API: {e}"))?;
    set_private_dir(&target)?;

    let managed_db = target.join("data").join("flow.db");
    if managed_db.exists() {
        return Ok("Data Flow2API internal sudah siap.".into());
    }

    let Some(src) = source.filter(|p| p.is_dir()) else {
        ensure_layout(&target)?;
        return Ok("Data Flow2API baru disiapkan; tidak ada instalasi lama yang ditemukan.".into());
    };
    let source_db = src.join("data").join("flow.db");
    if !source_db.is_file() {
        ensure_layout(&target)?;
        return Ok("Folder Flow2API ditemukan tanpa database; data baru disiapkan.".into());
    }

    let stage = target.join(format!(".migration-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&stage).map_err(|e| format!("Gagal membuat area migrasi: {e}"))?;
    let result = migrate_into(&src, &stage);
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }

    let stage_data = stage.join("data");
    let stage_config = stage.join("config");
    let final_data = target.join("data");
    let final_config = target.join("config");
    if final_data.exists() || final_config.exists() {
        let _ = fs::remove_dir_all(&stage);
        return Err("Data Flow2API tujuan sudah ada sebagian; migrasi tidak akan menimpa data.".into());
    }

    fs::rename(&stage_data, &final_data).map_err(|e| format!("Gagal menerbitkan database Flow2API: {e}"))?;
    if let Err(e) = fs::rename(&stage_config, &final_config) {
        let _ = fs::remove_dir_all(&final_data);
        let _ = fs::remove_dir_all(&stage);
        return Err(format!("Gagal menerbitkan konfigurasi Flow2API: {e}"));
    }
    let _ = fs::remove_dir_all(stage);
    set_private_dir(&final_data)?;
    set_private_dir(&final_config)?;
    Ok(format!("Database dan konfigurasi Flow2API dimigrasikan dari {}. Data sumber tidak diubah.", src.display()))
}

/// Where an existing standalone Flow2API might live, for the one-time import.
/// `DE_STORY_FLOW2API_IMPORT` (or `BANG_STORY_FLOW2API_IMPORT`) wins so tests and
/// power users can point at a specific folder; otherwise only the well-known
/// Windows location is probed.
pub fn discover_source() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("DE_STORY_FLOW2API_IMPORT").or_else(|| std::env::var_os("BANG_STORY_FLOW2API_IMPORT")) {
        return Some(PathBuf::from(path));
    }
    #[cfg(windows)]
    {
        let known = PathBuf::from(r"E:\Tools\flow2api");
        if known.is_dir() { return Some(known); }
    }
    None
}

fn migrate_into(source: &Path, stage: &Path) -> Result<(), String> {
    let data = stage.join("data");
    let config = stage.join("config");
    fs::create_dir_all(&data).map_err(|e| e.to_string())?;
    fs::create_dir_all(&config).map_err(|e| e.to_string())?;

    let source_db = source.join("data").join("flow.db");
    let from = Connection::open_with_flags(&source_db, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("Tidak dapat membuka database Flow2API sumber: {e}"))?;
    let mut to = Connection::open(data.join("flow.db")).map_err(|e| e.to_string())?;
    {
        let backup = Backup::new(&from, &mut to).map_err(|e| format!("Gagal menyalin database Flow2API: {e}"))?;
        backup.run_to_completion(128, std::time::Duration::from_millis(25), None)
            .map_err(|e| format!("Backup database Flow2API gagal: {e}"))?;
    }
    let check: String = to.query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|e| format!("Validasi database migrasi gagal: {e}"))?;
    if check != "ok" { return Err(format!("Database Flow2API sumber tidak valid: {check}")); }
    drop(to);
    drop(from);

    let source_config = source.join("config").join("setting.toml");
    let fallback = source.join("config").join("setting_example.toml");
    let config_source = if source_config.is_file() { source_config } else { fallback };
    if config_source.is_file() {
        fs::copy(config_source, config.join("setting.toml"))
            .map_err(|e| format!("Gagal menyalin konfigurasi Flow2API: {e}"))?;
    }
    for dir in ["src/data", "src/tmp", "tmp"] {
        fs::create_dir_all(stage.join(dir)).map_err(|e| e.to_string())?;
    }
    set_private_file(&data.join("flow.db"))?;
    if config.join("setting.toml").exists() { set_private_file(&config.join("setting.toml"))?; }
    Ok(())
}

fn ensure_layout(root: &Path) -> Result<(), String> {
    for dir in ["data", "config", "src/data", "src/tmp", "tmp"] {
        fs::create_dir_all(root.join(dir)).map_err(|e| format!("Gagal menyiapkan {}: {e}", dir))?;
        set_private_dir(&root.join(dir))?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_private_dir(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())
}
#[cfg(not(unix))]
fn set_private_dir(_path: &Path) -> Result<(), String> { Ok(()) }

#[cfg(unix)]
fn set_private_file(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())
}
#[cfg(not(unix))]
fn set_private_file(_path: &Path) -> Result<(), String> { Ok(()) }

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn source(root: &Path) -> PathBuf {
        fs::create_dir_all(root.join("data")).unwrap();
        let conn = Connection::open(root.join("data/flow.db")).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE tokens(id INTEGER PRIMARY KEY, token TEXT); INSERT INTO tokens(token) VALUES ('keep-me');").unwrap();
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(root.join("config/setting.toml"), "[global]\nadmin_username='admin'\n").unwrap();
        root.to_path_buf()
    }

    #[test]
    fn creates_fresh_layout_without_source() {
        let d = tempdir().unwrap();
        let message = prepare(&d.path().join("app"), None).unwrap();
        assert!(message.contains("tidak ada"));
        assert!(d.path().join("app/flow2api/data").is_dir());
        // No database is invented when there is nothing to import.
        assert!(!d.path().join("app/flow2api/data/flow.db").exists());
    }

    #[test]
    fn folder_without_a_database_does_not_block_a_fresh_start() {
        let d = tempdir().unwrap();
        let src = d.path().join("source");
        fs::create_dir_all(&src).unwrap();
        let message = prepare(&d.path().join("app"), Some(&src)).unwrap();
        assert!(message.contains("tanpa database"));
        assert!(d.path().join("app/flow2api/data").is_dir());
    }

    #[test]
    fn discovery_prefers_the_env_override() {
        // The hardcoded Windows path must never win over an explicit request.
        let d = tempdir().unwrap();
        std::env::set_var("BANG_STORY_FLOW2API_IMPORT", d.path());
        let found = discover_source();
        std::env::remove_var("BANG_STORY_FLOW2API_IMPORT");
        assert_eq!(found.as_deref(), Some(d.path()));
    }

    #[test]
    fn migrates_wal_and_never_overwrites_destination() {
        let d = tempdir().unwrap();
        let src = source(&d.path().join("source"));
        let data = d.path().join("app");
        prepare(&data, Some(&src)).unwrap();
        let migrated = Connection::open(data.join("flow2api/data/flow.db")).unwrap();
        let value: String = migrated.query_row("SELECT token FROM tokens", [], |r| r.get(0)).unwrap();
        assert_eq!(value, "keep-me");
        migrated.execute("UPDATE tokens SET token='new-value'", []).unwrap();
        prepare(&data, Some(&src)).unwrap();
        let again = Connection::open(data.join("flow2api/data/flow.db")).unwrap();
        let value: String = again.query_row("SELECT token FROM tokens", [], |r| r.get(0)).unwrap();
        assert_eq!(value, "new-value");
        let original = Connection::open(src.join("data/flow.db")).unwrap();
        let value: String = original.query_row("SELECT token FROM tokens", [], |r| r.get(0)).unwrap();
        assert_eq!(value, "keep-me");
    }

    #[test]
    fn corrupt_source_fails_and_preserves_source() {
        let d = tempdir().unwrap();
        let src = d.path().join("source");
        fs::create_dir_all(src.join("data")).unwrap();
        fs::write(src.join("data/flow.db"), b"not sqlite").unwrap();
        assert!(prepare(&d.path().join("app"), Some(&src)).is_err());
        assert_eq!(fs::read(src.join("data/flow.db")).unwrap(), b"not sqlite");
    }
}


#[cfg(windows)]
use std::ptr::null_mut;
#[cfg(windows)]
use windows_sys::Win32::Foundation::LocalFree;
#[cfg(windows)]
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB as CRYPTOAPI_BLOB, CRYPTPROTECT_UI_FORBIDDEN,
};
use serde::{Deserialize, Serialize};

pub const PROVIDERS: &[&str] = &[
    "higgsfield",
    "gemini",
    "elevenlabs",
    "openrouter",
    "groq",
    "custom",
    "flow2api",
];

pub fn provider_name(p: &str) -> &'static str {
    match p {
        "higgsfield" => "Higgsfield",
        "gemini" => "Google Gemini",
        "elevenlabs" => "ElevenLabs",
        "openrouter" => "OpenRouter",
        "groq" => "Groq",
        "custom" => "endpoint custom",
        "flow2api" => "Google Flow (Flow2API)",
        _ => "Layanan",
    }
}

pub fn mask(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 10 {
        return "•".repeat(chars.len().max(4));
    }
    let lead_len = 8.min(chars.len() / 3);
    let lead: String = chars[..lead_len].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{lead}{}{tail}", "•".repeat(10))
}

pub fn normalized_gemini_keys(raw: &str) -> Vec<String> {
    crate::services::gemini::parse_key_pool(raw)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiKeyTestResult {
    pub index: usize,
    pub preview: String,
    pub ok: bool,
    pub message: Option<String>,
}

#[cfg(windows)]
pub fn encrypt_dpapi(plaintext: &str) -> Result<Vec<u8>, String> {
    let bytes = plaintext.as_bytes();
    let mut input = CRYPTOAPI_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPTOAPI_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };

    let success = unsafe {
        CryptProtectData(
            &mut input,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };

    if success == 0 {
        return Err("CryptProtectData failed".to_string());
    }

    let result = unsafe {
        let slice = std::slice::from_raw_parts(output.pbData, output.cbData as usize);
        let vec = slice.to_vec();
        LocalFree(output.pbData as _);
        vec
    };

    Ok(result)
}

#[cfg(windows)]
pub fn decrypt_dpapi_raw(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut input = CRYPTOAPI_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPTOAPI_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };

    let success = unsafe {
        CryptUnprotectData(
            &mut input,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };

    if success != 0 {
        let slice = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) };
        let vec = slice.to_vec();
        unsafe { LocalFree(output.pbData as _) };
        Some(vec)
    } else {
        None
    }
}

#[cfg(windows)]
fn get_chromium_master_key() -> Option<Vec<u8>> {
    let appdata = std::env::var_os("APPDATA")?;
    let path = std::path::PathBuf::from(&appdata).join("De Story").join("Local State");
    let legacy = std::path::PathBuf::from(&appdata).join("Bang Story").join("Local State");
    let content = std::fs::read_to_string(path).or_else(|_| std::fs::read_to_string(legacy)).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let enc_key_b64 = json.get("os_crypt")?.get("encrypted_key")?.as_str()?;
    use base64::Engine;
    let enc_key = base64::engine::general_purpose::STANDARD.decode(enc_key_b64).ok()?;
    if !enc_key.starts_with(b"DPAPI") || enc_key.len() <= 5 {
        return None;
    }
    decrypt_dpapi_raw(&enc_key[5..])
}

#[cfg(windows)]
fn decrypt_electron_v10(ciphertext: &[u8]) -> Option<String> {
    if !ciphertext.starts_with(b"v10") || ciphertext.len() < 3 + 12 + 16 {
        return None;
    }
    let master_key = get_chromium_master_key()?;
    if master_key.len() != 32 {
        return None;
    }
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Key, Nonce,
    };
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&master_key));
    let nonce = Nonce::from_slice(&ciphertext[3..15]);
    let payload = &ciphertext[15..];
    let decrypted = cipher.decrypt(nonce, payload).ok()?;
    String::from_utf8(decrypted).ok()
}

#[cfg(windows)]
pub fn decrypt_dpapi(ciphertext: &[u8]) -> Result<String, String> {
    if ciphertext.is_empty() {
        return Ok(String::new());
    }

    // 1. Direct DPAPI decryption (standard native Tauri format)
    if let Some(raw) = decrypt_dpapi_raw(ciphertext) {
        if let Ok(s) = String::from_utf8(raw) {
            return Ok(s);
        }
    }

    // 2. Electron safeStorage on Windows (AES-256-GCM prefixed with "v10")
    if ciphertext.starts_with(b"v10") {
        return decrypt_electron_v10(ciphertext)
            .ok_or_else(|| "Electron secret could not be decrypted".to_string());
    }

    // 3. Fallback: raw UTF-8 string if stored unencrypted
    if let Ok(s) = String::from_utf8(ciphertext.to_vec()) {
        return Ok(s);
    }

    Err("CryptUnprotectData failed".to_string())
}

// The public names are retained for callers; Unix secrets use a keyring-backed
// AES-256-GCM envelope, not DPAPI. Windows' on-disk formats remain unchanged.
#[cfg(not(windows))]
pub fn encrypt_dpapi(plaintext: &str) -> Result<Vec<u8>, String> {
    let key = platform_master_key(true)?;
    envelope::encrypt(&key, plaintext)
}

#[cfg(not(windows))]
pub fn decrypt_dpapi(ciphertext: &[u8]) -> Result<String, String> {
    // Reject plaintext/foreign/truncated blobs before touching the keyring.
    envelope::validate(ciphertext)?;
    let key = platform_master_key(false)?;
    envelope::decrypt(&key, ciphertext)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn platform_master_key(create_if_missing: bool) -> Result<[u8; 32], String> {
    use aes_gcm::aead::{rand_core::RngCore, OsRng};
    // Serialize first-use initialization in this single-instance application.
    static KEYRING_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = KEYRING_LOCK.lock().map_err(|_| "OS keyring lock poisoned".to_string())?;
    let unavailable = |e| format!("OS keyring unavailable or locked; unlock your login keychain/Secret Service: {e}");
    let entry = keyring::Entry::new("De Story", "secret-master-key-v1")
        .or_else(|_| keyring::Entry::new("Bang Story", "secret-master-key-v1"))
        .map_err(unavailable)?;
    match entry.get_secret() {
        Ok(key) => key.try_into().map_err(|_| "OS keyring master key is invalid (expected 32 bytes); it was not replaced".to_string()),
        Err(keyring::Error::NoEntry) if create_if_missing => {
            let mut key = [0u8; 32];
            OsRng.try_fill_bytes(&mut key).map_err(|e| format!("Secure randomness unavailable: {e}"))?;
            entry.set_secret(&key).map_err(unavailable)?;
            Ok(key)
        }
        // Decryption must never generate a replacement key for existing data.
        Err(keyring::Error::NoEntry) => Err("OS keyring master key is missing; existing secrets cannot be decrypted. Restore the original keychain.".to_string()),
        Err(e) => Err(unavailable(e)),
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn platform_master_key(_create_if_missing: bool) -> Result<[u8; 32], String> {
    Err("OS keyring unavailable: secret storage is unsupported on this platform".to_string())
}

#[cfg(any(not(windows), test))]
mod envelope {
    use aes_gcm::{
        aead::{rand_core::RngCore, Aead, KeyInit, OsRng, Payload},
        Aes256Gcm, Nonce,
    };

    // Header (including version), 96-bit random nonce, ciphertext and 128-bit tag.
    const HEADER: &[u8] = b"DeStorySecret\x01";
    const LEGACY_HEADER: &[u8] = b"BangStorySecret\x01";
    const NONCE_LEN: usize = 12;
    const TAG_LEN: usize = 16;

    pub(super) fn validate(blob: &[u8]) -> Result<(), String> {
        let is_valid = (blob.starts_with(HEADER) && blob.len() >= HEADER.len() + NONCE_LEN + TAG_LEN)
            || (blob.starts_with(LEGACY_HEADER) && blob.len() >= LEGACY_HEADER.len() + NONCE_LEN + TAG_LEN);
        if !is_valid {
            return Err("Unsupported or malformed encrypted secret (expected De Story envelope)".to_string());
        }
        Ok(())
    }

    pub(super) fn encrypt(key: &[u8; 32], plaintext: &str) -> Result<Vec<u8>, String> {
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "Invalid secret encryption key")?;
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.try_fill_bytes(&mut nonce).map_err(|e| format!("Secure randomness unavailable: {e}"))?;
        let ciphertext = cipher.encrypt(Nonce::from_slice(&nonce), Payload {
            msg: plaintext.as_bytes(), aad: HEADER,
        }).map_err(|_| "Secret encryption failed")?;
        let mut blob = Vec::with_capacity(HEADER.len() + NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(HEADER);
        blob.extend_from_slice(&nonce);
        blob.extend_from_slice(&ciphertext);
        Ok(blob)
    }

    pub(super) fn decrypt(key: &[u8; 32], blob: &[u8]) -> Result<String, String> {
        validate(blob)?;
        let header = if blob.starts_with(HEADER) { HEADER } else { LEGACY_HEADER };
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| "Invalid secret encryption key")?;
        let nonce_end = header.len() + NONCE_LEN;
        let plaintext = cipher.decrypt(Nonce::from_slice(&blob[header.len()..nonce_end]), Payload {
            msg: &blob[nonce_end..], aad: header,
        }).map_err(|_| "Secret authentication failed (wrong key or damaged data)")?;
        String::from_utf8(plaintext).map_err(|_| "Decrypted secret is not UTF-8".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyStatus {
    pub provider: String,
    pub configured: bool,
    pub key_count: usize,
    pub last_ok: Option<bool>,
    pub last_message: Option<String>,
    pub checked_at: Option<i64>,
    pub preview: Option<String>,
    pub unreadable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyTestResult {
    pub ok: bool,
    pub message: Option<String>,
}

pub use crate::db::repo::{clear_secret, key_statuses, reveal_secret, set_secret};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(windows), ignore = "requires a live OS keyring")]
    fn test_dpapi_roundtrip() {
        let text = "AIzaSyTestKey123456789";
        let encrypted = encrypt_dpapi(text).expect("encryption should work");
        assert!(!encrypted.is_empty());
        let decrypted = decrypt_dpapi(&encrypted).expect("decryption should work");
        assert_eq!(decrypted, text);
    }

    #[test]
    fn mask_handles_unicode_boundaries() {
        assert_eq!(mask("é🔑abcdefghi"), "é🔑a••••••••••fghi");
    }

    #[test]
    fn aes_envelope_roundtrip_and_tamper_rejection() {
        let key = [37u8; 32];
        let blob = envelope::encrypt(&key, "unicode 🔑 secret").unwrap();
        assert_ne!(blob, "unicode 🔑 secret".as_bytes());
        assert_eq!(envelope::decrypt(&key, &blob).unwrap(), "unicode 🔑 secret");
        assert_ne!(blob, envelope::encrypt(&key, "unicode 🔑 secret").unwrap());
        assert!(envelope::decrypt(&[42u8; 32], &blob).is_err());
        let mut tampered = blob.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(envelope::decrypt(&key, &tampered).is_err());
        assert!(envelope::decrypt(&key, b"raw plaintext").is_err());
        assert!(envelope::decrypt(&key, &blob[..blob.len() - 20]).is_err());
    }

    #[test]
    #[cfg_attr(not(windows), ignore = "requires a live OS keyring")]
    fn test_key_statuses_roundtrip() {
        let conn = rusqlite::Connection::open_in_memory().expect("open memory db");
        for m in crate::db::MIGRATIONS {
            conn.execute_batch(m).expect("migration");
        }
        set_secret(&conn, "gemini", "AIzaSyTest123").expect("set secret");
        crate::db::repo::record_check(&conn, "gemini", true, Some("Terhubung")).expect("record check");

        let statuses = key_statuses(&conn).expect("key statuses");
        let gemini = statuses.iter().find(|s| s.provider == "gemini").expect("gemini status");
        assert!(gemini.configured);
        assert_eq!(gemini.last_ok, Some(true));
        assert!(gemini.preview.is_some());
    }

    fn memory_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        for migration in crate::db::MIGRATIONS {
            conn.execute_batch(migration).unwrap();
        }
        conn
    }

    #[test]
    #[cfg_attr(not(windows), ignore = "requires a live OS keyring")]
    fn pool_storage_append_counts_and_cache_invalidation() {
        use crate::db::repo::*;
        let conn = memory_db();
        let seed_cache = || {
            for source in ["gemini", "gemini-tts", "groq"] {
                write_cache(&conn, &format!("models:{source}"), &serde_json::json!([])).unwrap();
            }
        };
        seed_cache();
        set_secret(&conn, "gemini", " first-key-123456 \nsecond-key-789\nfirst-key-123456").unwrap();
        assert!(read_cache(&conn, "models:gemini").is_none());
        assert!(read_cache(&conn, "models:gemini-tts").is_none());
        assert!(read_cache(&conn, "models:groq").is_some());
        let (blob, keys) = gemini_secret_snapshot(&conn).unwrap().unwrap();
        assert_eq!(keys, vec!["first-key-123456", "second-key-789"]);
        assert_eq!(decrypt_dpapi(&blob).unwrap(), keys.join("\n"));
        assert_ne!(blob, keys.join("\n").as_bytes());

        seed_cache();
        append_gemini_keys(&conn, &["second-key-789\nthird-key-123".into()]).unwrap();
        assert_eq!(get_secret(&conn, "gemini").unwrap(), "first-key-123456\nsecond-key-789\nthird-key-123");
        assert!(read_cache(&conn, "models:gemini").is_none());
        assert!(read_cache(&conn, "models:gemini-tts").is_none());
        set_secret(&conn, "groq", "single-key").unwrap();
        let statuses = key_statuses(&conn).unwrap();
        let gemini = statuses.iter().find(|s| s.provider == "gemini").unwrap();
        assert_eq!(gemini.key_count, 3);
        assert_eq!(gemini.preview, Some(mask("first-key-123456")));
        for status in &statuses {
            let expected = match status.provider.as_str() { "gemini" => 3, "groq" => 1, _ => 0 };
            assert_eq!(status.key_count, expected);
            assert_eq!(serde_json::to_value(status).unwrap()["keyCount"], expected);
        }
        seed_cache();
        clear_secret(&conn, "gemini").unwrap();
        assert!(read_cache(&conn, "models:gemini").is_none());
        assert!(read_cache(&conn, "models:gemini-tts").is_none());
    }

    #[test]
    fn append_rejects_unreadable_secret_without_overwriting() {
        use crate::db::repo::*;
        let conn = memory_db();
        // Invalid UTF-8 plus a truncated Electron header exercises both failure
        // paths without consulting the user's Local State or actual database.
        for blob in [vec![255, 254], b"v10broken".to_vec()] {
            conn.execute("INSERT OR REPLACE INTO secrets (provider, value) VALUES ('gemini', ?)", rusqlite::params![blob]).unwrap();
            assert!(append_gemini_keys(&conn, &["new-key".into()]).is_err());
            let stored: Vec<u8> = conn.query_row("SELECT value FROM secrets WHERE provider = 'gemini'", [], |r| r.get(0)).unwrap();
            assert_eq!(stored, blob);
            let status = key_statuses(&conn).unwrap().into_iter().find(|s| s.provider == "gemini").unwrap();
            assert!(status.unreadable);
            assert_eq!(status.key_count, 0);
            assert!(status.preview.is_none());
        }
    }

    #[test]
    #[cfg_attr(not(windows), ignore = "requires a live OS keyring")]
    fn pool_checks_ignore_replaced_deleted_and_aba_secrets() {
        use crate::db::repo::*;
        let conn = memory_db();
        set_secret(&conn, "gemini", "original-key").unwrap();
        let (original, _) = gemini_secret_snapshot(&conn).unwrap().unwrap();
        assert!(record_gemini_check_if_unchanged(&conn, &original, true, Some("ok")).unwrap());
        set_secret(&conn, "gemini", "replacement-key").unwrap();
        assert!(!record_gemini_check_if_unchanged(&conn, &original, false, Some("stale")).unwrap());
        set_secret(&conn, "gemini", "original-key").unwrap();
        let (new_blob, _) = gemini_secret_snapshot(&conn).unwrap().unwrap();
        assert_ne!(original, new_blob);
        assert!(!record_gemini_check_if_unchanged(&conn, &original, false, Some("stale ABA")).unwrap());
        let status = key_statuses(&conn).unwrap().into_iter().find(|s| s.provider == "gemini").unwrap();
        assert_eq!(status.last_ok, None);
        assert_eq!(status.checked_at, None);
        clear_secret(&conn, "gemini").unwrap();
        assert!(!record_gemini_check_if_unchanged(&conn, &new_blob, true, Some("deleted")).unwrap());
        assert!(gemini_secret_snapshot(&conn).unwrap().is_none());
    }
}

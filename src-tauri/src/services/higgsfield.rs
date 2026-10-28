use std::fs;
use std::path::Path;
use std::time::Duration;
use reqwest::Client;
use serde_json::{json, Value};
use uuid::Uuid;
use crate::secrets::KeyTestResult;
use crate::services::http::read_error;

const BASE_URL: &str = "https://api.higgsfield.ai";

fn auth_header(key: &str) -> String {
    format!("Key {key}")
}

pub async fn test_credentials(key: &str) -> KeyTestResult {
    if key.trim().is_empty() {
        return KeyTestResult {
            ok: false,
            message: Some("Kunci belum diatur".into()),
        };
    }

    let client = Client::new();
    let url = format!("{BASE_URL}/estimate/higgsfield-ai/soul/v2/standard");
    let body = json!({
        "prompt": "test",
        "aspect_ratio": "16:9",
        "resolution": "720p",
        "batch_size": 1
    });

    match client.post(&url)
        .header("Authorization", auth_header(key))
        .json(&body)
        .send()
        .await
    {
        Ok(res) => {
            if res.status().is_success() {
                if let Ok(v) = res.json::<Value>().await {
                    let credits = v.get("credits").and_then(|x| x.as_str().or_else(|| x.as_f64().map(|_| ""))).unwrap_or("");
                    KeyTestResult {
                        ok: true,
                        message: Some(if !credits.is_empty() { format!("Terhubung · 1 gambar Soul ≈ {credits} kredit") } else { "Terhubung".into() }),
                    }
                } else {
                    KeyTestResult { ok: true, message: Some("Terhubung".into()) }
                }
            } else {
                let err = read_error(res).await;
                if !key.contains(':') {
                    KeyTestResult {
                        ok: false,
                        message: Some("Kunci ditolak. Kunci Higgsfield berformat KEY_ID:KEY_SECRET. Pakai tombol 'Copy API key' di console Higgsfield.".into()),
                    }
                } else {
                    KeyTestResult {
                        ok: false,
                        message: Some(format!("Higgsfield gagal: {err}")),
                    }
                }
            }
        }
        Err(e) => KeyTestResult {
            ok: false,
            message: Some(format!("Higgsfield gagal: {e}")),
        },
    }
}

pub async fn estimate(key: &str, endpoint: &str, body: &Value) -> Option<Value> {
    let client = Client::new();
    let clean_ep = endpoint.trim_start_matches('/');
    let url = format!("{BASE_URL}/estimate/{clean_ep}");

    let res = client.post(&url)
        .header("Authorization", auth_header(key))
        .json(body)
        .send()
        .await
        .ok()?;

    if res.status().is_success() {
        res.json::<Value>().await.ok()
    } else {
        None
    }
}

pub async fn submit(key: &str, endpoint: &str, body: &Value) -> Result<String, String> {
    let client = Client::new();
    let clean_ep = endpoint.trim_start_matches('/');
    let url = format!("{BASE_URL}/{clean_ep}");
    let idem = Uuid::new_v4().to_string();

    let mut last_err = String::new();
    for attempt in 0..6 {
        let res = client.post(&url)
            .header("Authorization", auth_header(key))
            .header("Idempotency-Key", &idem)
            .json(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = res.status();
        if status.is_success() {
            let root: Value = res.json().await.map_err(|e| e.to_string())?;
            return root.get("request_id")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| "No request_id in response".to_string());
        }

        let err = read_error(res).await;
        last_err = err.clone();
        if status.as_u16() == 429 || (status.as_u16() == 400 && err.to_lowercase().contains("concurrent")) {
            tokio::time::sleep(Duration::from_millis(4000 + attempt * 3000)).await;
            continue;
        }
        return Err(format!("Higgsfield error ({status}): {err}"));
    }

    Err(last_err)
}

pub async fn get_status(key: &str, request_id: &str) -> Result<Value, String> {
    let client = Client::new();
    let url = format!("{BASE_URL}/requests/{request_id}/status");

    let res = client.get(&url)
        .header("Authorization", auth_header(key))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        let err = read_error(res).await;
        return Err(format!("Status check error: {err}"));
    }

    res.json::<Value>().await.map_err(|e| e.to_string())
}

pub async fn cancel_request(key: &str, request_id: &str) {
    let client = Client::new();
    let url = format!("{BASE_URL}/requests/{request_id}/cancel");
    let _ = client.post(&url)
        .header("Authorization", auth_header(key))
        .send()
        .await;
}

pub async fn upload_file(key: &str, abs_path: &Path) -> Result<String, String> {
    let client = Client::new();
    let ext = abs_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let content_type = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "wav" => "audio/wav",
        _ => "image/png",
    };

    let slot_url = format!("{BASE_URL}/files/generate-upload-url");
    let slot_res = client.post(&slot_url)
        .header("Authorization", auth_header(key))
        .json(&json!({ "content_type": content_type }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !slot_res.status().is_success() {
        let err = read_error(slot_res).await;
        return Err(format!("Gagal membuat upload slot: {err}"));
    }

    let slot: Value = slot_res.json().await.map_err(|e| e.to_string())?;
    let upload_url = slot.get("upload_url").and_then(|x| x.as_str()).ok_or("No upload_url")?;
    let public_url = slot.get("public_url").and_then(|x| x.as_str()).ok_or("No public_url")?;

    let bytes = fs::read(abs_path).map_err(|e| e.to_string())?;

    let mut put_req = client.put(upload_url).body(bytes).header("Content-Type", content_type);
    if let Some(headers) = slot.get("upload_headers").and_then(|x| x.as_object()) {
        for (k, v) in headers {
            if let Some(v_str) = v.as_str() {
                put_req = put_req.header(k, v_str);
            }
        }
    }

    let put_res = put_req.send().await.map_err(|e| e.to_string())?;
    if !put_res.status().is_success() {
        return Err(format!("Gagal upload bytes: HTTP {}", put_res.status()));
    }

    Ok(public_url.to_string())
}

pub async fn wait_for_result<F>(
    key: &str,
    request_id: &str,
    mut on_tick: F,
) -> Result<Value, String>
where
    F: FnMut(&str, u64),
{
    let started = std::time::Instant::now();
    let mut delay = 2000u64;

    loop {
        if started.elapsed().as_secs() > 20 * 60 {
            return Err("Higgsfield terlalu lama merespons. Coba lagi nanti.".into());
        }

        let st = get_status(key, request_id).await?;
        let status = st.get("status").and_then(|x| x.as_str()).unwrap_or("in_progress");
        let elapsed = started.elapsed().as_millis() as u64;

        on_tick(status, elapsed);

        match status {
            "completed" => return Ok(st),
            "failed" => {
                let err = st.get("error").and_then(|x| x.as_str()).unwrap_or("tanpa keterangan");
                return Err(format!("Higgsfield gagal membuat hasil: {err}"));
            }
            "nsfw" => return Err("Ditolak moderasi Higgsfield. Ubah prompt lalu coba lagi.".into()),
            "canceled" => return Err("Dibatalkan".into()),
            _ => {
                tokio::time::sleep(Duration::from_millis(delay)).await;
                delay = (delay * 15 / 10).min(10_000);
            }
        }
    }
}

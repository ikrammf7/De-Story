use std::time::Duration;
use reqwest::Response;
use serde_json::Value;

pub async fn read_error(res: Response) -> String {
    let status = res.status();
    let text = res.text().await.unwrap_or_default();
    if let Ok(body) = serde_json::from_str::<Value>(&text) {
        if let Some(detail) = body.get("detail").or_else(|| body.get("error")).or(Some(&body)) {
            if let Some(s) = detail.as_str() {
                return s.to_string();
            }
            if let Some(arr) = detail.as_array() {
                let msgs: Vec<String> = arr
                    .iter()
                    .map(|x| x.get("msg").and_then(|m| m.as_str()).map(|s| s.to_string()).unwrap_or_else(|| x.to_string()))
                    .collect();
                return msgs.join("; ");
            }
            if let Some(obj) = detail.as_object() {
                if let Some(msg) = obj.get("message").or_else(|| obj.get("msg")).and_then(|m| m.as_str()) {
                    return msg.to_string();
                }
            }
        }
    }
    let truncated = if text.len() > 300 { &text[..300] } else { &text };
    if !truncated.is_empty() {
        truncated.to_string()
    } else {
        format!("HTTP {status}")
    }
}

pub async fn download_to_bytes(url: &str) -> Result<(Vec<u8>, String), String> {
    let client = reqwest::Client::new();
    let mut last_err = String::new();

    for attempt in 0..3 {
        match client.get(url).send().await {
            Ok(res) => {
                let status = res.status();
                if res.status().is_success() {
                    let content_type = res
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_string();
                    let bytes = res.bytes().await.map_err(|e| e.to_string())?.to_vec();
                    return Ok((bytes, content_type));
                } else {
                    last_err = format!("Gagal mengunduh hasil (HTTP {status})");
                    if status.as_u16() < 500 {
                        return Err(last_err);
                    }
                }
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
        tokio::time::sleep(Duration::from_millis(1500 * (attempt + 1))).await;
    }

    Err(last_err)
}

pub use download_to_bytes as download_to;

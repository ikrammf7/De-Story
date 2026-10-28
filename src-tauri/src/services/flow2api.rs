use std::time::Duration;
use base64::prelude::*;
use regex::Regex;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::secrets::KeyTestResult;
use crate::services::generation::{HfInput, HfModel};

pub fn sanitize_url(base: &str) -> String {
    let trimmed = base.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        "http://localhost:8000".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn flow_image_models() -> Vec<HfModel> {
    vec![
        HfModel {
            id: "flow:gemini-3.1-flash-image".to_string(),
            name: "Nano Banana 2.1 (Flash)".to_string(),
            family: "Google Flow".to_string(),
            kind: "image".to_string(),
            input: HfInput {
                field: Some("image".to_string()),
                mode: "reference".to_string(),
                max: 3,
            },
            required: vec!["prompt".to_string()],
            props: [
                ("prompt".to_string(), json!({ "type": "string", "maxLength": 4000 })),
                ("aspect_ratio".to_string(), json!({ "type": "string", "enum": ["16:9", "9:16", "1:1", "4:3", "3:4"], "default": "16:9" })),
                ("resolution".to_string(), json!({ "type": "string", "enum": ["1k", "2k", "4k"], "default": "2k" })),
            ].into_iter().collect(),
            durations: None,
            tags: vec!["Rekomendasi".into(), "Google Flow".into(), "Referensi".into(), "Cepat".into()],
        },
        HfModel {
            id: "flow:gemini-3.1-flash-lite-image".to_string(),
            name: "Nano Banana 2 Lite".to_string(),
            family: "Google Flow".to_string(),
            kind: "image".to_string(),
            input: HfInput {
                field: Some("image".to_string()),
                mode: "reference".to_string(),
                max: 1,
            },
            required: vec!["prompt".to_string()],
            props: [
                ("prompt".to_string(), json!({ "type": "string", "maxLength": 4000 })),
                ("aspect_ratio".to_string(), json!({ "type": "string", "enum": ["16:9", "9:16", "1:1"], "default": "16:9" })),
            ].into_iter().collect(),
            durations: None,
            tags: vec!["Google Flow".into(), "Ringan".into(), "Cepat".into()],
        },
        HfModel {
            id: "flow:gemini-3.0-pro-image".to_string(),
            name: "Banana Pro".to_string(),
            family: "Google Flow".to_string(),
            kind: "image".to_string(),
            input: HfInput {
                field: Some("image".to_string()),
                mode: "reference".to_string(),
                max: 2,
            },
            required: vec!["prompt".to_string()],
            props: [
                ("prompt".to_string(), json!({ "type": "string", "maxLength": 4000 })),
                ("aspect_ratio".to_string(), json!({ "type": "string", "enum": ["16:9", "9:16", "1:1"], "default": "16:9" })),
            ].into_iter().collect(),
            durations: None,
            tags: vec!["Google Flow".into(), "Pro".into(), "Artistik".into()],
        },
    ]
}

pub fn flow_video_models() -> Vec<HfModel> {
    vec![
        HfModel {
            id: "flow:veo-3.1-fast".to_string(),
            name: "Google Veo 3.1 Fast".to_string(),
            family: "Google Flow".to_string(),
            kind: "video".to_string(),
            input: HfInput {
                field: Some("image".to_string()),
                mode: "first-frame".to_string(),
                max: 1,
            },
            required: vec!["prompt".to_string(), "image".to_string()],
            props: [
                ("prompt".to_string(), json!({ "type": "string", "maxLength": 2500 })),
                ("resolution".to_string(), json!({ "type": "string", "enum": ["720p", "1080p"], "default": "1080p" })),
            ].into_iter().collect(),
            durations: Some(json!({ "values": [4, 6] })),
            tags: vec!["Rekomendasi".into(), "Google Flow".into(), "Veo".into(), "Cepat".into()],
        },
        HfModel {
            id: "flow:veo-3.1-quality".to_string(),
            name: "Google Veo 3.1 Quality".to_string(),
            family: "Google Flow".to_string(),
            kind: "video".to_string(),
            input: HfInput {
                field: Some("image".to_string()),
                mode: "first-frame".to_string(),
                max: 1,
            },
            required: vec!["prompt".to_string(), "image".to_string()],
            props: [
                ("prompt".to_string(), json!({ "type": "string", "maxLength": 2500 })),
                ("resolution".to_string(), json!({ "type": "string", "enum": ["720p", "1080p"], "default": "1080p" })),
            ].into_iter().collect(),
            durations: Some(json!({ "values": [4, 6] })),
            tags: vec!["Google Flow".into(), "Veo".into(), "Kualitas".into()],
        },
        HfModel {
            id: "flow:veo-3.1-lite".to_string(),
            name: "Google Veo 3.1 Lite".to_string(),
            family: "Google Flow".to_string(),
            kind: "video".to_string(),
            input: HfInput {
                field: Some("image".to_string()),
                mode: "first-frame".to_string(),
                max: 1,
            },
            required: vec!["prompt".to_string(), "image".to_string()],
            props: [
                ("prompt".to_string(), json!({ "type": "string", "maxLength": 2000 })),
                ("resolution".to_string(), json!({ "type": "string", "enum": ["720p"], "default": "720p" })),
            ].into_iter().collect(),
            durations: Some(json!({ "values": [4, 6] })),
            tags: vec!["Google Flow".into(), "Veo".into(), "Ringan".into()],
        },
        HfModel {
            id: "flow:omni-1.1-flash".to_string(),
            name: "Google Omni 1.1 Flash".to_string(),
            family: "Google Flow".to_string(),
            kind: "video".to_string(),
            input: HfInput {
                field: Some("image".to_string()),
                mode: "first-frame".to_string(),
                max: 1,
            },
            required: vec!["prompt".to_string(), "image".to_string()],
            props: [
                ("prompt".to_string(), json!({ "type": "string", "maxLength": 2500 })),
                ("resolution".to_string(), json!({ "type": "string", "enum": ["720p", "1080p"], "default": "1080p" })),
            ].into_iter().collect(),
            durations: Some(json!({ "values": [4, 6, 8, 10] })),
            tags: vec!["Google Flow".into(), "Omni".into(), "Dinamis".into()],
        },
    ]
}

/// Map a `flow:` model id (with the prefix already stripped) onto the name the
/// bundled backend actually serves. UseAPI-era ids from retired releases keep
/// generating instead of failing with "unknown model".
fn canonical_model(raw_model: &str) -> &str {
    match raw_model {
        "nano-banana-2" => "gemini-3.1-flash-image",
        "nano-banana-2-lite" => "gemini-3.1-flash-lite-image",
        "nano-banana-pro" => "gemini-3.0-pro-image",
        "omni-flash" => "omni-1.1-flash",
        other => other,
    }
}

/// Translate a model id saved while the app pointed at useapi.net into the id
/// this build serves, so old projects keep resolving. Unrelated ids pass
/// through untouched.
pub fn canonical_flow_id(id: &str) -> String {
    match id.strip_prefix("flow:") {
        Some(rest) => {
            let mapped = canonical_model(rest);
            if mapped == rest {
                id.to_string()
            } else {
                format!("flow:{mapped}")
            }
        }
        None => id.to_string(),
    }
}

pub async fn test_connection(base_url: &str, key: Option<&str>) -> KeyTestResult {
    let base = sanitize_url(base_url);
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
    {
        Ok(c) => c,
        Err(e) => return KeyTestResult {
            ok: false,
            message: Some(format!("Gagal inisialisasi HTTP client: {e}")),
        },
    };

    // First attempt: GET {base}/health
    let health_url = format!("{base}/health");
    let mut req = client.get(&health_url);
    if let Some(k) = key {
        if !k.trim().is_empty() {
            req = req.header("Authorization", format!("Bearer {}", k.trim()));
        }
    }

    if let Ok(res) = req.send().await {
        if res.status().is_success() {
            return KeyTestResult {
                ok: true,
                message: Some(format!("Terhubung ke Flow2API di {base}")),
            };
        }
    }

    // Second attempt: GET {base}/v1/models
    let models_url = format!("{base}/v1/models");
    let mut req_models = client.get(&models_url);
    if let Some(k) = key {
        if !k.trim().is_empty() {
            req_models = req_models.header("Authorization", format!("Bearer {}", k.trim()));
        }
    }

    match req_models.send().await {
        Ok(res) if res.status().is_success() => KeyTestResult {
            ok: true,
            message: Some(format!("Terhubung ke Flow2API ({base})")),
        },
        Ok(res) => {
            let status = res.status();
            KeyTestResult {
                ok: false,
                message: Some(format!("Server Flow2API mengembalikan status HTTP {status}")),
            }
        }
        Err(err) => KeyTestResult {
            ok: false,
            message: Some(format!("Tidak dapat terhubung ke server Flow2API di {base}: {err}")),
        },
    }
}

pub fn extract_media_url_or_data(text: &str) -> Option<String> {
    let trimmed = text.trim();

    // 0. Direct data URI (starts with data:image/ or data:video/)
    if trimmed.starts_with("data:image/") || trimmed.starts_with("data:video/") {
        let clean = trimmed.trim_matches(|c| c == '"' || c == '\'' || c == ')' || c == '>');
        return Some(clean.to_string());
    }

    // 1. Direct URL (starts with http:// or https://)
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        let clean = trimmed.split_whitespace().next().unwrap_or(trimmed);
        let clean = clean.trim_matches(|c| c == '"' || c == '\'' || c == ')' || c == '>');
        return Some(clean.to_string());
    }

    // 2. Direct relative path (e.g. /files/..., /static/..., /output/..., /v1/files/...)
    if trimmed.starts_with("/files/")
        || trimmed.starts_with("files/")
        || trimmed.starts_with("/static/")
        || trimmed.starts_with("/output/")
        || trimmed.starts_with("/v1/")
    {
        let clean = trimmed.split_whitespace().next().unwrap_or(trimmed);
        let clean = clean.trim_matches(|c| c == '"' || c == '\'' || c == ')' || c == '>');
        return Some(clean.to_string());
    }

    // 3. Check for markdown video / image: ![...](...) or [...](...)
    if let Ok(re_md) = Regex::new(r"!\[.*?\]\((https?://[^\s\)]+|data:[^\s\)]+|/[^\s\)]+)\)") {
        if let Some(caps) = re_md.captures(text) {
            if let Some(m) = caps.get(1) {
                return Some(m.as_str().to_string());
            }
        }
    }

    // 4. Check for html video/img: <video ... src="..." or <img ... src="..."
    if let Ok(re_html) = Regex::new(r#"<(?:video|img)[^>]+src=["'](.*?)["']"#) {
        if let Some(caps) = re_html.captures(text) {
            if let Some(m) = caps.get(1) {
                return Some(m.as_str().to_string());
            }
        }
    }

    // 5. Check for embedded data URI anywhere in text
    if let Some(start_idx) = text.find("data:image/").or_else(|| text.find("data:video/")) {
        let rest = &text[start_idx..];
        let end_idx = rest
            .find(|c: char| c == ')' || c == '"' || c == '\'' || c == '<' || c == '>' || c == ']' || c == '}' || c.is_control())
            .unwrap_or(rest.len());
        let data_uri = &rest[..end_idx];
        return Some(data_uri.to_string());
    }

    // 6. Check for markdown link: [...](https://... or /...)
    if let Ok(re_link) = Regex::new(r"\[.*?\]\((https?://[^\s\)]+|/[^\s\)]+)\)") {
        if let Some(caps) = re_link.captures(text) {
            if let Some(m) = caps.get(1) {
                return Some(m.as_str().to_string());
            }
        }
    }

    // 7. Check for standalone URL: https?://...
    if let Ok(re_url) = Regex::new(r#"https?://[^\s<>'")]+"#) {
        if let Some(m) = re_url.find(text) {
            return Some(m.as_str().to_string());
        }
    }

    // 8. Check for relative path pattern: /(?:files|static|output)/...
    if let Ok(re_path) = Regex::new(r#"/(?:files|static|output|v1)/[^\s<>'")]+"#) {
        if let Some(m) = re_path.find(text) {
            return Some(m.as_str().to_string());
        }
    }

    None
}

/// The error text carried by one parsed chunk, if it has any. Flow2API reports
/// an upstream failure as a normal chunk on a 200 stream, so this is the only
/// place the real reason can be found.
fn chunk_error(v: &Value) -> Option<String> {
    let err = v.get("error")?;
    if let Some(msg) = err.get("message").and_then(|m| m.as_str()) {
        return Some(msg.to_string());
    }
    if let Some(s) = err.as_str() {
        return Some(s.to_string());
    }
    Some(err.to_string())
}

/// A short, readable excerpt for messages that would otherwise quote a whole
/// stream at the user.
fn snippet(text: &str) -> String {
    let mut clean: Vec<&str> = Vec::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches("data:").trim();
        if line.is_empty() || line == "[DONE]" {
            continue;
        }
        clean.push(line);
    }
    let joined = clean.join(" ");
    let mut out: String = joined.chars().take(240).collect();
    if joined.chars().count() > 240 {
        out.push('…');
    }
    out
}

/// Flow2API must obtain a reCAPTCHA token from its selected provider.
/// Keep the message focused on that failure without guessing its cause.
fn friendly_flow_error(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("recaptcha") {
        return format!(
            "Pembuatan media gagal: server Flow2API melaporkan masalah reCAPTCHA. \
             Penyebabnya belum diketahui; periksa log Flow2API untuk rincian metode captcha yang dipilih. \
             Pesan dari server: {raw}"
        );
    }
    format!("Flow2API gagal membuat media: {raw}")
}

pub fn parse_chat_completion_content(body: &str) -> Result<String, String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err("Respons dari Flow2API kosong".to_string());
    }

    // 1. Try parsing as SSE stream (lines starting with `data:`)
    let mut sse_content = String::new();
    let mut sse_reasoning = String::new();
    let mut sse_error: Option<String> = None;
    let mut is_sse = false;

    for line in trimmed.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("data:") {
            is_sse = true;
            let data = rest.trim();
            if data == "[DONE]" || data.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<Value>(data) {
                // A failed generation arrives as an ordinary chunk, so an error
                // is recorded rather than discarded as an unrecognised shape.
                if let Some(e) = chunk_error(&v) {
                    sse_error = Some(e);
                    continue;
                }
                if let Some(choices) = v.get("choices").and_then(|c| c.as_array()) {
                    for choice in choices {
                        let delta = choice.get("delta");
                        if let Some(d) = delta {
                            if let Some(c) = d.get("content").and_then(|s| s.as_str()) {
                                sse_content.push_str(c);
                            }
                            // Not an answer, but it does explain a failure.
                            if let Some(r) = d.get("reasoning_content").and_then(|s| s.as_str()) {
                                sse_reasoning.push_str(r);
                            }
                        }
                        if let Some(msg) = choice.get("message") {
                            if let Some(c) = msg.get("content").and_then(|s| s.as_str()) {
                                sse_content.push_str(c);
                            }
                        }
                    }
                } else if let Some(e) = chunk_error(&v) {
                    sse_error = Some(e);
                }
            }
        }
    }

    if is_sse && !sse_content.trim().is_empty() {
        return Ok(sse_content);
    }

    // A stream that produced no content but reported an error: say what the
    // provider actually said instead of quoting the whole stream back.
    if let Some(err) = sse_error {
        return Err(friendly_flow_error(&err));
    }

    // 2. Try parsing as standard JSON
    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
        if let Some(err) = chunk_error(&parsed) {
            return Err(friendly_flow_error(&err));
        }
        if let Some(choices) = parsed.get("choices").and_then(|c| c.as_array()) {
            if let Some(first) = choices.first() {
                if let Some(content) = first.get("message").and_then(|m| m.get("content")) {
                    if let Some(s) = content.as_str() {
                        return Ok(s.to_string());
                    }
                    if let Some(arr) = content.as_array() {
                        let mut combined = String::new();
                        for item in arr {
                            if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                                combined.push_str(t);
                                combined.push('\n');
                            }
                        }
                        if !combined.is_empty() {
                            return Ok(combined);
                        }
                    }
                }
                if let Some(delta) = first.get("delta").and_then(|d| d.get("content")).and_then(|s| s.as_str()) {
                    return Ok(delta.to_string());
                }
            }
        }
    }

    // 3. Fallback: if trimmed contains media or URL directly
    if extract_media_url_or_data(trimmed).is_some() {
        return Ok(trimmed.to_string());
    }

    // No result and no stated error. The stream's own running commentary is the
    // only clue, and it is far more useful than the raw bytes.
    if !sse_reasoning.trim().is_empty() {
        return Err(format!(
            "Flow2API tidak mengirim hasil dan tidak menyebut alasan. Keterangan dari server: {}",
            snippet(&sse_reasoning)
        ));
    }

    Err(format!(
        "Tidak dapat mengekstrak konten dari respons Flow2API: {}",
        snippet(trimmed)
    ))
}

async fn fetch_or_decode_bytes(base_url: &str, target: &str) -> Result<Vec<u8>, String> {
    if target.starts_with("data:") {
        let comma_idx = target.find(',').ok_or("Data URI tidak valid (tanpa koma)")?;
        let b64 = &target[comma_idx + 1..];
        let clean_b64: String = b64.chars().filter(|c| !c.is_whitespace()).collect();
        BASE64_STANDARD.decode(&clean_b64).map_err(|e| format!("Gagal decode base64: {e}"))
    } else {
        let full_url = if target.starts_with("http://") || target.starts_with("https://") {
            target.to_string()
        } else {
            let base = sanitize_url(base_url);
            let path = target.trim_start_matches('/');
            format!("{base}/{path}")
        };

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .map_err(|e| e.to_string())?;

        let res = client.get(&full_url)
            .send()
            .await
            .map_err(|e| format!("Gagal mengunduh media dari {full_url}: {e}"))?;

        if !res.status().is_success() {
            return Err(format!("Gagal mengunduh media (HTTP {}) dari {full_url}", res.status()));
        }

        let bytes = res.bytes().await.map_err(|e| format!("Gagal membaca bytes media: {e}"))?;
        Ok(bytes.to_vec())
    }
}

pub async fn generate_image(
    base_url: &str,
    key: Option<&str>,
    model_id: &str,
    prompt: &str,
    aspect_ratio: &str,
    ref_images: &[Vec<u8>],
    token: &CancellationToken,
) -> Result<Vec<u8>, String> {
    let base = sanitize_url(base_url);
    let raw_model = canonical_model(model_id.trim_start_matches("flow:"));

    let aspect_suffix = match aspect_ratio {
        "16:9" => "-landscape",
        "9:16" => "-portrait",
        "1:1" => "-square",
        "4:3" => "-four-three",
        "3:4" => "-three-four",
        _ => "-landscape",
    };

    let target_model = if raw_model.contains("-landscape")
        || raw_model.contains("-portrait")
        || raw_model.contains("-square")
        || raw_model.contains("-four-three")
        || raw_model.contains("-three-four")
    {
        raw_model.to_string()
    } else {
        format!("{raw_model}{aspect_suffix}")
    };

    let content_val = if ref_images.is_empty() {
        json!(prompt)
    } else {
        let mut content_parts: Vec<Value> = Vec::new();
        content_parts.push(json!({
            "type": "text",
            "text": prompt
        }));
        for img_bytes in ref_images {
            let b64 = BASE64_STANDARD.encode(img_bytes);
            content_parts.push(json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:image/png;base64,{b64}")
                }
            }));
        }
        json!(content_parts)
    };

    let payload = json!({
        "model": target_model,
        "messages": [
            {
                "role": "user",
                "content": content_val
            }
        ],
        "stream": true
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("{base}/v1/chat/completions");
    let mut req = client.post(&url).json(&payload);

    if let Some(k) = key {
        if !k.trim().is_empty() {
            req = req.header("Authorization", format!("Bearer {}", k.trim()));
        }
    }

    let response = tokio::select! {
        _ = token.cancelled() => return Err("Pembuatan gambar dibatalkan".into()),
        res = req.send() => res.map_err(|e| format!("Koneksi ke Flow2API gagal: {e}"))?,
    };

    let status = response.status();
    let body_text = tokio::select! {
        _ = token.cancelled() => return Err("Pembuatan gambar dibatalkan".into()),
        t = response.text() => t.unwrap_or_default(),
    };

    if !status.is_success() {
        // The body of a failed stream is chunks, not a sentence; route it through
        // the same reader so the user gets the reason rather than the raw bytes.
        return Err(match parse_chat_completion_content(&body_text) {
            Err(msg) => msg,
            Ok(text) => format!("Flow2API error (HTTP {status}): {}", snippet(&text)),
        });
    }

    let content_str = parse_chat_completion_content(&body_text)?;
    let media_target = extract_media_url_or_data(&content_str)
        .ok_or_else(|| format!("Tidak dapat menemukan URL atau data gambar di respons Flow2API: {}", snippet(&content_str)))?;

    fetch_or_decode_bytes(&base, &media_target).await
}

pub async fn generate_video(
    base_url: &str,
    key: Option<&str>,
    model_id: &str,
    prompt: &str,
    first_frame_bytes: &[u8],
    duration_sec: f64,
    aspect_ratio: &str,
    token: &CancellationToken,
) -> Result<Vec<u8>, String> {
    let base = sanitize_url(base_url);
    let raw_model = canonical_model(model_id.trim_start_matches("flow:"));

    let aspect = if aspect_ratio == "9:16" {
        "portrait"
    } else {
        "landscape"
    };

    let is_veo = raw_model.starts_with("veo-");
    let sec = if is_veo {
        if duration_sec <= 5.0 {
            4
        } else {
            6
        }
    } else {
        if duration_sec <= 5.0 {
            4
        } else if duration_sec <= 7.0 {
            6
        } else if duration_sec <= 9.0 {
            8
        } else {
            10
        }
    };

    let target_model = if raw_model.contains("-i2v-") || raw_model.contains("-fast-") || raw_model.contains("-quality-") {
        if raw_model.ends_with("-landscape") || raw_model.ends_with("-portrait") {
            raw_model.to_string()
        } else {
            format!("{raw_model}-i2v-{sec}s-{aspect}")
        }
    } else if is_veo {
        format!("{raw_model}-i2v-{sec}s-{aspect}")
    } else {
        format!("{raw_model}-{sec}s-{aspect}")
    };

    let frame_b64 = BASE64_STANDARD.encode(first_frame_bytes);
    let content_parts = vec![
        json!({
            "type": "text",
            "text": format!("{prompt} Silent clip: no background music, no soundtrack, no singing or humming.")
        }),
        json!({
            "type": "image_url",
            "image_url": {
                "url": format!("data:image/jpeg;base64,{frame_b64}")
            }
        }),
    ];

    let payload = json!({
        "model": target_model,
        "messages": [
            {
                "role": "user",
                "content": content_parts
            }
        ],
        "stream": true
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600)) // 10 minutes max for video generation
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("{base}/v1/chat/completions");
    let mut req = client.post(&url).json(&payload);

    if let Some(k) = key {
        if !k.trim().is_empty() {
            req = req.header("Authorization", format!("Bearer {}", k.trim()));
        }
    }

    let response = tokio::select! {
        _ = token.cancelled() => return Err("Pembuatan video dibatalkan".into()),
        res = req.send() => res.map_err(|e| format!("Koneksi ke Flow2API gagal: {e}"))?,
    };

    let status = response.status();
    let body_text = tokio::select! {
        _ = token.cancelled() => return Err("Pembuatan video dibatalkan".into()),
        t = response.text() => t.unwrap_or_default(),
    };

    if !status.is_success() {
        // The body of a failed stream is chunks, not a sentence; route it through
        // the same reader so the user gets the reason rather than the raw bytes.
        return Err(match parse_chat_completion_content(&body_text) {
            Err(msg) => msg,
            Ok(text) => format!("Flow2API error (HTTP {status}): {}", snippet(&text)),
        });
    }

    let content_str = parse_chat_completion_content(&body_text)?;
    let media_target = extract_media_url_or_data(&content_str)
        .ok_or_else(|| format!("Tidak dapat menemukan URL atau data video di respons Flow2API: {}", snippet(&content_str)))?;

    fetch_or_decode_bytes(&base, &media_target).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_url() {
        assert_eq!(sanitize_url("http://localhost:8000/"), "http://localhost:8000");
        assert_eq!(sanitize_url("   http://localhost:8000   "), "http://localhost:8000");
        assert_eq!(sanitize_url(""), "http://localhost:8000");
        assert_eq!(sanitize_url("https://my-flow-proxy.com/"), "https://my-flow-proxy.com");
    }

    #[test]
    fn test_extract_markdown_image() {
        let input = "Here is your image:\n![Generated](https://images.example.com/output.png)\nHope you like it!";
        assert_eq!(
            extract_media_url_or_data(input),
            Some("https://images.example.com/output.png".to_string())
        );
    }

    #[test]
    fn test_extract_html_video() {
        let input = r#"<video controls src="https://videos.example.com/veo.mp4"></video>"#;
        assert_eq!(
            extract_media_url_or_data(input),
            Some("https://videos.example.com/veo.mp4".to_string())
        );
    }

    #[test]
    fn test_extract_direct_data_uri() {
        let input = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUA";
        assert_eq!(
            extract_media_url_or_data(input),
            Some("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUA".to_string())
        );
    }

    #[test]
    fn test_extract_embedded_data_uri() {
        let input = "Generated result: [image](data:image/jpeg;base64,/9j/4AAQSkZJRgABAQEASABIAAD...) done.";
        assert_eq!(
            extract_media_url_or_data(input),
            Some("data:image/jpeg;base64,/9j/4AAQSkZJRgABAQEASABIAAD...".to_string())
        );
    }

    #[test]
    fn test_extract_standalone_url() {
        let input = "Check out https://flow2api.local/files/result_123.mp4 for your video";
        assert_eq!(
            extract_media_url_or_data(input),
            Some("https://flow2api.local/files/result_123.mp4".to_string())
        );
    }

    #[test]
    fn test_extract_relative_path() {
        let input = "/files/generated_video_abc123.mp4";
        assert_eq!(
            extract_media_url_or_data(input),
            Some("/files/generated_video_abc123.mp4".to_string())
        );
    }

    #[test]
    fn test_parse_chat_completion_sse_stream() {
        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"![image]\"}}]}\n\
                   data: {\"choices\":[{\"delta\":{\"content\":\"(https://test.com/img.png)\"}}]}\n\
                   data: [DONE]";
        let parsed = parse_chat_completion_content(sse).expect("should parse SSE");
        assert_eq!(parsed, "![image](https://test.com/img.png)");
        assert_eq!(
            extract_media_url_or_data(&parsed),
            Some("https://test.com/img.png".to_string())
        );
    }

    #[test]
    fn test_parse_chat_completion_json() {
        let json_body = r#"{"choices":[{"message":{"content":"https://test.com/output.png"}}]}"#;
        let parsed = parse_chat_completion_content(json_body).expect("should parse JSON");
        assert_eq!(parsed, "https://test.com/output.png");
    }

    /// The exact shape seen in the wild: a 200 stream whose only content is
    /// `reasoning_content`, ended by a chunk carrying the real error.
    #[test]
    fn test_parse_stream_reports_the_error_a_failed_generation_sends() {
        let sse = concat!(
            "data: {\"id\":\"chatcmpl-1\",\"object\":\"chat.completion.chunk\",\"model\":\"flow2api\",",
            "\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"reasoning_content\":\"🖼️ 图片生成任务已启动\"},",
            "\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-2\",\"object\":\"chat.completion.chunk\",\"model\":\"flow2api\",",
            "\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"正在进行打码验证并提交图片生成请求…\\n\"},",
            "\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-3\",\"object\":\"chat.completion.chunk\",",
            "\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"错误 生成失败: Failed to obtain reCAPTCHA token\"},",
            "\"finish_reason\":null}]}\n\n",
            "data: {\"error\":{\"message\":\"生成失败: Failed to obtain reCAPTCHA token\",\"type\":\"server_error\",",
            "\"code\":\"generation_failed\",\"status_code\":500}}\n\n",
            "data: [DONE]\n"
        );

        let err = parse_chat_completion_content(sse).unwrap_err();
        assert!(err.contains("reCAPTCHA"), "unexpected message: {err}");
        assert!(err.contains("Penyebabnya belum diketahui"), "should avoid guessing the cause: {err}");
        assert!(!err.contains("kedaluwarsa"), "unsupported diagnosis leaked: {err}");
        // The raw event stream must never reach the user.
        assert!(!err.contains("chat.completion.chunk"), "raw stream leaked: {err}");
    }

    /// A stream that stops without content and without an error still explains
    /// itself better through its commentary than through its raw bytes.
    #[test]
    fn test_parse_stream_without_result_quotes_the_reasoning_not_the_stream() {
        let sse = concat!(
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"正在渲染\"},\"finish_reason\":null}]}\n\n",
            "data: [DONE]\n"
        );
        let err = parse_chat_completion_content(sse).unwrap_err();
        assert!(err.contains("正在渲染"), "unexpected message: {err}");
        assert!(!err.contains("choices"), "raw stream leaked: {err}");
    }

    #[test]
    fn test_parse_stream_error_chunk_without_a_message() {
        let sse = "data: {\"error\":\"boom\"}\n\ndata: [DONE]\n";
        let err = parse_chat_completion_content(sse).unwrap_err();
        assert!(err.contains("boom"), "unexpected message: {err}");
    }

    #[test]
    fn test_parse_stream_still_returns_content_when_it_succeeds() {
        // An error chunk after real content must not discard the content.
        let sse = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"https://test.com/a.png\"}}]}\n\n",
            "data: {\"error\":{\"message\":\"trailing noise\"}}\n\n",
            "data: [DONE]\n"
        );
        let parsed = parse_chat_completion_content(sse).expect("content should win");
        assert_eq!(parsed, "https://test.com/a.png");
    }

    #[test]
    fn test_snippet_bounds_a_long_body() {
        let long = format!("data: {}\n\ndata: {}\n", "a".repeat(200), "b".repeat(200));
        let out = snippet(&long);
        assert!(out.chars().count() <= 241, "snippet should be bounded");
        assert!(!out.contains("data:"), "snippet should drop the SSE prefix");
    }
}


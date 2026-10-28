use reqwest::Client;
use serde_json::{json, Value};
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use regex::Regex;
use crate::secrets::KeyTestResult;
use crate::services::audio::{is_wav, pcm_to_wav};
use crate::services::http::read_error;
use crate::services::json::extract_json;

const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

// reqwest has no default deadline, so a stalled connection would leave a job
// hanging with no error and a progress bar that never moves. Speech synthesis
// legitimately takes minutes for a long take, so it gets its own, far longer
// budget than the metadata and text calls.
const REQUEST_TIMEOUT_SECS: u64 = 90;
// A full storyboard includes detailed descriptions for every scene in one call.
const TEXT_TIMEOUT_SECS: u64 = 5 * 60;
const TTS_TIMEOUT_SECS: u64 = 10 * 60;

fn http_client(timeout_secs: u64) -> Client {
    Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .unwrap_or_else(|_| Client::new())
}

pub const GEMINI_VOICES: &[(&str, &str, &str)] = &[
    ("Puck", "Puck", "Semangat & ceria"),
    ("Charon", "Charon", "Tenang & dalam"),
    ("Kore", "Kore", "Lembut & ramah"),
    ("Fenrir", "Fenrir", "Tegas & berwibawa"),
    ("Aoede", "Aoede", "Elegan & naratif"),
];

// ==========================================
// Key pool
// ==========================================
//
// Gemini rate limits are per Google project, so the user stores one key per
// project and the app rotates through them. There is no separate table for a
// list: the pool *is* the encrypted `gemini` secret, one key per line. A single
// legacy key therefore reads back as a pool of one.

pub fn parse_key_pool(raw: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in raw.split(['\n', '\r']) {
        let key = line.trim();
        if key.is_empty() || out.iter().any(|k| k == key) {
            continue;
        }
        out.push(key.to_string());
    }
    out
}

/// Short display form of one key. Never used inside an error message that could
/// reach a log, only in the per-key test results the Settings page renders.
pub fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 10 {
        return "•".repeat(chars.len().max(4));
    }
    let lead: String = chars[..6].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{lead}{}{tail}", "•".repeat(6))
}

/// Which key the next request starts from. Kept pure so the wrap-around can be
/// tested without touching the shared counter.
fn start_for(counter: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        counter % len
    }
}

/// The positions successive calls would take, without advancing the shared
/// counter — used by the tests to prove rotation wraps around.
#[cfg(test)]
fn next_start_sequence(counter: &AtomicUsize, len: usize, calls: usize) -> Vec<usize> {
    let start = counter.load(Ordering::SeqCst);
    (0..calls).map(|i| start_for(start + i, len)).collect()
}

/// Calls made so far across the whole app; shared by the async jobs and the
/// commands so two at once still take different keys. Resets on app restart.
static NEXT_KEY: AtomicUsize = AtomicUsize::new(0);

pub fn friendly_error(e_str: &str) -> String {
    let lower = e_str.to_lowercase();
    if lower.contains("api key not valid") || lower.contains("api_key_invalid") || lower.contains("permission_denied") || lower.contains("401") || lower.contains("403") {
        "Kunci Gemini ditolak. Periksa kunci di Pengaturan.".to_string()
    } else if lower.contains("resource_exhausted") || lower.contains("429") {
        "Kuota Gemini habis atau terlalu banyak permintaan. Coba lagi sebentar.".to_string()
    } else if lower.contains("melewati batas waktu") || lower.contains("timed out") || lower.contains("timeout") {
        format!("Gemini gagal: {e_str}")
    } else if lower.contains("not found") || lower.contains("404") {
        format!("Model Gemini tidak tersedia untuk kunci ini: {e_str}")
    } else if lower.starts_with("gemini ") || lower.starts_with("kunci ") || lower.starts_with("model ") || lower.starts_with("suara ") || lower.contains("pengaturan") {
        // Already one of our own messages; wrapping it again only adds noise.
        e_str.to_string()
    } else {
        format!("Gemini gagal: {e_str}")
    }
}

// ==========================================
// Failure classification and rotation
// ==========================================

/// A rejected key, an exhausted quota, or a busy model is worth trying on the
/// next key. Anything else is a real problem with the request (bad model name,
/// malformed payload) and rotating would only hide it behind repeated failures.
#[derive(Debug, Clone, PartialEq)]
enum GeminiFailure {
    Quota,
    InvalidKey,
    /// The provider itself is temporarily unable to serve — overloaded model,
    /// gateway hiccup. Another key is often served by a different capacity.
    Transient(String),
    Other(String),
}

fn classify_status(status: u16) -> GeminiFailure {
    match status {
        429 => GeminiFailure::Quota,
        401 | 403 => GeminiFailure::InvalidKey,
        // 503 UNAVAILABLE is what a "currently experiencing high demand" model
        // answers with; 500/502/504 are the same kind of provider-side blip.
        503 | 500 | 502 | 504 => GeminiFailure::Transient(format!("HTTP {status}")),
        _ => GeminiFailure::Other(format!("HTTP {status}")),
    }
}

/// Google normally signals overload with 503, but the message also arrives on a
/// 200-wrapped body or a rewritten status, so the wording is checked too. An
/// unusable key is likewise reported as a plain 400 INVALID_ARGUMENT.
fn classify_body(status: u16, body: &str) -> GeminiFailure {
    let kind = classify_status(status);
    let lower = body.to_lowercase();
    let overloaded = lower.contains("high demand")
        || lower.contains("currently experiencing")
        || lower.contains("overloaded")
        || lower.contains("try again later");
    if overloaded {
        return GeminiFailure::Transient(body.to_string());
    }
    let dead_key = lower.contains("api key not valid")
        || lower.contains("api_key_invalid")
        || lower.contains("api key expired")
        || lower.contains("invalid api key");
    if dead_key {
        return GeminiFailure::InvalidKey;
    }
    match kind {
        GeminiFailure::Other(_) => GeminiFailure::Other(body.to_string()),
        failure => failure,
    }
}

/// reqwest's `Display` stops at "error sending request for url (…)" and keeps
/// the actual reason (timed out, connection reset, DNS, TLS) in its `source()`
/// chain. Walking it turns an opaque toast into something the user can act on.
/// The URL is left as-is: the API key rides in a header, so it never appears here.
fn describe_transport(e: &reqwest::Error) -> String {
    let mut chain: Vec<String> = vec![e.to_string()];
    let mut source = std::error::Error::source(e);
    while let Some(err) = source {
        let text = err.to_string();
        if !text.is_empty() && !chain.iter().any(|c| c == &text) {
            chain.push(text);
        }
        source = err.source();
    }
    chain.join(" → ")
}

fn transport_failure(e: reqwest::Error) -> GeminiFailure {
    let described = describe_transport(&e);
    // Even on timeout, Gemini may have received the request. Do not rotate to
    // another key and submit it again without the user's explicit action.
    if e.is_timeout() {
        return GeminiFailure::Other(format!(
            "Permintaan Gemini melewati batas waktu. Coba lagi atau pilih model lain di Pengaturan. Detail: {described}"
        ));
    }
    GeminiFailure::Other(described)
}

/// Reads a non-2xx response, keeping the status as the failure kind. The
/// provider's own wording is attached to the message-bearing variants so
/// unexpected errors still reach the user in full.
async fn read_failure(res: reqwest::Response) -> GeminiFailure {
    let status = res.status().as_u16();
    let body = read_error(res).await;
    classify_body(status, &body)
}

// One starting position per operation, at most one attempt per key. Taking the
// counter as an argument keeps offline tests independent of concurrent requests.
async fn pool_run<T, F, Fut>(raw: &str, counter: usize, mut attempt: F) -> Result<T, String>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<T, GeminiFailure>>,
{
    let pool = parse_key_pool(raw);
    if pool.is_empty() {
        return Err("Kunci Gemini belum diatur. Buka Pengaturan lalu masukkan API key.".into());
    }
    let start = start_for(counter, pool.len());
    let mut quota = 0;
    let mut rejected = 0;
    let mut busy = 0;
    for offset in 0..pool.len() {
        match attempt(pool[(start + offset) % pool.len()].clone()).await {
            Ok(value) => return Ok(value),
            Err(GeminiFailure::Quota) => quota += 1,
            Err(GeminiFailure::InvalidKey) => rejected += 1,
            Err(GeminiFailure::Transient(_)) => busy += 1,
            // A genuine request error stops the walk: retrying it on every key
            // would only repeat the same bad request.
            Err(GeminiFailure::Other(e)) => return Err(redact(&friendly_error(&e), &pool)),
        }
    }
    let mut parts: Vec<String> = Vec::new();
    if quota > 0 {
        parts.push(format!("{quota} terkena batas kuota"));
    }
    if rejected > 0 {
        parts.push(format!("{rejected} ditolak"));
    }
    if busy > 0 {
        parts.push(format!("{busy} sedang sibuk atau kelebihan beban"));
    }
    let detail = if parts.is_empty() {
        "tidak ada yang berhasil".to_string()
    } else {
        parts.join(", ")
    };
    Err(format!(
        "Semua {} kunci Gemini telah dicoba ({}). Tunggu sebentar atau periksa kunci di Pengaturan.",
        pool.len(),
        detail
    ))
}

async fn with_pool<T, F, Fut>(raw: &str, attempt: F) -> Result<T, String>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<T, GeminiFailure>>,
{
    pool_run(raw, NEXT_KEY.fetch_add(1, Ordering::Relaxed), attempt).await
}

fn redact(message: &str, keys: &[String]) -> String {
    let mut result = message.to_string();
    for key in keys {
        result = result.replace(key, "[API key]");
    }
    result
}

// ==========================================
// Requests
// ==========================================

fn endpoint(model: &str) -> String {
    format!("{BASE_URL}/models/{model}:generateContent")
}

fn with_key(req: reqwest::RequestBuilder, key: &str) -> reqwest::RequestBuilder {
    // Sent as a header rather than a query parameter so a key never lands in a
    // URL that a transport error might echo back into a log.
    req.header("x-goog-api-key", key)
}

/// Runs one POST against `model` with one key, returning the parsed body.
/// The HTTP status is inspected *before* the body is reduced to error text,
/// which is what lets the caller tell a quota failure from a bad request.
async fn post_attempt(client: &Client, model: &str, key: &str, body: &Value) -> Result<Value, GeminiFailure> {
    let res = with_key(client.post(endpoint(model)).json(body), key)
        .send()
        .await
        .map_err(|e| transport_failure(e))?;

    if !res.status().is_success() {
        return Err(read_failure(res).await);
    }

    let root: Value = res
        .json()
        .await
        .map_err(|e| GeminiFailure::Other(e.to_string()))?;

    // An overloaded model can also answer 200 with an empty candidate list and
    // a complaint instead of content; that is transient, not a bad request.
    let has_candidates = root["candidates"]
        .as_array()
        .map(|c| !c.is_empty())
        .unwrap_or(false);
    if !has_candidates {
        let text = root.to_string();
        if let GeminiFailure::Transient(reason) = classify_body(200, &text) {
            return Err(GeminiFailure::Transient(reason));
        }
    }

    Ok(root)
}

/// Tests one key on its own. Used for the per-key results in Pengaturan; it
/// deliberately does not rotate, and never receives a whole pool.
pub async fn test_key(key: &str) -> KeyTestResult {
    let key = key.trim();
    if key.is_empty() {
        return KeyTestResult {
            ok: false,
            message: Some("Kunci belum diatur".into()),
        };
    }

    let client = http_client(REQUEST_TIMEOUT_SECS);
    let url = format!("{BASE_URL}/models?pageSize=50");
    match with_key(client.get(&url), key).send().await {
        Ok(res) => {
            if res.status().is_success() {
                if let Ok(v) = res.json::<Value>().await {
                    let count = v.get("models").and_then(|m| m.as_array()).map(|a| a.len()).unwrap_or(0);
                    KeyTestResult {
                        ok: true,
                        message: Some(format!("Terhubung · {count}+ model tersedia")),
                    }
                } else {
                    KeyTestResult { ok: true, message: Some("Terhubung".into()) }
                }
            } else {
                let message = match read_failure(res).await {
                    GeminiFailure::Quota => friendly_error("resource_exhausted"),
                    GeminiFailure::InvalidKey => friendly_error("api key not valid"),
                    // A busy-check is not the key's fault; say so plainly.
                    GeminiFailure::Transient(_) => {
                        "Model Gemini sedang sibuk saat dites. Coba lagi sebentar.".to_string()
                    }
                    GeminiFailure::Other(err) => friendly_error(&err),
                };
                KeyTestResult {
                    ok: false,
                    message: Some(message),
                }
            }
        }
        Err(e) => KeyTestResult {
            ok: false,
            message: Some(friendly_error(&e.to_string())),
        },
    }
}

pub fn list_voices() -> Vec<Value> {
    GEMINI_VOICES
        .iter()
        .map(|(id, name, desc)| {
            json!({
                "id": id,
                "name": name,
                "description": desc,
                "previewUrl": null
            })
        })
        .collect()
}

pub async fn list_models(key_pool: &str, kind: &str) -> Result<Vec<Value>, String> {
    let client = http_client(REQUEST_TIMEOUT_SECS);
    let url = format!("{BASE_URL}/models?pageSize=200");

    with_pool(key_pool, |key| {
        let client = &client;
        let url = &url;
        async move {
            let res = with_key(client.get(url), &key)
                .send()
                .await
                .map_err(|e| transport_failure(e))?;

            if !res.status().is_success() {
                return Err(read_failure(res).await);
            }

            let root: Value = res
                .json()
                .await
                .map_err(|e| GeminiFailure::Other(e.to_string()))?;
            Ok(filter_models(&root, kind))
        }
    })
    .await
}

fn filter_models(root: &Value, kind: &str) -> Vec<Value> {
    let mut out = Vec::new();

    let not_text = Regex::new(r"(?i)tts|embedding|imagen|image|veo|native-audio|live|aqa|transcribe|robotics|computer-use|lyria").unwrap();

    if let Some(models) = root.get("models").and_then(|m| m.as_array()) {
        for m in models {
            let full_name = m.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let id = full_name.trim_start_matches("models/");
            if id.is_empty() {
                continue;
            }

            // The REST API reports `supportedGenerationMethods`; keep the older
            // `supportedActions` spelling as a fallback. If neither is present we
            // don't filter, rather than silently dropping every model.
            let actions: Vec<&str> = m
                .get("supportedGenerationMethods")
                .or_else(|| m.get("supportedActions"))
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str()).collect())
                .unwrap_or_default();

            if !actions.is_empty() && !actions.contains(&"generateContent") {
                continue;
            }

            let is_tts = id.to_lowercase().contains("tts");
            if kind == "tts" {
                if !is_tts {
                    continue;
                }
            } else if not_text.is_match(id) {
                continue;
            }

            let display_name = m.get("displayName").and_then(|v| v.as_str()).unwrap_or(id);
            let description = m.get("description").and_then(|v| v.as_str()).unwrap_or("");
            // Trim to ~280 bytes on a char boundary; slicing mid-codepoint panics.
            let description = if description.len() > 280 {
                let mut end = 280;
                while end > 0 && !description.is_char_boundary(end) {
                    end -= 1;
                }
                &description[..end]
            } else {
                description
            };
            let context_length = m.get("inputTokenLimit").and_then(|v| v.as_i64());

            let mut tags = Vec::new();
            if kind == "text" && id.starts_with("gemini") {
                tags.push("JSON".to_string());
            }

            out.push(json!({
                "id": id,
                "name": display_name,
                "description": description,
                "contextLength": context_length,
                "priceIn": null,
                "priceOut": null,
                "tags": tags
            }));
        }
    }

    out.sort_by(|a, b| {
        let a_id = a["id"].as_str().unwrap_or("");
        let b_id = b["id"].as_str().unwrap_or("");
        let ga = if a_id.starts_with("gemini") { 0 } else { 1 };
        let gb = if b_id.starts_with("gemini") { 0 } else { 1 };
        if ga != gb {
            ga.cmp(&gb)
        } else {
            b_id.cmp(a_id)
        }
    });

    out
}

pub async fn generate_json(
    key_pool: &str,
    model: &str,
    system: &str,
    prompt: &str,
    schema: &Value,
) -> Result<Value, String> {
    // Storyboards send every scene's description in one call; give it the long
    // text budget rather than the short metadata one.
    let client = http_client(TEXT_TIMEOUT_SECS);
    let structured = json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "systemInstruction": { "parts": [{ "text": system }] },
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseSchema": schema
        }
    });
    let fallback_system = format!(
        "{system}\n\nReply with ONE JSON object only (no markdown, no commentary) that matches this JSON Schema:\n{}",
        serde_json::to_string(schema).unwrap_or_default()
    );
    let fallback = json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "systemInstruction": { "parts": [{ "text": fallback_system }] }
    });

    with_pool(key_pool, |key| {
        let client = &client;
        let structured = &structured;
        let fallback = &fallback;
        async move {
            // Keep the same key for the prompt-guided fallback. On quota/auth
            // failure move straight to the next key instead of retrying this one.
            let first = post_attempt(client, model, &key, structured).await;
            match first {
                Ok(root) => {
                    if let Some(text) = root["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                        if let Ok(value) = serde_json::from_str::<Value>(text) {
                            return Ok(value);
                        }
                    }
                }
                Err(GeminiFailure::Other(err)) if err.contains("responseSchema") || err.contains("response_schema") || err.contains("INVALID_ARGUMENT") => {
                    // A model may reject responseSchema but accept a prompt-guided response.
                }
                Err(failure) => return Err(failure),
            }

            let root = post_attempt(client, model, &key, fallback).await?;
            let text = root["candidates"][0]["content"]["parts"][0]["text"]
                .as_str()
                .ok_or_else(|| GeminiFailure::Other("Gemini tidak mengembalikan teks".to_string()))?;
            extract_json(text).map_err(GeminiFailure::Other)
        }
    }).await
}

/// Endpoint for one speech request. Kept separate from the HTTP call so the
/// chosen model and missing-setting behavior can be tested without a network request.
fn tts_endpoint(tts_model: &str) -> Result<String, String> {
    let tts_model = tts_model.trim();
    if tts_model.is_empty() {
        return Err("Model suara Gemini belum dipilih. Buka Pengaturan lalu pilih Model suara.".to_string());
    }
    Ok(endpoint(tts_model))
}

pub async fn speak_with_model(
    key_pool: &str,
    tts_model: &str,
    text: &str,
    voice: &str,
    _language_code: &str,
) -> Result<Vec<u8>, String> {
    // Never substitute a model of our own: the user picks this in Pengaturan.
    let url = tts_endpoint(tts_model)?;
    let voice = voice.trim().to_string();
    let client = http_client(TTS_TIMEOUT_SECS);

    let body = json!({
        "contents": [{
            "role": "user",
            "parts": [{
                "text": text,
                "speechMetadata": {
                    "style": "natural storytelling narrator, clear and engaging, steady pace"
                }
            }]
        }],
        "generationConfig": {
            "responseModalities": ["AUDIO"],
            "speechConfig": {
                "voiceConfig": {
                    "prebuiltVoiceConfig": {
                        "voiceName": voice
                    }
                }
            }
        }
    });

    with_pool(key_pool, |key| {
        let client = &client;
        let url = &url;
        let body = &body;
        async move {
            let res = with_key(client.post(url).json(body), &key)
                .send()
                .await
                .map_err(|e| transport_failure(e))?;

            if !res.status().is_success() {
                return Err(read_failure(res).await);
            }

            let root: Value = res
                .json()
                .await
                .map_err(|e| GeminiFailure::Other(e.to_string()))?;

            // Prefer an overload/transient reading over "no audio" when the
            // provider explains that it is busy rather than returning content.
            let has_candidates = root["candidates"]
                .as_array()
                .map(|c| !c.is_empty())
                .unwrap_or(false);
            if !has_candidates {
                if let GeminiFailure::Transient(reason) = classify_body(200, &root.to_string()) {
                    return Err(GeminiFailure::Transient(reason));
                }
            }

            let inline_data = &root["candidates"][0]["content"]["parts"][0]["inlineData"];
            let b64 = inline_data["data"]
                .as_str()
                .ok_or_else(|| GeminiFailure::Other("Gemini tidak mengirim data audio".to_string()))?;

            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| GeminiFailure::Other(e.to_string()))?;

            if is_wav(&bytes) {
                return Ok(bytes);
            }

            let mime = inline_data["mimeType"].as_str().unwrap_or("");
            let re_rate = Regex::new(r"rate=(\d+)").unwrap();
            let rate = re_rate
                .captures(mime)
                .and_then(|c| c.get(1)?.as_str().parse::<u32>().ok())
                .unwrap_or(24000);

            Ok(pcm_to_wav(&bytes, rate, 1, 16))
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::{
        classify_body, classify_status, describe_transport, filter_models, mask_key, next_start_sequence,
        parse_key_pool, pool_run, start_for, tts_endpoint, transport_failure, GeminiFailure,
    };
    use serde_json::json;

    // ----- key pool -----

    #[test]
    fn parse_key_pool_trims_drops_blank_lines_and_duplicates() {
        let raw = "  AIzaOne \r\n\nAIzaTwo\nAIzaOne\n   \nAIzaThree\t\n";
        assert_eq!(
            parse_key_pool(raw),
            vec![
                "AIzaOne".to_string(),
                "AIzaTwo".to_string(),
                "AIzaThree".to_string(),
            ]
        );
    }

    #[test]
    fn parse_key_pool_reads_a_single_legacy_key() {
        assert_eq!(parse_key_pool("AIzaSolo"), vec!["AIzaSolo".to_string()]);
        assert!(parse_key_pool("   \n  ").is_empty());
        assert!(parse_key_pool("").is_empty());
    }

    #[test]
    fn mask_key_never_shows_the_middle_or_splits_a_character() {
        let masked = mask_key("AIzaSyABCDEFGHIJKLMNOP");
        assert_eq!(masked, "AIzaSy••••••MNOP");

        // A short key is fully hidden rather than partly revealed.
        assert_eq!(mask_key("short"), "•••••");
        assert_eq!(mask_key(""), "••••");

        // Multi-byte characters must not panic the slicing.
        assert_eq!(mask_key("kunci-rahasia-panjang-🌟"), "kunci-••••••ng-🌟");
    }

    // ----- round robin -----

    #[test]
    fn start_for_walks_the_pool_and_wraps_around() {
        let counter = std::sync::atomic::AtomicUsize::new(0);
        let seq = next_start_sequence(&counter, 3, 7);
        assert_eq!(seq, vec![0, 1, 2, 0, 1, 2, 0]);
    }

    #[test]
    fn start_for_handles_an_empty_pool() {
        assert_eq!(start_for(5, 0), 0);
    }

    // ----- classification -----

    #[test]
    fn classify_status_maps_quota_and_rejected_keys() {
        assert_eq!(classify_status(429), GeminiFailure::Quota);
        assert_eq!(classify_status(401), GeminiFailure::InvalidKey);
        assert_eq!(classify_status(403), GeminiFailure::InvalidKey);
        assert!(matches!(classify_status(400), GeminiFailure::Other(_)));
    }

    #[test]
    fn classify_status_treats_server_errors_as_transient() {
        // Google reports an overloaded model as 503; another key is often served
        // by different capacity, so the request should move on rather than stop.
        for status in [500, 502, 503, 504] {
            assert!(
                matches!(classify_status(status), GeminiFailure::Transient(_)),
                "HTTP {status} should be rotatable"
            );
        }
    }

    #[test]
    fn classify_body_reads_a_busy_model_from_the_wording() {
        let busy = "This model is currently experiencing high demand. Spikes in demand are usually temporary. Please try again later.";
        assert!(matches!(classify_body(503, busy), GeminiFailure::Transient(_)));

        // The same wording can arrive wrapped in a 200 with no candidates.
        assert!(matches!(classify_body(200, busy), GeminiFailure::Transient(_)));
        assert!(matches!(classify_body(200, "model is overloaded"), GeminiFailure::Transient(_)));

        // A real request error keeps its own classification and is not rotated.
        assert!(matches!(
            classify_body(400, "Invalid JSON payload received"),
            GeminiFailure::Other(_)
        ));
        assert_eq!(classify_body(400, "API key not valid"), GeminiFailure::InvalidKey);
    }

    #[test]
    fn transport_errors_name_their_cause_not_just_the_url() {
        // The reported bug: the toast said only "error sending request for url
        // (…)" — reqwest's Display drops the reason, which lives in source().
        let err = reqwest::Client::new()
            .get("http://127.0.0.1:1/never")
            .send();
        let err = tauri::async_runtime::block_on(err).unwrap_err();
        let described = describe_transport(&err);

        assert!(described.to_lowercase().contains("connection")
            || described.to_lowercase().contains("refused")
            || described.to_lowercase().contains("connect"),
            "expected a cause in {described}");
        assert!(!described.contains("user:"), "no token may appear: {described}");

        // Transport failures remain non-retriable: Gemini may already have
        // received the request, so an automatic retry could duplicate billing.
        assert!(matches!(
            transport_failure(err),
            GeminiFailure::Other(_)
        ));
    }

    // ----- rotation -----

    #[test]
    fn pool_run_moves_to_the_next_key_after_a_quota_failure() {
        let mut tried: Vec<String> = Vec::new();
        let out: Result<&str, String> = tauri::async_runtime::block_on(pool_run(
            "k1\nk2\nk3",
            0,
            |key| {
                tried.push(key.clone());
                async move {
                    if key == "k1" {
                        Err(GeminiFailure::Quota)
                    } else {
                        Ok("ok")
                    }
                }
            },
        ));
        assert_eq!(out.unwrap(), "ok");
        assert_eq!(tried, vec!["k1".to_string(), "k2".to_string()]);
    }

    #[test]
    fn pool_run_starts_from_the_given_round_robin_position() {
        let mut tried: Vec<String> = Vec::new();
        let out: Result<&str, String> = tauri::async_runtime::block_on(pool_run(
            "k1\nk2\nk3",
            2,
            |key| {
                tried.push(key.clone());
                async move { Ok("ok") }
            },
        ));
        assert_eq!(out.unwrap(), "ok");
        assert_eq!(tried, vec!["k3".to_string()], "the third call starts at the second key");
    }

    #[test]
    fn pool_run_counts_quota_and_rejected_keys_when_all_fail() {
        let out: Result<&str, String> = tauri::async_runtime::block_on(pool_run(
            "k1\nk2\nk3",
            0,
            |key| async move {
                if key == "k2" {
                    Err(GeminiFailure::InvalidKey)
                } else {
                    Err(GeminiFailure::Quota)
                }
            },
        ));
        let err = out.unwrap_err();
        assert!(err.contains("3 kunci"), "unexpected error: {err}");
        assert!(err.contains("2 terkena batas kuota"), "unexpected error: {err}");
        assert!(err.contains("1 ditolak"), "unexpected error: {err}");
    }

    #[test]
    fn pool_run_does_not_rotate_a_real_request_error() {
        let mut tried = 0;
        let out: Result<&str, String> = tauri::async_runtime::block_on(pool_run(
            "k1\nk2\nk3",
            0,
            |_key| {
                tried += 1;
                async move { Err(GeminiFailure::Other("model salah".into())) }
            },
        ));
        assert!(out.is_err());
        assert_eq!(tried, 1, "a bad model name must not be retried on every key");
    }

    #[test]
    fn pool_run_keeps_rotating_when_a_key_is_busy() {
        // The reported bug: a "high demand" (overloaded) key used to stop the
        // pool with "Gemini gagal:" instead of moving on to the next key.
        let mut tried: Vec<String> = Vec::new();
        let out: Result<&str, String> = tauri::async_runtime::block_on(pool_run(
            "busy\nok",
            0,
            |key| {
                tried.push(key.clone());
                async move {
                    if key == "busy" {
                        Err(GeminiFailure::Transient("high demand".into()))
                    } else {
                        Ok("ok")
                    }
                }
            },
        ));
        assert_eq!(out.unwrap(), "ok");
        assert_eq!(tried, vec!["busy".to_string(), "ok".to_string()]);
    }

    #[test]
    fn pool_run_counts_busy_keys_when_all_are_overloaded() {
        let out: Result<&str, String> = tauri::async_runtime::block_on(pool_run(
            "k1\nk2\nk3",
            0,
            |_key| async move { Err(GeminiFailure::Transient("overloaded".into())) },
        ));
        let err = out.unwrap_err();
        assert!(err.contains("3 kunci"), "unexpected error: {err}");
        assert!(err.contains("sedang sibuk"), "unexpected error: {err}");
    }

    #[test]
    fn pool_run_skips_a_rejected_key() {
        let mut tried: Vec<String> = Vec::new();
        let out: Result<&str, String> = tauri::async_runtime::block_on(pool_run(
            "bad\ngood",
            0,
            |key| {
                tried.push(key.clone());
                async move {
                    if key == "bad" {
                        Err(GeminiFailure::InvalidKey)
                    } else {
                        Ok("ok")
                    }
                }
            },
        ));
        assert_eq!(out.unwrap(), "ok");
        assert_eq!(tried, vec!["bad".to_string(), "good".to_string()]);
    }

    #[test]
    fn pool_run_rejects_an_empty_pool_without_calling_anything() {
        let mut calls = 0;
        let out: Result<&str, String> = tauri::async_runtime::block_on(pool_run(
            "  \n ",
            0,
            |_key| {
                calls += 1;
                async move { Ok("never") }
            },
        ));
        assert!(out.unwrap_err().contains("Pengaturan"));
        assert_eq!(calls, 0);
    }

    #[test]
    fn pool_run_never_puts_a_key_in_the_error_message() {
        let secret = "AIzaSuperSecretKeyValue";
        let out: Result<&str, String> = tauri::async_runtime::block_on(pool_run(
            secret,
            0,
            |key| async move { Err(GeminiFailure::Other(format!("gagal dengan {key}"))) },
        ));
        assert!(!out.unwrap_err().contains(secret));
    }

    // ----- model list -----

    #[test]
    fn list_models_reads_supported_generation_methods() {
        let models = json!({
            "models": [
                {
                    "name": "models/gemini-3.8-flash-lite-tts",
                    "displayName": "Gemini 3.8 Flash Lite TTS",
                    "description": "Model text to speech",
                    "supportedGenerationMethods": ["generateContent"]
                },
                {
                    "name": "models/gemini-3.8-flash",
                    "supportedGenerationMethods": ["generateContent"]
                },
                {
                    "name": "models/embedding-001",
                    "supportedGenerationMethods": ["embedContent"]
                }
            ]
        });

        let tts = filter_models(&models, "tts");
        assert_eq!(tts.len(), 1);
        assert_eq!(tts[0]["id"], "gemini-3.8-flash-lite-tts");

        let text = filter_models(&models, "text");
        assert_eq!(text.len(), 1);
        assert_eq!(text[0]["id"], "gemini-3.8-flash");
    }

    #[test]
    fn list_models_keeps_unicode_description_truncation_safe() {
        let long_description = format!("{}説明", "a".repeat(279));
        let models = json!({
            "models": [{
                "name": "models/gemini-tts",
                "description": long_description,
                "supportedGenerationMethods": ["generateContent"]
            }]
        });
        let result = filter_models(&models, "tts");
        assert_eq!(result.len(), 1);
        assert!(result[0]["description"].as_str().unwrap().len() <= 280);
    }

    // ----- speech endpoint -----

    #[test]
    fn tts_endpoint_uses_the_configured_model() {
        let url = tts_endpoint("gemini-3.8-flash-tts").unwrap();
        assert!(url.contains("/models/gemini-3.8-flash-tts:generateContent"));

        // A different id must reach the URL untouched, so no model is baked in.
        let url = tts_endpoint("gemini-9.9-other-tts").unwrap();
        assert!(url.contains("/models/gemini-9.9-other-tts:generateContent"));
        assert!(!url.contains("gemini-2.0-flash"));
    }

    #[test]
    fn tts_endpoint_rejects_a_missing_model() {
        for empty in ["", "   "] {
            let err = tts_endpoint(empty).unwrap_err();
            assert!(err.contains("Pengaturan"), "unexpected error: {err}");
        }
    }
}

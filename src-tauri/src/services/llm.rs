use regex::Regex;
use reqwest::Client;
use serde_json::{json, Value};

use crate::db::repo::{get_secret, get_settings, read_cache, write_cache};
use crate::db::DbPool;
use crate::secrets::KeyTestResult;
use crate::services::http::read_error;
use crate::services::json::extract_json;
use crate::services::{elevenlabs, gemini};

pub fn provider_base_url(provider: &str, custom_base: &str) -> Result<String, String> {
    match provider {
        "openrouter" => Ok("https://openrouter.ai/api/v1".to_string()),
        "groq" => Ok("https://api.groq.com/openai/v1".to_string()),
        "custom" => {
            let b = custom_base.trim().trim_end_matches('/');
            if b.is_empty() {
                Err("Alamat endpoint custom belum diisi. Buka Pengaturan untuk menambahkannya.".into())
            } else {
                Ok(b.to_string())
            }
        }
        _ => Err(format!("Unknown provider: {provider}")),
    }
}

pub async fn test_llm(provider: &str, key: Option<&str>, base_override: Option<&str>) -> KeyTestResult {
    let custom_base = base_override.unwrap_or("");
    let base = match provider_base_url(provider, custom_base) {
        Ok(b) => b,
        Err(e) => return KeyTestResult { ok: false, message: Some(e) },
    };

    let client = Client::new();
    let url = format!("{base}/models");
    let mut req = client.get(&url);

    if let Some(k) = key {
        if !k.is_empty() {
            req = req.bearer_auth(k);
        }
    }
    if provider == "openrouter" {
        req = req.header("X-OpenRouter-Title", "De Story");
    }

    match req.send().await {
        Ok(res) => {
            if res.status().is_success() {
                KeyTestResult { ok: true, message: Some("Terhubung".into()) }
            } else {
                let err = read_error(res).await;
                KeyTestResult { ok: false, message: Some(format!("Gagal ({provider}): {err}")) }
            }
        }
        Err(e) => KeyTestResult {
            ok: false,
            message: Some(format!("Tidak bisa terhubung ke {provider}: {e}")),
        },
    }
}

pub async fn fetch_generic_openai_models(provider: &str, key: Option<&str>, base: &str) -> Result<Vec<Value>, String> {
    let base_url = provider_base_url(provider, base)?;
    let client = Client::new();
    let url = format!("{base_url}/models");
    let mut req = client.get(&url);

    if let Some(k) = key {
        if !k.is_empty() {
            req = req.bearer_auth(k);
        }
    }
    if provider == "openrouter" {
        req = req.header("X-OpenRouter-Title", "De Story");
    }

    let res = req.send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        let err = read_error(res).await;
        return Err(format!("Gagal memuat model: {err}"));
    }

    let root: Value = res.json().await.map_err(|e| e.to_string())?;
    let mut out = Vec::new();

    if let Some(arr) = root.get("data").and_then(|x| x.as_array()) {
        for m in arr {
            let id = m.get("id").and_then(|x| x.as_str()).unwrap_or("");
            if id.is_empty() || id.ends_with(":batch") {
                continue;
            }

            let name = m.get("name").and_then(|x| x.as_str()).unwrap_or(id);
            let description = m.get("description").and_then(|x| x.as_str());
            let context_length = m.get("context_length").and_then(|x| x.as_i64());

            let mut tags = Vec::new();
            if let Some(params) = m.get("supported_parameters").and_then(|x| x.as_array()) {
                let p_strs: Vec<&str> = params.iter().filter_map(|p| p.as_str()).collect();
                if p_strs.contains(&"structured_outputs") || p_strs.contains(&"response_format") {
                    tags.push("JSON".to_string());
                }
            }

            out.push(json!({
                "id": id,
                "name": name,
                "description": description.map(|d| if d.len() > 280 { &d[..280] } else { d }),
                "contextLength": context_length,
                "priceIn": null,
                "priceOut": null,
                "tags": tags
            }));
        }
    }

    Ok(out)
}

pub async fn generate_json(
    provider: &str,
    key: &str,
    base: &str,
    model: &str,
    system: &str,
    prompt: &str,
    schema: &Value,
) -> Result<Value, String> {
    let base_url = provider_base_url(provider, base)?;
    let client = Client::new();
    let url = format!("{base_url}/chat/completions");

    // 1. Try with response_format = json_object
    let body1 = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": format!("{system}\n\nSchema:\n{}", serde_json::to_string(schema).unwrap_or_default())},
            {"role": "user", "content": prompt}
        ],
        "response_format": {"type": "json_object"}
    });

    let mut req1 = client.post(&url).bearer_auth(key).json(&body1);
    if provider == "openrouter" {
        req1 = req1.header("X-OpenRouter-Title", "De Story");
    }

    if let Ok(res) = req1.send().await {
        if res.status().is_success() {
            if let Ok(root) = res.json::<Value>().await {
                if let Some(content) = root["choices"][0]["message"]["content"].as_str() {
                    if let Ok(val) = serde_json::from_str::<Value>(content) {
                        return Ok(val);
                    }
                }
            }
        }
    }

    // 2. Fallback: prompt only without response_format
    let fallback_system = format!(
        "{system}\n\nReply with ONE valid JSON object only (no commentary, no markdown fences) matching this JSON Schema:\n{}",
        serde_json::to_string(schema).unwrap_or_default()
    );

    let body2 = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": fallback_system},
            {"role": "user", "content": prompt}
        ]
    });

    let mut req2 = client.post(&url).bearer_auth(key).json(&body2);
    if provider == "openrouter" {
        req2 = req2.header("X-OpenRouter-Title", "De Story");
    }

    let res = req2.send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        let err = read_error(res).await;
        return Err(format!("Llm error: {err}"));
    }

    let root: Value = res.json().await.map_err(|e| e.to_string())?;
    let content = root["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| "No message content in LLM reply".to_string())?;

    extract_json(content)
}

async fn openrouter_models(key: Option<&str>) -> Result<Vec<Value>, String> {
    let client = Client::new();
    let mut req = client
        .get("https://openrouter.ai/api/v1/models")
        .header("X-OpenRouter-Title", "De Story");
    if let Some(k) = key {
        if !k.is_empty() {
            req = req.bearer_auth(k);
        }
    }
    let res = req.send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        let err = read_error(res).await;
        return Err(format!("OpenRouter error: {err}"));
    }
    let root: Value = res.json().await.map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    if let Some(data) = root.get("data").and_then(|d| d.as_array()) {
        for m in data {
            let id = m.get("id").and_then(|x| x.as_str()).unwrap_or("");
            if id.ends_with(":batch") {
                continue;
            }
            if let Some(arch) = m.get("architecture") {
                if let Some(outs) = arch.get("output_modalities").and_then(|o| o.as_array()) {
                    if !outs.iter().any(|v| v.as_str() == Some("text")) {
                        continue;
                    }
                }
            }
            let name = m.get("name").and_then(|x| x.as_str()).unwrap_or(id);
            let desc = m.get("description").and_then(|x| x.as_str()).map(|s| {
                if s.len() > 280 {
                    s[..280].to_string()
                } else {
                    s.to_string()
                }
            });
            let ctx = m
                .get("context_length")
                .or_else(|| m.get("top_provider").and_then(|tp| tp.get("context_length")))
                .and_then(|x| x.as_u64());

            let mut tags = Vec::new();
            if let Some(params) = m.get("supported_parameters").and_then(|p| p.as_array()) {
                if params.iter().any(|p| {
                    p.as_str() == Some("structured_outputs") || p.as_str() == Some("response_format")
                }) {
                    tags.push("JSON".to_string());
                }
                if params.iter().any(|p| p.as_str() == Some("reasoning")) {
                    tags.push("Reasoning".to_string());
                }
            }
            if m.get("reasoning").and_then(|r| r.as_bool()).unwrap_or(false)
                && !tags.iter().any(|t| t == "Reasoning")
            {
                tags.push("Reasoning".to_string());
            }

            let p_in = m
                .get("pricing")
                .and_then(|p| p.get("prompt"))
                .and_then(|x| x.as_f64())
                .map(|n| (n * 1e6 * 10000.0).round() / 10000.0);
            let p_out = m
                .get("pricing")
                .and_then(|p| p.get("completion"))
                .and_then(|x| x.as_f64())
                .map(|n| (n * 1e6 * 10000.0).round() / 10000.0);

            if p_in == Some(0.0) && p_out == Some(0.0) {
                tags.push("Gratis".to_string());
            }

            out.push(json!({
                "id": id,
                "name": name,
                "description": desc,
                "contextLength": ctx,
                "priceIn": p_in,
                "priceOut": p_out,
                "tags": tags
            }));
        }
    }
    Ok(out)
}

async fn groq_models(key: &str) -> Result<Vec<Value>, String> {
    if key.is_empty() {
        return Err("Kunci API Groq belum diatur.".into());
    }
    let client = Client::new();
    let res = client
        .get("https://api.groq.com/openai/v1/models")
        .bearer_auth(key)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        let err = read_error(res).await;
        return Err(format!("Groq error: {err}"));
    }
    let root: Value = res.json().await.map_err(|e| e.to_string())?;
    let not_chat = Regex::new(r"(?i)whisper|tts|orpheus|playai|guard|safeguard|embed").unwrap();
    let mut out = Vec::new();
    if let Some(data) = root.get("data").and_then(|d| d.as_array()) {
        for m in data {
            let id = m.get("id").and_then(|x| x.as_str()).unwrap_or("");
            let active = m.get("active").and_then(|x| x.as_bool()).unwrap_or(true);
            if !active || not_chat.is_match(id) {
                continue;
            }
            let ctx = m.get("context_window").and_then(|x| x.as_u64());
            let max_comp = m.get("max_completion_tokens").and_then(|x| x.as_u64());
            let desc = max_comp.map(|c| format!("Maks {} token keluaran", c));
            let mut tags = Vec::new();
            if let Some(owned) = m.get("owned_by").and_then(|x| x.as_str()) {
                tags.push(owned.to_string());
            }
            out.push(json!({
                "id": id,
                "name": id,
                "description": desc,
                "contextLength": ctx,
                "priceIn": null,
                "priceOut": null,
                "tags": tags
            }));
        }
    }
    out.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    Ok(out)
}

async fn custom_models(base: &str, key: Option<&str>) -> Result<Vec<Value>, String> {
    let clean = base.trim_end_matches('/');
    if clean.is_empty() {
        return Err("Alamat endpoint custom belum diisi.".into());
    }
    let client = Client::new();
    let mut req = client.get(format!("{clean}/models"));
    if let Some(k) = key {
        if !k.is_empty() {
            req = req.bearer_auth(k);
        }
    }
    let res = req.send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        let err = read_error(res).await;
        return Err(format!("Custom server error: {err}"));
    }
    let root: Value = res.json().await.map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let list = root
        .get("data")
        .and_then(|d| d.as_array())
        .or_else(|| root.get("models").and_then(|m| m.as_array()));
    if let Some(arr) = list {
        for m in arr {
            let id = m
                .get("id")
                .or_else(|| m.get("model"))
                .or_else(|| m.get("name"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            if id.is_empty() {
                continue;
            }
            let owned = m.get("owned_by").and_then(|x| x.as_str());
            let desc = owned.map(|o| format!("Pemilik: {o}"));
            out.push(json!({
                "id": id,
                "name": id,
                "description": desc,
                "contextLength": null,
                "priceIn": null,
                "priceOut": null,
                "tags": []
            }));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::model_key_provider;

    #[test]
    fn tts_uses_shared_gemini_key() {
        assert_eq!(model_key_provider("gemini-tts"), "gemini");
        for source in ["gemini", "elevenlabs", "groq", "custom", "openrouter"] {
            assert_eq!(model_key_provider(source), source);
        }
    }
}

fn model_key_provider(source: &str) -> &str {
    match source {
        "gemini-tts" => "gemini",
        _ => source,
    }
}

pub async fn list_models(db: &DbPool, source: &str, refresh: bool) -> Result<Value, String> {
    let (key_val, custom_base) = {
        let conn = db.lock().unwrap();
        let s = get_settings(&conn);
        let b = s.get("customBaseUrl").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let k = get_secret(&conn, model_key_provider(source)).unwrap_or_default();
        (k, b)
    };

    let cache_key = if source == "custom" {
        format!("models:custom:{}", custom_base.trim())
    } else {
        format!("models:{}", source)
    };

    if !refresh {
        let conn = db.lock().unwrap();
        if let Some((val, updated_at)) = read_cache(&conn, &cache_key) {
            let now = chrono::Utc::now().timestamp_millis();
            if now - updated_at < 24 * 60 * 60 * 1000 {
                return Ok(json!({
                    "models": val,
                    "fetchedAt": updated_at
                }));
            }
        }
    }

    let models = match source {
        "gemini" => gemini::list_models(&key_val, "text").await?,
        "gemini-tts" => gemini::list_models(&key_val, "tts").await?,
        "elevenlabs" => elevenlabs::list_models(&key_val).await?,
        "openrouter" => openrouter_models(Some(&key_val)).await?,
        "groq" => groq_models(&key_val).await?,
        "custom" => custom_models(&custom_base, Some(&key_val)).await?,
        _ => return Err(format!("Sumber model tidak dikenal: {source}")),
    };

    let now = chrono::Utc::now().timestamp_millis();
    {
        let conn = db.lock().unwrap();
        let _ = write_cache(&conn, &cache_key, &json!(models));
    }

    Ok(json!({
        "models": models,
        "fetchedAt": now
    }))
}


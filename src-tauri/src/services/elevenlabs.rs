use reqwest::Client;
use serde_json::{json, Value};
use crate::secrets::KeyTestResult;
use crate::services::http::read_error;
use crate::speech::align::WordTiming;
use base64::Engine;

const BASE_URL: &str = "https://api.elevenlabs.io";

pub async fn test_key(key: &str) -> KeyTestResult {
    if key.trim().is_empty() {
        return KeyTestResult {
            ok: false,
            message: Some("Kunci belum diatur".into()),
        };
    }

    let client = Client::new();
    let url = format!("{BASE_URL}/v1/user/subscription");
    match client.get(&url).header("xi-api-key", key).send().await {
        Ok(res) => {
            if res.status().is_success() {
                if let Ok(v) = res.json::<Value>().await {
                    let count = v.get("character_count").and_then(|x| x.as_i64()).unwrap_or(0);
                    let limit = v.get("character_limit").and_then(|x| x.as_i64()).unwrap_or(0);
                    let left = (limit - count).max(0);
                    KeyTestResult {
                        ok: true,
                        message: Some(format!("Terhubung · sisa {left} karakter")),
                    }
                } else {
                    KeyTestResult { ok: true, message: Some("Terhubung".into()) }
                }
            } else {
                let err = read_error(res).await;
                KeyTestResult {
                    ok: false,
                    message: Some(format!("ElevenLabs gagal: {err}")),
                }
            }
        }
        Err(e) => KeyTestResult {
            ok: false,
            message: Some(format!("ElevenLabs gagal: {e}")),
        },
    }
}

pub async fn list_voices(key: &str) -> Result<Vec<Value>, String> {
    let client = Client::new();
    let mut out = Vec::new();
    let mut token: Option<String> = None;

    for _page in 0..5 {
        let mut url = format!("{BASE_URL}/v2/voices?page_size=100&include_total_count=false");
        if let Some(ref tok) = token {
            url.push_str(&format!("&next_page_token={tok}"));
        }

        let res = client.get(&url).header("xi-api-key", key).send().await.map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            let err = read_error(res).await;
            return Err(format!("ElevenLabs gagal: {err}"));
        }

        let root: Value = res.json().await.map_err(|e| e.to_string())?;
        if let Some(voices) = root.get("voices").and_then(|v| v.as_array()) {
            for v in voices {
                let voice_id = v.get("voice_id").and_then(|x| x.as_str()).unwrap_or("");
                let name = v.get("name").and_then(|x| x.as_str()).unwrap_or(voice_id);
                let preview = v.get("preview_url").and_then(|x| x.as_str());

                let mut labels = Vec::new();
                if let Some(lbl_obj) = v.get("labels").and_then(|x| x.as_object()) {
                    for val in lbl_obj.values() {
                        if let Some(s) = val.as_str() {
                            if !s.is_empty() {
                                labels.push(s);
                            }
                        }
                    }
                }
                let desc = if !labels.is_empty() {
                    labels.into_iter().take(3).collect::<Vec<_>>().join(", ")
                } else {
                    v.get("category").and_then(|x| x.as_str()).unwrap_or("").to_string()
                };

                out.push(json!({
                    "id": voice_id,
                    "name": name,
                    "description": desc,
                    "previewUrl": preview
                }));
            }
        }

        let has_more = root.get("has_more").and_then(|x| x.as_bool()).unwrap_or(false);
        token = root.get("next_page_token").and_then(|x| x.as_str()).map(|s| s.to_string());

        if !has_more || token.is_none() {
            break;
        }
    }

    Ok(out)
}

pub async fn list_models(key: &str) -> Result<Vec<Value>, String> {
    let client = Client::new();
    let url = format!("{BASE_URL}/v1/models");
    let res = client.get(&url).header("xi-api-key", key).send().await.map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        let err = read_error(res).await;
        return Err(format!("ElevenLabs gagal: {err}"));
    }

    let arr: Vec<Value> = res.json().await.map_err(|e| e.to_string())?;
    let mut out = Vec::new();

    for m in arr {
        if m.get("can_do_text_to_speech").and_then(|x| x.as_bool()) == Some(false) {
            continue;
        }
        let id = m.get("model_id").and_then(|x| x.as_str()).unwrap_or("");
        let name = m.get("name").and_then(|x| x.as_str()).unwrap_or(id);
        let desc = m.get("description").and_then(|x| x.as_str()).unwrap_or("");

        let mut tags = Vec::new();
        if let Some(langs) = m.get("languages").and_then(|x| x.as_array()) {
            for l in langs {
                let code = l.get("language_id").and_then(|x| x.as_str()).unwrap_or("");
                if code == "id" || code == "ind" {
                    tags.push("Indonesia".to_string());
                    break;
                }
            }
        }

        out.push(json!({
            "id": id,
            "name": name,
            "description": desc,
            "contextLength": null,
            "priceIn": null,
            "priceOut": null,
            "tags": tags
        }));
    }

    Ok(out)
}

pub async fn speak(
    key: &str,
    voice_id: &str,
    model_id: &str,
    text: &str,
    language_code: &str,
) -> Result<(Vec<u8>, Vec<WordTiming>), String> {
    let client = Client::new();
    let url = format!("{BASE_URL}/v1/text-to-speech/{voice_id}/with-timestamps");

    let body = json!({
        "text": text,
        "model_id": model_id,
        "language_code": language_code
    });

    let res = client
        .post(&url)
        .header("xi-api-key", key)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        let err = read_error(res).await;
        return Err(format!("ElevenLabs gagal: {err}"));
    }

    let root: Value = res.json().await.map_err(|e| e.to_string())?;
    let b64 = root.get("audio_base64").and_then(|x| x.as_str()).ok_or("No audio_base64 returned")?;
    let audio_bytes = base64::engine::general_purpose::STANDARD.decode(b64).map_err(|e| e.to_string())?;

    let mut words = Vec::new();
    if let Some(align) = root.get("alignment") {
        if let (Some(chars), Some(starts), Some(ends)) = (
            align.get("characters").and_then(|x| x.as_array()),
            align.get("character_start_times_seconds").and_then(|x| x.as_array()),
            align.get("character_end_times_seconds").and_then(|x| x.as_array()),
        ) {
            let mut cur: Option<WordTiming> = None;
            for i in 0..chars.len() {
                let ch = chars[i].as_str().unwrap_or("");
                let s = starts.get(i).and_then(|x| x.as_f64()).unwrap_or(0.0);
                let e = ends.get(i).and_then(|x| x.as_f64()).unwrap_or(0.0);

                if ch.chars().any(|c| c.is_whitespace()) {
                    if let Some(w) = cur.take() {
                        words.push(w);
                    }
                } else if let Some(ref mut c) = cur {
                    c.text.push_str(ch);
                    c.end = e;
                } else {
                    cur = Some(WordTiming {
                        text: ch.to_string(),
                        start: s,
                        end: e,
                    });
                }
            }
            if let Some(w) = cur {
                words.push(w);
            }
        }
    }

    Ok((audio_bytes, words))
}

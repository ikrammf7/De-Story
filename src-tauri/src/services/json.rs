use regex::Regex;
use serde_json::Value;

pub fn extract_json(raw: &str) -> Result<Value, String> {
    // 1. Remove <think>...</think> tags
    let re_think = Regex::new(r"(?is)<think>.*?</think>").unwrap();
    let without_think = re_think.replace_all(raw, "");
    let mut text = without_think.trim();

    // 2. Remove markdown code fences ```json ... ```
    let re_fence = Regex::new(r"(?is)```(?:json)?\s*([\s\S]*?)```").unwrap();
    let fence_extracted = if let Some(caps) = re_fence.captures(text) {
        if let Some(m) = caps.get(1) {
            Some(m.as_str().trim().to_string())
        } else {
            None
        }
    } else {
        None
    };

    if let Some(ref fe) = fence_extracted {
        text = fe.as_str();
    }

    // 3. Try direct parse
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        return Ok(v);
    }

    // 4. Try finding outermost { ... }
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
        if end > start {
            let slice = &text[start..=end];
            if let Ok(v) = serde_json::from_str::<Value>(slice) {
                return Ok(v);
            }
        }
    }

    // 5. Try finding outermost [ ... ]
    if let (Some(start), Some(end)) = (text.find('['), text.rfind(']')) {
        if end > start {
            let slice = &text[start..=end];
            if let Ok(v) = serde_json::from_str::<Value>(slice) {
                return Ok(v);
            }
        }
    }

    Err("Balasan model bukan JSON".to_string())
}

use std::sync::OnceLock;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedStyle {
    id: String,
    prompt: String,
    character_hint: String,
}

fn catalog() -> &'static [SharedStyle] {
    static CATALOG: OnceLock<Vec<SharedStyle>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../../../src/shared/visual-styles.json"))
            .expect("Invalid bundled visual style catalog")
    })
}

fn lookup(style_id: &str) -> &'static SharedStyle {
    let styles = catalog();
    // Match the frontend's fallback for unknown or empty legacy ids.
    styles.iter().find(|s| s.id == style_id)
        .unwrap_or_else(|| styles.first().expect("Empty bundled visual style catalog"))
}

pub fn style_prompt(style_id: &str) -> &'static str {
    &lookup(style_id).prompt
}

pub fn style_character_hint(style_id: &str) -> &'static str {
    &lookup(style_id).character_hint
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_and_unknown_ids_resolve() {
        assert!(style_prompt("anime").starts_with("High-quality anime film still"));
        assert!(style_character_hint("anime").contains("anime character"));
        for id in ["does-not-exist", ""] {
            assert_eq!(style_prompt(id), style_prompt("stickman"));
            assert_eq!(style_character_hint(id), style_character_hint("stickman"));
        }
    }

    #[test]
    fn shared_catalog_is_complete() {
        let styles = catalog();
        assert_eq!(styles.len(), 16);
        assert_eq!(styles[0].id, "stickman");
        let mut ids = std::collections::HashSet::new();
        for style in styles {
            assert!(ids.insert(&style.id), "Duplicate style id");
            assert!(!style.prompt.is_empty());
            assert!(!style.character_hint.is_empty());
            assert!(style.prompt.contains("No text"));
        }
    }
}

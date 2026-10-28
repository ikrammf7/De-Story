use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use ttf_parser::Face;

pub struct FontMetricsCache {
    font_data: Mutex<HashMap<String, Vec<u8>>>,
}

impl FontMetricsCache {
    pub fn new() -> Self {
        Self {
            font_data: Mutex::new(HashMap::new()),
        }
    }

    pub fn measure_text_advance(&self, font_path: &Path, text: &str) -> f64 {
        let path_str = font_path.to_string_lossy().to_string();
        let mut data_guard = self.font_data.lock().unwrap();

        if !data_guard.contains_key(&path_str) {
            if let Ok(bytes) = fs::read(font_path) {
                data_guard.insert(path_str.clone(), bytes);
            } else {
                return text.chars().count() as f64 * 0.55;
            }
        }

        let data = match data_guard.get(&path_str) {
            Some(d) => d,
            None => return text.chars().count() as f64 * 0.55,
        };

        let face = match Face::parse(data, 0) {
            Ok(f) => f,
            Err(_) => return text.chars().count() as f64 * 0.55,
        };

        let upm = face.units_per_em() as f64;
        if upm == 0.0 {
            return text.chars().count() as f64 * 0.55;
        }

        let mut sum = 0.0;
        for ch in text.chars() {
            if let Some(glyph_id) = face.glyph_index(ch) {
                if let Some(adv) = face.glyph_hor_advance(glyph_id) {
                    sum += (adv as f64) / upm;
                    continue;
                }
            }
            sum += 0.55;
        }
        sum
    }
}

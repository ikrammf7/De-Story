use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heard {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordTiming {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

pub fn norm(w: &str) -> String {
    w.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

pub fn similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    if a.len() >= 3 && b.len() >= 3 && (a.starts_with(b) || b.starts_with(a)) {
        return 0.8;
    }

    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    let mut prev: Vec<usize> = (0..=n).collect();
    let mut cur: Vec<usize> = vec![0; n + 1];

    for i in 1..=m {
        cur[0] = i;
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev.copy_from_slice(&cur);
    }

    1.0 - (prev[n] as f64) / (m.max(n) as f64)
}

pub fn split_heard(segments: &[Heard]) -> Vec<Heard> {
    let mut out = Vec::new();
    for s in segments {
        let words: Vec<&str> = s.text.split_whitespace().filter(|w| !norm(w).is_empty()).collect();
        if words.is_empty() {
            continue;
        }
        let total_chars: usize = words.iter().map(|w| w.len()).sum();
        let total_dur = (s.end - s.start).max(0.0);
        let mut t = s.start;
        for w in words {
            let dur = if total_chars > 0 {
                (total_dur * (w.len() as f64)) / (total_chars as f64)
            } else {
                0.0
            };
            out.push(Heard {
                text: w.to_string(),
                start: t,
                end: t + dur,
            });
            t += dur;
        }
    }
    out
}

pub fn align_words(script: &[String], heard: &[Heard], total: f64) -> Vec<WordTiming> {
    let n = script.len();
    let m = heard.len();
    if n == 0 {
        return Vec::new();
    }

    let a: Vec<String> = script.iter().map(|s| norm(s)).collect();
    let b: Vec<String> = heard.iter().map(|h| norm(&h.text)).collect();

    const GAP: f64 = 1.0;
    let cost = |i: usize, j: usize| -> f64 {
        let s = similarity(&a[i], &b[j]);
        if s >= 0.99 {
            0.0
        } else if s >= 0.6 {
            0.5
        } else {
            1.3
        }
    };

    let mut d: Vec<Vec<f64>> = vec![vec![0.0; m + 1]; n + 1];
    for i in 1..=n {
        d[i][0] = (i as f64) * GAP;
    }
    for j in 1..=m {
        d[0][j] = (j as f64) * GAP;
    }

    for i in 1..=n {
        for j in 1..=m {
            let match_cost = d[i - 1][j - 1] + cost(i - 1, j - 1);
            let del_cost = d[i - 1][j] + GAP;
            let ins_cost = d[i][j - 1] + GAP;
            d[i][j] = match_cost.min(del_cost).min(ins_cost);
        }
    }

    let mut pair = vec![-1isize; n];
    let mut i = n;
    let mut j = m;

    while i > 0 && j > 0 {
        let match_cost = d[i - 1][j - 1] + cost(i - 1, j - 1);
        if (d[i][j] - match_cost).abs() < 1e-9 {
            pair[i - 1] = (j - 1) as isize;
            i -= 1;
            j -= 1;
        } else if (d[i][j] - (d[i - 1][j] + GAP)).abs() < 1e-9 {
            i -= 1;
        } else {
            j -= 1;
        }
    }

    let mut out: Vec<WordTiming> = script
        .iter()
        .enumerate()
        .map(|(k, text)| {
            let p = pair[k];
            if p >= 0 {
                let h = &heard[p as usize];
                WordTiming {
                    text: text.clone(),
                    start: h.start,
                    end: h.end,
                }
            } else {
                WordTiming {
                    text: text.clone(),
                    start: -1.0,
                    end: -1.0,
                }
            }
        })
        .collect();

    let weak: Vec<bool> = script
        .iter()
        .enumerate()
        .map(|(k, _)| {
            let p = pair[k];
            p >= 0 && similarity(&a[k], &b[p as usize]) < 0.99
        })
        .collect();

    // Interpolate unmatched words
    let mut k = 0;
    while k < n {
        if out[k].start >= 0.0 {
            k += 1;
            continue;
        }
        let from = k;
        while k < n && out[k].start < 0.0 {
            k += 1;
        }
        let before_end = if from > 0 { Some(out[from - 1].end) } else { None };
        let after_start = if k < n { Some(out[k].start) } else { None };

        let mut start = before_end.unwrap_or(0.0);
        let mut end = after_start.unwrap_or(total);
        let run_len = k - from;

        if end - start < 0.08 * (run_len as f64) {
            if let Some(after_idx) = if k < n && weak[k] { Some(k) } else { None } {
                end = out[after_idx].end;
            } else if from > 0 {
                start = out[from - 1].start;
            }
        }
        end = end.max(start + 0.08 * (run_len as f64));

        let weights: Vec<f64> = (from..k)
            .map(|x| (norm(&out[x].text).len() as f64).max(2.0))
            .collect();
        let sum_weights: f64 = weights.iter().sum();

        let mut t = start;
        for (idx, x) in (from..k).enumerate() {
            let dur = if sum_weights > 0.0 {
                ((end - start) * weights[idx]) / sum_weights
            } else {
                (end - start) / (run_len as f64)
            };
            out[x].start = t;
            out[x].end = t + dur;
            t += dur;
        }
    }

    // Minimum length adjustment
    for x in 0..n {
        if out[x].end - out[x].start >= 0.08 {
            continue;
        }
        let limit = if x + 1 < n {
            out[x + 1].start
        } else {
            total.max(out[x].end)
        };
        out[x].end = (out[x].start + 0.3).min(limit).max(out[x].end);
    }

    out
}

pub fn strip_voice_tags(text: &str) -> String {
    // Both brackets must be escaped inside the character class: an unescaped
    // `[` opens a nested class that never closes, which makes the regex fail to
    // compile. This is called from tight loops, so the patterns are built once.
    static ANGLE: OnceLock<regex::Regex> = OnceLock::new();
    static SQUARE: OnceLock<regex::Regex> = OnceLock::new();

    let re_angle = ANGLE.get_or_init(|| regex::Regex::new(r"<[^<>]{1,40}>").expect("perintah suara <>"));
    let re_square = SQUARE.get_or_init(|| regex::Regex::new(r"\[[^\[\]]{1,40}\]").expect("perintah suara []"));

    let no_angle = re_angle.replace_all(text, " ");
    let no_square = re_square.replace_all(&no_angle, " ");
    no_square.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn spoken_words(text: &str) -> Vec<String> {
    strip_voice_tags(text)
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

pub fn estimate_word_timings(text: &str, duration_sec: f64) -> Vec<WordTiming> {
    let tokens = spoken_words(text);
    if tokens.is_empty() || duration_sec <= 0.0 {
        return Vec::new();
    }
    let lead = 0.15f64.min(duration_sec * 0.03);
    let tail = 0.25f64.min(duration_sec * 0.05);
    let usable = 0.1f64.max(duration_sec - lead - tail);

    let weight = |w: &str| -> f64 {
        let clean: String = w.chars().filter(|c| c.is_alphanumeric()).collect();
        let mut v = (clean.len() as f64).max(2.0) + 1.5;
        if w.ends_with('.') || w.ends_with('!') || w.ends_with('?') || w.ends_with('…') {
            v += 4.0;
        } else if w.ends_with(',') || w.ends_with(';') || w.ends_with(':') {
            v += 2.0;
        }
        v
    };

    let weights: Vec<f64> = tokens.iter().map(|w| weight(w)).collect();
    let total: f64 = weights.iter().sum();
    let mut t = lead;

    tokens
        .into_iter()
        .zip(weights.into_iter())
        .map(|(w, wt)| {
            let d = if total > 0.0 { (wt / total) * usable } else { 0.0 };
            let item = WordTiming {
                text: w,
                start: t,
                end: t + d,
            };
            t += d;
            item
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::strip_voice_tags;

    #[test]
    fn strip_voice_tags_takes_out_angle_and_square_directions() {
        let text = "Milo <bersemangat> berlari [pelan] ke hutan.";
        assert_eq!(strip_voice_tags(text), "Milo berlari ke hutan.");
    }

    #[test]
    fn strip_voice_tags_handles_a_square_bracket_pair() {
        // The `[` inside the character class has to be escaped; an unescaped one
        // makes the pattern fail to compile and used to panic every narration.
        assert_eq!(strip_voice_tags("[marah] Diam!"), "Diam!");
        assert_eq!(strip_voice_tags("a [x] b [y] c"), "a b c");
        assert_eq!(strip_voice_tags("tanpa perintah suara"), "tanpa perintah suara");
    }

    #[test]
    fn strip_voice_tags_keeps_long_brackets_as_text() {
        // Only short directives (up to 40 characters) are treated as directions.
        let long = format!("[{}]", "a".repeat(60));
        assert!(strip_voice_tags(&long).contains(&"a".repeat(60)));
    }
}

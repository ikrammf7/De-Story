use std::path::Path;
use serde_json::Value;
use crate::media::font_metrics::FontMetricsCache;
use crate::speech::align::WordTiming;

const CATALOG: &str = include_str!("../../../src/shared/caption-catalog.json");

fn catalog() -> Value { serde_json::from_str(CATALOG).expect("bundled caption catalog") }
fn number(v: &Value, key: &str, fallback: f64) -> f64 {
    v.get(key).and_then(Value::as_f64).filter(|n| n.is_finite()).unwrap_or(fallback)
}
fn text<'a>(v: &'a Value, key: &str, fallback: &'a str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or(fallback)
}
fn contrast_ink(hex: &str) -> &'static str {
    let h = hex.trim_start_matches('#');
    let rgb = (0..3).map(|i| u8::from_str_radix(h.get(i * 2..i * 2 + 2).unwrap_or(""), 16).unwrap_or(255) as f64 / 255.0).collect::<Vec<_>>();
    if 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2] > 0.55 { "#1F1D1A" } else { "#FFFFFF" }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedCaption {
    pub font: String,
    pub font_file: String,
    pub cell: f64,
    pub weight: i64,
    pub size: f64,
    pub color: String,
    pub highlight: String,
    pub bg: Option<String>,
    pub bg_alpha: f64,
    pub outline: f64,
    pub outline_color: String,
    pub uppercase: bool,
}

pub fn resolve_caption(editor: &Value, catalog: &Value) -> Option<ResolvedCaption> {
    let style = catalog["styles"].as_array()?.iter().find(|s| s["id"].as_str() == Some(text(editor, "captionStyle", "karaoke")))?;
    let fonts = catalog["fonts"].as_array()?;
    let font = fonts.iter().find(|f| f["id"].as_str() == editor["captionFont"].as_str()).unwrap_or(&fonts[0]);
    let is_poppins = font["id"] == "poppins";
    let weight = if is_poppins { number(style, "weight", 700.0) as i64 } else { 400 };
    let font_file = if is_poppins {
        catalog["poppinsFiles"].get(weight.to_string()).and_then(Value::as_str)
            .or_else(|| catalog["poppinsFiles"]["700"].as_str()).unwrap_or("Poppins-Bold.ttf")
    } else { text(font, "file", "Poppins-Bold.ttf") };
    let color = text(editor, "captionColor", text(style, "color", "#FFFFFF")).to_string();
    let bg = if let Some(v) = editor.get("captionBg") {
        v.get("color").and_then(Value::as_str).map(str::to_string)
    } else { style["box"].as_str().map(str::to_string) };
    let bg_alpha = if editor.get("captionBg").is_some() { number(&editor["captionBg"], "opacity", 0.0) } else { number(style, "boxAlpha", 0.0) };
    let base_outline = number(style, "outline", 0.0);
    let outline = if bg.is_some() { 0.0 } else if base_outline == 0.0 { 0.004 } else { base_outline };
    let outline_color = if bg.is_some() { text(style, "outlineColor", "#1F1D1A") } else { contrast_ink(&color) };
    Some(ResolvedCaption {
        font: text(font, "family", "Poppins").to_string(), font_file: font_file.to_string(),
        cell: number(font, "cell", 1.762), weight, size: number(style, "size", 0.058), color,
        highlight: text(editor, "captionHighlight", text(style, "highlight", "#FFFFFF")).to_string(),
        bg, bg_alpha, outline, outline_color: outline_color.to_string(),
        uppercase: style["uppercase"].as_bool().unwrap_or(false),
    })
}

pub fn caption_scale_of(editor: &Value, catalog: &Value) -> f64 {
    if let Some(scale) = editor.get("captionScale").and_then(Value::as_f64) {
        if scale.is_finite() {
            return scale;
        }
    }
    let size_key = text(editor, "captionSize", "sedang");
    number(&catalog["sizeScale"], size_key, 1.0)
}

pub fn caption_font_file(style: &ResolvedCaption, catalog: &Value) -> String {
    if style.font.eq_ignore_ascii_case("poppins") {
        catalog["poppinsFiles"]
            .get(style.weight.to_string())
            .and_then(Value::as_str)
            .or_else(|| catalog["poppinsFiles"]["700"].as_str())
            .unwrap_or("Poppins-Bold.ttf")
            .to_string()
    } else {
        style.font_file.clone()
    }
}

fn caption_ass_family(style: &ResolvedCaption) -> String {
    if style.font.eq_ignore_ascii_case("poppins") {
        match style.weight {
            500 => "Poppins Medium",
            600 => "Poppins SemiBold",
            700 => "Poppins",
            800 => "Poppins ExtraBold",
            _ => "Poppins",
        }.to_string()
    } else {
        style.font.clone()
    }
}

fn overlay_ass_family(bold: bool) -> &'static str {
    if bold { "Poppins" } else { "Poppins Medium" }
}

#[cfg(test)]
mod font_family_tests {
    use super::*;

    #[test]
    fn poppins_uses_installed_face_names_for_each_weight() {
        for (weight, family) in [
            (500, "Poppins Medium"),
            (600, "Poppins SemiBold"),
            (700, "Poppins"),
            (800, "Poppins ExtraBold"),
        ] {
            let style = ResolvedCaption {
                font: "Poppins".into(), font_file: String::new(), cell: 1.762,
                weight, size: 0.05, color: "#FFFFFF".into(), highlight: "#FFFFFF".into(),
                bg: None, bg_alpha: 0.0, outline: 0.0, outline_color: "#000000".into(), uppercase: false,
            };
            assert_eq!(caption_ass_family(&style), family);
        }
        assert_eq!(overlay_ass_family(false), "Poppins Medium");
        assert_eq!(overlay_ass_family(true), "Poppins");
    }
}

pub fn ass_time(sec: f64) -> String {
    let cs = (sec * 100.0).round().max(0.0) as i64;
    let h = cs / 360000;
    let m = (cs % 360000) / 6000;
    let s = (cs % 6000) / 100;
    let c = cs % 100;
    format!("{h}:{m:02}:{s:02}.{c:02}")
}

pub fn srt_time(sec: f64) -> String {
    let ms = (sec * 1000.0).round().max(0.0) as i64;
    let h = ms / 3600000;
    let m = (ms % 3600000) / 60000;
    let s = (ms % 60000) / 1000;
    let msec = ms % 1000;
    format!("{h:02}:{m:02}:{s:02},{msec:03}")
}

pub fn ass_color(hex: &str, opacity: f64) -> String {
    let h = hex.trim_start_matches('#');
    let a = ((1.0 - opacity).clamp(0.0, 1.0) * 255.0).round() as u8;
    if h.len() >= 6 {
        format!("&H{:02X}{}{}{}", a, &h[4..6], &h[2..4], &h[0..2]).to_uppercase()
    } else {
        format!("&H{:02X}FFFFFF", a)
    }
}

pub fn ass_inline_color(hex: &str) -> String {
    let h = hex.trim_start_matches('#');
    if h.len() >= 6 {
        format!("&H{}{}{}&", &h[4..6], &h[2..4], &h[0..2]).to_uppercase()
    } else {
        "&HFFFFFF&".to_string()
    }
}

/// Keep user braces out of override tags. An empty override after a literal backslash
/// prevents libass from interpreting a following N, n, or h as a control sequence.
pub fn escape_ass(t: &str) -> String {
    t.replace('{', "｛").replace('}', "｝").replace('\\', "\\{}")
        .replace("\r\n", "\n").replace('\r', "\n").replace('\n', "\\N")
}

pub fn rounded_rect_path(x0: f64, y0: f64, x1: f64, y1: f64, radius: f64) -> String {
    let r = radius.max(0.0).min((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
    let k = r * 0.4477;
    let n = |v: f64| -> String { format!("{}", v.round() as i64) };
    format!(
        "m {} {} l {} {} b {} {} {} {} {} {} l {} {} b {} {} {} {} {} {} l {} {} b {} {} {} {} {} {} l {} {} b {} {} {} {} {} {}",
        n(x0 + r), n(y0), n(x1 - r), n(y0), n(x1 - k), n(y0), n(x1), n(y0 + k), n(x1), n(y0 + r),
        n(x1), n(y1 - r), n(x1), n(y1 - k), n(x1 - k), n(y1), n(x1 - r), n(y1), n(x0 + r), n(y1),
        n(x0 + k), n(y1), n(x0), n(y1 - k), n(x0), n(y1 - r), n(x0), n(y0 + r),
        n(x0), n(y0 + k), n(x0 + k), n(y0), n(x0 + r), n(y0)
    )
}

#[derive(Debug, Clone)]
pub struct CaptionChunk {
    pub start: f64,
    pub end: f64,
    pub words: Vec<WordTiming>,
}

pub fn build_srt(chunks: &[CaptionChunk]) -> String {
    let mut out = String::new();
    for (i, c) in chunks.iter().enumerate() {
        let next = chunks.get(i + 1);
        let end = if let Some(n) = next { n.start.min(c.end + 0.6) } else { c.end + 0.6 };
        let text = c.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
        out.push_str(&format!("{}\n{} --> {}\n{}\n\n", i + 1, srt_time(c.start), srt_time(end), text));
    }
    out
}

pub fn chunk_words(words: &[WordTiming], max_words: usize, max_chars: usize) -> Vec<CaptionChunk> {
    let mut chunks = Vec::new();
    let mut cur: Vec<WordTiming> = Vec::new();
    let flush = |cur: &mut Vec<WordTiming>, chunks: &mut Vec<CaptionChunk>| {
        if cur.is_empty() { return; }
        chunks.push(CaptionChunk { start: cur[0].start, end: cur.last().unwrap().end, words: cur.clone() });
        cur.clear();
    };
    for (i, w) in words.iter().enumerate() {
        // JavaScript string.length counts UTF-16 code units, not UTF-8 bytes.
        let chars: usize = cur.iter().map(|x| x.text.encode_utf16().count() + 1).sum::<usize>() + w.text.encode_utf16().count();
        if !cur.is_empty() && (cur.len() >= max_words || chars > max_chars) { flush(&mut cur, &mut chunks); }
        cur.push(w.clone());
        let pause = words.get(i + 1).map_or(0.0, |next| next.start - w.end);
        if w.text.ends_with(['.', '!', '?', '…']) || pause > 0.45 { flush(&mut cur, &mut chunks); }
    }
    flush(&mut cur, &mut chunks);
    chunks
}

pub fn timeline_chunks(items: &[(f64, Vec<WordTiming>)]) -> Vec<CaptionChunk> {
    let mut out = Vec::new();
    for (start_offset, words) in items {
        for c in chunk_words(words, 5, 30) {
            out.push(CaptionChunk {
                start: c.start + *start_offset, end: c.end + *start_offset,
                words: c.words.into_iter().map(|w| WordTiming {
                    text: w.text, start: w.start + *start_offset, end: w.end + *start_offset,
                }).collect(),
            });
        }
    }
    out
}

/// Same balanced two-line choice as layoutCaptionLines in captions.ts.
pub fn layout_caption_lines<F>(words: &[String], mut measure: F, max_width: f64) -> Vec<Vec<String>>
where
    F: FnMut(&str) -> f64,
{
    if words.len() < 2 || measure(&words.join(" ")) <= max_width { return vec![words.to_vec()]; }
    let mut best: Option<(usize, f64)> = None;
    for at in 1..words.len() {
        let top = measure(&words[..at].join(" "));
        let bottom = measure(&words[at..].join(" "));
        if top > max_width || bottom > max_width { continue; }
        let score = top.max(bottom) + if bottom > top { 0.01 * max_width } else { 0.0 };
        if best.is_none_or(|(_, old)| score < old) { best = Some((at, score)); }
    }
    if let Some((at, _)) = best { return vec![words[..at].to_vec(), words[at..].to_vec()]; }
    let mut lines: Vec<Vec<String>> = vec![Vec::new()];
    for word in words {
        let last = lines.last_mut().unwrap();
        let mut test = last.clone();
        test.push(word.clone());
        if !last.is_empty() && measure(&test.join(" ")) > max_width { lines.push(vec![word.clone()]); }
        else { last.push(word.clone()); }
    }
    lines
}

fn event(layer: usize, start: f64, end: f64, style: &str, body: &str) -> String {
    format!("Dialogue: {layer},{},{},{style},,0,0,0,,{body}", ass_time(start), ass_time(end))
}

pub fn build_ass_content(
    chunks: &[CaptionChunk], editor: &Value, total: f64, w: u32, h: u32,
    fonts_dir: &Path, metrics: &FontMetricsCache,
) -> String {
    let cat = catalog();
    let style = resolve_caption(editor, &cat);
    let pos = text(editor, "captionPosition", "bawah");
    let scale = caption_scale_of(editor, &cat);
    let mut lines = vec![
        "[Script Info]".to_string(), "ScriptType: v4.00+".to_string(),
        format!("PlayResX: {w}"), format!("PlayResY: {h}"),
        "WrapStyle: 2".to_string(), "ScaledBorderAndShadow: yes".to_string(),
        "".to_string(), "[V4+ Styles]".to_string(),
        "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding".to_string(),
    ];
    if let Some(s) = &style {
        let px = s.size * h as f64 * scale;
        let font_size = (px * s.cell).round().max(1.0);
        let outline = if s.bg.is_none() { (s.outline * h as f64).max(1.0) } else { 0.0 };
        lines.push(format!(
            "Style: Cap,{},{font_size},{},{},{},&HFF000000,{},0,0,0,100,100,0,0,1,{outline:.2},0,5,0,0,0,1",
            caption_ass_family(s), ass_color(&s.color, 1.0), ass_color(&s.color, 1.0),
            ass_color(&s.outline_color, 1.0), if s.weight == 700 { -1 } else { 0 },
        ));
        if let Some(bg) = &s.bg {
            lines.push(format!(
                "Style: CapBox,{},{font_size},{},{},&HFF000000,&HFF000000,0,0,0,0,100,100,0,0,1,0,0,7,0,0,0,1",
                caption_ass_family(s), ass_color(bg, s.bg_alpha), ass_color(bg, s.bg_alpha),
            ));
        }
    }
    lines.push("Style: Ov,Poppins,48,&H00FFFFFF,&H00FFFFFF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,0,0,5,0,0,0,1".to_string());
    lines.extend(["", "[Events]", "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text"].map(str::to_string));

    if let Some(s) = &style {
        let px = s.size * h as f64 * scale;
        let cell = px * s.cell;
        let pad = if s.bg.is_some() { px * number(&cat, "boxPad", 0.32) } else { 0.0 };
        let font_file = fonts_dir.join(caption_font_file(s, &cat));
        for (i, c) in chunks.iter().enumerate() {
            if c.words.is_empty() { continue; }
            let hold_until = chunks.get(i + 1).map_or(c.end + 0.6, |n| n.start.min(c.end + 0.6)).min(total);
            if hold_until <= c.start { continue; }
            let words = c.words.iter().map(|wd| if s.uppercase { wd.text.to_uppercase() } else { wd.text.clone() }).collect::<Vec<_>>();
            let wrapped = layout_caption_lines(&words, |value| metrics.measure_text_advance(&font_file, value) * px, w as f64 * number(&cat, "maxWidth", 0.84));
            let text_h = cell * wrapped.len() as f64;
            let top = match pos { "atas" => h as f64 * 0.07, "tengah" => (h as f64 - text_h) / 2.0, _ => h as f64 * 0.93 - text_h };
            let box_w = wrapped.iter().map(|line| metrics.measure_text_advance(&font_file, &line.join(" ")) * px).fold(0.0, f64::max);
            if s.bg.is_some() {
                let path = rounded_rect_path(w as f64 / 2.0 - box_w / 2.0 - pad, top - pad,
                    w as f64 / 2.0 + box_w / 2.0 + pad, top + text_h + pad,
                    px * number(&cat, "boxRadius", 0.4));
                lines.push(event(0, c.start, hold_until, "CapBox", &format!("{{\\an7\\pos(0,0)\\p1}}{path}")));
            }
            let body_for = |active: Option<usize>| {
                let mut index = 0;
                wrapped.iter().map(|row| row.iter().map(|word| {
                    let current = index;
                    index += 1;
                    if active == Some(current) {
                        format!("{{\\1c{}}}{}{{\\1c{}}}", ass_inline_color(&s.highlight), escape_ass(word), ass_inline_color(&s.color))
                    } else { escape_ass(word) }
                }).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\\N")
            };
            // Position at the top of the first line, not the center of the entire frame.
            let placed = |body: String| format!("{{\\an8\\pos({},{})}}{body}", w / 2, top.round() as i64);
            if s.highlight.eq_ignore_ascii_case(&s.color) {
                lines.push(event(1, c.start, hold_until, "Cap", &placed(body_for(None))));
            } else {
                // captionAt falls back to the LAST word during gaps and the final hold.
                let mut boundaries = vec![c.start, hold_until];
                for word in &c.words {
                    for t in [word.start, word.end] {
                        if t > c.start && t < hold_until { boundaries.push(t); }
                    }
                }
                boundaries.sort_by(f64::total_cmp);
                boundaries.dedup();
                let mut spans: Vec<(f64, f64, usize)> = Vec::new();
                for pair in boundaries.windows(2) {
                    let (start, end) = (pair[0], pair[1]);
                    let t = start + (end - start) / 2.0;
                    let active = c.words.iter().position(|word| t >= word.start && t < word.end)
                        .unwrap_or(if t < c.words[0].start { 0 } else { c.words.len() - 1 });
                    if let Some(last) = spans.last_mut().filter(|last| last.2 == active) {
                        last.1 = end;
                    } else { spans.push((start, end, active)); }
                }
                for (start, end, active) in spans {
                    if ass_time(start) != ass_time(end) {
                        lines.push(event(1, start, end, "Cap", &placed(body_for(Some(active)))));
                    }
                }
            }
        }
    }
    if let Some(overlays) = editor.get("overlays").and_then(Value::as_array) {
        for (overlay_index, overlay) in overlays.iter().enumerate() {
            if overlay["kind"] != "text" { continue; }
            let start = number(overlay, "start", 0.0).max(0.0);
            let end = overlay.get("end").and_then(Value::as_f64).unwrap_or(total).min(total);
            if start >= end { continue; }
            let fs = (number(overlay, "size", 0.065) * h as f64).max(1.0);
            let anchor = number(overlay, "anchor", 5.0) as i64;
            let anchor = anchor.clamp(1, 9);
            let color = text(overlay, "color", "#FFFFFF");
            let contrast = contrast_ink(color);
            let kind = text(overlay, "style", "box");
            let pad = (fs * 0.28).round();
            let outline = (fs * 0.08).round().max(1.0);
            let shadow = (fs * 0.05).round().max(1.0);
            let (outline_width, shadow_width) = match kind {
                "outline" => (outline, 0.0),
                "plain" => (0.0, shadow),
                _ => (0.0, 0.0),
            };
            let x = (number(overlay, "x", 0.5) * w as f64).round();
            let y = (number(overlay, "y", 0.5) * h as f64).round();
            let file = if overlay["bold"] == true { "Poppins-Bold.ttf" } else { "Poppins-Medium.ttf" };
            let font_file = fonts_dir.join(file);
            // CSS font size is an em; ASS size uses the Poppins Windows ascent/descent cell.
            let font_size = (fs * number(&cat, "poppinsCell", 1.762)).round().max(1.0);
            let bold = if overlay["bold"] == true { 700 } else { 0 };
            let font_family = overlay_ass_family(overlay["bold"] == true);
            let raw = text(overlay, "text", "").replace("\r\n", "\n").replace('\r', "\n");
            if raw.is_empty() { continue; }
            let rows = raw.split('\n').collect::<Vec<_>>();
            let widths = rows.iter().map(|row| metrics.measure_text_advance(&font_file, row) * fs).collect::<Vec<_>>();
            let max_width = widths.iter().copied().fold(0.0, f64::max);
            let total_height = font_size * rows.len() as f64;
            // ASS anchor 7 is top-left, while preview's anchor 1 is bottom-left.
            let ax = ((anchor - 1) % 3) as f64 / 2.0;
            let ay = 1.0 - ((anchor - 1) / 3) as f64 / 2.0;
            let pad = if kind == "box" { pad } else { 0.0 };
            let left = x - ax * max_width - pad;
            let top = y - ay * total_height - pad;
            let alpha = ((1.0 - number(overlay, "opacity", 1.0).clamp(0.0, 1.0)) * 255.0).round() as u8;
            let layer = 2 + overlay_index * 2;
            // Background is drawn as a box layer before the corresponding foreground text layer.
            if kind == "box" {
                let path = rounded_rect_path(left, top, left + max_width + pad * 2.0, top + total_height + pad * 2.0, 0.0);
                let box_body = format!("{{\\an7\\pos(0,0)\\1c{}\\1a&H26&\\bord0\\shad0\\p1}}{path}", ass_inline_color(contrast));
                lines.push(event(layer, start, end, "Ov", &box_body));
            }
            for (row, content) in rows.iter().enumerate() {
                let row_anchor = 7 + (anchor - 1) % 3;
                let row_y = top + pad + row as f64 * font_size;
                let tags = format!("\\rOv\\an{row_anchor}\\pos({x:.2},{row_y:.2})\\fn{font_family}\\fs{font_size:.2}\\b{bold}\\1c{}\\3c{}\\4c&H000000&\\1a&H{alpha:02X}&\\3a&H{alpha:02X}&\\4a&H70&\\bord{outline_width:.2}\\shad{shadow_width:.2}\\q2",
                    ass_inline_color(color), ass_inline_color(contrast));
                lines.push(event(layer + 1, start, end, "Ov", &format!("{{{tags}}}{}", escape_ass(content))));
            }
        }
    }
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    fn fonts_dir() -> PathBuf {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/fonts");
        for file in ["Poppins-Bold.ttf", "Poppins-Medium.ttf", "Bangers-Regular.ttf"] {
            assert!(p.join(file).is_file(), "bundled font missing: {file}");
        }
        p
    }

    #[test]
    fn resolve_caption_defaults_and_overrides() {
        let cat = catalog();

        // 1. Default karaoke
        let ed_karaoke = json!({ "captionStyle": "karaoke" });
        let res_karaoke = resolve_caption(&ed_karaoke, &cat).expect("karaoke resolves");
        assert_eq!(res_karaoke.font, "Poppins");
        assert_eq!(res_karaoke.font_file, "Poppins-ExtraBold.ttf"); // weight 800
        assert_eq!(res_karaoke.color, "#FAF7F2");
        assert_eq!(res_karaoke.highlight, "#FFC93C");
        assert_eq!(res_karaoke.bg.as_deref(), Some("#1F1D1A"));
        assert!((res_karaoke.bg_alpha - 0.92).abs() < 1e-4);
        assert!(!res_karaoke.uppercase);

        // 2. Tebal uppercase & outline
        let ed_tebal = json!({ "captionStyle": "tebal" });
        let res_tebal = resolve_caption(&ed_tebal, &cat).expect("tebal resolves");
        assert!(res_tebal.uppercase);
        assert!(res_tebal.bg.is_none());
        assert!(res_tebal.outline > 0.0);

        // 3. Custom font, colors, and box overrides
        let ed_custom = json!({
            "captionStyle": "karaoke",
            "captionFont": "bangers",
            "captionColor": "#00FF00",
            "captionHighlight": "#FF00FF",
            "captionBg": { "color": "#112233", "opacity": 0.5 }
        });
        let res_custom = resolve_caption(&ed_custom, &cat).expect("custom resolves");
        assert_eq!(res_custom.font, "Bangers");
        assert_eq!(res_custom.font_file, "Bangers-Regular.ttf");
        assert_eq!(res_custom.color, "#00FF00");
        assert_eq!(res_custom.highlight, "#FF00FF");
        assert_eq!(res_custom.bg.as_deref(), Some("#112233"));
        assert!((res_custom.bg_alpha - 0.5).abs() < 1e-4);

        // 4. Style none returns None
        let ed_none = json!({ "captionStyle": "none" });
        assert!(resolve_caption(&ed_none, &cat).is_none());
    }

    #[test]
    fn text_overlay_emits_ass_events_and_escapes() {
        let f_dir = fonts_dir();
        let metrics = FontMetricsCache::new();
        let editor = json!({
            "captionStyle": "none",
            "overlays": [
                {
                    "id": "ov1",
                    "kind": "text",
                    "text": "Halo {dunia}! \\ teks",
                    "anchor": 8,
                    "x": 0.5,
                    "y": 0.1,
                    "size": 0.065,
                    "color": "#FFFFFF",
                    "style": "box",
                    "bold": true,
                    "start": 1.0,
                    "end": 4.5
                }
            ]
        });

        let ass = build_ass_content(&[], &editor, 10.0, 1920, 1080, &f_dir, &metrics);
        assert!(ass.contains("Dialogue: 2,0:00:01.00,0:00:04.50,Ov,,0,0,0,,{\\an7\\pos(0,0)"));
        assert!(ass.contains("Dialogue: 3,0:00:01.00,0:00:04.50,Ov,,0,0,0,,"));
        // Braces escaped to Unicode full-width braces so libass doesn't parse as tags
        assert!(ass.contains("Halo ｛dunia｝! \\{} teks"));
    }

    #[test]
    fn catalog_fonts_scale_and_layout() {
        let cat = catalog();
        let dir = fonts_dir();
        let metrics = FontMetricsCache::new();
        for preset in cat["styles"].as_array().unwrap() {
            for font in cat["fonts"].as_array().unwrap() {
                let s = resolve_caption(&json!({"captionStyle": preset["id"], "captionFont": font["id"], "captionBg": null}), &cat).unwrap();
                let file = dir.join(caption_font_file(&s, &cat));
                assert!(file.is_file(), "{}", file.display());
                assert!(s.bg.is_none());
                assert!(s.outline > 0.0);
                assert!(metrics.measure_text_advance(&file, "Caption") > 0.0);
            }
        }
        assert_eq!(caption_scale_of(&json!({"captionSize":"kecil"}), &cat), 0.82);
        assert_eq!(caption_scale_of(&json!({"captionSize":"besar", "captionScale":1.4}), &cat), 1.4);
        let words = vec!["Caption".to_string(); 6];
        let font = dir.join("Poppins-Bold.ttf");
        let measure = |s: &str| metrics.measure_text_advance(&font, s) * 30.0;
        let width = measure("Caption Caption Caption") + 1.0;
        let rows = layout_caption_lines(&words, measure, width);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows.concat(), words);
        assert!(layout_caption_lines(&words, measure, measure("Caption") + 1.0).len() > 2);
    }

    #[test]
    fn overlay_anchors_styles_and_clipping() {
        let metrics = FontMetricsCache::new();
        for anchor in 1..=9 {
            for style in ["plain", "outline", "box"] {
                let editor = json!({"captionStyle":"none", "overlays":[
                    {"kind":"text", "text":"First\nSecond", "anchor":anchor, "style":style,
                     "start":-1, "end":null, "bold":false},
                    {"kind":"text", "text":"INVALID", "start":10, "end":2}
                ]});
                let ass = build_ass_content(&[], &editor, 3.0, 640, 360, &fonts_dir(), &metrics);
                assert!(!ass.contains("Style: Cap,"));
                assert!(!ass.contains("INVALID"));
                assert!(ass.contains("0:00:00.00,0:00:03.00,Ov,"));
                assert!(ass.contains("\\fnPoppins Medium\\fs"));
                assert!(ass.contains("\\b0"));
                assert!(ass.contains("\\4a&H70&"));
                assert!(ass.contains(&format!("\\an{}\\pos", 7 + (anchor - 1) % 3)));
                assert_eq!(ass.matches("Dialogue:").count(), if style == "box" { 3 } else { 2 });
            }
        }
    }

    #[test]
    fn chunking_matches_sentence_pause_and_utf16_rules() {
        let word = |text: &str, start, end| WordTiming { text: text.into(), start, end };
        let words = vec![word("é", 0.0, 0.1), word("é", 0.1, 0.2)];
        assert_eq!(chunk_words(&words, 5, 3).len(), 1);
        let words = vec![word("Hi!", 0.0, 0.1), word("next", 0.1, 0.2), word("pause", 1.0, 1.1)];
        assert_eq!(chunk_words(&words, 5, 30).len(), 3);
    }

    // Invoked by scripts/test-render-fixture.py in a fresh temporary directory.
    #[test]
    #[ignore = "writes ASS fixtures for the optional FFmpeg integration test"]
    fn write_render_fixtures() {
        let dir = std::path::PathBuf::from(std::env::var_os("BANG_STORY_FIXTURE_DIR").expect("fixture directory"));
        let chunks = vec![CaptionChunk {
            start: 0.5, end: 2.0,
            words: vec![
                WordTiming { text: "Kucing".into(), start: 0.5, end: 1.2 },
                WordTiming { text: "lucu".into(), start: 1.2, end: 2.0 },
            ],
        }];
        let metrics = FontMetricsCache::new();
        for preset in ["karaoke", "minimal", "subtitle", "tebal", "none", "custom"] {
            let mut editor = json!({
                "captionStyle": preset,
                "overlays": [{"kind":"text", "text":"Halo dunia!\nBaris kedua", "bold":false,
                    "style":"box", "anchor":7, "x":0.1, "y":0.1, "size":0.04, "start":0.2, "end":2.8},
                    {"kind":"text", "text":"Bold", "bold":true, "style":"outline",
                    "anchor":9, "x":0.9, "y":0.3, "size":0.04, "start":0.2, "end":2.8}]
            });
            if preset == "custom" {
                editor["captionStyle"] = json!("karaoke");
                editor["captionFont"] = json!("bangers");
                editor["captionColor"] = json!("#00FF00");
                editor["captionHighlight"] = json!("#FF00FF");
                editor["captionBg"] = Value::Null;
                editor["captionScale"] = json!(1.3);
                editor["captionPosition"] = json!("tengah");
            }
            let ass = build_ass_content(&chunks, &editor, 3.0, 1920, 1080, &fonts_dir(), &metrics);
            std::fs::write(dir.join(format!("{preset}.ass")), ass).unwrap();
        }
        let editor = json!({"captionStyle":"none", "overlays":[{
            "kind":"text", "text":r"C:\New\notes\home", "style":"plain", "bold":false,
            "anchor":7, "x":0.1, "y":0.1, "size":0.05
        }]});
        std::fs::write(dir.join("literal.ass"), build_ass_content(&[], &editor, 3.0, 1920, 1080, &fonts_dir(), &metrics)).unwrap();
    }

    #[test]
    fn caption_highlight_and_box_events() {
        let f_dir = fonts_dir();
        let metrics = FontMetricsCache::new();
        let chunks = vec![CaptionChunk {
            start: 0.5,
            end: 2.5,
            words: vec![
                WordTiming { text: "Kucing".to_string(), start: 0.5, end: 1.2 },
                WordTiming { text: "lucu".to_string(), start: 1.2, end: 2.5 },
            ],
        }];
        let editor = json!({
            "captionStyle": "karaoke",
            "captionColor": "#FFFFFF",
            "captionHighlight": "#FFC93C"
        });

        let ass = build_ass_content(&chunks, &editor, 5.0, 1920, 1080, &f_dir, &metrics);
        assert!(ass.contains("Style: Cap,Poppins ExtraBold,"));
        assert!(ass.contains("Style: CapBox,Poppins ExtraBold,"));
        // Box event on layer 0
        assert!(ass.contains("Dialogue: 0,0:00:00.50,0:00:03.10,CapBox,"));
        // Karaoke word highlight on layer 1
        assert!(ass.contains("Dialogue: 1,0:00:00.50,0:00:01.20,Cap,"));
        assert!(ass.contains("Dialogue: 1,0:00:01.20,0:00:03.10,Cap,"));
        assert!(ass.contains("\\1c&H3CC9FF&")); // #FFC93C in BGR
    }
}


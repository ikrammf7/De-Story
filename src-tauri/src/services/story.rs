use std::collections::HashMap;
use serde_json::{json, Value};

use crate::db::repo::{
    apply_visuals, characters_of, clips_of, get_bundle, get_project, get_secret, get_settings,
    replace_script, NewScriptScene, ProjectBundle, SceneVisualUpdate,
};
use crate::db::DbPool;
use crate::services::{gemini, llm};

const CAMERA_IDS: &[&str] = &[
    "zoomin",
    "zoomout",
    "panleft",
    "panright",
    "kenburns",
    "shake",
    "static",
];

const STRENGTH_IDS: &[&str] = &["halus", "sedang", "kuat"];

const TRANSITION_IDS: &[&str] = &["fade", "slideleft", "zoomin", "cut"];

fn language_of(code: &str) -> &'static str {
    match code {
        "en" => "English",
        "jv" => "Javanese",
        "su" => "Sundanese",
        _ => "Indonesian",
    }
}

fn clip_count_for(duration_sec: i64) -> usize {
    match duration_sec {
        15 => 3,
        30 => 5,
        60 => 8,
        90 => 12,
        120 => 15,
        180 => 22,
        _ => ((duration_sec as f64 / 8.0).round() as usize).clamp(3, 40),
    }
}

fn estimate_narration_ms(narration: &str) -> i64 {
    let words = narration.split_whitespace().count();
    // ~150 words per minute => ~400ms per word + 800ms base pause
    ((words as i64 * 400 + 800) as i64).max(1500)
}

async fn execute_llm_json(
    db: &DbPool,
    system: &str,
    prompt: &str,
    schema: &Value,
) -> Result<Value, String> {
    let (provider, model, custom_base) = {
        let conn = db.lock().unwrap();
        let s = get_settings(&conn);
        let p = s
            .get("llmProvider")
            .and_then(|x| x.as_str())
            .unwrap_or("gemini")
            .to_string();
        let m = s
            .get("llmModels")
            .and_then(|x| x.get(&p))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let base = s
            .get("customBaseUrl")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        (p, m, base)
    };

    if model.is_empty() {
        return Err(format!("Pilih model untuk provider '{provider}' di Pengaturan."));
    }

    let key = {
        let conn = db.lock().unwrap();
        get_secret(&conn, &provider).unwrap_or_default()
    };

    if provider == "gemini" {
        if key.is_empty() {
            return Err("Kunci API Gemini belum diatur. Buka Pengaturan untuk menambahkannya.".into());
        }
        gemini::generate_json(&key, &model, system, prompt, schema).await
    } else {
        llm::generate_json(&provider, &key, &custom_base, &model, system, prompt, schema).await
    }
}

pub async fn generate_script(db: &DbPool, project_id: &str) -> Result<ProjectBundle, String> {
    let project = {
        let conn = db.lock().unwrap();
        get_project(&conn, project_id).ok_or("Proyek tidak ditemukan")?
    };

    if project.synopsis.trim().is_empty() {
        return Err("Tulis ide cerita atau sinopsis dulu.".into());
    }

    let lang = language_of(&project.language);
    let count = clip_count_for(project.duration_sec);

    let system = format!(
        "You are a scriptwriter for faceless YouTube storytelling and explainer videos. \
You turn an idea into a sequence of short scenes. Each scene has one line of voice-over. \
The first scene is a strong hook. The story has a clear beginning, rising tension and a satisfying ending. \
The last scene can invite viewers to subscribe. Keep facts accurate for history and science topics, \
and always call each character by the same name. \
The narration is voiced by an expressive text-to-speech model. Where it helps the delivery, add a few English \
vocal tags in angle brackets inside the narration, such as <short pause>, <long pause>, <chuckles>, <sigh>, \
<gasp> or <whispers>. Use them sparingly, at most one or two per scene, and never other markup. \
Write everything in {lang}, except the vocal tags, which stay in English."
    );

    let prompt = format!(
        "Idea / synopsis:\n{}\n\nTarget video length: about {} seconds.\n\nWrite exactly {} scenes.",
        project.synopsis, project.duration_sec, count
    );

    let schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "title": { "type": "string", "description": "Short catchy video title in the target language, max 60 characters." },
            "scenes": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "title": { "type": "string", "description": "Short scene title in the target language." },
                        "story": { "type": "string", "description": "What happens in this scene, one or two sentences in the target language." },
                        "narration": { "type": "string", "description": "Voice-over for this scene in the target language, 10 to 18 spoken words." }
                    },
                    "required": ["title", "story", "narration"]
                }
            }
        },
        "required": ["title", "scenes"]
    });

    let plan = execute_llm_json(db, &system, &prompt, &schema).await?;

    let scenes_arr = plan
        .get("scenes")
        .and_then(|x| x.as_array())
        .ok_or("Model tidak menghasilkan adegan. Coba lagi atau pilih model lain di Pengaturan.")?;

    let mut new_scenes = Vec::new();
    for (i, sc) in scenes_arr.iter().enumerate() {
        let title = sc.get("title").and_then(|x| x.as_str()).unwrap_or("").trim();
        let story = sc.get("story").and_then(|x| x.as_str()).unwrap_or("").trim();
        let narration = sc.get("narration").and_then(|x| x.as_str()).unwrap_or("").trim();

        if story.is_empty() && narration.is_empty() {
            continue;
        }

        new_scenes.push(NewScriptScene {
            title: if title.is_empty() { format!("Adegan {}", i + 1) } else { title.to_string() },
            story: story.to_string(),
            narration: narration.to_string(),
            duration_ms: estimate_narration_ms(narration),
        });
    }

    if new_scenes.is_empty() {
        return Err("Model tidak menghasilkan adegan yang valid. Coba lagi.".into());
    }

    let plan_title = plan.get("title").and_then(|x| x.as_str()).unwrap_or("").trim();
    let final_title = if plan_title.is_empty() { &project.title } else { plan_title };

    {
        let conn = db.lock().unwrap();
        replace_script(&conn, project_id, final_title, new_scenes)?;
    }

    let conn = db.lock().unwrap();
    get_bundle(&conn, project_id).ok_or("Gagal mengambil bundle proyek".into())
}

fn visual_text(a: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(s) = a.get("setting").and_then(|x| x.as_str()) {
        if !s.trim().is_empty() {
            parts.push(format!("Setting: {}", s.trim()));
        }
    }
    if let Some(act) = a.get("action").and_then(|x| x.as_str()) {
        if !act.trim().is_empty() {
            parts.push(format!("Action: {}", act.trim()));
        }
    }
    if let Some(w_arr) = a.get("wardrobe").and_then(|x| x.as_array()) {
        let mut w_lines = Vec::new();
        for item in w_arr {
            let name = item.get("name").and_then(|x| x.as_str()).unwrap_or("").trim();
            let src = item.get("source").and_then(|x| x.as_str()).unwrap_or("");
            let outfit = item.get("outfit").and_then(|x| x.as_str()).unwrap_or("").trim();
            if !name.is_empty() {
                if src == "reference" {
                    let desc = if !outfit.is_empty() { format!(" ({outfit})") } else { String::new() };
                    w_lines.push(format!("{name} - default outfit as on the reference sheet{desc}"));
                } else {
                    w_lines.push(format!("{name} - custom for this scene: {outfit}"));
                }
            }
        }
        if !w_lines.is_empty() {
            parts.push(format!("Wardrobe: {}", w_lines.join("; ")));
        }
    }
    if let Some(f) = a.get("framing").and_then(|x| x.as_str()) {
        if !f.trim().is_empty() {
            parts.push(format!("Framing: {}", f.trim()));
        }
    }
    if let Some(av) = a.get("avoid").and_then(|x| x.as_str()) {
        if !av.trim().is_empty() {
            parts.push(format!("Avoid: {}", av.trim()));
        }
    }
    parts.join("\n")
}

pub async fn generate_visuals(db: &DbPool, project_id: &str, mode: &str) -> Result<ProjectBundle, String> {
    let (project, clips, known) = {
        let conn = db.lock().unwrap();
        let p = get_project(&conn, project_id).ok_or("Proyek tidak ditemukan")?;
        let c = clips_of(&conn, project_id);
        let k = characters_of(&conn, project_id);
        (p, c, k)
    };

    if clips.is_empty() {
        return Err("Naskah masih kosong. Tambahkan adegan dulu.".into());
    }

    let mut targets = Vec::new();
    for (i, c) in clips.iter().enumerate() {
        let needs = c.visual_prompt.trim().is_empty() || mode == "all";
        if needs {
            targets.push((i + 1, c.clone()));
        }
    }

    if targets.is_empty() {
        let conn = db.lock().unwrap();
        return get_bundle(&conn, project_id).ok_or("Proyek tidak ditemukan".into());
    }

    let system = "You are the storyboard artist for a faceless YouTube video. The script is final; you design the pictures.\n\
For each requested scene you describe ONE still image, choose the camera move and its strength over that image, the transition into the next scene, and write how the image would move if it is animated as an AI video.\n\
Rules for the image descriptions, which go to a text-to-image model that never sees the script:\n\
1. Every scene is self-contained. Always name the world (planet or country, era, technology) in the setting, even when the story makes it obvious, and add background details that prove it.\n\
2. Turn narration into a literal, showable moment.\n\
3. Describe each character's identity clearly, rendered in the project's art style rather than imposing realistic anatomy.\n\
4. Clothing must suit the place and action.\n\
5. Keep every recurring character visually consistent.\n\
6. No readable text, signs, captions or labels in the image.\n\
Write \"world\", \"identity\", \"outfit\", \"setting\", \"action\", \"wardrobe\", \"framing\", \"avoid\" and \"motion\" in English.";

    let script = clips
        .iter()
        .enumerate()
        .map(|(i, c)| {
            format!(
                "Scene {}: {}\nStory: {}\nNarration ({}): {}",
                i + 1,
                c.title,
                if c.story.is_empty() { "(none)" } else { &c.story },
                language_of(&project.language),
                if c.narration.is_empty() { "(none)" } else { &c.narration }
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let existing_chars = if !known.is_empty() {
        format!(
            "Existing characters (keep them exactly):\n{}",
            known.iter().map(|c| format!("- {}: {}", c.name, c.description)).collect::<Vec<_>>().join("\n")
        )
    } else {
        "There are no existing characters yet.".to_string()
    };

    let target_nums = targets.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>().join(", ");

    let prompt = format!(
        "Frame format: {}.\n\
Art style of every image (all descriptions must fit this style): {}\n\
Idea of the video: {}\n\
{}\n\
Full script:\n{}\n\n\
Write visuals for these scenes only: {}.\n\
Allowed camera values: {}.\n\
Allowed strength values: {}.\n\
Allowed transition values: {}.",
        if project.aspect_ratio == "9:16" { "vertical 9:16" } else { "horizontal 16:9" },
        crate::services::styles::style_prompt(&project.style_id),
        project.synopsis,
        existing_chars,
        script,
        target_nums,
        CAMERA_IDS.join(", "),
        STRENGTH_IDS.join(", "),
        TRANSITION_IDS.join(", ")
    );

    let schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "world": { "type": "string" },
            "characters": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "name": { "type": "string" },
                        "identity": { "type": "string" },
                        "outfit": { "type": "string" }
                    },
                    "required": ["name", "identity", "outfit"]
                }
            },
            "scenes": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "scene": { "type": "integer" },
                        "setting": { "type": "string" },
                        "action": { "type": "string" },
                        "wardrobe": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "additionalProperties": false,
                                "properties": {
                                    "name": { "type": "string" },
                                    "source": { "type": "string", "enum": ["reference", "custom"] },
                                    "outfit": { "type": "string" }
                                },
                                "required": ["name", "source", "outfit"]
                            }
                        },
                        "framing": { "type": "string" },
                        "avoid": { "type": "string" },
                        "characters": { "type": "array", "items": { "type": "string" } },
                        "camera": { "type": "string", "enum": CAMERA_IDS },
                        "strength": { "type": "string", "enum": STRENGTH_IDS },
                        "motion": { "type": "string" },
                        "transition": { "type": "string", "enum": TRANSITION_IDS }
                    },
                    "required": ["scene", "setting", "action", "wardrobe", "framing", "avoid", "characters", "camera", "strength", "motion", "transition"]
                }
            }
        },
        "required": ["world", "characters", "scenes"]
    });

    let plan = execute_llm_json(db, system, &prompt, &schema).await?;

    let scenes_arr = plan
        .get("scenes")
        .and_then(|x| x.as_array())
        .ok_or("Model tidak menghasilkan visual. Coba lagi atau pilih model lain di Pengaturan.")?;

    let mut by_number: HashMap<i64, &Value> = HashMap::new();
    let mut loose = Vec::new();

    for sc in scenes_arr {
        let n = sc.get("scene").and_then(|x| x.as_i64()).unwrap_or(-1);
        if n > 0 && !by_number.contains_key(&n) {
            by_number.insert(n, sc);
        } else {
            loose.push(sc);
        }
    }

    let mut updates = Vec::new();
    for (n, clip) in targets {
        let answer = if let Some(a) = by_number.get(&(n as i64)) {
            *a
        } else if !loose.is_empty() {
            loose.remove(0)
        } else {
            continue;
        };

        let v_text = visual_text(answer);
        let char_names = answer
            .get("characters")
            .and_then(|x| x.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        let camera = answer.get("camera").and_then(|x| x.as_str()).unwrap_or(&clip.camera_preset);
        let strength = answer.get("strength").and_then(|x| x.as_str()).unwrap_or(&clip.motion_strength);
        let transition = answer.get("transition").and_then(|x| x.as_str()).unwrap_or(&clip.transition);
        let video_prompt = answer.get("motion").and_then(|x| x.as_str()).unwrap_or("");

        updates.push(SceneVisualUpdate {
            clip_id: clip.id,
            visual_prompt: v_text,
            character_names: char_names,
            camera_preset: camera.to_string(),
            motion_strength: strength.to_string(),
            video_prompt: video_prompt.to_string(),
            transition: transition.to_string(),
        });
    }

    let mut new_chars = Vec::new();
    if let Some(chars_arr) = plan.get("characters").and_then(|x| x.as_array()) {
        for c in chars_arr {
            let name = c.get("name").and_then(|x| x.as_str()).unwrap_or("").trim();
            let identity = c.get("identity").and_then(|x| x.as_str()).unwrap_or("").trim();
            let outfit = c.get("outfit").and_then(|x| x.as_str()).unwrap_or("").trim();
            if !name.is_empty() {
                let hint = crate::services::styles::style_character_hint(&project.style_id);
                let desc = format!("{identity}, drawn as {hint}. Default outfit: {outfit}.");
                new_chars.push((name.to_string(), desc));
            }
        }
    }

    {
        let conn = db.lock().unwrap();
        apply_visuals(&conn, project_id, new_chars, updates)?;
    }

    let conn = db.lock().unwrap();
    get_bundle(&conn, project_id).ok_or("Gagal mengambil bundle proyek".into())
}

pub async fn generate_music_prompt(db: &DbPool, project_id: &str) -> Result<Value, String> {
    let (project, clips) = {
        let conn = db.lock().unwrap();
        let p = get_project(&conn, project_id).ok_or("Proyek tidak ditemukan")?;
        let c = clips_of(&conn, project_id);
        (p, c)
    };

    let total_ms: i64 = clips.iter().map(|c| c.duration_ms).sum();
    let duration_sec = if total_ms > 0 { (total_ms / 1000).max(10) } else { project.duration_sec };

    let system = "You are a music supervisor for faceless YouTube storytelling videos.\n\
You pick background music that supports a voice-over without competing with it: instrumental, no vocals or choirs with words, clear space in the mid range where the voice sits.\n\
Match the setting and era of the story and follow its emotional arc from the hook to the ending.\n\
Write the music fields in English. Never name real artists or copyrighted songs.";

    let scenes = clips
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let txt = if !c.story.is_empty() { &c.story } else { &c.narration };
            format!("{}. {}: {}", i + 1, c.title, txt)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "Video title: {}\n\
Idea: {}\n\
Video length: about {} seconds.\n\
Scenes:\n{}",
        project.title, project.synopsis, duration_sec, scenes
    );

    let schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "title": { "type": "string", "description": "Short English track title, max 40 characters." },
            "style": { "type": "string", "description": "Suno Style of Music field: 6 to 12 comma-separated English tags." },
            "exclude": { "type": "string", "description": "Suno Exclude styles field." },
            "description": { "type": "string", "description": "One English paragraph for text-to-music AI." },
            "reason": { "type": "string", "description": "One sentence in Indonesian telling why this music suits the video." }
        },
        "required": ["title", "style", "exclude", "description", "reason"]
    });

    let res = execute_llm_json(db, system, &prompt, &schema).await?;
    let mut out = res;
    if let Some(obj) = out.as_object_mut() {
        obj.insert("durationSec".to_string(), json!(duration_sec));
    }
    Ok(out)
}

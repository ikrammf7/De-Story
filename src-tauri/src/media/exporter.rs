use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::db::repo::{get_bundle, insert_job, touch_project, AssetRow, ClipRow, ProjectBundle};
use crate::db::DbPool;
use crate::jobs::{JobManager, QueueName, TaskCtx};
use crate::media::ass::{build_ass_content, build_srt, timeline_chunks};
use crate::media::font_metrics::FontMetricsCache;
use crate::paths::{asset_abs_path, ffmpeg_path, fonts_dir, user_data_dir};
use crate::speech::align::WordTiming;

pub const TRANSITION_SEC: f64 = 0.4;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub fps: u32,
    pub resolution: u32, // 720, 1080, 2160
    pub burn_captions: bool,
    pub write_srt: bool,
    pub duck_music: bool,
    pub normalize_audio: bool,
    pub folder: String,
    pub file_name: String,
}

#[derive(Debug, Clone)]
pub struct OutputSize {
    pub w: u32,
    pub h: u32,
}

pub fn output_size(aspect: &str, res: u32) -> OutputSize {
    let long = match res {
        720 => 1280,
        1080 => 1920,
        2160 => 3840,
        _ => 1920,
    };
    let short = res;
    if aspect == "16:9" {
        OutputSize { w: long, h: short }
    } else {
        OutputSize { w: short, h: long }
    }
}

pub fn strength_amount(strength: &str) -> f64 {
    match strength {
        "halus" => 0.08,
        "sedang" => 0.15,
        "kuat" => 0.25,
        _ => 0.08,
    }
}

pub struct CameraZoompan {
    pub z: String,
    pub x: String,
    pub y: String,
}

pub fn camera_zoompan(preset: &str, strength: &str, frames: u32, fps: u32) -> CameraZoompan {
    let s = strength_amount(strength);
    let n = (frames.max(2) - 1).max(1);
    let e = format!("(min(on/{n},1)*min(on/{n},1)*(3-2*min(on/{n},1)))");
    let t = format!("(on/{fps})");
    let lin = |a: f64, b: f64| -> String { format!("({a}+({b_diff})*{e})", b_diff = b - a) };

    let mut z = "1".to_string();
    let mut x = "0".to_string();
    let mut y = "0".to_string();

    match preset {
        "zoomin" => {
            z = lin(1.0, 1.0 + s);
        }
        "zoomout" => {
            z = lin(1.0 + s, 1.0);
        }
        "panleft" => {
            z = format!("{}", 1.0 + s);
            x = lin(0.9, -0.9);
        }
        "panright" => {
            z = format!("{}", 1.0 + s);
            x = lin(-0.9, 0.9);
        }
        "kenburns" => {
            z = lin(1.0 + s * 0.3, 1.0 + s);
            x = lin(-0.5, 0.5);
            y = lin(0.3, -0.3);
        }
        "shake" => {
            z = format!("{}", 1.0 + (s * 0.5).max(0.04));
            x = format!("(0.35*sin(2*PI*1.3*{t}))");
            y = format!("(0.3*sin(2*PI*1.7*{t}+1))");
        }
        _ => {}
    }

    CameraZoompan {
        z,
        x: format!("(iw-iw/zoom)*(1+{x})/2"),
        y: format!("(ih-ih/zoom)*(1+{y})/2"),
    }
}

pub struct AnchorOffset {
    pub ax: f64,
    pub ay: f64,
}

pub fn anchor_offset(anchor: u32) -> AnchorOffset {
    let col = (anchor.saturating_sub(1)) % 3;
    let row = (anchor.saturating_sub(1)) / 3;
    let ax = (col as f64) / 2.0;
    let ay = if row == 2 {
        0.0
    } else if row == 1 {
        0.5
    } else {
        1.0
    };
    AnchorOffset { ax, ay }
}

fn parse_time_sec(s: &str) -> Option<f64> {
    if let Some(pos) = s.find("time=") {
        let rem = &s[pos + 5..];
        let token = rem.split_whitespace().next().unwrap_or("");
        let parts: Vec<&str> = token.split(':').collect();
        if parts.len() == 3 {
            let h: f64 = parts[0].parse().ok()?;
            let m: f64 = parts[1].parse().ok()?;
            let sec: f64 = parts[2].parse().ok()?;
            return Some(h * 3600.0 + m * 60.0 + sec);
        }
    }
    None
}

async fn run_ffmpeg<F>(
    args: &[String],
    cwd: &Path,
    token: &CancellationToken,
    total_sec: Option<f64>,
    mut on_progress: F,
    resource_dir: Option<&Path>,
) -> Result<(), String>
where
    F: FnMut(f64),
{
    let ffmpeg = ffmpeg_path(resource_dir);
    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-hide_banner")
        .arg("-y")
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        // 0x08000000 = CREATE_NO_WINDOW
        cmd.creation_flags(0x08000000);
    }

    let mut child = cmd.spawn().map_err(|e| format!("Gagal menjalankan FFmpeg ({:?}): {e}", ffmpeg))?;

    let stderr = child.stderr.take().ok_or("Failed to capture FFmpeg stderr")?;
    let mut reader = BufReader::new(stderr).lines();

    let mut tail = Vec::new();

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                let _ = child.kill().await;
                return Err("Dibatalkan".into());
            }
            line_res = reader.next_line() => {
                match line_res {
                    Ok(Some(line)) => {
                        if tail.len() > 20 {
                            tail.remove(0);
                        }
                        tail.push(line.clone());

                        if let Some(tot) = total_sec {
                            if tot > 0.0 {
                                if let Some(sec) = parse_time_sec(&line) {
                                    let prog = (sec / tot).clamp(0.0, 1.0);
                                    on_progress(prog);
                                }
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        }
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    if token.is_cancelled() {
        return Err("Dibatalkan".into());
    }
    if status.success() {
        Ok(())
    } else {
        let msg = tail.join("\n");
        Err(format!("FFmpeg gagal (exit code {:?}): {msg}", status.code()))
    }
}

pub struct SegmentPlan {
    pub clip: ClipRow,
    pub start: f64,
    pub own: f64,
    pub overlap: f64,
    pub transition: String,
}

pub fn plan_segments(clips: &[ClipRow], fps: u32) -> Vec<SegmentPlan> {
    let fps = fps.max(1) as f64;
    let mut elapsed = 0.0;
    let mut boundary = 0u64;
    let mut segs = Vec::new();
    let n = clips.len();

    for (i, clip) in clips.iter().enumerate() {
        // Round cumulative boundaries, not individual lengths, to avoid drift.
        elapsed += (clip.duration_ms.max(1) as f64) / 1000.0;
        let end = ((elapsed * fps).round() as u64).max(boundary + 1);
        let start = boundary as f64 / fps;
        let own = (end - boundary) as f64 / fps;
        let last = i == n - 1;
        let tr_str = match clip.transition.as_str() {
            "slideleft" => "slideleft",
            "zoomin" => "zoomin",
            "cut" => "",
            _ => "fade",
        };
        let overlap = if last || tr_str.is_empty() {
            0.0
        } else {
            (TRANSITION_SEC * fps).round().max(1.0) / fps
        };

        segs.push(SegmentPlan {
            clip: clip.clone(),
            start,
            own,
            overlap,
            transition: if tr_str.is_empty() { "fade".to_string() } else { tr_str.to_string() },
        });
        boundary = end;
    }

    segs
}

fn video_graph(segs: &[SegmentPlan], fps: u32) -> (Vec<String>, String) {
    if segs.len() == 1 {
        return (Vec::new(), "[0:v]".to_string());
    }

    let mut graph = Vec::new();
    let mut blocks = Vec::new();
    let mut start = 0;

    // Concatenate runs of hard cuts before chaining fades. A concat output has
    // a different time base from MP4 inputs, so normalize every block for xfade.
    for i in 0..segs.len() {
        if segs[i].overlap > 0.0 || i == segs.len() - 1 {
            let label = format!("[block{}]", blocks.len());
            if start == i {
                graph.push(format!("[{start}:v]settb=AVTB{label}"));
            } else {
                let inputs = (start..=i).map(|n| format!("[{n}:v]")).collect::<String>();
                graph.push(format!(
                    "{inputs}concat=n={}:v=1:a=0,settb=AVTB{label}",
                    i - start + 1
                ));
            }
            blocks.push((i, label));
            start = i + 1;
        }
    }

    let mut v = blocks[0].1.clone();
    for (i, (last, _)) in blocks.iter().take(blocks.len() - 1).enumerate() {
        let seg = &segs[*last];
        let next = &blocks[i + 1].1;
        let label = format!("[x{i}]");
        let offset = segs[*last + 1].start;
        graph.push(format!(
            "{v}{next}xfade=transition={tr}:duration={dur:.6}:offset={off:.6}{label}",
            tr = seg.transition,
            dur = seg.overlap,
            off = offset
        ));
        v = label;
    }

    // Keep graph timestamps frame-aligned for output and overlays.
    graph.push(format!("{v}fps={fps},settb=AVTB[vbase]"));
    (graph, "[vbase]".to_string())
}

fn find_asset<'a>(bundle: &'a ProjectBundle, id: Option<&str>) -> Option<&'a AssetRow> {
    let aid = id?;
    bundle.assets.iter().find(|a| a.id == aid)
}

fn image_overlay_filters(o: &Value, size: &OutputSize, total: f64, input: usize, k: usize, video: &str) -> [String; 2] {
    let width = o.get("width").and_then(Value::as_f64).unwrap_or(0.14);
    let opacity = o.get("opacity").and_then(Value::as_f64).unwrap_or(0.9).clamp(0.0, 1.0);
    let anchor = o.get("anchor").and_then(Value::as_u64).unwrap_or(9) as u32;
    let x = o.get("x").and_then(Value::as_f64).unwrap_or(0.5) * size.w as f64;
    let y = o.get("y").and_then(Value::as_f64).unwrap_or(0.5) * size.h as f64;
    let start = o.get("start").and_then(Value::as_f64).unwrap_or(0.0).max(0.0);
    let end = o.get("end").and_then(Value::as_f64).unwrap_or(total).min(total);
    let iw = ((((width * size.w as f64) / 2.0).round() * 2.0) as u32).max(2);
    let offset = anchor_offset(anchor);
    [
        format!("[{input}:v]format=rgba,scale={iw}:-1,setsar=1,colorchannelmixer=aa={opacity:.4}[ov{k}]"),
        format!("{video}[ov{k}]overlay=x='{x:.3}-overlay_w*{ax}':y='{y:.3}-overlay_h*{ay}':enable='gte(t,{start:.3})*lt(t,{end:.3})'[vo{k}]", ax = offset.ax, ay = offset.ay),
    ]
}

fn still_image_filter(size: &OutputSize, resolution: u32, fps: u32, preset: &str, strength: &str, frames: u32) -> String {
    let ss: f64 = if resolution == 2160 { 1.5 } else { 2.0 };
    let bw = (((size.w as f64 * ss) / 2.0).round() * 2.0) as u32;
    let bh = (((size.h as f64 * ss) / 2.0).round() * 2.0) as u32;
    let zp = camera_zoompan(preset, strength, frames, fps);
    format!(
        "scale={bw}:{bh}:force_original_aspect_ratio=increase:flags=lanczos,crop={bw}:{bh},format=yuv444p,zoompan=z='{z}':x='{x}':y='{y}':d=1:s={w}x{h}:fps={fps},setsar=1,format=yuv420p",
        z = zp.z, x = zp.x, y = zp.y, w = size.w, h = size.h,
    )
}

async fn render_segment(
    bundle: &ProjectBundle,
    seg: &SegmentPlan,
    index: usize,
    opts: &ExportOptions,
    size: &OutputSize,
    tmp: &Path,
    token: &CancellationToken,
    resource_dir: Option<&Path>,
) -> Result<String, String> {
    let clip = &seg.clip;
    let out_file = format!("seg_{:03}.mp4", index);
    let len = seg.own + seg.overlap;
    let frames = ((len * (opts.fps as f64)).round() as u32).max(1);

    let enc_args = vec![
        "-an".to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "veryfast".to_string(),
        "-crf".to_string(),
        "17".to_string(),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        "-r".to_string(),
        opts.fps.to_string(),
    ];

    if clip.motion_type == "video" {
        if let Some(video) = find_asset(bundle, clip.video_asset_id.as_deref()) {
            let in_path = asset_abs_path(&bundle.project.id, &video.local_path);
            let vf = format!(
                "scale={}:{}:force_original_aspect_ratio=increase:flags=lanczos,crop={}:{},setsar=1,fps={},tpad=stop_mode=clone:stop_duration={:.3},format=yuv420p",
                size.w, size.h, size.w, size.h, opts.fps, len
            );
            let mut args = vec![
                "-i".to_string(),
                in_path.to_string_lossy().to_string(),
                "-vf".to_string(),
                vf,
                "-frames:v".to_string(),
                frames.to_string(),
            ];
            args.extend(enc_args);
            args.push(out_file.clone());

            run_ffmpeg(&args, tmp, token, None, |_| {}, resource_dir).await?;
            return Ok(out_file);
        }
    }

    let image = find_asset(bundle, clip.image_asset_id.as_deref())
        .ok_or_else(|| format!("Klip {:02} (“{}”) belum punya gambar.", index + 1, clip.title))?;
    let in_path = asset_abs_path(&bundle.project.id, &image.local_path);

    let own_frames = ((seg.own * (opts.fps as f64)).round() as u32).max(2);
    let vf = still_image_filter(size, opts.resolution, opts.fps, &clip.camera_preset, &clip.motion_strength, own_frames);

    let mut args = vec![
        "-loop".to_string(),
        "1".to_string(),
        "-framerate".to_string(),
        opts.fps.to_string(),
        "-t".to_string(),
        format!("{:.3}", len),
        "-i".to_string(),
        in_path.to_string_lossy().to_string(),
        "-vf".to_string(),
        vf,
        "-frames:v".to_string(),
        frames.to_string(),
    ];
    args.extend(enc_args);
    args.push(out_file.clone());

    run_ffmpeg(&args, tmp, token, None, |_| {}, resource_dir).await?;
    Ok(out_file)
}

pub async fn render_project(
    ctx: &TaskCtx,
    project_id: &str,
    opts: &ExportOptions,
    db: &DbPool,
    resource_dir: Option<&Path>,
) -> Result<String, String> {
    let bundle = {
        let conn = db.lock().unwrap();
        get_bundle(&conn, project_id).ok_or("Proyek tidak ditemukan")?
    };

    if bundle.clips.is_empty() {
        return Err("Proyek belum punya klip".into());
    }

    let size = output_size(&bundle.project.aspect_ratio, opts.resolution);
    let tmp = user_data_dir().join("tmp").join(format!("export-{}", ctx.job_id));
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;

    let result = render_project_inner(ctx, &bundle, opts, &size, &tmp, resource_dir).await;

    let _ = fs::remove_dir_all(&tmp);

    if result.is_ok() {
        let conn = db.lock().unwrap();
        let _ = touch_project(&conn, project_id, Some("exported"));
    }

    result
}

async fn render_project_inner(
    ctx: &TaskCtx,
    bundle: &ProjectBundle,
    opts: &ExportOptions,
    size: &OutputSize,
    tmp: &Path,
    resource_dir: Option<&Path>,
) -> Result<String, String> {
    let segs = plan_segments(&bundle.clips, opts.fps);
    let total: f64 = segs.iter().map(|s| s.own).sum();

    // 1. Sequential segment rendering (silent video segments)
    let segs_len = segs.len();
    let mut files = vec![String::new(); segs_len];

    let mut done = 0usize;
    for i in 0..segs_len {
        if ctx.token.is_cancelled() {
            return Err("Dibatalkan".into());
        }
        let f = render_segment(bundle, &segs[i], i, opts, size, tmp, &ctx.token, resource_dir).await?;
        files[i] = f;
        done += 1;
        let p = 0.03 + 0.60 * (done as f64 / segs_len as f64);
        ctx.progress(p, Some(&format!("Merender klip {} dari {}", done, segs_len)));
    }

    // 2. Captions & Overlays
    let f_dir = fonts_dir(resource_dir);
    let metrics = FontMetricsCache::new();

    let mut chunk_inputs: Vec<(f64, Vec<WordTiming>)> = Vec::new();
    for s in &segs {
        let mut words = Vec::new();
        if let Some(audio) = find_asset(bundle, s.clip.audio_asset_id.as_deref()) {
            if let Some(w_arr) = audio.meta.get("words").and_then(|x| x.as_array()) {
                for item in w_arr {
                    if let (Some(text), Some(st), Some(en)) = (
                        item.get("text").and_then(|x| x.as_str()),
                        item.get("start").and_then(|x| x.as_f64()),
                        item.get("end").and_then(|x| x.as_f64()),
                    ) {
                        words.push(WordTiming {
                            text: text.to_string(),
                            start: st,
                            end: en,
                        });
                    }
                }
            }
        }
        chunk_inputs.push((s.start, words));
    }

    let chunks = timeline_chunks(&chunk_inputs);
    let burn = opts.burn_captions && !chunks.is_empty();

    let overlays = bundle
        .project
        .editor
        .get("overlays")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();

    let mut text_overlays: Vec<Value> = Vec::new();
    let mut image_overlays: Vec<(Value, PathBuf)> = Vec::new();

    for o in &overlays {
        let kind = o.get("kind").and_then(|x| x.as_str()).unwrap_or("");
        let start = o.get("start").and_then(|x| x.as_f64()).unwrap_or(0.0);
        if start >= total {
            continue;
        }

        if kind == "text" {
            let t = o.get("text").and_then(|x| x.as_str()).unwrap_or("");
            if !t.trim().is_empty() {
                text_overlays.push(o.clone());
            }
        } else if kind == "image" {
            let aid = o.get("assetId").and_then(|x| x.as_str());
            if let Some(asset) = find_asset(bundle, aid) {
                let p = asset_abs_path(&bundle.project.id, &asset.local_path);
                if p.exists() {
                    image_overlays.push((o.clone(), p));
                }
            }
        }
    }

    let use_ass = burn || !text_overlays.is_empty();
    if use_ass {
        let ass_text = build_ass_content(
            if burn { &chunks } else { &[] },
            &bundle.project.editor,
            total,
            size.w,
            size.h,
            &f_dir,
            &metrics,
        );
        fs::write(tmp.join("captions.ass"), ass_text).map_err(|e| e.to_string())?;

        let tmp_fonts = tmp.join("fonts");
        fs::create_dir_all(&tmp_fonts).map_err(|e| e.to_string())?;
        if let Ok(entries) = fs::read_dir(&f_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|s| s.to_str()) == Some("ttf") {
                    if let Some(fname) = p.file_name() {
                        let _ = fs::copy(&p, tmp_fonts.join(fname));
                    }
                }
            }
        }
    }

    // 3. One ffmpeg pass: transitions + overlays + captions + audio mix
    let mut args: Vec<String> = Vec::new();
    for f in &files {
        args.push("-i".to_string());
        args.push(f.clone());
    }

    let (mut graph, mut v) = video_graph(&segs, opts.fps);

    // Logos / image overlays
    for (k, (o, p)) in image_overlays.iter().enumerate() {
        args.push("-i".to_string());
        args.push(p.to_string_lossy().to_string());

        let [scaled, composite] = image_overlay_filters(o, size, total, files.len() + k, k, &v);
        graph.push(scaled);
        graph.push(composite);
        v = format!("[vo{k}]");
    }

    let ass_filter = if use_ass { "ass=captions.ass:fontsdir=fonts," } else { "" };
    graph.push(format!("{v}{ass_filter}format=yuv420p[vout]"));

    let mut input_index = files.len() + image_overlays.len();
    let mut voices = Vec::new();

    for (i, s) in segs.iter().enumerate() {
        if let Some(audio) = find_asset(bundle, s.clip.audio_asset_id.as_deref()) {
            let p = asset_abs_path(&bundle.project.id, &audio.local_path);
            args.push("-i".to_string());
            args.push(p.to_string_lossy().to_string());

            let ms = (s.start * 1000.0).round() as i64;
            graph.push(format!(
                "[{input_index}:a]aresample=48000,aformat=channel_layouts=stereo,adelay={ms}|{ms}[a{i}]"
            ));
            voices.push(format!("[a{i}]"));
            input_index += 1;
        }
    }

    let music_aid = bundle.project.editor.get("musicAssetId").and_then(|x| x.as_str());
    let music = find_asset(bundle, music_aid);
    let mut aout: Option<String> = None;

    if !voices.is_empty() {
        graph.push(format!(
            "{voices_inputs}amix=inputs={count}:normalize=0:dropout_transition=0[voice]",
            voices_inputs = voices.join(""),
            count = voices.len()
        ));
        aout = Some("[voice]".to_string());
    }

    if let Some(mus) = music {
        let p = asset_abs_path(&bundle.project.id, &mus.local_path);
        args.push("-stream_loop".to_string());
        args.push("-1".to_string());
        args.push("-i".to_string());
        args.push(p.to_string_lossy().to_string());

        let vol = bundle.project.editor.get("musicVolumeDb").and_then(|x| x.as_f64()).unwrap_or(-14.0);
        let fade_st = (total - 2.0).max(0.0);
        graph.push(format!(
            "[{input_index}:a]aresample=48000,aformat=channel_layouts=stereo,atrim=0:{total:.3},volume={vol}dB,afade=t=out:st={fade_st:.3}:d=2[mus]"
        ));

        if let Some(_voice_label) = aout {
            if opts.duck_music {
                graph.push("[voice]asplit=2[v1][v2]".to_string());
                graph.push("[mus][v2]sidechaincompress=threshold=0.02:ratio=8:attack=20:release=500[duck]".to_string());
                graph.push("[v1][duck]amix=inputs=2:normalize=0:dropout_transition=0[mix]".to_string());
            } else {
                graph.push("[voice][mus]amix=inputs=2:normalize=0:dropout_transition=0[mix]".to_string());
            }
            aout = Some("[mix]".to_string());
        } else {
            aout = Some("[mus]".to_string());
        }
    }

    if let Some(cur_aout) = aout {
        let loudnorm = if opts.normalize_audio { "loudnorm=I=-14:LRA=11:TP=-1.5," } else { "" };
        graph.push(format!("{cur_aout}{loudnorm}aresample=48000,apad,atrim=0:{total:.3}[aout]"));
    } else {
        graph.push(format!("anullsrc=r=48000:cl=stereo,atrim=0:{total:.3}[aout]"));
    }

    fs::write(tmp.join("graph.txt"), graph.join(";\n")).map_err(|e| e.to_string())?;

    let folder = PathBuf::from(&opts.folder);
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;

    let clean_base = regex::Regex::new(r#"[<>:"/\\|?*]+"#)
        .unwrap()
        .replace_all(&opts.file_name, "-")
        .trim_end_matches(".mp4")
        .to_string();
    let base_name = if clean_base.trim().is_empty() { "video".to_string() } else { clean_base };

    let mut out_path = folder.join(format!("{base_name}.mp4"));
    let mut n = 2;
    while out_path.exists() {
        out_path = folder.join(format!("{base_name}-{n}.mp4"));
        n += 1;
    }

    ctx.progress(0.64, Some("Menggabungkan klip, suara, dan caption"));

    let crf = if opts.resolution == 2160 { "20" } else { "19" };
    let mut final_args = args;
    final_args.extend(vec![
        "-filter_complex_script".to_string(),
        "graph.txt".to_string(),
        "-map".to_string(),
        "[vout]".to_string(),
        "-map".to_string(),
        "[aout]".to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "medium".to_string(),
        "-crf".to_string(),
        crf.to_string(),
        "-r".to_string(),
        opts.fps.to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        "192k".to_string(),
        "-ar".to_string(),
        "48000".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        "-t".to_string(),
        format!("{:.3}", total),
        out_path.to_string_lossy().to_string(),
    ]);

    run_ffmpeg(
        &final_args,
        tmp,
        &ctx.token,
        Some(total),
        |f| {
            ctx.progress(0.64 + 0.34 * f, Some("Menggabungkan klip, suara, dan caption"));
        },
        resource_dir,
    )
    .await?;

    if opts.write_srt && !chunks.is_empty() {
        let srt_path = out_path.with_extension("srt");
        let _ = fs::write(srt_path, build_srt(&chunks));
    }

    Ok(out_path.to_string_lossy().to_string())
}

pub fn start_export(
    db: DbPool,
    job_mgr: Arc<JobManager>,
    app: AppHandle,
    project_id: String,
    opts: ExportOptions,
    resource_dir: Option<PathBuf>,
) -> Result<Value, String> {
    let job = {
        let conn = db.lock().unwrap();
        insert_job(
            &conn,
            &project_id,
            "export",
            "ffmpeg",
            serde_json::to_value(&opts).unwrap_or_default(),
            None,
            None,
        )?
    };

    let j_id = job.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let _ = app.emit("event:job", &job);

    let db_clone = db.clone();
    let p_id = project_id.clone();
    let res_dir = resource_dir.clone();

    job_mgr.spawn_task(&j_id, QueueName::Export, move |ctx| async move {
        let out = render_project(&ctx, &p_id, &opts, &db_clone, res_dir.as_deref()).await?;
        {
            let conn = db_clone.lock().unwrap();
            let _ = touch_project(&conn, &p_id, Some("exported"));
        }
        Ok(Some(out))
    });

    Ok(job)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(duration_ms: i64, transition: &str) -> ClipRow {
        ClipRow {
            id: String::new(),
            project_id: String::new(),
            sort: 0,
            title: String::new(),
            story: String::new(),
            visual_prompt: String::new(),
            visual_source: None,
            narration: String::new(),
            duration_ms,
            character_ids: Vec::new(),
            motion_type: "camera".to_string(),
            camera_preset: "zoomin".to_string(),
            motion_strength: "halus".to_string(),
            video_prompt: String::new(),
            transition: transition.to_string(),
            image_asset_id: None,
            video_asset_id: None,
            audio_asset_id: None,
        }
    }

    #[test]
    fn cuts_concatenate_all_clips_at_frame_boundaries() {
        let clips = [clip(5429, "cut"), clip(6872, "cut"), clip(7337, "cut")];
        let segs = plan_segments(&clips, 60);
        assert_eq!(segs[0].own, 326.0 / 60.0);
        assert_eq!(segs[1].start, 326.0 / 60.0);
        assert_eq!(segs[2].start, 738.0 / 60.0);
        assert_eq!(segs.iter().map(|s| s.own).sum::<f64>(), 1178.0 / 60.0);
        assert!(segs.iter().all(|s| s.overlap == 0.0));

        let (graph, output) = video_graph(&segs, 60);
        assert_eq!(output, "[vbase]");
        assert_eq!(graph[0], "[0:v][1:v][2:v]concat=n=3:v=1:a=0,settb=AVTB[block0]");
        assert!(graph.iter().all(|line| !line.contains("xfade")));
    }

    #[test]
    fn camera_motion_uses_chroma_safe_working_format() {
        let size = OutputSize { w: 1920, h: 1080 };
        let vf = still_image_filter(&size, 1080, 30, "zoomin", "halus", 150);
        assert!(vf.contains("format=yuv444p,zoompan="));
        assert!(vf.contains("setsar=1,format=yuv420p"));
        assert!(vf.contains("scale=3840:2160"));
        let vf_4k = still_image_filter(&OutputSize { w: 3840, h: 2160 }, 2160, 30, "zoomin", "halus", 150);
        assert!(vf_4k.contains("scale=5760:3240")); // 1.5x at 4k
        for (preset, strength, frames, fps) in [("zoomin", "halus", 300, 30), ("zoomout", "kuat", 900, 60), ("kenburns", "sedang", 600, 30)] {
            let motion = camera_zoompan(preset, strength, frames, fps);
            assert!(motion.z.contains("min(on/"));
            assert!(motion.x.contains("iw-iw/zoom"));
            assert!(motion.y.contains("ih-ih/zoom"));
        }
    }

    #[test]
    fn mixed_cuts_and_fades_use_frame_aligned_xfade() {
        let clips = [
            clip(3000, "cut"),
            clip(4000, "fade"),
            clip(5000, "cut"),
            clip(3500, "zoomin"),
            clip(4500, "cut"),
            clip(3000, "fade"),
        ];
        let segs = plan_segments(&clips, 60);
        let (graph, output) = video_graph(&segs, 60);
        assert_eq!(output, "[vbase]");
        assert_eq!(graph[0], "[0:v][1:v]concat=n=2:v=1:a=0,settb=AVTB[block0]");
        assert_eq!(graph[1], "[2:v][3:v]concat=n=2:v=1:a=0,settb=AVTB[block1]");
        assert_eq!(graph[2], "[4:v][5:v]concat=n=2:v=1:a=0,settb=AVTB[block2]");
        assert_eq!(
            graph[3],
            "[block0][block1]xfade=transition=fade:duration=0.400000:offset=7.000000[x0]"
        );
        assert_eq!(
            graph[4],
            "[x0][block2]xfade=transition=zoomin:duration=0.400000:offset=15.500000[x1]"
        );
    }

    #[test]
    fn anchor_offset_matches_preview_nine_points() {
        // Preview anchor points: 1=bottom-left, 2=bottom-center, 3=bottom-right
        // 4=middle-left, 5=center, 6=middle-right
        // 7=top-left, 8=top-center, 9=top-right
        let a1 = anchor_offset(1);
        assert_eq!((a1.ax, a1.ay), (0.0, 1.0));
        let a5 = anchor_offset(5);
        assert_eq!((a5.ax, a5.ay), (0.5, 0.5));
        let a7 = anchor_offset(7);
        assert_eq!((a7.ax, a7.ay), (0.0, 0.0));
        let a9 = anchor_offset(9);
        assert_eq!((a9.ax, a9.ay), (1.0, 0.0));
    }

    #[test]
    fn output_size_handles_aspect_ratios() {
        let l1080 = output_size("16:9", 1080);
        assert_eq!((l1080.w, l1080.h), (1920, 1080));
        let p1080 = output_size("9:16", 1080);
        assert_eq!((p1080.w, p1080.h), (1080, 1920));
        let l4k = output_size("16:9", 2160);
        assert_eq!((l4k.w, l4k.h), (3840, 2160));
    }

    #[test]
    fn image_overlay_preserves_aspect_ratio_and_uses_actual_dimensions() {
        let size = OutputSize { w: 1920, h: 1080 };
        let overlay = serde_json::json!({
            "width": 0.2,
            "opacity": 0.85,
            "anchor": 9,
            "x": 0.95,
            "y": 0.05,
            "start": 1.0,
            "end": 4.5
        });
        let [scaled, composite] = image_overlay_filters(&overlay, &size, 10.0, 3, 0, "[vbase]");
        // Scale preserves aspect ratio with -1 height
        assert_eq!(scaled, "[3:v]format=rgba,scale=384:-1,setsar=1,colorchannelmixer=aa=0.8500[ov0]");
        // Uses overlay_w and overlay_h in placement expressions, with anchor 9 (top right, ax=1, ay=0)
        assert!(composite.contains("overlay=x='1824.000-overlay_w*1':y='54.000-overlay_h*0'"));
        assert!(composite.contains("enable='gte(t,1.000)*lt(t,4.500)'"));
    }
}


use std::path::Path;
use tokio::process::Command;
use regex::Regex;

pub fn pcm_to_wav(pcm: &[u8], sample_rate: u32, channels: u16, bits: u16) -> Vec<u8> {
    let block_align = (channels * bits) / 8;
    let byte_rate = sample_rate * (block_align as u32);
    let mut out = Vec::with_capacity(44 + pcm.len());

    out.extend_from_slice(b"RIFF");
    let file_size = (36 + pcm.len()) as u32;
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // subchunk1 size
    out.extend_from_slice(&1u16.to_le_bytes());  // audio format (PCM)
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    let data_len = pcm.len() as u32;
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(pcm);

    out
}

pub fn is_wav(bytes: &[u8]) -> bool {
    bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE"
}

pub async fn probe(ffmpeg_exe: &Path, file_path: &Path) -> Result<(Option<i64>, Option<(u32, u32)>), String> {
    let mut cmd = Command::new(ffmpeg_exe);
    cmd.args(&["-hide_banner", "-i"]).arg(file_path);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("FFmpeg probe execute failed: {e}"))?;

    let text = String::from_utf8_lossy(&output.stderr);

    // Parse duration: Duration: 00:01:23.45
    let re_dur = Regex::new(r"Duration:\s*(\d+):(\d+):(\d+(?:\.\d+)?)").unwrap();
    let duration_ms = re_dur.captures(&text).and_then(|c| {
        let h = c.get(1)?.as_str().parse::<f64>().ok()?;
        let m = c.get(2)?.as_str().parse::<f64>().ok()?;
        let s = c.get(3)?.as_str().parse::<f64>().ok()?;
        Some(((h * 3600.0 + m * 60.0 + s) * 1000.0).round() as i64)
    });

    // Parse resolution: Video: ..., 1920x1080
    let re_size = Regex::new(r"Video:.*?,\s*(\d{2,5})x(\d{2,5})").unwrap();
    let size = re_size.captures(&text).and_then(|c| {
        let w = c.get(1)?.as_str().parse::<u32>().ok()?;
        let h = c.get(2)?.as_str().parse::<u32>().ok()?;
        Some((w, h))
    });

    Ok((duration_ms, size))
}

use crate::paths::ffmpeg_path;

pub async fn probe_duration_ms(file_path: &Path) -> Result<f64, String> {
    let ffmpeg = ffmpeg_path(None);
    let (dur, _) = probe(&ffmpeg, file_path).await?;
    dur.map(|d| d as f64).ok_or_else(|| "Durasi media tidak terbaca".to_string())
}

pub async fn probe_size(file_path: &Path) -> Result<(u32, u32), String> {
    let ffmpeg = ffmpeg_path(None);
    let (_, size) = probe(&ffmpeg, file_path).await?;
    size.ok_or_else(|| "Resolusi media tidak terbaca".to_string())
}


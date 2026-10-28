"""Verify multi-clip render beyond 0:12 (e.g. 16s total) with cuts, fades, audio mix.
Ensures video never freezes at 0:12 and maintains exact frame count & audio sync.
"""
import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from PIL import Image, ImageDraw

FFMPEG = shutil.which('ffmpeg') or r'C:\Program Files\ffmpeg\bin\ffmpeg.EXE'
FFPROBE = shutil.which('ffprobe') or str(Path(FFMPEG).with_name('ffprobe.EXE'))
assert FFMPEG and Path(FFMPEG).is_file(), f"ffmpeg not found: {FFMPEG}"
assert FFPROBE and Path(FFPROBE).is_file(), f"ffprobe not found: {FFPROBE}"

root = Path(tempfile.mkdtemp(prefix='bang-story-multiclip-'))
print(f"Working in: {root}")

try:
    fps = 30
    w, h = 640, 360 # Fast smoke resolution
    # 4 clips: 4.5s (cut), 4.5s (fade with 0.4s overlap), 4.0s (cut), 4.0s (end) = 17.0s total
    clip_durations = [4.5, 4.5, 4.0, 4.0]
    clip_overlaps = [0.0, 0.4, 0.0, 0.0] # clip 1 has fade overlap of 0.4s
    total_dur = sum(clip_durations) # 17.0s
    total_frames = int(round(total_dur * fps)) # 510 frames

    # Generate 4 distinct color clips
    colors = [(180, 40, 40), (40, 180, 40), (40, 40, 180), (180, 180, 40)]
    clip_files = []
    audio_files = []

    for i, (dur, overlap, col) in enumerate(zip(clip_durations, clip_overlaps, colors)):
        img = Image.new('RGB', (w, h), color=col)
        draw = ImageDraw.Draw(img)
        draw.text((w // 2 - 40, h // 2 - 10), f"Clip {i+1}", fill=(255, 255, 255))
        img_path = root / f"img_{i}.png"
        img.save(img_path)

        # Still video segment matching render_segment: length is own + overlap!
        seg_len = dur + overlap
        frames = int(round(seg_len * fps))
        bw, bh = w * 2, h * 2
        vf = f"scale={bw}:{bh},crop={bw}:{bh},format=yuv444p,zoompan=z='(1+0.05*min(on/{frames-1},1))':x='(iw-iw/zoom)/2':y='(ih-ih/zoom)/2':d=1:s={w}x{h}:fps={fps},setsar=1,format=yuv420p"
        mp4_path = root / f"seg_{i}.mp4"
        subprocess.run([
            FFMPEG, '-y', '-hide_banner', '-loglevel', 'error',
            '-loop', '1', '-framerate', str(fps), '-t', str(seg_len),
            '-i', str(img_path), '-vf', vf,
            '-c:v', 'libx264', '-preset', 'ultrafast', '-crf', '22', '-pix_fmt', 'yuv420p',
            str(mp4_path)
        ], check=True)
        clip_files.append(mp4_path)

        # Audio beep for each clip
        wav_path = root / f"audio_{i}.wav"
        subprocess.run([
            FFMPEG, '-y', '-hide_banner', '-loglevel', 'error',
            '-f', 'lavfi', '-i', f'sine=frequency={440 + i * 110}:duration={dur}',
            str(wav_path)
        ], check=True)
        audio_files.append(wav_path)

    # Graph matching production:
    # Segments:
    # 0: own=4.5, cut (overlap=0)
    # 1: own=4.5, fade (overlap=0.4) -> offset = 4.5 + 4.5 = 9.0
    # 2: own=4.0, cut (overlap=0)
    # 3: own=4.0, cut (overlap=0)
    # Block 0: seg 0 + seg 1 concat -> 9.0s
    # Block 1: seg 2 + seg 3 concat -> 8.0s
    # xfade block0 + block1 with fade at offset 9.0, duration 0.4
    filter_complex = (
        "[0:v][1:v]concat=n=2:v=1:a=0,settb=AVTB[block0];"
        "[2:v][3:v]concat=n=2:v=1:a=0,settb=AVTB[block1];"
        "[block0][block1]xfade=transition=fade:duration=0.400000:offset=9.000000[x0];"
        f"[x0]fps={fps},settb=AVTB,format=yuv420p[vout];"
        # Audio mix
        "[4:a]adelay=0|0[a0];"
        "[5:a]adelay=4500|4500[a1];"
        "[6:a]adelay=9000|9000[a2];"
        "[7:a]adelay=13000|13000[a3];"
        "[a0][a1][a2][a3]amix=inputs=4:duration=longest:dropout_transition=0,volume=4.0[aout]"
    )

    out_mp4 = root / "multiclip_out.mp4"
    cmd = [
        FFMPEG, '-y', '-hide_banner', '-loglevel', 'error',
        '-i', str(clip_files[0]),
        '-i', str(clip_files[1]),
        '-i', str(clip_files[2]),
        '-i', str(clip_files[3]),
        '-i', str(audio_files[0]),
        '-i', str(audio_files[1]),
        '-i', str(audio_files[2]),
        '-i', str(audio_files[3]),
        '-filter_complex', filter_complex,
        '-map', '[vout]',
        '-map', '[aout]',
        '-c:v', 'libx264', '-preset', 'ultrafast', '-crf', '22',
        '-c:a', 'aac', '-b:a', '128k',
        str(out_mp4)
    ]
    subprocess.run(cmd, check=True)

    # Probe both video and audio streams
    probe = subprocess.run(
        [FFPROBE, '-v', 'error', '-show_entries', 'stream=codec_type,duration,nb_frames', '-of', 'json', str(out_mp4)],
        capture_output=True, text=True, check=True
    )
    streams = json.loads(probe.stdout)['streams']
    v_stream = next(s for s in streams if s['codec_type'] == 'video')
    a_stream = next(s for s in streams if s['codec_type'] == 'audio')

    v_dur = float(v_stream['duration'])
    a_dur = float(a_stream['duration'])
    v_frames = int(v_stream['nb_frames'])

    print(f"Video duration: {v_dur:.3f}s, frames: {v_frames}")
    print(f"Audio duration: {a_dur:.3f}s")

    # Assert video played through all 17 seconds (well past 0:12)
    assert v_dur >= 16.9, f"Video duration {v_dur} cut short before 17s!"
    assert abs(v_dur - a_dur) < 0.1, f"Video ({v_dur}s) and Audio ({a_dur}s) duration mismatch!"
    assert v_frames == total_frames, f"Expected {total_frames} frames, got {v_frames}"

    print(f"PASS: 17s multi-clip render passed cleanly past 0:12 with matching A/V duration.")
finally:
    shutil.rmtree(root, ignore_errors=True)

"""Local test fixture rendering an actual MP4 with zoom, ASS captions, and overlays.
Uses production ASS generated directly by Rust ass.rs, bundled fonts, and non-square overlays.
Validates output video stream with ffprobe (frames, duration, resolution, codec).
"""
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from PIL import Image, ImageDraw

FFMPEG = shutil.which('ffmpeg') or r'C:\Program Files\ffmpeg\bin\ffmpeg.EXE'
FFPROBE = shutil.which('ffprobe') or str(Path(FFMPEG).with_name('ffprobe.EXE'))
assert FFMPEG and Path(FFMPEG).is_file(), f"ffmpeg not found: {FFMPEG}"
assert FFPROBE and Path(FFPROBE).is_file(), f"ffprobe not found: {FFPROBE}"

root = Path(tempfile.mkdtemp(prefix='bang-story-fixture-'))
print(f"Working in: {root}")

try:
    # 1. Base image 1920x1080
    w, h, fps, dur_sec = 1920, 1080, 30, 3
    base_img = Image.new('RGB', (w, h), color=(30, 40, 60))
    draw = ImageDraw.Draw(base_img)
    for y in range(0, h, 60):
        draw.line([(0, y), (w, y)], fill=(50, 70, 100), width=2)
    for x in range(0, w, 100):
        draw.line([(x, 0), (x, h)], fill=(50, 70, 100), width=2)
    draw.ellipse([(w // 2 - 150, h // 2 - 150), (w // 2 + 150, h // 2 + 150)], fill=(200, 180, 50))
    base_img.save(root / 'base.png')

    # 2. Non-square logo (400x120) with transparency
    logo_w, logo_h = 400, 120
    logo_img = Image.new('RGBA', (logo_w, logo_h), (0, 0, 0, 0))
    draw_logo = ImageDraw.Draw(logo_img)
    draw_logo.rounded_rectangle([(0, 0), (logo_w - 1, logo_h - 1)], radius=20, fill=(230, 50, 50, 220))
    draw_logo.text((50, 40), "LOGO TEST", fill=(255, 255, 255, 255))
    logo_img.save(root / 'logo.png')

    # 3. Copy bundled fonts
    fonts_src = Path('resources/fonts')
    if not fonts_src.exists():
        fonts_src = Path('../resources/fonts')
    fonts_dst = root / 'fonts'
    fonts_dst.mkdir()
    for f in fonts_src.glob('*.ttf'):
        shutil.copy(f, fonts_dst / f.name)

    # 4. Generate actual production ASS files via Rust unit test
    print("Generating production ASS fixtures via Rust...")
    env = dict(os.environ, BANG_STORY_FIXTURE_DIR=str(root))
    subprocess.run(
        ['cargo', 'test', '--manifest-path', 'src-tauri/Cargo.toml', 'write_render_fixtures', '--', '--ignored'],
        env=env, check=True, capture_output=True
    )
    generated = list(root.glob('*.ass'))
    print(f"Generated {len(generated)} ASS files: {[p.name for p in generated]}")
    assert (root / 'custom.ass').is_file()
    assert (root / 'karaoke.ass').is_file()

    # 5. FFmpeg pipeline matching exporter.rs:
    frames = fps * dur_sec
    bw, bh = w * 2, h * 2
    e = f'(min(on/{frames-1},1)*min(on/{frames-1},1)*(3-2*min(on/{frames-1},1)))'
    z = f'(1+0.08*{e})'
    vf_still = (
        f"scale={bw}:{bh}:force_original_aspect_ratio=increase:flags=lanczos,crop={bw}:{bh},"
        f"format=yuv444p,zoompan=z='{z}':x='(iw-iw/zoom)/2':y='(ih-ih/zoom)/2':"
        f"d=1:s={w}x{h}:fps={fps},setsar=1,format=yuv420p"
    )

    seg_mp4 = root / 'seg.mp4'
    cmd1 = [
        FFMPEG, '-y', '-hide_banner', '-loglevel', 'error',
        '-loop', '1', '-framerate', str(fps), '-t', str(dur_sec),
        '-i', str(root / 'base.png'),
        '-vf', vf_still,
        '-c:v', 'libx264', '-preset', 'ultrafast', '-crf', '20',
        '-pix_fmt', 'yuv420p',
        str(seg_mp4)
    ]
    print("Rendering base segment with smoothstep zoom...")
    subprocess.run(cmd1, check=True)

    # Test rendering with each generated ASS preset
    w_frac = 0.15
    iw = round(w_frac * w / 2) * 2
    ox, oy = 0.85, 0.15
    x = f"{ox * w:.3f}-overlay_w*1.0"
    y = f"{oy * h:.3f}-overlay_h*0.0"
    fonts_path_escaped = str(root / 'fonts').replace('\\', '/').replace(':', '\\:')

    expected_fonts = {
        'karaoke.ass': ['Poppins-ExtraBold', 'Poppins-Medium', 'Poppins-Bold'],
        'minimal.ass': ['Poppins-SemiBold', 'Poppins-Medium'],
        'subtitle.ass': ['Poppins-Medium'],
        'tebal.ass': ['Poppins-ExtraBold'],
        'none.ass': ['Poppins-Medium', 'Poppins-Bold'],
        'custom.ass': ['Bangers', 'Poppins-Medium'],
        'literal.ass': ['Poppins-Medium'],
    }
    for ass_name, faces in expected_fonts.items():
        ass_path_escaped = str(root / ass_name).replace('\\', '/').replace(':', '\\:')
        filter_complex = (
            f"[0:v]format=yuv420p[vbase];"
            f"[1:v]format=rgba,scale={iw}:-1,setsar=1,colorchannelmixer=aa=0.9000[ov0];"
            f"[vbase][ov0]overlay=x='{x}':y='{y}':enable='gte(t,0.000)*lt(t,3.000)'[vo0];"
            f"[vo0]ass='{ass_path_escaped}':fontsdir='{fonts_path_escaped}',format=yuv420p[vout]"
        )

        out_mp4 = root / f'out_{ass_name}.mp4'
        cmd2 = [
            FFMPEG, '-y', '-hide_banner', '-loglevel', 'verbose',
            '-i', str(seg_mp4),
            '-i', str(root / 'logo.png'),
            '-filter_complex', filter_complex,
            '-map', '[vout]',
            '-c:v', 'libx264', '-preset', 'ultrafast', '-crf', '20',
            '-pix_fmt', 'yuv420p',
            str(out_mp4)
        ]
        print(f"Compositing with {ass_name}...")
        res = subprocess.run(cmd2, capture_output=True, text=True, check=True)
        for face in faces:
            assert face in res.stderr, f"Expected {face} to be selected by libass in {ass_name}! Log excerpt:\n{res.stderr[-800:]}"

        probe = subprocess.run(
            [FFPROBE, '-v', 'error', '-show_entries', 'stream=width,height,duration,nb_frames,codec_name', '-of', 'json', str(out_mp4)],
            capture_output=True, text=True, check=True
        )
        data = json.loads(probe.stdout)
        stream = data['streams'][0]
        assert stream['width'] == w
        assert stream['height'] == h
        assert int(stream['nb_frames']) == frames
        assert abs(float(stream['duration']) - dur_sec) < 0.05
        print(f"PASS ({ass_name}): {w}x{h}, {stream['nb_frames']} frames, duration {stream['duration']}s.")

    print("ALL PRODUCTION ASS FIXTURES PASSED!")
finally:
    shutil.rmtree(root, ignore_errors=True)

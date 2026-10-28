"""Local FFmpeg motion sampling check; writes only to a fresh temporary directory."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

import numpy as np
from PIL import Image, ImageDraw

parser = argparse.ArgumentParser()
parser.add_argument('--width', type=int, default=640)
parser.add_argument('--height', type=int, default=360)
parser.add_argument('--fps', type=int, default=30)
parser.add_argument('--frames', type=int, default=180)
parser.add_argument('--include-4x', action='store_true')
args = parser.parse_args()
w, h, fps, frames = args.width, args.height, args.fps, args.frames
ffmpeg = shutil.which('ffmpeg')
assert ffmpeg
root = Path(tempfile.mkdtemp(prefix='bang-story-motion-'))
im = Image.new('RGB', (w, h), 'black')
draw = ImageDraw.Draw(im)
draw.rectangle((w // 2 - 2, 0, w // 2 + 2, h - 1), fill='white')
im.save(root / 'line.png')
results = []
variants = [
    ('baseline', 1.5 if h >= 2160 else 2.0, 'yuv420p'),
    ('production (444)', 1.5 if h >= 2160 else 2.0, 'yuv444p'),
]
if args.include_4x:
    variants.append(('444-4x', 4.0, 'yuv444p'))
for direction in ('zoomin', 'zoomout'):
    for name, factor, fmt in variants:
        bw, bh = round(w * factor / 2) * 2, round(h * factor / 2) * 2
        e = f'(min(on/{frames-1},1)*min(on/{frames-1},1)*(3-2*min(on/{frames-1},1)))'
        z = f'(1+0.08*{e})' if direction == 'zoomin' else f'(1.08-0.08*{e})'
        vf = f"scale={bw}:{bh}:flags=lanczos,format={fmt},zoompan=z='{z}':x='(iw-iw/zoom)/2':y='(ih-ih/zoom)/2':d=1:s={w}x{h}:fps={fps},format=gray"
        command = [ffmpeg, '-hide_banner', '-loglevel', 'error', '-filter_threads', '1', '-loop', '1', '-framerate', str(fps), '-i', str(root / 'line.png'), '-vf', vf, '-frames:v', str(frames), '-f', 'rawvideo', '-pix_fmt', 'gray', '-']
        started = time.monotonic()
        proc = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        centers = []
        for i in range(frames):
            raw = proc.stdout.read(w * h)
            if len(raw) != w * h:
                raise RuntimeError(proc.stderr.read().decode())
            row = np.frombuffer(raw, dtype=np.uint8).reshape(h, w)[h // 2].astype(float)
            centers.append(float(np.dot(row, np.arange(w)) / row.sum()))
        assert proc.wait() == 0, proc.stderr.read().decode()
        delta = np.diff(centers)
        results.append({'direction': direction, 'variant': name, 'seconds': round(time.monotonic() - started, 2), 'max_center_step_px': round(float(np.max(np.abs(delta))), 4), 'rms_center_step_px': round(float(np.sqrt(np.mean(delta * delta))), 4), 'center_range_px': round(float(np.ptp(centers)), 4)})
print(json.dumps({'width': w, 'height': h, 'fps': fps, 'frames': frames, 'output': str(root), 'results': results}, indent=2))

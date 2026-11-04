"""A Python program animating a flame: settings changed over time, every
frame saved as a PNG and piped to FFmpeg.

    python python/examples/animate_config.py [CONFIG.fflame] [OUT_DIR]
"""

import math
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import pyfflame as ff  # noqa: E402

config_path = sys.argv[1] if len(sys.argv) > 1 else "tests/visual/configs/2d/plastic-sierpinski.fflame"
out = sys.argv[2] if len(sys.argv) > 2 else "output/python/animate_config"
os.makedirs(out, exist_ok=True)

W, H, FPS, SECONDS = 640, 360, 30, 3
frames = FPS * SECONDS

eng = ff.Engine()
cfg = ff.Config.load(config_path)
affine = list(cfg.get_affine(0))
b0 = affine[1]

start = time.perf_counter()
with ff.VideoWriter(os.path.join(out, "animation.mp4"), W, H, fps=FPS) as video:
    for i in range(frames):
        t = i / frames
        # Shear the first transform and turn the view, one cycle over the clip.
        affine[1] = b0 + 0.25 * math.sin(2 * math.pi * t)
        cfg.set_affine(0, *affine)
        cfg.rotation = 2 * math.pi * t
        image = eng.render(cfg, W, H, iterations=100_000_000, sequence=True)
        image.save(os.path.join(out, f"frame{i:04d}.png"))
        video.write(image)
print(f"{frames} frames in {time.perf_counter() - start:.1f}s -> {out}")

"""A Rhai script written in Python adds an animation to a flame; the
animation engine renders it, frame by frame, into a video -- the frames
the app's video export would make.

    python wasm/python/examples/rhai_animation.py [CONFIG.fflame] [OUT_DIR]
"""

import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import fflame  # noqa: E402

config_path = sys.argv[1] if len(sys.argv) > 1 else "tests/visual/configs/2d/plastic-sierpinski.fflame"
out = sys.argv[2] if len(sys.argv) > 2 else "output/python-spike/rhai_animation"
os.makedirs(out, exist_ok=True)

# A modifier: it starts from the flame it is given. Touching `anim` makes
# the run emit an animation (docs/main/SCRIPTING.md, "The anim object").
TURNTABLE = r'''
script("Turntable", "modifier");
let turns = param("turns", 1.0, 0.25, 4.0);
let seconds = param("seconds", 4.0, 1.0, 30.0);
anim.name = "Turntable";
anim.key("rotation", 0.0, 0.0);
anim.key("rotation", seconds, turns * 2.0 * PI());
anim.key("zoom", 0.0, 1.0);
anim.key("zoom", seconds * 0.5, 1.6, "ease_in_out");
anim.key("zoom", seconds, 1.0, "ease_in_out");
anim.duration = seconds;
'''

eng = fflame.Engine()
print("declared:", [p["key"] for p in eng.script_params(TURNTABLE)["params"]])
result = eng.run_script(TURNTABLE, params={"turns": 0.5, "seconds": 3.0}, base=fflame.load_config(config_path))
for w in result.warnings:
    print("warning:", w)

W, H = 640, 360
start = time.perf_counter()
with eng.open_animation(result.animation, W, H, fps=30, config=result.config, iterations=100_000_000) as anim:
    print(f"{anim.frames} frames at {anim.fps} fps")
    with fflame.VideoWriter(os.path.join(out, "turntable.mp4"), W, H, fps=anim.fps) as video:
        for n, image in enumerate(anim):
            video.write(image)
            if n % 30 == 0:
                image.save(os.path.join(out, f"frame{n:04d}.png"))
print(f"rendered in {time.perf_counter() - start:.1f}s -> {out}")

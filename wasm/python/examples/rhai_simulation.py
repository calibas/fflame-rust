"""A Rhai generator written in Python builds a simulation and animates
its run; the animation engine renders it into a video, the field running
on from frame to frame. Then a library script, run by id with a seed.

    python wasm/python/examples/rhai_simulation.py [OUT_DIR]
"""

import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import fflame  # noqa: E402

out = sys.argv[1] if len(sys.argv) > 1 else "output/python-spike/rhai_simulation"
os.makedirs(out, exist_ok=True)

# docs/main/SCRIPTING.md, "Animating the simulation itself".
CORAL = r'''
script("Coral run", "generator");
sim.model("gray_scott");
sim.preset("coral");
sim.grid(384, 384);
sim.init("blobs");
sim.seed(7);
anim.key("Sim.Steps", 0.0, 0.0);
anim.key("Sim.Steps", 4.0, 6000.0);
anim.key("Sim.ModelParam.feed", 0.0, 0.0545);
anim.key("Sim.ModelParam.feed", 4.0, 0.030);
anim.duration = 4.0;
'''

eng = fflame.Engine()
result = eng.run_script(CORAL)
W, H = 512, 512
start = time.perf_counter()
with eng.open_animation(result.animation, W, H, fps=24, config=result.config) as anim:
    with fflame.VideoWriter(os.path.join(out, "coral.mp4"), W, H, fps=anim.fps) as video:
        for n, image in enumerate(anim):
            video.write(image)
        image.save(os.path.join(out, "coral-last.png"))
print(f"{anim.frames} frames in {time.perf_counter() - start:.1f}s")

# A library script by id, with a parameter and a seed: byte for byte the
# file the CLI's `generate --script sim_sweep.rhai --seed 42 --set
# model=brusselator` writes.
sweep = eng.run_script(eng.script_source("sim_sweep"), seed=42, params={"model": "brusselator"})
eng.render(sweep.config, W, H).save(os.path.join(out, "sim_sweep-42.png"))
fflame.save_config(sweep.config_text, os.path.join(out, "sim_sweep-42.fflame"))
print("sim_sweep seed 42 ->", out)

# fflame for Python (spike)

Branch `python-wasm-spike`. The engine -- flames, escape-time fractals,
simulations, terrains, the animation engine and Rhai scripting -- built
as one WebAssembly module for WASI, which Python runs with
[wasmtime](https://pypi.org/project/wasmtime/). The GPU work goes to the
host through a wgpu custom backend (`src/backend.rs`) and is done there
by [wgpu-py](https://pypi.org/project/wgpu/) (`fflame/_gpu.py`). Nothing
is compiled per platform: the same `.wasm` runs wherever wasmtime and
wgpu-py do.

## Build and run

```bash
# the module (from wasm/python; uses wasm/.cargo/config.toml's target dir)
CARGO_TARGET_WASM32_WASIP1_RUSTFLAGS="-Ctarget-feature=+simd128 -Clink-arg=-zstack-size=67108864" \
    cargo build --release --target wasm32-wasip1
pip install wasmtime wgpu          # numpy/Pillow optional (Image.to_numpy / to_pil)

python wasm/python/examples/animate_config.py    # Python animates a config: PNG frames + MP4
python wasm/python/examples/rhai_animation.py    # a Rhai script adds an animation; the animation engine renders it
python wasm/python/examples/rhai_simulation.py   # a Rhai simulation run, animated; a library script by seed
python wasm/python/compare.py CONFIG.fflame...   # the module's pictures against the native CLI's
```

The package finds the module beside itself (`fflame/fflame.wasm`), at
`$FFLAME_WASM`, or in the build output above. MP4s need FFmpeg on the
PATH.

## The Python API (`fflame/__init__.py`)

```python
import fflame
eng = fflame.Engine()
cfg = fflame.load_config("flame.fflame")        # a config is a dict
cfg["zoom"] *= 2
image = eng.render(cfg, 1280, 720)              # RGBA; .save(path) writes the CLI's PNG, metadata and all
image.to_numpy()

with fflame.VideoWriter("out.mp4", 1280, 720, fps=30) as video:
    for i in range(90):
        cfg["rotation"] = i / 90 * 6.2832
        video.write(eng.render(cfg, 1280, 720, sequence=True))   # a simulation runs on between frames

result = eng.run_script(rhai_source, seed=7, params={...}, base=cfg)   # byte-identical to the CLI's `generate`
with eng.open_animation(result.animation, 1280, 720, fps=30, config=result.config) as anim:
    for image in anim:                          # the video export's frames
        ...
eng.scripts(); eng.script_source("sim_sweep"); eng.presets()
eng.import_flame("x.flame"); eng.export_flame(cfg)
```

Renders are the CLI's: 1024 iterations per thread unless given, and the
sticky shader superset off unless `sticky=True` (fewer compiles across a
batch of different flames, at the price of the specialized shader's
exact picture).

## What crosses the boundary

- **Python -> module**: one export, `fflame_call`: a JSON request
  (`{"fn": ...}`) and a block of bytes; back come a status, bytes and a
  JSON note. `src/lib.rs` lists the calls.
- **module -> Python**: two imports, `fflame_gpu.call` / `take`: each GPU
  call as a JSON command. Command buffers are recorded in the module
  and replayed by the host at submit; buffer maps run when the device
  is polled, as in wgpu-core.

## Measured (GTX 1660 SUPER, Windows)

- Pictures: 254 visual-suite configs against the native CLI, none
  failing. Every escape render and all 107 simulations bit-identical,
  but two perturbed deep zooms (~5% of pixels in chaotic regions; the
  CPU's libm the likelier cause). Flames bit-identical at the same
  iterations per thread.
- Speed: a warm 1e9-iteration flame at 800x600 in 507 ms (native CLI
  585); a Cahn-Hilliard simulation (16,000 dispatches) 351 ms against
  388. A flame whose variation set is new costs a pipeline compile,
  200-500 ms; a repeated one renders in milliseconds.
- The module is 12.5 MB and loads in ~0.4 s; wgpu-py's device takes ~1 s.

## Not done

- Depth/stencil, vertex and index buffers, render bundles and timestamp
  queries are not forwarded (nothing in the 254 configs needed them).
  Error scopes report no error; a validation error raises in Python.
- wgpu-py 0.32's `queue.on_submitted_work_done_sync` is broken; the host
  waits with the private `device._poll_wait()`.
- The script calls are a port of `wasm/script`'s, and the animation
  frame loop a copy of the video export's core; both should be one
  function in the engine that every module calls.
- Audio-driven signal tracks (an audio file) are not loaded; generator
  signals are.

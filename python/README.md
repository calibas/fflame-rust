# pyfflame

Fractal flames, escape-time fractals and simulations from Python: build
and edit them, run the app's Rhai scripts, render them on the GPU,
animate them, and write PNGs and video.

pyfflame is the Fractal Art Editor's own engine, built as one
WebAssembly module: [wasmtime](https://pypi.org/project/wasmtime/) runs
it and [wgpu-py](https://pypi.org/project/wgpu/) does its GPU work, so
one wheel serves every platform those two do. A flame saved here is the
file the app writes; a script and seed give the app's flame byte for
byte; a render is the CLI's picture.

## Build and install

```bash
rustup target add wasm32-wasip1        # once
python python/build.py                 # the module -> python/pyfflame/fflame.wasm
pip install -e python/                 # or: python python/build.py --wheel; pip install python/dist/*.whl
```

Dependencies: `wasmtime`, `wgpu` (both install prebuilt wheels from
their maintainers); `numpy` and `pillow` for `Image.to_numpy` / `to_pil`;
FFmpeg on the PATH for `VideoWriter`.

## Build and edit a flame

```python
import pyfflame as ff

c = ff.Config()
i = c.add_transform()
c.set_weight(i, 0.5)
c.set_affine(i, 0.5, 0.0, 0.0, 0.5, 0.25, 0.0)
c.add_variation(i, "julian", 1.0)          # checked: a typo raises
c.set_variation_param(i, "julian.power", 3.0)
c.zoom = 1.5

c.save("mine.fflame")                      # the app's JSON
c.save_flame_xml("mine.flame")             # Apophysis / JWildfire XML
c.set("exposure", 0.8)                     # any setting, by the app's rules
c.set("Sim.Terrain.Height", 0.25)          # the file's name or the track's
```

`Config.set(key, value)` is a script's `config.set`: a setting the
panels edit is clamped and applied as the panel applies it (a time step
of 0 is held to the solver's floor), the flame is refused (use the
transform calls), and a name that changes nothing warns. The properties
(`zoom`, `pan_x`, ...) set through it. `c.data` is the file's JSON, for
reading anything, or writing a field as given.

`Config.load` / `from_json`, `load_flame_xml` / `parse_flame_xml` (a
list: one file may hold several flames); transforms by index:
`get_/set_weight`, `_color`, `_color_speed`, `_opacity`, `_affine`,
`add_variation`, `remove_variation`, `set_variation_param`,
`get_variations`, `get_variation_params`; `variations()`,
`variation_params(name)`.

## Run a script

The app's sandboxed Rhai engine and seeded RNG: `(script, seed, params)`
gives the flame the editor and `FractalArtEditor generate` give.

```python
src = open("assets/scripts/generators/lsystem.rhai", encoding="utf-8").read()
r = ff.run_script(src, seed=3, params={"output": "Path (finite depth)", "angle": 90.0})
print(r.messages)
r.config.save("hilbert.fflame")            # byte-identical to the CLI's
```

A choice parameter takes its option's text or index, a colour `"#rrggbb"`
or `(r, g, b)`; an undeclared name raises. Seeds wrap modulo 2^64. A
script that keys `anim` returns `r.animation` (a `.anim`).

## Render

```python
eng = ff.Engine()                          # the GPU starts on the first render
image = eng.render(c, 1920, 1080)          # Config, dict, JSON or a path
image.save("mine.png")                     # the CLI's PNG, the config in its metadata
array = image.to_numpy()                   # (1080, 1920, 4) uint8
```

`render(config, w, h, iterations=None, iterations_per_thread=None,
transparent=False, premultiplied=False, sticky=False, sequence=False)`.
Every engine: flames 2D and 3D, escape-time (deep zoom, solids, terrains)
and simulations.

## Animate

Change a config over time from Python. `sequence=True` makes each frame
continue the last: a simulation runs on to the frame's step count
instead of restarting from its seed.

```python
with ff.VideoWriter("spin.mp4", 1280, 720, fps=30) as video:
    for n in range(90):
        c.rotation = n / 90 * 6.2832
        frame = eng.render(c, 1280, 720, sequence=True)
        frame.save(f"frame{n:04d}.png")
        video.write(frame)
```

Or let the animation engine do it, from a `.anim` or a script's animation
-- the frames the app's video export makes:

```python
r = ff.run_script(turntable_source, base=c)
with eng.open_animation(r.animation, 1280, 720, fps=30) as frames:
    for image in frames:
        video.write(image)
```

`python/examples/` has all three, runnable from the repository root.

## Tests and tools

```bash
python -m pytest python/tests              # GPU tests skip without an adapter
python python/tools/compare_native.py tests/visual/configs/escape/*.fflame
                                           # the module's pictures against the CLI's
```

The design, what was measured, and what is open:
[docs/archive/projects/pyfflame.md](../docs/archive/projects/pyfflame.md).

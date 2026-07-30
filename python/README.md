# pyfflame

Build, edit and convert fractal flames from Python.

```python
import pyfflame as ff

c = ff.Config()
i = c.add_transform()
c.set_weight(i, 0.5)
c.set_affine(i, 0.5, 0.0, 0.0, 0.5, 0.25, 0.0)
c.add_variation(i, "julian", 1.0)
c.set_variation_param(i, "julian.power", 3.0)

c.save("mine.fflame")          # our JSON format
c.save_flame_xml("mine.flame") # Apophysis / JWildfire XML
```

Run a flame script — the same sandboxed Rhai engine and the same seeded
RNG the app uses, so a given `(script, seed, params)` produces the same
flame here as in the editor:

```python
src = open("assets/scripts/generators/lsystem.rhai", encoding="utf-8").read()
r = ff.run_script(src, seed=3, params={
    "axiom": "X",
    "rule_1": "X=-YF+XFX+FY-",
    "rule_2": "Y=+XF-YFY-FX+",
    "angle": 90.0,
    "output": "Path (finite depth)",
})
for line in r.messages:
    print(line)
r.config.save("hilbert.fflame")
```

## What's here

| | |
| --- | --- |
| `Config` | a whole flame: model, camera, colour and render settings |
| `Config.load` / `.save` | `.fflame` (JSON) |
| `Config.load_flame_xml` / `.save_flame_xml` | `.flame` (Apophysis/JWildfire XML) |
| `Config.from_json` / `.to_json` / `.parse_flame_xml` / `.to_flame_xml` | the same, as strings |
| `run_script(source, seed=1, params=None, base=None)` | run a Rhai flame script |
| `variations()` / `variation_params(name)` | what the registry knows |

Transforms are addressed by index: `add_transform()` returns one, and
`get_/set_weight`, `_color`, `_color_speed`, `_opacity`, `get_/set_affine`,
`add_variation`, `remove_variation`, `set_variation_param`,
`get_variations`, `get_variation_params` all take it first.

**No rendering.** The wheel carries no GPU code. To turn a flame into a
PNG, call the app's headless exporter:

```python
import subprocess
c.save("out.fflame")
subprocess.run(["fractal_flame_wgpu", "export",
                "-i", "out.fflame", "-o", "out.png",
                "-w", "1920", "-H", "1080"], check=True)
```

## Three things that will bite you otherwise

**Values are f32.** The flame model is 32-bit throughout (WGSL has no
f64), so a Python float is stored rounded — set `-0.1` and read back
`-0.10000000149011612`. Compare with a tolerance.

**Angles are radians.** `rotation`, `camera_pitch` and `camera_yaw` are
stored in radians — the app's View panel converts for display, so the
number you see in the UI is not the number in the file. Use
`math.radians(35)`, not `35`.

**Names are checked.** `add_variation` and `set_variation_param` raise on
an unknown variation or parameter, and `set_variation_param` lists the
ones that do exist. A misspelled variation would otherwise render as a
silently missing one, which is near-impossible to spot in an image.

## Building

```bash
cd python
maturin build --release      # -> target/wheels/*.whl
pip install target/wheels/pyfflame-*.whl
python tests/test_pyfflame.py
```

This crate is a **standalone build** with its own `[workspace]`,
depending on the main crate by path. Nothing here changes the app: no
features are toggled and no modules are gated, so the editor's build and
codegen are untouched. It links the app's whole dependency graph, but the
linker drops what is never called — the wheel comes out around 2.6 MB
with no GPU or window code in it.

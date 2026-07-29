# pyfflame — Python bindings

Status doc for the Python side. Phase 5 of
[flame-scripting.md](flame-scripting.md) shipped a working wheel; this
tracks what is left, roughly in the order it is worth doing.

User-facing usage lives in [`python/README.md`](../../python/README.md)
and is not repeated here.

---

## 1. What ships today

`python/` is a **standalone crate with its own `[workspace]`**, depending
on the main crate by path.

- `Config` — flame model, camera, colour and render settings.
- `.fflame` and `.flame` read/write, as files and as strings.
- Transform editing by index: weight, colour, colour speed, opacity,
  affine, variations, variation parameters — with registry validation.
- `run_script(source, seed, params, base)` — the app's Rhai engine.
- `variations()`, `variation_params(name)`.
- Ten Python tests; `run_script` output is byte-identical to
  `fractal_flame_wgpu generate` for the same script and seed.

### The constraints this was built under

The app must be **untouched**: no features toggled, no modules gated, so
its build and codegen are byte-for-byte what they were, with zero
performance impact and no rewrite risk. Everything below inherits that
constraint — if a change would require editing the main crate, it needs
justifying on its own terms, not smuggling in as "the Python work needs
it".

**The `gui` feature-gating refactor is not needed and should not be
done for this.** It was in the original plan; measurement killed it. The
core is nearly free-standing already (211 variation files with zero
GUI/GPU references; exactly one code edge into GPU-land, the
`MAX_BLUR_BUFFERS` const), and the linker drops what is never called —
**the wheel is 2.6 MB** with no GPU or window code in it. Revisit only
if Linux CI proves impossible (see §5).

---

## 2. Tier 1 — ergonomics

The index-based API is correct but wordy. Everything here is additive.

**Transform proxy.** `c.transforms[0].weight = 0.5` instead of
`c.set_weight(0, 0.5)`, and iteration: `for t in c.transforms:`.
A proxy holds `(Py<Config>, index)` and revalidates on every access.

> Pitfall to design against: a proxy is a *reference by index*, so after
> `remove_transform(0)` a live proxy silently addresses a different
> transform. Revalidate the index on access and raise if it is out of
> range; do not cache the transform itself. Document that proxies are
> views, not values.

**Palette access.** Currently invisible from Python, though every flame
carries one. Wants: read/write stops as `[(position, (r, g, b)), …]`,
the palette name, load a `.palette` file, and list the built-in library.
Also `palette_rotation` / `palette_reverse` / squeeze settings, which are
plain `FractalConfig` fields.

**`render()` helper.** A thin subprocess wrapper around
`fractal_flame_wgpu export`, so the README's boilerplate becomes
`c.render("out.png", width=1920, height=1080)`. Needs a binary-discovery
rule: explicit argument → `FFLAME_BIN` env var → `PATH` → raise with a
message naming all three. Deliberately still a subprocess: no GPU in the
wheel.

**Bulk property access.** `c.camera` / `c.tonemap` returning a dict-like
view, rather than one getter per field, for the ~60 flat `FractalConfig`
fields. Cheaper to maintain than 120 hand-written accessors.

---

## 3. Tier 2 — model coverage

Real parts of the model Python cannot currently reach.

| Gap | Notes |
| --- | --- |
| **Linked / final transforms** | `flame.linked_transforms`, `flame.final_transforms` — same shape as `transforms`, so this falls out of the proxy work. |
| **Post-affine** | Per-transform second affine; `post_affine_enabled` plus six coefficients. |
| **Xaos** | `flame.xaos: Option<Vec<Vec<f32>>>` — a from/to matrix. Natural as `c.xaos[from][to]`, or get/set with a resize-aware setter. |
| **Subflames** | `flame.subflames` — nested flames. Structural; needs its own thought about how a subflame is addressed. |
| **Solo transform** | `flame.solo_transform: Option<usize>`. Trivial. |
| **Post-symmetry** | `flame.post_symmetry`. Trivial. |
| **Tone mapping / colour** | ~30 flat fields (exposure, gamma, vibrancy, levels, curve, background). Covered by the bulk-view idea above. |
| **Effects chain** | `density_effects` / `color_effects` — `Vec<EffectInstance>`. Structural, lower value for a scripting audience. |
| **Presets** | Read the shipped `.fflame` preset library as `Config`s. |

Rough priority: proxy-driven items (linked/final, post-affine) first
since they come nearly free; then xaos; then the flat-field bulk view;
subflames and effects last.

---

## 4. Tier 3 — distribution

**Type stubs (`.pyi`).** PyO3 generates none, so editors offer no
completion and no signatures — the single biggest quality-of-life win
per line of effort. Hand-written `pyfflame.pyi` shipped as package data,
with a test that every exported name appears in it so the two cannot
drift.

**CI wheel matrix.** `maturin-action` / `cibuildwheel` across
Windows / macOS / Linux, x86_64 + arm64. Already `abi3-py38`, so one
wheel per platform covers every Python ≥ 3.8. **See §5 for the Linux
risk before starting this.**

**PyPI.** Needs a name check (`pyfflame` availability), a licence field,
project URLs, and a long description. Trivial once wheels build.

**Docs.** The README covers usage. A short API reference — generated
from the docstrings already in `python/src/lib.rs` — would be worth it
once the surface stops moving.

---

## 5. Known risks

**Linux CI is the real one.** Because the wheel links the whole crate,
building it drags in `cpal`, which needs ALSA headers
(`libasound2-dev`) at *build* time, and the wgpu/winit stack wants X11
and Wayland dev packages. On `manylinux` those are not present by
default. Three outs, in order of preference:

1. Install the dev packages in the build container — a `before-all`
   line in the CI config. Almost certainly enough.
2. Build Linux wheels on plain Ubuntu runners rather than manylinux,
   accepting a narrower glibc floor.
3. Only if both fail: revisit feature-gating, scoped to making `cpal`
   and the window stack optional. This is the one scenario where the
   refactor earns its risk — and even then it should be driven by the
   app's own needs, not by the wheel.

**Wheel size if the linker stops being clever.** 2.6 MB today. If a
future change makes the GPU code reachable from a path Python touches
(anything that pulls in `renderer` or `gpu`), the wheel could balloon.
Worth a CI assertion on wheel size so the regression is loud.

**f32 storage** is documented in the README but will keep surprising
people: set `-0.1`, read back `-0.10000000149011612`. If it becomes a
support burden, consider rounding on read for display purposes only —
though quietly lying about the stored value is probably worse.

---

## 6. Tier 4 — nice-to-haves

Genuinely optional; listed so they are not lost.

- **Built-ins as plain Python functions.** `lsystem()`,
  `kleinian_generators()`, `hilbert3d_maps()`, `sphere_packing_mirrors()`
  and friends are currently reachable only through Rhai. Exposing them
  directly would let someone compute maps in Python and feed them to a
  flame. Cost: a second registration surface alongside the Rhai one,
  with the same drift risk `builtins.rs` already carries against its
  shader sources.
- **Animation.** `.anim` load/save and track editing — blocked on
  Phase 6, which adds animation tracks to the script object model.
  Should arrive through the same bindings, not a parallel API.
- **Batch helpers.** `run_script` over a seed range with
  `multiprocessing`, returning a list of `Config`s. Note `run_script`
  currently holds the GIL for its duration; the script budget bounds
  that, but `py.allow_threads` would let real threading work.
- **Notebook display.** `_repr_png_` on `Config`, rendering a small
  preview via the CLI so a flame shows inline in Jupyter. Cute, and
  probably the single best demo of the library.
- **numpy interop.** Affines as `(2, 3)` arrays, palettes as `(N, 3)`.
  Only worth it if a real workflow asks.
- **CLI entry point.** `python -m pyfflame convert a.flame b.fflame` —
  format conversion without writing a script. Small, and useful for
  people who only want the converter.
- **Palette generation.** Follows Phase 7 (colour-theory palettes),
  which is shared with the Palette UI. Same rule as animation: arrive
  through the shared implementation, not a Python-only one.

---

## 7. Explicitly not planned

- **GPU rendering in the wheel.** Rendering stays in the CLI exporter,
  called as a subprocess. Putting wgpu in a Python extension module
  means device management, driver variance and a much larger wheel, for
  a capability the subprocess already provides.
- **A pure-Python reimplementation** of any part of the model. The whole
  value of these bindings is that they are the *same* code as the app —
  the same serde, the same XML quirks, the same registry, the same RNG.
  Anything reimplemented is something that can silently disagree.

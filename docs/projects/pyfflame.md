# pyfflame — Python bindings

Status doc for the Python side. User-facing usage lives in
[`python/README.md`](../../python/README.md) and is not repeated here.

pyfflame was a PyO3 extension (Phase 5 of
[flame-scripting.md](../archive/projects/flame-scripting.md)): config
editing, `.flame` conversion and scripts, no rendering, a wheel per
platform. Since 2026-11-04 it is the engine built as **one WebAssembly
module for WASI** (`wasm/python`), which Python runs with wasmtime, its
GPU work done by wgpu-py. Same name, same API (the PyO3 package's tests
pass unchanged), plus rendering, the animation engine and video. The
PyO3 crate had stopped compiling unnoticed (it was in no gate) and is
gone.

---

## 1. What ships

- **`python/`** — the package: pure Python plus `pyfflame/fflame.wasm`.
  One wheel for every platform wasmtime and wgpu-py support.
  `python/build.py` builds the module and puts it in the package
  (`--wheel` builds the wheel).
- **`wasm/python/`** — the module's crate, a standalone workspace like
  the gallery crates (`cargo build --release` there targets
  `wasm32-wasip1` through its `.cargo/config.toml`).
- **The API.** `Config` (editing with registry checks, `.fflame` and
  `.flame` files), `run_script` (byte-identical to the CLI's `generate`),
  `variations()`, `Animation` (`.anim`), and new: `Engine.render`
  (stills; `sequence=True` carries a simulation's run between frames),
  `Image` (`save` writes the CLI's PNG with its metadata, `to_numpy`),
  `Engine.open_animation` (the video export's frames), `VideoWriter`
  (FFmpeg), presets, the script library.
- **Tests.** `python/tests`: the PyO3 package's suite, unchanged, and
  rendering (skipped without a GPU). Gates: `release.py check` compiles
  the module; `release.py gallery` builds it and runs the tests.

## 2. Design

**The engine builds for WASI.** WASI is wasm32 too, so the code split
on `target_arch = "wasm32"` meant two things at once: single-threaded
and async (which WASI is), and a web page with wasm-bindgen and web-sys
(which it is not). `build.rs` sets `browser` (wasm32 minus WASI) for the
second; only the 18 sites that call into the page say it. WASI has no
store and no network: `storage::backend`, `resources::fetch` and
`api::client` answer "not available" there, and Python does its own
files and fetching. Window, file-dialog, audio-device and platform-folder
crates are off for WASI in `Cargo.toml`; egui-wgpu assumes a browser on
wasm32, so the engine names `wgpu` directly (the same crate everywhere).

**Python to the module**: one export, `fflame_call`: a JSON request and
a block of bytes; back a status, bytes and a JSON note
(`wasm/python/src/lib.rs` lists the calls).

**The module to Python**: a wgpu custom backend
(`wasm/python/src/backend.rs`) turns each GPU call into a JSON command
for two imports, `fflame_gpu.call` / `take`; `python/pyfflame/_gpu.py`
makes it with wgpu-py. Objects are integer handles. Command buffers are
recorded in the module and replayed at submit (a Cahn-Hilliard render is
16,000 dispatches; recording drops a pipeline or bind group the pass
already has). Buffer maps and work-done callbacks run on the device's
poll, as in wgpu-core.

**Renders are the CLI's**: 1024 iterations per thread unless given, the
sticky superset off unless asked for.

## 3. Measured (2026-11-04, GTX 1660 SUPER, Windows)

- 254 visual-suite configs against the native CLI
  (`python/tools/compare_native.py`), none failing: every escape render
  (terrains, sky, the path-traced IFS solid) and all 107 simulations
  bit-identical, but the two perturbed deep zooms (~5% of pixels in
  chaotic regions; the CPU's libm is the likelier cause, unproven).
  Flames bit-identical at the same iterations per thread.
- A warm 1e9-iteration flame at 800x600: 507 ms (native CLI 585).
  Cahn-Hilliard: 351 ms against 388. A flame whose variation set is new
  costs a pipeline compile, 200-500 ms.
- Module 15 MB, ~0.4 s to load; wgpu-py's device ~1 s.

## 4. Copied code — to review after the merge

Copies made to get the module working without touching their sources.
Each should become one function of the engine's that every caller uses.

| Copy | Of | What differs |
| --- | --- | --- |
| `wasm/python/src/scripts.rs` (`with_host`, `resolve_params`, `list`, `source`, `params`, `run`) | `wasm/script/src/lib.rs` (`with_host`, `resolve_params`, `list_scripts_impl`, `script_source_impl`, `collect_params_impl`, `run_impl` + `run_chain_impl`) | `run` merges `run` and `run_chain`; results are `serde_json::Value`, not strings; a one-stage error carries no "stage 0:" |
| `wasm/python/src/render.rs`: `Session`, `Gpu::open`, `Gpu::frame`, `Session::frame_config` | `src/animation/export.rs`, `export_animation`: track binding (`remove_broken_tracks`, `bind_to_config`), `export_signals` / `tracks_missing_signals`, `set_step`, the per-frame evaluate / `apply_animation_values` / `moment_configs` / `RenderJob` with engines and moments, sticky off | Left out: `ExportRung` (render scale, upscaler, escape supersampling), the deep-zoom size check, audio files, the pixel scale, the motion-blur inlined-constants switch |
| `wasm/python/src/render.rs`: `renderer_for`, `Gpu::render` | `wasm/render/src/lib.rs`, `render_with_live` (a renderer kept per size, destroyed on resize) | Keyed on the engine family too (`for_job`'s tail); sticky set per call |
| `python/pyfflame/__init__.py`: `Config`'s editing (`add_variation`, `set_variation_param`, transforms by index, the properties) and its error messages | `python/src/lib.rs` (the PyO3 crate, deleted; in git history) | Python over the config's JSON instead of Rust over `FractalConfig`; parsing, absent-field defaults and the registry still come from the engine (`config_normalize`, `defaults`, `variations`) |
| `python/pyfflame/_gpu.py`, `op_init`: the device's limits and features | the native exports' device setup (`src/animation/export.rs`, `src/app/export.rs`): the adapter's buffer, binding and texture limits, `FLOAT32_FILTERABLE` | A parallel choice, not a line-for-line copy |
| `python/tools/compare_native.py`, `png_text` | `src/png_metadata.rs`, `read_png_metadata` (the tEXt chunks) | Python, reading only |
| WASI stubs in `src/resources/fetch.rs` (`mod wasi`) and `src/api/client.rs` (`mod wasi`) | the browser modules' signatures beside them | Not copies of behaviour, but signatures that must track the browser's |

## 5. Known gaps

- **The backend forwards what the 254 configs use.** Not forwarded:
  depth/stencil, vertex and index buffers, render bundles, timestamp
  queries, immediates. Error scopes always report none, so the
  engine's out-of-memory checks (`push_error_scope`) see no error; a
  validation error raises in Python instead.
- **wgpu-py quirks**: 0.32's `queue.on_submitted_work_done_sync` fails
  (a cffi callback signature), so the host waits with the private
  `device._poll_wait()`; `write_texture` needs `bytes_per_row` even for
  one row; WebGPU has no `clear_texture` (emulated by a copy from a
  zeroed buffer); wgpu's multisample mask is 64-bit, WebGPU's 32.
- **Audio-driven signal tracks** (an audio file) are not loaded;
  generator signals are.
- **A panic traps the module**; the Engine starts a new one and raises,
  and open animations are lost with the old.

---

## 6. Backlog (carried over, still open)

**Ergonomics.** A transform proxy (`c.transforms[0].weight = 0.5`,
revalidating its index on every access: a proxy is a view, and after
`remove_transform(0)` it addresses another transform); palette access
(stops, name, `.palette` files, the library); a bulk view of the ~60 flat
fields (`c.camera`, `c.tonemap`). `Config.data` reaches all of it raw
meanwhile.

**Model coverage** through `Config`: linked and final transforms,
post-affine, xaos, subflames, solo, post-symmetry, effects.

**Distribution.** Type stubs (a test that every exported name is in
them); PyPI (the name, a licence field, URLs); an API reference from the
docstrings.

**Nice to have.** Building an animation from Python (tracks and keys
without a script); batch helpers over seed ranges; `_repr_png_` for
notebooks (now a real render); numpy affines and palettes; `python -m
pyfflame convert a.flame b.fflame`.

**Not planned**: a pure-Python reimplementation of the model. The value
is that this is the app's code -- its serde, its XML quirks, its
registry, its RNG; anything reimplemented can silently disagree. The
`Config` editing helpers are the one thin layer in Python, over the
engine's JSON (section 4).

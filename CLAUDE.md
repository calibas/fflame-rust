# Fractal Flame Renderer - Project Context

## Overview

A GPU fractal renderer (Rust, wgpu/WGSL, egui) with three engines and
four render modes: **fractal flames** (`TwoD`, `ThreeD`), **escape-time
fractals** (`Escape`) and **simulations** (`Simulation`). This file is the
quick reference for AI assistants; the detail is in the docs below, which
were refreshed against the code on 2026-10-10 -- still verify against the
source when something looks off.

**Documentation:**
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) - system overview, module map, where to start for common changes
- [docs/main/](docs/main/) - the reference docs:
  - Flame engine: [RENDERER.md](docs/main/RENDERER.md) (pipeline, persistent orbits, governor, solid rendering), [SHADERS.md](docs/main/SHADERS.md) (shader assembly, sticky shaders, lints, cross-platform rules), [BUFFERS.md](docs/main/BUFFERS.md) (bindings, GPU structs), [TRANSFORMS.md](docs/main/TRANSFORMS.md) (the flame model, xaos, linked/final transforms), [VARIATIONS.md](docs/main/VARIATIONS.md) (registry, defs, features, plugins), [FOCUSED-RENDERING.md](docs/main/FOCUSED-RENDERING.md) (flame deep zoom)
  - Other engines: [ESCAPE.md](docs/main/ESCAPE.md) (escape-time), [SIMULATION.md](docs/main/SIMULATION.md) + [SIMULATION-CATALOG.md](docs/main/SIMULATION-CATALOG.md), [TERRAIN.md](docs/main/TERRAIN.md) (3D terrains, render tiers, path tracer, materials), [CAMERA.md](docs/main/CAMERA.md) (every camera)
  - Shared: [COLOR.md](docs/main/COLOR.md), [TONE-MAPPING.md](docs/main/TONE-MAPPING.md), [EFFECTS.md](docs/main/EFFECTS.md), [EXPORT.md](docs/main/EXPORT.md), [ANIMATION.md](docs/main/ANIMATION.md), [CONFIG.md](docs/main/CONFIG.md), [UI.md](docs/main/UI.md), [ONLINE.md](docs/main/ONLINE.md), [SCRIPTING.md](docs/main/SCRIPTING.md) (Rhai API reference, kept honest by a staleness test) + [SCRIPTING-GUIDE.md](docs/main/SCRIPTING-GUIDE.md), [PALETTE_LIBRARY.md](docs/main/PALETTE_LIBRARY.md), [FRACTAL-BROWSER.md](docs/main/FRACTAL-BROWSER.md) (the Fractal Browser), [I18N.md](docs/main/I18N.md)
- [docs/RELEASE.md](docs/RELEASE.md) (procedures, gates), [docs/TESTING-GUIDE.md](docs/TESTING-GUIDE.md), [docs/WASM.md](docs/WASM.md)
- [docs/projects/](docs/projects/) - work in progress and open lists ([README](docs/projects/README.md)); [docs/experimental/](docs/experimental/) - ideas; [docs/archive/](docs/archive/) - finished plans and history ([README](docs/archive/README.md)). Finished project docs go to `docs/archive/projects/` -- move them with `scripts/move_docs.py`, which rewrites references in code comments too.

Project history is in git: `git log --oneline`.

## Environment
- Shell within VSCode: Git Bash (MSYS2 MinGW64)
- Use forward slashes for paths
- Avoid `/dev/null` (creates literal files on Windows)
- **Use relative paths for file edits** (e.g., `src/ui/mod.rs` not `c:\projects\fflame-rust\src\ui\mod.rs`) - helps avoid "file unexpectedly modified" errors from IDE/linter race conditions
- One-shot codemod scripts go in `scripts/`, not temp folders outside the repo

## Quick Reference

### Engines and render modes
- **Four modes**: `TwoD`, `ThreeD` (flames: one `shaders/core/main_template.wgsl`, specialized by the RENDER_3D flag), `Escape` (`src/escape/`), `Simulation` (`src/sim/`). The escape and simulation generators hand `render_with` an `Rgba32Float` image in the flame accumulator's layout and share everything from the tonemap on (`FlameRenderer::for_tail` + `tonemap_pass_with_input`; effects, export, animation, scripting).
- **The render mode gates the UI.** `src/ui/render_mode.rs` is the only writer of `ConfigPath::RenderMode` (a source-scanning test enforces it) and owns what a mode change implies (`choose`: choosing something for an engine enters its mode in the same undo step; the mode's editor brought forward; engine lifetimes); `src/ui/visibility.rs` is the only answer to "does this panel/control act in this mode". **Add a case there, never a `matches!(render_mode, ...)` inside a panel.** One workspace serves every mode and every panel is drawn in every mode; one the mode does not use wears a banner saying so ([docs/projects/one-workspace.md](docs/projects/one-workspace.md)). (Not to be confused with `src/app/render_mode.rs`, the Normal/Animating/Overwrite state machine.)
- **Escape** ([ESCAPE.md](docs/main/ESCAPE.md)): FormulaDef/ColoringDef registries mirroring the variation system (inline WGSL, append-only, feature flags); a marker-splicing assembler; `EscapeRenderer` (direct and perturbed paths; deep zoom by perturbation with exact-decimal centres + `zoom_log2`, big-float references, BLA). Fragment modes: A (iterate & classify), B (fields), D (the flame as a distance field: the Flame Attractor and its 3D solid). The camera lens (`src/escape/lens.rs`) warps the view by any variation. Entering Escape or Simulation from a flame whose tonemap is Log switches it to Linear and resets exposure/gamma (`defaults::entry_tone_mapping`; flame presets carry Log-calibrated values that render Linear output invisibly); entering a lit 3D view (a solid, a terrain) sets the 3D look as well (`ui::render_mode::look_3d`: Linear, exposure 2, gamma 1, saturation 1, an untouched rig at strength 1 and ambient 0). Online, escape configs are stored AS FLAMES, told apart by `render_mode` alone.
- **Simulation** ([SIMULATION.md](docs/main/SIMULATION.md)): a grid of `vec4<f32>` cells stepped by a neighbour-coupled rule (31 models, 13 colourings), layers and couplings, textures. **The picture is the state of a run at a step count** -- `(model, params, seed, init, steps)` is in the config and PNG metadata, so a still is reproducible. The rule is not invertible: `sim.steps` is a CAP to the panel's transport and a TARGET to the animation timeline, and a backward step track is held rather than played.
- **3D beyond the flame** ([TERRAIN.md](docs/main/TERRAIN.md)): escape and simulation terrains (height fields over sections to the horizon), mode D solids, render tiers Lit / PathTraced / Auto, a path tracer with a denoiser and sky, one material and light rig. **Cameras** ([CAMERA.md](docs/main/CAMERA.md)): `src/camera/` is the one model behind all seven cameras (`view3d`, `chain`, `quat`, `gesture` per `ViewKind`, `fly`), each edit a `CameraEdit`.
- Dispatch for every engine lives inside `render_with` (`src/renderer/render.rs`), so CLI, thumbnails, video and the gallery inherit it; the app holds lazy `escape_renderer` / `escape_terrain` / `sim_renderer` / `sim_terrain`.

### Module map
- `src/app/` - the frame loop and event handling (mod.rs, ui_handlers.rs for `UiResponse`, gpu_updates.rs incl. `freeze_viewport`, input.rs, fly_camera.rs, export.rs, governor.rs + aa_governor.rs (the quality governor; its fallback without a GPU timer), config.rs, render_progress.rs, script_cloud.rs, variation/effect fetch)
- `src/config/` - ConfigManager + undo (manager.rs), ConfigPath/ConfigValue/UpdateType (delta.rs), FractalConfig + versioned migration (fractal_config.rs), EscapeConfig (escape.rs), SimConfig (sim.rs), defaults.rs, slider.rs
- `src/scene/` - the flame model (transforms.rs), xaos.rs, palettes, presets, randomize; the deep-zoom planners (cylinder, backward, plan_gpu, word_tree, final_map, importance) and IFS analysis (ifs_analysis, ifs_estimate, ifs_real)
- `src/variations/` - registry (mod.rs, definition.rs), 647 defs in `defs/*.rs`, inverse/bound/derive (planner inputs), docs corpus
- `src/renderer/` - FlameRenderer (compute_kernel.rs), the unified render API (render.rs), sticky.rs, shade_pass.rs, dof_pass.rs, effect_chain.rs, thumbnails
- `src/shader_builder_v2.rs`, `src/shader_cache.rs`, `src/gpu/` (buffers.rs, pipelines.rs), `shaders/` - see [SHADERS.md](docs/main/SHADERS.md)
- `src/escape/`, `src/sim/`, `src/camera/`, `src/textures/` (simulation recipes as textures)
- `src/export/` - high-res export (tiled GPU, CPU fallback), 2x AA (supersample.rs)
- `src/ui/` - dockable panels (egui_dock); `src/animation/`, `src/signal/`, `src/audio/`; `src/effects/`
- `src/script/` - Rhai scripting (api.rs registers every function; cli.rs; assets/scripts/)
- `src/api/` (online client), `src/storage/` (settings, backend, caches, catalogs, plugins, credentials), `src/resources/` (HTTP/filesystem fetching, lazy palette packs)
- `src/probe/`, `src/census/` - the variation math probe and reachability census; `src/contract.rs` (engine contract), `src/provenance.rs`, `src/shader_dumps.rs`
- `src/flame_xml.rs` (.flame import/export), `src/png_metadata.rs`, `src/wasm_api.rs`, `src/i18n.rs`
- `wasm/{render,script,flame,escape,sim}` - single-engine gallery modules; `python/` - pyfflame bindings

### The flame pipeline (each frame; [RENDERER.md](docs/main/RENDERER.md))
1. **Compute** - the chaos game; each plotted sample atomically adds into a per-pixel u32 histogram (R, G, B, density x `color_scale = 100`). Dispatch = workgroups x 64 threads x `iterations_per_thread` (default 256). **Orbits persist** between dispatches (orbit buffer, binding 8): only an edit to the dynamics (the orbit key: the flame, render mode, preserve_z, colour mode, speed factor) starts a new generation, never a pan or zoom -- except on a flame carrying variation STATE, which restarts with the accumulation (JWF re-inits variations per render). Burn-in is paid once per restart, and the plotted count comes from `OrbitFuses` -- every sample-counting caller must use what `compute_pass` returns. Bad-value recovery as flam3/JWF: X/Y divergence respawns; Z saturates at +/-1e32.
2. **Accumulate** (`shaders/accumulate.wgsl`) - folds the histogram into an `Rgba32Float` accumulator: rgb = density-weighted mean colour (cumulative by default, or a fixed EMA), a = raw cumulative hit count.
3. **Tonemap** (`shaders/tonemap.wgsl`) - Linear / Log (flam3 `k1`/`k2` normalised by `sample_density = total_iters / pixel_count`, so brightness is iteration-count invariant) / Density, then highlights, Levels, the curve LUT, background or transparency. [TONE-MAPPING.md](docs/main/TONE-MAPPING.md)

Between them: analytic blur, the solid shade pass and DoF (3D), density effects; colour effects after the tonemap. The frame-time **governor** (`app/mod.rs`: `Batch`, `Knee`) shortens the dispatch first (to 64 iterations a thread), then sheds workgroups; off in the Rendering menu (`frame_governor`, the Quality Governor), every frame dispatches `manual_workgroups` at full length. The same switch governs escape-time, terrains and simulations, a flame's render scale and its batch's pacing cap (`app/governor.rs`; [quality-governor.md](docs/projects/quality-governor.md)): a Target Frame Rate held while the picture plays or is edited.

### Focused Rendering (flame deep zoom; [FOCUSED-RENDERING.md](docs/main/FOCUSED-RENDERING.md))
Opt-in, 2D flames only (config `cylinder_targeting`; code says "cylinder targeting"). A **plan** enumerates the words -- sequences of transforms -- whose images reach the view; each sample runs the free orbit and plots its point forced through a word drawn from the plan, so samples land in frame at zooms where the plain chaos game lands almost none. Brightness stays unbiased (`Cylinders::iteration_scale`; off-probability words deposit the inverse of their draw rate). Two shader arms: composed (affine words) and replay (`shaders/core/replay.wgsl`; offsets from reference orbits past f32). Works for a narrow range of flames; **coverage work is paused** -- status and order of work in [docs/projects/deep-zoom/deep-zoom-tracker.md](docs/projects/deep-zoom/deep-zoom-tracker.md). Compare pictures by density, not the tone map (`read_density_blocking`, `compare_blocks`). Unrelated to the escape engine's perturbation deep zoom.

### Variations ([VARIATIONS.md](docs/main/VARIATIONS.md))
- 647 `static VariationDef`s in `src/variations/defs/*.rs` (flam3 / Apophysis / JWildfire), inline WGSL (2D + 3D), registered by appending to `defs/mod.rs` (**append-only**: registration order is the stable ID order). With the `engine-flame` feature off only `linear` is registered.
- `global_registry()` returns a read guard; the registry changes at runtime -- API-downloaded variations and local plugins register through `global_registry_mut()`, and so do **Rhai variations** (`variations::custom`, `Provenance::Custom`: a Formula Editor transformation `fn variation(z, p)`; the flame carries the source in `Flame::custom_variations` and `custom::ensure` registers it wherever a flame is read; `.flame` export drops them).
- Up to `MAX_VARIATIONS_PER_FLAME = 100` per flame, with a **per-flame local index map** (no global indices). The shader is the **sticky superset**: a canonical union of the flame's variations plus retained extras at weight 0, so edits rarely recompile (`src/renderer/sticky.rs`); chained pre/post order falls back to a specialized compile.
- Parameters: per-transform `HashMap` with `"varname.param"` keys, packed into `MAX_VARIATION_PARAM_SLOTS = 1600` floats (user params, then `wgsl_init`-derived); shaders read `get_param(xform_id, variation_id, slot)`. Per-thread state (`state_count`) is not in that buffer: it is a `var<private> thread_state` array, read via `get_state`/`set_state` and saved with the orbit in the orbit buffer. ParamType: Float, UnlimitedFloat, Integer, UnlimitedInteger, Angle, Boolean, Enum.
- UI ordering: by category, then registration order -- never HashMap order.

### Colour, palettes, tone
- **Colour modes**: Palette (Apophysis colour-coordinate evolution), Speed (distance per iteration), PathMap (colours by the word Focused Rendering draws through; needs it, 2D). [COLOR.md](docs/main/COLOR.md)
- **Palette packs** (`assets/palettes/packs/*.json`): **the folder is the catalog** -- desktop scans it at startup (`enabled_by_default` inside each pack); `starter_pack.json` is also embedded as the offline fallback (`BUILTIN_PACK_FILE`; both discovery paths skip that name); WASM discovers packs via a manifest `build.rs` generates (guarded by `generated_manifest_matches_packs_folder`) and fetches them from `pkg/assets/palettes/packs/`. Loose `assets/palettes/*.palette` files load into a "Local Files" pack. Clicking a library palette sets the config's palette to a copy; the editor's Save to Library writes `custom_palettes.json`. [PALETTE_LIBRARY.md](docs/main/PALETTE_LIBRARY.md)

### UI ([UI.md](docs/main/UI.md))
- Dockable panels (`PanelType` in `src/ui/workspace.rs`: 32), each in its own `src/ui/*.rs`; cross-panel results bubble through `UiResponse` (`src/ui/response.rs`) and are handled in `src/app/ui_handlers.rs`. Menus: File, Edit, View, Rendering, Window, Help, then the mode button (named by the current mode; `src/ui/menu_bar.rs`), the language (globe) menu on the right; `src/ui/compact_menu.rs` is the mobile equivalent. Both Window menus draw their rows from `visibility::WINDOW_MENU`.
- Input: one control set for every camera (`camera::gesture`): drag pans, Alt+drag or right-drag turns (2D rotates, 3D orbits), the wheel zooms toward the cursor (3D dollies; in fly mode it sets the speed). Touch: long press = right button, the viewport's Pan/Turn toggle = Alt. Fly mode (F2) in every 3D view: FreeLook or FPS. The table is in UI.md.
- i18n: rust-i18n, `locales/*.yml` (en, es, ja, zh-CN; the non-English locales cover a fraction of the keys and fall back to English). Desktop loads Noto Sans JP/SC at runtime for CJK; WASM has no CJK font. [I18N.md](docs/main/I18N.md)

### Config, state, storage ([CONFIG.md](docs/main/CONFIG.md))
- **All configuration changes flow through ConfigManager**: `ConfigPath` identifies every parameter; the `UpdateType` it returns drives selective GPU updates. Undo: delta-based, 500 states; rapid changes coalesce (500 ms inactivity, 3 s max span); camera gestures coalesce by description.
- `FractalConfig` (per fractal, undoable; versioned with `migrate_value`, now v6 -- `render_mode`, `perspective_strength`, `preserve_z` live on the config, not the flame) vs `SystemSettings` (device preferences, no undo, saved immediately via `update_system_setting()`): VSync/FPS, iterations per thread, burn-in, the governor and manual workgroups, orbit cache size, the video export's encoder, the viewport letterbox, fly-mode preferences, online credentials, compact mode. The picture's size (`image_size`, what the exports render at) is the config's.
- Storage (`src/storage/backend.rs`): Windows `%APPDATA%\Fractals for All\Fractal Art Editor\data\`, macOS/Linux via `directories`, WASM localStorage. Presets are one bundled file, `assets/presets.fflame`.

### GPU memory during exports ([EXPORT.md](docs/main/EXPORT.md))
A background export (PNG on its own device, high-res, video) **freezes the viewport** (`App::freeze_viewport`): nothing renders, its last picture stays, the escape renderer and terrains are freed and rebuilt after. Renderers hold only what they draw: the solid shade pass's textures are made on its first run; escape/terrain/sim renders get `FlameRenderer::for_tail`; a terrain's section renderer frees its pixels once the sections are in; on a discrete GPU an export fits its terrain section cap (64-256) to the card (`fit_export_sections`).

### Implementation details worth knowing
- **JWildfire/Apophysis is the gold standard**: when our rendering differs from theirs for the same `.flame`, ours is wrong (deliberate divergences -- world-space `cam_pos`, 3D pan semantics -- are documented in [CAMERA.md](docs/main/CAMERA.md) and code comments).
- **f32 vs Apophysis double**: WGSL has no f64, so weights and parameters are f32 (~7 digits); minimal impact on typical flames.
- GPU buffers: std430 (storage) and std140 (uniform); fields in `GpuParams` must start on 16-byte boundaries -- keep the explicit pad before `post_symmetry` in sync between `src/gpu/buffers.rs` and `shaders/core/header.wgsl`.
- Serialization is JSON; new config fields are skip-if-default so old files stay stable.
- Full WASM support, including export; WebGL is impossible (compute shaders).

## Build, Release, Test

```bash
cargo run --release                      # desktop app (binary: FractalArtEditor)
# CLI subcommands: export, export-animation, generate (Rhai scripts), list-encoders
cargo run --release -- export -i tests/visual/configs/2d -o output/visual-2d   # one folder (not recursive); the whole suite is run_tests.py
./build-wasm.sh                          # web app (or build-wasm.bat): --profile dist + wasm-bindgen, not wasm-pack
wasm-pack build --target web --release   # gallery modules only: in wasm/render, script, flame, escape, sim
```
CLI flags (`--width`, `-H`, `--supersample`, `--transparent`, `--premultiplied`, `--strip-metadata`, `--engine`, `--category`, ...) and PNG metadata (`png_metadata::read_png_metadata(&bytes)`) are in [EXPORT.md](docs/main/EXPORT.md); the JS API (`WasmApi.export_png`) in [WASM.md](docs/WASM.md). iOS/Android builds cross-compile but do not run (`docs/experimental/ios-native-app.md`, `android-native-app.md`); phones use the web app's compact layout.

### Releasing, and changing anything the API sees
**See [docs/RELEASE.md](docs/RELEASE.md).** Nothing runs on push or on a timer.
```bash
python scripts/release.py check      # every fast gate (versions agree, tests, wasm builds, contract, shader dumps, doc links, all five gallery crates' tests -- the single-engine ones are the engine feature matrix)
python scripts/release.py            # ...and a changelog preview
```
Two committed files are **generated**, and a stale one fails a test rather than failing silently -- regenerate deliberately and read the diff:
```bash
UPDATE_CONTRACT=1 cargo test --lib contract_is_current        # docs/generated/engine-contract.json
UPDATE_SHADER_DUMPS=1 cargo test --lib canonical_shader_dumps # tests/shader_dumps/
```
The API's corpora come from `cargo run --release --bin export_variations_json` / `export_scripts_json` / `export_effects_json` (written to `output/`, gitignored). **A new `Feature`, `VariationCategory` or `ParamType` does NOT move the contract's shape fingerprint** -- tell the API repository directly (RELEASE.md §3). Release builds use `cargo build --profile dist` (31% smaller; the profile name is recorded in every PNG).

### Testing ([docs/TESTING-GUIDE.md](docs/TESTING-GUIDE.md))
- `cargo test --release --lib` - ~1,900 tests; GPU tests skip without an adapter.
- `python tests/visual/run_tests.py` - the visual suite (~375 configs in 2d/3d/escape/sim/solid/tonemap/variations, tolerance compare). **It does not build**: run `cargo build --release` first, and note a running app locks the binary.
- Render configs for inspection without the binary: `INSPECT="a.fflame=a.png" INSPECT_SIZE=WxH cargo test --release --lib -- --ignored render_configs_for_inspection`.
- `python scripts/run_benchmarks.py --quick` - CPU + GPU benchmarks (its exact-hash tables are informational).
- `cargo run --release --bin variation_probe` - every variation's arithmetic at adversarial inputs in 2D and 3D (`docs/generated/variation-probe*.txt`); `variation_probe -- compare OLD NEW` finds cross-platform divergence by value class. The first run on a machine pays cold driver compiles (~4 min).

## Coding Guidelines

### GPU Code
- All shaders are **WGSL**; follow std140/std430 layout rules.
- **WASM**: `textureSample()` only from uniform control flow; use `textureLoad()` inside conditionals (browsers fail silently with black output where desktop drivers are lenient).
- **Parenthesize bitwise operators**: WGSL forbids `a * b ^ c`, `x & 1u == 1u`, a shift beside arithmetic and `&&` beside `||`. naga accepts them; Chrome's Tint refuses the whole shader. The `mixed_operators` lint (`shader_lint`) scans every `.wgsl`, every Rust raw string of WGSL and the assemblers' output.
- **The flame compute layout holds only the bindings its shader uses** (`gpu::pipelines::used_bindings`; `a_layout_holds_every_binding_its_shader_uses`). Browsers count a layout's storage buffers against `max_storage_buffers_per_shader_stage` whether the shader reads them or not -- a fixed layout of 11 broke every flame on a Chrome that allows 10. A new binding goes in `compute_layout_entries` / `compute_entries`; `a_flame_renders_within_a_browsers_storage_limit` renders at 8 and 10.
- **Metal runs shaders with fast-math ON** (wgpu never clears `fastMathEnabled`), so IEEE NaN/Inf rules do not hold there -- measured on an M2, `x != x` is false for a real NaN and `Inf/Inf` is 1.0:
  - Never detect non-finite values with a self-compare; use `!(abs(x) <= 1e32)`, the idiom bad-value recovery uses.
  - Never write a bare self-division (`x / x`): fast-math folds 0/0 and Inf/Inf to 1.0. Write the intended value and reproduce the reference's zero case (worked example: `cut_btree` in `defs/cut_simple.rs`). The `shader_lint` tests (`src/variations/mod.rs`) reject both, and subnormal literals, in every variation and `shaders/*.wgsl`.
  - **Metal's `atan2` is wrong at zero pairs**: same-sign zeros give pi/4, mixed-sign zeros give NaN. Use `ff_atan2` (`shaders/core/utilities.wgsl`, IEEE-exact for all four sign pairs) wherever a zero pair is reachable -- affine sums and FTZ can launder any zero's sign. When guarding, reproduce IEEE (`atan2(+0,-0) = +pi`, `atan2(-0,-0) = -pi`; read the sign with `bitcast<u32>`), never 0. Most raw `atan2` call sites are unaudited; `npolar` was found by bisection.
  - Trig of huge arguments (past ~2^24) is garbage on every platform, differently; do not guard it (`docs/accepted-divergences.txt`), but flag ports whose reachable inputs feed it.
  - Fix at the shader level, bit-identical under IEEE; disabling fast-math globally measured 1.4x-5.2x slower and only moved the failures.
- **Trust the shader compiler's CSE**: write clear code, don't hand-hoist ([analysis](docs/archive/optimization-attempt-2025-11-02/SHADER_COMPILER_CSE_ANALYSIS.md)).
- **Feature flags keep WGSL byte-identical when off** (SOLID, CYLINDER_*, ...): the shader dumps prove it.

### Rust Code
- `bytemuck::Pod` / `Zeroable` for GPU structs; vec3 needs 16-byte alignment -- add explicit padding and mirror it in the WGSL header.
- Prefer `Queue::write_buffer()` over buffer mapping for updates.
- Cargo features `web-app`, `engine-flame`, `engine-escape`, `engine-sim`, `terrain` (all default) gate whole engines for the gallery modules: new engine code needs its `#[cfg]`, and `release.py check` builds each combination.

### State Management
```rust
// 1. Single parameter change
if response.changed() {
    config_manager.update_param(path, value.into())?;
}
// 2. Batch update (one undo point, e.g. triangle editor, color pickers)
config_manager.update_batch(changes, "description".to_string())?;
```
Structural actions (transform add/delete, config import/export, preset loading) are intentionally outside ConfigManager. A brief overwrite window after parameter changes keeps real-time updates smooth before the accumulation resets.

## Common Tasks

### Adding a New Variation ([VARIATIONS.md](docs/main/VARIATIONS.md))
1. Create `src/variations/defs/<name>.rs` with a `static VariationDef` (recent examples: `defs/octapol.rs`, `defs/szubieta.rs`):
   ```rust
   pub static MYVAR: VariationDef = VariationDef {
       name: "myvar",
       aliases: &[],                       // foreign names for .flame import
       display_name: "My Var",
       category: VariationCategory::Plugin,
       phase: VariationPhase::Normal,      // or Pre / Post
       features: &[],                      // NeedsRng, NeedsTransform, NeedsAccum, WritesColor, WritesRgb, AlwaysZ, NeverZ, ...
       parameters: &[
           param!("power", "Power", float, 2.0, -10.0, 10.0, "Tooltip text."),
       ],
       init_param_count: 0,                // derived slots filled by wgsl_init
       wgsl_init: None,
       state_count: 0,                     // per-thread state slots
       wgsl_state_init: None,
       wgsl_2d: r#" fn variation_myvar(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> { ... } "#,
       wgsl_3d: r#" fn variation_myvar(p: vec3<f32>, ...) -> vec3<f32> { ... } "#,
   };
   ```
2. Register in `src/variations/defs/mod.rs`: `mod <name>;`, `pub use <name>::*;`, and append `&MYVAR` to the END of the registration list (append-only!).
3. Signature rules (enforced by the shader builder): parameters -> `xform_id: u32, variation_id: u32`, read via `get_param`; `NeedsRng` -> `rng: ptr<function, RngState>`; `NeedsAccum` -> `accum: vec2/3<f32>` right after `p` (for JWF variations that read or clobber `pVarTP`); weight-inside-the-formula JWF variations read `transforms[xform_id].variations[variation_id]` and pre-divide by it (the idisc pattern); helper functions are name-prefixed (`fn myvar_helper`).
4. Port faithfully from the JWF/Apo source (keep quirks; cite the source file in the module docs).
5. Foreign parameter units: a conversion pair in `flame_xml.rs` (`variation_param_from_xml` / `_to_xml`; see radial_blur).
6. **Z semantics**: if the JWF source writes `pVarTP.z` unconditionally (no `isPreserveZCoordinate()` gate), add `Feature::AlwaysZ`, or its z is zeroed under `preserve_z = false` (`scripts/audit_z_write_semantics.py` classifies from `output/variation-jwf-source/`).

### Other additions
- **An escape formula or colouring**: [ESCAPE.md](docs/main/ESCAPE.md). **A simulation model or colouring**: [SIMULATION.md](docs/main/SIMULATION.md). **An effect**: [EFFECTS.md](docs/main/EFFECTS.md). All registries are append-only.
- **A palette**: a pack JSON in `assets/palettes/packs/` (it appears on the next start), a loose `.palette` in `assets/palettes/`, or the Palette Editor's import/Save to Library.
- **A preset**: add the config to `assets/presets.fflame` (one JSON array; desktop reads it at runtime, the binary embeds it as the fallback).
- **Tone mapping**: `shaders/tonemap.wgsl`, `TonemapParams` in `src/gpu/buffers.rs`, controls in `src/ui/tone_mapping.rs`.
- **UI controls**: in the panel's own `src/ui/*.rs`, writing through `update_param` / `update_batch`; cross-panel results via `UiResponse`, handled in `src/app/ui_handlers.rs`; availability per mode in `src/ui/visibility.rs`.
- **3D flame presets**: set the config's `render_mode = RenderMode::ThreeD` and `perspective_strength` (0 orthographic; ~0.1-0.3 typical); give Z structure with 3D variations (zcone, flatten, hemisphere, zscale, ...) and per-transform `g` offsets (`xform.variations.insert("zcone".into(), 0.5)`); check with camera pitch/yaw or fly mode.

## Dependencies
See @Cargo.toml. Key: **wgpu 29.0** (desktop vulkan/metal/gles -- dx12 deliberately omitted; WASM webgpu only), **winit 0.30**, **egui 0.34** + **egui_dock 0.19**, **serde/serde_json**, **quick-xml** (.flame), **rust-i18n 3.1**, **image/png**, **bytemuck**, **cpal/symphonia/rustfft/ringbuf** (audio), **rayon** (CPU export paths), **ureq** (desktop HTTP), **rhai** + **rand_pcg** (scripting; the PCG stream is what `script + seed` means, so it is pinned).

## File Formats
- **.fflame** - JSON, the complete FractalConfig (see `src/config/fractal_config.rs`): everything needed for exact reproduction -- the flame, view and camera, rendering settings, colour and palette data, tone mapping, depth effects, and the escape/sim/terrain blocks. Versioned (`CURRENT_CONFIG_VERSION`, migrated on load); new fields are skip-if-default.
- **.flame** - Apophysis/JWildfire XML (`src/flame_xml.rs`): round-trips the camera (`cam_pitch/yaw`, the `cam_roll`->bank rename quirk, `rotate`->rotation, `cam_pos_x/y/z` + legacy `cam_zpos`), perspective, DoF, post-symmetry, xaos, palettes and variation parameters (with per-parameter unit conversions). Our own extensions (depth-density compensation, far-density fade) are `.fflame`-only.
- **.palette** / pack JSON - a palette's name and its colours (stops, or `indexed_colors` hex in most packs).

## Known Issues
- **macOS is not green**, both known and parked: `renderer::sticky::extras_alone_never_change_pixels` fails on Metal (dead variations change codegen; visually harmless; the fix is a tolerance compare), and 21 escape-time visual renders differ from Windows -- investigated, perceptually indistinguishable, next leads recorded in the escape design record.
- **Config v1 files from two windows** load with the wrong values, by decision: explicit Levels values saved 2026-01-15..05-12 (raw density, now multiples of the mean) and a DoF blur strength saved 2026-01-24..05-29 (now scaled by 0.1). A version stamp cannot pick those files out, so nothing migrates them; re-set them by hand (`docs/archive/projects/config-v2-migration.md`).
- **Focused Rendering's open gaps** (opt-in only): a forced sample is plotted by the free orbit's transform's opacity, not its word's last transform's; in Speed colour mode it wears the free orbit's colour; the GPU planner's f32 can drop children at 1e5-1e6 (C16).
- Density effects on the `HighResExporter` path are skipped above a ~4 GiB budget (~9,100^2, 52 B/px with the `Rgba32Float` density chain) to avoid OOM; below it they apply as in the app.
- No error handling for invalid `.fflame` or `.palette` imports.

## Optional/Future Features
- **Async export** for a custom-size 2D escape or simulation export (flame high-res and terrain exports already run in the background).
- **EXR/HDR export**; **layered compositing**; **denoising for flames** (the path tracer has one).
- **Orbital camera** from JWF `cam_*focus` (a 3D turn already orbits the target) and camera-path tours.
- The camera lens in every mode: planned in `docs/projects/camera-lens-everywhere.md`.

# Architecture

The system overview and the map to everything else. It says what the
parts are, how they connect, and which document holds the detail. It
does not repeat that detail: when a reference doc in [main/](main/)
covers a topic, this page links to it.

The program is a GPU fractal editor written in Rust on wgpu (WGSL
compute shaders) and egui. One library crate (`fractal_flame_wgpu`,
`src/lib.rs`) is built five ways: the desktop app, which is also a
headless CLI; the web app (WASM on WebGPU); the single-purpose gallery
modules in `wasm/`; and the Python bindings in `python/`.

---

## 1. Three engines, four render modes, one tail

`RenderMode` (`src/scene/transforms.rs`) has four values, served by
three generators that really are different algorithms:

| mode | generator | lives in | read |
| --- | --- | --- | --- |
| `TwoD` / `ThreeD` | the chaos game: iterate points, splat them into a histogram, accumulate | `src/renderer/`, `src/variations/`, `src/scene/` | [RENDERER.md](main/RENDERER.md), [TRANSFORMS.md](main/TRANSFORMS.md) |
| `Escape` | one compute pass that iterates every pixel to escape | `src/escape/` | [ESCAPE.md](main/ESCAPE.md) |
| `Simulation` | a grid of cells stepped by a neighbour-coupled rule | `src/sim/` | [SIMULATION.md](main/SIMULATION.md) |

**The tail is shared, and that is the point.** Every generator produces
an `Rgba32Float` image in the flame accumulator's layout (`rgb` =
colour, `a` = density or coverage). Everything after that belongs to the
flame renderer: density effects, the tone map, colour effects, readback,
PNG metadata.

```
 FLAME (TwoD / ThreeD)                ESCAPE                              SIMULATION
 FractalConfig.flame                  FractalConfig.escape                FractalConfig.sim
   |                                    |                                   |
 shader_builder_v2: template +        escape::assembler: one formula      sim::assembler: one model
 active variations' inline WGSL       (or field / IFS def) + one          + one colouring spliced
   |                                  colouring spliced into a template   into a template
 shader_cache: pipelines keyed          |                                   |
 by the generated WGSL                EscapeRenderer: one dispatch;       SimRenderer: ping-pong
   |                                  deep zoom by perturbation           texture array, one slice
 FlameRenderer::compute_pass          (reference.rs, bla.rs,              per layer, N steps per
 chaos game, persistent orbits        fixedpoint.rs)                      frame, then a colour pass
   -> atomic u32 histogram              |                                   |
 FlameRenderer::accumulate_pass       [terrain: footprint.rs ->           [terrain: sim/terrain.rs]
   -> Rgba32Float accumulator          terrain.rs, lit or path traced]      |
 [solid 3D: shade_pass, dof_pass]       |                                   |
   |                                    |                                   |
   +------------------------------------+-----------------------------------+
                                        |
                       Rgba32Float image, accumulator layout
                                        |
             density effects (renderer/effect_chain.rs, EffectChainRunner)
                                        |
             tone map (FlameRenderer::tonemap_pass / tonemap_pass_with_input,
                       shaders/tonemap.wgsl; transparency is a tonemap mode)
                                        |
             colour effects (EffectChainRunner)
                                        |
             viewport  |  readback -> PNG + metadata  |  video frame  |  thumbnail
```

`render_with` in `src/renderer/render.rs` is where a non-interactive
render picks its generator. CLI export, thumbnails, the Fractal Browser,
video export and the gallery modules all go through it, so they render
every engine the build has. The tail's stages are shared, but the
sequence that calls them is written out per generator: the flame path
in `render_with`, `render_sim`, and `escape_tail` (shared by the escape
picture and its terrain), plus the app's own frame loop. A new tail
stage must be added to each.

Each engine is a Cargo feature (`engine-flame`, `engine-escape`,
`engine-sim`), all on by default; see [section 5](#5-cargo-features-and-build-targets).

**3D views beyond the flame.** A 3D flame can render as a lit solid
(`solid_strength`, `renderer/shade_pass.rs`, `renderer/dof_pass.rs`).
The escape engine also renders a flame as a distance field ("mode D",
`escape/ifs.rs`, path tracer in `escape/renderer/solid_path.rs`). An
escape or simulation picture can be shown as a lit height field, the
terrain view (feature `terrain`; `escape/footprint.rs`,
`escape/terrain.rs`, `escape/terrain_tiers.rs`, `escape/path_core.rs`,
`sim/terrain.rs`). See [TERRAIN.md](main/TERRAIN.md).

---

## 2. Module map

From the module list in `src/lib.rs`. Gating is noted where it applies.

### Entry points

| path | what it is | read |
| --- | --- | --- |
| `src/main.rs` | The `FractalArtEditor` binary. With no subcommand it launches the GUI (`desktop_main`). Subcommands: `generate` (run a Rhai script to a `.fflame`), `export` (headless PNG, one file or a directory), `export-animation` (video through ffmpeg), `list-encoders` | [EXPORT.md](main/EXPORT.md) |
| `src/lib.rs` | Library root: module list, `desktop_main`, `export_mode`, `export_animation_mode`, the WASM entry point `wasm_main`, and the `prelude` | |
| `src/wasm_api.rs` | `WasmApi`, the JavaScript API of the web app (config load, progress, `export_png`, online calls), plus a `PlanBench` for timing deep-zoom planning in a browser. wasm32 + `web-app` only | [WASM.md](WASM.md) |
| `src/web_clipboard.rs`, `src/web_text_agent.rs` | Browser clipboard bridge and virtual-keyboard bridge for egui. wasm32 only | |
| `src/bin/` | Tools: `variation_probe` (variation math probe and census), `export_variations_json`, `export_scripts_json`, `export_effects_json` (API corpora), `compare_images`, `shader_bench`, `simple_benchmark`, `census_dumpseed`, `test_curve` | [TESTING-GUIDE.md](TESTING-GUIDE.md) |

### Application and UI (feature `web-app`)

| path | what it is | read |
| --- | --- | --- |
| `src/app/` | The running editor. `mod.rs` (about 4,800 lines) holds `App`, the winit event loop, the frame (`App::render`) and the frame governor (`Batch`, `Knee`, `adjust_iter_scale`). Beside it: `ui_handlers.rs` (acting on `UiResponse`), `gpu_updates.rs` (applying `UpdateAction` to the GPU, the overwrite window, `freeze_viewport`), `config.rs` (load/import/export, undo/redo glue, background export launch), `export.rs` (headless export and `ExportEngine`), `input.rs` (keyboard, fullscreen), `fly_camera.rs`, `animation_update.rs`, `governor.rs` (the quality governor: escape-time's, terrains', simulations' rungs, a flame's render scale and its batch's pacing cap), `aa_governor.rs` (its fallback without a GPU timer), `render_mode.rs` (the Normal/Animating/Overwrite state machine; not the render-mode policy), `render_progress.rs`, `texture_sync.rs`, `variation_fetch.rs`, `effect_fetch.rs`, `script_cloud.rs` | [UI.md](main/UI.md), [RENDERER.md](main/RENDERER.md) |
| `src/ui/` | 51 files: the egui_dock workspace and every panel. Coordination: `mod.rs`, `workspace.rs` (`PanelType`, 30 panels; `WorkspaceLayout`), `panel_viewer.rs`, `menu_bar.rs`, `compact_menu.rs`, `response.rs` (`UiResponse`). Policy: `render_mode.rs`, `visibility.rs`. Panels include `transforms`, `triangle_editor`, `view` and `view_controls`, `tone_mapping`, `palette_editor`, `palette_library`, `fractal_browser` (with `fractal_gallery`), `escape_panel`, `sim_panel`, `textures_panel`, `lighting_panel` (Lighting), `effects_panel`, `animation_panel`, `track_editor`, `signal_panel`, `scripts_panel`, `paths_panel`, `xaos_editor`, `subflames`, `variations`, `random_generator`, `undo_history`, `export_panel` | [UI.md](main/UI.md), [FRACTAL-BROWSER.md](main/FRACTAL-BROWSER.md) |
| `src/i18n.rs` | rust-i18n setup and locale switching; strings in `locales/*.yml` | [I18N.md](main/I18N.md) |

### Model and state

| path | what it is | read |
| --- | --- | --- |
| `src/config/` | `FractalConfig` (`fractal_config.rs`), with `EscapeConfig` (`escape.rs`) and `SimConfig` (`sim.rs`) inside it; `ConfigManager` (`manager.rs`); `ConfigPath`, `ConfigValue`, `UpdateType` (`delta.rs`); defaults; the f32 precision policy (`precision.rs`); slider widgets (`slider.rs`, `web-app` only) | [CONFIG.md](main/CONFIG.md) |
| `src/scene/` | The flame model: `Flame`, `Transform`, `RenderMode` (`transforms.rs`); palettes and `ColorMode` (Palette, Speed, PathMap) (`palette.rs`); tone-map types and presets; xaos; the preset library (`presets.rs`, from `assets/presets.fflame`); random flames (`randomize.rs`). Also the Focused Rendering planners: `cylinder.rs`, `backward.rs`, `plan_gpu.rs`, `word_tree.rs`, `final_map.rs`, `importance.rs`, `slice.rs`, and the IFS analysis they rest on (`ifs_analysis.rs`, `ifs_estimate.rs`, `ifs_ball.rs`, `ifs_real.rs`, `mobius.rs`, `forward_delta.rs`) | [TRANSFORMS.md](main/TRANSFORMS.md), [COLOR.md](main/COLOR.md), [FOCUSED-RENDERING.md](main/FOCUSED-RENDERING.md) |
| `src/camera/` | Every camera behind one interface: `quat`, `chain`, `view3d`, `gesture`, `fly`. Returns a `CameraEdit` the caller applies | [CAMERA.md](main/CAMERA.md) |

### Flame engine

| path | what it is | read |
| --- | --- | --- |
| `src/variations/` | The variation registry (`mod.rs`, `global_registry()`), `VariationDef` and `Feature` (`definition.rs`), 647 definitions with inline WGSL in `defs/*.rs` (registered in `defs/mod.rs`'s `ALL_VARIATIONS`, append-only), bounds and inverses for the planners (`bound.rs`, `inverse.rs`, `derive.rs`), `analytic_blur.rs`, `docs.rs`. With `engine-flame` off the registry holds `linear` alone | [VARIATIONS.md](main/VARIATIONS.md) |
| `src/shader_builder_v2.rs` | Assembles a flame's WGSL from `shaders/core/` templates and the active variations, with a per-flame local index map and template flags | [SHADERS.md](main/SHADERS.md) |
| `src/shader_cache.rs` | Compiled pipelines keyed by the WGSL that produced them, each with a bind-group layout holding only the bindings its shader uses (`gpu::pipelines::used_bindings`) | [SHADERS.md](main/SHADERS.md) |
| `src/renderer/` | `FlameRenderer` (`compute_kernel.rs`: compute, accumulate and tonemap passes, targeting state); the unified render API (`render.rs`); `effect_chain.rs`; `shade_pass.rs` and `dof_pass.rs` (solid 3D); `sticky.rs` (sticky variation superset, to avoid recompiles); `thumbnail.rs`; `histogram.rs` (density histogram for the UI); `density_stats.rs` (attractor bounds for shadow fit). Test-only: `density_gates.rs`, `orbit_gates.rs` | [RENDERER.md](main/RENDERER.md) |
| `src/gpu/` | `GpuContext` (`device.rs`), `FlamePipelines` and the compute layout (`pipelines.rs`), `FlameBuffers` and the GPU structs (`buffers.rs`) | [BUFFERS.md](main/BUFFERS.md) |

### Escape and simulation engines

| path | what it is | read |
| --- | --- | --- |
| `src/escape/` (`engine-escape`) | `FormulaDef` and `ColoringDef` registries (`formulas.rs`, `colorings.rs`, lists in `mod.rs`), fields (`fields.rs`), the flame as a distance field (`ifs.rs`), `assembler.rs`, `EscapeRenderer` (`renderer.rs`), deep zoom (`reference.rs`, `bla.rs`, `fixedpoint.rs`, `bigfloat.rs`, `nucleus.rs`, `orbit_store.rs` on desktop), the camera lens (`lens.rs`), presets, and the terrain files (feature `terrain`) | [ESCAPE.md](main/ESCAPE.md), [TERRAIN.md](main/TERRAIN.md) |
| `src/sim/` (`engine-sim`) | `ModelDef` and `SimColoringDef` registries (`models.rs`, `colorings.rs`, lists in `mod.rs`), `assembler.rs`, `SimRenderer` (`renderer.rs`), GPU FFT for exact convolutions (`fft.rs`, `spectral.rs`), `terrain.rs` | [SIMULATION.md](main/SIMULATION.md), [SIMULATION-CATALOG.md](main/SIMULATION-CATALOG.md) |
| `src/textures/` | Simulation configs used as textures by escape configs; the shipped set is embedded by `build.rs` from `assets/textures/`, the generated images cached | [ESCAPE.md](main/ESCAPE.md) |

### Shared rendering, export and media

| path | what it is | read |
| --- | --- | --- |
| `src/effects/` | `EffectRegistry`, the built-in effects (`register_builtin_effects`), density and colour chains; WGSL in `shaders/effects/` | [EFFECTS.md](main/EFFECTS.md) |
| `src/export/` | High-resolution export: `pick_strategy` (`Direct`, `ParallelTiles`, `SerialTiles`), `HighResExporter` (`high_res.rs`, tiled and CPU-histogram paths), the sample-scatter pass (`accumulate.rs`), 2x supersampling (`supersample.rs`) | [EXPORT.md](main/EXPORT.md) |
| `src/png_metadata.rs` | Writing and reading the tEXt chunks every exported PNG carries | [EXPORT.md](main/EXPORT.md) |
| `src/animation/` | `Animation`, `Track`, keyframes and interpolation; `AnimationController` (playback); `export.rs`, the one video loop | [ANIMATION.md](main/ANIMATION.md) |
| `src/signal/` | `SignalManager`, the `.signal` format, generators; inputs for animation tracks | [ANIMATION.md](main/ANIMATION.md) |
| `src/audio/` | Decoding (symphonia), analysis (rustfft), playback and live capture (cpal on desktop, Web Audio in the browser); produces signals | [ANIMATION.md](main/ANIMATION.md) |
| `src/flame_xml.rs` | Apophysis / JWildfire / Chaotica `.flame` XML import and export | [CONFIG.md](main/CONFIG.md) |

### Content, persistence and the network

| path | what it is | read |
| --- | --- | --- |
| `src/storage/` | `SystemSettings`, the storage backend (files on desktop, localStorage in the browser), custom palettes, the thumbnail cache, caches and catalogs of API-loaded variations and effects, local plugins, encrypted credentials (desktop) | [CONFIG.md](main/CONFIG.md), [ONLINE.md](main/ONLINE.md) |
| `src/resources/` | Fetching shipped and remote resources on both platforms: palette packs (`palettes.rs`), presets, where `assets/` lives at runtime (`bundled.rs`) | [PALETTE_LIBRARY.md](main/PALETTE_LIBRARY.md) |
| `src/api/` | The online API client: auth, HTTP client, health, `FractalConfig` to API conversion (`sync.rs`), wire types | [ONLINE.md](main/ONLINE.md) |
| `src/provenance.rs` | Where a variation or effect came from: `Builtin`, `Api { version }`, `Local` | [ONLINE.md](main/ONLINE.md) |
| `src/contract.rs` | Generates the client/API contract, `docs/generated/engine-contract.json` | [RELEASE.md](RELEASE.md) |
| `src/script/` | Sandboxed Rhai scripting: the host and seeded RNG (`host.rs`), the registered API (`api.rs`, `builtins.rs`, `color.rs`, `anim.rs`), the script library and store, the `generate` CLI (`cli.rs`) | [SCRIPTING.md](main/SCRIPTING.md), [SCRIPTING-GUIDE.md](main/SCRIPTING-GUIDE.md) |

### Verification and diagnostics

| path | what it is | read |
| --- | --- | --- |
| `src/probe/` | The variation math probe: every variation's shader math at fixed adversarial inputs, compared across platforms by class | [TESTING-GUIDE.md](TESTING-GUIDE.md) |
| `src/census/` | Which input classes real renders feed each variation; run through `variation_probe` | [TESTING-GUIDE.md](TESTING-GUIDE.md) |
| `src/shader_dumps.rs` | Golden dumps of the generated flame WGSL in `tests/shader_dumps/` (test-only) | [SHADERS.md](main/SHADERS.md) |
| `src/sim_microbench.rs` | Stencil-step cost measurement (test-only) | |
| `src/profiler.rs`, `src/util.rs`, `src/version.rs` | GPU timestamp and CPU scope profiling; frame metrics; build information captured by `build.rs` | |

`src/undo.rs` is in the tree but not in `src/lib.rs`'s module list, so
it is not compiled. Undo lives in `ConfigManager`.

### Outside `src/`

| path | what it is |
| --- | --- |
| `shaders/` | `core/` (the flame template and its helpers: `header`, `main_template`, `utilities`, `rng`, `affine`, `affine_3d`, `accumulate_samples`, `noise`, `voronoi`, `complex`, `fractwf`, `polychora`, `polyhedra`, `su_mobius`, `subflame`, and for Focused Rendering `replay`, `replay_delta`, `plan_eval`, `plan_gather`, `plan_offsets`); `accumulate`, `accumulate_tiled`, `tonemap`, `downsample`, `histogram_blur`, `blur_convolve`, `blur_upscale`; solid shading `shade`, `normals`, `atrous`, `dof`; `effects/{density,color,common}/`. Variation, escape and simulation WGSL is inline in Rust |
| `assets/` | `presets.fflame`, `palettes/` (packs and loose files), `scripts/`, `textures/`, `fonts/`, `branding/`, `plugins-example/` |
| `build.rs` | Build provenance (version, git hash, profile), the WASM palette manifest, embedded texture presets, asset and shader copies into the target directory, the Windows icon |
| `wasm/` | Gallery crates `script`, `render`, `flame`, `escape`, `sim`; see [wasm/README.md](../wasm/README.md) |
| `python/` | `pyfflame`, PyO3 bindings built with maturin: config editing, `.flame` conversion, scripts; no rendering |
| `tests/` | `regression.rs`, `shader_dumps/`, `test_configs/`, and `visual/` (374 configs in `2d`, `3d`, `escape`, `sim`, `solid`, `tonemap`, `variations`) |
| `benches/` | `flame_bench.rs` (Criterion) |
| `scripts/` | `release.py`, `run_benchmarks.py`, `check_doc_links.py`, audits and one-shot codemods |

---

## 3. Cross-cutting systems

### Registries

Variations, escape formulas and colourings, simulation models and
colourings, and effects all follow one pattern: a `static` definition
per entry carrying its WGSL inline and its parameters described well
enough for the UI to build controls, collected in an ordered,
append-only list and looked up by name. The shader for a frame is
assembled from the entries it uses. Variations and effects can also
arrive at runtime from the online API or from local plugins
(`storage::load_installed_resources`, called by every entry point that
renders), which is why `global_registry()` and `global_effect_registry()`
return lock guards.

### Configuration and undo

All editable state is one `FractalConfig`, serialised as JSON
(`.fflame`). Device preferences are `SystemSettings`, which has no undo.
`ConfigManager` owns both.

- A UI control writes through `update_param(ConfigPath, ConfigValue)` or
  `update_batch(...)` (one undo step for several paths). `ConfigPath` in
  `src/config/delta.rs` names every editable parameter (several hundred
  variants) and knows its history label, its animation key and its
  `UpdateType`.
- `UpdateType` says how much work a change costs: `None`, `ViewOnly`,
  `ToneMappingOnly`, `ShadingOnly`, `ColorOnly`, `IterationReset`,
  `EscapeRerender`, `SimRerender`, `SimResample`, `SimReseed`. Several
  merge to the strongest. The app collects pending changes as an
  `UpdateAction` and applies them in `app/gpu_updates.rs`.
- History holds 500 states. Rapid edits coalesce: a new entry after
  500 ms without changes, or after a 3 s span.
- Structural actions (adding or deleting transforms, loading a preset
  or file) go through `apply_config_edit` / `apply_structural_change`.

Detail: [CONFIG.md](main/CONFIG.md).

### Render-mode policy

The mode decides which panels, menu items and controls exist.

- `src/ui/render_mode.rs` is the only writer of
  `ConfigPath::RenderMode` (a source-scanning test enforces it). It
  maps each mode to a workspace layout and decides which engines stay
  resident.
- `src/ui/visibility.rs` answers "is this available in this mode" for
  every panel × mode and control × mode. Both Window menus draw their
  rows from `visibility::WINDOW_MENU`. Add a case there, never a
  `matches!(render_mode, ...)` inside a panel.

`src/app/render_mode.rs` is unrelated despite the name: it is the
Normal / Animating / Overwrite state machine of the frame loop.

Detail: [UI.md](main/UI.md).

### UI organisation

- All UI is dockable panels (egui_dock). `PanelType` in
  `src/ui/workspace.rs` defines 32 panels; `WorkspaceLayout` holds the
  presets (Standard, Flames, Escape Time, Simulation, Animation,
  Scripting, Compact), none applied by a mode change. One workspace
  serves every mode: in Standard, Transforms, Escape and Simulation are
  left-dock tabs; a mode change brings its editor forward; every panel is
  drawn in every mode, one the mode does not use saying so at its top
  (`docs/projects/one-workspace.md`). Nothing about the dock tree is
  written to disk.
- Menu bar: File, Edit, View, Rendering, Window, Help, then the mode
  button (named by the current mode; its menu built from
  `RenderMode::ALL`), plus a language selector.
  `src/ui/compact_menu.rs` is the small-screen equivalent.
- Panels return a `UiResponse` (`src/ui/response.rs`) for actions that
  are not config edits (file I/O, palette library, transport, exports);
  `src/app/ui_handlers.rs` acts on it.
- Translations: English is complete; es, ja and zh-CN are partial
  ([I18N.md](main/I18N.md)).

### Cameras

The app has seven cameras (`ViewKind` in `src/camera/gesture.rs`): the
2D flame view, the 3D flame camera, the escape plane, the escape solid
(mode D), the escape terrain, the simulation view and the simulation
terrain. Each keeps its own config fields. `src/camera/` is the one
place that knows what they mean: viewport gestures, the View panel and
fly mode call into it and get back a `CameraEdit` (config writes plus a
history label) to apply through `ConfigManager`, so undo, animation and
scripts see ordinary edits. Detail: [CAMERA.md](main/CAMERA.md).

### The unified render API

`src/renderer/render.rs`:

- `RenderJob` describes a render (config, size, iteration target,
  iterations per thread, transparency, optional engines).
- `render` builds a `FlameRenderer` per call; `render_with` reuses a
  caller's renderer, which matters on WebGPU where dropped GPU memory
  lingers until garbage collection.
- `RenderEngines` is engine state a looping caller keeps alive between
  renders (the simulation's field, the escape renderer's reference
  orbit, the terrains), so a video continues a simulation instead of
  reseeding it every frame.

Users: `app/export.rs` (CLI and headless export), `renderer/thumbnail.rs`,
`animation/export.rs`, `wasm/render` and its siblings. The interactive
viewport does not use it; `App::render` drives the renderers directly.

### Export

- Still PNG: rendered afresh at the picture's size (`image_size`, the View
  panel's Output Size, fitted to the export's budget), 2x supersampling
  included, through the custom-size path; Export Current Image reads back
  the live renderer instead. Above the
  storage-binding limit `pick_strategy` routes to tiled rendering or a
  CPU histogram (`src/export/`). `--engine` on the CLI forces a path.
  Transparency is a mode of the tonemap shader (`transparent_mode`), with
  straight or premultiplied alpha.
- Every PNG embeds build information, render settings and the complete
  config (`src/png_metadata.rs`), so an image reproduces itself.
- **Video** goes through one loop, `animation::export::export_animation`,
  shared by the app's dialog and the CLI and built on `render_with`. Add
  a stage to the render tail and video gets it; do not add a second loop
  (the two that existed until 2026-09-09 disagreed about density
  effects, depth of field and which engine was running).
- While an export runs in the background, the viewport freezes (below).

Detail: [EXPORT.md](main/EXPORT.md).

### Persistence and storage

| what | where |
| --- | --- |
| Device settings, custom palettes, user scripts, thumbnails, cached downloads | `src/storage/`: on desktop the platform data directory (`ProjectDirs` "Fractals for All" / "Fractal Art Editor"), in the browser localStorage |
| Shipped content | `assets/`, embedded or copied by `build.rs`; located at runtime by `resources::bundled` |
| Palette packs | the pack folder is the catalog on desktop; WASM uses a manifest `build.rs` generates ([PALETTE_LIBRARY.md](main/PALETTE_LIBRARY.md)) |
| Deep-zoom reference orbits | `escape/orbit_store.rs` (desktop only) |

### Online

`src/api/` talks to the Fractal Flame API: flames (escape and simulation
configs are stored as flames, distinguished by `render_mode`),
animations, palettes, scripts, and the variation and effect catalogs.
A flame that names a variation or effect this build lacks triggers a
fetch (`app/variation_fetch.rs`, `app/effect_fetch.rs`); the downloads
are cached in `src/storage/`. The Scripts panel's cloud library is
`app/script_cloud.rs`. `src/contract.rs` keeps the generated contract
the API checks against. Detail: [ONLINE.md](main/ONLINE.md); release
procedure in [RELEASE.md](RELEASE.md).

### Scripting

Rhai scripts generate a config (generators) or change a copy of the
current one (modifiers), under execution budgets and with a pinned PRNG,
so script plus seed reproduces a flame byte for byte on desktop, web and
Python. Scripts may declare animation. They run from the Scripts panel,
the `generate` subcommand, `wasm/script` and `pyfflame.run_script`.
Reference: [SCRIPTING.md](main/SCRIPTING.md); language guide:
[SCRIPTING-GUIDE.md](main/SCRIPTING-GUIDE.md).

### Animation, signals and audio

Tracks bind config parameters to keyframes or to signals (generators,
audio analysis). During playback the controller writes the config
without creating undo entries. For simulations the step count is a
target the timeline drives. Detail: [ANIMATION.md](main/ANIMATION.md).

---

## 4. The frame

`App::render` in `src/app/mod.rs`, once per redraw:

1. **UI first.** egui runs and shows the previous frame's picture.
2. **Responses and config updates.** `UiResponse` handling, animation
   playback, then `process_gpu_updates` applies the frame's
   `UpdateAction` to the GPU.
3. **Render.** One read of the final config, then the active engine:
   flame compute, accumulate and tonemap; or an escape re-render when
   `escape_dirty`; or simulation steps. Then present.

**Event-driven.** The loop sleeps (`ControlFlow::Wait`) unless something
needs frames: an unfinished render, recent input, animation or audio, a
running simulation, fly-mode movement, or an export's progress bar
(redrawn ten times a second).

**Persistent orbits.** Each flame thread's orbit survives between
dispatches, so burn-in is paid once per restart and the picture does
not depend on how work is sliced ([RENDERER.md](main/RENDERER.md)).

**The frame governor** sizes each flame dispatch to the frame budget:
it shortens iterations per thread first (to 64, or three burn-ins),
then sheds workgroups, down to one workgroup (`Batch`, `Knee`,
`adjust_iter_scale` in `src/app/mod.rs`). The Rendering menu turns it
off (`frame_governor`), and the dispatch is then `manual_workgroups` at
full length. The same switch, the Quality Governor, governs escape-time,
terrains and simulations while they play or are edited
(`src/app/governor.rs`).

**The overwrite window.** After a view, palette or flame edit the
accumulator is overwritten rather than blended, and this continues for
100 ms after the last change so a drag shows no blank frames; then the
iteration counter resets for a clean rebuild (`src/app/gpu_updates.rs`).
Escape mode has no accumulator and skips this.

**Frozen during exports.** While a background export runs,
`freeze_viewport` stops viewport rendering, keeps the last picture on
screen, and releases the escape renderer and terrains' GPU memory,
since the export competes for the same GPU.

---

## 5. Cargo features and build targets

| feature | default | gates |
| --- | --- | --- |
| `web-app` | on | `mod app`, `mod ui`, the editor's wasm-bindgen exports (`wasm_main`, `WasmApi`) |
| `engine-flame` | on | the variation catalog; off, the registry holds `linear` alone |
| `engine-escape` | on | `src/escape/` |
| `engine-sim` | on | `src/sim/` |
| `terrain` | on | the terrain view; implies `engine-escape` |

With an engine off, `render_with` returns `RenderError::EngineMissing`
for configs in that mode. `python scripts/release.py check` compiles the
library with each single engine (`--no-default-features --features
engine-X`), because a `#[cfg]` on the wrong item compiles in every other
combination.

| target | how | notes |
| --- | --- | --- |
| Desktop app and CLI | `cargo run --release`; release builds use `cargo build --profile dist` | wgpu backends: Vulkan, Metal, GLES (no DX12) |
| Web app | `./build-wasm.sh` (or `.bat`): `--profile dist` plus wasm-bindgen | WebGPU only; [WASM.md](WASM.md) |
| Gallery modules | `wasm-pack build --target web --release` in `wasm/script`, `wasm/render`, `wasm/flame`, `wasm/escape`, `wasm/sim` | each depends on the library with `default-features = false`; [wasm/README.md](../wasm/README.md) |
| Python | maturin in `python/` | standalone crate, default features; [projects/pyfflame.md](projects/pyfflame.md) |

Profiles in `Cargo.toml`: `release`; `dist` (fat LTO, stripped,
`panic = "abort"`); `dist-symbols` (dist with names kept); `dist-debug`
(assertions and overflow checks). The profile name is recorded in every
exported PNG. Release procedure: [RELEASE.md](RELEASE.md).

---

## 6. Where to start for common changes

| change | files | read |
| --- | --- | --- |
| Add a variation | new `src/variations/defs/<name>.rs`; register at the end of `ALL_VARIATIONS` in `src/variations/defs/mod.rs` (append-only); unit conversions in `src/flame_xml.rs` if the foreign app differs | [VARIATIONS.md](main/VARIATIONS.md) |
| Add an escape formula or colouring | `src/escape/formulas.rs` or `colorings.rs`; append to `FORMULAS` / `COLORINGS` in `src/escape/mod.rs` | [ESCAPE.md](main/ESCAPE.md) |
| Add a simulation model or colouring | `src/sim/models.rs` or `colorings.rs`; append to `MODELS` / `COLORINGS` in `src/sim/mod.rs` | [SIMULATION.md](main/SIMULATION.md) |
| Add an effect | WGSL in `shaders/effects/density/` or `color/`; register in `register_builtin_effects` in `src/effects/mod.rs`; regenerate with `export_effects_json` | [EFFECTS.md](main/EFFECTS.md) |
| Add a panel | `src/ui/<panel>.rs`; a `PanelType` in `src/ui/workspace.rs`; dispatch in `src/ui/panel_viewer.rs`; availability in `visibility::panel` and a `WINDOW_MENU` row | [UI.md](main/UI.md) |
| Add a config field | the struct in `src/config/fractal_config.rs` (or `escape.rs`, `sim.rs`), skip-if-default so old files stay stable | [CONFIG.md](main/CONFIG.md) |
| Make a field editable | a `ConfigPath` variant in `src/config/delta.rs` (history label, string key, `update_type`); read and write in `get_value` / `set_value` in `src/config/manager.rs` | [CONFIG.md](main/CONFIG.md) |
| Gate a control by mode | a case in `src/ui/visibility.rs` | [UI.md](main/UI.md) |
| Change tone mapping | `shaders/tonemap.wgsl`; `TonemapParams` in `src/gpu/buffers.rs`; controls in `src/ui/tone_mapping.rs` | [TONE-MAPPING.md](main/TONE-MAPPING.md) |
| Add a GPU binding to the flame shader | `compute_layout_entries` / `compute_entries` in `src/gpu/pipelines.rs`, the declaration in `shaders/core/header.wgsl` | [BUFFERS.md](main/BUFFERS.md) |
| Add a stage to the render tail | the flame path in `render_with`, `render_sim` and `escape_tail` in `src/renderer/render.rs`, and the viewport in `src/app/mod.rs` | [RENDERER.md](main/RENDERER.md) |
| Add a keyboard shortcut | `handle_keyboard` in `src/app/input.rs` | [UI.md](main/UI.md) |
| Add a palette or preset | a pack in `assets/palettes/packs/` or a `.palette` file; presets in `assets/presets.fflame` | [PALETTE_LIBRARY.md](main/PALETTE_LIBRARY.md), [CONFIG.md](main/CONFIG.md) |
| Expose something to scripts | `src/script/api.rs` | [SCRIPTING.md](main/SCRIPTING.md) |
| Change what the API sees | regenerate the contract and corpora | [RELEASE.md](RELEASE.md) |

---

## 7. Testing

- Unit tests live beside the code: `cargo test`.
- Visual regression: `tests/visual/run_tests.py` renders 374 configs
  through the CLI and compares them with a tolerance against small,
  metadata-free baseline thumbnails; `run_all_tests.py` adds WASM
  (Playwright). It does not build the binary first.
- Golden files that fail when stale: the generated WGSL
  (`tests/shader_dumps/`) and the API contract
  (`docs/generated/engine-contract.json`).
- `variation_probe` writes `docs/generated/variation-probe*.txt` for
  cross-platform comparison of variation maths; the census writes
  `docs/generated/variation-census-*.txt`.
- `python scripts/release.py check` runs the fast gates (unit tests,
  WASM check, contract, shader dumps, doc links, the engine feature
  matrix, the gallery crates' tests).

Detail: [TESTING-GUIDE.md](TESTING-GUIDE.md).

---

## 8. Documentation map

### Reference (`docs/main/`)

| doc | covers |
| --- | --- |
| [RENDERER.md](main/RENDERER.md) | `FlameRenderer`, the passes, persistent orbits, the render API |
| [SHADERS.md](main/SHADERS.md) | WGSL templates, the shader builder, flags, the cache |
| [BUFFERS.md](main/BUFFERS.md) | bind groups (up to 17 compute bindings), GPU structs, layout rules |
| [TRANSFORMS.md](main/TRANSFORMS.md) | the flame algorithm, transforms, xaos |
| [VARIATIONS.md](main/VARIATIONS.md) | the variation registry and how to port a variation |
| [COLOR.md](main/COLOR.md) | colour modes, palettes, the histogram and accumulator |
| [TONE-MAPPING.md](main/TONE-MAPPING.md) | the tone map and its controls |
| [CONFIG.md](main/CONFIG.md) | `FractalConfig`, `ConfigManager`, undo, presets, file formats |
| [UI.md](main/UI.md) | panels, menus, render-mode gating, input |
| [CAMERA.md](main/CAMERA.md) | the unified cameras, gestures, fly mode |
| [EXPORT.md](main/EXPORT.md) | PNG, high-res and video export, CLI, metadata |
| [EFFECTS.md](main/EFFECTS.md) | density and colour effects |
| [ANIMATION.md](main/ANIMATION.md) | tracks, signals, audio, playback |
| [ESCAPE.md](main/ESCAPE.md) | the escape-time engine |
| [TERRAIN.md](main/TERRAIN.md) | the 3D terrain view |
| [SIMULATION.md](main/SIMULATION.md), [SIMULATION-CATALOG.md](main/SIMULATION-CATALOG.md) | the simulation engine and its models |
| [FOCUSED-RENDERING.md](main/FOCUSED-RENDERING.md) | deep-zoom targeting for flames |
| [SCRIPTING.md](main/SCRIPTING.md), [SCRIPTING-GUIDE.md](main/SCRIPTING-GUIDE.md) | the Rhai API and the language guide |
| [PALETTE_LIBRARY.md](main/PALETTE_LIBRARY.md) | palette packs and the library panel |
| [FRACTAL-BROWSER.md](main/FRACTAL-BROWSER.md) | the Fractal Browser |
| [ONLINE.md](main/ONLINE.md) | the online API, sync and downloads |
| [I18N.md](main/I18N.md) | translations |

### Elsewhere

| where | what |
| --- | --- |
| [RELEASE.md](RELEASE.md) | release procedure and generated files |
| [TESTING-GUIDE.md](TESTING-GUIDE.md) | tests, benchmarks, probes |
| [WASM.md](WASM.md) | building and running the web app |
| [tutorials/](tutorials/README.md) | user-facing introductions |
| [projects/](projects/README.md) | active plans; the paused flame deep-zoom work is in [projects/deep-zoom/](projects/deep-zoom/deep-zoom-tracker.md) |
| [experimental/](experimental/README.md) | ideas not yet planned |
| [archive/](archive/README.md) | finished projects and history, e.g. [the histogram's evolution](archive/histogram/README.md), [persistent orbits](archive/projects/persistent-orbits.md), [the delta-based config migration](archive/delta-migration/) |
| `docs/generated/` | generated reports and the API contract; do not edit by hand |

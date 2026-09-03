# Fractal Flame Renderer - Project Context

## Overview

**Quick Start:** This file provides a concise quick reference for AI assistants. For detailed documentation, see the docs below.

**Core Documentation:**
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) - System overview and navigation hub
- [docs/main/](docs/main/) - Detailed topic documentation:
  - [UI.md](docs/main/UI.md) - Windows, panels, input handling
  - [BUFFERS.md](docs/main/BUFFERS.md) - GPU layouts, data structures
  - [TRANSFORMS.md](docs/main/TRANSFORMS.md) - Flame algorithm, IFS, thread isolation
  - [RENDERER.md](docs/main/RENDERER.md) - Render pipeline, FlameRenderer
  - [SHADERS.md](docs/main/SHADERS.md) - WGSL modular system
  - [VARIATIONS.md](docs/main/VARIATIONS.md) - Variation system, registry
  - [COLOR.md](docs/main/COLOR.md) - Color modes, palette, histogram
  - [CONFIG.md](docs/main/CONFIG.md) - FractalConfig, presets, undo/redo
  - [EXPORT.md](docs/main/EXPORT.md) - PNG export, metadata, CLI batch mode
  - [SCRIPTING.md](docs/main/SCRIPTING.md) - Rhai script API reference — every callable function (kept honest by a staleness test)
  - [SCRIPTING-GUIDE.md](docs/main/SCRIPTING-GUIDE.md) - Rhai language guide: syntax, running scripts, worked examples (CLI-verified)
- [docs/TESTING-GUIDE.md](docs/TESTING-GUIDE.md) - Unit tests, regression, benchmarks
- [docs/WASM.md](docs/WASM.md) - WebAssembly build guide
- [docs/projects/](docs/projects/) - Per-feature design docs and plans

**Note:** Project history is tracked via git commits. Use `git log --oneline` to see recent changes.

## Quick Reference

### Project Structure

- **Shaders** — dynamically assembled by `src/shader_builder_v2.rs` from modular components; only the active variations' WGSL is compiled into each flame's shader:
  - `shaders/core/` - Modular shader components
    - `header.wgsl` - Params struct, bind groups, Sample struct (template-gated for direct-histogram vs sample-emit output)
    - `main_template.wgsl` - Main compute shader (template flags: RENDER_3D, OUTPUT_HISTOGRAM_DIRECT, PATH_TRACKING, HAS_POST_SYMMETRY, …)
    - `utilities.wgsl` - Camera matrix, projection, world→pixel mapping, color helpers
    - `rng.wgsl` - Random number generation (PCG)
    - `affine.wgsl` / `affine_3d.wgsl` - Affine transform application
    - `accumulate_samples.wgsl` - Sample-emit scatter pass (tiled high-res path)
    - `path_filter.wgsl` - Path filtering for the path-map color mode
    - `noise.wgsl`, `voronoi.wgsl`, `complex.wgsl`, `fractwf.wgsl`, `subflame.wgsl` - Helper libraries pulled in by variations that need them
  - `shaders/accumulate.wgsl` (+ `accumulate_tiled.wgsl`) - Histogram → accumulator fold (see Render Pipeline below)
  - `shaders/tonemap.wgsl` - Display tone mapping pass
  - `shaders/downsample.wgsl`, `shaders/histogram_blur.wgsl` - Supersample/density-estimation support
  - **Note**: Variation WGSL lives inline in `src/variations/defs/*.rs`, not in shader files

- **Core Modules**:
  - `src/app/` - Application state and event handling (mod.rs core loop, input.rs, fly_camera.rs, export.rs, gpu_updates.rs)
  - `src/config/` - **Delta-based state management** (manager.rs ConfigManager + undo/redo, delta.rs ConfigPath/ConfigValue, fractal_config.rs, defaults.rs, slider.rs)
  - `src/variations/` - **Variation system**: registry (mod.rs), static definitions with inline WGSL (`defs/*.rs`, 100+ files, 500+ variations)
  - `src/scene/` - Flame/Transform model (transforms.rs), palettes, presets, randomize.rs
  - `src/renderer/` - GPU orchestration (compute_kernel.rs), unified headless render API (render.rs), thumbnails
  - `src/export/` - High-resolution export (high_res.rs: tiled + CPU-histogram paths)
  - `src/shader_builder_v2.rs` - Per-flame WGSL assembly from templates + active variation defs
  - `src/gpu/` - Buffer types and std140/std430 layouts (buffers.rs)
  - `src/ui/` - **Dockable panel UI** (egui_dock): 25+ panels — viewport, transforms, triangle editor, view, colors, palette editor/library, fractal browser, history, animation, effects, xaos editor, random generator, variations browser, subflames, signal, export, performance, …
  - `src/animation/` - Track-based animation system + video export
  - `src/audio/` - Audio analysis for audio-reactive animation (cpal/symphonia/rustfft)
  - `src/signal/` - Signal/generator routing for animation inputs
  - `src/effects/` - Post-effect chain (density/color effects)
  - `src/api/` - Online sync API client (types.rs, sync.rs)
  - `src/storage/` - Local storage: SystemSettings (settings.rs), cross-platform backend, thumbnail cache, custom palettes
  - `src/resources/` - HTTP/filesystem resource fetching (lazy palette packs; WASM fetch API)
  - `src/escape/` - **Escape-time fractal system** (Mandelbrot and kin; `docs/projects/escape-time-fractals.md`): FormulaDef/ColoringDef registries mirroring the variation system (inline WGSL, append-only, feature flags), marker-splicing assembler, EscapeRenderer (one compute dispatch into an Rgba32Float image in flame-accumulator format, consumed by the shared tonemap/effects tail). Dispatch lives inside `render_with` (CLI/thumbnails/video inherit); the app holds a lazy `escape_renderer` (event-driven, `escape_dirty`). `RenderMode::Escape`; EscapeConfig center is exact decimal strings + `zoom_log2`; entering escape defaults tonemap Linear + resets exposure/gamma (flame presets carry Log-calibrated values that render Linear output invisibly). Online sync supports escape configs (`ApiRenderMode::Escape`): stored AS FLAMES on the same endpoints, told apart by `render_mode` alone, so `transform_count` is 0 and `variation_names` empty for them legitimately
  - `src/flame_xml.rs` - Apophysis/JWildfire `.flame` XML import/export
  - `src/i18n.rs` - rust-i18n integration; translations in `locales/*.yml` (en, es, ja, zh-CN)

## Environment
- Shell within VSCode: Git Bash (MSYS2 MinGW64)
- Use forward slashes for paths
- Avoid `/dev/null` (creates literal files on Windows)
- **Use relative paths for file edits** (e.g., `src/ui/mod.rs` not `c:\projects\fflame-rust\src\ui\mod.rs`) - helps avoid "file unexpectedly modified" errors from IDE/linter race conditions
- One-shot codemod scripts go in `scripts/`, not temp folders outside the repo

### Key Concepts
- **Fractal Flames**: IFS (Iterated Function System) with variations
- **Render Modes**: 2D (classic) and 3D (pseudo-3D with depth); single `main_template.wgsl` specialized via the RENDER_3D template flag
- **Variations**: registry holds 500+ ported variations (flam3 / Apophysis / JWildfire plugins)
  - Definitions are `static VariationDef`s in `src/variations/defs/*.rs` with inline WGSL (2D + 3D bodies), parameters, and feature flags
  - Up to `MAX_VARIATIONS_PER_FLAME = 100` active per flame; the shader builder compiles only the active set, with a **per-flame local index map** (no global fixed indices)
  - UI ordering: by category, then registration order (the `defs/mod.rs` registration list is append-only)
- **Variation Parameters**: per-transform values in a HashMap (`"varname.param"` keys)
  - Uploaded via a packed storage buffer (`MAX_VARIATION_PARAM_SLOTS = 1600` floats, variable slots per variation: user params + init-derived + state slots)
  - Shaders read via `get_param(xform_id, variation_id, slot)`; per-thread state via `get_state`/`set_state`
  - UI sliders auto-generate from the param defs (Float, UnlimitedFloat, Integer, Angle, Boolean, Enum types)
- **Render Pipeline** (each frame):
  1. **Compute Pass** - chaos-game iteration; each plotted sample atomically adds into a per-pixel u32 histogram (R, G, B, density × `color_scale = 100`). Dispatch size = workgroups × 64 threads × `iterations_per_thread` (default 256). flam3/JWF-style bad-value recovery: X/Y divergence respawns + re-fuses the point; Z saturates at ±1e32 with an amortized respawn (see `docs/projects/preserve-z-semantics.md`). High-res exports above the 128 MB storage-binding limit switch to a sample-emit + tiled scatter path (`accumulate_samples.wgsl`) or CPU histogram (`export/high_res.rs`).
  2. **Accumulate Pass** (`accumulate.wgsl`) - folds the frame histogram into an `Rgba32Float` accumulator: rgb = density-weighted running-mean color, a = raw cumulative hit count. Adaptive blend: `effective_blend = max(new_density/total_density, blend_factor)`; `blend_factor = 0` is the reference density-weighted mean, higher values keep late batches contributing (`use_dynamic_blend` toggles the adaptive mode).
  3. **Tonemap Pass** (`tonemap.wgsl`) - flam3-style log mapping with `k1`/`k2` normalized by `sample_density = total_iters / pixel_count` (brightness is iteration-count invariant), plus exposure/gamma/brightness/vibrancy, scale-invariant Levels, optional tone curve, background blend.
- **Color Modes**: Palette (Apophysis color-coordinate evolution), Speed (distance per iteration), PathMap (transform-path history visualization with path filters)
- **Projection**: `perspective_strength: f32` (0 = orthographic; Apophysis `zr = 1 − persp·z` formula with behind-camera clipping)
- **Camera**: full 4-angle Apophysis/JWildfire camera (pitch, yaw, bank, roll/rotation — effective chain `Rz(rotation)·Rx(pitch)·Ry(bank)·Rz(−yaw)`) plus world-space position (`camera_x/y/z`, JWF `cam_pos_*` round-trip)
  - **Fly mode** (F2 / 🚀): WASD/QE movement + mouse-look; two modes in SystemSettings — FreeLook (screen-relative, gimbal-free) and FPS (world-up anchored)
- **Depth tools** (3D): DoF blur, depth fog, depth-density compensation (radiance-preserving splats), far-density fade
- **Solid rendering** (3D): splat-native pipeline — occlusion = per-pixel nearest-depth culling (`solid_strength` 0 = classic transparency, 1 = hard surface, blendable via the per-sample `density_weight` channel; `surface_thickness` shell), lighting = deferred shade pass (Blinn-Phong + screen-space normals/AO), shadows = 4 light-space shadow maps at splat resolution (histogram tail, auto-fit from measured attractor bounds). Depth region lives inside the histogram buffer (5th u32/pixel, inverted ordered-float encoding, atomicMax); SOLID shader-builder flag ⇒ byte-identical WGSL when off; at-splat DoF compiles out in solid mode. All export paths supported (no shadow maps on the CPU-histogram fallback). Solid renders are NOT bit-reproducible (in-batch depth race) — the `solid-*` visual regression tests use tolerance compare. The Phase 2/3 density volume was removed 2026-07-16. See [docs/projects/solid-rendering.md](docs/projects/solid-rendering.md)
- **Pan/rotation**: both render modes compose pan → rotate → zoom (Apophysis convention); all pan inputs share `FractalConfig::screen_delta_to_pan_frame`

### UI Architecture (egui_dock)
- **Docking system**: all UI is dockable panels (`src/ui/workspace.rs` defines `PanelType` — 25+ panels)
- Main editing panels: Fractal Viewport, Transforms, Triangle Editor, View, Colors/Tone Mapping, Palette Editor/Library, Fractal Browser, History, Animation, Effects, Xaos Editor, Random Generator, Variations, Subflames
- **Menu bar**: File, Edit, View, Fractal, Rendering, Window, Help (`src/ui/menu_bar.rs`)
- Per-panel code lives in its own `src/ui/*.rs` file; `src/ui/mod.rs` coordinates docking and bubbles responses through `UiResponse`

### Palette Library System
- **Pack-based organization** (`assets/palettes/packs/*.json`): **the folder is the catalog** — desktop scans it at startup (drop a pack JSON in, it appears; `enabled_by_default` inside each pack, absent = enabled); `starter_pack.json` is also embedded in the binary as the offline fallback (`BUILTIN_PACK_FILE` — both discovery paths skip that filename so it doesn't load twice); `apophysis1-4.json` ship disabled. WASM discovers packs via a manifest `build.rs` generates into OUT_DIR (browsers can't list directories; nothing committed, guarded by `generated_manifest_matches_packs_folder`) and lazy-fetches content via `src/resources/`. Loose `assets/palettes/*.palette` files load into a "Local Files" pack (desktop)
- Palette Library panel: gradient previews, expand/collapse per pack, click-to-select creates an editable `"Name (Custom)"` copy
- All load routes use `add_or_update()` with case-insensitive duplicate checking
- **See**: [docs/main/COLOR.md](docs/main/COLOR.md) and [docs/main/PALETTE_LIBRARY.md](docs/main/PALETTE_LIBRARY.md)

### Internationalization (i18n)
- **Framework**: rust-i18n with YAML translation files in `locales/` (compile-time embedding)
- **Current languages**: English (en), Spanish (es), Japanese (ja), Simplified Chinese (zh-CN)
- Language switcher in Settings → Preferences (persisted in SystemSettings)
- **Font support** (egui default): full Latin/Cyrillic/Greek; CJK limited to basic characters unless a CJK font is added via `FontDefinitions`; no RTL
- **See**: [docs/main/I18N.md](docs/main/I18N.md)

### System Settings & Local Storage
- **Architecture**: ConfigManager owns both
  - **FractalConfig**: per-fractal artistic parameters (undo/redo enabled)
  - **SystemSettings**: device preferences (no undo, persisted to disk immediately)
- **System settings include**: VSync + target FPS, iterations per thread, language, export defaults, fly-mode input preferences (sensitivity, speed, sprint, invert-Y, camera mode), online-mode credentials, compact mode
- **Storage backend** (`src/storage/backend.rs`):
  - Windows: `%APPDATA%\Fractals for All\Fractal Art Editor\data\`; macOS/Linux equivalents via `directories`
  - WASM: browser localStorage
- All changes flow through `config_manager.update_system_setting()` which returns an `UpdateType` for GPU synchronization
- **See**: [docs/projects/local-storage-system.md](docs/projects/local-storage-system.md)

### Important Implementation Details
- **Atomic u32 histogram accumulation** (thread-safe; 4×u32 per pixel) feeding an `Rgba32Float` accumulator — see Render Pipeline above and [docs/main/COLOR.md](docs/main/COLOR.md)
- Using **JSON** for serialization
- **Precision Limitation (f32 vs Apophysis double):**
  - Apophysis/JWildfire use 64-bit `double` for variation weights and parameters (~15-16 digit precision)
  - We use 32-bit `f32` (~7 digit precision) - **WGSL has no f64 support**
  - Impact: minimal for typical flames, may cause slight differences at extreme values
- **Undo/redo**: delta-based, 500-state history; rapid changes coalesce (500 ms inactivity threshold, 3 s max span); fly-mode camera gestures coalesce as a unit via a shared history marker
- **Full WASM support** for web builds (including PNG export)
- GPU buffers use **std430 layout** (storage buffers) and **std140 layout** (uniform buffers); struct fields in `GpuParams` must start on 16-byte boundaries — keep the explicit pad before `post_symmetry` in sync between `src/gpu/buffers.rs` and `shaders/core/header.wgsl`
- **WASM shader compatibility:** Use `textureLoad()` instead of `textureSample()` inside non-uniform control flow (browser WebGPU strictly enforces the WGSL spec; desktop drivers are lenient)
- **JWildfire/Apophysis is the gold standard**: when our rendering differs from theirs for the same `.flame`, ours is wrong (deliberate divergences — e.g. world-space `cam_pos`, 3D pan semantics — are documented in code comments and `docs/projects/free-camera-movement.md`)

### Build Commands
```bash
# Desktop GUI (Windows/macOS/Linux)
cargo run --release

# Headless CLI Export (batch PNG generation)
cargo run --release -- export --input tests/visual/configs --output tests/visual/current
# See CLI Export section below for details

# WASM (Web app) — the scripts, not wasm-pack: they build `--profile dist`
# (18% smaller than release, see docs/RELEASE.md §4c) and run wasm-bindgen.
./build-wasm.sh          # or build-wasm.bat on Windows

# WASM (gallery modules only — these DO use wasm-pack)
wasm-pack build --target web --release   # in wasm/render, wasm/script

# iOS / Android (experimental, not fully functional — see Mobile section)
cargo build --target aarch64-apple-ios
cargo build --target aarch64-linux-android
```

### Releasing, and changing anything the API sees

**See [docs/RELEASE.md](docs/RELEASE.md)** — every procedure that has to
run, in order, plus what is still undecided about packaging.

```bash
python scripts/release.py check      # every fast gate, ~4s
python scripts/release.py            # ...and a changelog preview
```

Nothing runs on push or on a timer. It is a command you type.

The short version of the part that bites most often: three committed
files are **generated**, and a stale one fails a test rather than
failing silently. Regenerate deliberately and read the diff.

```bash
UPDATE_CONTRACT=1 cargo test --lib contract_is_current        # docs/generated/engine-contract.json
UPDATE_SHADER_DUMPS=1 cargo test --lib canonical_shader_dumps # tests/shader_dumps/
cargo run --release --bin export_variations_json              # the corpus the API serves
cargo run --release --bin export_scripts_json                 # built-in scripts, source included
cargo run --release --bin export_effects_json
```

**A new `Feature`, `VariationCategory` or `ParamType` does NOT move the
contract's shape fingerprint** — it adds an array element, not a key
path — so the API's staleness pin will not fire. Tell the API repository
directly. See RELEASE.md §3.

Release builds use `cargo build --profile dist`, not `--release`: 31%
smaller, ~10 minutes, and the profile name is recorded in every exported
PNG so a bug report identifies its binary.

### Testing & Profiling

See [docs/TESTING-GUIDE.md](docs/TESTING-GUIDE.md) for complete guide.

```bash
# Unit tests (embedded in source files; 240+ tests)
cargo test

# Unified benchmark suite (CPU + GPU + visual regression)
python scripts/run_benchmarks.py          # Full suite
python scripts/run_benchmarks.py --quick  # Quick mode (skip WASM)
```

**Unified Benchmark Suite** (`scripts/run_benchmarks.py`):
- CPU microbenchmarks (Criterion), GPU desktop + WASM render tests, visual regression via pixel-hash comparison, CSV performance history with color-coded regression output

**What's tested:** transform math, variations, palette interpolation, XML round-trips (camera, variation-param unit conversions), camera-matrix/rotation math (fly-mode `to_euler_near` round-trips and pole handling), config storage keys, version info

**Variation math probe** (`cargo run --release --bin variation_probe`):
evaluates every shipped variation's shader arithmetic at a fixed
adversarial input grid (all four signed zeros, values straddling the
`1e32` bad-value threshold, subnormals) in 2D and 3D, and writes
`docs/generated/variation-probe.txt`, plus a `-sweep.txt` companion that
moves each parameter one at a time (both arms of every boolean, every
enum choice, numeric extremes) — that sweep reaches NaN/Inf in 386 places
the defaults never do. Diff two of them —
`variation_probe -- compare OLD NEW` — to find cross-platform
divergence; it is the tool for the fast-math class of bug that cost
`npolar` 73% of its pixels on Metal. Compares **classes** (zero / finite
/ NaN / inf / past-threshold), which no rounding difference can perturb,
separately from quantised magnitudes; only a class change exits
non-zero. The first run on a machine pays cold driver compiles (~4 min);
subsequent runs are seconds. See
[docs/projects/variation-math-probe.md](docs/projects/variation-math-probe.md).

### CLI Export Mode

The main app supports headless batch PNG export for testing and automation:

```bash
# Export single file
FractalArtEditor export -i config.fflame -o output.png --width 1920 --height 1080

# High-resolution export (automatically switches paths for large sizes)
FractalArtEditor export -i config.fflame -o output.png --width 4000 --height 4000

# Batch export directory
FractalArtEditor export -i tests/visual/configs -o tests/visual/current

# With test category metadata
FractalArtEditor export -i tests/visual/configs/variations -o tests/visual/current --category variations
```

**Features:**
- Any resolution (4K, 8K+); automatic path selection around the 128 MB storage-binding limit (direct histogram below it; tiled sample-emit / CPU histogram above — see `src/export/high_res.rs` and `docs/projects/unified-render-pipeline.md`)
- Renders exact `max_iterations` from config for reproducibility, with progress indicator
- Full PNG metadata embedding; batch processes directories; headless (no window)

### PNG Metadata

All exported PNGs include metadata in tEXt chunks: build info (version, git hash, platform), render settings (resolution, iterations, time), the complete FractalConfig JSON + SHA256 checksum, display settings, and optional test name/category for visual regression.

```rust
use fractal_flame_wgpu::png_metadata::read_png_metadata;
let metadata = read_png_metadata("output.png")?;
```

### WASM API

The WASM build exposes a JavaScript API (`src/wasm_api.rs`) for headless PNG export in browsers:

```javascript
import init, { WasmApi } from './pkg/fractal_flame_wgpu.js';
await init();
const api = new WasmApi();
const config = await fetch('config.fflame').then(r => r.json());
api.load_config_json(JSON.stringify(config));
const pngData = await api.export_png(800, 600, 256, false); // w, h, iters/thread, transparent
```

**Browser compatibility:** Chrome/Chromium 113+ and Firefox 121+ fully tested; Safari WebGPU experimental; mobile browsers limited. WebGL fallback is impossible (compute shaders required).

## Coding Guidelines

### GPU Code
- All shaders use **WGSL**; follow std140/std430 layout rules for buffers
- **WASM Compatibility**: `textureSample()` only from uniform control flow; use `textureLoad()` inside conditionals (desktop drivers are lenient, browsers fail silently with black output)
- **Metal runs shaders with fast-math ON, so IEEE NaN/Inf rules do not hold there.** wgpu never clears `MTLCompileOptions.fastMathEnabled`, which defaults to true. Measured on an M2: `x != x` returns **false** for a real NaN, and `Inf/Inf` returns **1.0** instead of NaN — so a bad value can pass a guard *and* survive as a plausible finite number.
  - Never detect non-finite values with a self-compare (`x != x`) or by expecting `Inf` arithmetic to poison a result. Use `!(abs(x) <= 1e32)`, the idiom `main_template.wgsl`'s bad-value recovery uses — it is negated-comparison based, so NaN fails the comparison and the negation fires. Verified to survive the optimizer.
  - Never write a bare self-division (`x / x`) either: it is 1.0 except at 0/0 and Inf/Inf, and fast-math folds those to **1.0 too** — a plausible finite value no later guard can catch. Write the intended value explicitly and reproduce the reference's zero-case behaviour (worked example: `cut_btree` in `defs/cut_simple.rs`). **Both idioms are enforced**: the `shader_lint` tests (`src/variations/mod.rs`) reject self-compares and self-divisions in every registered variation's WGSL and all `shaders/*.wgsl` files, alongside the subnormal-literal lint.
  - **Metal's `atan2` is broken at zero pairs in both ways** (measured): same-sign zeros give **π/4** — a plausible finite value that silently relocates the point (cost `apo-misc7` 73% of its lit pixels via `npolar`) — and *mixed*-sign zeros give **NaN**, which kills the point in bad-value recovery (cost `circular`/`circular2`/`ex`/`flower_db` their origin behaviour and `hypercrop` its n-gon corners). Away from zero pairs it agrees to ≤1 ulp. The guard is `ff_atan2` in `shaders/core/utilities.wgsl` — always in scope, IEEE-exact for all four sign pairs; adopted wherever a zero pair is reachable (npolar, ho, log_db, circular, circular2, ex, flower_db, hypercrop). Sign-of-zero **laundering** matters too: affine sums and FTZ can turn any zero into either sign, so "same-sign only" is not a safe assumption at any reachable origin.
    - When guarding it, reproduce IEEE rather than returning 0: `atan2(+0,-0)` is `+π` and `atan2(-0,-0)` is `-π`. Flattening all four sign cases to 0 changes the render on Vulkan — measured, it moved the image. Read the sign via `bitcast<u32>` (`x < 0.0` is false for `-0.0`, and integer ops are immune to fast-math).
    - **616 `atan2` call sites across 86 variation files are unaudited.** Only those that can reach exactly (0,0) are affected; `npolar` was found by bisection, not by audit.
  - **Trig of huge arguments is garbage everywhere, differently.** Measured at `sin/cos(1e20·π)`: Metal fast-math gives `(+0, +0)`, NVIDIA gives `(0, garbage)`, f64 IEEE gives a scattered finite pair — three mutually-incompatible answers, because past ~2²⁴ adjacent f32s differ by more than the whole period. Do not guard these (there is no reference class to guard toward — see the third clause in `docs/accepted-divergences.txt`); do flag ports whose *reachable* inputs feed trig unbounded arguments.
  - This is a real rendering difference, not just rounding. Disabling fast-math globally is not the answer — measured **1.4x slower** typical, **5.2x** worst case, and it merely traded which tests failed. Fix at the shader level and verify the guard is bit-identical under IEEE so Windows is unaffected.
- **Trust the shader compiler for optimization**: modern GPU compilers perform aggressive CSE — write clear code, don't hand-hoist (see [docs/SHADER_COMPILER_CSE_ANALYSIS.md](docs/archive/optimization-attempt-2025-11-02/SHADER_COMPILER_CSE_ANALYSIS.md))

### Rust Code
- Use `bytemuck::Pod` and `bytemuck::Zeroable` for GPU data structures
- GPU struct alignment: vec3 needs 16-byte alignment; add explicit padding and mirror it in the WGSL header
- Prefer `Queue::write_buffer()` over buffer mapping for updates

### State Management
**All configuration changes flow through ConfigManager** - see [docs/main/CONFIG.md](docs/main/CONFIG.md).

**Core principles:**
- Type-safe `ConfigPath` enum identifies all parameters; `UpdateType` return value drives selective GPU updates
- All updates are immediate; coalescing merges rapid changes (500 ms inactivity / 3 s span) into one undo point
- A brief overwrite window after parameter changes keeps real-time updates smooth before the iteration reset

**UI patterns:**

```rust
// 1. Single parameter change
if response.changed() {
    config_manager.update_param(path, value.into())?;
}

// 2. Batch update (one undo point, e.g. triangle editor, color pickers)
config_manager.update_batch(changes, "description".to_string())?;
```

Structural actions (transform add/delete, config import/export, preset loading) are intentionally outside ConfigManager.

### Variation Registry Architecture
- **Global singleton**: `global_registry()` returns `&'static VariationRegistry` (once_cell)
- **Definitions**: `static VariationDef` consts in `src/variations/defs/*.rs`; registered by appending to the list at the end of `defs/mod.rs` (**append-only** — registration order is the stable ID order)
- **Per-flame local index mapping**: `Flame::get_id_mapping()` / `compute_local_index_map()` assign shader indices to only the flame's active variations (max 100); the shader is compiled with exactly that set
- **Param packing**: variable slots per variation (user params, then init-derived, then per-thread state) packed into a 1600-float buffer
- **UI ordering rule**: sort by category first, then registration order — iterate `ordered_names`, never HashMap order

### Performance
- Target 60+ FPS at 1080p; progressive refinement adds samples every frame
- Dispatch = workgroups × 64 threads × `iterations_per_thread` (SystemSettings, default 256)
- Track total iterations for quality measurement; histogram density is iteration-count-normalized in the tonemap, so brightness is stable as accumulation runs

## Common Tasks

### Adding a New Variation

1. Create `src/variations/defs/<name>.rs` with a `static VariationDef` (see `defs/octapol.rs` or `defs/szubieta.rs` for recent examples):
   ```rust
   pub static MYVAR: VariationDef = VariationDef {
       name: "myvar",
       aliases: &[],                       // foreign names for .flame import
       display_name: "My Var",
       category: VariationCategory::Plugin,
       phase: VariationPhase::Normal,      // or Pre / Post
       features: &[],                      // NeedsRng, NeedsTransform, NeedsAccum, WritesColor, WritesRgb, AlwaysZ, NeverZ
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
2. Register in `src/variations/defs/mod.rs`: `mod <name>;`, `pub use <name>::*;`, and append `&MYVAR` to the END of the registration list (append-only!)
3. Signature rules (enforced by the shader builder):
   - parameters present → `xform_id: u32, variation_id: u32` args; read via `get_param(xform_id, variation_id, slot)`
   - `NeedsRng` → `rng: ptr<function, RngState>` arg
   - `NeedsAccum` → `accum: vec2/3<f32>` right after `p` (the running weighted sum — for JWF variations that read or clobber `pVarTP`)
   - weight-inside-the-formula JWF variations: read `transforms[xform_id].variations[variation_id]` and pre-divide the result by it (idisc pattern) so the dispatcher's outer `result += w * f(p)` cancels
   - helper functions must be name-prefixed (`fn myvar_helper(...)`) to avoid collisions
4. Port faithfully from the JWF/Apo source (keep quirks; cite the source file in the module docs); the variation auto-appears in the UI under its category
5. If the foreign app stores a parameter in different units, add a conversion pair in `flame_xml.rs` (`variation_param_from_xml` / `_to_xml` — see radial_blur)
6. **Z semantics**: if the JWF source writes `pVarTP.z` UNCONDITIONALLY (no `isPreserveZCoordinate()` gate), add `Feature::AlwaysZ` — otherwise its z contribution is zeroed under `preserve_z = false`. `scripts/audit_z_write_semantics.py` classifies automatically from `output/variation-jwf-source/`. See `docs/projects/preserve-z-semantics.md`.

### Adding a New Palette
**Option 1: Code-based** — add to `src/scene/palette.rs` + `PaletteLibrary::new()`
**Option 2: File-based** — `.palette` file in `assets/palettes/` (desktop auto-load), or a pack JSON in `assets/palettes/packs/`
**Option 3: Import/Export** — Palette Editor → Import/Export (clipboard or `.palette` file)

### Adding a New Preset
**Option 1: Code-based** — `src/scene/presets.rs` + `PresetLibrary::new()`
**Option 2: File-based** — `.fflame` in `assets/presets/` (desktop auto-load)
**Option 3: Export current state** — Save Config → place in `assets/presets/`

### Modifying Tone Mapping
1. Edit `shaders/tonemap.wgsl`
2. Update `TonemapParams` in `src/gpu/buffers.rs` if adding parameters
3. Expose controls in `src/ui/tone_mapping.rs`

### Adding UI Controls
- Each panel lives in its own `src/ui/*.rs` file; add controls there and write through `config_manager.update_param` / `update_batch`
- Cross-panel results bubble via `UiResponse` (`src/ui/response.rs`) and are handled in `src/app/mod.rs`

### Creating 3D Presets
1. Set `flame.render_mode = RenderMode::ThreeD`
2. Set `flame.perspective_strength` (0.0 = orthographic; ~0.1-0.3 typical, higher = stronger)
3. Use 3D variations (zcone, flatten, hemisphere, zscale, pre/post rotations, …) for Z structure; `Transform::g` is the Z offset
4. Variations are set by name: `xform.variations.insert("zcone".into(), 0.5);`
5. Give transforms different `g` values to create layers; verify with camera pitch/yaw (or fly mode)

## Dependencies
See @Cargo.toml for the full list. Key dependencies:
- **wgpu 29.0** - WebGPU API (desktop: vulkan/metal/gles — dx12 deliberately omitted; WASM: webgpu only)
- **winit 0.30** - Window management
- **egui 0.34** + **egui_dock 0.19** - UI and docking
- **serde / serde_json** - Serialization; **quick-xml** - .flame import
- **rust-i18n 3.1** - Internationalization
- **image / png** - PNG export; **bytemuck** - GPU data layout
- **cpal / symphonia / rustfft / ringbuf** - audio-reactive animation stack
- **rayon** - CPU-parallel export paths; **ureq** - desktop HTTP

## File Formats

### Palette Files (.palette)
JSON with `name` + `stops` (position 0-1, RGB 0-1 colors).

### Config Files (.fflame)
JSON containing full fractal state (see `src/config/fractal_config.rs`). Includes **everything needed for exact reproduction**: flame definition (transforms, variations + params, colors), view state (zoom/pan/rotation, 4-angle camera + position), rendering settings (`max_iterations`, `deterministic_rng`), color settings (mode, embedded palette data, background), tone mapping (mode, curve, exposure, gamma, levels), depth effects. New fields are skip-if-default so old files stay stable.

### Flame Files (.flame)
Apophysis/JWildfire XML (`src/flame_xml.rs`). Round-trips camera (`cam_pitch/yaw`, `cam_roll`→bank rename quirk, `rotate`→rotation, `cam_pos_x/y/z` + legacy `cam_zpos`), perspective, DoF, post-symmetry, xaos, palettes, and variation parameters (with per-param unit conversions where JWF units differ — see `variation_param_from_xml`). Our own extensions (depth-density compensation, far-density fade) are `.fflame`-only and deliberately not written to XML.

## Important Implementation Notes

### Preset System
Presets store **complete FractalConfig** (not just the Flame). Transform buffers are pre-allocated for `MAX_TRANSFORMS` (128) with zero-padding of unused slots; `FlameRenderer::load_config()` synchronizes all GPU state atomically; `reset()` only clears accumulation.

### Asset Loading
Desktop builds auto-load `assets/palettes/packs/*.json` (pack scan), `assets/palettes/*.palette` (the "Local Files" pack) and `assets/presets/*.fflame` at startup; WASM embeds the builtin pack + a build-generated manifest and lazy-loads the rest via `src/resources/`.

### Pan / Zoom / Rotation Input
All pan inputs (mouse drag, arrow keys, View-panel buttons, wheel zoom-to-cursor, pinch) convert screen deltas through `FractalConfig::screen_delta_to_pan_frame` — rotation-aware, identical in 2D and 3D because both pipelines compose pan → rotate → zoom. Wheel zoom anchors to the cursor except in fly mode (zooms to center).

### 3D Rendering System
- Single `main_template.wgsl` specialized at build time via the RENDER_3D flag; 3D tracks `vec3` through the chaos game
- **Camera**: JWildfire's 4-angle matrix built in `utilities.wgsl::build_camera_matrix` — effective chain `Rz(rotation)·Rx(pitch)·Ry(bank)·Rz(−yaw)` applied as `M·(p − camera_pos)`. JWF applies its matrix transposed, which is why the call-site slot mapping looks swapped; see the comments there before touching it.
- **Projection**: Apophysis `zr = 1 − persp·z` with behind-camera clipping (`zr < 1e-3` discarded, matches JWF)
- **Fly mode**: `src/app/fly_camera.rs` — SO(3) mouse-look composition with continuity-preserving Euler decomposition (FreeLook) or classic Euler increments (FPS mode); WASD basis comes from the camera matrix rows
- **Depth effects** at plot time: DoF blur, fog (color blend toward background), depth-density compensation and far-density fade (per-sample density weighting; carried through all accumulation paths)
- **preserve_z**: JWF flag; default false flattens Z each iteration to keep Z-scaling variations from diverging
- Old presets with array-style variations are migrated by the custom deserializers

### Histogram Color Accumulation
Thread-safe atomic u32 accumulation (4×u32 per pixel: R, G, B, density), fixed internal `color_scale = 100` (the former user-tunable slider was removed — the value cancels in color recovery). Per-sample density weights (depth-density compensation / far-density fade) scale all four channels so recovered color is weight-invariant. **See [docs/main/COLOR.md](docs/main/COLOR.md).**

## Known Issues
- No error handling for invalid `.fflame` or `.palette` file imports
- Transparent PNG export applies tone mapping on CPU from the accumulation buffer (the tonemap shader blends RGB with background before alpha)
- Density effects on the `HighResExporter` path are gated by a resolution budget: they run in Phase B (after the histogram buffer is freed) on full-res ping-pong textures, costing ~36 B/px on top of the accumulation. Above a ~4 GiB budget (≈119 MP, ~10,900² square) they're skipped (export still completes, warning logged) to avoid OOM on limited-VRAM GPUs. Below that they apply identically to the FlameRenderer/in-app path.
- Topic docs under `docs/main/` may lag the code; verify against source when something looks off
- **macOS is not green, and both entries are known and tabled.** (1) `renderer::sticky::extras_alone_never_change_pixels` fails on Metal — it asserts that dead, branch-skipped variations are BIT-identical, but their presence changes codegen, so 44% of bytes differ (mean 1.8, max 40 on 64×64). Visually harmless: every flame visual test passes. The test body is identical on `main`, so this is not an escape-time regression; the fix is a tolerance compare, as the equally non-reproducible `solid-*` visual tests already use. It is the only failing `scripts/release.py check` gate on macOS. (2) The visual suite reads **217/238** on macOS against **238/238** on Windows — 21 escape-time renders differ. Fully investigated in [docs/projects/escape-time-fractals.md](docs/projects/escape-time-fractals.md): the df fold and trig range reduction were found and fixed (both were wrong on BOTH platforms), and reassociation and ulp-amplification were both measured and ruled out. The residual is unexplained and deliberately parked — perceptually the renders are indistinguishable. Next lead recorded there: `normalize`/`inversesqrt`, the `clamp(dot(...))` discontinuity, `pow`, and palette texture filtering.

## Mobile Platform Support (Experimental)

**Status:** Cross-compilation works; runtime needs dependency fixes.

- **iOS** (`aarch64-apple-ios`): blocked on `rfd` (file dialogs) — needs conditional compilation or native pickers
- **Android** (`aarch64-linux-android`): needs `android-activity` features + manifest/packaging work (`cargo-apk`/`xbuild`)
- General: touch controls, mobile GPU iteration defaults, small-screen UI (compact mode exists), platform storage APIs

**Feasibility:** Medium-High — wgpu supports mobile GPUs; the surrounding infrastructure needs platform adaptations.

## Optional/Future Features

### Medium Priority
- **Async export progress UI** - export currently blocks the UI during rendering
- **EXR/HDR export** - high dynamic range output for compositing

### Low Priority / Future Expansions
- **Layered compositing** - multiple flames blended together
- **Adaptive sampling** - focus iterations on high-detail areas
- **Denoising** - faster convergence
- **Orbital camera mode** (JWF `cam_*focus`) and camera-path "tour mode" for fly mode

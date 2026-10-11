# WebAssembly (WASM) Build Guide

> **Two different WASM products.** This document is the **web app** —
> the full editor in a browser, built by `build-wasm.sh` / `.bat`. The
> other is the set of headless **gallery modules** under `wasm/`
> (`render`, `flame`, `escape`, `sim`, `script`), which take config
> JSON and return pixels; they are built with `wasm-pack` and
> documented in [wasm/README.md](../wasm/README.md).
>
> All three render engines work in the web app. Among the gallery
> modules, `wasm/render` is built with the flame and escape-time
> engines only (its `Cargo.toml` features), so a simulation needs
> `wasm/sim`. The single-engine modules exist only to cut download
> size, and are not interchangeable: a module built without an engine
> reports that the engine is missing (`RenderError::EngineMissing`)
> rather than rendering something the file never described.

The live web app is `https://fractalsforall.com/editor/`; how it gets
there is [RELEASE.md §5](RELEASE.md).

## What runs in the browser

The same application as the desktop build: the library crate compiled
to `wasm32-unknown-unknown`, booted by `wasm_main` (`src/lib.rs`, a
`#[wasm_bindgen(start)]` function), drawing the same egui panels on a
canvas.

- **All four render modes** — 2D and 3D flames, escape-time fractals
  ([ESCAPE.md](main/ESCAPE.md)) and simulations
  ([SIMULATION.md](main/SIMULATION.md)).
- **The full variation registry** — 647 variations.
- **Colour modes** Palette, Speed and PathMap
  ([COLOR.md](main/COLOR.md)).
- **Presets** from `assets/presets.fflame`, embedded in the module at
  compile time (`src/resources/presets.rs`).
- **Palette packs:** `starter_pack.json` is embedded; the other packs
  are listed by a manifest that `build.rs` generates from
  `assets/palettes/packs/` and are fetched on demand through
  `src/resources/` ([PALETTE_LIBRARY.md](main/PALETTE_LIBRARY.md)).
- **Files:** opening uses the browser's file picker
  (`trigger_browser_file_picker` in `src/app/mod.rs`, an
  `<input type="file">`) for `.fflame` / `.flame` configs, animations,
  scripts and audio; saving and PNG export arrive as browser downloads
  (`trigger_browser_download`).
- **Settings** persist in the browser's `localStorage`
  (`src/storage/backend.rs`).
- **Online sync** ([ONLINE.md](main/ONLINE.md)).

What differs from desktop is mostly what a browser cannot do — list a
directory, block on the GPU, read a file without the user picking it.
The constraints that shape code are below.

## Building the web app

### Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.121
```

The CLI's version must match the `wasm-bindgen` crate in `Cargo.lock`
(0.2.121 at the time of writing); `wasm-bindgen` refuses a mismatch.

### Build

```bash
./build-wasm.sh            # Linux, macOS, Git Bash
build-wasm.bat             # Windows cmd
```

Both scripts do the same three things:

1. `cargo build --lib --target wasm32-unknown-unknown --profile dist`,
   with `RUSTFLAGS` set to `--cfg=web_sys_unstable_apis`,
   `--cfg getrandom_backend="wasm_js"`, `-C target-feature=+simd128`
   and a 64 MiB shadow stack (`-C link-arg=-zstack-size=67108864`).
   Setting `RUSTFLAGS` *replaces* the target's list in
   `.cargo/config.toml`, so the scripts repeat those flags; that file
   explains each one (getrandom 0.3's backend selection; the escape
   engine's vectorized bignum multiply; a default 1 MiB stack that
   overflows silently as `index out of bounds`).
2. `wasm-bindgen --target web --out-dir ./pkg` on the built module.
3. Copy `assets/palettes/packs/*.json` into `pkg/assets/palettes/packs/`.

The `.bat` runs `setlocal` first, so its `RUSTFLAGS` does not leak into
later native builds in the same window. The two scripts are parallel
implementations of one procedure and nothing checks that they agree
([RELEASE.md §7](RELEASE.md)).

### Profiles

| flag | profile | what it is for |
|---|---|---|
| (none) | `dist` | the shipped build: fat LTO, one codegen unit, stripped, `panic = "abort"`. About 16 MiB of `.wasm` (16.0 MiB when measured for the `dist-symbols` comment in `Cargo.toml`) |
| `--symbols` | `dist-symbols` | `dist`'s codegen exactly, with the symbol table kept (`strip = false`, `debug = false`). Writes to `./pkg-names/` and copies the module to `pkg/fractal_flame_wgpu_bg.names.wasm`, leaving the served `./pkg` untouched. 18.9 MiB against dist's 16.0 |
| `--debug` | `dist-debug` | debug info, debug assertions, overflow checks and unwinding panics, with thin LTO and 16 codegen units. A `debug_assert` or a wrapping subtraction fails at its source instead of corrupting an index somewhere else. Bigger and slower, and because it changes codegen it does not reproduce faults that depend on how the shipped build is optimized |

Why `dist` and not `--release` (size, and the profile name recorded in
every PNG and in the Performance panel): [RELEASE.md §4c](RELEASE.md).
Why `dist-symbols` keeps `debug` off: emitting DWARF took the module
from 17 MB to 206 MB and changed its compile and instantiation timing
enough to hide the race it was built to catch (the comment on
`[profile.dist-symbols]` in `Cargo.toml`).

### Run locally

Serve the repository root (wasm will not instantiate from `file://`):

```bash
python -m http.server 8080      # or: npx serve
# open http://localhost:8080
```

`index.html` imports `./pkg/fractal_flame_wgpu.js`. Palette packs are
fetched relative to the page, at `assets/palettes/packs/<file>`
(`load_pack` in `src/resources/palettes.rs`), which a server rooted at
the repository finds in `assets/` itself.

### Deploy

Upload `pkg/`, `index.html`, `css/` and `js/` (the virtual-keyboard
bridge). The list and the checks after an upload are in
[RELEASE.md §5](RELEASE.md).

The build copies the palette packs to `pkg/assets/palettes/packs/`,
but `load_pack` requests `assets/palettes/packs/<file>` relative to the
page, not to `pkg/`. After a deploy, open a pack other than the
embedded starter pack in the Palette Library and check that it loads.

## JavaScript API — `WasmApi`

`src/wasm_api.rs` exports a `WasmApi` class (only in builds with the
`web-app` feature, which is on by default). It holds one config and
renders it headless — no canvas, no app — through the same
`renderer::render` the desktop CLI export uses
(`export_headless_wasm` in `src/app/export.rs`), so every render mode
works.

```javascript
import init, { WasmApi } from './pkg/fractal_flame_wgpu.js';
await init();
const api = new WasmApi();
api.load_config_json(JSON.stringify(config));       // a .fflame's JSON
const png = await api.export_png(800, 600, 256, false);
// width, height, iterations per thread, transparent [, premultiplied] -> Uint8Array (PNG, with metadata)
```

| method | does |
|---|---|
| `new WasmApi()` | an empty instance |
| `load_config_json(json)` | parse a `FractalConfig` |
| `load_config_from_url(url)` | read `?config=<base64 JSON>` or `?preset=<name>` from a URL |
| `load_preset(name)` | a built-in preset, by flame name (case-insensitive) |
| `get_preset_names()` | JSON array of preset names |
| `get_config_json()` | the loaded config, pretty-printed |
| `has_config()` | whether one is loaded |
| `export_png(w, h, ipt, transparent, premultiplied?)` | async; render and encode a PNG. `premultiplied` (optional, default false) is premultiplied alpha for a transparent export |
| `get_target_iterations()` / `set_target_iterations(n)` | the iteration target the next `export_png` renders to (set from the config's `max_iterations` on load) |
| `get_current_iterations()` / `get_progress()` | the **last export's** iterations, and that over the target; 0 before an export and after a load. An export is one awaited call, so there is nothing in flight to poll |
| `api_get_auth_status()`, `api_validate_token()` | online account state (cookie session) |
| `api_save_current_flame(name)`, `api_update_flame(id, name)`, `api_list_flames(page, per_page)`, `api_load_flame(id)`, `api_delete_flame(id)` | online flame storage, async ([ONLINE.md](main/ONLINE.md)) |
| `api_set_base_url(url)` | no-op, kept for old callers; the API URL is a compile-time constant |

The same file exports `PlanBench`, a test harness for Focused
Rendering's planner in the browser; `tests/visual/wasm/test_plan.py`
drives it.

## Gallery modules

The `wasm/` crates are separate, small modules — config JSON in, RGBA
out (`render`, `flame`, `escape`, `sim`), or script + seed in, config
out (`script`). Each is built with
`wasm-pack build --target web --release` in its own directory, does not
include the editor (they depend on this crate with
`default-features = false`, so `web-app` and `wasm_main` stay out), and
has its own `[profile.release]` tuned for size plus a `wasm-opt -Oz`
pass. Engines per module, from each `Cargo.toml`: `render` flame +
escape-time, `flame` flame, `escape` escape-time, `sim` simulation;
`script` links all three, to generate configs. API, sizes and an
end-to-end example: [wasm/README.md](../wasm/README.md).
How they are gated and released: [RELEASE.md §5](RELEASE.md).

## Browser constraints

Rules for code that runs on the web. Desktop drivers tolerate most of
these violations, so a mistake shows up only in a browser — often as a
black canvas rather than an error.

- **WebGPU only.** There is no WebGL fallback: the renderer is built on
  compute shaders. Chrome / Edge 113+ and Firefox 121+ work; Safari's
  WebGPU is experimental; mobile browsers are limited.
- **`textureSample()` only in uniform control flow.** Inside a branch
  that varies per invocation, use `textureLoad()`. Browsers enforce
  the WGSL uniformity rules strictly.
- **Storage buffers per shader stage are scarce.** A browser counts
  every storage buffer in a bind group layout against
  `max_storage_buffers_per_shader_stage`, used or not — a fixed layout
  of eleven broke every flame on a laptop Chrome that allows ten. So
  the flame compute layout holds only the bindings its generated WGSL
  uses (`gpu::pipelines::used_bindings`, read from the source because
  naga is not in the web build), and the device asks for the adapter's
  own limit (`src/gpu/device.rs`). Two tests hold this:
  `a_layout_holds_every_binding_its_shader_uses` and
  `a_flame_renders_within_a_browsers_storage_limit`.
- **Limits start from `downlevel_webgl2_defaults()`** on the web, with
  storage-binding size, buffer size, 2D texture size and storage-buffer
  count raised to what the adapter reports (`src/gpu/device.rs`).
- **No blocking device poll.** WebGPU cannot wait for the GPU, so
  nothing on the web may: GPU readbacks complete on a later frame, long
  work is a future polled a slice per frame (the Focused Rendering
  planner), and renderers that size their work from timing must not
  infer GPU cost from the gap between calls — on the web that gap is
  just the frame period (see the chunk sizing in
  `src/escape/renderer.rs`).
- **No directory listing.** Anything discovered by scanning a folder
  on desktop needs a build-time manifest on the web, as the palette
  packs have.
- **Panics abort.** `dist` sets `panic = "abort"`;
  `console_error_panic_hook` still prints the panic message to the
  console before the abort.

## Hunting a crash in the shipped build

A trap in the stripped `dist` bundle reaches the console as a bare
`RuntimeError: memory access out of bounds` (Chrome) or
`index out of bounds` (Firefox) at a byte offset, with no name.

1. **Read the offset before rebuilding.** An offset only means
   something against the module that produced it.
2. `python scripts/wasm-locate.py <offset|#index>` identifies the
   function containing it from `pkg/fractal_flame_wgpu_bg.wasm` alone,
   using the string literals the function references and the export
   names that survive `strip`.
3. To keep names in the module, rebuild with `--symbols` (same
   codegen); for a bug that is not optimization-dependent, `--debug`
   turns on assertions and overflow checks so it fails at its source.

The investigation that produced these tools — and the tooling errors
that gave confident wrong answers on the way — is recorded in
[wasm-load-crash.md](archive/projects/wasm-load-crash.md).

## Testing

- `python scripts/release.py check` compiles the library for
  `wasm32-unknown-unknown`, and at each single-engine feature
  combination the gallery modules link; it also runs the gallery
  crates' script-parity and renderer smoke tests.
- `tests/visual/wasm/test_wasm.py` renders the visual-suite configs in
  Chrome through `WasmApi.export_png`, driven by Selenium, and
  compares exact pixel hashes against `tests/visual/baseline/wasm/`.
- `tests/visual/wasm/test_plan.py` times the planner in the browser
  (`PlanBench`).

Details and commands: [TESTING-GUIDE.md §4](TESTING-GUIDE.md).

Older material — the original port, quick start and troubleshooting
notes — is in the archive: [README-WASM.md](archive/README-WASM.md),
[QUICKSTART-WASM.md](archive/QUICKSTART-WASM.md),
[WASM-TROUBLESHOOTING.md](archive/WASM-TROUBLESHOOTING.md),
[WASM-PROFILING.md](archive/WASM-PROFILING.md).

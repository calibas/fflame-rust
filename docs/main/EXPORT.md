# Export: PNG, Metadata, CLI and Video

Every engine exports through one function: `render` in
`src/renderer/render.rs`, which builds a renderer, hands the job to
`render_with`, and returns RGBA8 pixels. The flame chaos game, the
escape-time engine and the simulation engine are dispatched from
`render_with`, and all three share the flame renderer's tail — density
effects, tone map, colour effects, readback — so a still, a CLI batch, a
thumbnail, a texture and a video frame of the same config come out the
same. Flames too large for one GPU storage binding go to a second
engine, `HighResExporter`.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) — overview and data flow
- [RENDERER.md](RENDERER.md) — the flame pipeline
- [TONE-MAPPING.md](TONE-MAPPING.md) — the tone map, including its transparent modes
- [EFFECTS.md](EFFECTS.md) — density and colour effects
- [ESCAPE.md](ESCAPE.md), [SIMULATION.md](SIMULATION.md), [TERRAIN.md](TERRAIN.md) — the other engines
- [FOCUSED-RENDERING.md](FOCUSED-RENDERING.md) — flame deep zoom, which exports keep
- [ANIMATION.md](ANIMATION.md) — tracks, which video export plays
- [CONFIG.md](CONFIG.md) — `FractalConfig`, which every PNG embeds
- [UI.md](UI.md) — the export panel and the progress bar
- [TESTING-GUIDE.md](../TESTING-GUIDE.md) — the visual suite, which renders through the CLI
- [WASM.md](../WASM.md) — the web build and its JavaScript API

**Design record** (archived; the code is the authority where they differ):
- [unified-rendering-api.md](../archive/projects/unified-rendering-api.md) — `render` / `RenderJob`
- [unified-render-pipeline.md](../archive/projects/unified-render-pipeline.md) — the strategy picker and the sample-emit path
- [parallel-tiles.md](../archive/projects/parallel-tiles.md) — GPU tiles, and why serial tiles were not built
- [high-res-export.md](../archive/projects/high-res-export.md), [tiled-high-res-export.md](../archive/tiled-high-res-export.md)
- [supersampling-antialiasing.md](../archive/supersampling-antialiasing.md) — 2× antialiasing
- [png-export-fix.md](../archive/projects/png-export-fix.md)
- [animation-export.md](../archive/projects/animation-export.md), [video-loop-and-sim-timeline.md](../archive/projects/video-loop-and-sim-timeline.md)

**Code locations:**

| file | holds |
| --- | --- |
| `src/renderer/render.rs` | `render`, `render_with`, `RenderJob`, `RenderEngines`, `RenderOutput`, `RenderError`, `render_escape`, `render_escape_terrain`, `render_sim` |
| `src/app/export.rs` | `export_headless` (CLI routing), `export_headless_wasm`, `headless_device`, `ExportEngine` |
| `src/app/config.rs` | the app's custom-size and background exports |
| `src/app/mod.rs` | the app's viewport-size export |
| `src/export/mod.rs` | `histogram_size_bytes`, `route_custom_size_export`, `pick_strategy`, `ExportReporter` |
| `src/export/high_res.rs` | `HighResExporter` |
| `src/export/accumulate.rs` | the sample-scatter pass (`shaders/core/accumulate_samples.wgsl`) |
| `src/export/supersample.rs` | 2× antialiasing |
| `src/png_metadata.rs` | `PngMetadata`, encoding, reading, the strip switch |
| `src/main.rs`, `src/lib.rs` | the CLI |
| `src/animation/export.rs` | video |
| `src/ui/export_panel.rs`, `src/ui/export_status.rs` | the export panel; export status and the progress bar's reporter |
| `src/wasm_api.rs` | `WasmApi.export_png` |

---

## The export paths

| path | entry | device | engine |
| --- | --- | --- | --- |
| app, viewport size | the export panel's buttons (`src/app/mod.rs`) | the app's | none: reads back what the viewport has drawn |
| app, custom size or 2× AA | `App::export_custom_size` (`src/app/config.rs`) | the app's, or a device of its own in the background | `render`, or `HighResExporter` |
| CLI `export` | `export_mode` → `export_headless` | its own | `render`, or `HighResExporter` |
| video, CLI and app | `export_animation` (`src/animation/export.rs`) | its own | `render_with`, frame by frame |
| web API | `WasmApi::export_png` → `export_headless_wasm` | its own | `render` |
| thumbnails, the gallery, textures | `render` | the caller's | `render` |

---

## The render API

`src/renderer/render.rs`.

### RenderJob

`RenderJob::new(config, width, height)` with builder methods:

| field | default | meaning |
| --- | --- | --- |
| `target_iterations` | `None` (the config's `max_iterations`) | flame samples to render (`with_iterations`) |
| `iterations_per_thread` | 256 | dispatch depth (`with_iterations_per_thread`) |
| `burn_in` | 20 | (`with_burn_in`) |
| `transparent`, `premultiplied` | false | see *Transparency* |
| `engines` | `None` | engine state to continue (`with_engines`) |

**`RenderEngines`** is engine state a caller keeps alive across renders:
the simulation's field and step counter, the escape renderer (its
allocations and warm reference orbit), the escape terrain, and a
simulation's terrain. The video loop passes one so each frame continues
the previous one's run; every still path passes none and gets fresh
engines.

`RenderOutput` carries the pixels, their size, `total_iterations` (the
flame's samples; for a simulation, the step count; for escape, the
per-pixel `max_iter`), `render_time_ms`, and `frame_coverage` (1.0 unless
the flame's auto exposure measured it). `RenderProgress` reports
progress and can cancel; `NoProgress` ignores both.

### render

`render(device, queue, job, progress)`:

1. Refuses, before allocating anything, a size the device's limits
   cannot hold: `EscapeTerrain::allocation_error` for an escape terrain,
   `EscapeRenderer::allocation_error` at the supersample factor it would
   actually use, `SimRenderer::allocation_error` for a simulation. Each
   returns `RenderError::TooLarge` with the reason.
2. Builds a `FlameRenderer` inside an out-of-memory error scope. Escape
   and Simulation configs get `FlameRenderer::for_tail`, without the
   flame's per-pixel histograms and accumulators (64 bytes a pixel they
   never read).
3. Turns sticky shaders off: an export renders exactly the specialised
   shader its reproducibility depends on.
4. Calls `render_with`, then destroys the renderer (on WebGPU dropping
   frees nothing).

### render_with

`render_with(renderer, device, queue, job, progress)` renders into a
caller-owned renderer of the right size. It dispatches on the mode:

- **Escape** → `render_escape`, which hands a terrain config to
  `render_escape_terrain`.
- **Simulation** → `render_sim`: seed (or continue), step to `sim.steps`
  in progress batches of 512, colour, draw the terrain if it is on.
- **2D and 3D flames** → the chaos game below.
- A build without the escape or simulation engine returns
  `RenderError::EngineMissing` for that mode rather than rendering the
  config's flame.

The flame path: `load_config`; for a solid flame with shadows, six
warm-up batches to measure the attractor's bounds and refit the shadow
maps, then a reset; then dispatches of 128 workgroups × 64 threads ×
`iterations_per_thread` until the target is reached, accumulated every
four dispatches, counting the samples `compute_pass` reports as plotted.
Then the tail: the transparent mode if asked, the solid density
renormalisation, the shade pass and depth of field, density effects,
the tone map, colour effects, and the readback (`read_fractal_pixels`).
The escape and simulation paths run the same tail on their own
`Rgba32Float` image.

### RenderError

| variant | when |
| --- | --- |
| `EngineMissing(engine)` | the config's mode needs an engine this build does not have |
| `TooLarge(reason)` | a size the device's limits refuse, found before allocating — a texture side, a binding, a simulation grid |
| `OutOfMemory(what)` | a GPU allocation failed. Caught by an error scope, so the export fails rather than writing an all-black PNG |
| `Cancelled` | the progress callback cancelled |
| `PixelReadFailed`, `NoPaletteFound` | readback and palette failures |

`unknown_names(config)` lists names the config uses that this build has
no registry entry for (an escape formula, colouring or lens; a
simulation model or colouring). The engines fall back to a default for
these; the CLI refuses to export such a config instead, so an unknown
formula does not come out as a Mandelbrot with exit code 0.

---

## Transparency

Transparent export is a mode of the GPU tone map
(`FlameRenderer::set_transparent_mode`, `shaders/tonemap.wgsl`), applied
to the `Rgba32Float` image the engine produced; `read_fractal_pixels`
then reads the RGBA8 result.

| mode | when | output |
| --- | --- | --- |
| 0 | opaque | composited over the background colour |
| 1 | a transparent flame | straight alpha built so a standard flatten over black (rgb·a) reproduces the opaque-over-black export exactly: alpha is the over-black colour's largest channel, rgb is that colour divided by it |
| 2 | `premultiplied` | the over-black colour as rgb and the fractal's opacity as alpha, for premultiplied compositing pipelines |
| 3 | a transparent escape or simulation picture | straight alpha: the picture's colour, and its own coverage as alpha |

The app offers *Export with Background* and *Export Transparent*, with a
*Premultiplied alpha* checkbox; the CLI has `--transparent` and
`--premultiplied`. A viewport-size transparent export re-runs the tone
map (and colour effects) on the live image in transparent mode, reads
it, and restores the normal tone map.

`FlameRenderer::capture_pixels`, `capture_from_accumulation_buffer` and
`capture_from_tonemap_render` (`src/renderer/compute_kernel.rs`) are an
older CPU path, marked `#[allow(dead_code)]` and called by nothing.

---

## 2× antialiasing

`--supersample` in the CLI and *2× Antialiasing* in the app (desktop
only) antialias each engine its own way (`supersample_plan`,
`src/export/supersample.rs`):

| engine | what renders | then |
| --- | --- | --- |
| 2D / 3D flame | 2W × 2H, with `filter_radius` × 2 and `max_iterations` × 4 (`scale_config_for_supersample`), so each doubled pixel has a 1× pixel's sample density and brightness matches | boxed down |
| escape (plane, solid) | the output size, with the config's own `escape.supersample` doubled (at most 8) | nothing to box |
| escape terrain | 2W × 2H | boxed down |
| simulation | 2W × 2H, with a viewport-bound grid pinned to the output's grid (`round(output × scale)`, at least 16), so the doubled render resolves the same field rather than simulating twice the cells | boxed down |

The flame is downsampled **after** the tone map: the tone map is
logarithmic, and averaging densities before it would shift brightness by
density. `downsample_2x_firefly` box-filters each 2×2 quad, first
scaling down any sample whose luminance (r + 2g + b) exceeds 4 × the
quad's median + 64, so a single lucky bucket does not survive as a
bright dot; alpha is averaged unclamped. The PNG's metadata embeds the
original config, not the scaled copy.

In the app, 2× AA always goes through the custom-size machinery, even at
viewport size, since the viewport's own image cannot be supersampled.

---

## Routing flames by size

A flame histogram is 16 bytes a pixel, 20 with solid rendering
(`histogram_size_bytes`). The direct path needs it in one storage
binding, so routing compares it with the adapter's own
`max_storage_buffer_binding_size` — probed from the adapter by the CLI
(`probe_max_binding_size`), read from the app's device in the app — not
WebGPU's 128 MB floor. Desktop adapters commonly report far more, so 8K
can take the direct path.

**CLI** (`export_headless`):
- Escape and Simulation always go to `render`. Above one binding they
  are refused: the shared tail allocates its histogram at the export
  size whatever the mode.
- A flame goes to `HighResExporter` when the histogram exceeds the
  binding, otherwise to `render`. `--engine flamerenderer` or
  `--engine highres` forces one (for parity testing; forcing the direct
  path above the binding logs a warning and will likely fail to
  allocate).

**App** (`export_custom_size`, rule in `route_custom_size_export`):
- Escape and Simulation: `Direct` (`render`), or `TooLarge` above one
  binding, reported with a toast.
- A flame: `HighRes` — in the background — when the histogram exceeds
  the binding **or** `max_iterations` exceeds 250,000,000
  (`BACKGROUND_EXPORT_ITER_THRESHOLD`), so a long render shows progress
  instead of freezing the window; otherwise `Direct`, synchronously on
  the app's device.
- A terrain (escape or simulation) renders through `render` in the
  background, on a device of its own.
- Before a synchronous escape export the app frees its viewport escape
  renderer and escape terrain, which are rebuilt afterwards; the
  simulation renderer is kept, because its field is the user's run.

The export panel bounds the size: each side at most the device's
maximum texture size (halved with 2× AA), and a pixel budget — 400 MP on
desktop; on the web a 3.3 GB budget divided by a per-pixel cost that
grows with the spatial filter, solid rendering, lighting and depth of
field (quartered with 2× AA).

---

## HighResExporter

`src/export/high_res.rs`, flames only. Instead of plotting into a
histogram the iteration shader emits samples (the sample-emit build of
the unified template), which are accumulated in tiles.

`pick_strategy` (`src/export/mod.rs`):

| strategy | when |
| --- | --- |
| `Direct` | the whole histogram fits one binding |
| `ParallelTiles` | it does not, but fits one buffer (`max_buffer_size`) |
| `SerialTiles` | it does not fit one buffer |

Tiles are horizontal slices the full width of the image, each sized to
one binding and with a row count rounded to keep every tile's byte
offset a multiple of `min_storage_buffer_offset_alignment`.

What `HighResExporter::new` does with each:

- **`ParallelTiles`** — one concatenated tile-histogram buffer on the
  GPU; each dispatch's samples are scattered into every tile by
  `accumulate_samples.wgsl`, so the chaos game runs once (1× iteration
  cost). Only when the total fits both `max_buffer_size` and a 2,560 MB
  cap (`GPU_HISTOGRAM_BUDGET`): drivers report buffer limits beyond the
  free memory, and a failed allocation is fatal. Larger goes to the CPU
  path.
- **`SerialTiles`** — the CPU path: samples are read back and
  accumulated into an f64 histogram in main memory with rayon. Serial
  GPU tiles (re-running the chaos game per tile) were not built; the CPU
  path is faster ([parallel-tiles.md](../archive/projects/parallel-tiles.md)).
- **`Direct`** — reached only by the app's long-render route: the whole
  image as one GPU tile, which is leaner than the direct renderer and
  fits beside the app's own device.

Then the tone map, on the same `tonemap.wgsl`. Above a 1 GB
accumulation (about 8000²) it runs in strips of at most 256 MB, so the
`Rgba32Float` accumulation is never whole on the GPU. Density effects
need a full-resolution set of about 36 bytes a pixel after the histogram
is freed; above a 4 GiB budget (about 119 MP, ~10,900²) they are skipped
with a warning. Colour effects always run.

(The module's own header comment still describes the CPU-histogram path
as the only one.)

**Focused Rendering (cylinder targeting):** a 2D config with targeting on
is planned ONCE, for the whole export view, in `HighResExporter::new`
(`renderer::compute_kernel::plan_view`, the renderer's own decisions).
Every dispatch emits samples for the whole image and the tiles only
scatter them, so one plan serves every tile. The table, the shader's
targeting flags and the tone map's iteration scale are the renderer's
for the same view, so a long in-app render routed here comes out as the
viewport shows it. Gate: `a_targeted_export_is_the_targeted_render`.

---

## Escape and simulation exports

**Escape.** One engine at any size up to the tail's binding limit; there
is no escape-native tiling. The config's `escape.supersample` is always
applied, so a saved file renders as it was saved, and 2× AA doubles it.
See [ESCAPE.md](ESCAPE.md).

**Simulation.** What an export draws depends on the grid
([SIMULATION.md](SIMULATION.md)):

- A **Fixed** grid is the same field at any output size; the resolve
  scales it.
- A **viewport-bound** grid is `round(output × scale)` cells (scale
  clamped to 0.125–4), so an export at a size other than the viewport's
  simulates a different grid — a different picture.
- An export renders through `render`, which seeds and runs exactly
  `sim.steps` from the seed. At `steps = 0` (uncapped in the panel) that
  is the seed itself; the panel says so.
- The **viewport-size** export instead reads the live picture, and
  records the live run's step index as `sim.steps` in the PNG's config,
  so the file re-renders as saved.
- `SimRenderer::allocation_error` refuses a grid or output past the
  device's texture limit or a 4 GiB budget.
- A terrain renders in the background, on its own device.

---

## Background exports and progress

A terrain export, a `HighResExporter` export and a video each run on a
thread with a GPU device of their own: the terrain export and the CLI
through `headless_device` (`src/app/export.rs`), `HighResExporter::new`
and `export_animation` through their own device setup. Each asks for the
adapter's real binding, buffer and texture limits rather than WebGPU's
defaults, and for `FLOAT32_FILTERABLE` when the adapter has it
(`headless_device` and the video's device also for `TIMESTAMP_QUERY`;
`HighResExporter`'s does not ask for it).
For a PNG the destination is chosen first, on the UI thread: a native
save dialog opened from a background thread cannot come to the front on
Windows.

Progress goes through `ExportReporter` (`src/export/mod.rs`): a
fraction and a detail line. `UiReporter` (`src/ui/export_status.rs`)
writes it into the shared `ExportStatus`, which drives the menu bar's
progress bar and ends in a toast; `ConsoleReporter` prints one rewriting
line for the CLI. While `ExportStatus.active` is set the export buttons
are disabled, so one export runs at a time. There is no cancel control
in the app.

### GPU memory during an export

A background export -- a terrain's PNG, a high-res PNG, a video -- runs
on a device of its own, so it competes with the viewport for the same
card. Three things keep the two inside it (measured on a 6 GB GTX 1660
SUPER, a seahorse terrain at a 3x3 prefilter, detail 8, 8x AA):

- **The viewport freezes** (`App::freeze_viewport`, `app/gpu_updates.rs`)
  while `export_status` is active: the frame renders nothing, so its last
  picture stays on screen, and the engines that rebuild themselves give
  their memory back -- the escape renderer, the escape terrain (its
  section atlas alone is over 1 GB) and a simulation's terrain. They are
  made again on the first frame after, and render from scratch: a
  terrain's sections fill in again. The flame renderer stays (it is what
  the viewport shows), and so does a simulation, whose run cannot be
  remade but by running it.
- **A tail-only renderer** (`FlameRenderer::for_tail`): an escape
  picture, a terrain and a simulation use the flame renderer only for the
  tonemap of their own image, so `render()` builds them one without the
  flame's histograms and accumulators (64 bytes a pixel). And no renderer
  makes the solid shade pass's four full-frame textures (64 bytes a pixel
  more) until a lit solid first runs it.
- **A terrain export fits its section cap to the card**
  (`fit_export_sections`): up to 256 sections where the memory is there
  beside the rest of its peak, against the viewport's 64 -- the cap is
  what decides how fine a large export's ground is. Discrete cards only;
  unified memory, the web and software renderers keep 64.
- **A terrain's section renderer** gives its per-pixel memory back
  between bursts of sections (`EscapeRenderer::release_pixels`) -- before
  an export's still is drawn, and in the viewport once the ground is in.

Measured peaks above the idle card: the 8K export of that terrain (a 4K
export at 2x AA) went from out of memory at 5.1 GB -- alone, before the
viewport's share -- to 3.2 GB; the 4K one from 3.4 to 2.1 GB. Beside
it, the frozen viewport frees its own terrain: an atlas of up to 64
sections is about 1.1-1.3 GB by its sizes (not measured in the app).

---

## CLI

The desktop binary is also the CLI (`src/main.rs`). Release builds on
Windows are GUI-subsystem and re-attach to the parent console so the
CLI can print.

### export

```bash
FractalArtEditor export -i config.fflame -o out.png -w 3840 -H 2160
FractalArtEditor export -i tests/visual/configs -o tests/visual/current --category variations
FractalArtEditor export -i scene.flame -o out/ --transparent --supersample
```

| flag | default | meaning |
| --- | --- | --- |
| `-i`, `--input` | required | a `.fflame` file (one config), a `.flame` XML file (each `<flame>` in it), or a directory (every `*.fflame` in it, not `.flame`, ordered by flame name) |
| `-o`, `--output` | required | a PNG path for a single config; otherwise a directory, created if missing |
| `-w`, `--width` | 1920 | the config's `image_size` is not used |
| `-H`, `--height` | 1080 | |
| `-c`, `--category` | none | written into the PNG metadata |
| `--iterations-per-thread` | 1024 | `DEFAULT_EXPORT_ITERATIONS_PER_THREAD`; the app uses 256. A depth-versus-breadth trade-off, not a quality setting (see the constant's comment) |
| `--transparent` | off | transparent PNG |
| `--premultiplied` | off | premultiplied alpha, with `--transparent` |
| `--strip-metadata` | off | no metadata chunks |
| `--supersample` | off | 2× antialiasing |
| `--engine` | `auto` | `auto`, `flamerenderer` (alias `flame`), `highres` (alias `tiled`) |
| `--dump-shader` | off | write the flame shader to `debug_shader_2d.wgsl` / `debug_shader_3d.wgsl` in the working directory |

In batch mode each PNG is named after the flame (lower case, spaces as
underscores; a repeated name gets `_2`, `_3`, …). Locally installed
plugins are loaded first, and a refused plugin is reported on stderr.
The CLI compiles flame data into the shader as constants
(`enable_inlined_constants`). A config naming something the build lacks
is reported and skipped; a config that fails is reported and the batch
goes on. The exit code is non-zero if any config was not exported, and
on a missing input or unwritable output.

### export-animation

```bash
FractalArtEditor export-animation -c config.fflame -a animation.anim -o out.mp4 \
  -w 1920 -H 1080 --fps 30 --video-codec h265
```

| flag | default |
| --- | --- |
| `-c`, `--config` | the base config; without it, the `.anim`'s embedded `base_config` (an error if it has none). Given with one, `-c` wins |
| `-a`, `--animation` | required: the `.anim` file |
| `-o`, `--output` | required |
| `-w`, `--width` / `-H`, `--height` | 1920 / 1080 |
| `--fps` | 30 |
| `--iterations-per-thread` | 1024 |
| `--video-codec` | `h265`; also `h264`, `vp9` (WebM). An unknown name falls back to H.264 |
| `--hw-accel` | `none`; `nvenc`, `qsv`, `amf`, `videotoolbox`; falls back to software when the encoder cannot do the codec |
| `--video-quality` | 18 (CRF, 0–51, lower is better) |
| `--audio` | none: an MP3, WAV, FLAC or OGG file to mux in |
| `--audio-offset`, `--audio-fade-in`, `--audio-fade-out` | 0 seconds |
| `--audio-bitrate` | 192 kbps |

### list-encoders

Prints the FFmpeg encoders and hardware acceleration found on this
machine.

### generate

Runs a Rhai script headlessly and writes the `.fflame` it produces:
`--script`, `--output`, `--seed` (any integer, reduced modulo 2^64),
`--base` (a starting config for modifier scripts), `--set key=value`,
`--list-params`. See [SCRIPTING-GUIDE.md](SCRIPTING-GUIDE.md).

---

## PNG metadata

Every exported PNG carries text metadata unless stripping is on
(`src/png_metadata.rs`). `PngMetadata::from_app_state(width, height,
total_iterations, render_time_ms, iterations_per_thread, speed_factor,
config)` builds it; `encode_png_with_metadata` writes the chunks before
the image data.

| chunk key | contents |
| --- | --- |
| `Software` | `Fractal Art Editor v<version>` |
| `GitHash`, `Branch`, `BuildDate`, `Platform`, `RustcVersion`, `BuildProfile` | build information; the profile names the binary (`dist`, `release`, …) |
| `Resolution` | `WxH` |
| `Iterations` | the render's `total_iterations`: flame samples (under 2× AA, the quadrupled count that was rendered), a simulation's step count, an escape picture's `max_iter` |
| `RenderTime` | milliseconds, as `12.34ms` |
| `IterationsPerThread`, `SpeedFactor` | |
| `ConfigChecksum` | `sha256:` and the hex SHA-256 of the `Config` text |
| `Config` | the complete `FractalConfig` as JSON |
| `BackgroundColor`, `Exposure`, `Gamma` | |
| `ToneCurve` | whether the tone curve has more than two points |
| `RenderMode` | `2D`, `3D`, `Escape` or `Simulation` |
| `Projection` | `Orthographic` or `Perspective{strength:…}` in 3D; `N/A` otherwise |
| `ShaderVariations` | how many variations the flame uses |
| `TestName`, `TestCategory` | the CLI's test labels (`export --category`); written only when set |

The `Config` chunk is the canonical saved form,
`FractalConfig::to_json_value` (compact, with its `version`), so it
reloads through `FractalConfig::from_json` exactly as a `.fflame` does.
PNGs exported before that carry the plain serde derive with no
`version`; `from_json` dates those by their top-level `render_mode` (see
[CONFIG.md](CONFIG.md), *Versioning*).

**Stripping.** The privacy switch is one process-global flag checked by
the encoder itself, so no export path can forget it:
`set_strip_metadata` is called from `SystemSettings` at startup, when
the export panel's *Strip metadata* box changes (`SystemPngStripMetadata`),
and from `--strip-metadata` in the CLI. With it on the PNG has no text
chunks at all.

**Reading.** `read_png_metadata(png_bytes: &[u8])` returns a
`PngMetadata`. It parses the build fields, `Resolution`, `Iterations`,
`RenderTime`, `IterationsPerThread`, `SpeedFactor`, `ConfigChecksum`
(either `sha256:` or the older `hash:` prefix), `Config`, `ToneCurve`,
`RenderMode`, `Projection` and `ShaderVariations`; the background
colour, exposure and gamma are not parsed back and read as fixed
placeholders. It accepts the old product name, `Fractal Flame Renderer
v…`, too. `scripts/png_metadata.py` lists the chunks of any PNG and
extracts the config (`--config`, `-o out.fflame`) without a build.

```rust
use fractal_flame_wgpu::png_metadata::read_png_metadata;
use sha2::{Digest, Sha256};

let bytes = std::fs::read("output.png")?;
let meta = read_png_metadata(&bytes)?;
let hex: String = Sha256::digest(meta.config_json.as_bytes())
    .iter().map(|b| format!("{b:02x}")).collect();
assert_eq!(hex, meta.config_checksum);
```

---

## Video export

**Location:** [src/animation/export.rs](../../src/animation/export.rs)
— `export_animation`

Raw RGBA is piped straight to FFmpeg's stdin: no PNG encode, no disk
round-trip, no temp files. FFmpeg must be on `PATH`.

The same function serves the CLI's `export-animation` and the app's
export dialog. **There is one video loop, and this is it.** Each frame is
rendered by `render_with`, so a pass added to the render tail reaches
video for free; a second loop with its own copy of the tail is how
density effects, solid depth of field and the simulation's gating once
went missing from in-app video. Do not add one.

### What a frame is

Per frame: evaluate the animation tracks at that time, apply them to a
copy of the config (`ConfigManager::apply_value_detached`, the editor's
own setters), call `render_with`, hand the pixels to the writer thread.
Which passes run is decided by reading that frame's config, exactly as
a still render decides — that is what makes the video and a PNG of the
same config identical.

Two things a still render does not need, and the reason this is a loop
rather than a call in a `for`:

- **`RenderEngines`** — engine state that outlives the frame. The
  simulation's field continues its run rather than restarting from the
  seed each frame (restarting per frame is quadratic in the step count
  — a 100-frame ramp to 20,000 steps costs 1,000,000 steps instead of
  20,000, measured at 18.3 s against 4.6 s). The escape renderer keeps
  its allocations and its warm reference orbit. A still path passes no
  engines and gets a fresh one per render, unchanged.
- **A writer thread**, fed through a 16-frame bounded channel, so a
  CPU-starved encoder never stalls the GPU and memory cannot grow
  without bound.

### Device features

The export device requests `FLOAT32_FILTERABLE` and `TIMESTAMP_QUERY`
when the adapter advertises them, matching every other export path.
This is not optional decoration: density effects bilinear-sample the
`Rgba32Float` accumulation, and without that feature
`run_density_effects` **skips them silently** — by design, since on an
adapter that genuinely lacks it the bind group would be invalid. A
device that asked for less would drop every density effect from the
video on GPUs that support them perfectly well.

### Progress and cancellation

`FrameProgress` maps a generator's own progress — the simulation
stepping toward this frame's target, an escape render settling — onto
the overall bar, so a long first frame is not mistaken for a hang, and
carries cancellation *into* a frame rather than only between frames.

It is **rate-limited to one report per 100 ms**, and that matters: the
flame loop reports once per dispatch, about 120 times for a default
billion-iteration frame, and `ConsoleReporter` prints and flushes on
every call. Unlimited, that cost 505 ms a frame against 415 ms with no
reporting at all — an 18% tax for a status line nobody can read at
that rate.

### Simulation video

`Sim.Steps` is an ordinary animatable track, and a ramp on it **is**
the simulation progressing. The state at time *t* is a function of *t*,
not of how many frames preceded it, so the exporter advances the grid
to each frame's step count (`render_sim` through
`SimRenderer::advance_steps`, continuing the run `RenderEngines` keeps).
A track that counts DOWN is rendered correctly — every frame reseeds and
re-runs to its own target — at a cost that grows with the square of the
step count. See
[video-loop-and-sim-timeline.md](../archive/projects/video-loop-and-sim-timeline.md)
D9 for the ascending-order trick that would make it linear, which is
recorded and not built.

One caveat inherent to a non-invertible rule: a re-run uses the
parameters as they are *now*, so a reversed step track retraces the
forward pictures exactly only when the model parameters are constant.

---

## The web build

**`WasmApi.export_png(width, height, iterations_per_thread, transparent, premultiplied?)`**
(`src/wasm_api.rs`) renders the loaded config through
`export_headless_wasm` (`src/app/export.rs`): a WebGPU device of its own
with the adapter's binding and storage-buffer limits, `render`, then
`device.destroy()` so the export's memory is freed at once rather than
whenever the JavaScript garbage collector runs. The PNG carries the same
metadata. It renders to the API's iteration target: the config's
`max_iterations` on load, or `set_target_iterations`. The optional
`premultiplied` argument is the CLI's `--premultiplied` (default
straight alpha). See [WASM.md](../WASM.md).

**In the web app**, the viewport-size export reads the live picture
back asynchronously and downloads it. The custom-size export runs its
own render loops in `src/app/mod.rs` — the flame chaos game, the escape
chunk loop and `SimRenderer::render_still` — rather than calling
`render_with`, so a stage added to `render_with` does not reach it
automatically. 2× antialiasing is not offered on the web.

---

## Limits

- PNG output is RGBA8 only. EXR/HDR is not implemented.
- Video is MP4 (H.264, H.265) or WebM (VP9) through FFmpeg.
- An escape or simulation export cannot exceed one storage binding of
  flame histogram at its render size (the shared tail's allocation).
- The app's custom-size export of a 2D escape or simulation picture is
  synchronous and blocks the window while it renders.

# WASM modules: what they lack, and loading engines on demand

Research 2026-10-23; nothing built. The user's words: "I want to review the
WASM modules. We've added things like flame variations used in Simulation
and Escape-time modes, and I think we skipped them. Could we do something
like the flame variations themselves are their own WASM module, and it's
only downloaded as needed? ... Something I'd like to do is support nearly
everything in WASM (minus things like FFMPEG exports) but only have the
required modules downloaded as required for the current config."

Measured on the `dist` build of 2026-10-22 (`pkg/`) and a `--symbols`
build of 2026-10-23 (`pkg-names/`, the same codegen with names kept);
the gallery modules' `pkg/`s date from 2026-09-29 and were not rebuilt.

## 1. The gallery modules silently drop flame variations

`wasm/escape` builds with `engine-escape` alone, `wasm/sim` with
`engine-sim` alone; without `engine-flame` the registry holds `linear`
(`variations/defs/mod.rs`, `ALL_VARIATIONS`), and a name the registry
lacks contributes nothing -- no error (`storage/mod.rs`: "a missing
variation is a weight that contributes zero"; the gallery's
`render_impl` never calls `unknown_names`, which checks only the lens
anyway). `wasm/README.md`'s "an escape or simulation gallery has no use
for [the catalogue]" is no longer true:

| Feature | Config fields | In the escape / sim module |
|---|---|---|
| Camera lens | `escape.lens`, `lens_amount`, `lens_params` | No lens (`lens_on` needs the registry entry); `linear` and Rhai lenses work |
| Escape transformations (Plane / Orbit / Iteration) | `escape.transforms[]` | Dropped (`drawn_transforms`, `map_usable`) |
| Mode D, the Flame Attractor (2D and solid) | `escape.formula` = `ifs_flame` / `ifs_flame_3d`, reading `config.flame` | **Blank** unless every variation is `linear`: the analysis refuses an unknown name (`NotAffine::Variation`), `pack_for` returns None, the renderer draws nothing. The inverse kernels themselves are in the binary |
| A Rhai solid's `vary("name", v)` of a built-in | `escape.custom_formula` (e.g. `solid-inverted-menger.rhai`) | **A Mandelbrot**: the compile fails, `custom::ensure` logs it, `get_formula` falls back to `FORMULAS[0]` |
| Rhai custom variations | `flame.custom_variations` | Work in the escape module; **lost in the sim and flame modules** -- `variations::custom` is gated on `engine-escape` |
| Simulation layers warped by the flame | `sim.use_transforms` | Affine only: the variations are skipped (`build_layer_map`) |
| Escape textures from a simulation recipe | `escape.texture` | Missing in the escape module: the recipe renders through `render()`, which returns `EngineMissing("simulation")`; a warning only |

No gallery test covers any row (the fixtures' flames are `linear`; the
crates' smoke tests check that dropped engines are gone, not what a config
needing them does). `every_registered_inverse_names_a_variation`
(`variations/inverse.rs`) is not engine-gated and would fail without
`engine-flame`, but the main crate's tests never run that way.

The app's own trigger for fetching variations a config needs,
`missing_variations_in`, scans the flame's transforms only -- not the
lens, the escape transformations or a solid's `vary` -- which matters
once variations are fetched (section 4).

## 2. The web app against the desktop

Video export (ffmpeg) aside, by intent:

| Feature | Web app |
|---|---|
| Signal file load / save | Silent no-op (no wasm branch, `ui_handlers.rs`) |
| HDRI skies | None ("the web build has none yet", `escape/environment.rs`) |
| CJK fonts | Not embedded (desktop loads Noto JP/SC from disk) |
| Deep zoom's orbit worker, progressive refinement, disk orbit store | Synchronous; no store; no cache setting |
| Detect Period (escape) | Absent; Center Minibrot synchronous on a small budget |
| Focused Rendering's planner | A polled task, 8M replays (desktop: a thread, 64M) |
| naga: WGSL validation of custom sources, derived forward bounds | Skipped; hand bounds only |
| 2x supersample export; terrain background export | Hidden; absent |
| Texture and thumbnail caches | Session memory only |
| Saved credentials | Cookie only |
| VSync toggle; device-lost rebuild | Fifo only; none |

Everything else -- terrains, the path tracer, audio playback and capture,
plug-ins (localStorage), scripts, custom-size PNG export (on the app's
device) -- is in the web app.

## 3. What the web app's download is made of

The shipped module: **19.9 MB raw, 6.5 MB gzip** (`pkg/`, 2026-10-22).
The all-engines gallery renderer, with no editor: 4.6 MB / 1.15 MB.
Attributed from the names build (`--symbols`, the same code; each group
compressed on its own, so the gzip column overshoots the whole a little):

| Part | Raw | Gzip | Needed when |
|---|---|---|---|
| Generic code (`core`, `alloc`: monomorphised iterators, formatting, ...) | 2.8 MB | 0.77 MB | Always |
| egui, epaint, egui_dock | 1.2 MB | 0.34 MB | Always |
| Fonts (egui's four defaults) | 1.4 MB | 0.78 MB | Always (the two emoji fonts could wait) |
| Font rasterising (skrifa, read_fonts, vello_cpu, ...) | 0.7 MB | 0.23 MB | Always |
| Our UI, config, scene, app, renderer, other | 2.8 MB | 0.99 MB | Always (the flame engine and the editor) |
| **Formula languages and scripting**: the `rhai` crate, `escape::lang` (the Rhai / UF / Fractint compilers), `script`, the bundled examples | 3.1 MB | **0.78 MB** | A custom formula, solid, colouring or variation; the Formula Editor; the Scripts panel |
| **Escape engine** (less `lang`) and most of the other WGSL | 0.6 MB + ~1 MB | **~0.5 MB** | Escape mode |
| **Audio decoding** (symphonia, rustfft, cpal) | 0.55 MB | **0.20 MB** | An audio file, capture, an audio signal |
| **Variation WGSL** (647 variations) | 1.47 MB | **0.18 MB** | Each variation a config uses |
| Simulation engine | 0.13 MB + shaders | ~0.08 MB | Simulation mode |
| i18n (rust-i18n's generated tables) | 0.38 MB | 0.03 MB | Always |

So the parts a config may not need come to about **1.7 MB of the 6.5 MB
gzip**, a quarter. The variation catalogue the question started from is
the smallest of them: WGSL is repetitive text and compresses 8x.

## 4. Ways to load on demand

**A. Variations as data, fetched per variation.** The path exists: the
API serves a `VariationDownload` (2D, 3D and init WGSL, parameters,
features, state count, aliases, plot emits; `api/types.rs`),
`VariationInfo::from_download` makes a registry entry and
`register_from_api` registers it, cached in localStorage on the web
(`app/variation_fetch.rs`, `Provenance::Api`). What would change:
- Built-ins served the same way (from `pkg/assets/variations/` or the
  API), with an index (names, categories, parameters) in the binary or
  fetched first, so the variation picker can list what is not loaded.
- `missing_variations_in` covering every use: the lens, the escape
  transformations, a solid's `vary`, `sim.use_transforms`, subflames.
- A config waits for its variations before its shader is built (the
  fetches already run in the background, `app/variation_fetch.rs`).
- What stays Rust, keyed by name, and so stays in the binary: the 26
  inverse kernels (mode D), the forward bounds, the name-gated WGSL
  libraries (fract_*_wf, noise, voronoi, polyhedra, polychora), the
  `synth` / `mondrianomies` specializers, `subflame_wf`, the curated
  lists (`SLOW_PARAMS`, lens lists, `GOOD_VARIATIONS`), and the `.flame`
  importer's unit conversions.

Saves 0.18 MB gzip in the web app; its real value is in the gallery
modules, which could then render any config without carrying the
catalogue.

**B. The formula languages as a separate module.** Compiling a Rhai,
Ultra Fractal or Fractint source is CPU-only: source text in, WGSL and
parameter definitions out. A second wasm instance (as `fflame-script`
already is for scripts) loaded on the first custom source, called
synchronously once loaded (two instances on one page call each other
through JS without awaiting). The work is the boundary: what the app
reads from a compiled entry would have to be serialisable (a `FormulaDef`
/ `IfsDef` / `VariationDef`'s data), and the CPU-side uses of a program
-- the interpreter for a solid's ball (`custom::solid_ball`), the Rhai
variations' CPU runs, the Scripts panel's runs -- become calls into it.
0.78 MB gzip.

**C. wasm-split, for engine code.** The only way to load Rust code that
shares the app's memory and GPU device: one module split into chunks that
share its memory and function table, fetched when a function marked as a
split point is first called. It exists as `wasm-split-cli` in the Dioxus
repository (Dioxus 0.7 ships it; started as a prototype by jbms,
wasm-bindgen issue #3939, which the wasm-bindgen maintainers declined to
take in). Requirements and limits read from its source: a build with
`--emit-relocs`, symbols and LTO; the pre-bindgen module for its
relocations; split points are async functions; only the first data
segment can move; its shared-chunk step is a no-op as written; a user
reported build errors in February 2026. For us the larger cost is
structural: everything in a chunk must be reachable only through its
split points, and the escape engine is reached from `render_with`, the
frame loop and several panels -- an engine interface the app calls
through, installed when the chunk loads. Candidates: the escape engine
(~0.5 MB), audio (0.2 MB), later the terrain and the path tracer.

**D. Separate engine instances (not viable).** A second wasm module with
its own wgpu cannot share the app's `GPUDevice` (wgpu has no way to adopt
one on the web), so its picture would cross through a readback; and Rust
objects cannot cross instances. Fine for CPU-only work (B), not for an
engine.

## 5. Recommended order

1. **Make the gallery modules correct** (small, and a bug today): fetch
   the variations a config uses (A, from the same files) in the escape
   and sim modules -- or, as a stopgap, build them with `engine-flame`;
   move `variations::custom` off the `engine-escape` gate; say what a
   config needs that the module lacks instead of rendering without it;
   tests for each row of section 1.
2. **Variations as data (A)** in the web app, with `missing_variations_in`
   covering every use. Small saving, but it is the same machinery as 1
   and makes the catalogue extensible from the server.
3. **The language module (B)**: the largest single saving that does not
   need experimental tooling.
4. **A wasm-split spike** on audio (the cleanest boundary) to see whether
   the tool works with our build at all, before any engine refactor.

## Decisions for the user

- Is a quarter of the download (1.7 of 6.5 MB gzip) worth B and C's
  complexity, or is fixing the gallery modules (1) and variations as data
  (2) enough?
- For 1: fetch variations in the gallery modules, or just build them with
  the catalogue (+0.2-0.4 MB gzip each)?
- Where built-in variation files are served from: `pkg/assets/` beside the
  module (offline-capable, versioned with it) or the API.

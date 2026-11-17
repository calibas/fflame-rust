# Effects

**Overview:** Post-processing passes in two chains around the tone map.
**Density effects** run before it, on the HDR image in the
accumulator's layout (`rgb` colour, `a` density). **Colour effects** run
after it, on the display image. Each effect is a fullscreen fragment
shader named by a string, looked up in a registry, and stored in the
config as an ordered list of instances. An empty or fully disabled list
costs nothing: no passes, no textures.

Every engine shares the chains. Flames, escape-time fractals and
simulations all hand the same tail an image in the accumulator's
layout, so an effect works in every render mode and every output path.

**See also:**
- [RENDERER.md](RENDERER.md) — the flame pipeline and its tail
- [TONE-MAPPING.md](TONE-MAPPING.md) — the pass the two chains sit around
- [SHADERS.md](SHADERS.md) — WGSL conventions and the shader lints
- [ESCAPE.md](ESCAPE.md), [SIMULATION.md](SIMULATION.md) — the other two
  engines that feed the same tail
- [EXPORT.md](EXPORT.md) — where effects run in each export path
- [ONLINE.md](ONLINE.md) — the API client effects are downloaded through
- [SCRIPTING.md](SCRIPTING.md) — `flame.add_effect`, `flame.set_effect_param`
- [CONFIG.md](CONFIG.md) — `ConfigPath`, undo, `UpdateType`

**Design record:** [effects-system.md](../archive/projects/effects-system.md).
Research survey of candidate effects:
[new-shaders.md](../experimental/new-shaders.md).

**Code locations:**
- [src/effects/mod.rs](../../src/effects/mod.rs) — `EffectRegistry`,
  `EffectInfo`, `EffectInstance`, the built-in definitions, download
  checks
- [src/renderer/effect_chain.rs](../../src/renderer/effect_chain.rs) —
  `EffectChainRunner`: ping-pong textures, the parameter uniform,
  `EffectClock`
- [src/renderer/effect_passes.rs](../../src/renderer/effect_passes.rs) —
  multi-pass effects: their pipelines, buffers and persistent history
- [src/effects/passes.rs](../../src/effects/passes.rs) — a multi-pass
  effect's plan (`INFO.passes`, `INFO.buffers`, `INFO.warmup`)
- [assets/effects/](../../assets/effects/) — every built-in, as the Rhai
  source the app runs
- [shaders/effects/](../../shaders/effects/) — `density/`, `color/`, and
  `common/blend_modes.wgsl`: the blend library, and the WGSL the Rhai
  built-ins replaced (the reference, below)
- [src/ui/effects_panel.rs](../../src/ui/effects_panel.rs) — the Effects panel
- [src/app/effect_fetch.rs](../../src/app/effect_fetch.rs) — fetching an
  effect a flame names and this build lacks; the catalog
- [src/storage/effect_cache.rs](../../src/storage/effect_cache.rs),
  [effect_catalog.rs](../../src/storage/effect_catalog.rs) — downloads
  and the catalog
- [src/bin/export_effects_json.rs](../../src/bin/export_effects_json.rs)
  — the corpus the API serves

---

## Where the chains run

```
generator ──▶ [density effects] ──▶ tonemap ──▶ [colour effects] ──▶ display / readback
  flame: accumulator          Rgba32Float      fractal       Rgba8Unorm
         (or solid shade/DoF)   ping-pong        texture        ping-pong
  escape: its image
  sim:    its resolved field
```

| Path | Density chain reads | Colour chain reads | Where |
| --- | --- | --- | --- |
| App frame | The accumulator; in solid mode the shade or DoF output; the escape or simulation image | The tone map's output; the result is registered with egui for the viewport | `src/app/mod.rs` |
| Flame render (`render_with`: CLI, thumbnails, video) | Same order as the app: shade, DoF, density effects, tonemap | The tone map's output, then read back | `src/renderer/render.rs` |
| Escape render, 2D and terrain | The escape image | The tone map's output | `render.rs`, `finish` |
| Simulation render | The resolved field, or its terrain | The tone map's output | `render.rs`, `finish` |
| Tiled exporter | The full-resolution accumulation, after the histogram is freed | The whole output, after any tiling: colour effects are not local | `src/export/high_res.rs` |

**The headless tails build a runner only when an effect is enabled**,
and destroy it after the readback (on WebGPU a dropped buffer is not
freed). The app keeps one `EffectChainRunner` for the session and
resizes it with the renderer; its textures are allocated on first use
and dropped on a resize.

**Density effects need `FLOAT32_FILTERABLE`.** They bilinear-sample the
`Rgba32Float` accumulation. The devices request the feature where the
adapter offers it; where it does not, `run_density_effects` logs a
warning and skips the whole chain.

**The tiled exporter's density budget.** Density effects there run at
full resolution after the histogram is freed, at 52 bytes a pixel
(the accumulation, the density ping-pong pair, the tonemap output).
Above 4 GiB of that -- about 83 MP, roughly 9,100 × 9,100 -- they are
skipped with a warning and the export continues
(`DENSITY_EFFECT_PHASE_B_BUDGET_BYTES`). The Effects panel says so.
Colour effects are not gated.

**What density means.** A density effect sees whatever the generator
put in alpha. For a flame that is the raw cumulative hit count per
pixel. Escape and simulation images carry coverage there instead, so a
density-keyed effect behaves differently in those modes.

---

## The registry

`global_effect_registry()` returns a read guard on a lazily built
`EffectRegistry`; `global_effect_registry_mut()` a write guard. It is
behind a lock because effects are not a closed set: downloads and Rhai
effects register at runtime. Bind the guard to a variable before
borrowing from it -- `global_effect_registry().get(x)` drops the guard
at the end of the statement.

Iteration is in **registration order** (`all`, `by_category`). The panel's
Add lists and the corpus exporter (sorted by name) read it.

**`EffectInfo`:**

| Field | Meaning |
| --- | --- |
| `name` | The key a config stores (`"vignette"`) |
| `category` | `Density` or `Color`: the pipeline position |
| `display_name` | Empty for built-ins, which are labelled from `locales/*.yml` (`effects.<name>.name`). A download or a Rhai effect carries its own |
| `source` | `EffectSource::Builtin { embedded, path }` or `EffectSource::Owned(wgsl)` |
| `parameters` | `ParamDef`s, in slot order |
| `provenance` | `Builtin`, `Api { version }` or `Local` (a Rhai effect the config carries, or a plug-in) |
| `labels` | A Rhai effect's labels as written, strings or by locale (`scripted::Labels`); `None` otherwise |
| `passes` | A multi-pass effect's passes and buffers (`effects::passes::Passes`); `None` for one pass, `fs_main` |

`EffectInfo::translated_name` reads the locale first, then the labels'
title in the current language, then the declared `display_name`, then
the raw `name`. A locale miss is detected rather than trusted, because
`t!` returns the key on a miss. `param_label` gives a parameter's
caption and hint the same way; the Effects panel draws them.

Each registration of a name is counted (`EffectRegistry::revision`):
the runner keeps the revision a pipeline was built from and builds it
again when the name is registered again -- a Rhai effect edited, a
download updated. `remove_by_name` removes one.

**A parameter** is a `params::ParamDef`, the definition every kind
shares ([one-parameter-system.md](../archive/projects/one-parameter-system.md)):
name, label, hint (the control's tooltip), and its kind with its default and range
-- `Spec::float(0.5, 0.0, 1.0)`, `Spec::int`, `Spec::bool`,
`Spec::angle`, `Spec::choice` (a blend mode). The Effects panel draws
them through `ui::params`, and the API corpus spells them as before
(`params::api_json`). A downloaded effect's unranged float is drawn
0..1, as the panel always drew one (`params::from_api`).

**Shader source.** Every built-in is Rhai (rhai-plugins.md, D1;
[Effects written in Rhai](#effects-written-in-rhai)):
`assets/effects/<name>.rhai`, embedded with `include_str!` and compiled
when the registry is first used (about 25 ms for fifteen, native; a
shipped one is not validated again at startup, the tests do that). The
sixteen that replaced a WGSL effect are its twins
(`effects::custom::BUILTIN_TWINS`); `bloom` and `trails` were written in
Rhai with no WGSL behind them (`custom::RHAI_BUILTINS`, registered after
the WGSL list). On the desktop the copy beside the app is preferred when
it differs and compiles, so a built-in can be edited and seen after a
restart without a rebuild; one that does not compile is logged and the
shipped copy runs (a twin's WGSL, where it has one). Their registry
entries hold the compiled WGSL (`EffectSource::Owned`).

The WGSL the sixteen replaced stays, as **the reference**
(`effects::wgsl_builtins`, the definitions in registration order): what
each Rhai built-in is held to pixel for pixel. An edit to one of those
files reaches neither the app nor the twin, and each says so in its
header. A download carries its WGSL in `EffectSource::Owned`.

---

## The shipped effects

From the registry, in registration order (`register_builtin_effects`,
over `wgsl_builtins`, then `custom::RHAI_BUILTINS`). The label is the
curated name from `locales/en.yml`. Every one runs as Rhai.

| Name | Label | Kind | Params | What it does |
| --- | --- | --- | --- | --- |
| `vignette` | Vignette | colour | 4 | Darkens the edges with a smooth radial falloff |
| `film_grain` | Film Grain | colour | 3 | Per-pixel random noise; `seed` varies it between frames |
| `chromatic_aberration` | Chromatic Aberration | colour | 4 | Offsets the red and blue channels, optionally growing toward the edges |
| `hue_shift` | Hue Shift | colour | 3 | Rotates every hue by a fixed angle (shader `hue_cycle.wgsl`); keyframe `offset` to animate it |
| `density_blur` | Density Blur | density | 3 | Blurs low-density areas more than dense ones; `threshold` and `falloff` are multiples of the mean density |
| `sharpen` | Sharpen | density | 2 | Unsharp mask |
| `bilateral_blur` | Bilateral Blur | density | 3 | Edge-preserving blur, weighted by distance and by luminance similarity |
| `kaleidoscope` | Kaleidoscope | colour | 7 | N-fold rotational symmetry by folding coordinates in polar space |
| `plasma` | Plasma | colour | 6 | Demoscene plasma of summed sinusoids, blended over the image |
| `tunnel` | Tunnel | colour | 6 | Maps the image onto an endless tunnel in polar coordinates |
| `sobel_edges` | Edge Glow | colour | 5 | Sobel edge detection with a neon glow |
| `domain_warp` | Domain Warp | colour | 6 | Flowing distortion by fractal Brownian motion noise |
| `simplex_noise` | Simplex Noise | colour | 7 | Noise as a colour overlay, a distortion or a mask (`mode`) |
| `worley_noise` | Worley Noise | colour | 7 | Cellular patterns: cells, edges, organic or crystal |
| `julia` | Julia/Mandelbrot | colour | 10 | A Mandelbrot or Julia set rendered as an overlay |
| `distance_field` | Distance Field | colour | 5 | Jump-floods the bright pixels into a distance field and shades it: glow, contour bands, or nearest-colour fill (multi-pass: seed, ~log2 floods, composite) |
| `bloom` | Bloom | colour | 8 | The bright parts glow, near and wide: a thresholded down pyramid, an up pyramid summing each level with the wider ones (multi-pass, pyramids) |
| `trails` | Trails | colour | 2 | What moves leaves a fading afterimage (temporal: a persistent buffer) |

Three density effects, fifteen colour effects, 91 parameters. The
counts above are what `cargo run --release --bin export_effects_json`
emits.

**The animated built-ins take their time as a parameter.** `plasma`,
`tunnel`, `domain_warp`, `simplex_noise` and `worley_noise` take a
`time` parameter, and `film_grain` a `seed`; they move when an animation
track or a script drives that parameter. An effect may read the clock
instead (`ctx.time`, [Temporal effects](#temporal-effects-the-clock)).

---

## The runner

`EffectChainRunner` ([src/renderer/effect_chain.rs](../../src/renderer/effect_chain.rs))
runs one chain at a time:

1. Compiles each enabled effect's pipeline on first use, keyed by name
   (`ensure_effect_compiled`) -- a multi-pass effect's pipelines, one a
   pass ([below](#multi-pass-effects)). The WGSL is the source with its
   includes spliced (below). An effect the registry does not know is
   recorded with `note_missing_effect` and skipped.
2. Allocates a ping-pong texture pair for the chain on first use:
   `Rgba32Float` for density, `Rgba8Unorm` for colour.
3. Runs the enabled effects in list order. The first reads the chain's
   input; each later one reads the previous one's output. Every pass is
   a fullscreen triangle that overwrites its target: `BlendState::REPLACE`
   for colour, no blend state for density (`Rgba32Float` is not
   blendable without `FLOAT32_BLENDABLE`, and asking for REPLACE would
   fail pipeline creation).
4. The caller takes the result (`get_density_output`,
   `get_color_output`) or reads it back (`read_color_output_pixels`).

**The parameter uniform.** One shared uniform buffer of slots 256
bytes apart, one per pass, so effects in one submission keep separate
parameters. It starts at `MAX_EFFECT_SLOTS` (32) and `reserve_slots`
grows it to the next power of two when a frame's chains need more --
a multi-pass effect takes one a pass run: `distance_field` its flood
count plus two (13 at 1080p), `bloom` its levels twice and one. (The
33rd slot used to be clamped to an offset equal to the buffer's size, a
validation error that rejected the whole frame.) `reset_slots` is called
once per frame or render. A slot holds:

```wgsl
struct EffectParams {
    params: array<vec4<f32>, 12>,   // MAX_EFFECT_PARAMS = 48 floats
    width: u32,                     // the grid the pass writes (the image's, for one pass)
    height: u32,
    density_mean: f32,              // density effects: the frame's mean density
    pixel_scale: f32,               // render pixels per logical viewport point
    pass_index: u32,                // a multi-pass effect's pass run (below)
    passes: u32,
    step: u32,
    level: u32,
    levels: u32,
    buffer_levels: u32,             // each buffer's level to read, 4 bits a buffer
    frame: u32,                     // the clock (below): 0 for a still
    time: f32,
    dt: f32,
    _pad0: f32, _pad1: f32, _pad2: f32,
}
```

A shader may declare only the first fields: every WGSL effect written
before the pass and clock fields declares the first 208 bytes, and
binds the same layout.

Parameter `i` of the effect's definition goes to
`params[i / 4][i % 4]`, so the definition's order is the shader's slot
order. A value missing from the instance takes the registry default.

`density_mean` is, for a density effect on a flame, the frame's mean
density (`FlameRenderer::mean_density`, the unit Levels measures in):
`a / density_mean` is a density that does not grow with the iteration
count. It is 1 for an escape or simulation image, whose `a` is not a
hit count, and 0 for colour effects. It occupies what was an 8-byte
padding, so a shader that declares `_padding: vec2<f32>` there binds
the same layout and ignores it.

`pixel_scale` is the viewport's render scale
(`EffectChainRunner::set_pixel_scale`; [UI.md](UI.md), the Rendering
menu): render pixels per logical viewport point, 1 for every export. A
size in pixels is in points, so an effect multiplies it by this to keep
its size on the screen at 50%. The density blur's radius, sharpen's
radius, the bilateral blur's radius and spatial sigma, chromatic
aberration's amount and the kaleidoscope's edge offset do; the rest
declare the slot as padding. Film grain's per-pixel hash and the Sobel
stencil are per render pixel by nature and turn coarser at a lower scale.

**The 48-parameter cap is physical.** 48 floats plus the dimensions,
the pass fields and the clock is 256 bytes, the slot stride exactly; a
compile-time assertion holds the struct to it. Going much past about 60 parameters would need
a storage buffer, not a bigger constant. `check_download` refuses an
effect declaring more than `MAX_EFFECT_PARAMS`, and the corpus exporter
exits non-zero if a built-in does. The API's publish-time check must
use the same number. The `12` in every effect shader's struct must stay
in step with it.

**Bindings,** for every single-pass effect: `0` the input texture
(filterable float), `1` a linear sampler, `2` the parameter slot.
Entry points `vs_main` (fullscreen triangle) and `fs_main`.

### The blend-mode library

`shaders/effects/common/blend_modes.wgsl` is spliced into any shader
holding the line `// INCLUDE_BLEND_MODES` -- a directive **on its own
line**: the marker quoted inside a comment is left alone, and a
repeated directive includes once (`process_shader_includes`). Twelve of
the colour effects use it; `distance_field` and the three density
effects do not.

`apply_blend(base, effect, mode, intensity)` blends by mode, then mixes
with the base by `intensity`. `BLEND_MODE_NAMES` (`src/effects/mod.rs`)
names the modes for every `blend_mode` dropdown, held to the shader's
`switch` by `blend_mode_names_match_apply_blend`:

| `blend_mode` | Mode | `blend_mode` | Mode |
| --- | --- | --- | --- |
| 0 | Normal | 7 | Color Dodge |
| 1 | Add | 8 | Color Burn |
| 2 | Multiply | 9 | Hue |
| 3 | Screen | 10 | Saturation |
| 4 | Overlay | 11 | Color |
| 5 | Soft Light | 12 | Luminosity |
| 6 | Hard Light | | |

The library also defines colour-space helpers (`rgb_to_hsl`,
`hsl_to_rgb`, `luminance`, `set_luminance`, `saturation`, ...).

### Multi-pass effects

An effect whose `EffectInfo::passes` is set (docs/archive/projects/multipass-effects.md)
runs through `renderer::effect_passes`, the same in both chains: each
pass in order, each its own fragment entry point in the effect's one
module, each repeat its own parameter slot. A pass writes one of the
effect's buffers or, the last, the chain's output. `distance_field`,
`bloom` and `trails` are multi-pass; how a Rhai effect declares its
passes is [below](#multi-pass-effects-in-rhai).

- **Buffers** are `Rgba32Float`, any four numbers (the jump flood's
  coordinates). A plain buffer is a ping-pong pair: a pass that reads the
  buffer it writes reads the version before. A pyramid is one texture
  with a mip a level, bound a level at a time.
- **A pass over a pyramid's levels** (`repeat: "levels"`) writes every
  level -- down, finest first; up, coarsest first -- its target read at
  the level it came from (a zero texel at the first), every other
  pyramid at the level being written (`buffer_levels`). A level and its
  neighbour are distinct subresources, so writing one while reading the
  other is legal in WebGPU.
- **Bindings**: `0` the image, `1` the sampler, `2` the slot, then
  buffer `k` at `3 + k`, unfilterable (every read a `textureLoad`; the
  filtered read is bilinear by hand, the same on every vendor).
- **The state** of each multi-pass effect in a chain -- its buffers, for
  one effect, revision and size -- is kept by its place among the
  enabled effects (`EffectChainRunner::multipass_states`): made again
  when a different effect sits there or the size changes, freed when
  none does.

`distance_field`'s WGSL reference describes its own passes
(`effects::distance_field_passes`: `fs_seed`, `fs_flood` with the step
from the uniform, `fs_composite`), so it runs through the same runner as
its twin. `distance_field_shader_validates_all_entry_points` parses and
validates it with naga.

### Temporal effects: the clock

A buffer with `persist: true` outlives the frame. What a run does with
it is the chain's clock (`EffectClock`, set by `set_clock`) against the
clock the effect last ran at:

| The run | Its history |
| --- | --- |
| The same frame again (the viewport converging, an edit while paused) | Re-reads what the last run read |
| The next frame (`frame + 1`, the same `epoch`) | The last write becomes the history |
| Anything else: a jump, a new `epoch` (a reset), a still | Zero |

- **A still** (`EffectClock::STILL`, the default: PNG, high-res and CLI
  exports of a config, thumbnails) has no history: `ctx.frame` is 0
  and a persistent buffer reads zero, so a still is reproducible with no
  timeline.
- **A video** renders each frame with `EffectClock::video(frame, fps)`
  on engines carried from frame to frame (`RenderEngines::effects`, the
  chain kept beside the simulation's field and the escape renderer;
  `RenderJob::with_effect_clock`). Without engines a frame renders as a
  still would.
- **The viewport** (`app::effect_timeline`): playback moves the moment
  on a little each frame -- the next frame, `dt` the real interval --
  and a paused view holds the history while it is edited. A seek, a
  loop's wrap or a jump of more than half a second starts a new history,
  and so does the Effects panel's **Reset History** (shown when a
  temporal effect is in the chain).
- **A frame rendered alone** in a sequence -- Python's
  `Session::frame`, out of order -- first renders, unseen, the frames
  the effect remembers: from `frame - INFO.warmup`, or from frame 0
  without one (`effects::warmup_frames`, `temporal_window`).

---

## The config

```json
"color_effects": [
  { "effect_type": "vignette", "enabled": true, "params": { "intensity": 0.6 } }
],
"density_effects": []
```

`FractalConfig::density_effects` and `color_effects` are
`Vec<EffectInstance>`, skipped when empty. An `EffectInstance` is
`effect_type`, `enabled` and `params` (name to value). It also has a
session-local `id`, never serialized, which animation tracks bind to so
they follow an effect across add, delete and reorder;
`FractalConfig::fixup_ids` assigns it after loading.

| `ConfigPath` | Change |
| --- | --- |
| `DensityEffectEnabled { index }`, `ColorEffectEnabled { index }` | The enable box |
| `DensityEffectParam { index, param }`, `ColorEffectParam { index, param }` | One parameter; animatable |

Adding, deleting and reordering are structural changes with their own
undo entries (`ConfigChange::add_color_effect_snapshot`,
`delete_*_effect_snapshot`, `move_*_effect_snapshot`). Every effect
change is `UpdateType::ToneMappingOnly`: the accumulation is kept, and
only the tail re-runs.

**Scripts** add an effect by name, routed to its chain by its category
(`flame.add_effect(name)`), and set a parameter on the most recently
added instance of that effect (`flame.set_effect_param(name, param,
value)`). See [SCRIPTING.md](SCRIPTING.md).

---

## The Effects panel

[src/ui/effects_panel.rs](../../src/ui/effects_panel.rs), "Post-Processing
Effects", available in every render mode:

- **Color Effects** (open) and **Density Effects** (collapsed), each a
  list of the config's instances by their translated names (the raw
  key for an effect this build lacks), with an enable box, up/down
  reorder, remove, and a control per parameter while enabled. The
  controls are the one parameter renderer's (`ui::params::list` over a
  `ConfigStore` addressed through an `EffectTarget`, with a Rhai
  effect's conditions and grids): a slider for a Float (0 to 1 where its range is
  absent), an integer slider for an Integer, a dropdown for an Enum,
  a checkbox for a Boolean, degrees for an Angle, with the description
  as the tooltip. Every `blend_mode`, `mode` and `color_mode` is an
  Enum: the value stays the choice's index, the shader reads it as
  `i32(round(get_param(n)))` and the dropdown rounds the same way, so a
  value saved while these were integer sliders is the same entry, and
  a track animating one switches at the halfway point. A value is
  written only when it changes, never by being drawn. Undo entries name
  the effect the same way.
- **Rhai**, on an instance of a built-in that is Rhai: Edit as Rhai.
  The instance runs the user's own copy of the source the built-in runs,
  under a UUID minted for it, its settings kept, one undo step
  (`ColorEffectType` / `DensityEffectType` and `CustomEffects` in one
  batch), and the Rhai Editor opens the copy unsaved and in use, so
  Live shows each edit; Save keeps it in the library. The built-in is
  untouched (`RhaiEditorPanel::edit_effect_as_rhai`).
- An **Add effect** combo per chain, listing that category's
  registered effects by their translated names.
- **Online library**, when a catalog is cached or fetched: how many the
  server lists and how many are here, downloaded effects with a newer
  version -- each with **Update**, and **Update all** for several, the
  path that installs (`App::handle_effect_updates`; signed in) -- effects
  available to fetch, and how many the server lists without a shader
  yet. What is available is a listing, not download buttons: an effect
  is fetched when a flame uses it.
- **Clear downloaded effects (N)**, when any are here: the cached
  downloads and the catalog go, the registry's downloads with them
  (`EffectRegistry::clear_api`); a flame using one fetches it again. The
  built-ins and the user's own are not touched.

---

## Effects that did not ship

An effect a flame names may not be in this build. Downloads fill the
registry beyond the built-ins through one registration path
(`EffectRegistry::register_from_api`) with one set of refusals; a
config's own Rhai effects come with it ([below](#effects-written-in-rhai)).

**At startup** `storage::load_installed_resources` registers cached
downloads (`effect_cache::load_all_into_registry`). Every entry point
that renders calls it: `App::new` and the headless entry points. A
cached entry the current build would refuse is dropped with a warning.

**On demand.** When compiling finds an unknown name it records it
(`effects::note_missing_effect`). The app drains that set each frame
(`App::handle_effect_fetches`) and, if someone is signed in, fetches
each name once a session (`GET /api/v1/effects/{name}`). A fetched effect
is registered first and cached second (`effect_cache::save`), so a
payload this build refuses is never cached. The next frame compiles it;
nothing has to be invalidated. Unlike a missing variation, a missing
effect does not pause rendering: the picture renders without it
meanwhile. Signed out, the fetch is skipped with an info log. Headless
renders do not fetch: the effect is skipped with a warning.

**The catalog** (`App::refresh_effect_catalog`, `GET /api/v1/effects`) is
fetched once a session in the background and cached
(`storage::effect_catalog`). It is panel metadata; no frame depends on
it, and being offline is not an error. Each row says whether it is
`downloadable`; every row is `false` until the server's shaders are
seeded, and the panel counts those as "not available to download yet".

Local WGSL plug-in files (`plugins/effects/<name>.json`) were
retired 2026-11-01 ([rhai-plugins.md](../archive/projects/rhai-plugins.md), D3): nothing in the UI ever installed one, and Rhai plug-ins replace them.

**Rhai plug-ins** ([rhai-plugins.md](../archive/projects/rhai-plugins.md), P3): a Rhai variation, effect, formula, colouring, solid or simulation colouring in the user's formulas with **Load at start** set in the Catalog's info pane ([CATALOG.md](CATALOG.md); `formulas/_links.json`, `storage::links::Link::plugin`) is registered at every start, after the downloads (`storage::library::load_plugins`, from `load_installed_resources`: desktop, web and headless), under its key in the user's namespace (`Provenance::Local`). It is then in the lists without a config carrying it -- the variation pickers, the Effects panel's Add lists and the escape panel's formula and colouring pickers, each under "Your plug-ins" -- and choosing it carries its source into the config, as Use does. Saving the file registers the new version; turning it off unlists it; deleting it forgets the flag. One that does not register is named at start (a notice; stderr headless: `storage::library::plugin_failures`).

**Refusals** (`effects::check_download`, `register_from_api`):

| Refused | Why |
| --- | --- |
| `category` not `density` or `color` | The category is the pipeline position; there is no safe default |
| No shader, or only whitespace | It would appear in the panel and render nothing |
| More than `MAX_EFFECT_PARAMS` parameters | The uniform has nowhere to put them |
| Calls a blend-library function without `// INCLUDE_BLEND_MODES` | It would fail to compile naming a function its author never wrote. Checked against every function the library defines, parsed from the library |
| A name a built-in has | A download would change what a shared flame renders |

The wire types are `EffectListItem` and `EffectDownload` in
`src/api/types.rs`. `requires_blend_modes` on the wire is advisory: the
client splices on the marker actually present.

---

## Effects written in Rhai

An effect can be written in the formula language's Rhai
([rhai-plugins.md](../archive/projects/rhai-plugins.md), P2; the language is in
[ESCAPE.md](ESCAPE.md#the-formula-language)): `fn effect(c, p, ctx)`,
run per pixel, `c` the input's colour there (an `rgba`), `p` its
parameters, `ctx` its context. [effects/custom.rs](../../src/effects/custom.rs)
compiles it (`EntryKind::Effect`) to `fx_effect(uv)` and wraps that in
the effect template (`assemble`): the `EffectParams` uniform with
`get_param`, the input texture and sampler, the fullscreen triangle,
and the blend library when the effect blends. It registers as an
`EffectInfo` with `EffectSource::Owned` WGSL, its labels kept.

```rhai
const PARAMS = #{
    amount: #{ value: 0.5, min: 0.0, max: 1.0, caption: #{ en: "Amount", ja: "量" } },
    tint: #{ value: rgba(1.0, 0.5, 0.0, 1.0), caption: "Tint" },
    blend_mode: #{ value: 2, choices: ["Normal", "Add", "Multiply"] },
};
const INFO = #{ title: "Tint", id: "6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b", category: "color" };

fn effect(c, p, ctx) {
    #{ color: c.rgb * p.tint.rgb, blend: p.blend_mode, amount: p.amount }
}
```

- **The result** is an `rgba`; a `vec3`, the input's alpha kept; or
  `#{ color, blend, amount }`, blended over the input by `apply_blend`
  with mode `blend` (an int, in `BLEND_MODE_NAMES`' order) at `amount`
  (1 when absent), the input's alpha kept -- what the built-ins do.
- **Reading the image:** `input(uv)`, filtered at a position
  (`textureSampleLevel` at level 0, legal in any control flow, so the
  WASM rule holds), and `input_at(x, y)`, loaded at a pixel and clamped
  to the image. Each is four per-channel reads the shader compiler
  merges.
- **The context:** `ctx.uv` (the pixel's position, 0 to 1, a complex
  number), `ctx.width` and `ctx.height` (ints), `ctx.density_mean` and
  `ctx.pixel_scale` (the uniform's).
- **Parameters** are numbers, bools, choices and colours (`rgba(...)`,
  four slots `name_r` ...), at most `MAX_EFFECT_PARAMS`, `get_param` in
  declaration order. Not complex numbers, not function parameters.
  Labels may be by locale (design rule 8).
- **No** `rand()` (a picture is the same every frame: hash the position
  and a seed parameter), no `prepare`; state only in a persistent buffer
  (below).
- **`INFO`**: `title`; `id` -- a UUID for one of the user's (design rule
  7: `scripted::mint_id`; the Rhai Editor writes one into the file on
  Use), a built-in's name for a shipped one; `category`, `"color"`
  (after the tone map, the default) or `"density"`; a multi-pass
  effect's `passes`, `buffers` and `warmup` (below).

### Multi-pass effects in Rhai

```rhai
const INFO = #{
    title: "Distance Field", id: "distance_field",
    buffers: #{ coords: #{} },
    passes: [
        #{ run: "seed", to: "coords" },
        #{ run: "flood", to: "coords", repeat: "log2" },
        #{ run: "effect" },
    ],
};
fn seed(c, p, ctx) { ... }
fn flood(c, p, ctx) { ... buffer_at("coords", x, y) ... }
fn effect(c, p, ctx) { ... }
```

- **A pass** names a function of `effect`'s shape with `run` (`fn` is a
  Rhai keyword, so no map key). Each compiles as its own program
  (`lang::rhai::effect_pass`), prefixed `fx<k>_`, into the effect's one
  module (`custom::assemble_passes`), entry point `fs_pass<k>`. `to`
  names the buffer it writes; the last pass is `effect` and writes the
  output, and only it may blend. At most 16 passes (`passes::MAX_PASSES`).
- **`repeat`**: 1 to 64, `"log2"` (the covering power of two of the
  larger side: the jump flood's count), or `"levels"` over a pyramid
  target, with `order: "down"` (the default) or `"up"`.
- **`buffers`**: a map of name to `#{ scale, levels, persist }`, each
  optional (`#{}`). `scale` sizes it against the frame (more than 0, at
  most 4); `levels` 1 to 12 makes it a pyramid; `persist` keeps it from
  one frame to the next (not a pyramid; written by one pass, once a
  frame). Indices follow the names' sorted order. At most 8.
- **`warmup`**: the frames a temporal effect forgets past (`trails`
  declares 240), for a frame rendered alone.
- **Reads**: `buffer(name, uv)`, filtered, and `buffer_at(name, x, y)`,
  the texel, clamped -- the name a string literal the plan declares.
  Each reads once into a slot, as a texture's read does (`Host::BufferLoad`).
- **The context** gains `ctx.xy` (the pixel's position in the grid the
  pass writes, exact: WGSL's `position.xy`), `ctx.pass`, `ctx.passes`,
  `ctx.step` (`2^(passes - 1 - pass)`), `ctx.level`, `ctx.levels`,
  `ctx.frame`, `ctx.time` and `ctx.dt`. In a pass into a buffer,
  `ctx.width` and `ctx.height` are that grid's size; `input_at` clamps
  to the image's own.

**Keyed by id and version.** One of the user's effects registers as
`scripted::key(id, source)`, `u<the id's 32 hex>_<the first 8 hex of its
source's SHA-256>` (`custom::key_for`; rule 7; the same key a Rhai
variation has), the name its instances run; a built-in's key is its name. So
two versions of one effect are two entries, and every config renders
the version it carries whatever else is open -- nothing looks the
version up through the config. `custom::id_of` drops the version.

**Carried.** A config holds the sources its chains run
(`FractalConfig::custom_effects`, `ConfigPath::CustomEffects`), `entry`
the key its instances name. `effects::ensure_custom(config)` registers
them, under that key, where every path decides it has effects
(`render_with`'s three kinds, the high-res exports, the viewport); it
compares the text first and hashes only a new one, and a source that
does not compile is logged once (`scripted::Ledger`). A build without
`lang` has no compiler, and a carried effect renders as an unknown one
does.

**In the Rhai Editor**, an effect's row offers *Use as effect*: a
UUID is minted into the file if it has none of its own -- none, or an
id that is not a UUID, such as a twin's copied `vignette` or one typed
by hand (`rhai::with_own_id`) -- the effect registers under its key,
and in one undo step the instances of an earlier version move to this
one (`ConfigPath::ColorEffectType` / `DensityEffectType`, their
parameters kept) and the source no instance runs any more goes. The
first time, an instance is added to its chain (a second undo step:
adding an instance is structural). The panel's Add lists leave Rhai
effects out: each version would list once.
*Live* re-applies it by its id.

**The built-ins are Rhai** (rhai-plugins.md, D1, P2 step 7): sixteen
are `assets/effects/<name>.rhai` (`custom::BUILTIN_TWINS`, compiled by
`custom::builtin_twin` under the built-in's name, `Provenance::Builtin`),
each replacing its WGSL once nothing was lost -- `distance_field` the
last, once the language had passes (2026-11-14; within one 8-bit step of
its WGSL at five settings, and its renders pixel-identical to the old
runner's). A twin that has not passed waits in
`custom::PENDING_TWINS` (none now), its WGSL running. The checks stay
as tests against the WGSL (`wgsl_builtins`):
- **Pixels**: `the_rhai_twins_match_their_wgsl` (the thirteen colour
  effects, within a step of 8-bit rounding, at four or five settings
  each) and
  `the_density_twins_match_their_wgsl` (the three density ones, to a
  part in a thousand).
- **The values a config reads**: each parameter's name, type, default,
  range, label and hint, in order, and the category
  (`reference_beside_twin`, in both); the title is the locale's English
  (`a_builtin_takes_its_curated_locale_name`).
- **The web**: `the_builtin_twins_are_web_safe` -- explicit-level
  sampling only, no operator mix Tint refuses, no self-compare or
  self-division, no subnormal literal -- with naga's validation of
  every built-in (`every_built_in_effect_validates`).
- **Speed**: `the_twins_are_no_slower`, ignored (a GPU measurement, run
  alone, the app closed): each at 1080p and 4K against its WGSL, nine
  rounds of a ~20 ms batch, the twin straight after the WGSL, and the
  median of the rounds' ratios (best-of-N moved with the GPU's clocks);
  a replaced twin more than 5% slower fails, and a pending one is
  reported, ready once it is within 5%. On a GTX 1660 SUPER (2026-11-14)
  all sixteen are within 3% of their WGSL or faster: julia and
  `density_blur` 5% faster (the latter's taps skipped as the shader skips
  them), kaleidoscope +2%, `distance_field` +3% at 1080p and +1% at 4K
  (7.8 and 31.5 ms for the WGSL: its ~log2 flood passes are the cost
  either way, not the language). Bloom and Trails have no WGSL to time
  against.
- **Why one is slower**: `time_effect_variants`, ignored, times the WGSL
  files in `output/effect-variants/<effect>/` against the built-in's
  WGSL and twin (both written there to start from), the median of
  per-round ratios, at `EFFECT_VARIANTS_SIZE` -- an experiment is a
  file edit, not a rebuild. Moving the twin toward the WGSL one change
  at a time, and the WGSL toward the twin, is how each cost below was
  found.
- **The corpus** exports the compiled WGSL, the blend marker kept.
A built-in that fails a check goes back to its WGSL by moving from
`BUILTIN_TWINS` to `PENDING_TWINS`.

**What the speed check changed in the compiler** (every cause was the
language's code, not the twins'), so an effect compiles to what a
hand-written one is:
- **In an effect**: real `sin`, `cos` and `tan` are the GPU's own, not
  range-reduced (an effect's arguments are its image's coordinates, and
  the WGSL effects call them so; plasma 1.5x); an int parameter read as
  a float, or compared with an int literal, is the float rounded, and
  `ctx.width` / `ctx.height` the u32 widened, with no conversion through
  i32 per pixel (kaleidoscope 6%); a real's `floor`, `trunc` or `ceil`
  read as a float is the float function; Rhai's `min`, `max` and
  `clamp`, which the language lowers to a test and an assignment, are
  WGSL's own (2%). Each gives the same values (below 2^31; `min`/`max`
  differ only at NaN, and an effect has no interpreter to agree with).
- **Everywhere**: a loop whose counter bounds it folds the cap into its
  bound instead of running a second counter (julia 1.4x), a loop's `&&`
  test is one `break` for each part (julia 6%; [ESCAPE.md](ESCAPE.md#the-formula-language)),
  and `atan2`'s zero-pair guard is a `select`, not a branch.

## The corpus for the API

```bash
cargo run --release --bin export_effects_json            # output/effects-corpus.json
cargo run --release --bin export_effects_json -- out.json
```

It writes every registered effect, sorted by name: its curated
`display_name`, category, parameters, `version` 1, and its **raw** WGSL
with the `// INCLUDE_BLEND_MODES` marker intact (the client splices at
compile time, so a pre-spliced copy would duplicate the library), plus
`requires_blend_modes`, and `multi_pass` and `temporal`: a multi-pass
effect's module has an entry point a pass and no `fs_main`, so an app
from before multi-pass effects cannot run one, and the API serves them
only to readers that can. For a built-in that is Rhai the WGSL is what its
source compiles to (the catalogs stay WGSL, D2); the Rhai source itself
is not in the corpus yet (P6). `description`, `description_plain` and `authors`
are emitted as null. Run it whenever an effect is added or edited, and
send the result to the API repository ([RELEASE.md](../RELEASE.md) §3).

---

## Adding a built-in effect

A new built-in is a Rhai source: write `assets/effects/<name>.rhai`
(`INFO.id` its name, `INFO.title` its English label), append it to
`custom::RHAI_BUILTINS`, and do steps 4-5 below; the web-safety lint and
the label test cover it. A WGSL effect (or the reference a twin is held
to):

1. Write `shaders/effects/{density,color}/<name>.wgsl` with the
   `EffectParams` struct above, bindings 0–2, `vs_main` and `fs_main`.
   A colour effect that blends puts `// INCLUDE_BLEND_MODES` on its own
   line and calls `apply_blend`.
2. Add an `include_str!` constant to `effects::embedded_shaders`.
3. Add it to `wgsl_builtins` with an empty
   `display_name`, `EffectSource::Builtin { embedded, path }` (`path`
   relative to `shaders/`), `Provenance::Builtin`, and its parameters in
   slot order. A registry name is never reused for something else: it
   is what saved configs store.
4. Add `effects.<name>.name` and its parameter labels to
   `locales/en.yml` (and the other locales).
   `every_shipped_effect_has_a_curated_label` fails without the English
   name.
5. Regenerate the corpus (above) and tell the API.

The shader lints in `src/variations/mod.rs` walk all of `shaders/`, so
an effect shader is held to the same rules as a variation: no
self-compares, no self-divisions, no subnormal literals. Sample the
input with `textureSampleLevel(.., 0.0)` (or read it with
`textureLoad`), never `textureSample`: the implicit-derivative forms
are valid only in uniform control flow, which a browser's WebGPU
enforces and naga does not check, so a per-pixel branch before one
compiles on desktop and fails in Chrome (see [SHADERS.md](SHADERS.md)).
The effect textures have one mip, so level 0 is what `textureSample`
read. `effects_never_sample_with_implicit_derivatives`
(`src/effects/mod.rs`) rejects the implicit forms anywhere under
`shaders/effects/`.

---

## Gotchas

**The density chain is `Rgba32Float`, like the accumulation.** A
flame's alpha is a raw hit count, which a filament flame pushes past
65504 well inside the default budget. The ping-pong was `Rgba16Float`
until 2026-10: its largest finite half float is 65504, so those
pixels became Inf (black under Log) or saturated. Full floats cost
the tiled exporter's budget its ceiling (above).

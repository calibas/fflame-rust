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
  `EffectChainRunner`: ping-pong textures, the parameter uniform, the
  `distance_field` jump flood
- [shaders/effects/](../../shaders/effects/) — `density/`, `color/`, and
  `common/blend_modes.wgsl`
- [src/ui/effects_panel.rs](../../src/ui/effects_panel.rs) — the Effects panel
- [src/app/effect_fetch.rs](../../src/app/effect_fetch.rs) — fetching an
  effect a flame names and this build lacks; the catalog
- [src/storage/effect_cache.rs](../../src/storage/effect_cache.rs),
  [effect_catalog.rs](../../src/storage/effect_catalog.rs),
  [plugins.rs](../../src/storage/plugins.rs) — downloads, the catalog,
  local plugins
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
| Escape render, 2D and terrain | The escape image (`escape_tail`) | The tone map's output | `render.rs`, `escape_tail` |
| Simulation render | The resolved field, or its terrain | The tone map's output | `render.rs`, an inline copy of the same tail |
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
behind a lock because effects are not a closed set: downloads and local
plugins register at runtime. Bind the guard to a variable before
borrowing from it -- `global_effect_registry().get(x)` drops the guard
at the end of the statement.

Iteration is in **registration order** (`all`, `by_category`). The panel's
Add lists and the corpus exporter (sorted by name) read it.

**`EffectInfo`:**

| Field | Meaning |
| --- | --- |
| `name` | The key a config stores (`"vignette"`) |
| `category` | `Density` or `Color`: the pipeline position |
| `display_name` | Empty for built-ins, which are labelled from `locales/*.yml` (`effects.<name>.name`). A download or plugin carries its own |
| `source` | `EffectSource::Builtin { embedded, path }` or `EffectSource::Owned(wgsl)` |
| `parameters` | `EffectParameter`s, in slot order |
| `provenance` | `Builtin`, `Api { version }` or `Local` |

`EffectInfo::translated_name` reads the locale first, then the declared
`display_name`, then the raw `name`. A locale miss is detected rather
than trusted, because rust-i18n returns the key on a miss.

**`EffectParameter`** carries `name`, `display_name`, `param_type` (the
variations' `ParamType`), `default_value`, optional `min_value` and
`max_value`, and an optional `description` shown as the slider's
tooltip. Only `distance_field`'s parameters have descriptions today.

**Shader source.** A built-in's WGSL is embedded with `include_str!`
(`effects::embedded_shaders`) on every target. On the desktop the
on-disk copy under `shaders/` is preferred when it exists, so an effect
can be edited and seen after a restart without a rebuild; an absent
file falls back to the embedded copy. A download or plugin carries its
WGSL in `EffectSource::Owned`.

---

## The shipped effects

From the registry, in registration order (`register_builtin_effects`).
The label is the curated name from `locales/en.yml`.

| Name | Label | Kind | Params | What it does |
| --- | --- | --- | --- | --- |
| `vignette` | Vignette | colour | 4 | Darkens the edges with a smooth radial falloff |
| `film_grain` | Film Grain | colour | 3 | Per-pixel random noise; `seed` varies it between frames |
| `chromatic_aberration` | Chromatic Aberration | colour | 4 | Offsets the red and blue channels, optionally growing toward the edges |
| `hue_shift` | Hue Shift | colour | 3 | Rotates every hue by a fixed angle (shader `hue_cycle.wgsl`); keyframe `offset` to animate it |
| `density_blur` | Density Blur | density | 3 | Blurs low-density areas more than dense ones |
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
| `distance_field` | Distance Field | colour | 5 | Jump-floods the bright pixels into a distance field and shades it: glow, contour bands, or nearest-colour fill |

Three density effects, thirteen colour effects, 81 parameters. The
counts above are what `cargo run --release --bin export_effects_json`
emits.

**Animated effects have no clock.** `plasma`, `tunnel`, `domain_warp`,
`simplex_noise` and `worley_noise` take a `time` parameter, and
`film_grain` a `seed`; they move when an animation track or a script
drives that parameter.

---

## The runner

`EffectChainRunner` ([src/renderer/effect_chain.rs](../../src/renderer/effect_chain.rs))
runs one chain at a time:

1. Compiles each enabled effect's pipeline on first use, keyed by name
   (`ensure_effect_compiled`). The WGSL is the source with its
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
each `distance_field` alone takes its flood count plus two (13 at
1080p). (The 33rd slot used to be clamped to an offset equal to the
buffer's size, a validation error that rejected the whole frame.)
`reset_slots` is called once per frame or render. A slot holds:

```wgsl
struct EffectParams {
    params: array<vec4<f32>, 12>,   // MAX_EFFECT_PARAMS = 48 floats
    width: u32,
    height: u32,
    _padding: vec2<f32>,
}
```

Parameter `i` of the effect's definition goes to
`params[i / 4][i % 4]`, so the definition's order is the shader's slot
order. A value missing from the instance takes the registry default.

**The 48-parameter cap is physical.** 48 floats plus the dimensions is
208 bytes, inside the 256-byte slot stride; a compile-time assertion
holds the struct to it. Going much past about 60 parameters would need
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
with the base by `intensity`:

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

### `distance_field`: the jump flood

`distance_field` registers like any colour effect -- config, panel,
animation and scripts treat it the same -- but has no `fs_main`. The
runner special-cases it **by name**: `compile_effects` skips it, and
`run_color_effects` builds its resources (`ensure_jfa`) and runs
`run_jfa` in its place:

1. **Seed** (`fs_seed`): pixels whose luminance is above `threshold`
   write their own coordinate into an `Rg32Float` texture; the rest a
   sentinel.
2. **Flood** (`fs_flood`), `ceil(log2(max(width, height)))` passes over
   an `Rg32Float` ping-pong, the step halving each pass, `params[0]`
   holding the step.
3. **Composite** (`fs_composite`): the base image shaded by the
   distance to the nearest seed, within `spread` (a fraction of the
   larger dimension) -- `mode` 0 glow, 1 contour bands (`band_count`),
   2 the nearest seed's colour.

It takes one parameter slot a pass: the flood count plus two. The
coordinate textures are unfilterable, so every read is a `textureLoad`.
`distance_field_shader_validates_all_entry_points` parses and validates
the module with naga and runs the shader lints on it.

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
  list of the config's instances with an enable box, up/down reorder,
  remove, and a slider per parameter while enabled. Sliders run from
  the parameter's min to its max (0 to 1 where either is absent), with
  its description as the tooltip.
- An **Add effect** combo per chain, listing that category's
  registered effects by their translated names.
- **Online library**, when a catalog is cached or fetched: how many the
  server lists and how many are here, downloaded effects with a newer
  version, effects available to fetch, and how many the server lists
  without a shader yet. It is a listing, not download buttons: an
  effect is fetched when a flame uses it.

---

## Effects that did not ship

An effect a flame names may not be in this build. Three sources fill
the registry beyond the built-ins, all through one registration path
(`EffectRegistry::register_from_api`) with one set of refusals.

**At startup** `storage::load_installed_resources` registers cached
downloads (`effect_cache::load_all_into_registry`) and then local
plugins (`plugins::load_all`). Every entry point that renders calls it:
`App::new` and the headless entry points. A cached entry the current
build would refuse is dropped with a warning. A refused plugin is
reported -- a notification in the app, stderr headless.

**On demand.** When compiling finds an unknown name it records it
(`effects::note_missing_effect`). The app drains that set each frame
(`App::handle_effect_fetches`) and, if someone is signed in, fetches
each name once a session (`GET /api/effects/{name}`). A fetched effect
is registered first and cached second (`effect_cache::save`), so a
payload this build refuses is never cached. The next frame compiles it;
nothing has to be invalidated. Unlike a missing variation, a missing
effect does not pause rendering: the picture renders without it
meanwhile. Signed out, the fetch is skipped with an info log. Headless
renders do not fetch: the effect is skipped with a warning.

**The catalog** (`App::refresh_effect_catalog`, `GET /api/effects`) is
fetched once a session in the background and cached
(`storage::effect_catalog`). It is panel metadata; no frame depends on
it, and being offline is not an error. Each row says whether it is
`downloadable`; every row is `false` until the server's shaders are
seeded, and the panel counts those as "not available to download yet".

**Local plugins** are files in the same JSON shape as a download
(`EffectDownload`): `<app data>/plugins/effects/<name>.json` on the
desktop, localStorage under the same `plugins/` prefix on the web. The
file name is the identity. They live apart from the download cache, so
clearing the cache never touches them. `plugins::install` validates a
file with the same `check_download` before saving it.

**Refusals** (`effects::check_download`, `register_from_api`):

| Refused | Why |
| --- | --- |
| `category` not `density` or `color` | The category is the pipeline position; there is no safe default |
| No shader, or only whitespace | It would appear in the panel and render nothing |
| More than `MAX_EFFECT_PARAMS` parameters | The uniform has nowhere to put them |
| Calls a blend-library function without `// INCLUDE_BLEND_MODES` | It would fail to compile naming a function its author never wrote. Checked against every function the library defines, parsed from the library |
| A name a built-in has | A download would change what a shared flame renders |
| A name a local plugin has, from a download | It would replace the user's own work |
| A plugin whose name is already registered | The user's file is the newcomer, and is refused with the conflict named |

The wire types are `EffectListItem` and `EffectDownload` in
`src/api/types.rs`. `requires_blend_modes` on the wire is advisory: the
client splices on the marker actually present.

---

## The corpus for the API

```bash
cargo run --release --bin export_effects_json            # output/effects-corpus.json
cargo run --release --bin export_effects_json -- out.json
```

It writes every registered effect, sorted by name: its curated
`display_name`, category, parameters, `version` 1, and its **raw** WGSL
with the `// INCLUDE_BLEND_MODES` marker intact (the client splices at
compile time, so a pre-spliced copy would duplicate the library), plus
`requires_blend_modes`. `description`, `description_plain` and `authors`
are emitted as null. Run it whenever an effect is added or edited, and
send the result to the API repository ([RELEASE.md](../RELEASE.md) §3).

---

## Adding a built-in effect

1. Write `shaders/effects/{density,color}/<name>.wgsl` with the
   `EffectParams` struct above, bindings 0–2, `vs_main` and `fs_main`.
   A colour effect that blends puts `// INCLUDE_BLEND_MODES` on its own
   line and calls `apply_blend`.
2. Add an `include_str!` constant to `effects::embedded_shaders`.
3. Register it in `register_builtin_effects` with an empty
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

**A label in the list is the registry key.** The panel's rows show
`effect_type` (`sobel_edges`); only the Add combo shows the curated
label (Edge Glow).

**Every parameter is a float slider.** The panel draws Boolean,
Integer and Angle parameters the same way, over their min and max.

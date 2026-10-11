# Colour

**Overview:** How a flame's samples get their colour, how that colour
reaches the per-pixel histogram, and what the accumulator holds
afterwards. Everything downstream of the accumulator (the tone map,
Levels, highlight handling, the background, transparent output) is in
[TONE-MAPPING.md](TONE-MAPPING.md).

The escape-time and simulation engines share the palette and everything
from the accumulator on, but colour their pixels directly; see
[The other engines](#the-other-engines).

**See also:**
- [TONE-MAPPING.md](TONE-MAPPING.md) — everything after the accumulator
- [RENDERER.md](RENDERER.md) — the passes, overwrite mode, persistent orbits
- [BUFFERS.md](BUFFERS.md) — the structs and bindings named here
- [PALETTE_LIBRARY.md](PALETTE_LIBRARY.md) — packs, the Palette Library
  panel, custom palettes
- [FOCUSED-RENDERING.md](FOCUSED-RENDERING.md) — the planner PathMap
  colours through
- [ESCAPE.md](ESCAPE.md), [SIMULATION.md](SIMULATION.md) — the other two
  engines
- [VARIATIONS.md](VARIATIONS.md) — the `WritesColor` / `WritesRgb`
  features

**Code locations:**
- [src/scene/palette.rs](../../src/scene/palette.rs) — `ColorMode`,
  `PathMapStyle`, `Palette`, `PaletteTransform`, `render_palette_lookup`,
  `PaletteLibrary`
- [src/gpu/buffers.rs](../../src/gpu/buffers.rs) — the palette texture
  (`PALETTE_FORMAT`, `write_palette_table`, `FlameBuffers::update_palette`)
  and `AccumulateParams`
- [shaders/core/main_template.wgsl](../../shaders/core/main_template.wgsl)
  — the colour update, direct colour, the histogram deposit
- [shaders/core/utilities.wgsl](../../shaders/core/utilities.wgsl) —
  `srgb_to_linear`, `speed_to_color`
- [shaders/accumulate.wgsl](../../shaders/accumulate.wgsl) — histogram to
  accumulator
- [src/renderer/compute_kernel.rs](../../src/renderer/compute_kernel.rs)
  — blend-mode selection, the orbit key, palette upload
- [src/ui/tone_mapping.rs](../../src/ui/tone_mapping.rs) — the Colors
  panel, including its colour-mode and palette controls
- [src/ui/settings.rs](../../src/ui/settings.rs) — the Rendering panel's
  blend controls

**Design record:**
[accumulator-unification.md](../archive/projects/accumulator-unification.md)
(the accumulator format),
[histogram/](../archive/histogram/README.md) (how the histogram reached
4 × u32; start with
[HISTOGRAM_FINAL.md](../archive/histogram/HISTOGRAM_FINAL.md)),
[COLOR_PIPELINE.md](../archive/histogram/COLOR_PIPELINE.md) (the
pipeline this one replaced),
[palette-improvements.md](../archive/projects/palette-improvements.md),
[tonemap-and-palette-improvements.md](../archive/projects/tonemap-and-palette-improvements.md)
(the palette transforms),
[needs-transform-and-direct-color.md](../archive/needs-transform-and-direct-color.md)
(direct colour),
[word-editing.md §10](../projects/deep-zoom/word-editing.md) (PathMap).

---

## The path of a colour

```
palette ──▶ lookup table ──▶ Rgba16Float texture (sRGB values)
            rotation, squeeze,          │
            log, reverse                │ sampled at each plot,
                                        │ decoded sRGB → linear
chaos game ──▶ colour coordinate ───────┤
               or speed colour          ▼
               or path colour     plotted colour × weight
                                        │
                                        ▼
                     histogram: 4 × u32 per pixel, one batch
                                        │  [spatial filter]
                                        ▼
                     accumulator: Rgba32Float, rgb = mean colour,
                                  a = cumulative weighted hits
                                        │
                                        ▼
                                 TONE-MAPPING.md
```

Three facts shape the rest of this document.

**Colour is fixed when a sample is plotted.** The histogram holds colour
sums, not palette positions. A palette edit cannot be re-applied to what
has already accumulated, so a change to the palette, its transforms, the
colour mode or the speed factor (`UpdateType::ColorOnly` in
[src/config/delta.rs](../../src/config/delta.rs)) uploads the new palette
and re-accumulates through the overwrite window. A tone-map edit re-runs
only the last pass.

**The arithmetic is in linear light.** Palette colours and the
background are authored as sRGB. The shaders decode a palette texel with
`pow(c, 2.2)` (`srgb_to_linear`) and the tone map encodes its output with
`pow(c, 1/2.2)`. Both are the gamma-2.2 approximation rather than the
piecewise sRGB curve, so decode followed by encode is the identity.

**Colour and density are kept apart.** The accumulator's `rgb` is a mean
colour in [0, 1]; how bright a pixel is comes from its `a`, the hit
count, through the tone map.

---

## Colour modes

`FractalConfig::color_mode`, the `ColorMode` enum in
[src/scene/palette.rs](../../src/scene/palette.rs). The shader builder
compiles it in as `const COLOR_MODE`
([src/shader_builder_v2.rs](../../src/shader_builder_v2.rs)), and it is
part of the orbit key together with `speed_factor`, so changing either
starts new orbits ([RENDERER.md](RENDERER.md), persistent orbits).

| value | mode | serialized as | colour of a plotted sample |
| --- | --- | --- | --- |
| 0 | Palette (default) | `"palette"` | the palette at the orbit's colour coordinate |
| 1 | Speed | `"speed"` | a running blend of palette colours indexed by step length |
| 2 | PathMap | `"path_map"` | the palette at the colour of the sample's path |

The PascalCase names of older files (`"Palette"`, `"Speed"`,
`"PathMap"`) load through serde aliases.

### Palette

Each orbit carries a colour coordinate, `color_index`. It starts at 0
for a fresh orbit and persists across dispatches with the rest of the
orbit. Every iteration, the chosen transform pulls it toward its own
`color`, with `color_speed` playing Apophysis's *symmetry*
(`main_template.wgsl`):

```
color_index = color_index · (1 + color_speed) / 2  +  color · (1 − color_speed) / 2
```

| `color_speed` | effect on the coordinate |
| --- | --- |
| 1 | unchanged: the transform has no colour influence |
| 0 | moved halfway to the transform's `color` (the default) |
| −1 | replaced by the transform's `color` |

The transform fields, edited in the Transforms panel
([src/ui/transforms.rs](../../src/ui/transforms.rs)), are on
`Transform` in [src/scene/transforms.rs](../../src/scene/transforms.rs):

| field | UI range | default | meaning |
| --- | --- | --- | --- |
| `color` | 0–1 | 0.5 | the palette position the transform pulls toward |
| `color_speed` | −1–1 | 0.0 | the symmetry above |
| `opacity` | 0–1 | 1.0 | probability that a sample from this transform is plotted; the orbit continues either way |
| `direct_color` | 0–1 | 1.0 | strength of direct-colour variations, below |

Only the selected (normal) transform's `color` and `color_speed` apply.
Linked transforms that run after it affect colour only through
direct-colour writes. Final transforms change neither the coordinate nor
the colour: they are a plot-time filter, and their direct-colour writes
are discarded.

At the plot the shader samples the palette texture at
`(color_index, 0.5)` with `textureSampleLevel`, through a nearest-filter,
clamp-to-edge sampler (`FlameBuffers::sampler`). A coordinate therefore
picks a single table entry, as flam3 and Apophysis floor their colormap
index, and a coordinate outside [0, 1] takes the end colour. The texel
is decoded with `srgb_to_linear`.

### Direct colour

A variation can set colour itself. Two features in
[src/variations/definition.rs](../../src/variations/definition.rs) cover
it. Each is compiled in only when an active variation has it (the
`HAS_DC` and `HAS_RGB` template flags), so a flame without them gets a
byte-identical shader.

- **`WritesColor`** writes a palette coordinate, `vc`. With it the
  update above becomes the three-step flow of Apophysis's `XForm.pas`:
  1. `c_base` = the symmetry update above;
  2. the normal and linked variations run, and direct-colour ("DC")
     variations write `vc`, which starts at `c_base`;
  3. `color_index = c_base + direct_color · (vc − c_base)`.
- **`WritesRgb`** writes an RGB colour, `vrc`, directly. The register
  starts each iteration at a sentinel of −1e30, and only when a
  variation actually wrote it does the plot blend:
  `colour = mix(palette_colour, vrc, direct_color)`. Transforms with no
  such variation keep their palette colour. PathMap skips the override.

`direct_color` defaults to 1.0 on new transforms, where Apophysis
defaults it to 0, because it is a no-op on a transform without DC
variations and so adding one works without hunting for a slider. A file
that omits the field loads as 0.0, the old default, so old files keep
their look.

### Speed

`speed` is the distance the point moved this iteration,
`length(current − old_pos)`, measured after the linked chain and before
the final one (with z in 3D). Every iteration:

```
speed_colour = srgb_to_linear(palette(clamp(ln(10 · speed + 1) / 3, 0, 1)))
colour       = mix(colour, speed_colour, speed_factor)
```

`colour` starts white for a fresh orbit and persists with it, and it is
what gets plotted. `speed_factor` ("Speed Blend Factor" in the Colors
panel, 0–1, default 0.5) is therefore the rate of a running average over
the orbit's steps: at 1 a sample wears its own step's colour, and at 0
the colour never leaves white. The palette's top entry is reached at a
step length of (e³ − 1)/10 ≈ 1.91.

The speed colour reads the same palette texture, so the palette
transforms apply. Under Focused Rendering a forced sample wears the free
orbit's speed colour, which is one of that feature's open gaps
([deep-zoom-tracker.md](../projects/deep-zoom/deep-zoom-tracker.md)).

### PathMap

PathMap colours each part of the picture by its *path*: the sequence of
transforms Focused Rendering drew it through. It needs Focused Rendering
(`cylinder_targeting`), which plans in 2D only. Without a plan it runs
Palette mode's colour flow exactly, so with Focused Rendering off, or in
3D, it equals the Palette render bit for bit; the Colors panel says so
and offers a button to turn Focused Rendering on.

- **Path, Path (distinct) and Depth** colour each path on the CPU
  (`word_tree::path_colours`) and pack the colour into the path's colour
  fold with a slope of 0. The forced prefix's
  `color_index · H + G` then *is* the path's colour, with no shader
  change.
- **Origin** styles compute the coordinate in the shader
  (`pathmap_origin`) from where the sample's point was before its path
  carried it into view.
- The coordinate is looked up in the palette as in Palette mode.

| style (`path_map_style`) | serialized as | colour by |
| --- | --- | --- |
| Path (default) | `"path"` | a gradient by address: the last transform picks a range of the palette, the one before it a range within that |
| Path (distinct) | `"path_distinct"` | the same transforms, hashed, so neighbouring parts differ |
| Depth | `"depth"` | the path's length, shortest to longest on screen |
| Origin (Radial) | `"origin_radial"` | the starting point's distance from the fractal's centre |
| Origin (Horizontal) | `"origin_horizontal"` | the starting point's horizontal position across the fractal |
| Origin (Vertical) | `"origin_vertical"` | the starting point's vertical position across the fractal |

The two Path styles read `path_map_level` transforms of each path (UI
1–6, default 2), counted from where the paths on screen first differ.
Style and level are packed with the plan, so changing either restarts
accumulation. The shader also records, per pixel, the path it was last
drawn through (`path_ids`, binding 7), which the viewport's right-click
reads. How the plan, the colour fold and the right-click work is
[FOCUSED-RENDERING.md](FOCUSED-RENDERING.md)'s subject.

---

## The palette

### Palettes and stops

`Palette` in [src/scene/palette.rs](../../src/scene/palette.rs):

| field | meaning |
| --- | --- |
| `name` | display name; the library deduplicates on it, case-insensitively |
| `stops` | `ColorStop { position, color }`: position in [0, 1], colour as sRGB in [0, 1] |
| `locked` | fixed 256-colour mode (`convert_to_fixed` samples the gradient at 256 positions `i/255`; `convert_to_free` unlocks) |
| `built_in` | set at runtime on library palettes, never serialized; picking one in the Colors panel makes an editable copy with it cleared |

`sample_color(t)` clamps `t` to [0, 1] and interpolates linearly between
the two stops around it; past the last stop it returns that stop's
colour. `sample_color_stepped(t)` returns the colour of the last stop at
or before `t` (the first stop's before it), drawing each stop as a flat
band.

**Serialized form**, which is also the `.palette` file format: a palette
of exactly 256 stops at positions `i/255` is written compactly as
`indexed_colors`, a comma-separated list of `RRGGBB` hex values;
anything else as a `stops` array. Both carry `name` and `locked`, and
both load.

**Built-in palettes.** Grayscale is always the first entry of the
library's flat list (`PaletteLibrary::palettes`), and
`Palette::default()` returns it. Fire, Cool,
Rainbow and Purple Pink are added only if no pack loaded at all. A new
`FractalConfig` starts with Fire (`default_palette` in
[src/config/fractal_config.rs](../../src/config/fractal_config.rs)).

**Packs** live in `assets/palettes/packs/`, and the folder is the
catalogue. `starter_pack.json` is also embedded in the binary as the
fallback. As shipped:

| file | pack | palettes | enabled by default |
| --- | --- | --- | --- |
| `starter_pack.json` | Starter Pack | 16 | yes (embedded) |
| `iq_palettes.json` | IQ's Procedural Palettes | 11 | yes |
| `apophysis1.json` – `apophysis4.json` | Apophysis Classic 1–4 | 200, 204, 123, 174 | no |

On top of these, the library builds a Custom pack from the user's saved
palettes and, on desktop, a Local Files pack from loose
`assets/palettes/*.palette` files. Loading, enabling and the Palette
Library panel are covered in [PALETTE_LIBRARY.md](PALETTE_LIBRARY.md).

### Palette transforms

Five config fields reshape the palette without editing its stops. All
are in the Colors panel's *Color & Appearance* section:

| control | field | UI range | default |
| --- | --- | --- | --- |
| Palette Rotation | `palette_rotation` | 0–1 (the field accepts −1–1) | 0 |
| Squeeze Mode | `palette_squeeze_mode` | Linear, Geometric | Linear |
| Palette Squeeze (Linear) | `palette_squeeze` | 0.1–16 | 1 |
| Squeeze Falloff (Geometric) | `palette_squeeze_falloff` | 0.1–0.95 | 0.5 |
| Log Redistribute | `palette_log_strength` | −5–5 | 0 |
| Reverse Palette | `palette_reverse` | on/off | off |
| Palette Size | `palette_size` | 256, 512, 1024, 2048, 4096 | 256 |

`render_palette_lookup` samples the palette into a table of
`palette_size` entries and then remaps it in four stages. Each stage
copies entries by index (nearest), without interpolating:

1. **Squeeze.** Linear: `t → fract(t · squeeze)`, so a value above 1
   repeats the palette that many times and one below 1 shows only its
   first part. Geometric: octave packing with ratio `r` = the falloff;
   the first `1 − r` of the range maps to the whole palette, the next
   `r(1 − r)` to the whole palette again, and so on.
2. **Log redistribution**, strength `s`. For `s > 0`,
   `t → (e^(s·t) − 1) / (e^s − 1)`, which packs more of the palette into
   the top of the range; `s < 0` is the mirror image. 0 is the identity.
3. **Rotation**: a cyclic shift by `round(rotation · size)` entries.
4. **Reverse**: the table flipped.

Read as a lookup, the stages compose the other way round. The colour the
shader finds at coordinate `t` is

```
base( squeeze( log( wrap(t′ + rotation) ) ) ),   t′ = 1 − t if reversed, else t
```

The Colors panel's preview strip calls the same function (with a
512-entry table), so the preview cannot disagree with the renderer.
Larger `palette_size` gives finer steps; a 256-colour palette in a
larger table is interpolated between its stops.

### The texture

`FlameBuffers::update_palette` writes two tables on every change:

- the smooth table from `render_palette_lookup`;
- the stepped table from `render_stepped_palette_lookup`, every stop a
  flat band, through the same four stages. The escape engine binds it
  when asked for a stepped palette, so toggling that costs no upload.

Each table is a `palette_size × 1` 2D texture in `PALETTE_FORMAT`,
`Rgba16Float`, holding the sRGB values clamped to [0, 1]. The
[buffers.rs](../../src/gpu/buffers.rs) comment gives the reason for the
format: an 8-bit table truncated dark entries channel by channel, so a
dark grey could arrive as pure red, while a half float carries every
`k/255` to within a sixteenth of a level. `Rgba32Float` would be exact,
but browsers cannot filter it and the escape engine filters its palette.
The flame shader samples the smooth table through a nearest sampler.
The high-resolution exporter builds its own table through the same
functions, so an export's colours are the viewport's.

---

## What a plot deposits

The **histogram** holds one batch: 4 × `u32` per pixel (R, G, B,
density), cleared at the start of each compute pass
(`FlameBuffers::clear_histogram`). A plotted sample adds
(`main_template.wgsl`):

```
s  = 100 · density_weight                        // the colour scale
R += u32(clamp(r, 0, 1) · s)
G += u32(clamp(g, 0, 1) · s)
B += u32(clamp(b, 0, 1) · s)
D += u32(s)
```

The colour is the linear plotted colour, clamped to [0, 1]. Post-symmetry
plots each copy from the same base colour (3D fog may still tint copies
differently by depth). `density_weight` is 1 unless something below
multiplies into it:

| factor | applies when |
| --- | --- |
| the emission's own weight | a `PlotEmits` variation's extra plot points |
| the drawn word's weight (`ct_weight`) | Focused Rendering: a word drawn off its probability deposits the difference |
| the window's likelihood ratio × the warm-up rate | importance sampling (`IMPORTANCE_SAMPLING`) |
| `zr^(−2s)`, clamped to [1/64, 64] | 3D depth-density compensation |
| `1 − solid_strength` behind the surface, 0 in a priming batch | solid rendering |
| `exp(−zdist² · far_density_fade)` | 3D far-density fade |

Depth fog is not a weight: it blends the colour toward the background.

**Why the scale cancels.** Colour is recovered by dividing the colour
sums by the density sum, Σ(s · w · c) / Σ(s · w). The constant 100 and
any weight common to all four channels cancel, so a weight changes
density, and so brightness, without tinting anything. That is why the
scale is a constant (100, written in both `main_template.wgsl` and
`accumulate.wgsl`, where it is `HISTOGRAM_COLOR_SCALE`) and no longer a
setting: its value changes nothing but quantisation. Two consequences
follow.

- **Overflow** needs about 43 million unit-weight hits on one pixel
  within one batch (2³² / 100), since the histogram is cleared every
  batch.
- **Truncation** costs each deposit less than one unit per channel, under
  1/100 of full scale at weight 1. A weight under 1/100 deposits nothing
  on the ordinary path. Under importance sampling the scale is instead
  rounded stochastically (`is_deposit` in
  [shaders/core/header.wgsl](../../shaders/core/header.wgsl): the floor,
  plus one with probability equal to the fraction), with one draw shared
  by all four channels, so the expected deposit is exact.

The tiled sample-emit export path
([accumulate_samples.wgsl](../../shaders/core/accumulate_samples.wgsl))
and the CPU-histogram path
([src/export/high_res.rs](../../src/export/high_res.rs)) use the same
scale and weighting.

**Spatial filter.** Between the compute pass and the fold, an optional
bilateral Gaussian ([shaders/histogram_blur.wgsl](../../shaders/histogram_blur.wgsl),
Apophysis's `filter`) blurs the batch histogram: all four channels
together, so colour and density spread as one. `filter_radius` is the
sigma in pixels (0–5, 0 is off) and `filter_blur_edges` (0–1) relaxes
its edge preservation. Both are in the Colors panel under *Advanced →
Spatial Filter*, in the flame modes only.

---

## The accumulator

A pair of `Rgba32Float` textures, ping-ponged, one texel per pixel.
After every batch [accumulate.wgsl](../../shaders/accumulate.wgsl) folds
the histogram into it:

```
n     = D / 100                                  // this batch's weighted hits
total = prev.a + n
c_new = clamp((R, G, B) / max(D, 1), 0, 1)       // this batch's mean colour
α     = n / total                                // cumulative mean (default)
      | blend_factor                             // fixed EMA
rgb   = prev.rgb · (1 − α) + c_new · α
a     = total
```

A pixel with no hits this batch keeps its previous value.

| channel | holds |
| --- | --- |
| `rgb` | the density-weighted mean colour of everything that has landed there, linear, in [0, 1] |
| `a` | the cumulative weighted hit count: not scaled, not blended, cumulative in both modes |

The tone map turns `a` into brightness by comparing it with the render's
iterations per pixel ([TONE-MAPPING.md](TONE-MAPPING.md)).

### Blend modes

Two controls in the Rendering panel
([src/ui/settings.rs](../../src/ui/settings.rs)): *Use Dynamic Blend*
(`use_dynamic_blend`, default on) and *Fixed Blend Rate*
(`blend_factor`, 0.001–1 on a log slider, default 0.1, enabled only when
dynamic blend is off). `FlameRenderer` turns them into
`AccumulateParams { blend_factor, use_fixed_ema }`:

| state | `blend_factor`, `use_fixed_ema` sent | α |
| --- | --- | --- |
| overwrite window | 1.0, 0 | the previous value is discarded |
| dynamic blend on (default) | 0.0, 0 | `n / total`: cumulative mean |
| dynamic blend off | `blend_factor`, 1 | `blend_factor`: fixed EMA of colour |

- **Cumulative mean** is the reference flame algorithm and is right from
  the first batch. The shader comment puts its f32 precision floor near
  10⁹ hits per pixel, where a batch's share falls below what the stored
  value can resolve.
- **Fixed EMA** stays precise indefinitely, because each batch moves the
  stored colour by a constant share. Its first frames are dim while the
  average climbs from zero, and it carries a slight recency bias.
- **Overwrite.** The shader treats any `blend_factor ≥ 0.99` as
  overwrite and discards the previous value. The host sends 1.0 during
  the overwrite window that follows a parameter change
  ([RENDERER.md](RENDERER.md)). On leaving the window, fixed-EMA mode
  also clears both textures, so the last drag frame does not dominate
  the new average.

The name `use_dynamic_blend` predates the cumulative mean; it now
selects it. Changing either control restarts accumulation.

**Solid rendering** adds one rule. When a pixel's nearest depth moves
closer by more than about 1.5 × `surface_thickness`, its history is
discarded, because that history belongs to a surface now known to be
hidden ([solid-rendering.md](../archive/projects/solid-rendering.md)).

---

## The other engines

The escape-time and simulation engines use neither the histogram nor the
accumulate pass. Each writes an `Rgba32Float` image in the accumulator's
layout, `rgb` a linear colour and `a` its coverage, and hands it to the
shared tail. Both read the same palette textures, so the palette
transforms apply to them too.

- **Escape** binds `FlameRenderer::escape_palette_view(stepped)`: the
  stepped table when the escape config's `palette_map.stepped` is on,
  the smooth one otherwise. Its colourings decode the texel with
  `pow(2.2)` like the flame plot ([src/escape/assembler.rs](../../src/escape/assembler.rs)).
  See [ESCAPE.md](ESCAPE.md).
- **Simulation** binds the smooth table (`FlameRenderer::palette_view`).
  A colouring returns `(rgb, coverage)`, and coverage 0 lets the
  background through. See [SIMULATION.md](SIMULATION.md).

Because `a` is a coverage of at most 1 rather than a hit count, the
logarithmic tone map does not suit these images; entering either mode
switches it to Linear ([TONE-MAPPING.md](TONE-MAPPING.md), *Per-engine
behaviour*). The Colors panel hides the colour-mode selector in these
modes, since neither engine reads it, and always shows the palette
controls.

---

## The Colors panel's colour controls

The *Color & Appearance* section of the Colors panel
([src/ui/tone_mapping.rs](../../src/ui/tone_mapping.rs)):

| control | shown |
| --- | --- |
| Color Mode | flame modes |
| Palette picker, Edit Palette, Palette Library, preview strip, the palette transforms, Palette Size | Palette and Speed modes, and every non-flame mode |
| Speed Blend Factor | Speed mode |
| Path Style, Path level, the Focused Rendering notice | PathMap mode |
| Background Color | always (used by the tone map) |

The palette picker lists the palettes of enabled packs. The Palette
Editor and Palette Library panels are covered in
[PALETTE_LIBRARY.md](PALETTE_LIBRARY.md).

---

## Common tasks

| task | where |
| --- | --- |
| Add a built-in palette | a pack JSON in `assets/palettes/packs/` ([PALETTE_LIBRARY.md](PALETTE_LIBRARY.md)) |
| Change the colour update | `main_template.wgsl`, the Step 1 / Step 3 blocks |
| Add a colour mode | `ColorMode` in `palette.rs`, the `COLOR_MODE` branches in `main_template.wgsl`, the orbit key in `compute_kernel.rs`, the Colors panel |
| Add a palette transform | `PaletteTransform` and `transform_lookup_table` in `palette.rs`, `FlameBuffers::update_palette`, the config field and its `ConfigPath` (`ColorOnly`) |
| Change the histogram encoding | the deposit in `main_template.wgsl`, `accumulate.wgsl`, `accumulate_samples.wgsl`, the CPU path in `export/high_res.rs` |
| Change the accumulation formula | `accumulate.wgsl`, `AccumulateParams` in `buffers.rs`, the mode selection in `compute_kernel.rs` |

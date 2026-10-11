# Tone Mapping

**Overview:** Everything after the accumulator: how an `Rgba32Float`
texel holding a mean colour and a hit count becomes a displayed pixel.
One fragment shader, [shaders/tonemap.wgsl](../../shaders/tonemap.wgsl),
does it for every engine, drawing a full-screen triangle into an
`Rgba8Unorm` texture. What the accumulator holds and how colour got
there is [COLOR.md](COLOR.md).

**See also:**
- [COLOR.md](COLOR.md) — colour modes, the palette, the histogram, the
  accumulator
- [RENDERER.md](RENDERER.md) — the pass sequence and when the tonemap runs
- [BUFFERS.md](BUFFERS.md) — `TonemapParams` and the tonemap bind group
- [EFFECTS.md](EFFECTS.md) — the density and colour effect chains on
  either side of this pass
- [FOCUSED-RENDERING.md](FOCUSED-RENDERING.md) — the iteration scale and
  auto exposure
- [EXPORT.md](EXPORT.md) — transparent PNG export
- [ESCAPE.md](ESCAPE.md), [SIMULATION.md](SIMULATION.md) — the engines
  that enter Linear

**Code locations:**
- [shaders/tonemap.wgsl](../../shaders/tonemap.wgsl) — the pass
- [src/scene/tonemap.rs](../../src/scene/tonemap.rs) — `ToneMapMode`,
  `HighlightMode`, `ToneCurve`
- [src/scene/tonemap_presets.rs](../../src/scene/tonemap_presets.rs) —
  `TONEMAP_PRESETS`
- [src/config/defaults.rs](../../src/config/defaults.rs) — the defaults,
  the Apophysis constants, `entry_tone_mapping`
- [src/renderer/compute_kernel.rs](../../src/renderer/compute_kernel.rs)
  — `update_tonemap`, `set_transparent_mode`, `refresh_sample_density`
- [src/gpu/buffers.rs](../../src/gpu/buffers.rs) — `TonemapParams`, the
  curve LUT
- [src/ui/tone_mapping.rs](../../src/ui/tone_mapping.rs) — the Colors
  panel
- [src/ui/histogram.rs](../../src/ui/histogram.rs) and
  [src/renderer/histogram.rs](../../src/renderer/histogram.rs) — the
  density histogram and the Levels controls
- [src/ui/visibility.rs](../../src/ui/visibility.rs) — which controls each
  engine offers

**Design record:**
[tone-mapping-improvements.md](../archive/projects/tone-mapping-improvements.md)
(Levels, the curve),
[tonemap-highlights.md](../archive/projects/tonemap-highlights.md)
(highlight modes and the Highlights slider),
[tonemap-and-palette-improvements.md](../archive/projects/tonemap-and-palette-improvements.md)
(presets),
[levels-scale-invariance.md](../archive/projects/levels-scale-invariance.md)
(Levels in multiples of the mean),
[accumulator-unification.md](../archive/projects/accumulator-unification.md)
(the scale-invariant `sample_density`).

---

## Where the pass sits

```
accumulator ─▶ [solid shade, DoF] ─▶ [density effects] ─▶ TONEMAP ─▶ [colour effects] ─▶ readback
```

Density effects see the accumulator, density in its alpha, and hand the
tonemap their output; colour effects work on the tone-mapped RGB
([src/renderer/render.rs](../../src/renderer/render.rs), `render_with`).
Every control on this page except auto exposure is
`UpdateType::ToneMappingOnly` ([src/config/delta.rs](../../src/config/delta.rs)):
it re-runs this pass and keeps every accumulated sample. Auto exposure
changes the compute shader, so it restarts accumulation.

## The pass, in order

1. **Read** the accumulator texel with `textureLoad` (an `Rgba32Float`
   texture is not filterable). `count = accum.a × 100`.
2. **Map** colour and alpha by the tone-map mode: Linear, Logarithmic or
   Density.
3. **HSV**: hue shift, then saturation, skipped when both are neutral.
4. **Exposure**, for Logarithmic. Linear applied it in step 2; Density
   used it as its sensitivity.
5. **Highlights**: negative channels to 0, then Clip, MaxNorm, Reinhard
   or Filmic brings the colour into [0, 1].
6. **Tone curve**, if enabled and the pixel was hit.
7. **Alpha**: the mode's alpha blended with a linear alpha, then capped
   by Levels when they are on.
8. **Output**: composite over the background and encode, or one of
   three transparent encodings.

Steps 3–6 act on the fractal's colour only; the background never passes
through them.

---

## Tone-map modes

`FractalConfig::tonemap_mode`, `ToneMapMode` in
[src/scene/tonemap.rs](../../src/scene/tonemap.rs), serialized as
`"linear"`, `"logarithmic"` and `"density"` (the old
`"DensityVisualization"` loads as an alias). The default is Logarithmic.
It is chosen in the Colors panel under *Tone Mapping → Advanced → Tone
Map Mode*.

Every mode inverts `gamma`: the exponent is `1/gamma`, except that a
gamma of exactly 0 gives an exponent of 0.

| mode | value | colour | alpha |
| --- | --- | --- | --- |
| Linear | 0 | `(accum.rgb · exposure)^(1/gamma)` | `clamp(accum.a · density_scale, 0, 1)` |
| Logarithmic | 1 | the Apophysis pipeline below | gamma-corrected log density, blended with the linear alpha |
| Density | 2 | grey `d^(1/gamma)`, with `d = clamp(accum.a · exposure, 0, 1)` | `d` |

Linear and Density read the raw hit count and never consult the
iteration count. In a flame, Linear's alpha reaches 1 at
`1/density_scale` hits on a pixel, and Density saturates wherever
`accum.a · exposure ≥ 1`. Only Logarithmic holds its brightness as a
render accumulates.

---

## The logarithmic pipeline

The flam3 / Apophysis calculation (`ImageMaker.pas`, cited line by line
in the shader), simplified to contrast 1 and oversample 1. The constants
come from Apophysis's `ControlPoint.pas` (`defaults.rs`):
`BRIGHT_ADJUST = 2.3`, `PREFILTER_WHITE = 2²⁶`.

```
k1     = 2.3 · brightness · 268 · PREFILTER_WHITE / 256
k2     = 1 / (area · white_level · sample_density)
ls     = k1 · log10(1 + white_level · count · k2) / (white_level · count) / PREFILTER_WHITE
fp_rgb = ls · accum.rgb · count
fp3    = ls · count · white_level
```

Collected, the two outputs are

```
fp3    = (2.3 · 268 / 256) · brightness · log10(1 + count / (area · sample_density))
fp_rgb = fp3 · accum.rgb / white_level
```

### Normalisation

`area` is the view's area in fractal units, `width · height / ppu²`,
with `ppu = min(width, height) / 4 · zoom`, the scale `world_to_pixel`
uses. `sample_density` is the render's iterations per pixel,
`samples_in_buffer / (width · height)`, rewritten before every tonemap
pass by `refresh_sample_density` and floored at 1e-6. Their product is
`samples / ppu²`, so the log's argument is

```
100 · accum.a · ppu² / samples
```

the hits a pixel received per sample drawn, per unit of fractal-space
area. That is a property of the attractor, not of the render: it does
not change with the number of samples, the output resolution or the
zoom. So brightness does not drift as a render accumulates, and
`iterations_per_thread` is purely a speed setting.

The `× 100` in `count` is a calibration constant. The shader's comment
gives its role: it lifts the log's argument into the range where the
default brightness and exposure give a visible picture. (It used to
undo the 0.01 scale of an older accumulator's alpha.)

`refresh_sample_density` also folds three corrections into
`sample_density`, all 1 when their features are off:

| factor | from | effect |
| --- | --- | --- |
| `solid_density_fraction` | solid rendering | the measured share of samples that survive occlusion, so a solid tone-maps at the brightness its visible samples deserve |
| `frame_coverage_fraction` | auto exposure | the measured share of plot attempts landing in frame |
| `cylinder_iteration_scale` | Focused Rendering | how many unbiased iterations one forced iteration stands for |

The last two are described under
[Focused Rendering and auto exposure](#focused-rendering-and-auto-exposure).

### Gamma and the gamma threshold

With `g = 1/gamma` and threshold `T`:

```
alpha = fp3^g                                         if fp3 > T
alpha = (1 − fp3/T) · fp3 · T^(g − 1) + (fp3/T) · fp3^g   if 0 < fp3 ≤ T
```

Below the threshold the curve blends from linear into the gamma curve,
which keeps the steep start of a fractional power from amplifying noise
in sparse areas. `T = 0` turns the blend off.

### Vibrancy

```
vib    = round(vibrancy · 256)
notvib = 256 − vib
ls′    = vib · alpha / fp3
colour = ls′ · fp_rgb + notvib · fp_rgb^g     if notvib > 0
colour = ls′ · fp_rgb                         otherwise
```

The first term expands to `vib · alpha · accum.rgb / white_level`: the
mean colour scaled by the gamma-corrected density, so colours keep
their saturation however bright the pixel. The second applies gamma to
each channel separately, which washes bright colours toward white. At
the defaults (vibrancy 1, white level 200) the colour is
`1.28 · alpha · accum.rgb`. Above 1, `notvib` is negative, so only the
first term remains and the colour scales with `vib`: a brightness
multiplier on the colour that leaves alpha alone.

### White level ("Highlights")

`white_level` appears in `k2` and in `ls`'s divisor. In `fp3`, and so in
alpha, it cancels; in the colour it divides. A higher value darkens the
colour, so fewer channels reach the clip and highlights stay saturated.
A lower value brightens it, and dense areas bleach to white sooner. The
default, 200, is Apophysis's.

---

## HSV and exposure

**Hue shift** (degrees) is added to the hue and wrapped once into
[0, 360). **Saturation** multiplies HSV saturation, clamped to [0, 1].
Both act on the unclipped colour, whose value can exceed 1.

**Exposure** multiplies the colour: after HSV in Logarithmic, before
gamma in Linear, and on the count in Density.

## Highlights: back into [0, 1]

`FractalConfig::highlight_mode`, `HighlightMode` in
[src/scene/tonemap.rs](../../src/scene/tonemap.rs). It is chosen under
*Advanced → Highlight Clipping* and applies in every mode, Linear
included. Every mode first sets negative channels to 0.

| mode | value | mapping |
| --- | --- | --- |
| Clip (default) | 0 | `min(c, 1)` per channel, as Apophysis and JWildfire do. A bright colour shifts toward yellow, cyan, magenta or white as its channels saturate one by one. |
| MaxNorm | 1 | if the largest channel exceeds 1, divide all three by it: the hue is exact and the brightest channel lands on 1 |
| Reinhard | 2 | scale by `(L / (1 + L)) / L`, where `L` is Rec. 709 luminance, then `min(c, 1)`. It compresses every value, not only those above 1. |
| Filmic | 3 | the ACES fit of Narkowicz, `x(2.51x + 0.03) / (x(2.43x + 0.59) + 0.14)` per channel, clamped to [0, 1] |

## The tone curve

`FractalConfig::tonemap_curve`, a `ToneCurve` of control points
`(x, y)` in [0, 1], sorted by `x`, with endpoints at `x = 0` and `x = 1`
that move only vertically. It is evaluated as a monotone cubic Hermite:
Catmull-Rom tangents limited by the Fritsch–Carlson condition, so the
curve passes through every point without overshooting.

`generate_lut` samples it at `x = i/255` into a 256-entry `Rgba16Float`
texture (the value in R). The shader reads it through a linear-filtering
sampler once per channel, sampling unconditionally and choosing after,
because a browser requires `textureSample` in uniform control flow. The
curve is applied when `use_curve` is on and the pixel's `count` exceeds
0.001. `use_curve` defaults to on with the identity curve, so by default
it changes nothing.

In the Colors panel's *Tone Curve* section: an enable box; presets Linear,
S-Curve, Brighten Shadows and Darken Highlights; and an editor where
dragging moves a point, a double-click adds one, and Ctrl+click
(Cmd+click on a Mac) or a right-click -- a long press on touch --
removes an interior one. The endpoints move vertically only.

---

## Alpha and the background

The mode's alpha, clamped to [0, 1], is blended with a linear alpha:

```
gamma_alpha  = clamp(alpha, 0, 1)
linear_alpha = clamp(accum.a · density_scale, 0, 1)
base_alpha   = mix(gamma_alpha, linear_alpha, smoothstep(alpha_blend_low, alpha_blend_high, gamma_alpha))
```

Thin areas keep the gamma-corrected alpha and dense areas move to the
linear one. Under Linear the two are the same expression, so the blend
does nothing; the panel greys *Alpha Blend Low* and *High* there
(`Control::AlphaBlendCurve`). Like Linear mode's alpha, the linear term
reads the raw hit count: it reaches 1 at `1/density_scale` hits.

Levels then cap it ([Levels](#levels)):
`fractal_alpha = min(base_alpha, levels(accum.a))` when Levels are on,
`base_alpha` otherwise.

**Opaque output** composites in linear light and encodes:

```
bg    = srgb_to_linear(background_color)          // pow(c, 2.2)
out   = pow(bg · (1 − fractal_alpha) + colour · fractal_alpha, 1/2.2)
alpha = 1
```

`background_color` is stored as the sRGB value picked in the Colors
panel.

---

## Levels

An opt-in remap of opacity by relative density. Levels lower a pixel's
opacity; they never brighten or recolour it.

| control | field | UI | default |
| --- | --- | --- | --- |
| Levels | `levels_enabled` | checkbox | on |
| Low | `levels_low` | log slider, 0 to `max(max_density / mean, 10)`, values past the end accepted | 0 |
| High | `levels_high` | as Low | 10 |
| Midtones | `levels_gamma` | log slider, 0.001–10 | 0.5 |

```
n = accum.a / levels_density
levels(n) = 1                                      if levels_density ≤ 0
          = (n > low ? 1 : 0)                      if high ≤ low
          = clamp((n − low) / (high − low), 0, 1) ^ levels_gamma
```

The power is skipped at a gamma of 1. Below 1 it raises opacity, above
1 it lowers it.

**Units.** `low` and `high` are multiples of the mean density, so the
same setting clips the same way at a million iterations or two hundred
billion. The mean, `levels_density`, is `sample_density` with Focused
Rendering's inflation divided back out. Dividing by the inflated count
would put every pixel at a vanishing fraction of the mean and make the
frame transparent.

**Engines.** Levels apply only in the flame modes:
`FractalConfig::effective_levels_enabled` is false for escape-time and
simulation pictures, and the panel hides the section there.

**The Density Levels section** of the Colors panel shows the density
histogram ([src/ui/histogram.rs](../../src/ui/histogram.rs)): 256
log-spaced bins of `accum.a`, read back from the accumulator every 30
frames (`DensityHistogram` in
[src/renderer/histogram.rs](../../src/renderer/histogram.rs)), bar
heights on a log scale, with the minimum, maximum and median. Markers
show Low (black), High (white) and the midpoint (grey, at
`1 / (1 + levels_gamma)` of the way between them). *Auto* sets Low and
High to the 1st and 99th percentile densities divided by the mean. The
histogram computes its own mean, the samples accumulated over the pixel
count, without the solid and auto-exposure corrections `levels_density`
carries.

---

## Transparent output

`FlameRenderer::set_transparent_mode` picks one of four encodings. A
transparent export ignores the background and respects Levels, so
compositing it over the background reproduces the opaque export. The
Export panel's *Premultiplied alpha* box and the CLI's `--transparent`
and `--premultiplied` flags choose among them ([EXPORT.md](EXPORT.md)).

With `c` the fractal colour, `a` its alpha, and `C = pow(c · a, 1/2.2)`
the opaque-over-black colour:

| mode | value | used for | rgb | alpha |
| --- | --- | --- | --- | --- |
| opaque | 0 | display, opaque export | composite over the background | 1 |
| straight, flame | 1 | transparent flame export | `C / m` | `m`, the largest channel of `C` |
| premultiplied | 2 | *Premultiplied alpha*, any engine | `C` | `a` |
| straight, image | 3 | transparent escape-time or simulation export | `pow(c, 1/2.2)` | `a` |

- **Mode 1.** A flame's brightness lives in a low alpha: a bright
  colour at small opacity. Plain straight alpha (rgb = `c`) and plain
  premultiplied alpha both come out dim when an image editor flattens
  over black. Mode 1 stores the over-black picture itself, split so that
  the editor's flatten `rgb · alpha` returns `C` exactly; over other
  backgrounds it composites as ordinary straight alpha.
- **Mode 2** is for compositing tools that read premultiplied PNGs.
- **Mode 3.** An escape-time or simulation picture is opaque wherever it
  is drawn. Mode 1 would turn it into a luminance matte, so it stores
  the colour and the coverage.

---

## Per-engine behaviour

The escape-time and simulation engines write an image whose `a` is
coverage, at most 1, rather than a hit count, and they supply no sample
count ([COLOR.md](COLOR.md), *The other engines*). The logarithmic
branch needs both, and a flame's Log-calibrated settings render such an
image black.

**Entering the mode.** `entry_tone_mapping` in
[src/config/defaults.rs](../../src/config/defaults.rs) is the one rule,
applied by the Mode menu ([src/ui/render_mode.rs](../../src/ui/render_mode.rs))
and by a script's `escape` and `sim` handles. When a config moves from a
flame mode to Escape or Simulation while its tone map is Logarithmic, it
becomes Linear:

| entering | exposure | gamma |
| --- | --- | --- |
| Escape | 2.0 (`ESCAPE_EXPOSURE`) | 1.0 (`ESCAPE_GAMMA`) |
| Simulation | 1.0 (`DEFAULT_EXPOSURE`) | 4.0 (`DEFAULT_GAMMA`) |

A config already on Linear or Density, or moving between the two
non-flame modes, keeps its values. Under Linear the alpha is
`coverage · density_scale`.

**The Colors panel** in these modes (`Control` in
[src/ui/visibility.rs](../../src/ui/visibility.rs)):

| control | state |
| --- | --- |
| tone-map presets, Reset to Defaults | hidden: both restore Logarithmic |
| Tone Map Mode | greyed |
| Gamma Threshold, Brightness, Vibrancy, Highlights | greyed: read only by the logarithmic branch |
| Alpha Blend Low / High | greyed |
| Spatial Filter, Density Levels | hidden |
| Exposure, Gamma, Saturation, Hue Shift, Highlight Clipping, Density Scale, Tone Curve, Background Color | active |

---

## Focused Rendering and auto exposure

**Focused Rendering** forces every sample through a path that lands in
the view, and each forced sample deposits about one unit of weight,
though it stands for many iterations of the unbiased chaos game. The
tone map is told so: `sample_density` is multiplied by
`cylinder_iteration_scale`, which is `Cylinders::iteration_scale`,
`1 / (mass · draw_scale)`. A forced render therefore tone-maps at the
brightness of the unbiased render it stands in for. Levels divide the
factor back out (`levels_density`). See
[FOCUSED-RENDERING.md](FOCUSED-RENDERING.md).

**Auto exposure** (`auto_exposure`, in the View panel's *Deep Zoom*
section, off by default) measures the share of plot attempts that land
in the frame, smooths it (`0.7 · old + 0.3 · new`), and multiplies
`sample_density` by it. A zoomed-in view, most of whose attractor is
off-screen, is then exposed for the samples it actually holds rather
than fading as it goes deeper. For a targeted render every sample lands
in frame, so the two factors cancel. It is off by default because almost
every view loses some samples off-frame, so turning it on changes the
brightness of nearly every render.

---

## Presets

`TONEMAP_PRESETS` in
[src/scene/tonemap_presets.rs](../../src/scene/tonemap_presets.rs) is a
hardcoded list, shown in the Colors panel's *Preset* dropdown (flame
modes only). A preset sets the complete tone-map state in one undo step:
exposure, gamma, gamma threshold, brightness, vibrancy, white level,
saturation, hue shift, the mode, the highlight mode, `use_curve` and the
curve's shape, Levels on/off with low, high and gamma, alpha blend low
and high, and density scale. It leaves the flame, palette, background
and colour mode alone. Every preset is Logarithmic.

| preset | departs from Default in |
| --- | --- |
| Default | nothing: the `FractalConfig` defaults |
| More Contrast | exposure 0.2, gamma 1.0 |
| Saturated Bright | gamma 3, threshold 55, brightness 30, saturation 1.5 |
| Saturated Darker | gamma 5.7, threshold 140, brightness 12, vibrancy 1.7, saturation 1.85 |
| Vivid | vibrancy 1.3, saturation 1.4 |
| High Contrast | gamma 1.5, Levels high 2, Levels gamma 1 |
| Quiet | exposure 0.6 |
| Dim | exposure 0.15, gamma 0.5, threshold 0, brightness 0.5, Levels gamma 0.4 |
| Saturated High Contrast | exposure 0.016, gamma 0.35, brightness 1, saturation 1.8, Levels high 5, Levels gamma 0.6 |
| Saturated Balanced | exposure 0.15, gamma 1.6, threshold 10, brightness 15, saturation 1.5, Levels high 5 |
| Saturated Dim | exposure 0.01, gamma 0.3, threshold 0, brightness 0.7, saturation 3, Levels high 0.14, Levels gamma 1 |
| Pop | exposure 0.01, gamma 0.3, threshold 2, brightness 1, vibrancy 6.1, saturation 1.5, Levels high 1, Levels gamma 1 |
| Saturated Very Bright | exposure 0.3, gamma 0.5, threshold 2, brightness 1, saturation 2.1, Levels high 5, curve off, Levels off |
| Flat | exposure 0.07, gamma 0.4, threshold 4, brightness 0.6, saturation 1.5, Levels gamma 1.2, curve off, Levels off |
| Easy Mode | saturation 1.5, MaxNorm, curve off |
| Easy Mode (Low Contrast) | as Easy Mode, Levels gamma 0.05 |
| Easy Mode (Light Background) | as Low Contrast, alpha blend low and high 1.0 |

The panel's *Reset to Defaults* button
(`reset_colors_to_defaults` in [src/ui/mod.rs](../../src/ui/mod.rs))
restores the colour mode, the tone-map mode and most tone-map and
palette-transform settings to their `defaults.rs` values. It leaves the
palette and the background alone.

---

## Parameter reference

All of these are `FractalConfig` fields, in the Colors panel. "Log"
means the control is read only by the logarithmic branch.

| control | field | UI range | default | read by |
| --- | --- | --- | --- | --- |
| Exposure | `exposure` | 0.001–10 | 1.0 | all modes |
| Gamma | `gamma` | −1–10 | 4.0 | all modes |
| Gamma Threshold | `gamma_threshold` | 0–1000 | 5.0 | Log |
| Brightness | `brightness` | 0.001–100 | 4.0 | Log |
| Vibrancy | `vibrancy` | 0–30 | 1.0 | Log |
| Highlights | `white_level` | 50–1000 | 200 | Log |
| Saturation | `saturation` | 0–3 | 1.0 | all modes |
| Hue Shift | `hue_shift` | −360–360 | 0 | all modes |
| Tone Map Mode | `tonemap_mode` | Linear, Logarithmic, Density | Logarithmic | |
| Highlight Clipping | `highlight_mode` | Clip, MaxNorm, Reinhard, Filmic | Clip | all modes |
| Alpha Blend Low | `alpha_blend_low` | 0–1 | 0.3 | Log, Density |
| Alpha Blend High | `alpha_blend_high` | 0–1 | 0.8 | Log, Density |
| Density Scale | `density_scale` | 0.01–10 | 1.0 | all modes |
| Enable Tone Curve | `use_curve` | checkbox | on (identity curve) | all modes |
| Levels | `levels_enabled` | checkbox | on | flame modes |
| Low / High / Midtones | `levels_low` / `levels_high` / `levels_gamma` | see [Levels](#levels) | 0 / 10 / 0.5 | flame modes |
| Background Color | `background_color` | sRGB colour | black | all modes |
| Auto exposure | `auto_exposure` | checkbox (View panel) | off | Log |

The Apophysis constants `BRIGHT_ADJUST` and `PREFILTER_WHITE` are not
settings.

# Filmic (tunable / Hable) highlight mode

**Status:** Idea. Researched only enough to know whether it's worth doing.
Implementation not started. Pick this back up when shipping needs it or
when the existing four `HighlightMode` operators feel insufficient.

## Why

Our current `HighlightMode::Filmic` is the [Krzysztof Narkowicz 2015 ACES
approximation](https://knarkowicz.wordpress.com/2016/01/06/aces-filmic-tone-mapping-curve/):

```wgsl
let a = 2.51;
let b = 0.03;
let c = 2.43;
let d = 0.59;
let e = 0.14;
color = (color * (a * color + vec3<f32>(b)))
      / (color * (c * color + vec3<f32>(d)) + vec3<f32>(e));
```

Those a–e values **look** like tunable parameters but they're the curve-fit
coefficients of a rational polynomial — change them and you no longer have
ACES, you have a non-physically-motivated curve. The right way to expose
tunable filmic behavior is to add a *different* operator from the family
that's actually designed to be parameterized.

## Operator choice: Hable (Uncharted 2)

[John Hable's Uncharted 2 filmic](http://filmicworlds.com/blog/filmic-tonemapping-operators/)
is the standard tunable filmic in games. Six per-channel parameters plus a
linear white point, all artist-friendly:

```wgsl
fn hable(x: vec3<f32>, A: f32, B: f32, C: f32, D: f32, E: f32, F: f32) -> vec3<f32> {
    return ((x * (A * x + C * B) + D * E) / (x * (A * x + B) + D * F)) - E / F;
}

fn hable_filmic(color: vec3<f32>, params: HableParams) -> vec3<f32> {
    let white_scale = vec3<f32>(1.0) / hable(vec3(params.white), params.A, ...);
    return hable(color, params.A, ...) * white_scale;
}
```

Defaults from Naughty Dog's GDC talk (these are the "Uncharted 2 look"):

| Param | Default | Purpose |
|---|---|---|
| A | 0.15 | Shoulder strength |
| B | 0.50 | Linear strength |
| C | 0.10 | Linear angle |
| D | 0.20 | Toe strength |
| E | 0.02 | Toe numerator |
| F | 0.30 | Toe denominator |
| W | 11.2 | Linear white point |

Hejl-Burgess-Dawson is a simpler one-knob variant (just white point); skip
it — if a user wants tunable they'll want the full Hable controls.

## Implementation sketch

Coexists with the existing `HighlightMode::Filmic` (ACES Narkowicz), doesn't
replace it. Apophysis users get the no-knobs ACES default; users wanting
control opt into Hable.

1. **`HighlightMode` enum** ([src/scene/tonemap.rs](../../src/scene/tonemap.rs))
   gains a `HableFilmic` variant.
2. **`FractalConfig`** gains a `hable_params: HableParams` struct with
   the seven knobs above. Serde-default to the Uncharted 2 numbers so old
   `.fflame` files still load.
3. **`TonemapParams` uniform** ([src/gpu/buffers.rs](../../src/gpu/buffers.rs))
   gains the seven f32s. Mind the std140 padding — keep the trailing
   `_pad_highlight` slots in sync (we burned an hour on a 144-vs-160 byte
   mismatch in Phase 1 of the highlights work).
4. **Shader** gets a new branch in the `highlight_mode == ...` switch at
   [shaders/tonemap.wgsl](../../shaders/tonemap.wgsl) ~line 580. ~20 lines
   of WGSL.
5. **UI** ([src/ui/tone_mapping.rs](../../src/ui/tone_mapping.rs)) — the
   sliders only show when `HableFilmic` is the selected mode. Maybe a
   "Reset to Uncharted 2 defaults" button.
6. **API v2 wire format**: extend `ApiHighlightMode` with `HableFilmic`,
   add a `hable_params` field to `CreateFlameRequest` / `FlameResponse`.
   Additive on both sides.

Total: 1-2 days, mostly bookkeeping.

## Open questions

- Are the per-channel Hable params actually applied per-channel, or to
  luminance? Read the original talk — pretty sure per-channel, but worth
  confirming.
- One Hable parameter set per flame is what every game ships. Worth
  exposing presets ("Uncharted 2," "more contrast," "softer toe") rather
  than the raw a-f knobs? Probably yes, with a "show advanced" expander
  for the underlying params.
- Should `White Point` move out of `HableParams` into a top-level
  config field? It's meaningful for other operators too (Reinhard has a
  variant with white point). Probably keep it inside HableParams for now,
  promote later if other operators want it.

## Why not just expose the existing a-e

Tempting because the constants are right there in the shader. But they're
coefficients of a curve-fit to the ACES RRT+ODT pipeline — there's no
geometric meaning to "raise `c` by 10%." Hable's parameters are designed
around toe / shoulder / linear-section semantics that artists can
actually reason about.

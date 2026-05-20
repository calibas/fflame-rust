# 3D LUT support (color grading)

**Status:** Idea. Researched only enough to scope it. Not started.
Pick this up if a user with a color-grading workflow asks for it, or
when the current tone curve LUT feels insufficient.

## What we have today (1D LUT)

`curve_lut_texture` at [shaders/tonemap.wgsl:67](../../shaders/tonemap.wgsl#L67)
is a 256-sample 1D LUT, declared as `texture_2d<f32>` with height=1 for
WebGPU compatibility. It's sampled per-channel at
[lines 612-614](../../shaders/tonemap.wgsl#L612-L614):

```wgsl
let curve_r = textureSample(curve_lut_texture, curve_lut_sampler, vec2<f32>(color.r, 0.5)).r;
let curve_g = textureSample(curve_lut_texture, curve_lut_sampler, vec2<f32>(color.g, 0.5)).r;
let curve_b = textureSample(curve_lut_texture, curve_lut_sampler, vec2<f32>(color.b, 0.5)).r;
```

This is the **tone curve** — produced by `ToneCurve::generate_lut()` in
[src/scene/tonemap.rs](../../src/scene/tonemap.rs). It can lift shadows,
crush highlights, build S-curves — anything that's a single monotonic
mapping from input intensity to output intensity, applied to each channel
independently.

What it **can't** do: cross-channel color grading. "Shift blues toward
teal while leaving reds alone" is a 3D operation — the output of any
channel depends on the input of *all* channels. That's what a real LUT
delivers.

## What a 3D LUT does

Input: a `vec3<f32>` color. Output: a graded `vec3<f32>` color. The LUT
stores a sparse cube of samples (typically 17×17×17 or 33×33×33 RGB
triplets); a trilinear interpolation between the 8 nearest cube corners
gives the output. Industry-standard format is the `.cube` text file from
DaVinci Resolve / Adobe / Blender / nuke.

`.cube` files are dead simple:

```
LUT_3D_SIZE 33
0.000000 0.000000 0.000000
0.031250 0.000000 0.000000
...
1.000000 1.000000 1.000000
```

Header + N³ RGB triplets in lexicographic order (B outermost, then G,
then R innermost — confirm by reading the spec when implementing).

## Implementation sketch

Coexists with the existing tone curve — they compose naturally. Apply
1D first (tone shaping), then 3D (color grading). Or expose user choice
of order.

1. **Shader** — new bind: `var lut3d_texture: texture_3d<f32>;` plus a
   sampler. Sample with `textureSample(lut3d_texture, lut3d_sampler,
   color)` — that's it, WGSL handles trilinear interpolation natively.
   Roughly 10 lines of WGSL in [tonemap.wgsl](../../shaders/tonemap.wgsl)
   after the existing tone curve block.

2. **Rust** —
   - `.cube` parser, ~80 lines. Parse header, parse triplets, validate
     `n³` count.
   - 3D texture upload — `device.create_texture` with
     `dimension: TextureDimension::D3`, then `queue.write_texture`. Maybe
     50 lines of plumbing.
   - Default LUT (identity grade) so the shader binding always has a
     valid texture even when no user LUT is loaded. 17³ identity is
     trivial to generate.

3. **Config** — `FractalConfig` gains:
   - `lut3d: Option<Lut3dRef>` where `Lut3dRef` is either an inline path
     or an API ID (matches palette flow).
   - `lut3d_strength: f32` — 0.0 to 1.0, mixes between raw color and
     LUT-graded color.
   - Both serde-skipped at default for compact JSON.

4. **UI** — file picker for `.cube` import (same pattern as palette
   import), a strength slider, a "clear LUT" button. Could live in the
   Tone Mapping panel as a separate collapsing section.

5. **API v2 wire format** — LUTs probably want to be first-class
   entities like palettes (content-addressable upload, reference by ID).
   ~570 KB per LUT (33³ × 4 channels × 4 bytes) is fine to ship inline
   on a flame but better stored once and referenced. Mirror the palette
   flow: POST `/api/luts`, get ID, reference from flame.

6. **WebGPU 3D texture support** — in spec, browser support real but
   worth smoke-testing on Chrome and Firefox. SwiftShader (our Windows
   fallback) supports 3D textures.

Total: roughly 1-2 weeks. Most of the time is in palette-style upload
flow + UI polish, not in the actual shader or LUT math.

## Composition with the existing 1D tone curve

Two reasonable choices:

**Option A: 1D first, then 3D.** Tone curve does pre-grade intensity
shaping (S-curve contrast, lift shadows), then 3D LUT applies creative
look. This matches Resolve/Premiere where exposure/curves come before
LUT.

**Option B: 3D LUT subsumes 1D curve.** A 3D LUT can encode a diagonal
of any 1D operation, so theoretically the user could bake their tone
curve into the LUT. But that's a worse UX — the 1D editor is interactive
and visual, the 3D LUT is a baked artifact.

Go with A.

## Open questions

- **Memory** — at 33³, that's 570 KB. At 65³ (DaVinci's high-quality
  default), 4.5 MB. Acceptable but worth deciding the default size.
- **Animation** — should `lut3d_strength` be animatable? Probably yes,
  trivially via ConfigPath.
- **WASM** — can the browser read user-selected `.cube` files via the
  same file picker we use for `.fflame` files? Probably yes (it's just
  text), but verify the picker accepts the extension.
- **HDR LUTs** — `.cube` supports `DOMAIN_MIN`/`DOMAIN_MAX` for input
  ranges outside `[0,1]`. Our pipeline is sRGB-clamped already; either
  ignore extended-range LUTs or treat them as if clamped. Decide.

## Why not just extend the 1D LUT to 3D in-place

The existing `curve_lut_texture` binding could be replaced with a 3D
texture, but that breaks all existing `.fflame` files that ship a 1D
tone curve. Cleaner to keep them separate — 1D for the tone curve, 3D
for the LUT — and let them compose.

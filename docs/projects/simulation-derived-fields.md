# Simulation: derived fields — resolve, matte edges, vector colourings

**Status:** plan of record, 2026-09-06. **Phases A and B built and
gated the same day**; C and D to follow. The IFS phase is scoped, not
scheduled.

This is the plan for three requests made together: a low-resolution
grid that presents as a detailed picture rather than a pixelated one;
colourings driven by vector and tensor quantities; and, later, the
field used as a transform inside the flame renderer. They share one
idea, which is why they are one plan.

## 1. The idea: derive, do not store

A cell is a `vec4<f32>`. Models already read the four channels as
whatever they need — Gray–Scott's A and B, the snowfake's a, b, c, d,
the breakdown model's key, φ, age, occupancy — and read as a 2×2
matrix a texel *is* a rank-2 tensor. Storage is not the gap.
Interpretation is.

The colourings already receive one derived quantity: `grad`, the
central-difference gradient of `.x`, computed from the state at colour
time for colourings that declare `NeedsGradient`. Everything in this
plan extends that pattern — **quantities computed from the state when
the picture is made, never written into the field** — so no model
changes, no channel is spent, and a run's reproducibility is untouched
because the step never sees any of it.

What is derived, and where it is consumed:

| derived field | from | consumed by |
|---|---|---|
| interpolated state | the 2×2 or 4×4 cell neighbourhood | the resolve (phases A, B) |
| gradient | central differences of a channel | colourings (exists), the matte edge (D) |
| signed distance | occupancy, by jump flood | the matte edge (C) |
| structure tensor | smoothed gradient outer product | colourings (D) |
| velocity | a model that has one, stored as magnitude + angle | colourings (D) |

## 2. What the resolve does today, and the finding that orders the plan

`sim_shade(cell)` reads the state, computes the gradient, colours the
cell and applies the matte. The **bilinear upscale then blends four of
those colours** ([assembler.rs](../../src/sim/assembler.rs),
`resolve_body`). That is the wrong order for a picture: a magnified
boundary is a smear of palette entries, and a magnified matte edge is
an eight-pixel ramp of half-drawn cells.

Interpolating the *state* and colouring once puts every isoline of the
field where it belongs — a crisp curve at the exact level — and puts
the matte's cutoff on the interpolated occupancy, which makes a 0/1
edge a hard sub-cell boundary at the 0.5 isoline instead of a blend.
That is the "heat map" look the request describes, and it is the
first phase because it costs almost nothing and everything after it
assumes the order is right.

Downscaling is the other way round on purpose. A box average over
colours is a filter over the finished image — supersampling — and is
correct for any palette. Averaging state and colouring the average is
wrong for a non-linear palette. Downscale stays as it is.

## 3. Phases

### Phase A — interpolate state, then colour

- `sim_shade` splits: `sim_state(cell)`, `sim_grad(cell)`, and
  `sim_shade_from(state, grad, cell)` which colours and mattes.
- Nearest: unchanged in effect (`sim_shade_from` of one cell).
- Bilinear: the four states and, for colourings that want it, the four
  gradients are interpolated; **one** colouring call on the result.
  The cell coordinate handed to the colouring is the nearest cell —
  no colouring reads it today, and the test in §4 says so.
- Box downscale: unchanged (colour average, §2).
- ~~The `gray-scott-bilinear-upscale` baseline moves.~~ **It did not,
  and the reason is the point.** That config is a greyscale palette
  through the `channel` colouring with no matte — an affine map into
  a linear palette — which is exactly the case where
  `palette(mix(a, b)) == mix(palette(a), palette(b))`, so the two
  orders agree to the bit. The difference the change makes appears
  only where the colouring is non-linear: a clamp being crossed, a
  palette with structure, or a matte. So the visual suite had no
  baseline that exercised the resolve order at all, and it now has
  one: `eden-matte-bilinear`, a 48² cluster magnified with a hard
  matte, whose edge is a sub-cell boundary rather than a ramp.

**Gate (built 2026-09-06, passing):** at 8× with the matte on, a 0/1
occupancy edge has NO output pixel of partial coverage — a hard cutoff
on a ramp — where the old path had a full cell's width of them
(`the_bilinear_upscale_interpolates_state_not_colour`); at 1:1,
Bilinear equals Nearest byte for byte, because interpolation at cell
centres is the identity; every Nearest render and every existing
baseline is unchanged (310 of 310). The plan's risk that a colouring
might read the cell coordinate is a test now
(`no_colouring_reads_the_cell_coordinate`).

### Phase B — bicubic upscale

- `SimUpscale::Bicubic`, Catmull–Rom on the 4×4 neighbourhood of
  state (and gradient, where wanted). C¹ where bilinear is C⁰: blobby
  squares become smooth contours.
- The panel's upscale picker gains the entry; the config enum and its
  names round-trip like the others.

**Gate (built 2026-09-06, passing):** a Gaussian bump (σ = 2.5 cells)
written straight into a 32² field and rendered at 8× through the
`channel` colouring, whose linear greyscale palette returns the
interpolated value itself, compared with the analytic function at each
output pixel's centre over the bump's support:

| upscale | RMS error | worst |
|---|---|---|
| nearest | 0.0372 | 0.134 |
| bilinear | 0.0065 | 0.035 |
| bicubic | **0.00064** | **0.003** |

Ten times better than bilinear against a bar of two
(`the_bicubic_upscale_reconstructs_a_smooth_field_better`). Nearest
and Bilinear are unchanged — every existing baseline held — and a
second magnified-matte baseline, `eden-matte-bicubic`, is the same
cluster as phase A's through the new filter. Writing the field in
from a test needed `COPY_DST` on the field textures, which the §7
resize resampler will need anyway.

### Phase C — a distance field for occupancy edges

Interpolating a 0/1 field only ever gives a ramp one cell wide, and
phase A's hard cutoff on that ramp is a boundary that is exactly right
at the 0.5 isoline and wobbles between. The right primitive for a
boundary at any magnification is a **signed distance field**: convert
the matte's occupancy to the distance from the nearest occupied cell
(jump flood — ⌈log₂ N⌉ passes over a `rg32float` seed-coordinate
texture pair), sample *it* with the resolve's interpolant (distance
fields interpolate well), and take coverage from a sub-cell threshold
on the interpolated distance. This is font rendering's trick, and it
makes a 128² DLA present at 4K with crisp, smooth dendrite edges.

- `SimMatte` gains `edge: Threshold | Distance`; the SDF runs only
  when the matte is on and `Distance` is chosen, at colour time, so a
  paused run recomputes nothing and a running one pays ⌈log₂ N⌉
  cheap grid-resolution dispatches per frame.
- The SDF also feeds the feather: `softness` becomes a distance in
  cells, which is what a user means by it.
- New machinery: a texture pair, a pipeline, a bind group, and a
  binding in the colour template. Modelled on the pyramid stage.

**Gate:** a disc of occupied cells at 8× — the coverage isoline lies
within 0.15 output pixels of the analytic circle everywhere on its
circumference (measured as the RMS radial error), and the interior and
exterior are exactly 1 and 0.

### Phase D — vector colourings

All derived at colour time, none needing a model change except the
last:

- **`gradient`**: direction to hue, magnitude to value — the flow
  picture, on any model. Uses the existing `grad`.
- **`structure`**: the structure tensor (gradient outer product,
  smoothed over a small window) — its coherence separates ridges from
  blobs, its orientation gives the local texture direction. A second
  `ColoringFeature` so only this colouring pays for the window.
- **`flow` for models that have a velocity**: fingering computes
  `u = −m∇p` and keeps only `|u|` (channel `.x`, for the reduce). It
  stores the angle in the spare `.w` (`ff_atan2`, for Metal) so the
  vector is recoverable, and a colouring draws it. Model-specific and
  optional.
- **Line integral convolution** is the flow colouring everyone wants
  and is a hundred samples per pixel along the field. Last, and only
  if the cost measures acceptable at 1080p.

**Gate:** each colouring is checked against a CPU evaluation of its
formula on a read-back field, the way the existing colourings are;
`every_preset_draws_something` still passes; no existing baseline
moves.

### Later — the field as a flame transform

Not scheduled. What it needs, so the phases above build toward it
rather than away: a variation that samples a texture (a bind group
entry in the flame compute shader, and a `Feature` for variations
that read one); the simulation renderer kept alive beside the flame
renderer; a decision about the field's coordinate frame, its behaviour
past the grid, and whether it is frozen at a step or live. The natural
reading of a texel is a **local affine** — the flame's own `(a, b, c,
d)` — so the field becomes a spatially varying transform, and the
derived-field texture phases A–D produce is exactly what such a
variation would sample.

## 4. Test strategy

- Every gate above is a number, measured in `app_repro_test.rs` on a
  read-back render at a fixed magnification, against either the old
  path's behaviour, an analytic field, or a CPU evaluation.
- Two invariants protect what must not change: every Nearest render
  and every 1:1 render are unchanged by A and B; no existing visual
  baseline moves in any phase except the one bilinear-upscale baseline
  in A, which is regenerated and inspected.
- Naga validation of every colouring × boundary × resolve combination
  extends to the new entries automatically.

## 5. Risks

| risk | consequence | mitigation |
|---|---|---|
| a colouring that reads the cell coordinate | phase A hands it the nearest cell, which is wrong under interpolation | none does today; a test greps for it, so a future one fails loudly |
| the SDF's cost on a running simulation | ⌈log₂ N⌉ dispatches per frame at grid resolution | only when the matte is on and `Distance` is chosen; measured at 1080p before it ships |
| interpolation inventing detail | a smooth picture read as a finer one | said plainly in the tooltips: the grid is the information, the resolve is presentation |
| the bilinear baseline moving | a diff nobody inspected | inspected and described in the commit before it is accepted |

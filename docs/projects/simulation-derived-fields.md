# Simulation: derived fields — resolve, matte edges, vector colourings

**Status:** plan of record, 2026-09-06. **Phases A–D built and gated
the same day** — C not as expected, see its section. The IFS phase is
scoped, not scheduled.

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
| signed distance | occupancy, by jump flood | the matte's feather (C); distance colourings (D) |
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

### Phase C — a distance field from the occupancy

**Built 2026-09-06, and it does not do what this section expected.
The expectation is kept here, struck through, because the measurement
that overturned it is the useful part.**

~~Interpolating a 0/1 field only ever gives a ramp one cell wide, and
phase A's hard cutoff on that ramp is a boundary that is exactly right
at the 0.5 isoline and wobbles between. The right primitive for a
boundary at any magnification is a signed distance field … the font
rendering trick, and it makes a 128² DLA present at 4K with crisp,
smooth dendrite edges.~~

**What was measured.** A disc of radius 10.3 cells on a 32² grid,
magnified 8× with a hard matte, through both edges: the threshold and
the distance field classify **the same 568 pixels** the same way, with
**the same 1.034 px RMS radial error**. Not close — identical. The
reason is structural. A cell beside the edge has its nearest cell of
the other kind *adjacent*, so it reads exactly +½ (inside) or −½
(outside) in the distance field — which is the occupancy the threshold
interpolates, shifted by a half. In every cell square the boundary
passes through, the four corners are ±½, and the bilinear zero set of
that is the bilinear ½-set of the occupancy. They differ only where a
square has a cell two steps from the edge (a staircase corner), and
there by ~0.02 cells: working the (1,1,1,0) square by hand, the
occupancy's ½-isoline crosses the diagonal at t = 0.707 and the
distance field's zero at 0.729. **A distance field built from cell
centres knows no more about *where* the edge is than the cells do.**
The font-rendering analogy was wrong: a glyph's SDF is sampled from an
*exact* outline; ours is sampled from the occupancy, which is the
whole of what the simulation knows.

The 1.03 px (0.13 cells) both edges achieve is the occupancy data's
own resolution of a curve, and phase A already had it.

**What the distance field is actually for** — and it stays, because
this is real:

- **`softness` is a width in cells.** Under the threshold a feather is
  measured in the channel's units, so the same setting is a different
  width on every model and a wide one is a smear across a ramp. Under
  the distance edge a 2-cell feather is 2 cells wide: measured, 32
  output pixels per crossing at 8×. The field reads +0.5 / −0.5 either
  side of a straight edge and ≈ R at a disc's centre, so the feather
  is centred on the boundary and honest at any width.
- **Phase D's distance colourings.** Distance from the crystal, an
  outline, a glow that falls off in cells — all read this field, and
  none could exist without it. This is the derived field §1's table
  promised, and it is now produced.

What shipped: `SimMatteEdge { Threshold, Distance }` on the matte,
`ConfigPath::SimMatteEdge` through the tables; the jump flood as three
templates (seed, jump, seeds-to-distance) over a grid-sized texture
pair and a result texture, allocated when the edge is `Distance` and
freed when it is not (three grid-sized textures are 800 MB at 4K);
⌈log₂ N⌉ + 2 dispatches per coloured frame; a sixth binding in the
colour template, a 1×1 dummy when unused and never read then. The
seeds-to-distance pass had a bug the disc's centre caught: halving the
difference of the two distances is right beside the edge and half the
distance everywhere else, and it is `d_out − ½` inside, `½ − d_in`
outside. Distances are measured within the grid, so at a periodic
seam they are measured to the seam; said on the tooltip.

**Gate (passing):** the two edges agree on the disc's boundary to
within 5 % of the disagreeing pixels and 0.1 px RMS; the field is
±0.5 across a straight edge and within 1.5 cells of R at the centre; a
2-cell feather is 32 ± 8 output pixels per crossing at 8×
(`the_distance_matte_agrees_on_the_edge_and_feathers_in_cells`). The
test carries the finding in its name so nobody restores the
expectation.

### Phase D — vector colourings

**Built 2026-09-06.** All derived at colour time; no model changed.

**The interface first.** A colouring now receives one `SimSample` —
the state, the gradient of *every* channel, the signed distance, the
structure tensor — built per cell by `sim_sample` and **interpolated
by the resolve** like the state (bilinear lerps it, bicubic
accumulates it). A colouring never reads a neighbour itself, so under
a magnifying filter it sees smoothly varying derived quantities
without doing anything; and each derived quantity is zero, and its
reads skipped, unless the colouring declares the feature that pays
for it (`NeedsGradient`, `NeedsStructure`, `NeedsDistance` —
`a_colouring_without_needs_gradient_reads_no_neighbours` checks the
generated WGSL for both the gradient reads and the tensor window). The
six existing colourings changed only their signature.

- **`gradient`** — the gradient of a chosen channel: direction through
  the palette (`ff_atan2`, exact at the zero pair a flat cell hands
  it), magnitude as brightness. Gradient of every channel from the
  same four reads is what made the plan's `flow` entry unnecessary:
  fingering's velocity is `−m∇p`, and ∇p is the gradient of `.y`, so
  its direction is exact and its speed right up to the mobility,
  without the model storing an angle.
- **`structure`** — the structure tensor of `.x` over a 3×3 binomial
  window: orientation (half a turn is the palette once, bright where
  strong), coherence (`(λ₁−λ₂)/(λ₁+λ₂)`: 1 on a line, 0 on a blob),
  energy.
- **`distance`** — depth into the figure, an outline, or the signed
  distance, in cells, from phase C's field; the renderer builds the
  field for this colouring whatever the matte's edge. Found while
  writing its visual config: the matte cuts everything outside the
  figure, so an "outside" mode is never seen — the modes colour the
  figure by its own depth, and the space around a cluster is coloured
  by inverting the matte. Said on the tooltip.
- **`lic`** — line integral convolution: per-cell noise (keyed by cell
  and seed, not step, so it holds still while the run advances)
  averaged along the gradient of a channel or its perpendicular,
  `length` cells each way, the sign kept continuous. It has to walk
  the field, so it declares `ReadsCell` and is the one colouring
  computed at cell resolution and then interpolated.

**Measured**, ms per coloured 1080p frame, on a noise-filled field so
no cell is flat (the LIC's worst case — on a seeded run most walks
exit at once and it measured 0.7 ms):

| colouring | ms |
|---|---|
| `channel` (reference) | 0.68 |
| `gradient` | 1.03 |
| `structure` | 1.69 |
| `distance` | 7.86 — the jump flood, phase C's cost, the same the Distance edge pays |
| `lic`, length 8 (default) | 3.05 |
| `lic`, length 24 | 8.70 |

**Gate (passing):** `gradient` and `structure` (all three modes) agree
with a CPU evaluation of their formulas on a read-back Gray–Scott
field, through the linear test palette, **exactly** (worst mismatch
0.0 over 64²); `distance` agrees with the field it reads to 6e-8 in
each mode, with the matte's edge left at Threshold so the colouring's
own feature is what builds the field; the LIC is deterministic, and
its output differs 1.87× more across the flow than along it (0.388 vs
0.207 mean step difference). `every_preset_draws_something` passes;
the 75 sim baselines are unchanged under the new template; five new
baselines: `gray-scott-gradient`, `cahn-hilliard-structure`,
`dla-distance-depth`, `dla-distance-glow` (inverted matte),
`fingering-lic` (the flow through the fingers).

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
| a derived field promising more than the data holds | phase C's edge gate, written before the measurement | the gate measured it, the section keeps the struck-through expectation beside the finding, and the tooltip says what the field is for |
| the bilinear baseline moving | a diff nobody inspected | inspected and described in the commit before it is accepted |

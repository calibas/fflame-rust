# McCabe multi-scale: colour memory, a per-scale table, variation radius

Branch `mccabe-scales`, started 2026-10-04. **Phases 1 (colour memory)
and 2 (the per-scale table) done 2026-10-04**; results at the end of
sections 2 and 3. The model is catalogue
§10 (`mccabe` in [src/sim/models.rs](../../src/sim/models.rs)). This
plan brings it up to what the paper and the two implementations
everyone copies actually do. Decisions a reader should argue with are
marked **decision**.

## 0. Why: the sources against the model

Read 2026-10-04:
- McCabe's paper (Bridges 2010), our saved copy at `output/mccabe.txt`.
- Softology's 2011 post (Jason Rampe).
- Ricky Reusser's WebGL source (`step.js`, `initialize-kernel.js`,
  `index.js` in rreusser.github.io) and his WebGPU notebook.

| | paper | Softology | Reusser | `mccabe` today |
|---|---|---|---|---|
| the rule: argmin of \|a − b\|, ± step, renormalise to [−1, 1], periodic | ✓ | ✓ | ✓ | ✓ |
| radii | none given | free per scale | free per scale (default 250/45/20/10/3/1, ratio 2) | a doubling ladder from one radius, one ratio |
| step | — | per scale | per scale, one **negative** in the defaults | interpolated finest → coarsest, positive |
| weight | — | per scale: both averages × w | in the UI; unused in the 2018 code | none |
| variation smoothing | "variation around the pixel" | a variation radius | none | none |
| symmetry | global, and per scale (fig. 14: 3-fold small, 9-fold large) | per scale | per scale | one global |
| colour | greyscale | a per-cell colour lerped toward the winner's colour each step, × brightness | the same, the scale's step as the lerp factor | `scale_mix`: this step's winner's colour, no memory |
| compound (weighted sum of scales) | figs. 4–6, 13 | — | — | none |

The averages differ by design and stay as they are: our Gaussian
pyramid, calibrated to disc size, is O(1) per cell per scale. Softology
used separable Gaussians and then a 3× box blur; Reusser uses FFT with
exact discs or Gaussians.

## 1. Order

1. **Colour memory**, on internal slices (§2). The biggest visible gap.
2. **The per-scale table** (§3).
3. **Variation radius** (§4).
4. **Compound mode**, as an experiment (§5).

Each phase ships with the four `mccabe-*` visual baselines
byte-identical at its defaults, and the CPU mirror
(`mccabe_matches_a_cpu_mirror`) extended to cover what it adds.

## 2. Phase 1: colour memory

**What the references do.** Softology keeps an RGB colour per cell,
starting black. Each step it is lerped toward the winning scale's
colour by a bump amount, and drawn as colour × (f + 1)/2.

**What we store instead: the weights.** That colour is linear in a
one-hot of the winner. So a cell's colour is Σᵢ mᵢ cᵢ, where m is the
same lerp applied to the one-hot:

```
m ← (1 − b)·m + b·e_winner        m starts at 0 (black)
```

Storing m rather than the RGB keeps the palette a display-time choice.
A palette edit shows at once, even paused, and the timeline's
picture-at-a-step contract holds. With a fixed palette the picture is
the references' exactly, in float. **Decision** (asked, 2026-10-04):
weights over baked RGB.

**Where m lives: internal slices.** Two extra slices of the field
array, appended after the user's layers, hold 8 weights (6 scales, 2
spare). Appending them leaves every user-facing layer index
unchanged: couplings, the colour stack, `gather`, `minmax_back`.

| stage | what an internal slice gets |
|---|---|
| seed | zeros |
| warp, layer map | its owner's warp and rate, every channel, so memory moves with the pattern |
| pyramid, reduce | nothing (not built over it) |
| the owner's memory-writing pass | written by the owner's dispatch; no dispatch of its own that stage |
| every other stage | copy-through, as for any layer without a pass |
| resize | reseeded with the field (a bound grid's resize restarts the run) |

- **The model side.** `ModelFeature::Memory`, and a slice count from
  the parameters: 2 when `memory` > 0, else 0. The step template
  splices `sim_mem_read(p, k)` (from `field_in`) and
  `sim_mem_write(p, k, v)` (to `field_out`). The slices' count and
  first index sit in the last two slots of the layer's parameter block,
  which the colour pass reads too. McCabe's step updates the weights
  after choosing the winner.
- **The colouring side.** `ColoringFeature::ReadsMemory`. `SimSample`
  gains `m0`, `m1` (vec4), read from the source layer's memory slices,
  and lerped and summed in the resolve like the state. Interpolating
  weights is interpolating colours, which is right. A source layer
  without memory reads zeros.
- **The colouring.** `scale_memory`: Σ mᵢ · palette((i + ½)/bands),
  times `mix(1, (f + 1)/2, value_scale)`, Softology's brightness. Its
  parameters are the same two as `scale_mix`.
- **Turning memory on or off restarts the run.** The slice count is
  part of `SeedIdentity`. A memory switched on mid-run would make the
  picture at step N depend on when it was switched on. Changing `b`
  while memory stays on does not restart, like any parameter.
- **Memory cost.** Two slices, ping-ponged: 64 B a cell, about
  **133 MB at a 1080p grid**, only while memory is on. (The first
  estimate given was 66 MB, for one slice; six scales need two.)

**Gates.**
- `memory` = 0: every baseline is byte-identical.
- The CPU mirror covers m across 3 steps, to the field's tolerance.
- `steps_are_batch_invariant` passes with memory on.
- A one-cell integer pan of the warp moves m exactly with the field.
- A paused palette edit changes the picture.
- Toggling memory reseeds; changing b does not.
- A preset, run and inspected before it ships.

**As built.** The plan held, with one change: no gate on the shader
text. Every model's step shader gains the `MODEL_PARAM_SLOTS`
constant, and McCabe's gains its memory code. Simulation shaders are
not among the canonical dumps, so the baselines are the gate.

| gate | result |
|---|---|
| baselines at `memory` = 0 | 92/92 byte-identical; the new `sim-mccabe-memory` makes 93 |
| CPU mirror, 5 steps, the GPU's own winners | worst 6e-8 |
| field with memory on | bit-identical to memory off, every step |
| batch invariance, 300 steps | every slice bit-identical |
| one-cell Nearest pan | every weight moved exactly; the unshifted check fails, so the test can tell |
| toggling / changing b | reseeds / does not |
| colour stack | `scale_memory` on the McCabe layer of a two-layer config lights every cell; on the Gray–Scott layer, black |
| cost at 1080p | 4.17 → 4.29 ms/step, +3% |
| `every_preset_draws_something` | passes with the new `memory` preset |

The palette-edit gate is structural: the colouring reads the palette
at display time and the weights do not depend on it.

**Seen.** At 256² and 512², `scale_memory` blends where `scale_mix`
speckles inside a region. Moving boundaries leave soft trails of the
scale they displaced. Output in `output/mccabe_mem/`.

**Found.** On small grids the coarsest scale can win everywhere: its
activator and inhibitor both clamp to the pyramid's top level, and the
variation goes to ~0. The warp test hit it at 40² with five scales,
and now uses three. Section 3's warning is for this.

## 3. Phase 2: the per-scale table

- **`layout`**: Ladder (today's, the default) or Table.
- **Table mode**, per scale i = 0..5:
  - `s{i}_radius`: activator, 0.5–256 cells;
  - `s{i}_ratio`: 1.25–4;
  - `s{i}_amount`: −0.2–0.2, so a negative step is allowed;
  - `s{i}_weight`: −4–4, default 1;
  - `s{i}_symmetry`: 0–8.
- **Weight is Softology's.** Both averages are multiplied by w. So the
  variation is |w|·|a − b|, which biases which scale wins, and a
  negative w flips the direction.
- **Switching to Table fills it from the current ladder.** The switch
  changes nothing on screen. That is the gate: a table filled from the
  ladder is byte-identical to the ladder.
- **Parameter block 32 → 64 floats.** 6 globals plus 30 table entries
  plus phase 1's `memory` and phase 3's radius pass 32. The buffer is
  16 layers × 64 × 4 B = 4 KB.
- **UI.** A `ParamTable` on `ModelDef` (rows, column suffixes), drawn
  by the panel as a grid. Table entries are hidden in Ladder mode, and
  the ladder's in Table mode.
- **Radius past the pyramid.** An inhibitor read past the top level
  clamps there. Both averages then tend to the field mean, the
  variation tends to 0, and that scale wins everywhere. The panel
  warns when ratio × radius passes the top level's reach at the
  current grid. Reusser's 250/500 at 256² is such a scale on purpose,
  so it is warned about, not refused.

**Presets** (run and inspected before shipping):
- Reusser's 2018 defaults, 256².
- Fig. 14's mix: 3-fold on the small scales, 9-fold on the large.
- A wide-gap ladder ("large difference of activator and inhibitor
  radius between scales", Softology).

**Gates.** Ladder mode byte-identical; table-from-ladder
byte-identical; the CPU mirror with weights, a negative amount and
per-scale symmetry.

**As built.** As planned, with these differences:
- **The table is described, not hard-coded.** `ParamTable` in the
  simulation registry names the mode parameter, the row count, the
  columns, the generator's own parameters and a `fill` function. The
  panel draws any model's table from it, and `PARAM_TABLES` lists the
  one model that has one.
- **Per-scale symmetry goes to 12**, not 8. Fig. 14 needs 9, and the
  preset range test caught it.
- **Every McCabe preset now names `layout` and `memory`.** A preset
  sets only what it names, so a ladder preset applied in table mode
  stayed in table mode. Applied from a default state, the old presets
  are unchanged.
- **Table-from-ladder is not bit-identical**, and the gate says so (see
  the table below).

| gate | result |
|---|---|
| baselines, ladder mode | 93/93 byte-identical |
| table filled from the ladder, default and 5-fold coarse | one step: 0 cells choose another scale, field within 1.5e-8. 40 steps: 0 of 4,096 cells more than 0.05 apart |
| CPU mirror: weights 2 and −1, a negative step, symmetry on one scale | 4,096 of 4,096 cells exact (1.2e-7), no ties |
| `every_preset_draws_something` | passes with the two new presets |
| new baseline | `sim-mccabe-table`: every column away from its default |

**Why table-from-ladder is not bit-identical.** The ladder interpolates
its steps with WGSL's `mix()`. A driver may round that differently from
the CPU's `a(1 − t) + bt` in the last bit, and the run is chaotic, so
one ulp of step grows: 786 of 4,096 cells differed in their last bits
after 40 steps. None is visible: no cell is 0.05 apart. Matching the
driver's `mix` on the CPU is not portable, and changing the ladder's
arithmetic would move every existing McCabe picture.

**Presets** (512², inspected):
- **Mixed symmetry**: fig. 14's arrangement. The coarse ladder as a
  table, 3-fold on the three finest scales and 9-fold on the two
  coarsest. A 9-fold rosette of 3-fold detail.
- **Uneven scales**: radii 1, 3, 10, 20, 45, steps falling from finest
  to coarsest, with colour memory. A nested labyrinth.

Reusser's own 2018 table was not shipped. Its 250/500 scale is past
what the pyramid can average at any grid (8 levels reach about 233
cells), and its step order measured an axis bias (next).

**Found: tables whose coarse scales move fastest lean to the axes.**
*(Superseded by section 6: four seeds overstated it. Over 32 seeds the
lean is 1.08 ± 0.02 against 0.96–0.99 for exact averages, and its cause
is the pyramid's fixed lattice, not the read's shape.)*

Measured as spectral energy within 10° of the axes over that within
10° of the diagonals, mean of four seeds at 512² (1.0 is isotropic):

| table | axes / diagonals |
|---|---|
| the shipped coarse ladder (3 … 48, fine fastest) | 1.01 |
| the same ladder at base 2.5, 3.3, 3.6 | 1.00, 1.04, 1.10 |
| radii 1, 3, 10, 20, 45, **coarse fastest** (Reusser's order) | 1.25 |
| … with the negative step removed | 1.18 |
| … with the radius-1 row removed | 1.26 |
| … with the negative step on radius 10 instead | 1.18 |
| radii 1, 3, 10, 20, 45, **fine fastest** (the preset) | 0.99 |
| … adding a 100/200 scale | 1.12 |
| the doubling radii 2.5 … 40, coarse fastest | 1.06 |
| radii 1, 4, 16, 64 (wide gaps), fine fastest | 1.03 |

So neither the level a radius lands between nor the negative step is
the cause; the step order is. A likely reading, not proven: the coarse
scales read the pyramid's coarsest levels (a handful of texels at
512²). Bilinear reconstruction of so few texels is square, and when
those scales dominate the picture their squareness shows. A radius
near the pyramid's reach (the 100/200 row) does the same in a milder
form. Exact disc averages (FFT, parked) would be the fix. Until then,
the panel's reach warning catches the extreme case, and the presets
keep coarse scales slow.

## 4. Phase 3: variation radius

`variation` (0–3 cells): the variation averaged over a disc before the
argmin, as Softology and the paper describe.

That needs every scale's signed `w(a − b)` at the neighbours. So it is
two passes:
1. Write the signed values into two **scratch** internal slices.
2. Average their absolute values over the disc, take the argmin, and
   take the direction from the cell's own sign.

Scratch exists only while `variation` > 0. It does not persist, so
toggling it reallocates preserving the field (a slice copy) rather
than reseeding. 133 MB more at 1080p while on.

**Gates.** `variation` = 0 is byte-identical; the CPU mirror at radii
1 and 2.

## 5. Phase 4: compound mode, an experiment

The paper adds "multiple copies of the simple model (as a weighted
sum, with the possibility of negative weights)". It does not say
which sum, so both readings are tried:
- (a) Σ wᵢ·sᵢ·sign(aᵢ − bᵢ);
- (b) s·sign(Σ wᵢ(aᵢ − bᵢ)).

The target is the paper's figs. 4–6 description: "regions of light
dots balanced by regions of dark dots, within a larger stripe
structure". Whichever reading does that ships as `rule` = Compound,
with what was seen recorded in the catalogue. If neither does, nothing
ships and the attempt is recorded.

## 6. The axis lean, investigated (2026-10-04)

Section 3 found tables whose coarse scales move fastest leaning to the
grid axes. That was measured on four seeds, and the effect turned out
smaller than those suggested. It was then tested against exact
references, with fixes, in
[proto_mccabe_isotropy.py](../../scripts/sim_prototypes/proto_mccabe_isotropy.py),
which holds every number.

**It is real, and smaller than first reported.** On the coarse-fastest
table (radii 1, 3, 10, 20, 45), axes/diagonals, mean ± standard error:
- The shader, on the GPU, over 32 seeds: **1.08 ± 0.02**.
- The same rule with exact averages by FFT: a Gaussian 0.96 ± 0.02, a
  disc 0.99 ± 0.05.

A single seed spreads ±0.15, which is how four seeds read 1.25.

**It is the lattice, not the kernel.**
- **The kernel.** Plane-wave probes show the pyramid's
  activator-minus-inhibitor response is about 1% stronger along the axes
  at every radius; exact kernels show 0.1% or less. Its 7-tap build
  kernel `[1 6 31 52 31 6 1] / 128` matches a Gaussian's moments to
  fourth order and cuts that to 0.1%, but the dynamics did not move
  (GPU, 1.10 ± 0.02).
- **The lattice.** Moving the pyramid's lattice by a random offset
  every step, everything else unchanged, removes the lean entirely: 1.13
  → 0.97 on the CPU replica, lower on all eight seeds.

The bilinear reads crease along the coarse levels' texel lines, 32–64
cells apart, and a rule that turns on where `a − b` crosses zero locks
onto them. B-spline reads, smooth across texel lines, recover part of
it (1.03 ± 0.02 on the GPU over 32 seeds, 1.02 ± 0.03 on the CPU
replica over 8) at 4.1× the cost: 16.9 against 4.1 ms/step at 1080p.

**What was not kept.** The B-spline and round-kernel options were built
to measure on the GPU, then removed. The patch is
`output/sim_proto/isotropy/averaging_options_experiment.patch`.

**The fix this points to: per-step lattice jitter.**
- **The offset.** It comes from (seed, step), so a run stays
  reproducible and batch invariant.
- **The build.** Level 1 builds from the field at `2p + d − s`, and
  levels from 1 up are read at `pos + s`. Level 0 is the field and does
  not move.
- **The cost.** Almost nothing per read. The pyramid's parameters must
  become per step, since they are written once per batch today.
- **The price.** Somewhat more motion: direction flips per step 0.32 →
  0.37, per-step change +20%. Moving the lattice every few steps
  instead is the variant to measure.
- **Periodic only.** On a clamped or zero boundary a moved lattice
  would leave one edge's cells outside every texel.

**Separately: the disc look.** The exact disc's texture is finer and
sharper than any Gaussian method's, at the same calibrated feature
size. That is the disc-versus-Gaussian question, which only exact discs
(FFT, parked) answer. The calibration itself has also drifted since the
sampling-phase fix: at `CAL` 0.55 the pyramid's peak wavenumber is
66 / 18 / 9 against the disc's 54 / 16 / 8 at radii 3 / 10 / 20, so
its features are 12–20% too small.

### 6a. Built: the shifted grid (2026-10-04)

`averaging` = 1 ("Pyramid, shifted grid") moves the pyramid's lattice.
- **The offset.** A hash of (seed, step / 4) in [0, 128) per axis
  (`sim_lattice_shift`), carried to the shaders by the uniform's
  `xform.z` (the period, 0 when fixed).
- **The build.** Level 1 builds from the field at `2p + d − s`; the
  levels above inherit the offset.
- **The reads.** `pyr_level_avg` reads levels from 1 up at `pos + s`;
  level 0 is the field and does not move.
- **Per-step uniforms.** The pyramid's uniforms became a ring, one per
  (step, layer, level), since they were written once per batch.
- **Periodic only.** On any other boundary the period is 0.

**Default stays Pyramid**, so saved configs and baselines render as
before. *(Every preset was switched to the shifted grid here, then back:
see section 6c.)*

| gate | result |
|---|---|
| baselines at the default | 94/94 byte-identical; `sim-mccabe-shifted` makes 95 |
| CPU mirror, one step, offset (69, 49) | 4,096 of 4,096 cells exact; the unshifted mirror differs on 1,999 |
| batch invariance, 300 steps | bit-identical, and different from the fixed lattice's run |
| `every_preset_draws_something` | passes with every preset shifted |

**The lean, on the GPU** (coarse-fastest table, 512², 32 seeds; motion
over steps 200–232, 8 seeds):

| lattice | axes / diagonals | paired gain | direction flips / step | mean change / step |
|---|---|---|---|---|
| fixed | 1.07 ± 0.02 | — | 0.32 | 0.0137 |
| moved every step | 0.93 ± 0.02 | 0.14 ± 0.03 | 0.37 | 0.0165 |
| **every 4 steps (shipped)** | **0.93 ± 0.02** | **0.14 ± 0.03** | **0.26** | **0.0157** |
| every 16 steps | 0.97 ± 0.02 | 0.10 ± 0.03 | 0.30 | 0.0150 |

Every 4 removes the lean as fully as every step, and its cells reverse
direction less often than the fixed lattice's. A preset runs four or
more steps a frame, so a moving lattice moves about once a frame.

**Seen** (512², `output/mccabe_shift/sheet.png`):
- The default ladder is unchanged to the eye.
- The coarse ladder's nested contours are crisper shifted. The pinning
  was costing it structure as well as direction.
- The coarse-fastest table's squarish blobs become a round labyrinth.

## 6b. Plan: exact disc averages by FFT (2026-10-04)

`averaging` = 2, "Exact discs". It is the references' own method:
Reusser and Chau convolve by FFT. It fixes the disc-versus-Gaussian
texture, and with no lattice it has no lean.

**What is computed.** McCabe needs `a − b` per scale, never `a` and `b`
apart: the variation is `|w(a − b)|`, the direction its sign. So scale
`i` needs one field,

```
D_i = IFFT( FFT(f) · H_i ),   H_i = FFT(disc(r_a) − disc(r_b))
```

with antialiased discs (one-cell edge, as Reusser), each normalised to
sum 1, centred on cell (0, 0) and wrapped. That kernel is symmetric, so
`H_i` is real. Two scales' outputs are real, so they share one complex
inverse: `IFFT(F·H_i + i·F·H_j) = D_i + i·D_j`. Six scales cost one
forward and three inverse 2D FFTs a step. Symmetry reads `D_i` at the
rotated positions, bilinearly. The pyramid is not built.

**The FFT.**
- **Algorithm.** Mixed-radix Stockham, out of place, one dispatch per
  radix stage per axis, on `vec2<f32>` storage buffers.
- **Radices.** 4, 2, 3, 5, and a general pass for any prime up to 64
  (in-app grids follow the viewport, so 1920 × 1080 is normal).
- **Fallback.** A grid with a larger prime factor falls back to the
  pyramid, and the panel says so.
- **Kernel spectra.** Made on the GPU with the same FFT, from the
  spatial kernels, whenever the grid or a radius changes.
- **Where the fields live.** The `D` fields go in an `Rgba32Float`
  texture array, four scales a slice: a sampled texture adds nothing to
  the step layout's storage-buffer count, which browsers limit.

**Cost (estimated).**
- Memory at 1080p: about 150 MB (spectrum, scratch, `H`, `D`).
- Time: four 2D FFTs a step, about 50 dispatches. It is memory-bound;
  the estimate is near the pyramid's 4 ms at 1080p, and it will be
  measured.

**Periodic only.** A circular convolution is exactly the periodic
boundary. Any other boundary keeps the pyramid.

**Order.**
1. The FFT, against a CPU FFT (rustfft) at power-of-two, mixed and
   prime sizes, both directions.
2. The spectral stage, against direct disc sums on a small grid.
3. McCabe reading it: a CPU mirror of a step with exact discs, batch
   invariance, and the lean, look and cost measured against the shifted
   pyramid.
4. Presets and docs. The default stays Pyramid.

### 6c. Built: exact discs, and what they showed about the shifted grid

`averaging` = 2, "Exact discs", as planned in 6b.
- **The FFT** is `src/sim/fft.rs`: Stockham, radices 2 to 5 written out
  and any other prime up to 64 in a separate entry point (its array's
  registers would otherwise be every stage's). Column passes give
  neighbouring invocations neighbouring columns, so reads coalesce.
- **The spectral stage** is `src/sim/spectral.rs`. The renderer owns
  one `R32Float` array of difference fields, six slices per exact
  layer, at step binding 18; `xform.w` carries a layer's first slice.
- **Fallback.** A non-periodic boundary, or a grid with a prime factor
  above 64, falls back to the pyramid, bit for bit.

| gate | result |
|---|---|
| the FFT against rustfft, 8×8 to 1920×1080, prime and mixed sizes, both directions | worst 7.7e-7 of the largest output |
| difference fields against a direct convolution, 40×36, a pair and a single | worst 4e-7 (relative ~1e-6) |
| one exact step against a CPU mirror, plain and 3-fold symmetric | 2,304 of 2,304 cells exact, no ties |
| batch invariance, 150 steps | bit-identical |
| fallback on a 67×64 grid | bit-identical to the pyramid |
| baselines at the default | 95/95 byte-identical; `sim-mccabe-exact` makes 96 |

**One bug on the way.** The symmetric mirror first failed on 37 edge
cells. The rotated reads near a seam used `((q % g) + g) % g`, which on
this driver reads out of bounds at the top and left edges, exactly as
`sim_wrap_sized`'s comment records from SmoothLife. It now goes
through `sim_wrap_sized`.

**Cost at 1080p**, five scales, best of five:
- pyramid 4.40 ms/step;
- shifted grid 4.47;
- exact discs 7.53 (1.7×).

Exact discs were 125 ms before the column passes coalesced and the
general radix moved to its own entry point. Memory while on, at a 1080p
grid: about 150 MB (spectra, difference fields, complex scratch).

**The lean, all three on the GPU** (axes / diagonals, 1.0 isotropic):

| table | seeds | pyramid | shifted grid | exact discs |
|---|---|---|---|---|
| coarse fastest | 32 | 1.08 ± 0.02 | 0.92 ± 0.02 | **0.97 ± 0.03** |
| fine fastest | 16 | 0.95 ± 0.02 | 0.88 ± 0.01 | **1.00 ± 0.02** |

**Exact discs are isotropic**, matching the CPU references (disc 0.99,
Gaussian 0.96). **The shifted grid is not a fix.** It moves the lean
rather than removing it:
- on the coarse-fastest table, +0.11 from exact becomes −0.05;
- on the fine-fastest table, −0.05 becomes −0.12.

Every preset is a fine-fastest table, so the presets go back to the
fixed pyramid they were tuned and inspected on.
- **Shift plus a round kernel was tried too.** The 7-tap build kernel
  of section 6, with the shift, gives 0.88 and 0.86, worse still. The
  diagonal lean is not the kernel's.
- **The likely reading, not proven.** Over random offsets, the bilinear
  reads average into a square (separable) blur, which attenuates
  diagonal wavelengths less.

**Seen** (`output/mccabe_modes/`). Exact discs give a different
character from any pyramid mode:
- the coarse ladder at 2,000 steps is distinct cells with nested
  detail, where the pyramid draws smooth contour bands;
- fig. 14's mixed symmetry comes out as cleaner concentric rings.

The cells are the look the paper's multi-scale figures describe.

## 7. Not in scope

- **Exact disc kernels**, which would need FFT. That is parked with
  its trigger in simulation-fractals.md.
- **Per-scale colours other than palette bands.** They are a colouring
  parameter for later.
- **Re-measuring the pyramid's 0.55 calibration**, which predates the
  sampling-phase fix (catalogue §10). It would be measured and
  reported only: a changed constant moves every McCabe picture, and
  that is the user's call.

## 8. Experiments: what else is worth building (2026-10-04)

Asked: is P3 worth it, what does P4 do, what colourings (relief?), can
the lean be made on purpose per scale, and can per-scale maps (warps,
flame affines) make interesting patterns? **Only clear wins get built.**

**The script.**
[proto_mccabe_variants.py](../../scripts/sim_prototypes/proto_mccabe_variants.py)
runs every idea on the CPU from the same seed:
- exact-disc averages by FFT, 384², 300 steps;
- the ladder radii 2–32, colour memory 0.17.

The sheets are in `output/sim_proto/variants/`.

| idea | seen | cost to build | verdict |
|---|---|---|---|
| **P3**, variation radius 1 and 3 | smoother, larger scale domains; less speckle in which scale wins | a second pass and scratch slices (pyramid), or five more inverse FFTs a step (exact) | **not worth it**: a modest change at a real cost |
| **P4**, sum of each scale's signed step | white and black stripes with dots inside: the paper's figs. 4–6 | a rule switch in the step, either averaging | borderline |
| P4, same with alternating negative steps | dense fingerprint stripes, bent by large-scale flow | as above | the best P4 picture; overlaps Swift–Hohenberg |
| P4, sign of the weighted sum | coarse binary blobs; with ± weights a fine maze | as above | weaker |
| **lean on purpose**, all scales 2:1 at 0° | horizontal grain, streaked | the spectral stage's disc fill gets an aspect and an angle per scale; no cost per step | **clear win** |
| lean, 2:1 at 0/90° alternating | flowing fingerprint bands with fine detail inside | as above | **clear win** |
| lean, 3:1 on the coarse scales only, at 0/60/120° | bold zebra or wood-grain stripes, still multi-scale inside | as above | **clear win** |
| lean, 2:1, 36° apart | diagonal woven flow | as above | good |
| drift, activator offset from inhibitor | no difference in a still; the pattern travels | asymmetric kernels have complex spectra | animation only; not now |
| per-scale rotation ±3°, zoom on coarse scales | subtle | a map per scale in the step | **not worth it** |
| per-scale swirl | a strong vortex | as above | striking, but a whole-field swirl warp already exists |
| **relief**, the field as a height, raw | harsh: the finest scale's speckle dominates | — | — |
| relief, height smoothed over radius 2 | embossed cells with nested texture: the paper's raised and recessed look | a colouring for any model, in the colour stack under Multiply | **clear win** |
| Chau's YUV, luminance from the field, chroma from the memory | vivid, every scale's colour visible at full brightness | an option on Scale Memory | taste, not a clear win |

**Why "lean on purpose" is exact-discs only.** An ellipse is centrally
symmetric, so its spectrum stays real and the spectral stage's pairing
holds: the only change is the fill. The pyramid's reads are round by
construction. An elliptical read would cost a line of taps per average,
and section 6 showed how sensitive the pyramid's shape is.

**Relief** fits as a simulation colouring for any model: a channel as
height, smoothed, lit, and blended over Scale Memory or anything else.
No effect in the chain does it today (the sixteen effects include Sobel
edges, not shading).

## 9. Plan: P3, lean on purpose, Chau's colour (2026-10-04)

Decided after section 8: build P3, the elliptical kernels and Chau's
YUV. P4 waits. Per-scale warps and relief as an effect are open
questions, answered alongside, not built yet.

**Lean on purpose.**
- **Parameters.** `s{i}_stretch` (1–4) and `s{i}_angle` (0–180°), two
  more table columns, appended as parameters after `averaging` so no
  existing slot moves.
- **Exact discs only.** The spectral stage's fill draws each scale's two
  discs as ellipses of the same area, along the angle; an ellipse is
  centrally symmetric, so its spectrum is still real. The panel shows
  the columns only when the averaging is Exact discs.
- **Gates.** Defaults byte-identical; the spectral stage against a
  direct convolution with an elliptical scale.
- **As built.**
  - The stage's uniform carries the fill's 2×2 metric, computed on the
    CPU; a round disc's is exactly the identity, so the 97 sim baselines
    are untouched.
  - The angle runs counter-clockwise on screen, as the shading light's
    does; the grid's y runs down, so it is negated on the way in.
  - Tests: the spectral stage against a direct convolution with two
    ellipses (worst 2e-7); the one-step CPU mirror with a leaning
    table, which fails 1514 of 2304 cells if the stretch is ignored;
    and a `sim-mccabe-lean` baseline.
  - Demo: [make_mccabe_lean_demo.py](../../scripts/sim_prototypes/make_mccabe_lean_demo.py)
    writes five leans into `output/mccabe-lean/`.

**Chau's YUV.** Scale Memory gets a brightness mode, Multiply (today)
or Luminance (YUV):
- mix the bands as now;
- convert to YUV;
- set Y to a mix of the colour's own and the field's, by the existing
  brightness slider;
- convert back.

The default is Multiply, so it is byte-identical.

- **As built.** `brightness` (Multiply, Luminance (YUV)) on
  `scale_memory`.
  - **No colour-space round trip.** Replacing Y while U and V stay is
    one shift added to all three channels: Y's weights sum to 1, and U
    and V depend only on B − Y and R − Y. The same holds for YCbCr, the
    other space Chau names. The result is then clipped to [0, 1].
  - **Y is the field itself.** At brightness range 1, Y is
    `(f + 1) / 2`: Chau "maps the concentration value directly to the
    luminance". His post has no code. The prototype's 0.05–0.95 range
    was ours, so it was not carried over.
  - **The test.** Cell by cell, at Nearest on the grid, against the same
    run's memory colour and field, on a coloured palette: exact.
    `sim-mccabe-yuv` is the baseline.
  - **Seen.** Pale and luminous where Multiply is dark: every scale's
    hue survives into the bright regions
    (`output/mccabe-yuv/sheet.png`).

**P3, the variation radius**, `variation` (0–4 cells, 0 = off).
- **What it needs.** The argmin wants each scale's `|w(a − b)|`
  averaged over a disc around the cell, so it needs `a − b` at the
  neighbours. Recomputing that from the pyramid per neighbour would be
  about 9× the step.
- **A measure pass instead.** It writes each scale's symmetrised `a − b`
  into an `R32Float` array once a step:
  - pyramid mode reads the pyramid, as the step does today;
  - exact mode symmetrises the spectral stage's fields.
- **The step then gathers** an antialiased disc of `|S_i|` per scale for
  the variation, and takes the direction from the cell's own sign.
- **The cost.** The same reads as today, moved to the measure pass,
  plus `(2r+1)²` gathers per scale.
- **Memory.** One more six-slice array at 1080p, ~50 MB, while on.
- **Gates.** `variation` = 0 byte-identical; a CPU mirror in both
  averaging modes, with symmetry; batch invariance.
- **As built** (2026-10-05). The design changed from the plan above in
  where the measure goes and in how the step reads it.
  - **A measure pass, not a new array.** McCabe has two passes now.
    Pass 0, the measure, writes every scale's signed variation into two
    SCRATCH slices of the field, four scales to a texel. A new
    `ModelDef::measure` runs it only while `variation` > 0; off, the
    step is pass 1 alone, as before. The pass and slice machinery (the
    memory's) carries it, so no new bindings or layouts.
  - **The disc gather reads both slices a tap,** so six scales cost two
    `vec4` loads a tap rather than six.
  - **Disc, not Gaussian.** A separable Gaussian of the disc's variance
    would cost fewer taps at large radii.
    [proto_mccabe_variation_kernel.py](../../scripts/sim_prototypes/proto_mccabe_variation_kernel.py)
    ran both from one seed at radii 1, 2 and 4: the same look at each
    radius (`output/sim_proto/variation_kernel/sheet.png`). The disc
    was kept because it is the prototype's, needs no extra pass, and
    costs no more at radius 1–2.
  - **Memory.** Two `Rgba32Float` slices on both ping-pong sides,
    ~130 MB on a 1080p grid, against the plan's 50 MB for a
    single-buffered `R32Float` array. Turning it on or off keeps the
    run: scratch sits after every slice that persists, so the field is
    copied into the new arrays rather than reseeded.
  - **Cost at 1080p, five scales.**

    | variation | pyramid | exact discs |
    |---|---|---|
    | off | 4.36 ms | 7.42 ms |
    | 1 | 5.68 ms | 8.50 ms |
    | 2 | 7.18 ms | 9.99 ms |
    | 4 | 12.9 ms | 15.7 ms |

    Radius 4 is the disc's 69 taps. A separable Gaussian (an extra pass,
    the variation's sign carried in the blurred value's sign bit) would
    take it to roughly 9 ms. Not built.
  - **Byte identity cost a duplicate.** The step's loop first went
    through the measure pass's function. The arithmetic was the same,
    but the different shape let the driver fuse a weighted table's
    `w·a − w·b` into one multiply-add. The last bit moved, and
    `sim-mccabe-table` (a weight of 1.5) diverged, mean error 6.
    Returning the pair `(w·a, w·b)` was not enough either. So the
    no-variation path keeps the old loop verbatim, and the measure has
    its own copy of the arithmetic.
  - **Tests.**
    - Both CPU mirrors (pyramid at radius 2, and 1.5 with 3-fold
      symmetry; exact at radius 2, and 1 with 3-fold) match on every
      cell. The radius changes the winner at 25–30% of cells, so the
      mirrors are not passing trivially.
    - Batch invariance with memory and a warp.
    - Turning the radius on mid-run continues the run: step 41, one
      step's move, the memory written.
    - A `sim-mccabe-variation` baseline.
    - Two of the new tests first passed trivially: at 48² the default
      five scales reach past the pyramid, so the coarsest wins every
      cell and the run freezes. They run three scales and assert
      that more than one scale wins.
  - **Seen** (`output/mccabe-variation/sheet.png`). Larger, smoother
    regions per scale as the radius grows, in both averaging modes.

## 10. TODO, after P3 (2026-10-05)

Agreed, not planned in detail yet. P3 comes first.

**Per-scale warp.**
- **The UI.** A select list in the Warp section, "Field / Scale 1–6":
  the same zoom, rotation, pan and swirl controls, editing the chosen
  target.
- **The mechanism is not the field warp's.** The field warp resamples
  the whole field each step. A scale's warp moves where that scale
  reads its averages instead, so it compounds step by step like an
  advection of that scale alone. Only the controls are reused.
- **The cost.** Cheap in both averaging modes: a moved read. It needs
  about 30 more numbers in the step. Either the model's parameter
  block grows from 64 to 128, or the warp's own uniform carries them.
- **What to expect** (section 8). Per-scale rotation and zoom were
  subtle in stills; swirl was striking. Mostly a motion effect.

**Relief, in the simulation system, not as an Effect.**
- **Why not an Effect.** An effect sees only the finished image. Its
  height would be luminance after the palette, which bends the field's
  shape, and Scale Memory's hue edges would read as false ridges.
- **The precedent.** Escape-time relief is a pass inside the escape
  renderer that slopes the colouring's own scalar, not the image. Use
  its conventions, light angle counter-clockwise from east.
- **The shape.** A height stage turns a field channel, or age, into a
  smoothed height texture on the grid (section 8: raw is harsh, radius
  2 smoothing gave the paper's raised-and-recessed look). A shade step
  then lights it under any colouring.
- **Keep the height its own texture.** The long-term 3D height-field
  mode below wants exactly that texture as its displacement.
- **As built** (2026-10-05). A **Relief colouring** for the top of the
  colour stack, backed by a **relief stage** in the renderer.
  - **A colouring, not a config section.** Its parameters are colouring
    parameters: channel, softness, height, light angle, Tilt or Lambert,
    elevation, shadow, highlight. So it needed no new config paths, it
    animates like any colouring parameter, and stacks compose it with
    anything. The panel's **Add relief** puts one over the stack in a
    click.
  - **The relief stage.** Two separable passes before the colour pass,
    the Gaussian of the softness and its derivative, into a grid-sized
    `Rgba32Float` of (height, d/dx, d/dy). The derivative is the
    smoothed height's own: a derivative-of-Gaussian kernel, exact on a
    ramp, so no third pass differences the result. The colour pass
    reads it as `x.relief`, interpolated by the resolve like the state.
  - **Hard light, a new stack blend.** The relief's grey went over the
    stack under Overlay first. On a light palette Overlay's shadow
    barely lands, because below mid-grey it multiplies the BASE's
    darkness. Hard light multiplies by the top layer's instead. Over a
    grey centred on mid-grey it is exactly the escape relief's shading:
    a black multiply shadow and a white screen highlight, each by its
    strength. It is appended to the blend modes, so no existing stack
    changes.
  - **Defaults: Tilt, height 12, softness 2, shadow 0.8, highlight
    0.6.**
    - At height 6 (the prototype's), Tilt was too gentle on McCabe at
      softness 2.
    - Lambert at elevation 30 embosses harder, and is one click away.
    - Tilt is the escape relief's default, for its symmetric light and
      shade.
    - Sheets: `output/mccabe-relief/sheet.png` and
      `output/mccabe-relief-tune/sheet.png`.
  - **Cost at 1080p.** The colour pass is 0.6 ms alone, 2.4–2.8 with
    relief at softness 2 and 6.4 at softness 8.
    - The texture reads dominate.
    - Making the kernel once per workgroup in shared memory was tried
      and measured slower at softness 2 (3.3 ms): thread 0's serial
      normalisation cost more than the per-cell `exp()` it saved. It
      was reverted.
  - **Tests.**
    - The stage against a CPU Gaussian and its derivative at three
      softnesses, periodic, reading the third channel: worst 2e-7.
    - The colouring's light on ramps along x and y from each side, and
      flat ground at exactly mid-grey.
    - Hard light joins the blend-formula test.
    - The `sim-mccabe-relief` baseline.

**Long term: a 3D height-field mode for escape-time and simulations.**
- **The idea.** The user's plan, not designed yet. A 2D fractal (the
  Mandelbrot set, a simulation) is a flat plane in 3D space. Its relief
  and/or escape time (escape) or relief and/or age (simulations) give
  the depth. It is not full 3D fractals.
- **Reuse.** The 3D engine.
- **Rendering.** Path tracing, for quality ("same way 3D escape-time
  already works", in the user's words).
- **What it asks of work now.** Heights should be explicit scalar
  textures on the grid that a later renderer can sample, not values
  folded straight into a colour.


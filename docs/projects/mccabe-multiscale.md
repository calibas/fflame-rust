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

## 7. Not in scope

- **Exact disc kernels**, which would need FFT. That is parked with
  its trigger in simulation-fractals.md.
- **Per-scale colours other than palette bands.** They are a colouring
  parameter for later.
- **Re-measuring the pyramid's 0.55 calibration**, which predates the
  sampling-phase fix (catalogue §10). It would be measured and
  reported only: a changed constant moves every McCabe picture, and
  that is the user's call.

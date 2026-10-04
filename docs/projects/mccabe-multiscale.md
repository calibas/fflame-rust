# McCabe multi-scale: colour memory, a per-scale table, variation radius

Branch `mccabe-scales`, started 2026-10-04. The model is catalogue
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
| resize | resampled with the field |

- **The model side.** `ModelFeature::Memory`, and a slice count from
  the parameters: 2 when `memory` > 0, else 0. The step template
  splices `sim_mem_read(k)` (from `field_in`) and `sim_mem_write(k, v)`
  (to `field_out`) at `params.mem_base + k`. McCabe's step updates the
  weights after choosing the winner.
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
- `memory` = 0: every baseline is byte-identical, and so is the
  uncoupled step shader of every model.
- The CPU mirror covers m across 3 steps, to the field's tolerance.
- `steps_are_batch_invariant` passes with memory on.
- A one-cell integer pan of the warp moves m exactly with the field.
- A paused palette edit changes the picture.
- Toggling memory reseeds; changing b does not.
- A preset, run and inspected before it ships.

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

## 6. Not in scope

- **Exact disc kernels**, which would need FFT. That is parked with
  its trigger in simulation-fractals.md.
- **Per-scale colours other than palette bands.** They are a colouring
  parameter for later.
- **Re-measuring the pyramid's 0.55 calibration**, which predates the
  sampling-phase fix (catalogue §10). It would be measured and
  reported only: a changed constant moves every McCabe picture, and
  that is the user's call.

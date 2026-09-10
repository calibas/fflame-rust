# Simulation layers: N coupled systems, per-layer transforms, colouring layers

**Status: ARCHIVED 2026-09-09 — done.** All six phases built and
gated: 1–4 on 2026-09-08 (section 7), phase 6, the lattice as layers
(section 9), on 2026-09-09, and phase 5 — the panel split — as its own
project,
[simulation-panel.md](../archive/projects/simulation-panel.md).
Phases were ordered so each shipped with every existing baseline
byte-identical, and the decision points a reader should argue with are
marked **decision**.

**One thing noted here and not built:** a colouring that reads ACROSS
layers. The colour stack draws layers separately and blends; a single
colouring taking several layers as its inputs is a later extension
(section 7's colouring note). The `gather` mode added in phase 4 is
the narrow version of it — four consecutive layers' first channels as
one colouring's four channels.

## 0. What is being asked for

Three things, from the session that built the coupled Turing lattice
(catalog §28), the two-layer Brusselator (§29) and the layer-selective
warp (pipeline §4.1):

1. **N coupled systems**, each with its own rule and parameters, with
   the coupling between them controllable in form (linear,
   non-linear) and strength, per pair.
2. **Transforms per layer**, reusing the flame's transforms — the
   affine, the variations, and the Transforms and Triangle Editor
   panels that edit them — so a layer can be moved by any map the
   flame engine knows, at a rate, while other layers sit still.
3. **Colouring layers**: several colourings of the same state, each
   reading a chosen layer, stacked with blend modes and opacity, so
   "structure" or "distance" can be a layer above a base colouring
   rather than a replacement for it.

Plus, not in this plan: the Simulation panel split, which is UI work
that waits until the config has settled.

## 1. What exists and what it fixes

| today | limit | this plan |
|---|---|---|
| one `Rgba32Float` field: four scalar channels, or two two-species layers | N ≤ 4 fields, and every multi-layer rule is a hand-written model (`lattice4`, `brusselator2`) | a texture array with one slice per layer; every existing model is a layer rule unchanged |
| coupling is inside a model's WGSL | the form and the pairing are the model's | a coupling matrix the template applies after any rule, so any two models couple |
| the warp is one affine for the whole state, with a channel mask | rotation/zoom/pan/swirl only; per-channel, not per-layer | transform i moves layer i, and the map is the flame's: affine + variations, at a per-layer rate |
| one colouring, one matte | a colouring replaces the picture | a stack of colourings, each with its source layer, blend and opacity |

## 2. The state: a texture array

**Layer** = one slice of a `texture_2d_array<f32>` (`rgba32float`),
four channels, exactly today's field. N slices; N = 1 is today.

- Every read the templates make today, `sim_read(p)`, keeps its
  meaning: **the dispatch's own layer**, from `params.layer`. Models
  do not change. A new `sim_read_layer(l, p)` reads another layer
  through the same boundary rule; only the coupling code (§3) and the
  colouring layers (§5) use it.
- One dispatch **per layer per step**, in layer order, all reading the
  previous step's array (Jacobi across layers, as diffusion already is
  across cells). The params ring gains a layer index: slots per
  submit become `steps × layers`, so `MAX_STEPS_PER_SUBMIT` divides by
  N. The warp, pyramid, min/max, agents and JFA stages take the layer
  index the same way; pyramid and min/max are per layer and only
  built for layers whose rule declares them.
- The write side is `texture_storage_2d_array<rgba32float, write>`;
  the ping-pong pair becomes a pair of arrays. Resize and the octave
  doubling act on every slice.
- Memory: N × 2 × 16 bytes per cell — at 1080p, 66 MB per layer,
  eight layers half a gigabyte. Said in the panel; the grid is the
  user's.
- **Gate:** every sim baseline byte-identical with N = 1 (the array of
  one slice must read exactly as the texture did); the assembler's
  every-combination validation passes; `steps_are_batch_invariant`
  and the octave batch-invariance test pass with N = 3.

**Decision — one shader per layer or one mega-shader?** One compiled
step shader per *distinct* (rule, features) among the layers, dispatched
per layer with the layer index and that layer's parameter slots.
Layers sharing a rule share a shader. A single shader with a switch
over rules would compile every rule's WGSL into one kernel and pay
the registers of the largest for all; the per-layer dispatch keeps
today's cost per layer exactly and is what the ring already does per
step.

## 3. Layers and coupling in the config

```rust
pub struct SimLayer {
    pub name: String,              // the panel's label
    pub model: String,             // any registered model
    pub model_params: BTreeMap<String, f32>,
    pub enabled: bool,
}
pub struct SimCoupling {
    pub from: usize, pub to: usize,   // layer indices
    pub form: SimCouplingForm,        // Linear | Cubic | Quadratic | Product
    pub strength: f32,
    pub channels: u32,                // bit mask, which channels it drives; 15 = all
}
// SimConfig gains:
pub layers: Vec<SimLayer>,         // empty = today's single model (`model`/`model_params`)
pub couplings: Vec<SimCoupling>,
```

The coupling is applied by the **template**, after the rule, in the
layer's own step: `n = sim_step(s, p) + dt · Σ_k strength_k · form_k(s,
u_from)` over the couplings whose `to` is this layer, channel-wise, on
the channels in the mask. Forms, with `u` this layer's value and `v`
the other's, per channel:

| form | term | from |
|---|---|---|
| Linear | `v − u` | Yang et al. 2002 (inter-layer diffusion) |
| Cubic | `u·v·(v − u)` | Kyttä et al. 2007 (an active middle layer) |
| Quadratic | `v² − u²` | Barrio et al. 1999 (found weak there; offered because it is a form) |
| Product | `u·v` | a gate: one layer multiplies another's growth |

Channel-wise means species 1 couples to species 1: Gray–Scott's A to
a Brusselator's u. That is a choice the user makes by picking the
layers; a per-coupling channel *map* is a later extension if wanted.

Models keep one `dt` (the config's); the stability cap is the minimum
over the layers' rules at their parameters, which `max_dt_for`
already computes per model. The existing `brusselator2` and
`lattice4` stay as they are: they are the papers' exact systems, and
the first is this phase's gate.

- **Gate:** two `brusselator` layers under a cubic coupling of
  strength q reproduce the `brusselator2` model's field to a CPU-mirror
  tolerance at the same dt and seed for 1,000 steps (the coupling's
  arithmetic is the same; the seeds must be made to agree per layer,
  which the layer index in `sim_rand`'s salt gives). Every registered
  model runs as a layer (the every-preset probe over layers). A
  three-layer config is batch invariant.
- **Presets (measured before shipping, as always):** two Gray–Scotts
  at different feed/kill linearly coupled; a Gray–Scott driving a
  Brusselator through Product; the `brusselator2` boats as two layers.

## 4. Per-layer transforms: the flame's transforms

**Decision — reuse `flame.transforms` as the layer transforms, one to
one, in Simulation mode.** The Transforms and Triangle Editor panels
take a concrete `&mut Flame` and write `ConfigPath::Transform*`; in
Simulation mode the flame is not rendered, so its transform list is
free, and transform *i* is layer *i*'s map. The panels then need one
change: `panel_viewer.rs` stops hiding Transforms, Triangle Editor and
Variations in Simulation mode. The alternative — an abstract "transform
target" the panels edit — is a refactor of two of the largest panels
for the same result; rejected.

What a transform means for a layer:

| transform field | in Simulation mode |
|---|---|
| affine a–f, post affine, variations, variation params | **the map** `T` — the flame's own `apply_variations` on the affine's output |
| `weight` | **the rate** in [0, 1]: the layer's source coordinate each step is `p + rate · (T(p) − p)`; 0 leaves the layer still (the mask of pipeline §4.1 becomes "weight 0"), 1 applies the whole map per step |
| `color`, `color_speed`, `opacity`, xaos, linked/final pools, `g` | inert; the panel shows them, the docs say so |

The warp shader for a layer is the existing warp template plus the
flame builder's **definitions**: `shader_builder_v2::build_from_template`
is monolithic today, but everything before the main template is
definitions (header, rng, affine, variation bodies, `apply_variations`,
`get_param`), so `build_definitions(flame) -> String` is a split at
one line. The warp binds the flame's `transforms` and
`variation_params` buffers, packed by the same `pack_gpu_transforms` /
`pack_gpu_variation_params`, and reads `apply_variations(transforms[i],
i, affine(p), …)`. The map gives the *source* coordinate (a backward
resample), so a non-invertible variation folds the layer, which is
the point, and an area-changing one is a source or sink of the
layer's quantity, which the tooltip says.

The existing warp (zoom, rotation, pan, flow, octaves, filter, cull)
stays as the **global** warp on all layers; the per-layer transform
composes after it. The channel mask of pipeline §4.1 stays for the
global warp.

- **Gate:** a transform that is `linear` alone with an affine of pure
  rotation, at weight 1, equals the global warp's rotation of that
  layer to the bilinear mirror's tolerance (1.3e-5). Every registered
  variation validates in the warp shader (the assembler's
  combination test grows a variation axis: one flame per variation).
  The per-step blur finding still holds — a rate under 1 is a
  sub-cell resample every step — and the tooltip carries it.

## 5. Colouring layers

```rust
pub struct SimColorLayer {
    pub source: usize,                 // which sim layer it reads
    pub coloring: String,
    pub coloring_params: BTreeMap<String, f32>,
    pub matte: SimMatte,
    pub blend: SimBlend,               // Normal | Lighten | Darken | Multiply | Screen | Add | Overlay
    pub opacity: f32,
    pub enabled: bool,
}
// SimConfig gains:
pub color_layers: Vec<SimColorLayer>, // empty = today's `coloring`/`coloring_params`/`matte` on layer 0
```

Bottom to top. The colour template splices K colourings, each
renamed (`sim_color_k`, `cparam_k` at its own 16-slot offset in the
colouring-params buffer, its matte packed in a per-layer slot), and
the resolve composites `col = blend_k(col, shade_k(x_k))` in order,
where `x_k` is a `SimSample` read from layer `source_k` — so the
derived quantities (gradient, structure, distance) come per source
layer, and the interpolation invariant of the derived-fields plan
holds per layer. The blend formulas are the standard separable ones
on straight RGB with the layer's coverage × opacity as its alpha;
the bottom layer's coverage is the output's coverage. The distance
field (JFA) runs once per colouring layer that needs it, on its
source layer.

- **Gate:** one colouring layer, Normal, opacity 1, on layer 0 is
  byte-identical to today's output for every sim baseline; each blend
  mode matches a CPU evaluation of its formula on two read-back layers.

## 6. Panel

Not this plan, but the shape it should take so the config above is
editable: a **Layers** list (add / remove / reorder; per layer: model,
its parameters, enabled), a **Couplings** list (from → to, form,
strength, channels), and a **Colouring** list (the color layers). The
Simulation panel keeps grid, seed, init, steps, dt, boundary, warp,
fit. The Transforms and Triangle Editor panels are the per-layer
transform UI by construction (§4).

## 7. Order and gates

| phase | builds | gate |
|---|---|---|
| 1 | texture-array state, per-layer dispatch, `params.layer`, N = 1 everywhere | every baseline byte-identical; batch invariance at N = 3 — **built**: 87/87 sim baselines byte-identical; three mixed layers (Gray–Scott, Brusselator, lattice) batch invariant on every slice; and a stronger gate than planned, layer 0 of a two-layer config beside a two-pass model is bit-identical to the single-layer run (`a_layer_is_the_same_run_it_would_be_alone`). `SimLayer { model, model_params, enabled }` and `SimConfig::layers` landed here rather than in phase 2, since the per-layer dispatch needs each layer's model; couplings and the panel are phase 2. Cost probe (`layered_step_cost_at_1080p`): Gray–Scott at 1080p 0.290 ms/step alone, 0.284 per layer at two, 0.299 at four, 0.319 at eight — per-layer cost is flat; memory 63 MB per layer as predicted. One agent layer per config (one population, one deposit buffer); a layer is carried through a stage it has no pass for by the warp with an all-zero mask, exact. |
| 2 | `layers` + `couplings` in the config, template coupling, panel lists | `brusselator2` reproduced from two layers; every model runs as a layer — **built**: two `brusselator` layers on the 5-point stencil (a `stencil` choice added to that model for the purpose, default unchanged) under a cubic coupling of 0.15 match the `brusselator2` model from an identical analytic seed after 1,000 steps to 4e-6 RMS (`two_brusselator_layers_are_the_two_layer_brusselator`); every model's step shader validates with the coupling spliced in and its uncoupled shader is byte-for-byte unchanged; 87/87 baselines identical. The coupling table is a storage buffer the step shader loops over (entries aimed at its layer only), spliced into a model's LAST pass, added after the rule's own clamp — which differs from `brusselator2`'s in-sum coupling only where the clamp acts, and at a = 3, b = 9 it never does. Panel: Layers (model, on/off, parameters per layer; add / remove; "Add layer" on a single system splits it into layer 0 plus one more) and Couplings (from → to, form, strength, channels) lists, structural edits as full-config snapshots so they undo as one step; a layered-presets combo. Presets measured and shipped: `two_gray_scotts` (coral and maze, linear 0.02 — at 0.1 the labyrinth turns fine, at 0.3 both die) and `brusselator_layers` (the paper's spots with inner structure at 0.09). Not shipped, and the reason recorded: a Gray–Scott gated by a Brusselator through a Product coupling — Gray–Scott wants dt 1 and the Brusselator 0.01, one dt serves every layer, and at 0.01 the Gray–Scott layer barely moves in 6,000 steps. Risk 3 of section 8, met on the first try. |
| 3 | `build_definitions`, flame buffers bound to the warp, weight = rate, panels un-gated in Simulation mode | rotation-only transform equals the global warp; every variation validates — **built**: `ShaderBuilder::build_definitions` is the old builder's first sixteen steps, split at one line (the canonical shader dumps are unchanged), and `build_layer_map` adds `flame_map(xform_id, u, seed)` — affine, variations, post affine — with the flame's bindings moved to group 1 and its `params` renamed `flame_params`; `assemble_layer_warp` puts that in the warp template with the simulation's own `ff_atan2` dropped (the flame's utilities define one). The renderer takes the flame through `set_layer_transforms` from the app, the headless renderer and the animation export; the map's definitions rebuild only when the active variation set, transform count or flags change, and the buffers (the normal transforms only) rewrite each call. The stage runs after the global warp: a layer at rate 0 is carried across, one flip. Gates: a rotation-only transform at weight 1 equals the global warp's rotation to 1.7e-6 worst-case; all 647 registered variations validate in the layer warp; 89/89 baselines byte-identical. **One decision changed:** the map is behind `SimConfig::use_transforms` (off by default, a checkbox in the Layers section) rather than always on — every config carries a flame, and a default flame's transform happens to be the identity only by luck (0.5 scale × `linear` 2). Seen: layer 0 of two Gray–Scotts under `swirl` at rate 0.03 dragged into a spiral, its contrast softened by the per-step resample as section 8 said; a 0.995 affine at rate 1 is the inflation through the flame's own affine. The grid's short axis spans [−1, 1], a flame's default view. |
| 4 | `color_layers`, K colourings spliced, blend modes | single Normal layer byte-identical; blend modes vs CPU — **built**: `assemble_color_stack` splices K colourings with every function they define suffixed `_k` (so one colouring can appear twice), a `cparam_k` per layer at its own 16-slot block, a shade and a resolve per layer, and composites bottom to top through `sim_blend` (separable formulas on straight RGB, coverage × opacity as alpha, coverage accumulating as "over"; a bottom layer over nothing is itself exactly, which is what makes a one-layer stack the single colouring bit for bit). Each layer's record — source, blend, opacity, matte — is a storage buffer at binding 7; `sim_layer()` gained a private offset the stack sets per layer so `sim_sample` reads the layer's source; `sim_sample`'s gradient and tensor splices are the union over the stack. The single-colouring template is untouched. Gates: a one-layer Normal stack is byte-identical to the single colouring on a channel colouring, a matted growth model and a distance-reading colouring; all seven blend modes match a CPU evaluation to 2e-41 (denormal) worst-case; every colouring validates stacked with itself and all together; 89/89 baselines byte-identical and one new stacked baseline. **Two decisions made here:** per-field paths only for the animatable fields (a layer's parameters, opacity, matte cutoff and softness); source, colouring, blend, enabled, the matte's channel / invert / edge, and add / remove / reorder are snapshot edits, which undo as one step and do not restart the run (the app restarts only on `SimRestart` paths). And one distance field per frame, the first layer whose matte is on with a Distance edge or whose colouring reads the distance; the other layers read it as it is, said on the tooltip. |
| 5 | Simulation panel split | **built** — the ask grew past a split into merging Model with Layers and Colouring with the colour stack, so it had its own plan of record: [simulation-panel.md](../archive/projects/simulation-panel.md), now archived. Five phases; entry 0 of each list is the flat field until a second entry is added |
| 6 | `turing` layer model, `Signal` coupling, the pre-rule drive (section 9) | four `turing` layers under Signal couplings reproduce `lattice4`'s ring and independent presets; every model validates with Signal spliced; every baseline byte-identical — **built**: ring 5.7e-7 worst after 200 steps from the lattice's seed, independent 1.2e-7, the `species` colouring of a gathered colour layer 6.3e-7 (catalog §31a); 92/92 sim baselines byte-identical, two new. **Two things changed from the plan:** the seed is copied in for the gate rather than drawn alike, because the init mask's noise is salted by layer (the step's fluctuations do match by construction); and `turing` runs two passes, the first publishing its signal in `.y` for the couplings to read (`PublishesSignal`), after the single-pass form measured 5.5x the lattice's time — 2.0x now. **Added beyond the plan:** a colour layer's `gather` flag, the cross-layer colouring the plan deferred, small enough to take: four layers' first channels as one colouring's four channels. |

Phase 1 is the one with no visible feature and the most files; it is
first because every later phase reads the array.

## 9. Phase 6: the lattice as layers

**Why.** `lattice4` is four Turing fields on one grid, each with its
own activator-minus-inhibitor signal, driven through a 4×4 matrix and
then gain, the even term, the soft saturation and the decay. That is
a layer model plus couplings — except that today's couplings enter
*after* the rule, on values, times dt (section 3), and the lattice's
matrix enters *inside* the rule, on signals, before the saturation.
McCabe's multi-scale model, by contrast, does not decompose: it is
one field whose scales are competing measurements chosen by a
per-cell argmin, and nothing pairwise expresses that. This phase
takes the lattice apart and leaves McCabe as it is.

**What is built.**

- **`turing`**, a single-field layer model: `radius`, `ratio`,
  `amount`, `noise`, `gain`, `decay`, `quadratic`, and `self` (how
  much the field follows its own signal; `kaa` of the lattice). Its
  rule is one field of `lattice4`'s, with the drive `self · t_own +
  sim_drive(p).x`. No memory column: the lattice's memory reads all
  three other fields at once (their amplitude), which is not a
  pairwise term; it stays in `lattice4`.
- **`Signal`**, a fifth coupling form: the driving layer's field
  convolved with *the driving layer's own kernel table* — a two-block
  table as the difference of its blocks (activator minus inhibitor),
  a one-block table as itself. The coupling record carries the
  driver's table offset, radius and length (three words that were
  padding), so any model's step can read any layer's signal. A model
  without a kernel binds the table too when it is coupled, so a
  Signal coupling aimed at, say, a Gray–Scott is legal: it adds the
  signal after the rule like any other form.
- **`sim_drive(p)`**, the pre-rule hook: a model that declares
  `ModelFeature::TakesDrive` gets a function returning the sum of the
  Signal couplings aimed at its layer (strength × signal, masked), no
  dt; those couplings are then left out of the post-rule sum. For a
  model without the feature the uncoupled shader is byte-for-byte
  what it was; for `turing` uncoupled, the hook returns zero.
- **Noise.** The lattice draws field A's fluctuations with salt 0x41,
  B's with 0x42 and so on, all on layer 0's stream; `turing` draws
  its own with salt `0x41 + layer` on an *unsalted* stream (and seeds
  with `0x51 + layer`), so four `turing` layers draw exactly what the
  lattice's four fields drew. That is what makes the gate a
  trajectory comparison with the presets' noise on, not a noise-free
  special case.
- **Colouring.** The lattice's `species` colouring reads four
  channels of one layer; four layers are coloured by a stack of four
  `channel` layers. A colouring that reads across layers is a later
  extension, noted, not built here.
- **Presets:** the ring as four `turing` layers with its eight
  couplings (the gate), and one that the lattice cannot do — fields
  at different radii — measured before it ships.

**Decision:** the drive is a sum of Signal couplings only. Linear,
cubic, quadratic and product stay post-rule for every model; a
pre-rule value coupling has no source in the papers this catalogue
follows and is not offered.

**Gate.** `lattice4`'s `ring` preset against four `turing` layers
with the same parameters and its eight non-zero off-diagonal
couplings as Signal at those strengths, 64², 200 steps, noise on:
RMS and worst-case difference measured and asserted at what a change
of summation order costs (the lattice's `dot` against the layers'
running sum — the two cannot be bit-identical, and the tolerance says
so). The `independent` preset the same way with no couplings. Every
model's step validates with the Signal form spliced, including the
models without a kernel; every uncoupled shader unchanged; every
baseline byte-identical.

**What it buys.** Any number of fields instead of four; a radius per
field, so the fields can be multi-scale; a transform per field
through the layer map; and each field coloured on its own in the
stack.

## 8. Risks, stated

- **The panels in Simulation mode show flame semantics.** Weight is a
  rate, colour and xaos do nothing. The plan accepts that for the
  reuse; a later pass can hide the inert controls by render mode the
  way the panels already branch on 2D/3D.
- **Coupling channel-wise between unlike models** is only meaningful
  when the user knows the channels; the layer list shows each model's
  channel names next to the coupling mask.
- **One dt for all layers**: a stiff layer (the Rössler lattice at
  0.002) sets everyone's step. Stated in the panel as "dt capped by
  layer N".
- **Memory** at N × 2 × 16 B/cell, and the params ring dividing the
  submit batch by N: both measured and printed in the phase-1 cost
  probe before phase 2 starts.
- **Variations as warps blur** exactly as the affine warp does (the
  f(1−f) finding); rates near 1 with an octave-style discrete
  application is not available for a non-affine map, and the plan
  does not pretend otherwise.

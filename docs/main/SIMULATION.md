# Simulation Rendering

**Overview:** The third render mode. A **grid of cells** stepped by a
neighbour-coupled rule — reaction–diffusion, cellular automata,
aggregation, growth — then coloured and resolved to the output. Neither
a chaos game nor an escape-time iteration: the picture is the *state of
a run at a step count*.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) — overall system design
- [RENDERER.md](RENDERER.md) — the flame pipeline this shares a tail with
- [UI.md](UI.md) — the panel, and how the animation timeline drives the grid
- [EXPORT.md](EXPORT.md) — stills and video
- [CONFIG.md](CONFIG.md) — `ConfigManager`, undo, and where `SimConfig` sits
- [TERRAIN.md](TERRAIN.md) — the 3D terrain view a simulation shares
  with escape-time pictures
- [CAMERA.md](CAMERA.md) — the 2D view and the terrain camera's gestures
- [ESCAPE.md](ESCAPE.md) — the escape-time pictures that use simulation
  textures
- [SCRIPTING.md](SCRIPTING.md) — the `sim` script surface
- [SIMULATION-CATALOG.md](SIMULATION-CATALOG.md) — every model,
  its governing rule, its source, and what bit it. **Read this before
  touching a model.**

**Design record** (archived plans; the code is the authority where they
differ):
- [simulation-fractals.md](../archive/projects/simulation-fractals.md) — the
  master plan and its decision record
- [simulation-pipeline.md](../archive/projects/simulation-pipeline.md),
  [simulation-integration.md](../archive/projects/simulation-integration.md),
  [simulation-panel.md](../archive/projects/simulation-panel.md)
- [simulation-layers.md](../archive/projects/simulation-layers.md) — layers,
  couplings, the colour stack, `use_transforms`
- [simulation-derived-fields.md](../archive/projects/simulation-derived-fields.md)
  — the resolve filters, the matte's distance field, gradients
- [mccabe-multiscale.md](../archive/projects/mccabe-multiscale.md) — the
  pyramid, memory, measure passes, exact discs, relief
- [sim-textures.md](../archive/projects/sim-textures.md) — simulation
  textures
- [video-loop-and-sim-timeline.md](../archive/projects/video-loop-and-sim-timeline.md)
  — the step count as an animation target

**Code locations:**
- [src/sim/mod.rs](../../src/sim/mod.rs) — `ModelDef`, `SimColoringDef`,
  `SimPreset`, `SimLayeredPreset`, the `MODELS`, `COLORINGS` and
  `LAYERED_PRESETS` registries, the pure rules
- [src/sim/models.rs](../../src/sim/models.rs) — 31 models, inline WGSL
- [src/sim/colorings.rs](../../src/sim/colorings.rs) — 13 colourings
- [src/sim/assembler.rs](../../src/sim/assembler.rs) — WGSL assembly
- [src/sim/renderer.rs](../../src/sim/renderer.rs) — `SimRenderer`
- [src/sim/terrain.rs](../../src/sim/terrain.rs) — the simulation terrain
- [src/config/sim.rs](../../src/config/sim.rs) — `SimConfig` and its types
- [src/ui/sim_panel.rs](../../src/ui/sim_panel.rs) — the panel
- [src/textures/](../../src/textures/mod.rs) and
  [src/ui/textures_panel.rs](../../src/ui/textures_panel.rs) — textures

---

## What a simulation is here

A **cell** is a `vec4<f32>`. What the four channels mean is the model's
business: Gray–Scott's A and B, the snowfake's a/b/c/d, the dielectric
breakdown model's key/φ/age/occupancy. A **step** reads a cell and its
neighbours and writes the cell's next value. A **run** is a number of
steps from a seeded initial field.

Three consequences shape everything below.

**The grid is the information, not the resolution.** Doubling the grid
does not give a bigger picture of the same pattern; it gives *more*
pattern at the same feature size, because the cell is the unit the rule
works in. This is why the grid is a config field and not a render
setting, and why a fixed grid resolves to any output size rather than
re-simulating.

**A still is `(model, parameters, seed, init, steps)`.** All five are in
the config and in the PNG metadata, so a still is reproducible. There is
no notion of "converged": a model that never settles is as valid as one
that does, and the step count is the contract.

**The state is not invertible.** Nothing can run a step backwards. Every
"go to step N" where N is behind the current index means reseeding and
re-running, which is why the timeline behaves as it does (see
[UI.md](UI.md), *The timeline and the simulation grid*).

---

## The registries

Two append-only registries, in the same spirit as the variation and
escape-formula ones: a `static` definition with inline WGSL, registered
by appending to a list.

**`ModelDef`** ([src/sim/mod.rs](../../src/sim/mod.rs)) carries `name`,
`display_name`, `description`, `parameters`, `presets`, the `wgsl`
body, and:

| field | means |
| --- | --- |
| `passes` | dispatches per step, 1–4. A fourth-order PDE needs two; the breakdown model needs three (grow, relax, weigh). |
| `repeat` | `(pass index, parameter name)` — that pass runs a slider-controlled number of times. A relaxation sweep count cannot be compiled in. |
| `measure` | a `MeasurePass { param, slices }`: pass 0 writes `slices` scratch slices for the later passes to read at any cell, and runs only while `param` is above 0. Off, the step is the passes after it, and costs what it did before (McCabe's variation radius). |
| `max_dt` | stability bound for the explicit solver at the DEFAULT diffusion rates. Exceed it and the field diverges. The cap actually enforced (config writes, the panel's slider, the renderer's uniform) is `max_dt_for`, which takes the current parameters. |
| `diffusion` | which parameters are diffusion rates, for the dt ceiling. |
| `dt_bound` | a bound the model derives itself, overriding the diffusion one (the fourth-order PDEs). |
| `agents` | an `AgentDef` for the models with a moving population (physarum). |
| `state` | a `StateKind`: how a live field is carried into a grid of another size when a bound grid's scale changes (the field resampler). `Continuous` is interpolated (Catmull-Rom, held to each neighbourhood's range); `Discrete`, for any model with a discrete channel (states, spins, grains, occupancy), takes the nearest cell, which is exact; `Agents` is not carried and reseeds. A test holds `Agents` to the models with agents. |
| `kernel` | a function building a `SimKernel` lookup table, for the large-radius convolution models (Lenia, SmoothLife, McCabe). |
| `wgsl_seed` | `fn sim_seed(inside, noise, p)`: the initial state of a cell, given the init shape's mask. |
| `default_steps`, `default_dt` | what selecting the model sets `steps` and `dt` to (its parameters are cleared to their defaults too). |

**`ModelFeature`** is how a model opts into machinery it needs, so
nothing pays for what it does not use:

`NeedsRng`, `NeverStills`, `NoTimeStep`, `NeedsPyramid`, `NeedsAgents`,
`NeedsMinMax`, `TakesDrive`, `PublishesSignal`, `Memory`.

**`SimColoringDef`** is the same shape for the picture side, with
`ColoringFeature`: `NeedsGradient`, `NeedsStructure`, `NeedsDistance`,
`ReadsCell`, `ReadsMemory`, `NeedsRelief`. A colouring is `fn sim_color(x: SimSample, p: vec2<i32>) -> vec4<f32>`
where `rgb` is the colour and **`a` is coverage**. Coverage 0 lets the
shared tonemap composite the background through, which is what makes a
matte and a transparent PNG work; the matte multiplies into that same
channel.

A **`SimPreset`** is a whole recipe, not a parameter list: parameters,
the *measured* step count, the initial field it was measured from, the
colouring its state layout wants with *every* parameter that colouring
declares, and optionally a matte and a warp (`None` for the warp sets
the identity, clearing one a previous preset left). It carries no `dt`:
the model's `default_dt` is applied when the model is selected. Applying
half of one produces a picture of nothing — FitzHugh–Nagumo's constants
give spirals from a cut wavefront and a **flat field** from noise.

**The initial field is applied whole:** its kind, plus its noise
amplitude, its radius or its count (`init_changes` in the panel). The
kind alone kept the last field's radius and count and reset noise to
0.05, so the presets that want noise at 1 (Ising, percolation, Lenia
and most others) started from a near-uniform field. Every preset names
a field (`every_preset_names_its_initial_field`); Gray–Scott's are the
default blobs. **The boundary too** (`SimPreset::boundary`, required): a
preset applies the boundary it was measured with. Before, none did, and a
model ran on whatever the last one left. Viscous fingering's inlet and
outlet meet on a wrapping grid, so its front never moved; a sandpile is
defined with an open edge; a growing front's bottom row wraps to the
top. Clamp: fingering, Kobayashi, the snowfake, DLA, DBM. Zero: the
sandpile, percolation, invasion percolation, Packard's snowflake, Eden's
rough front. Everything else is periodic. **Choosing a model** in the panel applies its first
preset's field and boundary too, so a model never starts from the last
one's.
A layer's model leaves it, since the field belongs to the whole
simulation.

A **`SimLayeredPreset`** (`LAYERED_PRESETS` in
[src/sim/mod.rs](../../src/sim/mod.rs)) is the same idea for several
models on one grid: per-layer model and parameters, the couplings as
`(from, to, form, strength, channel mask)`, steps, `dt`,
`steps_per_frame`, init, a colouring or a colour stack. `apply` writes
all of it and resets the matte, the warp and `use_transforms`, so the
preset is the whole picture. The panel offers them in a Layered presets
list and applies one as a single undoable config edit. Four ship: two
coupled Gray–Scotts, two cubic-coupled Brusselators, and two four-field
Turing rings.

---

## The pipeline

```
seed ──▶ step × N ──▶ colour ──▶ resolve ──▶ [shared tail]
 │         │            │           │          density effects
 │         │            │           │          tonemap
 │         │            │           │          colour effects
 │         │            │           │          readback
 │         │            │           └─ grid → output size, filtered
 │         │            └─ cells → rgb + coverage, via the palette
 │         └─ 1–4 dispatches per step, batched into submissions
 └─ the initial field: noise, blob, blobs, ring, line, centre, broken wave
```

The tail from `resolve` on is **the flame renderer's own**, fed an
`Rgba32Float` image in the accumulator's layout. That is what gives
simulation mode density effects, tone mapping, the effect chain,
transparent export and PNG metadata without a line of its own.
`render_with` in [src/renderer/render.rs](../../src/renderer/render.rs)
dispatches a Simulation-mode config to `render_sim`, so CLI export,
thumbnails, textures, video and the gallery inherit it. A render that
only needs the tail gets a `FlameRenderer::for_tail`, without the
flame's per-pixel histograms (see [EXPORT.md](EXPORT.md)).

### The field

A `Rgba32Float` **texture array**, one slice per layer, ping-ponged
between two allocations. `SimRenderer::allocation_error` refuses a grid
or an output larger than the device's maximum texture side, or one whose
two field textures plus the output pass a 4 GiB budget, with a message
rather than an allocation failure; a caller that skipped the check gets
a grid clamped to 8192 cells a side.

**Memory slices.** A layer whose model keeps a memory
(`ModelFeature::Memory`, McCabe's colour memory) has two more
internal slices, appended after every user layer. User layer indices
are unchanged, so couplings, the colour stack and `gather` do not see
them. In the stage loop:
- The owner's last pass writes them (`sim_mem_write`), and they get
  no dispatch of their own in that stage.
- Every other stage copies them through.
- The warp and the layer map move them with their owner.
- They seed to zero.

Switching a memory on or off is part of `SeedIdentity`, so it
restarts the run. A `ReadsMemory` colouring sees them as
`x.m0`/`x.m1`, blended by the resolve like the state. See
[mccabe-multiscale.md](../archive/projects/mccabe-multiscale.md).

**Scratch slices.** A layer whose model has a measure pass that is on
has its scratch after ALL memory (`SliceLayout`). They do not persist:
- The measure pass writes them (`sim_scratch_write`) and the passes
  after it read them from the side it wrote (`sim_scratch_read`).
- No other stage dispatches them, warp and layer map included. A
  copy-through in the measure's stage would overwrite what it wrote,
  and a stale scratch is never read.
- Because they sit after every slice that persists, turning a measure
  pass on or off moves nothing else. The renderer copies the persistent
  slices into the new arrays and the run goes on; it does not reseed.

**Parameters.** Each layer's block in the model-parameter buffer is
128 floats. The last four are the renderer's:
- whether any per-scale warp is on;
- the scratch's first slice (0 when there is none);
- the memory's slice count;
- the memory's first slice.

**Per-scale warps** (`ScaleWarps`, McCabe):
- **Parameters.** Each scale has the field warp's five rates
  (`s{i}_warp_{zoom,rotation,pan_x,pan_y,flow}`). The Warp section
  draws them behind its Field / Scale *k* list, and the model's own
  list hides them.
- **What they do.** The field warp's map moves where that scale reads
  its averages, so its structure drifts as the field would while the
  other scales stay. A step costs no more for it.
- **The reserved flag.** The renderer decides once whether any live
  scale's warp is on and says so in a reserved slot. Off, the step is
  the rule exactly as it was; a per-cell check across the scales cost
  3–5% of every McCabe step.

**The pyramid** (`NeedsPyramid`, McCabe):
- **Build.** A Gaussian pyramid of the field, built before every step,
  one dispatch per level, each level's uniform its own slot in a
  per-step ring.
- **Reads.** Bilinear within a level and linear between two, at
  `level = log2(0.55 r)`.
- **Its fixed texel lattice is a hazard.** Patterns can lock onto the
  bilinear reads' creases along the coarse levels' texel lines.
  McCabe's "shifted grid" averaging moves the lattice every four steps
  by a hash of (seed, step), building level 1 from the field offset by
  it and reading every level above at the position plus it. Measured,
  this trades the axis lean for a diagonal one.

**Exact averages.** McCabe's "Exact discs" averaging replaces the
pyramid with a spectral stage:
- **What it computes.** Per scale, the activator-minus-inhibitor field
  as a circular convolution with antialiased discs, by FFT.
- **The FFT** (`src/sim/fft.rs`): mixed-radix Stockham, one dispatch per
  stage, any grid whose prime factors are at most 64.
- **The stage** (`src/sim/spectral.rs`): one forward and three inverse
  2D FFTs a step for six scales.
- **Where the results go.** An `R32Float` array the renderer owns, at
  step binding 18, which a sampled texture keeps off the browsers'
  storage-buffer count.
- **Limits.** Periodic boundary only; anything else falls back to the
  pyramid. About 1.7× the pyramid's step and ~150 MB at 1080p.
- **Leaning discs.** In table mode each scale's discs can be stretched
  into ellipses of the same area at an angle (`s{i}_stretch`,
  `s{i}_angle`), which grains that scale's pattern along the angle. An
  ellipse is centrally symmetric, so its spectrum is still real and only
  the stage's disc fill changes; a stretch of 1 fills to the bit what the
  round disc did. Exact discs only: the panel hides the columns
  otherwise.

See [mccabe-multiscale.md](../archive/projects/mccabe-multiscale.md), sections 6
to 6c and 9.

### Shaders

One model and one colouring are **spliced into a template** by
[src/sim/assembler.rs](../../src/sim/assembler.rs) — the same
marker-splicing approach the escape assembler uses. Templates exist for
the seed, the step, the colour pass, the warp, the agent passes, the
pyramid and reduction passes, and the three jump-flood passes. The
boundary rule is spliced in rather than branched on, so `sim_read` past
the edge does the right thing with no per-tap cost.

Every model's assembled WGSL is held to the same `shader_lint` rules as
every variation — no self-compares, no self-divisions, no subnormal
literals (see the Metal fast-math section of
[CLAUDE.md](../../CLAUDE.md)) — plus a naga parse and validate, at test
time, over every model × colouring × boundary × resolve combination.

### Stepping and batching

`run_steps` submits in **measured** batches. Each submission is timed
and the next is sized from it against a wall-clock budget, so the
driver never sees a pass long enough to trip the watchdog however large
the step count — and a model with a 200-sweep relaxation pass is sized
down before the first, blind submission rather than after it.

Batching does not change the sequence: one batch of 300 steps and 300
batches of one produce identical fields
(`steps_are_batch_invariant`). That is the property that lets an export
batch freely while a still stays reproducible.

---

## The config

`SimConfig` ([src/config/sim.rs](../../src/config/sim.rs)):

| group | fields |
| --- | --- |
| the run | `model`, `model_params`, `seed`, `init` (`Noise`, `Blob`, `Blobs`, `Ring`, `Line`, `Center`, `BrokenWave`), `steps`, `steps_per_frame`, `dt`, `boundary` |
| the grid | `grid` (`Fixed { width, height }` or `Viewport { scale }`) |
| the resolve | `upscale` (`Nearest`/`Bilinear`/`Bicubic`/`Fsr`), `downscale` (`Box`/`Nearest`), `fit` (`Letterbox`/`Cover`) |
| the view | `view`: `center_x`, `center_y` (fractions of the grid), `zoom` (over the fit), `rotation`, `tile` |
| the picture | `coloring`, `coloring_params`, `matte` (`channel`, `cutoff`, `softness`, `invert`, `edge`) |
| motion | `warp`: `zoom`, `rotation`, `pan_x`, `pan_y`, `flow`, `filter` (`Bilinear`/`Nearest`/`Bicubic`), `mode` (`Continuous`/`Octaves`), `cull`, `layers` (channel mask) |
| layers | `layers`, `couplings`, `color_layers`, `use_transforms` |
| 3D | `terrain` (`SimTerrainConfig`, see below) |

Every field is skip-if-default, so a config that never touched
simulation mode serialises without a `sim` block. Fields reach the
editor through `ConfigPath::Sim*` paths and `ConfigManager`, with undo;
the layer, coupling and colour-layer lists are added, removed and
reordered as whole-config edits (`ConfigManager::apply_config_edit`).
Which paths are animatable is the set `json_to_config_value` accepts
(see [CONFIG.md](CONFIG.md)); choices such as the boundary, the warp's
filter and mode, the fit and the matte's channel and edge are not.

What an edit costs the run is its `UpdateType`: `SimRerender` (colour,
resolve, view, matte, warp, model and colouring parameters, steps, `dt`,
the terrain: the run continues), `SimResample` (a bound grid's scale: the
live field is carried into the new grid as the model's `state` allows, and
the run continues at its step; see below),
or `SimReseed` (model, seed, init, boundary, grid mode or size: the run
restarts).

A bound grid is `round(output × scale)` cells with the scale clamped to
0.125–4 (`SimGrid::cells_for`), so it re-simulates whenever the output
size changes, including an export at another size. A `Fixed` grid is the
same picture at any output size.

**`steps` means two things, and whoever is driving decides which.** To
the panel's transport it is Max Steps, a *cap*: the run stops there
once, and `0` means uncapped. To the animation timeline and to the
exporter it is a *target*: the picture at a time is the state at that
step count. See [UI.md](UI.md).

**`boundary`** is what a step reads outside the grid: `Periodic` (wraps,
no edge artifacts), `Clamp`, `Zero` (a sink — growth models want this,
so the pattern has somewhere to grow into), `Mirror`.

---

## Layers, couplings and the colour stack

A config can hold several **layers** (`SimLayer`: a model, its
parameters, and whether it is stepped; up to `MAX_LAYERS` = 16) —
independent fields in the same texture array. With no `layers` list the
flat `model` / `model_params` are layer 0; adding a second layer moves
them into the list (`promote_model_to_layers`). **Couplings**
(`SimCoupling`: `from`, `to`, `form`, `strength`, a channel mask; up to
32) feed one layer's field into another's rule. Per channel, with *u*
the driven layer's value and *v* the driving layer's
(`COUPLING_ACCESSORS` in
[src/sim/assembler.rs](../../src/sim/assembler.rs)):

| form | term |
| --- | --- |
| `Linear` | *v* − *u*: diffusion between the layers |
| `Cubic` | *u*·*v*·(*v* − *u*) |
| `Quadratic` | *v*² − *u*² |
| `Product` | *u*·*v* |
| `Signal` | the driving layer's field through its OWN kernel table (two blocks = activator − inhibitor), or its published `.y` channel |

Each term is multiplied by the coupling's strength and masked to its
channels.

**Where a coupling lands depends on the target model, not only on the
form.** Everything is summed *after* the rule and scaled by `dt`, with
one exception: for a model declaring `ModelFeature::TakesDrive`, a
`Signal` coupling is instead delivered *before* the nonlinearity
through `sim_drive`, and skipped in the post-rule sum. A model without
`TakesDrive` still receives `Signal` couplings — post-rule, like every
other form. Only `turing` declares it today.

Pre-rule value couplings (linear, cubic, quadratic, product) are
deliberately not offered: they have no source in the papers this
catalogue follows.

**Colour layers** (`SimColorLayer`, up to 8) are a stack, bottom first,
each with its source layer, its own colouring, colouring parameters,
matte, blend mode, opacity and an enabled flag. With a stack, the flat
`coloring` fields are ignored. A `gather` colour layer reads the first
channel of four consecutive layers as one colouring's four channels. The
blend modes are Normal, Lighten, Darken, Multiply, Screen, Overlay, Add
and Hard light, the last being Overlay decided by the top layer.

**Relief** is a colouring meant for the top of the stack under Hard
light: one channel of its source layer as a height, smoothed and lit,
as grey centred on mid-grey. Over mid-grey, Hard light is a shadow
toward black and a highlight toward white by the distance from it,
which is the escape relief's shading. The panel's **Add relief** adds
one in a click. Its lighting follows the escape relief's
(`EscapeShading`): Tilt or Lambert, and the light angle
counter-clockwise from east, 135 by default.

**The relief stage** builds what it reads (`NeedsRelief`, `x.relief`):
- **Before the colour pass**, every frame a colouring needs it: two
  separable passes at grid size, the Gaussian of the softness and its
  derivative along x, then along y.
- **Into an `Rgba32Float`** of (height, d/dx, d/dy), in cells, which the
  resolve interpolates like the state, so the shading stays smooth at
  any zoom.
- **One height per frame**, from the first enabled colouring that asks
  (its parameters 0 and 1: channel and softness), as there is one
  distance field.
- **The cost** on a 1080p grid: about 2 ms a frame at softness 2, 6 ms
  at the maximum 8.
- **The same stage** makes the simulation terrain's height (see *The
  terrain* below).

**`use_transforms`** makes the flame's transforms the layers' per-step
maps: transform *i* warps layer *i*, by its affine **and its
variations**, at a rate that is its weight. Off by default. Note this is
the one place a simulation reaches into the variation registry — which
is why the single-engine `wasm/sim` module, built without
`engine-flame`, can only apply `linear` there.

---

## Presentation: resolve, fit, view, matte, warp

**The resolve** maps the grid to the output. Everything it produces is
*derived at colour time and never written into the field*, so no model
changes and a run's reproducibility is untouched. Interpolation happens
on the **state**, and the colouring is called once on the result — not
the other way round, which matters wherever the colouring is non-linear
(a clamp being crossed, a palette with structure, a matte).

**The fit** frames a grid whose aspect differs from the output's:
`Letterbox` shows the whole grid with bars, `Cover` fills the output and
crops the grid along the axis that does not fit.

**The view** (`SimView`) is a camera the colour pass applies between the
output's pixels and the grid's cells: a centre as a fraction of the grid
(0.5, 0.5 is the middle), a zoom over the fit, a rotation in radians
(positive clockwise on screen, the flame's convention), and `tile`.
Display only: the step never reads it, so moving it never touches the
run, and the viewport, an export and a video show the same framing.
`tile` repeats the field past its edges as the torus a periodic
boundary makes it; under any other boundary the edges show
(`SimView::tiles`).

The viewport navigates it through the shared gesture code
(`ViewKind::Sim2d` in [src/camera/gesture.rs](../../src/camera/gesture.rs)):
a drag pans, the wheel zooms toward the cursor, a turn rotates, and
Reset View restores the centre, zoom and rotation while keeping `tile`.
Fly mode is not offered for the 2D view. See [CAMERA.md](CAMERA.md).

**The matte** separates figure from background: a channel (or `Off`), a
cutoff, a softness, a direction (`invert`), and an edge. It multiplies
the colouring's coverage, so a colouring that already reports empty
cells keeps saying so. Applied per grid cell **before** the resolve
filter, so a magnified edge is antialiased by the same filter that
magnifies it. The edge is either `Threshold` — the cutoff on the
interpolated channel, softness in the channel's units — or `Distance`:
a signed distance to the edge in cells from a jump flood over the
occupancy, which interpolates smoothly at any magnification and makes
the softness a width in cells, at ceil(log2 N) + 2 grid-sized dispatches
per coloured frame.

**The warp** is a per-step affine resample of the field — zoom,
rotation, pan, and an analytic swirl — with a `filter` that is
deliberately not always bilinear. A fractional-pixel bilinear resample
is a small blur, and a step applies one; over thousands of steps they
add up and erase reaction–diffusion entirely (measured: a 0.4 %/step
zoom on the coral preset came out a single dot). Nearest is exact for
integer pans and for integer state, which bilinear would smear into
values a sandpile has no meaning for. Bicubic (Catmull-Rom) is sharper
than bilinear under magnification.

**FSR** (`SimUpscale::Fsr`) is the resolve's one upscale of COLOURS rather
than state. While the picture magnifies the grid, the colour pass runs at
about a cell a pixel through the same view (`SimRenderer::fsr_size`: the
output over the fit's magnification times the view's zoom), compiled as
not magnifying, and FSR 1 (`renderer::upscale::Fsr`, `FsrTarget::Linear`:
AMD's EASU and RCAS, `shaders/upscale_fsr.wgsl`) draws that up to the
output, the linear colours encoded into [0, 1) around its passes so its
limiters hold. An edge stays crisp where interpolating the state rounds
it, and a colour between two palette entries can appear where an edge
passes, as in any upscale of a picture.

The warp has two **modes**. `Continuous` resamples the whole affine into
the field every step. `Octaves` applies the zoom alone: the per-step
zoom accumulates as a view magnification in [1, 2) that the colour pass
shows, and the field is resampled by exactly 2 each time it reaches 2,
so there is no per-step blur; rotation, pan and flow are not applied in
this mode. Its state is a function of the step index alone
(`SimWarp::octave`), so a run stays reproducible and batch invariant.
In octave mode `cull` freezes the cells outside the visible window and a
halo around it, which the next doubling crops away anyway. `layers` is a
channel mask (bit 0 = x … bit 3 = w) of what the warp moves, continuous
mode only; moving one channel while the others stay is differential
advection.

The warp is **not a camera**: it moves the field, and changes what the
next steps do. The view above is the camera.

---

## The terrain

`sim.terrain` (`SimTerrainConfig`, drawn by
[src/sim/terrain.rs](../../src/sim/terrain.rs)) shows the grid as a lit
height field: one channel of one layer, smoothed by the relief stage, is
the height; the colour stack, once a cell, is the albedo; a solid camera
looks at it. The world is the grid's cells, x right and y up the
picture. It is drawn by the escape engine's terrain renderer and its
tiers, so the lighting, shadows, occlusion and path tracer are the
escape terrain's — see [TERRAIN.md](TERRAIN.md) for those.

| group | fields |
| --- | --- |
| the height | `enabled`, `layer`, `channel` (0–3), `softness` (cells), `height` (a field unit, in grid widths) |
| the ground | `tiling` (`Single`, or `Repeat` to the horizon, seamless on the periodic boundary), `far` and `haze` (a repeated ground's reach and fog) |
| the light | `tier` (`Auto`, `Lit`, `PathTraced`), `shadow`, `shadow_sharpness`, `occlusion`, `path` (the path tracer's settings) |
| the camera | `cam_pitch`, `cam_yaw`, `cam_bank`, `cam_fov`, `cam_distance` (grid widths), `target_x`, `target_y` (fractions of the grid), `target_lift` |

The ground is rebuilt whenever the field or how it is drawn changes —
every step of a running simulation — so a run shows the lit tier, and in
`Auto` the path tracer gathers once it rests; an export is path traced
unless the tier is `Lit`. The viewport's camera is `ViewKind::SimTerrain`,
which supports fly mode (see [CAMERA.md](CAMERA.md)). Terrain edits are
`SimRerender`: the run is kept.

The terrain needs the `terrain` Cargo feature (on by default). A build
without it — the gallery modules — renders a terrain config as its 2D
picture. In the app a terrain export runs in the background on a device
of its own (see [EXPORT.md](EXPORT.md)).

---

## Textures

A **texture** is not an image file: it is a Simulation-mode config, a
recipe that reproduces its field exactly, used by escape-time pictures
(`EscapeConfig::texture`; see [ESCAPE.md](ESCAPE.md)). The library
([src/textures/mod.rs](../../src/textures/mod.rs)) lists:

- **Presets**, `assets/textures/*.fflame`, embedded in the binary by
  `build.rs`.
- **User textures**, saved from Simulation mode, stored as
  `textures/<name>.fflame` through the storage backend (the data
  directory on desktop, browser storage on the web).

**Save as texture** (the Simulation panel) saves the simulation as it
stands: the live run's step index replaces `steps` once the run has
advanced, and a viewport-bound grid is fixed at its current size.
`textures::as_recipe` then fixes the grid (at most `MAX_TEXTURE_SIDE` =
2048 on its longer side), forces `Letterbox` so the grid fills the image
edge to edge, and drops any texture of the config's own — textures do
not nest. Names are sanitised and numbered if taken. On desktop, saving
also caches the image from the live run, so the texture never needs a
first generation (`App::cache_saved_texture_from_run`): it is rendered
through `render` continuing the viewport's simulation as a caller-owned
engine, which colours a field already at the recipe's step count without
stepping it. That is done only when the field is the recipe's: the same
grid, exactly its step count, and every step taken under its settings
(`SimRenderer::ran_steadily_as`; a parameter edited mid-run makes a field
no recipe reproduces). A run that uses the flame's transforms, or shows
a terrain, is left to generate.

**The Textures panel** ([src/ui/textures_panel.rs](../../src/ui/textures_panel.rs))
shows presets, then the user's own, each with a preview. Picking one
copies its recipe into the escape config in one undo step, so the file
that uses it needs nothing else; user textures can be renamed and
deleted, and a fractal already using one keeps its own copy. The panel
is available in Escape and Simulation modes (in Simulation, to manage
what was saved).

**Generating without asking** is only for what is quick:
`textures::may_generate` allows a texture that is cached, cheap
(`generation_cost`, grid cells × steps × layers, at most
`AUTO_GENERATE_COST` = 1e10, about four of the shipped presets), or asked
for this session (`approve`). Anything else waits: the panel shows a
Generate button with its size and step count, and Generate all; the
viewport draws an escape picture without it and says so once
(`texture_sync`), until the panel's Generate. An asked-for generation
still blocks the frame while it runs, but its "generating" label is drawn
first. Exports do not ask: a file must hold the picture its config
names.

**The image** is derived data. `textures::obtain` reads it from the
cache, or generates it by rendering the recipe through
`renderer::render` at the recipe's grid size and caches it. The cache
key is a SHA-256 of the recipe's file form without its `version` stamp,
its size and `TEXTURE_CACHE_VERSION`
([src/textures/cache.rs](../../src/textures/cache.rs)); desktop keeps
PNGs under `texture_cache/` in the data directory, the web keeps them in
memory for the session. The key used to carry the crate version, so a
release or a config format change regenerated every texture.
`TEXTURE_CACHE_VERSION` is what moves when the engine's output may:
`the_texture_engine_is_the_one_the_cache_was_versioned_for` pins a
fingerprint of the shaders a texture is made with (every model's seed and
step, every colouring, the tone map), and fails when they change. Old
entries stay on disk, unused; nothing evicts the cache.

---

## Reproducibility

A field is only ever the state of the run that produced it, so the
renderer restarts when the config no longer describes it. `SeedIdentity`
([src/sim/renderer.rs](../../src/sim/renderer.rs)) is the part that
matters: the model of every layer, the boundary, the init, the seed, and
each layer's memory slice count. Change any of those and the run
restarts. A change of grid size reseeds too (`SimReseed`), and so does a
resize that changes a bound grid's cells while nothing holds them.

**A bound grid's scale carries the field** (`SimResample`;
`SimRenderer::request_resample`, `carry_field`). The resize that changes
the grid keeps the old field, and before the next step or colour reads
it, every slice is resampled into the new grid, each cell from where its
centre falls in the old one: Catmull-Rom for a `Continuous` model, held
within its 2x2 neighbourhood's range so nothing overshoots, the nearest
cell for a `Discrete` one. A config with several layers takes the most
careful of their kinds (`StateKind::of_config`). The run goes on at its
step: a feature keeps its size in cells, so the picture is the same one,
finer or blockier, and the pattern then re-forms at its own size. An
agent model, a run not yet seeded, or a grid of another shape (the carry
maps each axis by its own ratio, and a field is never stretched) reseeds
as before.

**A resize does not restart an active run** (`src/app/sim_lock.rs`).
While the transport runs or the timeline drives the steps, a bound grid
is held at the run's cells: the renderer is resized with that grid fixed,
and once the viewport is not the size the run began at it letterboxes to
the grid's shape (`ui::SimLock`, `view_controls::letterbox_shape`), so
full screen shows the same field larger, at its step. Pausing or stopping
lets go: the viewport fills its panel and the grid follows it at the
config's scale, from the seed. Not with the letterbox setting on, whose
shape is the picture's already, nor for a fixed grid. A change of the
run's scale while it is held (an edit, or the quality governor's pick)
keeps the hold: the grid is the run's at the new scale, the same shape,
and the field is carried into it.

**The quality governor** (`src/app/governor.rs`, `SimGovernor`;
[quality-governor.md](../archive/projects/quality-governor.md)). While a bound
grid's run is active and the Rendering menu's Quality Governor is on, the
run's scale is picked when it starts, from the measured cost of a step,
as the best of the config's and 75, 50, 37.5, 25, 18.75 or 12.5% at which
its steps a frame fit the Target Frame Rate. It is held for the run,
picked again only when far wrong for two seconds with no edit, and the
field is carried back to the config's scale on stop. A governed run is a
different run from the still: a still and an export re-run at the
config's grid. The panel says "Playing at 50% (quality governor)" under
the grid scale, and **Fit to Target** writes the scale that plays in real
time to the config (an undoable edit), for a still or an export at it.

Model and colouring **parameters are deliberately excluded** — turning
Gray–Scott's feed rate while it runs is what the slider is for, and
reseeding on it would make every model unusable.

---

## Adding a model

1. Write a `static ModelDef` in
   [src/sim/models.rs](../../src/sim/models.rs) with inline WGSL.
2. Append it to `MODELS` in [src/sim/mod.rs](../../src/sim/mod.rs)
   (**append-only** — order is the stable ID order).
3. Add a section to
   [SIMULATION-CATALOG.md](SIMULATION-CATALOG.md): the
   governing rule as its source states it, the discretisation, the
   parameters, and what bit you. Cite the source and mark it `[read]`
   or `[verify]` honestly.
4. Give it presets you have **run and inspected**. Three tests hold you
   to this: `every_preset_names_a_colouring` and
   `preset_colorings_are_complete` run by default, and
   `every_preset_draws_something` renders every shipped preset and
   fails any whose image is flat. That last one needs a GPU and is
   `#[ignore]`d, so **run it deliberately** — it is the only check that
   can see whether a preset draws anything at all, and it has caught a
   model rendering black that no name check could.
5. A parameter that multiplies a step's work -- a kernel radius read
   cell by cell, an agent count -- goes in `SLOW_PARAMS` in
   [src/sim/mod.rs](../../src/sim/mod.rs), so the panel commits it when
   a drag ends instead of overshooting (UI.md, *Slow settings*).

---

## Gotchas

**A parameter plane is mostly uniform.** The interesting region is a
thin curve through it. Gray–Scott is the extreme case, measured:
`mitosis` is a blank image at +2 % on feed and kill together, while
`coral` survives +5 % and dies by +8 %. Other models hold at ±30 %.
Start from a preset.

**Steps are not free and not optional.** Reaction–diffusion needs
thousands of steps before the pattern is the pattern. A preset's count
is measured; scale it rather than replace it.

**`dt` has a ceiling per model** and the panel enforces it. Past it the
explicit solver diverges, and the picture goes to noise or to a flat
extreme rather than to something wrong-but-plausible.

**The registry's parameter names are not always the papers' letters.**
Brusselator's are `feed_a`/`feed_b`, not *a*/*b*. `sim.models()` and the
catalogue are the authority.

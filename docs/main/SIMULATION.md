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
- [SCRIPTING.md](SCRIPTING.md) — the `sim` script surface
- [simulation-catalog.md](../projects/simulation-catalog.md) — every model,
  its governing rule, its source, and what bit it. **Read this before
  touching a model.**
- [simulation-fractals.md](../projects/simulation-fractals.md) — the
  master plan and its decision record

**Code locations:**
- [src/sim/mod.rs](../../src/sim/mod.rs) — `ModelDef`, `SimColoringDef`,
  the `MODELS` and `COLORINGS` registries, the pure rules
- [src/sim/models.rs](../../src/sim/models.rs) — 31 models, inline WGSL
- [src/sim/colorings.rs](../../src/sim/colorings.rs) — 11 colourings
- [src/sim/assembler.rs](../../src/sim/assembler.rs) — WGSL assembly
- [src/sim/renderer.rs](../../src/sim/renderer.rs) — `SimRenderer`
- [src/config/sim.rs](../../src/config/sim.rs) — `SimConfig` and its paths
- [src/ui/sim_panel.rs](../../src/ui/sim_panel.rs) — the panel

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
| `max_dt` | stability bound for the explicit solver. Exceed it and the field diverges. |
| `diffusion` | which parameters are diffusion rates, for the dt ceiling. |
| `agents` | an `AgentDef` for the models with a moving population (physarum). |
| `kernel` | a function building a `SimKernel` lookup table, for the large-radius convolution models (Lenia, SmoothLife, McCabe). |

**`ModelFeature`** is how a model opts into machinery it needs, so
nothing pays for what it does not use:

`NeedsRng`, `NeverStills`, `NoTimeStep`, `NeedsPyramid`, `NeedsAgents`,
`NeedsMinMax`, `TakesDrive`, `PublishesSignal`.

**`SimColoringDef`** is the same shape for the picture side, with
`ColoringFeature`: `NeedsGradient`, `NeedsStructure`, `NeedsDistance`,
`ReadsCell`. A colouring is `fn sim_color(x: SimSample, p: vec2<i32>) -> vec4<f32>`
where `rgb` is the colour and **`a` is coverage**. Coverage 0 lets the
shared tonemap composite the background through, which is what makes a
matte and a transparent PNG work; the matte multiplies into that same
channel.

A **`SimPreset`** is a whole recipe, not a parameter list: parameters,
the *measured* step count, `dt`, the initial field the model needs, the
colouring its state layout wants with that colouring's parameters, and
optionally a matte and a warp. Applying half of one produces a picture
of nothing — FitzHugh–Nagumo's constants give spirals from a cut
wavefront and a **flat field** from noise.

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
 └─ the initial field: noise, blob(s), ring, line, centre
```

The tail from `resolve` on is **the flame renderer's own**, fed an
`Rgba32Float` image in the accumulator's layout. That is what gives
simulation mode density effects, tone mapping, the effect chain,
transparent export and PNG metadata without a line of its own.
`render_sim` in [src/renderer/render.rs](../../src/renderer/render.rs)
is the dispatch point, so CLI export, thumbnails, video and the gallery
inherit it.

### The field

A `Rgba32Float` **texture array**, one slice per layer, ping-ponged
between two allocations. A grid is capped at 8192 cells a side.

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
| the run | `model`, `model_params`, `seed`, `init`, `steps`, `steps_per_frame`, `dt`, `boundary` |
| the grid | `grid` (`Fixed` cells or `Viewport` × scale), `fit` |
| the resolve | `upscale` (`Nearest`/`Bilinear`/`Bicubic`), `downscale` (`Box`/`Nearest`) |
| the picture | `coloring`, `coloring_params`, `matte` |
| motion | `warp` |
| layers | `layers`, `couplings`, `color_layers`, `use_transforms` |

Every field has a `ConfigPath::Sim*`, so everything flows through
`ConfigManager` with undo, and every numeric one is animatable.

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

A config can hold several **layers** — independent fields in the same
texture array, each with its own model and parameters. **Couplings**
feed one layer's field into another's rule:

| form | what the driving layer contributes |
| --- | --- |
| `Linear`, `Cubic`, `Quadratic` | its value, raised to that power |
| `Product` | its value multiplied with the target's |
| `Signal` | its field through its OWN kernel (two-block = activator − inhibitor), or its published `.y` channel |

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

**Colour layers** are a stack, bottom first, each with its own colouring
and blend mode and opacity. With a stack, the flat `coloring` fields are
ignored. A `gather` colour layer reads the first channel of four
consecutive layers as one colouring's four channels.

**`use_transforms`** makes the flame's transforms the layers' per-step
maps: transform *i* warps layer *i*, by its affine **and its
variations**, at a rate that is its weight. Off by default. Note this is
the one place a simulation reaches into the variation registry — which
is why the single-engine `wasm/sim` module, built without
`engine-flame`, can only apply `linear` there.

---

## Presentation: resolve, matte, warp

**The resolve** maps the grid to the output. Everything it produces is
*derived at colour time and never written into the field*, so no model
changes and a run's reproducibility is untouched. Interpolation happens
on the **state**, and the colouring is called once on the result — not
the other way round, which matters wherever the colouring is non-linear
(a clamp being crossed, a palette with structure, a matte).

**The matte** separates figure from background: a channel, a cutoff, a
softness, and a direction. It multiplies the colouring's coverage, so a
colouring that already reports empty cells keeps saying so. Applied per
grid cell **before** the resolve filter, so a magnified edge is
antialiased by the same filter that magnifies it.

**The warp** is a per-step affine resample of the field — zoom,
rotation, pan, and an analytic swirl — with a `filter` that is
deliberately not always bilinear. A fractional-pixel bilinear resample
is a small blur, and a step applies one; over thousands of steps they
add up and erase reaction–diffusion entirely (measured: a 0.4 %/step
zoom on the coral preset came out a single dot). Nearest is exact for
integer pans and for integer state, which bilinear would smear into
values a sandpile has no meaning for.

The warp is **not a camera**. It moves the field, and it is why
viewport navigation is currently refused in Simulation rather than
misdirected — a display-time view is a separate, unbuilt feature.

---

## Reproducibility

A field is only ever the state of the run that produced it, so the
renderer restarts when the config no longer describes it. `SeedIdentity`
is the part that matters: the model of every layer, the boundary, the
init and the seed. Change any of those and the run restarts.

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
   [simulation-catalog.md](../projects/simulation-catalog.md): the
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

# Simulation Mode — the GPU pipeline

**Status:** Planning, 2026-09-01. No code. Companion to
[simulation-fractals.md](simulation-fractals.md) (the master plan),
[simulation-catalog.md](simulation-catalog.md) (every model, with
sources) and [simulation-integration.md](simulation-integration.md)
(the file-by-file checklist).

This document is the rendering pipeline for the third render mode:
what lives on the GPU, in which format, in which pass order, and why.
Every constraint below was checked against the code as it stands
(the survey is summarised in §1); where the plan departs from what the
escape engine does, it says so and why.

---

## 1. What the codebase already settles

The nearest relative is **not** the effects chain, which the seed doc
guessed — it is `EscapeRenderer`'s own compute ping-pong. Facts that
bind the design:

| fact | where | consequence |
|---|---|---|
| Read-write storage access is **rejected for `rgba32float`** at bind-group-layout creation; storage *buffers* exceed the 128 MB binding limit at 4K. | `src/escape/renderer.rs:547-555` | A field is a **pair** of textures: read via `texture_2d<f32>` + `textureLoad`, write via `texture_storage_2d<rgba32float, write>`, swap. Same for every op below. |
| `Queue::write_buffer` applies at the **submission** boundary, not between passes. Several passes sharing one uniform buffer all read the last write. | `src/escape/renderer.rs:736-741` (height blur), `src/renderer/effect_chain.rs:939-946` (JFA pre-allocates a slot per pass) | N steps per submission need N distinct uniform regions. Use **one buffer with dynamic offsets** (256-byte aligned), sized for the largest batch. |
| The tonemap input is any float 2D texture with `TEXTURE_BINDING`, point-fetched via `textureLoad`. Semantics: `rgb` = colour, `a` = raw count; `bucket = a·100`; `bucket < 0.001` is background. | `src/gpu/pipelines.rs:212-297`, `shaders/tonemap.wgsl:404-426` | The sim's colour pass writes `vec4(rgb, coverage)` exactly as the escape templates do. |
| `Features::FLOAT32_FILTERABLE` is optional and gated at runtime; `SHADER_F16` is never requested; no format-capability probing exists. | `src/gpu/device.rs:257-258`, `src/renderer/effect_chain.rs:841` | Do not depend on filterable f32 textures. Pyramid taps are **manual bilinear via `textureLoad`** — universal, no sampler, no feature. |
| Every compute shader in the tree is `@workgroup_size(8, 8, 1)` dispatched `dims.div_ceil(8)`. | survey §7 | Keep it. |
| Windows' driver watchdog is ~2 s per dispatch; the escape path exists in chunked/banded form with a `TimestampPacer` for exactly this reason. | `src/gpu/device.rs:326-330`, `src/escape/renderer.rs:769` | Steps are batched per frame under a measured GPU budget; no single dispatch may approach the watchdog. |
| Escape mode is excluded from `is_rendering` in `Event::AboutToWait`; without UI activity the loop sleeps. Progressive escape gets continuous frames by calling `request_redraw()` from inside `render()`. | `src/app/mod.rs:1108-1112`, `:2313` | A stepping simulation does the same from its render branch (self-contained), and adds a `sim_running` term to the frame-pacing condition so VSync-off pacing applies. |
| Metal runs shaders fast-math on; self-compare and self-division are lint-rejected in every `shaders/*.wgsl` and registered WGSL. | CLAUDE.md GPU rules, `shader_lint` in `src/variations/mod.rs` | Sim kernels use `!(abs(x) <= 1e32)` for non-finite guards, `ff_atan2` where a zero pair is reachable, and never `x / x`. |
| `textureSample` only from uniform control flow on WASM. | CLAUDE.md | Sim shaders never sample; `textureLoad` throughout. |

The only new GPU capability the mode needs that nothing in the tree
uses today is **integer atomics into a storage buffer from an
agent pass** — and that is the flame histogram's mechanism, so it is
new to the *escape* side only.

---

## 2. The shape

```
                    ┌──────────── one STEP (repeated K times per frame) ───────────┐
 seed(noise/shape)  │ [warp] → [pyramid build] → [update stencil] → [agents] → swap │
        ↓           └──────────────────────────────────────────────────────────────┘
   field pair (A/B) ──────────────────────────────────────────────────────┐
                                                                          ↓
                                                    colour pass → Rgba32Float (rgb, coverage)
                                                                          ↓
                                            resolve/downsample (if grid ≠ display)
                                                                          ↓
                                       density effects → tonemap → colour effects → present
```

Everything right of the colour pass is the existing tail, unchanged —
it already accepts `EscapeRenderer::output_view()` and will accept
`SimRenderer::output_view()` the same way.

Inside the step box, the ops are **fixed stages a model can enable**,
not a general graph. A model declares which stages it needs; the
renderer assembles the pass list once per model and replays it K
times per frame. This is the escape engine's registry-and-template
pattern (`FormulaDef` + `assemble`) applied to a multi-pass loop, and
it is deliberately narrower than the seed doc's "tiny op graph": four
stages compose every model in the catalogue, and a graph would be a
second scheduler to maintain for no additional model.

---

## 3. State: textures, formats, packing

### 3.1 The field pair

- `field[0]`, `field[1]`: `Rgba32Float`, `STORAGE_BINDING | TEXTURE_BINDING | COPY_SRC`, size = the **simulation grid** — fixed in the config, or derived from the viewport through the binding option (§7).
- Four channels per texel, packed per model:

| model class | .x | .y | .z | .w |
|---|---|---|---|---|
| Gray–Scott, Brusselator, Schnakenberg, Oregonator, FHN | u | v | age / last-change step (for colouring) | spare |
| Kobayashi phase field | phase p | temperature T | age | spare |
| McCabe multi-scale | field | chosen-scale index (as f32) | colour accumulator hue | spare |
| Lenia / SmoothLife | A (state) | potential U (last) | growth G (last) | spare |
| Swift–Hohenberg, Cahn–Hilliard | u | ∇²u (previous pass) | age | spare |
| discrete CA (cyclic, hodgepodge, RPS, Wolfram, GG snowflake) | state as f32 (exact ints ≤ 2²⁴) | aux | age | spare |
| DBM / Laplacian growth | potential φ | occupancy (0/1) | growth time | spare |
| Physarum, DLA walkers | trail / occupancy | — | age | spare |

- Why f32 and not the `rgba16float` the seed doc suggested: the
  escape accumulator moved from f16 to f32 precisely because f16's
  11-bit mantissa hit a floor for accumulated means
  (`src/gpu/buffers.rs:1569-1579`), and phase-field gradients and
  Kobayashi's `∂T/∂t = ∇²T + K ∂p/∂t` coupling are the same class of
  small-difference arithmetic. f32 costs 2× the bandwidth of f16; at
  1080p a step's two field reads + one write are ~100 MB, which is
  not the bottleneck (the pyramid is). Revisit only if measured.
- Integer state rides in f32 channels as exact small integers
  (every integer up to 2²⁴ is exact). Cyclic CA states, hodgepodge
  levels, sandpile grain counts, GG attachment flags all fit. A
  dedicated `R32Uint` texture is possible (r32uint supports
  read-write storage) but adds a second binding path for no gain
  until a model needs > 2²⁴ or bit-packing.

### 3.2 The pyramid

For every model that needs wide-radius averages (McCabe, Lenia,
SmoothLife, and the reduction used for settle detection):

- A chain of `Rgba32Float` levels, each half the previous, built each
  step by a 2×2 box downsample pass (compute, write-only storage per
  level — **separate textures per level**, because a storage view of
  a mip level and a sampled view of the base cannot share one
  texture cleanly across the two access modes without
  `TEXTURE_BINDING_ARRAY` gymnastics; separate textures cost nothing
  extra).
- A disc average of radius r is read as one **manual trilinear**
  fetch: level `l = log2(r)` fractional → bilinear at `floor(l)` and
  `ceil(l)` via four `textureLoad`s each, blended. Eight loads per
  scale per texel, O(1) in r. This is the seed doc's proposal, with
  the sampling done by hand so it needs no filterable-f32 feature.
- **Not** a summed-area table: an f32 SAT at 4K accumulates ~8M
  values, and f32's 24-bit mantissa makes the corner-difference
  error ~0.5 in field units — unusable for a [−1, 1] field. A SAT
  would need f64 or a two-float split; the pyramid needs neither.
- **Not** an FFT (Reusser's route): correct but ~log₂N dispatches per
  transform ×2 per field per step, and it needs its own precision
  work. Parked, as the seed doc says; the pyramid dissolves the cost
  question first.
- ~~The pyramid's box average is not a disc.~~ **Measured
  (2026-09-05): it does depend on the isotropy, and the box is out.**
  The A/B was run (`proto_mccabe_pyramid.py`): the box pyramid's
  McCabe texture is visibly axis-aligned with a spectrum half as
  peaked as the disc reference's. The shipped pyramid is **Gaussian**
  — each level is a separable [1 4 6 4 1]/16 blur then decimate, one
  25-tap dispatch per level — which is isotropic, and with the level
  mapping calibrated to `log2(0.55 r)` it reproduces the disc
  reference's feature size and amplitude. The fallback sketched here
  (a blur per scale) was not needed: blurring once per LEVEL is the
  same cost paid once rather than per scale. Cost: ~8 taps a cell for
  the whole pyramid, and McCabe at 1080p runs 5.25 ms/step.

### 3.3 Agents and deposit

Physarum and walker-DLA carry a **particle buffer**, not a field:

- `agents: array<Agent>` storage buffer, `Agent { pos: vec2<f32>, heading: f32, state: u32 }` (16 B), N up to a few million (the flame path already runs millions of points per frame).
- `deposit: array<atomic<u32>>` storage buffer, one u32 per grid cell (4 B/cell — 33 MB at 4K, under the 128 MB binding limit). Agents `atomicAdd` fixed-point deposits; a resolve pass converts the buffer into the trail field texture (multiply by `1/scale`, add to the diffused trail). This is the flame histogram (`color_scale = 100`, `accumulate.wgsl`) reused as a mechanism, which is what the seed doc noticed: walkers *are* the chaos game with a sticky/deposit test.
- Integer atomics sum exactly, so the deposit is order-independent and the simulation stays deterministic on a given GPU even though thousands of threads race.
- RNG: the PCG from `shaders/core/rng.wgsl`, seeded per agent from `(seed, agent_index)` and advanced per step. Deterministic.

### 3.4 Output

`output`: `Rgba32Float`, `STORAGE_BINDING | TEXTURE_BINDING | COPY_SRC`, at **display** size. Written only by the colour pass (and the resolve pass when grid ≠ display). This is the view handed to the tonemap.

### 3.5 Memory

Per cell: field pair 32 B + pyramid ≈ 11 B + output 16 B (+ deposit 4 B, + integer aux 0). At 1920×1080: ~120 MB. At 3840×2160: ~490 MB, plus agents. The grid decision in §7 is what keeps 4K exports affordable: a `Fixed` 2048² grid, or `Viewport { scale: 0.5 }`, upsampled to a 4K output costs 250 MB, not 490 MB, and the pattern's feature size is set by the model's radii in grid cells anyway. A bound grid at scale 1.0 on a 4K output pays the full 490 MB, which the `allocation_error` precheck must refuse on cards that cannot hold it.

---

## 4. The stages

Each stage is one compute pass, `@workgroup_size(8, 8, 1)`, one uniform slot (dynamic offset) carrying `{ step_index, dt, model params[16], grid size, boundary mode }`.

### 4.1 Warp (optional, first in the step)

Resamples `field[read]` into `field[write]` through a per-step affine about the grid centre — zoom `s`, rotation `θ`, translation `(tx, ty)` — or through a flow field (a second texture, or an analytic swirl). Bilinear via four `textureLoad`s; boundary mode wrap / clamp / mirror / zero. The same kernel, run once at a fixed grid-to-grid affine, is the resampler §7 uses when a viewport-bound grid changes size (nearest for integer channels).

This is the seed doc's "expanding space" resample promoted to a stage. It buys the zooming-BZ look (`s < 1` each step), McCabe's rotate-and-average symmetry when combined with the pyramid stage (§4.2 handles symmetry directly, cheaper), and the demoscene feedback-zoom family.

**Built 2026-09-05, and measured (master plan, phase 6):** a fractional-pixel bilinear resample is a blur of variance f(1−f) per axis, and a step applies one, so over thousands of steps the stage erases a reaction–diffusion pattern rather than moving it — the "zooming BZ" at 0.4 %/step for 4,000 steps is a dot. Nearest at a rate under half a cell is the identity. The stage therefore ships with a `filter` the spec did not have, and the regimes that work are nearest at rates that move whole cells, integer pans, and bilinear over short runs. It reuses nothing from the flame affine machinery in code — the maths is a 2×3 matrix — but it reuses the *vocabulary* the View panel already has (zoom, rotation, pan), which is what the panel exposes.

### 4.2 Pyramid build (optional)

`log2(max radius)` downsample passes from `field[read]` (or from the warped copy). Each pass reads level *l* and writes level *l+1* by a 2×2 box. Cost: 1/3 of one full-resolution read+write in total. Runs once per step; every scale then costs 8 loads per texel in the update stage.

McCabe's cyclic symmetry — average activator and inhibitor with their counterparts at `k·2π/n` (paper, figures 8–12) — is applied **in the update stage's reads**, not by warping the field: for each of the n rotations, fetch the pyramid at the rotated coordinate and average. That is n× the pyramid taps, which is why the prototype's symmetric variant is ~10× the plain step (§9) — the paper's own images are 5–9-fold, and it stays affordable because the taps are O(1).

### 4.3 Update stencil (the model)

Reads `field[read]` (3×3 or 5×5 neighbourhood, or pyramid taps), writes `field[write]`. This is the pass whose WGSL body the model registry supplies: a function

```wgsl
fn sim_step(p: vec2<i32>, n: Neighborhood, s: SimState) -> vec4<f32>
```

where `Neighborhood` gives the nine (or 25) taps already loaded with the boundary mode applied, and `SimState` carries the pyramid accessor and the params. The template does the loads, the clamp/wrap, the age bookkeeping and the store; the model does the arithmetic. Two-Laplacian models (Swift–Hohenberg, Cahn–Hilliard) declare `passes = 2`: the first stores ∇²u into `.y`, the second reads it back — the same "one pass per derivative order" the escape relief blur does in two passes.

Boundary conditions are a per-model default with a config override: **periodic** (McCabe's paper, most RD), **clamp** (growth models against a wall), **zero** (DLA on a black background).

### 4.4 Agents (optional, after the stencil)

Two passes: `agents_move` (one thread per agent: sense from the trail/occupancy field, turn, move, deposit via atomic) and `deposit_resolve` (one thread per cell: fold the u32 buffer into the trail channel, then clear it). Physarum's diffuse+decay is the model's stencil (§4.3) run on the trail channel, so the per-step order is stencil → agents → resolve, which matches Jones' "deposit, then diffuse, then decay" once the frame is viewed as a loop.

### 4.5 Colour

One pass, once per frame (not per step): reads `field[read]` and writes `output`. The colouring registry (§6) supplies the body. Coverage is 1.0 for dense models; for growth models it is the occupancy channel, so un-grown cells read as background and the tonemap's background colour shows through — the same coverage convention the escape interior uses.

### 4.6 Resolve

Colouring happens at **grid** resolution (§4.5 writes `output_grid`); the resolve pass scales that coloured image into `output_display` whenever grid ≠ display, so colouring and scaling are separate controls (§7). Filters, chosen per direction in the config:

- **Upscaling** (`sim.upscale`): `nearest` (crisp cells — the CA look), `bilinear`, `bicubic` (Catmull–Rom, 16 loads). A later option is `smooth`: resample the *field* and colour at display resolution, which gives continuous palette gradients across a cell; it needs the colouring to run inside the resolve pass and is deferred.
- **Downscaling** (`sim.downscale`): `box` (area average over the covering cells — exact for integer ratios, the default) or, when the pyramid stage is built anyway, the pyramid level nearest the ratio with a bilinear blend (cheaper at large ratios). Averaging the *coloured* image, not the field, is deliberate: averaging a binary CA's field and then colouring gives grey; averaging its colours gives the correct mixture.

Reuse the inline WGSL shape of `EscapeRenderer::run_resolve` (factor spliced into the source) rather than the unreferenced `shaders/downsample.wgsl`. Coverage (alpha) is averaged like the colour channels, so partially covered display pixels blend toward the background in the tonemap the way escape's supersampled edges do.

### 4.7 Settle reduction (optional)

For "run until settled" and for the HUD: a pyramid of `|field[write] − field[read]|` down to 1×1 gives the mean step change in ~log₂N tiny passes; read back once every few frames. This is the pyramid stage on a difference, nothing new.

---

## 5. Driver: steps per frame, pacing, watchdog

- **Continuous mode** (the panel's Run): each frame runs K steps. K is adaptive: start at 1, measure the batch's GPU time by timestamp query (reuse the `TimestampPacer` design — one query pair around the whole batch), grow K by 2× while the batch stays under budget, halve on overshoot. Budget = the pacer's same target (`GPU_TARGET_MS`, 10 ms) so the UI stays responsive; a "priority" slider can raise it for unattended runs.
- **Hard bound**: no single *dispatch* exceeds one full-resolution pass, and K is capped so a batch never approaches the ~2 s watchdog window — measured, not assumed, exactly as the escape chunk ceiling is. Measured 2026-09-03 (§10): batching steps into one command buffer is worth 0.8% across a 256× range, so K is a pacing and watchdog device only. Submitting one step at a time and re-reading the clock between them costs nothing and stops on a budget boundary instead of overshooting by a batch.
- **Step mode**: the panel's Step button runs exactly one step; Pause stops; Reset re-seeds.
- **Export mode**: `steps` from the config is the contract — run exactly that many from the seed, then colour. No pacing, no timestamps needed; batches are sized by the same watchdog bound. This is what makes an export reproducible on a given GPU.
- **Continuous redraw**: the render branch calls `window.request_redraw()` while running; the frame loop's pacing condition gains a `sim_running` term so VSync-off `target_fps` pacing applies (both hooks named in the integration doc).
- Playback of an animation keyframes the model params; the simulation keeps stepping at the configured steps-per-frame, so a 60 s video at 4 steps/frame is 14,400 deterministic steps. `steps_per_frame` is itself animatable.

---

## 6. Colouring

A second registry, `SimColoringDef`, mirroring `ColoringDef`: WGSL body

```wgsl
fn sim_color(f: vec4<f32>, grad: vec2<f32>, p: vec2<i32>) -> vec4<f32>
```

returning `(rgb, coverage)`. The template computes the central-difference gradient of the chosen channel once (for hillshade). First set:

| name | maps | for |
|---|---|---|
| `channel` | one channel through the palette (scale, offset, clamp or wrap) | everything |
| `two_channel` | u→palette position, v→brightness (or hue/value) | RD systems |
| `age` | time since the cell last changed → palette | CA, growth fronts, DLA growth rings |
| `scale_mix` | McCabe: per-scale colour blended into `.z` during the update, read here | McCabe |
| `hillshade` | gradient-lit channel (the escape relief look, computed here directly) | McCabe, Kobayashi, Cahn–Hilliard |
| `occupancy` | coverage = occupancy, rgb by age or by potential | DBM, DLA, percolation |

The palette is the shared `Rgba8Unorm` LUT (`palette_view`), bound exactly as escape binds it. Linear tonemap is the mode default, set on entry the way escape does it (exposure/gamma reset), because the same Log-calibrated flame values would render a unit-range field black.

Relief shading (`EscapeShading`) is *not* reused as code: its height field is escape-specific state. The `hillshade` colouring reproduces the look from the field gradient in one pass, which is cheaper than escape's blur-based relief and adequate for smooth fields. If the banded look is wanted later, the escape relief pass is the precedent to port.

---

## 7. The grid is separate from the viewport, with an optional binding

**Decided 2026-09-01 (with the user):** the simulation grid is its own quantity, distinct from the display size, so that colouring (at grid resolution, §4.5) and scaling to the output (the resolve stage, §4.6) are separate concerns. Binding the grid to the viewport is an option, not the model.

`sim.grid` is an enum in the config:

| variant | grid size | when the viewport resizes | export at W×H |
|---|---|---|---|
| `Fixed { width, height }` (presets 256…4096, free entry) | as set | nothing happens to the simulation; the resolve rescales | the *same* grid, run `steps` from the seed, resolved to W×H — the picture on screen at a different scale |
| `Viewport { scale }` (`scale` = grid cells per display pixel; default 1.0; 0.5 = half-resolution grid upscaled 2×; 2.0 = a 2× supersampled grid box-filtered down) | `round(display × scale)` | the field is **resampled** into the new grid (bilinear for float channels, nearest for integer/state channels, agent positions scaled); the run continues; the panel marks the run "resampled" | grid = `round(W×H × scale)`, a **fresh** run of `steps` from the seed — a different picture from the screen when the export size differs, and the export panel says so |

- Default for a new config: `Viewport { scale: 1.0 }` — fills the window with detail the way escape does, which is what a user expects on first entry. Model presets that need a specific cell count (Kobayashi at 300², Wolfram rows, the sandpile) pin a `Fixed` grid.
- Switching variants, or changing a `Fixed` size, re-seeds. A `scale` change in bound mode resamples like a resize.
- Resampling is a one-off pass (the warp stage's bilinear resampler, §4.1, at a fixed grid-to-grid affine) and is the only place the state is ever interpolated; it is never part of a step.
- Reproducibility: `Fixed` runs are reproducible from the config on the same GPU regardless of window size. `Viewport` runs are reproducible for a given output size — `steps`, `seed`, `scale` and the size together determine the picture, and the PNG metadata records the grid actually used.
- The viewport shows the grid scaled to fit through the resolve filters; in `Fixed` mode the aspect is the grid's, letterboxed.
- `supersample` in the escape sense does not apply. Antialiasing of a discrete CA is the resolve's job: `Viewport { scale: 2.0 }` *is* 2× supersampling.

This keeps what made the config-only design shareable (a `Fixed` config fully determines the run) while giving the fill-the-window behaviour the escape mode has, and it makes the resolve stage a first-class control rather than a hidden fit-to-window.

---

## 8. Determinism and reproducibility

- Seeded init: noise via a hash of `(x, y, seed)` (PCG mix), shapes via analytic masks. Same seed, same grid ⇒ same initial field on every platform.
- Stencil passes are deterministic per platform (fixed evaluation order per texel, no atomics).
- Agent deposit is integer-exact, so it is deterministic per platform despite the race.
- Across GPUs, f32 arithmetic differs in the last bits and chaotic dynamics amplify it. The seed doc's conclusion stands: **do not chase cross-GPU bit-identity**. A shared render that must match is shared as its PNG. The config carries `seed`, `steps`, grid size and every parameter, which is enough to reproduce on the same hardware.
- Visual regression baselines therefore run on the fixed test machine at small grids and fixed step counts, with exact pixel-hash compare (same machine, same driver, deterministic passes). If Metal fast-math ever makes a model class-divergent, that model's baseline moves to the `solid-*` tolerance mechanism.

---

## 9. What the prototypes measured (CPU, NumPy, 2026-09-01)

Both scripts live in the session scratchpad; they are validation of the *rules*, not of GPU cost, and should be committed under `scripts/sim_prototypes/` when implementation starts.

**McCabe multi-scale Turing**, 256², five scales (activator/inhibitor radii 1/2 … 16/32, increments 0.05 … 0.01), disc averages by FFT, rule exactly as the paper states it (least-variation scale fires; renormalise to [−1, 1]; periodic):

- The characteristic texture is visible at step 20, organised by step 100, and keeps refining through 300 (the paper says well defined at 10,000). **Hundreds of steps** is the interactive regime; thousands for a finished still.
- 9 ms/step on the CPU with exact FFT discs; the 5-fold symmetric variant 101 ms/step (n× the taps, plus resampling). On the GPU with the pyramid the plain step is ~4 passes; the symmetric variant is the same passes with 5× the loads in one of them.

**Gray–Scott** (Sims' discrete scheme, `D_A = 1`, `D_B = 0.5`, `Δt = 1`, 3×3 Laplacian weights −1 / 0.2 / 0.05): see [simulation-catalog.md](simulation-catalog.md) §Gray–Scott for the settle table. Two findings that bind the kernel:

- **Clamp to [0, 1] every step.** Without it an overshoot below zero feeds `uv²` with the wrong sign and the field is NaN within a few thousand steps (measured on the multi-seed variant). The GPU kernel must clamp.
- **Seed size matters at the sparse classes.** A 12-px `B = 1` blob dies at Pearson's λ (mitosis); 24 px survives. The presets carry their own seed shapes.

---

## 10. Feasibility, measured (2026-09-03)

Phase 0's GPU microbenchmark, `src/sim_microbench.rs` (test-only;
`cargo test --release --lib sim_microbench -- --ignored --nocapture`).
It runs the shape every Tier-1 model shares — two `rgba32float`
textures ping-ponged by a compute pass gathering 3×3 and writing one
texel, with Gray–Scott's arithmetic and its mandatory clamp, so the
cost is a real model's and not an empty kernel the compiler folds
away. GTX 1660 SUPER, Vulkan, driver 581.57 — deliberately a modest
card, so these are close to a floor rather than a best case.

| grid | field MiB | ms/step | steps/s | steps per 16.7 ms | cells/ns |
|---|---|---|---|---|---|
| 256² | 2.0 | 0.0222 | 45,120 | 753 | 2.96 |
| 512² | 8.0 | 0.0769 | 13,011 | 217 | 3.41 |
| 1024² | 32.0 | 0.2844 | 3,516 | 59 | 3.69 |
| **1920×1080** | 63.3 | **0.4954** | 2,019 | **34** | 4.19 |
| **3840×2160** | 253.1 | **2.0389** | 490 | **8** | 4.07 |

**The phase-1 gate is met with room.** It asks for ≥ 4 steps per frame
at 1080p at 60 fps; the plain stencil gives 34 in a whole frame
budget, so it still clears the gate with 88% of the frame left for the
UI and the resolve. At 4K, 8 steps per frame.

**Throughput is flat at 3–4 cells/ns and scaling is linear in cell
count**, which is the signature of a bandwidth-bound kernel — as
expected for two 16-byte-per-texel surfaces and nine loads. It also
means the numbers above transfer to the other Tier-1 models: they
differ in arithmetic, and arithmetic is not what this costs.

**Stills at catalogue grids are effectively free.** Gray–Scott's
10,000 steps to a settled 256² picture is **0.22 s**. The design
worried about the wrong end of the range: a 256² still is instant, and
it is the viewport-bound 4K grid that needs the driver's care.

### Submission batching is worth nothing, and that simplifies §5

The same 256 steps at 1080p, submitted in different batch sizes:

| steps per submit | ms/step |
|---|---|
| 1 | 0.5221 |
| 8 | 0.5134 |
| 64 | 0.5146 |
| 256 | 0.5179 |

**0.8% across a 256× range**, which is inside the noise. Per-submit
overhead is invisible next to a 0.5 ms dispatch, so batching steps into
one command buffer buys nothing at 1080p and above. (The CPU still
enqueues without waiting in every case, so this prices *submission*,
not synchronisation — which is the right question for a driver that
submits K steps per frame and polls once.)

What that changes: K in §5 exists **only** for pacing and the watchdog,
never to amortise submits. So the driver may submit one step at a time
and re-read the clock between them, which is strictly better — it can
stop on a budget boundary instead of overshooting by a whole batch, and
the watchdog bound becomes a per-step check rather than a batch-size
calculation. At 4K's 2.04 ms/step a 2 s watchdog window is ~980 steps,
so a 10,000-step 4K export needs at least 11 submissions; with per-step
submission the cap is simply "poll every N ms".

### Still to measure

- The pyramid stage (McCabe) and kernel-LUT gathers (Lenia,
  SmoothLife) — the estimates below stand until phase 3.
- The agent stage (Physarum, DLA) — phase 4.

---

## 10a. Original estimates, kept for comparison

**The pre-measurement expectations were:**

The one measurement the seed doc asked for, restated with the pyramid:

- 1080p step cost, McCabe, 5 scales: pyramid build (≈ 1/3 pass) + update with 5 × 8 taps (≈ 1 pass of 40 loads/texel) — expected well under 2 ms on a mid-range GPU, i.e. hundreds of steps per second. Measure with the timestamp pacer on the first build before tuning anything.
- 4K: 4× that. Still tens of steps per second at a 4K grid; a `Fixed` grid or `Viewport { scale: 0.5 }` (§7) is the lever if that is too slow for interactivity.
- Physarum with 1M agents: agent pass ≈ 1M threads × ~10 loads; deposit resolve = 1 pass. Comparable to a flame frame.

If the McCabe step measures over ~8 ms at 1080p, the fallbacks are (in order): fewer pyramid taps (bilinear at the nearest level only), `rgba16float` for the pyramid alone (it is filterable everywhere and its precision loss is confined to blurred averages), and only then the FFT.

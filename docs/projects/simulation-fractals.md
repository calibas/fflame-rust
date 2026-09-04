# Simulation Mode — master plan

**Status:** Planning, 2026-09-01. **No code has been written**; the
only artefacts are these documents and two NumPy prototypes under
`output/sim_proto/` (gitignored). Branch context: `escape-time` at
`331160e8`.

This is the plan of record for the third fractal family — the
neighbour-coupled simulations (reaction–diffusion, cellular automata,
aggregation and growth models) proposed in
[field-type-fractals.md](../archive/escape-time/field-type-fractals.md). It is split across
four documents so each can be read for one purpose:

| document | answers |
|---|---|
| **this file** | what is being built, why these decisions, in what order, with what risks and open questions |
| [simulation-pipeline.md](simulation-pipeline.md) | how the GPU renders it: state, stages, driver, colouring, determinism, feasibility numbers |
| [simulation-integration.md](simulation-integration.md) | every file the mode touches, mapped from where `RenderMode::Escape` reaches today |
| [simulation-catalog.md](simulation-catalog.md) | every model: rule as the source states it, discretisation, parameters, presets, sources with verification labels |

---

## 1. Analysis of the seed document

The seed document is right on the architectural point it makes: these
patterns are **whole-image feedback**. An escape-time formula computes
each pixel from its own coordinates and never reads a neighbour; a
Turing pattern, a BZ spiral or a DLA cluster exists *only* as a
relationship between neighbouring cells across time. They cannot be
escape formulas, and forcing them into that registry would produce a
one-off hack per pattern. A third render mode with its own registry,
renderer and panel is the correct shape, and the seed document's
"one shader per rule" instinct is the right granularity — it is the
same instinct that gave the variation and formula registries.

Points where planning changed or corrected the seed document:

1. **The Reusser quotation is not verbatim.** The seed document
   quotes Jonathan Reusser describing McCabe's algorithm; the text
   could not be matched to his page. What his implementation does that
   matters here: the multi-scale averages are computed with analytic
   kernels **in the frequency domain inside an FFT pipeline**. That is
   the standard high-quality route and it is not free on the GPU. The
   plan ships a mip-pyramid approximation first and parks the FFT
   (pipeline §3.4); the catalogue flags the box-vs-disc comparison as a
   phase-3 check.
2. **McCabe's paper gives no radius table and no colouring.** The
   radii ladders and the "colour by winning scale" look are from later
   implementations (Softology and others). The catalogue keeps that
   distinction so presets are not attributed to the paper.
3. **Kobayashi's phase-field dendrite is a dense-stencil PDE**, not a
   different category from the reaction–diffusion models; it lands in
   the same `update` stage with two passes. The seed document's
   "crystal growth" bucket splits into three mechanisms: PDE
   (Kobayashi), mass-transfer CA (Gravner–Griffeath), and aggregation
   (DLA, DBM) — each has a different cost profile and phase.
4. **Walkers are the chaos game.** DLA, Physarum and every other
   agent model is "many independent particles depositing into a
   grid" — precisely the atomic-histogram mechanism the flame renderer
   already has (`accumulate_samples.wgsl` and the u32 histogram). The
   agent stage in pipeline §4.4 is that mechanism with a field texture
   folded in; it is the least novel part of the pipeline.
5. **The name "Field" collides with escape-time vocabulary and
   inverts its meaning.** See §2.
6. **The grid is a quantity of its own, separate from the viewport**,
   unlike escape (which renders at the viewport size). A simulation's
   behaviour depends on its cell count — Gray–Scott at 256² and 2048²
   are different pictures — so colouring at grid resolution and
   scaling to the output are separate stages, and the grid is either
   fixed in the config or bound to the viewport by an explicit option
   (decided 2026-09-01; pipeline §7).

Everything else in the seed document — the model list, the "third
mode" framing, the wish for a custom workspace — carried through
unchanged.

---

## 2. Naming: `Simulation`

**Decided 2026-09-01, signed off by the user.** The seed document
calls the mode "Field". Inside the escape-time
engine that word already has a precise and opposite meaning:
`src/escape/fields.rs` is escape's **mode B, "field evaluation"** —
`FieldDef`, `FIELDS`, `FIELD_COLORINGS`, `FieldState`,
`assemble_field`, `get_field`, the colouring wire names `field_value`,
`field_diverging`, `field_hillshade`, and the `ShadingField` enum. The
docs define mode B as evaluating a function of the pixel's own
coordinates that *never reads a neighbour*. Naming the neighbour-only
mode "Field" would put two `fields.rs` modules, two `FieldDef`s and two
"field colourings" in one codebase with reversed meanings.

Three names had zero hits across `src/`, `docs/`, `locales/` and
`assets/`: **`Simulation`**, `Automata`, `Reaction`. `Automata` and
`Reaction` each exclude half the catalogue (DLA is neither; Gray–Scott
is not an automaton). **`Simulation`** covers all 27 models, reads
naturally in a menu ("Fractal Viewport · Simulation"), and is what the
literature calls every one of them. It is used throughout the four
documents:

| surface | name |
|---|---|
| enum / wire | `RenderMode::Simulation` / `"simulation"` |
| module | `src/sim/` (short, like `src/escape/`) |
| config | `FractalConfig.sim: SimConfig`, `ConfigPath::Sim*`, string keys `Sim.*` |
| UI | `PanelType::Simulation`, `WorkspaceLayout::Simulation`, panel title "Simulation" |
| feature flag | `engine-sim` |
| script | global `sim`, type `Sim` |
| tests | `tests/visual/configs/sim/` |

The name was the one decision that had to be signed off before any
identifier was typed, because it is the one that is expensive to
change afterwards. It is settled.

---

## 3. Decision record

Each decision names the alternative it rejected. Details and
measurements are in the pipeline document.

- **D1 — A third `RenderMode`, not a sub-mode of Escape.** Escape's
  renderer is "one dispatch, every pixel independent"; a simulation is
  "many dispatches, every pixel dependent". Sharing the enum arm would
  force every escape branch site (25 of them, integration §1) to
  sub-branch. Cost: every exhaustive match gains an arm — which is the
  safety net, not the price.
- **D2 — Mirror escape's registry pattern exactly.** `ModelDef` /
  `SimColoringDef` statics with inline WGSL, append-only registration,
  feature flags, a marker-splicing assembler, naga validation of every
  model × colouring in tests. The team already knows this shape; it
  exports to the API corpus the same way; the `choices` dropdowns and
  `param_control` come for free.
- **D3 — Two Rgba32Float field textures, ping-pong.** Read-write
  storage on `rgba32float` is rejected by wgpu; two textures with
  write-only storage on the destination is the only portable shape.
  Four channels per texel covers every model in the catalogue (max
  three state channels + one scratch). Integer states are bitcast.
- **D4 — The grid is separate from the viewport; binding is an
  option** (decided with the user, 2026-09-01). `sim.grid` is
  `Fixed { width, height }` or `Viewport { scale }`; colouring runs at
  grid resolution and the resolve stage scales to the output with
  chosen up/down filters. Bound grids resample on resize and
  re-simulate at the output size on export; fixed grids are
  reproducible from the config at any output size. Default
  `Viewport { scale: 1.0 }`. Rejected: viewport-only (no reproducible
  grid) and config-only (no fill-the-window behaviour) — the enum is
  both (§1 point 6; pipeline §7).
- **D5 — The renderer is stateful; export is "exactly N steps from
  the seed".** Interactive use runs, pauses and steps; a still is
  reproducible because `steps` and `seed` are in the config and the
  PNG metadata. Video export keeps one renderer alive and advances it
  per frame (integration §6). Rejected: re-running from the seed every
  frame (quadratic; and it makes "never stills" models impossible to
  export as video).
- **D5b — The timeline animates a cumulative STEP COUNT, not a rate**
  (decided 2026-09-04, prompted by asking whether the progression
  itself can be animated — it can, and this is how). A `Sim.StepCount`
  track makes the state at time *t* equal `round(track(t))` steps from
  the seed, so a frame stays a function of its time the way every other
  animatable quantity is. Advancing `steps_per_frame` per rendered
  frame instead would have made the simulation **frame-rate
  dependent**: the same project at 30 and 60 fps would differ at the
  same timestamp, and in-app playback (which advances by wall-clock
  delta) would diverge from export (which advances by `frame / fps`).
  Easing the track gives slow-in/slow-out on the simulation itself and
  a hold gives a freeze-frame that keeps animating colour; a decrease
  costs a reseed and re-run, the honest price of a non-invertible rule.
  `steps_per_frame` remains the interactive Run speed only, and is not
  an animation target. Integration §6.
- **D6 — Linear tonemap on entry, the coverage-alpha output
  convention.** The tonemap shader interprets alpha as hit count;
  simulations write alpha = 1 for covered cells and rgb in [0, 1] so
  the existing tonemap/effects/export tail works unchanged (the same
  contract escape adopted). Levels stay hidden (density-calibrated).
  A per-model auto-scale (running min/max) is a colouring option, not a
  tonemap mode.
- **D7 — Agents deposit through a u32 atomic buffer** resolved into
  the field by the update pass. Rejected: float atomics (absent on
  WebGPU, non-deterministic where present) and per-agent texture
  writes (racy).
- **D8 — No FFT in v1.** Kernel LUT gathers up to R ≈ 32 (Lenia,
  SmoothLife); the mip pyramid for box multi-scale averages (McCabe).
  FFT is parked with a defined trigger: McCabe ladders above 64 px or
  Lenia R > 32.
- **D9 — Determinism is per-GPU exact.** Every stochastic kernel uses
  the PCG in `rng.wgsl` keyed by (seed, cell/agent, step); integer
  deposits are order-independent; visual baselines are exact hashes on
  one machine and class-compared across drivers, like the variation
  probe.
- **D10 — Feature flag `engine-sim`**, default on, gating the module,
  the render branch, the script handle and the video-export branch, so
  the gallery WASM crates opt in per engine.
- **D11 — Online save refuses the mode client-side until the server's
  enum knows it.** Escape's precedent (2026-08-29): the Postgres
  `render_mode` enum is the API repository's to change; the client
  keeps `FetchError::Unsupported` in front of it until then.
  Confirmed for phase 1 by the user, 2026-09-01.
- **D12 — Fix, do not copy, escape's integration gaps** (integration
  doc, every row marked **gap**): CLI export routing by histogram size,
  layout not switching on file load, the 2D/3D menu bool, missing
  `history.param` keys, stale "refuses escape" prose, no escape
  mention in ARCHITECTURE.md.

---

## 4. What was measured

Two rules were run in NumPy to validate the discretisations the plan
ships and to size the step budgets. Images are in `output/sim_proto/`
(`gs_sheet.png`, `mccabe_sheet.png`), scripts alongside them.

**Gray–Scott** (Sims' scheme: D_A = 1, D_B = 0.5, dt = 1, 3×3 weights
−1 / 0.2 / 0.05, clamp to [0, 1]; 256², six 24-px seeds, periodic):

| pair | at 500 | at 5000 | at 10000 |
|---|---|---|---|
| mitosis 0.0367 / 0.0649 | dividing spots | lattice growing | hexagonal spot field |
| coral 0.0545 / 0.062 | branching | labyrinth | labyrinth, still growing |
| 0.030 / 0.057 | stripes + holes | maze | maze, filled |
| worms 0.046 / 0.065 | worm tips | worms | worms, filled |
| 0.010 / 0.041 | blobs | blank | blank — died; needs noise seeding |

Two findings changed the plan: the **clamp** is mandatory (NaN within
a few thousand steps without it at some pairs), and **seed size**
decides survival (12-px blobs died at mitosis, 24-px survived
everywhere). The settle metric (mean |Δv| < 10⁻⁴ for 200 steps) fires
long before the picture is finished — growth patterns keep advancing
into empty field at a fixed cells-per-step speed — so the driver's
budget is sized from the images: **thousands of steps for a still,
proportional to grid size; tens per frame interactively.**

**McCabe** (paper rule, five scales (1,2) … (16,32), FFT disc
averages, uniform-noise seed, 256²): the texture is present by step 20
and developed by 100; 5-fold symmetry gives the paper's rosettes;
9.0 ms/step plain, 101 ms/step symmetric in NumPy (the symmetry cost
is Python's rotation, not intrinsic). It never stills.

Codebase facts that constrained the pipeline were checked directly
(pipeline §1): read-write `rgba32float` storage is rejected; one
`Queue::write_buffer` per submission is visible, so per-step uniforms
use a ring with dynamic offsets; `FLOAT32_FILTERABLE` is optional, so
the pyramid samples manually; compute workgroups are (8, 8, 1)
everywhere; the tonemap reads alpha as a hit count; `is_rendering`
excludes escape, so continuous redraw needs its own term.

---

## 5. Phases and gates

Each phase ends at a gate that must pass before the next starts. The
escape-time mode took about 120 commits from its plan document to
today; this mode is comparable in surface and larger in models, and
the phases are ordered so that a usable product exists from the end of
phase 1 onward.

### Phase 0 — measure (no product code) — DONE 2026-09-03

Every item ran, and the gate is met: a step-cost table with no entry
marked "estimate". The grid axis is the GPU microbenchmark; the model
axis is one NumPy prototype per Tier-1 model plus the sandpile, all
under `scripts/sim_prototypes/` with a README, every number recorded
against its model in the [catalogue](simulation-catalog.md) with the
date and the method.

**ms per step, the shared 3×3 `rgba32float` ping-pong** (GTX 1660
SUPER, Vulkan; bandwidth-bound at 3–4 cells/ns, so it carries across
the Tier-1 models, which differ only in arithmetic):

| 256² | 512² | 1024² | 1920×1080 | 3840×2160 |
|---|---|---|---|---|
| 0.022 | 0.077 | 0.284 | 0.495 | 2.04 |

**Steps to a still, or to a developed picture for models that never
still** (256² unless stated; "never" means the churn plateaus rather
than decays, and the number is where the picture stops changing
character):

| model | steps | note |
|---|---|---|
| Gray–Scott | thousands, ∝ grid | settle metric fires early; sized from images |
| FitzHugh–Nagumo (excitable) | never · developed ≈ 4,000 | spirals need a broken-wave seed; dt cap 0.75 on the spiral |
| Brusselator (spots) | **1,180** | dt cap 0.04 |
| Brusselator (oscillating) | never | |
| Schnakenberg (spots) | **4,900** | dt cap 0.02; steps ∝ D, ∝ λ² |
| Hodgepodge | never · developed ≈ 200 | |
| Cyclic CA 1/1/14 · 1/3/3 | never · ≈ 300 · ≈ 7 | per-model defaults |
| Spatial RPS | never · ≈ 27 | three species coexist |
| Ising T=1.5 · T_c · T=3.5 | ≈ 436 · ≈ 27 · ≈ 1 sweeps | T_c figure is a coarsening snapshot, not equilibrium |
| Wolfram ECA | = grid height | exact by construction |
| Packard snowflake | = radius (125) | exact; rule changes density, not timing |
| Eden | radius/p · 127 → 1,158 for p 1 → 0.05 | overestimates ~2× at small p |
| Ballistic deposition | ≈ 1.4–1.8 × grid height | 361 / 452 to fill 256 rows |
| Percolation labels, p_c | 645 (256²) · 1,409 (512²) · ≈ 3,250 (1024², fit) | **4× spread between samples**; needs `settle`, not a count |
| Abelian sandpile, 2²⁰ grains | **190,006 rounds** | ∼ N^0.978; ≈ 55 s of GPU dispatches |
| McCabe | never · developed ≈ 100 | pyramid stage, phase 3 |

Tiers 2–4 (Oregonator, Swift–Hohenberg, Cahn–Hilliard, Lenia,
SmoothLife, Kobayashi, the snowfake, DLA, DBM, Saffman–Taylor, invasion
percolation, Physarum) are outside this gate by design — each needs a
stage the skeleton does not have yet, and its budget is measured when
that stage exists.

**What the measurements changed** — the reason the phase exists:

- A FitzHugh–Nagumo Turing preset that produces a **flat field**
  (spatial sd 0.0000) is struck. The excitable preset is confirmed and
  needs its broken-wave seed as the default.
- Three dt caps corrected: Brusselator 0.04 (was an untested 0.02),
  Schnakenberg 0.02 (every rung now run), FHN 0.75 (a first probe on a
  resting field said 0.5).
- Percolation's budget was **3–5× low**, and the sample-to-sample
  spread (332 vs 1,232 at one size) means a fixed `steps` cannot serve
  it.
- The sandpile's cost, which the catalogue could not estimate, is
  rounds ∝ radius² — mass diffuses rather than propagates.
- Submission batching is worth **0.8%** (pipeline §10), so the driver
  may submit one step at a time and re-read the clock.
- Turing feature size costs steps **linearly in D, quadratically in
  wavelength**, exactly as the dt argument predicts. A first
  measurement said the opposite and is retracted in the catalogue —
  its settle criterion loosened with dt.

**Traps the shader's `settle` stage inherits**, each found by falling
into it: a step that changes nothing is not the end of a run; the
settle metric fires during nucleation before a pattern exists (require
an amplitude floor); the window is counted in steps and the onset is
what gets reported; where a model clamps, count cells at the rails
rather than watching for NaN; probe stability on the active
configuration; one stochastic sample gives neither an exponent nor a
budget.

### Phase 1 — skeleton (the whole vertical slice, one model)

- `RenderMode::Simulation` and every exhaustive match (integration §1).
- `SimConfig`, `ConfigPath::Sim*`, manager arms, `UpdateType::SimRerender`
  / `SimReseed`, undo coalescing, round-trip tests (integration §2).
- `src/sim/`: registry (`ModelDef`, `SimColoringDef`), assembler,
  `SimRenderer` with seed / update / color / resolve — **Gray–Scott**
  and the `channel` colouring only.
- `SimGrid` with both variants, the one-off resample pass for
  bound-grid resizes, and the resolve filters nearest / bilinear / box
  (bicubic and pyramid downscaling wait for phase 3). The export panel
  states which grid an export will use (pipeline §7).
- App: renderer field, frame branch, run/pause/step/reset, continuous
  redraw term, device-loss handling (integration §3).
- UI: `sim_panel.rs`, `PanelType::Simulation`, `WorkspaceLayout::Simulation`,
  menus, compact menu, hide lists in both directions, viewport overlay,
  Linear tonemap on entry through the shared `switch_render_mode`,
  Levels hint, i18n keys in `en.yml` (integration §5, §10).
- Headless: `render_sim` in `render.rs`, OOM precheck, CLI export
  routing fixed on mode, thumbnails with a step cap, PNG metadata
  (integration §4, §7).
- Tests: repro test (two runs byte-identical; export at N equals N
  single steps; OOM refusal), config tests, contract regenerated with
  `len() == 4`, `tests/visual/configs/sim/gray-scott-*.fflame` with
  exact-hash baselines, `run_tests.py` choices and metadata exemption.
- Fix the six escape gaps D12 lists while the same files are open.

**Gate: MET 2026-09-04** (GTX 1660 SUPER, Vulkan;
`cargo test --release --lib sim::app_repro_test::phase1 -- --ignored
--nocapture --test-threads=1`).

| requirement | measured |
|---|---|
| 1080p, ≥ 4 steps/frame, ≥ 60 fps | **1.38 ms/frame at 4 steps — 723 fps** |
| — at 8 / 16 steps per frame | 2.44 ms (409 fps) / 4.57 ms (219 fps) |
| 4K export, 10,000 steps, no watchdog reset | **12.6 s, 1.26 ms/step**, field finite and patterned |
| byte-identical repeat runs | asserted, plus its converse (a different seed must differ) |

Twelve times the interactive headroom the gate asks for, and the 4K
step is faster than phase 0's bare-stencil estimate (1.26 against 2.04
ms) because the shipped kernel is one pass over a smaller working set
than the microbenchmark's.

**Measure GPU timings with `--test-threads=1`.** cargo runs tests in
parallel; sharing the device with the 13-second 4K gate reported the
interactive frame at 72.64 ms instead of 1.38 — a 50× error that reads
exactly like a real regression, and one that cost a round of
investigation before the contention was measured rather than assumed.

### Phase 2 — Tier-1 breadth

FitzHugh–Nagumo, Brusselator, Schnakenberg, hodgepodge, cyclic CA,
spatial RPS, Wolfram row mode, Packard snowflake (hex addressing),
Eden, ballistic deposition, Ising (checkerboard), percolation (label
propagation + settle reduction). Infrastructure they bring: integer
channels via bitcast, per-cell PCG, row-mode dispatch, hex offset-row
helper and resolve sampler, the settle reduction. Colourings
`two_channel`, `age`, `hillshade`. Init variants (noise, blobs, ring,
line, centre). Presets for every model — only pairs verified against
their source (catalogue labels).

**Gate:** every model × colouring naga-validates in the test suite;
CPU mirrors for Gray–Scott, cyclic CA and the sandpile rule agree
class-for-class with one GPU step; a visual baseline per model.

### Phase 3 — pyramid and large kernels

Mip pyramid stage with manual trilinear reads and symmetry in the
reads; McCabe with symmetry and the min/max renormalisation; Lenia and
SmoothLife via kernel LUT gathers; Swift–Hohenberg, Cahn–Hilliard and
Kobayashi as two-pass stencils with dt clamped at the stability bound;
Oregonator sub-stepping; `scale_mix` colouring. The box-vs-disc check
for McCabe against the NumPy images.

**Gate:** McCabe 1080p at the interactive budget; Lenia R = 13 at
512² ≥ 60 steps/s; baselines.

### Phase 4 — agents

Agent buffer, deposit buffer, resolve-into-field; Physarum and DLA;
`occupancy` colouring; agent state carried through video export.

**Gate:** deposit-order determinism test (two runs identical with
10⁶ agents); DLA cluster dimension ≈ 1.7 measured by box counting in a
test at 512².

### Phase 5 — growth and Laplacian models

DBM (parallel selection, then exact via scan), Saffman–Taylor as DBM +
curvature, invasion percolation (rising threshold), sandpile (bulk
toppling), Gravner–Griffeath snowfake — **after Part II has been
read**; its parameters are unverified today.

**Gate:** step budgets measured, not estimated; DBM η = 1 visually
matches the DLA of phase 4.

### Phase 6 — polish and reach

Warp stage (zoom/rotate/flow — the "living texture" look); animation
targets, video-export semantics, a shipped `sim_sweep.rhai`; the
script `sim` handle with SCRIPTING.md rows; the API enum (server
first, then drop the refusal), contract note to the API repository,
`openapi.json`; `es`/`ja`/`zh-CN` keys; display-only pan/zoom into
the grid; `wasm/sim` gallery module; docs (CLAUDE.md, ARCHITECTURE,
RENDERER, CONFIG, UI, EXPORT, WASM, RELEASE).

---

## 6. Risks

| risk | consequence | mitigation |
|---|---|---|
| API server enum lag (D11) | Save Online refuses the mode until the API repo ships the value | file the server change at phase-1 start; client refusal keeps it honest meanwhile |
| GPU watchdog (~2 s) on long exports | device loss mid-export | step batches sized by measured ms/step with a hard cap (pipeline §5); the phase-1 gate exercises a 4K 10k-step export |
| WASM limits | 128 MB storage bindings, no timestamp queries, main-thread blocking on long runs | deposit buffer sized under the limit; time by wall clock; chunk per animation frame as the escape WASM loop does |
| Explicit-scheme instability | NaN fields from a careless dt | dt sliders capped at the stability bound; every kernel clamps; the NaN idiom is the negated-comparison one (Metal fast-math) |
| Memory at 4K (~490 MB with pyramid + agents) | OOM on 4 GB cards | `allocation_error` precheck in `render()` and in the app before a bound-grid resize; a `Fixed` grid or a bound `scale` below 1 keeps a 4K output from forcing a 4K grid; at scale 1 the precheck refuses what the card cannot hold |
| Bound-grid export surprises | a picture tuned on screen re-simulates differently at export size | the export panel states the grid it will use; `Fixed` is one click away and keeps the on-screen picture |
| Cross-GPU divergence | baselines differ between machines | exact hashes per machine, class comparison across (the variation-probe discipline); no `x != x` |
| Unverified constants (catalogue `[verify]`) | a preset named after a paper that does not match it | nothing ships from memory; the catalogue lists exactly which papers still need reading (Part II, Kobayashi, Jones, Tyson–Fife, Gerhardt–Schuster) |
| "Never stills" models vs. the still-export contract | a user expects a settled image | `steps` is the contract and is shown in the panel; the model's tooltip says it is an animated subject |
| Vocabulary confusion with escape's field colourings | two things called "field" in the UI | §2; the escape panel keeps its names, the new mode never uses the word |
| Stateful export semantics | scrubbing backwards in the animation panel re-runs from the seed | documented in integration §6; the timeline shows a "re-simulating" state |

---

## 7. Test strategy

- **Unit:** `SimConfig` serde (default serialises to nothing; string
  keys; animation value conversion); registry invariants (append-only
  order, unique names, `choices` labelled across their range);
  assembler naga validation of every model × colouring; `shader_lint`
  over any new `shaders/*.wgsl`.
- **CPU mirrors:** one GPU step vs a Rust reference for three cheap
  rules, class-compared (zero / finite / NaN / inf) so no rounding
  difference can flake them.
- **GPU repro** (`src/sim/app_repro_test.rs`): the app's frame
  sequence on a fresh renderer; byte-identical repeat runs; export at
  N equals N single steps (batching invariance); OOM refusal at an
  absurd grid; agent determinism.
- **Visual regression:** `tests/visual/configs/sim/` at 256² grids
  with fixed `steps`, exact-hash baselines; the escape metadata
  exemption extended to the new mode.
- **Performance:** steps/s per model at 1080p recorded in the
  benchmark CSV next to the flame numbers.
- **Doc staleness:** the SCRIPTING.md test fails until every `sim.*`
  method is documented; the contract test fails until it is
  regenerated.

---

## 8. Open questions

Answers change the work; defaults are stated so nothing blocks.
Questions 1–3 were decided by the user on 2026-09-01.

1. **Name.** Decided: `Simulation` (§2).
2. **Grid.** Decided: the grid is separate from the viewport, with a
   `Viewport { scale }` option that binds them; colouring and
   up/down-scaling are separate stages (D4, pipeline §7).
3. **API timing.** Decided: phase 1 ships with the client-side
   refusal, exactly as escape did (D11).
4. **Hex lattices in v1?** Packard and the snowfake need them; DLA
   benefits. Default: yes, in phase 2 (one helper, one sampler).
5. **FFT.** Park (D8) or plan a phase? Default: park with the stated
   trigger.
6. **Which papers can you obtain?** Gravner–Griffeath Part II
   (Physica D 237), Kobayashi 1993 (Physica D 63), Jones 2010
   (Artificial Life 16), Tyson–Fife 1980, Gerhardt–Schuster 1989. Each
   unlocks a preset set the catalogue is holding back.
7. **Per-model default `steps`** (each model declares its own) or one
   global default? Default: per-model, declared on the `ModelDef`.
8. **Prototypes into `scripts/sim_prototypes/` now** (a repo change,
   but Python only) or at phase-0 start? Default: at phase-0 start;
   they are safe in `output/sim_proto/` until then.
9. **Tonemap.** Linear only on entry (as escape) or also expose the
   sim auto-scale as a first-class control on the Colors panel?
   Default: a colouring parameter, not a tonemap mode (D6).

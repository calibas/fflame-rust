# Simulation Mode — master plan

**Status:** Phases 0–4 shipped and phase 5 under way, 2026-09-05.
24 models and 6 colourings are in the registry, on branch
`simulation-mode`. The header below said "no code has been written"
until phase 5 wave 1, four phases after it stopped being true —
**§5 is where the per-phase status lives**, and each wave records what
it measured there rather than here. The plan's decisions (§3) are
unchanged; where a measurement contradicted one, the phase note says
so.

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

**Gate: MET 2026-09-04.** All twelve Tier-1 models ship, with four
colourings (`channel`, `two_channel`, `age`, `label`).

- Every model × colouring × boundary × resolve combination
  naga-validates: 13 × 4 × 4 × 8 in one test, which caught a WGSL
  reserved keyword (`target`) before it could reach a device.
- Correctness is checked against something falsifiable per model,
  because a baseline image cannot catch a rule that is wrong in a
  plausible-looking way:
  - Gray–Scott against a CPU mirror of one step (< 1e-6).
  - Ising against **Onsager's exact** nearest-neighbour correlation at
    T_c, 1/√2: measured 0.952 / 0.691 / 0.332 across the transition.
  - Wolfram rule 90 against binomials mod 2 — 2,079 of 2,079 cells.
  - Percolation against a CPU flood fill, both directions, over 122
    components.
  - Ballistic deposition's lateral sticking against interface width
    (2.71 correlated vs 7.30 uncorrelated).
- 30 visual baselines under `tests/visual/configs/sim/`; the full suite
  reads 268/268.

Infrastructure this phase actually needed turned out to be small: a
float-modulo helper for cyclic integer state, an offset-row hex
neighbourhood, a preset that can carry an initial field, and per-model
`dt` defaults. The settle reduction was **not** needed — percolation's
labels only decrease, so over-running is safe and a settle is an
optimisation rather than a correctness requirement.

**Review (2026-09-04), what it found and measured:**

- **The dt cap was not a cap.** `max_dt` was measured at each model's
  default diffusion rates, and phase 2's sliders reach 4–5× above
  them; explicit Euler's bound scales as 1/D. Measured at the slider
  maxima under the enforced cap, 128² after 200 steps: Brusselator and
  Schnakenberg **infinite in half their cells**, FitzHugh–Nagumo
  railed at ±3 by its clamp. The clamps in every kernel are what kept
  this invisible — a lattice of rails, not a NaN. The cap is now
  `ModelDef::max_dt_for(params)`: linear stability of the checkerboard
  mode, `dt · (λ_reaction + 1.6·D) < 2`, with `λ_reaction` inferred
  from the measured cap at the default D. A diffusion-only bound was
  tried first and FitzHugh–Nagumo still railed under it (the reaction
  term contributes at rest, `1 − v²`). Caps at the maxima: Gray–Scott
  1.2, FHN 0.257, Brusselator 0.019, Schnakenberg 0.0117 — all clean
  on the Nyquist-amplitude gate. The cap carries a 0.96 margin because
  AT the bound the mode is neutral, not damped: Gray–Scott at exactly
  2.00 held a 0.445-rms checkerboard in its [0,1] clamp. Enforced at
  the manager (both the dt arm and the model-param arm, since raising
  D pulls dt down), the panel slider and the renderer uniform.
- **Per-model step cost at 1080p** (`phase2_review_step_cost_per_model`,
  `--test-threads=1`): every model 0.24–0.38 ms/step except two.
  Hodgepodge cost 0.77 with its 3×3 loop and if/else chain; selects
  alone took it to 0.69, unrolling to **0.29** — the loop was the
  cost. Cyclic CA at R = 1 cost 0.44 through the same kind of loop;
  the default radius is now unrolled and reads **0.32**. At R = 5
  Moore it is **9.7 ms/step** — 121 reads a cell, memory-bound — and
  that is the shared-memory tile phase 3's large kernels need anyway,
  not a review item.
- **Which found a device-loss bug.** The probe first read R = 5 at
  4.7 ms/step, and 512 steps took exactly as long as 256: both runs
  were being cut off at ~2.3 s. That is Windows' GPU watchdog (TDR,
  2 s): `STEPS_PER_SUBMIT` was a fixed 256, one submit of 256 R = 5
  steps at 1080p is 2.5 s, the device is reset, the fence signals
  anyway, and the process aborts at teardown with
  `STATUS_STACK_BUFFER_OVERRUN`. Pinned between 192 steps (1.8 s,
  clean) and 224 (2.3 s, abort), and reproduced with the shipped
  binary: `export` of that config fails with "Parent device is lost".
  `run_steps` now sizes each submit from the measured cost of the
  previous one (a 250 ms budget, first submit small), which is what
  its comment had promised all along. Phase 3's kernels are slower
  still, so this would have surfaced there regardless.
- Nothing else moved: the rules were re-read against their sources
  (hodgepodge's `S/(A+B+1)+g`, Ising's checkerboard and Metropolis
  ratio, RPS's cycle, Packard's parity offsets, percolation's
  open-only propagation and root read), and all 30 sim baselines are
  unchanged by the two unrollings, as integer counts must be.

### Phase 3 — pyramid and large kernels

Mip pyramid stage with manual trilinear reads and symmetry in the
reads; McCabe with symmetry and the min/max renormalisation; Lenia and
SmoothLife via kernel LUT gathers; Swift–Hohenberg, Cahn–Hilliard and
Kobayashi as two-pass stencils with dt clamped at the stability bound;
Oregonator sub-stepping; `scale_mix` colouring. The box-vs-disc check
for McCabe against the NumPy images.

**Gate:** McCabe 1080p at the interactive budget; Lenia R = 13 at
512² ≥ 60 steps/s; baselines.

#### Wave 1 — the two-pass stage, and the two fourth-order PDEs

**Done 2026-09-05.** Swift–Hohenberg and Cahn–Hilliard ship;
`ModelDef::passes` is the infrastructure they needed.

- **Two passes, one WGSL.** A fourth-order operator cannot be one
  dispatch: the derivative of a derivative needs the *neighbours'*
  first-pass values, which do not exist until every cell is written.
  So `passes: 2` compiles the model's WGSL into two modules whose
  entry points call `sim_step` and `sim_step2`; the whole string goes
  into both, so a helper is written once. Both passes of step *i*
  carry the same `sim_step_index()` and share one uniform ring slot —
  a step is a step whatever it costs — and the double ping-pong lands
  the field back where it started, so nothing downstream knows how
  many passes a model has.
- **A per-model `dt_bound`.** The Sims-diffusion cap from the phase-2
  review does not describe a fourth-order operator, so `ModelDef` now
  carries an optional `fn(&Params) -> f32`. It composes with the same
  0.96 margin and the same three enforcement points.
- **They do NOT use the Sims Laplacian.** Its Fourier symbol is
  −0.3k², a Laplacian scaled by 0.3 — invisible in a second-order
  model, where the scale is absorbed into a free diffusion constant.
  Swift–Hohenberg *selects* the wavelength at which ∇² = −q₀², so the
  scale would move it by 1/√0.3 and make the documented λ = 2π/q₀
  wrong by 83%. Both use the standard 5-point kernel.
- **What the prototype refuted**, which is the point of running one
  (details and numbers in the catalogue, §6 and §7):
  - Swift–Hohenberg's `r = 0.2` makes no pattern at all — the drive
    has to be read relative to q₀⁴, and the shipped model exposes
    `wavelength` and a relative `drive` because of it.
  - Its `hexagons` preset makes a *uniform field*. Hexagons are
    subcritical and did not order from a noise seed in any of eight
    focused runs; `spots` ships instead, named for what was measured.
  - Cahn–Hilliard's dt bound was too loose by 50% **and failed
    slowly** — finite at 400 steps, infinite by 1,000 — so the first
    ladder called it stable. Ladders now run 4,000 steps.
- **Falsifiable per model**, as phase 2 established: a CPU mirror of
  both passes (3.7e-9 — it catches a ping-pong that lost a swap, which
  still looks like a PDE); Cahn–Hilliard's exact conservation of mean
  composition (3e-9 over 4,000 GPU steps while the field separates to
  sd 0.83); and Swift–Hohenberg's wavelength tracking its parameter,
  measured by zero crossings against the line-scan bias of an
  isotropic 2-D pattern.
- Six new visual baselines; the sim suite reads 36/36.

#### Wave 2 — the two models whose papers had to be read first

**Done 2026-09-05.** Oregonator and Kobayashi ship. Both catalogue
entries were written from memory and marked `[verify]`, Kobayashi's
saying outright that "the paper must be read before any of this
ships"; both papers were supplied and read before a line was written.
Full findings in the catalogue (§4 and §16).

- **Kobayashi verified almost entirely.** Every equation and every
  remembered constant holds — ε̄ = 0.01, τ = 0.0003, α = 0.9, γ = 10,
  dx = 0.03 on a 300² mesh. The paper added two things memory had
  lost: a noise term a·p(1−p)·χ that section 1 calls crucial to side
  branching, and θ₀ = π/2 for the ice dendrite. Since the paper fixes
  everything except K, δ, j and θ₀, those four are what the model
  exposes.
- **The plan's discretisation for it was wrong, and wrong in the way
  that hides.** §4.3's "one pass takes a gradient, the next its
  divergence" with central differences composes to a stencil that
  skips the immediate neighbour; the sublattices decouple and the
  field fills with a checkerboard while staying finite and inside
  [0, 1]. The prototype caught it — an `isfinite` ladder had called it
  stable at every dt. The shipped scheme stages the flux on cell
  faces, forward across the face and backward for the divergence,
  which composes to the compact Laplacian.
- **Oregonator's equations verified; its spirals refuted.** Tyson &
  Fife eq. (17) is confirmed verbatim, but the paper is analytic and
  carries no numeric set for a 2-D run, so ε, q and f were measured.
  The remembered spirals did not appear at any (ε, f) tried — a broken
  front retracts and heals into a closed loop — and the paper's own
  subject, target patterns, needs a pacemaker heterogeneity that the
  model has no channel for. What ships is what was measured: one
  excitation wave per seed, travelling at constant speed.
- **Falsifiable per model.** Kobayashi's symmetry is pinned by the
  angular harmonics of the crystal's reach (dominant harmonic 4 at
  11.46 vs 0.04 for k = 6; 6 at 9.66 vs 4.21 for k = 4) and by a
  Nyquist-amplitude check that would fail on the discretisation above.
  The Oregonator's front radius is measured at three times — 20.8,
  38.3, 56.1 cells, increment ratio 1.018 — which separates a
  travelling wave from diffusion (0.41).
- Four new visual baselines; 17 models.

#### Wave 3 — the large-kernel gathers, and the phase's gate

**Done 2026-09-05.** Lenia and SmoothLife ship, and the gate is met
with room to spare.

- **The kernel LUT.** `ModelDef::kernel` builds a `(2R+1)²` weight
  table on the CPU; the renderer uploads it beside the parameters and
  the step shader gathers against it. A model that needs two kernels
  (SmoothLife's disc and annulus) appends the second block after the
  first and offsets into it, which keeps each gather's reads
  contiguous. The buffer is sized once for the largest kernel allowed
  (R = 32, two blocks, 34 KB) so it never resizes, and the binding is
  always present — only the models that gather declare it in WGSL.
- **GATE MET.** Lenia R = 13 at 512² is 729 taps a cell, 1.91e8 taps a
  step, measured at **3.36 ms/step = 298 steps/s** against the
  required 60. The direct gather is enough and the **shared-memory
  tile held in reserve is not needed** — which also settles the
  question the phase-2 review left open about range-5 cyclic CA.
- **A four-year-old bug in the periodic boundary.** SmoothLife's CPU
  mirror disagreed by 0.228 at the edges while the interior was
  bit-exact. The wrap read `((p % g) + g) % g` — correct arithmetic
  that measurably behaved like a bare `p % g` on the device, byte for
  byte. Subtracting the truncated quotient instead agrees with the
  mirror exactly. Offsets of ±1 are demonstrably unaffected (all 33
  periodic visual baselines are byte-identical across the change), so
  it needed a large kernel to become visible: SmoothLife's annulus
  carries its weight at the outer radius, while Lenia's ring has
  almost none there and its growth term saturates exactly where the
  gather is wrong. **Why the original form failed is not
  established**, and the code says so rather than guessing. The old
  guard was a test asserting the SOURCE TEXT contained that idiom —
  which is why it passed throughout.
- **Falsifiable per model**: both gathers are compared against a CPU
  mirror using the exact table the GPU was handed, so a transposed
  index, a wrong radius, a mis-offset second block or a broken wrap
  all fail — 6.0e-8 for Lenia and 6.6e-7 for SmoothLife.
- What is deliberately not shipped: Orbium (needs a `Pattern` init),
  Lenia's multi-ring kernels and its polynomial and rectangular cores
  (formulas still `[verify]`), and SmoothLife's discrete time form.
- Two new visual baselines; 19 models.

#### Wave 4 — the pyramid, the reduction, and McCabe

**Done 2026-09-05.** McCabe ships with `scale_mix`, and both phase 3
gates are met. Every Tier-1 and Tier-2 model the phase named now
ships except none — 20 models.

- **The box-vs-disc question is answered, against the plan.** A box
  pyramid's McCabe texture is visibly axis-aligned (its spectrum half
  as peaked as the disc reference's); the shipped pyramid is
  **Gaussian**, one 25-tap blur-and-decimate dispatch per level, and
  with its level mapping calibrated to `log2(0.55 r)` it reproduces
  the exact-disc reference's feature size to 0.1% and amplitude to
  1%. Recorded in `proto_mccabe_pyramid.py` and the catalogue (§10).
- **Two new stages behind two features.** `NeedsPyramid` builds the
  pyramid before every step (separate textures per level, seven above
  the field, each with its own size uniform so the shared boundary
  wrap applies at every scale); `NeedsMinMax` reduces the new field's
  range after every step into a 257-slot ring — 64 cells per
  workgroup in shared memory, then one atomic min and max on an
  integer-ordered encoding, so 1080p is ~32,000 atomics. The next step
  normalises by the previous slot, which is the reference's own
  dependency. The ring has one more slot than the largest batch so the
  slot a step reads is never among the ones its batch clears.
- **All of it is pinned by CPU mirrors**: each pyramid level (6e-8),
  the reduce (bit-exact), and the whole McCabe step from the GPU's own
  seed — 4,096 of 4,096 cells to 1.2e-7 with zero tie disagreements.
- **GATE MET: McCabe at 1080p is 5.25 ms/step (191 steps/s)** against
  the 8 ms fallback threshold, after hoisting a per-level size loop
  out of the bilinear reads (7.78 before).
- Three new visual baselines; 20 models, 5 colourings.

**Phase 3 is complete.** What it deliberately did not ship, all
recorded in the catalogue: Orbium and Lenia's multi-ring kernels and
alternative cores, SmoothLife's discrete time form, Oregonator
spirals and target patterns, McCabe's `variation_blur`, and the
`hillshade` colouring the plan listed for three models.

**Review (2026-09-05) of the six phase-3 commits, what it found and
measured:**

- **The seed pass read a stale kernel radius.** `seed()` wrote its
  uniform BEFORE the parameter arrays, and building the kernel is
  what sets `kernel_radius` — so Lenia's seed, which sizes its noise
  patches by that radius, used whatever the previous model had left
  (1 on a fresh renderer, i.e. per-cell noise, the very thing its own
  docs say the ring averages flat). Every export is a fresh renderer,
  so the shipped soup baseline was the wrong seed. Order swapped; the
  baseline regenerated; exactly one baseline moved.
- **The pyramid was allocated for every model.** A third of a field
  texture again — 11 MB at 1080p, 44 MB at 4K — for the nineteen
  models that never read it. Now allocated by `ensure_pyramid` only
  for a `NeedsPyramid` model and freed when the model changes away.
- **McCabe 5.25 → 4.41 ms/step at 1080p** (227 steps/s) by computing
  the level count and every level's size once per invocation instead
  of by loop in each of the twenty bilinear reads. The CPU mirror is
  unchanged at 1.2e-7.
- **The per-frame kernel rebuild is not worth caching**: measured at
  8 µs (Lenia R = 13) to 49 µs (R = 32) per build, twice a frame.
- **Nothing exercised the min/max ring's wrap.** The clearing write
  splits in two when a batch straddles slot 257, and a wrong split
  would not fail — the range would fall back to [−1, 1] and the
  picture would drift. The reduce test now runs 600 steps across two
  wraps and checks the last slot bit-exact.
- **A new registry invariant**: every `mparam(N)` and `cparam(N)` in
  a definition's WGSL must index a declared parameter. The buffer is
  padded, so an index past the end reads 0.0 silently. All 25
  definitions pass; the check was run by hand in this review and
  belongs in the suite.
- **A false claim in Kobayashi's docs**: "the presets pin a 300 × 300
  grid". A preset carries no grid. The grid sets the vessel's size,
  not the crystal's; corrected to say so.
- Re-read against their sources with nothing else moving: the
  Oregonator kernel against the prototype's step, the Swift–Hohenberg
  and Cahn–Hilliard bounds, SmoothLife's anti-alias band, and the
  Lenia core; and every CPU mirror still holds.

**Hodgepodge corrected (2026-09-05), a phase-2 model.** The same batch
of papers settled a `[verify]` flag that had been open since the model
shipped: the rule everybody quotes is not the one Gerhardt & Schuster
state. Theirs divides the ILL count by k₁ (not the infected), averages
over the INFECTED cells alone (not every cell), and divides by that
count (not A + B + 1) — three differences, each of which still renders
plausible BZ scrolls, which is exactly why the baseline could not
catch it. Both rules now ship behind a `variant` parameter with the
paper's as the default, pinned by a CPU mirror of each published form
(0 mismatches in 4,096 cells; the two differ in 3,563 of them). The
paper's rule runs faster and wants g = 25 where the circulated one
wants 70.

### Phase 4 — agents

Agent buffer, deposit buffer, resolve-into-field; Physarum and DLA;
`occupancy` colouring; agent state carried through video export.

**Gate:** deposit-order determinism test (two runs identical with
10⁶ agents); DLA cluster dimension ≈ 1.7 measured by box counting in a
test at 512².

#### Wave 1 — the agent stage, Physarum and DLA

**Done 2026-09-05.** Both of phase 4's gates are met, and phase 4's
models ship.

- **The agent stage.** `ModelDef::agents` declares a population: a
  storage buffer of 16-byte records that move themselves and deposit
  into a per-cell integer buffer, which the step pass folds into the
  field and clears. Two model-supplied shaders, `sim_agent_seed` and
  `sim_agent`, with `agent_deposit`, `agent_rand` and the claim
  helpers provided. The population is allocated to the count the
  parameters ask for — a function of the GRID, so a percentage means
  the same density at any size — and a change of count reseeds,
  because half a new population is not a state.
- **GATE MET: reproducible with a million agents.** 1,048,576 agents
  on a 2048² grid, 40 steps, two independent renderers: **0 of
  4,194,304 cells differ**. This is what the integer deposit is for —
  agents land in one cell in an order the hardware chooses, and
  `atomicAdd` on a u32 does not care about that order. The exclusion
  is resolved the same way, by an atomic MINIMUM over agent indices.
- **Jones' exclusion turned out to be load-bearing.** The catalogue's
  GPU sketch dropped it; measured both ways on the same seed, without
  it the population collapses onto a few thick arcs and with it the
  same parameters give the paper's polygonal network. So Physarum
  declares two agent passes. Every one of the paper's Table 1 values
  is confirmed.
- **GATE MET: DLA's box-counting dimension is 1.753** at 512² (DLA is
  ≈1.71), with 39,000 particles clear of the walls. Getting there
  needed a fix the plan did not anticipate: any sensible walker count
  saturates a small cluster's launch circle and freezes a solid disc,
  so the ACTIVE population now tracks the circle's circumference and
  `crowding` is exposed as the speed-against-fidelity knob. A second
  bug fell out of the sweep — a kill radius smaller than the launch
  radius killed every walker at birth.
- `occupancy` colouring; 5 new visual baselines; 22 models, 6
  colourings.

**Review (2026-09-05) of the phase-4 commit, what it found and
measured:**

- **The turn rule was not Jones'.** When both sensors beat the front,
  figure 3 turns at RANDOM whichever side is stronger; the shader
  turned toward the stronger side. The prototype that validated every
  parameter had it right, so the two disagreed, and a network still
  formed either way — which is why neither gate caught it. Fixed, the
  three Physarum baselines regenerated, and a full CPU mirror of a
  Physarum step (sense, turn, claim-by-minimum, move, deposit,
  diffuse, decay, with the shader's PCG mirrored so the random draws
  match) now compares agents and field to float precision. It would
  have failed on the old shader.
- **Walls were not walls.** The position wrapped periodically under
  every boundary while the deposit clamped, so under Clamp an agent
  that walked off one edge reappeared on the other. Jones: an
  unsuccessful move leaves the agent where it is with a new random
  heading, and a wall is an unsuccessful move. Each boundary body now
  declares `SIM_PERIODIC`, and a test checks no agent moves further
  than its step size in one step — an in-range check could not catch
  a wrap, since a wrapped position is in range.
  **That test failed on its first run, and the fix had two bugs of
  its own.** The one the test caught: a destination of x = −0.4 is
  inside cell 0 and passes the wall check, and the float wrap that
  followed put it at 63.6 — agents crossed the low edge and reappeared
  on the high one. The wrap is now periodic-only. The one found
  reading the code while chasing that: refusing the move in pass 2
  while still claiming the clamped edge cell in pass 1 leaks the
  claim, because only the owner's check releases one. Measured with
  the guard removed: **129 stale claims** on a 64² grid after 140
  steps, most of the edge, each cell closed for ever to any agent of
  higher index. Now a move that cannot happen is not claimed, the
  contract is written where `agent_claim` lives, and the test reads
  the claim buffer back and asserts it is empty (it does fire on the
  unguarded claim).
- **The agent seed ran before the reduce it reads.** DLA's launch
  radius comes from the seed field's range; the seed pass ran first
  and read the previous run's slot. Reordered. No baseline moved,
  because a fresh renderer's slot happens to give the same answer as
  a centred seed.
- **The deposit is diffused one step late, and that is now measured
  rather than assumed.** Jones deposits, then applies the 3×3 mean,
  then decays; the step pass takes the mean of the old trail and adds
  the raw deposit, so a fresh deposit is spread on the following step.
  Run both ways on the prototype from the same seed: sd 3.58 against
  3.67, lit fraction 28.4% against 28.7%, and the same polygonal
  network with slightly grainier filaments. Matching Jones exactly
  would need a second deposit buffer (a cell cannot read its
  neighbours' deposits while they are being cleared, without a race),
  and the difference does not justify it. Recorded in the model.
- **Cost, at 1080p**: Physarum 1.39 ms/step at the paper's 5%
  population (103,680 agents) and 2.72 at 15% (311,040); DLA
  0.42 ms/step at 4% (82,944 walkers, most of them dormant by
  design). All well inside the interactive budget.
- Noted, not changed: `occupancy` draws `.w`, which Physarum's step
  fills with the step's deposit and DLA's does not, so the catalogue's
  "vapour halo" for DLA is not there yet — it would need walkers to
  mark their presence each step, a second deposit channel.

### Phase 5 — growth and Laplacian models

DBM (parallel selection, then exact via scan), Saffman–Taylor as DBM +
curvature, invasion percolation (rising threshold), sandpile (bulk
toppling), Gravner–Griffeath snowfake — **after Part II has been
read**; its parameters are unverified today.

**Gate:** step budgets measured, not estimated; DBM η = 1 visually
matches the DLA of phase 4.

**Papers, as of 2026-09-05:** Part II, the DBM paper
(Niemeyer–Pietronero–Wiesmann) and Saffman–Taylor 1958 are all in
`output/pdf/`. Nothing in this phase is now blocked on a source.

**What the phase needs that does not exist yet**, found by reading the
code against these five models before starting:

1. ~~`ModelDef::passes` is capped at 1 or 2 — the snowfake has FOUR
   substeps.~~ **Not needed, found by reading Part II.** Two of the
   four substeps read no neighbour, so they fold into the two that do,
   and a CPU mirror keeping all four separate agrees with the two-pass
   shader exactly. The cap stays until something actually needs it.
2. There is no per-step repeat count. DBM relaxes K = 5–50 sweeps per
   growth step and K is a slider, not a compile-time count.
3. There is no scan. The reduce does min/max only; exact selection
   needs a sum plus a descent to locate the chosen site. The
   approximation (`selection: parallel`) ships first, so this gates
   only the Tier-4 refinement.

Hex addressing (phase 2), the min/max reduce and the pyramid (phase 3)
cover everything else these models ask for.

#### Wave 1 — the two that needed nothing built

**Done 2026-09-05.** Sandpile and invasion percolation, both on
machinery already shipped, both held by a CPU mirror rather than by a
picture.

- **The sandpile is checked against an exact-integer mirror of the
  same parallel schedule**: 0 cells differ at 2¹², mass conserved to
  the grain, and the round count pinned from both sides (stable after
  `rounds`, over-full after `rounds − 1`). The prototype's counts are
  the shader's — 787 at 2¹², 12,837 at 2¹⁶ — so the presets' step
  counts are measurements. The Moore variant was measured rather than
  guessed and came out the opposite way round to the guess: denser,
  smaller and SOONER (4,652 rounds, 133 cells across, against 12,837
  and 189).
- **Invasion percolation's rising-threshold rule is checked against a
  flood fill** of the shader's own threshold field: 0 sites missing, 0
  extra, front finished at 1,640 of 2,000 steps. Measurement changed
  the design twice — a point seed turned out to be a lottery (three of
  five seeds gave a ~90-site cluster), so the presets inject from an
  edge as the paper does; and box counting at 256² does not resolve
  91/48, so the dimension is kept as a ramification check and the
  catalogue's D ≈ 1.89 is qualified rather than quoted.
- Two pictures were rendered and rejected rather than shipped: the
  spanning cluster (reads as noise at 50% occupancy) and the
  last-avalanche age field (nearly black). The odometer and the
  wrapped invasion contours took their places.
- 24 models, 5 new visual baselines, no existing baseline moved.

#### Wave 2 — Part II, and the snowfake

**Done 2026-09-05.** The Gravner–Griffeath snowfake ships, from the
paper rather than from the plan's memory of it, and the passes
generalisation the wave was scheduled around turned out to be
unnecessary.

- **Part II's rule is not Part III's**, and the catalogue had been
  carrying Part III's. Four fields rather than three, a one-cell seed
  rather than a hexagon, freezing that spends all the vapour rather
  than keeping κ of it, single constants where the entry had
  neighbour-count functions, and two parameters (α, θ — the knife-edge
  instability) with no Part III analogue at all. Reading the paper
  changed the model's shape, not just its numbers.
- **The four substeps fit two dispatches**, because freezing and
  melting read no neighbour. A CPU mirror that keeps all four separate
  agrees with the shader on every attachment over 400 steps, so the
  merge is exact rather than close.
- **The paper contradicts itself on α and θ**, and the fix came from
  measurement: under equation (3b) with the APPENDIX's values all
  three case studies reproduce the morphology their text describes,
  and under the same equation with section 6's values the first grows
  a featureless plate at every size tried, 40,000 steps on 1024²
  included. Two of the three case studies have text and table
  agreeing, so the table is the systematic source and it is what
  ships.
- **The paper's own conservation check is now a test.** Its drift is
  f32 and not the rule — about 1e-4 over 4,000 steps, in either
  direction — and the CPU mirror in the same precision drifts
  identically, which is what says so rather than assuming it.
- Four presets, each an unmodified row of the appendix; 4 new visual
  baselines at 512²; 25 models. A day spent on a stale binary: the
  first nine renders were Gray–Scott, which the CLI had been warning
  about in a log line nobody was reading.

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

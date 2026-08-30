# Escape-time: what is left before the branch is done

Agreed 2026-08-28, after the TDR-safety plan shipped and the NTT
project was measured and parked. Seven work items, each with what
the survey actually found rather than what it was assumed to be.

The survey's headline: **the engine is in far better shape than its
surface**. Rendering, deep zoom, orbit caching and crash-safety have
had weeks of work; scripting and the online API have had none at
all, and 20 of 23 formulas cannot zoom past the f32 direct path.

---

## 1. Accuracy against a reference — DONE 2026-08-28

Ran, results and the open item recorded in
[escape-time-fractals.md](escape-time-fractals.md) ("Formula accuracy
audit"), reproducible via `scripts/audit_escape_formulas.py`.
21 of 23 formulas verified against independent numpy oracles; one
real bug found and fixed (novaretti's pole sentinel overflowed f32 on
the next step, giving NaN on Vulkan and a plausible wrong number on
Metal); kaliset and novaretti's fine-grained fields remain open with
their excluded causes documented. Depth is confirmed as two tiers:
z9316+ for the three perturbing families, ~2^14 for the other twenty.

### The original scope, for reference

## 1. Accuracy against a reference, as deep as each formula goes

**What the survey found.** 23 formulas are registered. Exactly three
families can perturb — `mandelbrot` (p=2), `multibrot` (integer
powers 2..12) and `burning_ship` (6 fold variants). Everything else
renders through the DIRECT path, whose f32 pixel mapping runs out at
about zoom 14-16 (`PERTURB_MIN_ZOOM` sits at 14 for exactly that
reason). So "as deep as we can go" is not one number: it is ~2^14
for twenty formulas and ~2^9316-and-beyond for three.

That asymmetry is itself a finding worth publishing in the docs, and
it sets up item 2.

**Method** — the one that already worked. The Ducks correction was
settled by an f64 numpy transcription of the reference algorithm,
rendered as a field and compared against ours; it distinguished a
real formula error from an f32 artifact, and it found the beaded
region a contact sheet then confirmed. Repeat that per formula:

- a numpy ground truth per formula, from the ORIGINAL source
  (Fractint / Fractal Art Wiki / the paper), not from our WGSL;
- compare field structure at a shallow view (where f32 is not the
  limit) and at each formula's deepest usable zoom;
- record per formula: source citation, agreement, the depth at which
  our render stops being trustworthy, and any deliberate divergence.

Output: a table in `escape-time-fractals.md` (formula, reference,
verified depth, notes) and regression configs for anything that
moves. Expect this to find real bugs -- Ducks had two, and it was
the formula someone happened to look at closely.

## 2. Perturbation tiers — IN PROGRESS

Shipped since this plan was written, each verified against an
independent oracle and pinned by a visual config:

| tier | rungs | verified |
|---|---|---|
| Tricorn / multicorn | both | 0/768 vs direct, powers 2/3/5 |
| Phoenix | both | 0.00% vs an exact orbit at zoom 20 and 28 |
| Manowar | DEEP ONLY, by measurement | 1.6-2.1% vs an exact orbit at zoom 20-34; 18-27% on the scaled rung |
| Lambda | both | 0.23% / 0.26% vs an exact orbit at zoom 30 — see the note on why the CAP is part of that number |
| Feather | both | 0.00% / 0.00% vs an exact orbit at zoom 30, on the view that exposed the escape test's delta-blindness |

Plus 8 new `wgsl_derivative` blocks (tricorn, mcmullen, lambda,
cactus, exponential, trig, tetration, barnsley).

Still open, in the order the survey judged tractable:

- **Kaliset / Ducks** — blocked, with the reason measured rather than
  assumed: Kaliset needs a fixed-point reciprocal and about 37
  integer bits (|Z| reaches 1.8e11), Ducks needs delta forms for
  `ln` and `atan2`.
- **Lambda — DONE 2026-08-30.** The first tier whose PARAMETER
  MULTIPLIES, so its delta step reads the reference's own c and its
  parameter-plane term picks up `Z(1-Z)` rather than a bare `+ dc`.
  Both rungs. Details in
  [escape-time-fractals.md](escape-time-fractals.md).
- **Feather — DONE 2026-08-30** (same day, two commits). The gate
  described below lasted hours: the depth failure turned out to be the
  f32 escape test quantizing away sub-ulp deltas, fixed engine-wide by
  the delta-aware margin (`ref_r2`). 0.00% on both rungs at zoom 30 on
  the exact view that exposed it. The paragraph below records the
  original finding.

  **The original note — foundation shipped, tier NOT enabled.** The
  blocker for all three rational families turned out not to be "care
  near poles" but something more basic: the fixed-point layer had **no
  division at all** ("the core never divides", per its own header), so
  none of them could even build a reference orbit.

  That is now fixed — `FixedPoint::recip` (Newton, full width) and
  `FixedComplex::div`, both tested — and Feather has a working
  `MAP_FEATHER` reference branch, a `Feather(p)` tier and both delta
  rungs. Feather could go first because its denominator's real part is
  `1 + x^2`, so `|D| >= 1` always: the reciprocal is bounded by 1 and
  fits fixed point's ±128 range, where the pole-bearing families would
  overflow it. `recip` REFUSES out-of-range input rather than
  saturating, which is the guard that keeps Magnet and McMullen from
  being wired up carelessly.

  The tier is verified at zoom 15 (0.00% against an exact orbit) and
  20 (0.49%), and degrades past that — 26% at 25, and by 30 the delta
  stops iterating entirely. `perturb_tier` therefore declines to
  select it. The cause is NOT the algebra: an f64 simulation of the
  same recurrence, including the rebase and the scaled rung's `w = d/S`
  with every intermediate rounded to f32, matches the exact orbit at
  0.0% for zooms 10 through 30, and the generated WGSL matches that
  simulation line for line. The fault is in the shader environment,
  and it is unexplained.

- **McMullen, Magnet** — both need a reciprocal of a quantity that
  VANISHES at a pole, which is exactly what `FixedPoint::recip`
  refuses. Wiring them up needs a range strategy first (extend
  INT_BITS, carry an exponent, or detect near-pole references and
  decline the location) — a design question, not just code. The delta
  form itself is the same quotient one Feather uses.
- **Tetration / exponential / trig / Collatz** — transcendental
  deltas exist but the error analysis is its own study.
- **Newton / Nova** — convergent, not escaping; deep zoom there wants
  root-basin refinement, a different project.
- **Series approximation** — still absent, still unmeasured against
  BLA. Worth a Phase-0 measurement before building.

What the three shipped tiers taught, in case a fourth is attempted:
a two-term recurrence needs its history rebased WITH the current
delta and gated on the pair; the rebase target must be a reference
state that subtracts to zero, or the f32 subtraction quantises the
delta to ulp(c) (~80 pixels at zoom 22); and whether a tier needs the
deep rung is a question for MEASUREMENT, not for the zoom threshold —
Manowar needs it everywhere, Phoenix does not.

### The original scope, for reference

## 2. Mandelbrot tricks for the other formulas

**What the survey found.** Only 3 of 23 formulas define
`wgsl_derivative`. That single number explains most of the gap:
the derivative orbit is the raw material for BLA coefficients,
distance estimation AND Newton nucleus-finding. Adding derivatives
is the cheapest lever in this whole document.

Tractability, in the order worth attempting:

- **Tricorn / multicorn** — anti-holomorphic; the delta recurrence
  is the same as multibrot's with conjugates. Nearly free given the
  existing power tier.
- **Phoenix** — carries `z_prev`, so the delta state is a pair. A
  two-term linear recurrence; BLA generalises to 2x2 matrices.
- **Kaliset / Ducks family** — abs-folds plus a log; the Ship tier's
  diffabs machinery is the precedent, and the log needs a delta
  expansion (`log(Z+d) - log(Z) = log1p(d/Z)`), which is well
  conditioned away from |Z| ~ 0.
- **Lambda, Manowar, Feather, McMullen, Magnet** — polynomial or
  rational; perturbation works, division needs the usual care near
  poles.
- **Tetration / exponential / trig / Collatz** — transcendental
  deltas exist (`exp(Z+d) = exp(Z)exp(d)`), but the error analysis
  is its own study. Last.
- **Newton / Nova** — convergent, not escaping: deep zoom there
  wants root-basin refinement, a different project.

Also absent entirely, and worth its own line: **series
approximation** — the classic "skip the first N iterations for
every pixel at once". We have perturbation, rebasing and BLA but not
SA. Whether it still pays given BLA is an open question worth
measuring before building (the same discipline as Phase 0).

## 3. UI/UX pass — DONE 2026-08-29

All four of the rough edges the survey listed, plus the tests that
keep them honest:

- **Depth is stated.** `EscapeRenderer::usable_depth` is now public and
  the panel reports "unlimited (perturbed)" or "to about 2^14", and
  colours the label when the current zoom is PAST that. 17 of the 23
  formulas stop resolving there, and nothing in the panel said so --
  which leaves a user zooming into a flat wash with no way to tell a
  limitation from a bug. The hint follows the SETTINGS, not just the
  formula: biomorph and damping take a config off the perturbed path.
- **The coloring scale has an Auto button.** The scale/offset pair was
  the least guessable control in the panel (picking the Ducks
  showcase's 1.86/11.6 took a numpy probe). This is honestly labelled
  as a starting point rather than a measurement -- the real version
  reads the rendered value distribution back off the GPU, which does
  not exist yet.
- **A settled indicator.** Escape renders arrive in chunks, and a
  screenshot of an unsettled frame has been reported as a render bug.
  Progress rides a static, the same way reference-build progress
  already does, rather than threading a renderer handle through the UI
  to display one number.
- **Engine internals are collapsed.** The reference-orbit controls
  (period, detection, minibrot search) are 227 lines of machinery that
  change nothing about how the fractal looks; they now sit in a
  collapsed "Reference orbit (engine)" section.

Five tests, including two that LAY THE PANEL OUT headlessly for every
formula and every coloring. A panel that compiles can still panic at
layout on a duplicate widget id or an empty slider range, and neither
shows up in a build nor in the visual suite, which renders fractals
and not panels.

### The original scope, for reference

## 3. UI/UX pass

`src/ui/escape_panel.rs` is 692 lines grown feature by feature. It
has never had a pass for the workflow it now supports. Known rough
edges, to be confirmed by using it:

- the coloring-scale/offset relationship is unguessable (the Ducks
  showcase needed a numpy probe to pick 1.86/11.6 -- a user cannot
  do that), which argues for an auto-range button;
- reference-period, orbit-cache and supersample controls are engine
  internals sitting next to artistic ones;
- no indication of what a formula's usable depth is (item 1's table
  is the content for that);
- no "render settled" indicator, which is how a viewport screenshot
  of an unsettled frame got reported as a render bug.

## 4. Splitting the WASM modules — DONE 2026-08-29

Three shipping modules, from one source file and two Cargo features:

| module | engines | raw | gzip | vs `render` |
|---|---|---|---|---|
| `wasm/render` | both | 3.29 MB | **0.80 MB** | — |
| `wasm/flame` | flame only | 3.12 MB | **0.73 MB** | −8% |
| `wasm/escape` | escape only | 1.22 MB | **0.41 MB** | **−49%** |

### What the first attempt got wrong

The measurement before this one was of the FULL APPLICATION -- editor,
egui, the lot -- where every split looked marginal because a 4.2 MB
gzip floor swamped it. That was the wrong artifact. The gallery
modules carry no editor, so the same absolute savings are an order of
magnitude larger as a share: escape is 1.7% of the app and 10% of a
module; the variation catalog is 8% of the app and **half of a
module**.

### Why it was cheap

`ALL_VARIATIONS` is the only thing that reaches the 647 definitions,
so gating that ONE array drops the catalog and its 1.1 MB of inline
WGSL. `engine-flame` is a single `#[cfg]` on a single static.

`engine-escape` cost more (~15 sites) but far less than the 47 the
first attempt hit, because gating `mod app` and `mod ui` on the
existing `web-app` feature removed the editor from module builds
first. Those files are compiled even when nothing links them, and
being compiled is what made them the bulk of every engine seam.

The three modules SHARE ONE SOURCE FILE: `wasm/escape` and
`wasm/flame` both point `[lib] path` at `../render/src/lib.rs`. The
module's job is identical in all three -- config JSON in, RGBA out --
so duplicating 400 lines to vary a Cargo feature would guarantee
drift.

### What each module promises, and the tests that hold it

- `wasm/render` renders both, including an escape config.
- `wasm/flame` renders flames, carries the whole catalog, and
  REFUSES an escape config (`RenderError::EngineMissing`) rather than
  drawing the flame that config happens to also carry.
- `wasm/escape` renders escape configs and provably has no catalog:
  the test asserts the registry holds at most two variations, not
  that the file is small.

`the_catalog_matches_the_engine_feature` (in the main crate) is the
one that matters most. A module that forgets the feature does not
fail to build -- it builds a renderer whose catalog is `linear` alone,
and every config naming anything else renders WRONG rather than
erroring. That happened during this work: `wasm/script` inherited
`default-features = false` and its scripts started failing on
`spherical`, caught by CLI parity rather than by anything local.
Scripting now pins both engines, because a script that runs on the
desktop must run in the browser.

All four feature combinations pass the suite (852 / 681 / 546 / 453
tests). Test modules that assert on the shipped catalog -- flame XML
import, the shader dumps, the script corpus -- are gated to the
configuration they describe rather than being weakened.

### Not done, deliberately

**2D-only and 3D-only.** Worth about 25% each (`wgsl_2d` is 0.54 MB
of source against `wgsl_3d`'s 0.57 MB, so the catalog splits nearly in
half), but the seam is not a module boundary -- it is a FIELD on every
one of 647 defs, and every future variation would have to respect it.
Middling payoff, worst maintenance profile of the options.

**Animation.** A string scan says it is already absent from these
modules, so there is nothing to extract for size. A separate animation
module would be additive -- a new artifact driving a renderer -- and
belongs to its own plan.

### The original scope, for reference

## 4. Splitting the WASM builds three ways

**What the survey found.** Only 10 files outside `src/escape/`
mention escape at all, so the seam is real and shallow. The gallery
renderer (`wasm/render`) already renders escape configs, because it
calls the unified `render_with` path.

Shape: Cargo features `engine-flame` and `engine-escape`, both on by
default, with the app, config, render and UI touchpoints gated. Three
artifacts fall out (flames-only, escape-only, both). The value is
download size for the gallery and the embedded viewers; measure the
three sizes before committing to the maintenance cost, because a
feature-gated engine is a permanent tax on every future change that
crosses the seam.

## 5. Online API support

**What the survey found.** `ApiRenderMode` has two values, `2d` and
`3d`. `RenderMode::Escape` currently maps to `TwoD` (lossy), a
round-trip test pins that it does NOT round-trip, and
`api/mod.rs:166` refuses to save escape configs with an explicit
message. So the client side is honest about the gap and already
knows where to change.

Two halves, one of which is not ours:

- **Server**: needs an `escape` enum value and a place for the
  escape payload (formula, coloring, their params, and the exact
  decimal centre strings, which are the deep-zoom payload and must
  not be floats). Requires the API repository -- and per RELEASE.md
  section 3, a new enum value does not move the contract's shape
  fingerprint, so the API must be told directly rather than
  discovering it.
- **Client**: the mapping, the payload, thumbnails (escape
  thumbnails already work through `render_with`), and lifting the
  refusal once the server answers.

## 6. Scripting support — DONE 2026-08-29

An `escape` handle mirroring `flame`/`config`: formula and coloring by
name (validated against the registry, with the alternatives listed on
a typo), the catalog reachable via `formulas()`/`colorings()`/
`params()`, the view (`center`, `zoom`, `max_iter`, `bailout`,
`supersample`, `rotation`), the Julia plane, and both parameter maps.
Documented in [SCRIPTING.md](../main/SCRIPTING.md), which the existing
staleness test enforces, and covered by six tests.

Two decisions that are load-bearing rather than stylistic:

- **The centre takes STRINGS.** Zoom 60 needs about 20 significant
  digits and an f64 carries 15, so a float parameter would have capped
  every script near zoom 50 without saying so.
- **Touching `escape` switches the render mode** and resets tone
  mapping to Linear. A config carrying escape settings while rendering
  a flame is a silent no-op that reads as a bug in the script.

Ships `escape_deep.rhai` (five deep-capable formulas, each at a
verified location). It found a real bug on its first run: an escape
config has no transforms, and a zero-transform flame emitted
`array<f32, 0>` -- invalid WGSL -- so script-generated escape configs
could not export at all.

### The original scope, for reference

## 6. Scripting support

**What the survey found.** `src/script/api.rs` is 2,911 lines and
contains the word "escape" zero times. Scripts can build flames,
transforms, palettes and animations; they cannot touch an escape
config at all.

Work: an `Escape` handle mirroring the existing `Config`/`Flame`
handles (formula and coloring by name, their parameters, centre as
STRING to preserve deep-zoom precision, zoom, max_iter, julia,
supersample), plus generators worth shipping as built-ins. Note the
guard rail already in place: `every_script_api_name_is_documented`
fails the build if a registered name is missing from
`SCRIPTING.md`/`SCRIPTING-GUIDE.md`, so documentation is not
optional here, and `export_scripts_json` publishes built-ins to the
API.

## 7. Reference-build CPU performance (rescued from the shelved NTT doc)

The GPU measurement parked one approach; it did not make reference
builds fast. The f3-class target is ~8 minutes of arithmetic, cold.
What the queue already MEASURED, so nobody re-derives it:

- a reference iteration at 197 limbs is 48.9 us, and 44.6 us of that
  (**91%**) is the two `mul_trunc` calls;
- removing essentially every heap allocation from the step won
  **1.03x** -- the allocator was never the problem;
- **Karatsuba does not pay at 197 limbs**: the truncated high-window
  product already costs ~n^2/2 MACs, about what Karatsuba on the
  FULL product would cost at that size. It is worth revisiting only
  well above this limb count.

So the honest remaining lever is **SIMD**, and it aims squarely at
the 91%: 32-bit limbs through AVX2 `vpmuludq`, or 52-bit limbs
through AVX-512 IFMA, against the current scalar 64x64->128 chain
(measured at ~1.15 ns per limb MAC, i.e. 3-4 cycles -- about what
well-scheduled scalar `mulx`/`adc` achieves, so there is no scalar
win hiding here). A 2-4x cut would take the f3 cold build from eight
minutes to two or three.

Two caveats to design around before starting:

- the representation is the interface. `fixedpoint.rs`'s u64 limbs
  are load-bearing for the orbit store's format, the DF shadow's
  bit-exactness and every determinism guarantee in this project. A
  SIMD-friendly relimbing is a change to all of them, and the
  cold==warm identity test is the thing that must not break.
- **it must stay portable**: x86 SIMD needs a scalar fallback for
  ARM and wasm, and the two paths must agree BIT-EXACTLY or saved
  orbits stop being portable between machines -- the same
  requirement the NTT plan's Phase 1 identified, for the same
  reason.

Multithreading one multiply is plausible but fiddly: 44.6 us split
four ways needs a persistent spin-barrier pool, because dispatching
through a work queue ten million times would cost more than it
saves. Worth measuring only after SIMD, and probably only above
~1,000 limbs.

Perspective on priority: the orbit store already turns the second
visit to a location into seconds, and FFORBIT6 made those files ~100x
smaller. This item is about the FIRST visit only.

## Suggested order, and why

1. **Accuracy audit (1)** — it is foundational: optimizing or
   exposing a formula that renders the wrong set is wasted work, and
   the audit produces the depth table items 2 and 3 both need.
2. **Derivatives + the easy perturbation tiers (2)** — the biggest
   capability gain per unit of work, and the audit tells us which
   formulas are worth the effort.
3. **Scripting (6)** — self-contained, well-guarded by the existing
   staleness test, and it makes the whole formula catalog
   programmatically reachable.
4. **UI/UX (3)** — best done once the depth table and any new
   controls from 2 exist, so the panel is reorganized once.
5. **WASM split (4)** — measure the sizes first; the maintenance
   cost is permanent and the benefit is a number nobody has yet.
6. **API (5)** — gated on the server; prepare the client and hand
   the schema over.

Item **7 (CPU reference performance)** is deliberately outside that
sequence: it is independent of every other item, it is the only one
that shortens the eight-minute cold build, and its cost is dominated
by a representation change nobody should start casually. Schedule it
when a deep dive actually hurts, not because it is on the list.

## Derivative-based colorings no longer degrade silently — DONE 2026-08-30

`normal_map` was guarded when it shipped; `distance_estimate` was left
for a deliberate pass and has now had it. Both return a flat value
where no derivative is compiled, and the escape panel explains which
of the two causes applies (the formula defines none, or the view is on
the deep path, which iterates no derivative orbit whatever the formula
defines). Details in
[escape-time-fractals.md](escape-time-fractals.md).

## Known gap, found 2026-08-29: no recovery from DEVICE LOST

A TDR watchdog kill (observed when origami's first fold-table
implementation ran per-thread under 3x supersampling) leaves the app
hung: wgpu reports `DEVICE LOST (Unknown)` and nothing re-creates the
device, so the only way out is a restart. The trigger was fixed by
moving the table to the CPU, but any sufficiently expensive dispatch
on a slow GPU can still hit the watchdog, and the app should survive
it -- re-create the device and surfaces, rebuild the renderers, and
carry on with the current config. Non-trivial (every GPU resource is
owned somewhere) and low urgency now, but it is the difference between
"that render was too heavy" and "restart the app".

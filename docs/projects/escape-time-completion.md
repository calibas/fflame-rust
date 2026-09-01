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
| McMullen | both | 0.22% / 0.19% vs an exact orbit at zoom 30 (Julia plane; the parameter plane has no interior) |
| Magnet I and II | both | 0.00% vs an exact orbit at zoom 30, all four combinations; convergence tracked at 0.58/255 |

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

- **McMullen — DONE 2026-08-30, Julia mode only.** The range
  question is answered: `FixedPoint::recip_scaled` normalizes before
  inverting, so a SMALL denominator is fine and only an out-of-range
  QUOTIENT is refused — which for a pole-bearing map is the same event
  as "this orbit escaped". 0.22% / 0.19% against an exact orbit at
  zoom 30 on both rungs.

  Julia-only because of a finding about our own formula: McMullen
  seeds its parameter plane at `z_0 = c`, which is not a critical
  point of this map (`z = 0` is the POLE; the real critical points sit
  at `z^(n+m) = (m/n)c`). Measured, **0 of 4000 sampled parameters
  have a bounded orbit** — that plane has no interior to zoom into.
  The Sierpinski-carpet pictures the family is known for are Julia
  sets. Re-seeding the parameter plane at a proper critical point is a
  separate, visible formula change and is NOT done here.

- **Magnet — DONE 2026-08-30.** Both variants, both rungs, 0.00%
  against an exact orbit at zoom 30. `c` appears in numerator AND
  denominator, and the map is a squared quotient, so the delta form is
  the quotient one composed with `d(q^2) = 2q*dq + dq^2`.

  It also needed something none of the others did: the perturbed
  templates hardcoded `converged = false`, so a CONVERGENT family
  would have run every settling pixel to `max_iter`. Convergence
  detection is now spliced into both perturbed rungs, gated on the
  tier, and inert (byte-identical WGSL) for every non-convergent
  formula.

**Item 2's tractable four are complete**: Lambda, Feather, McMullen
and Magnet all perturb. What remains in item 2 is the harder set —
transcendentals (their own error analysis), Kaliset/Ducks (blocked for
measured reasons), Newton/Nova (root-basin refinement, a different
project), and the series-approximation-vs-BLA Phase-0 measurement.
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

**DONE 2026-08-31.** The follow-up prototyping settled every open
question above with measurements, and two levers shipped in
`fixedpoint.rs`. What was measured first (i5-10400F, 6C/12T):

- **native scalar really is saturated.** A u32 half-limb column
  rewrite ran at **0.5x** (LLVM will not auto-vectorize it, and it
  quadruples the multiply count), a Comba column scan at **0.91x**.
  The doc's ~1.15 ns/MAC stands.
- the x86-SIMD lever proposed above is dead on the development
  machine: Comet Lake has no AVX-512/IFMA, and AVX2 `vpmuludq` is the
  0.5x result above. Left unimplemented; revisit only as a
  capability-gated path on hardware that can measure it.
- **wasm was the real SIMD target.** On wasm every u128 product is a
  software `__multi3` libcall -- the same row scan measured ~6x
  slower than native under node. The u32-column form with no u128
  anywhere wins there even scalar (**1.28x** at 197 limbs), and
  vectorized with simd128 `u64x2_extmul_*` wins **1.7-1.9x** from 50
  to 1000 limbs.
- **multithreading needed no spin-barrier pool.** rayon (already a
  dependency) at the right granularity: `join` the complex square's
  two independent muls, and stripe each mul's rows across threads
  (interleaved -- row i costs O(i), contiguous chunks unbalance) into
  PRIVATE accumulators merged exactly at the end. Complex-square
  shape, joined + 8 stripes: **1.95x** at 197 limbs, **2.69x** at
  400, **3.45x** at 1000; ~1.0x at 100 limbs where the fork cost eats
  the work.

What shipped: `mul_trunc` is now a dispatcher over three
BIT-IDENTICAL implementations of the same truncated product set --
serial row scan (native, shallow), rayon-striped row scan (native,
>= 192 limbs on a >= 4-thread pool), u32 columns with a simd128 core
(wasm32; scalar-column fallback if simd128 is off). `FixedComplex::
sqr`/`mul` join their independent muls past the same threshold.
End-to-end through the real reference builder (DF shadow and orbit
store included): 197 limbs **46.2 -> 25.5 us/iter (1.81x)**, 400
limbs **173.2 -> 69.3 (2.50x)**.

Both caveats above held by construction rather than by care: integer
arithmetic is exact, so any correct summation of the same product
multiset gives the same limbs. No relimbing, no representation
change; differential tests (`mul_impl_tests` in `fixedpoint.rs`)
hold every implementation to the serial scan on adversarial inputs,
and a dispatcher test crosses the parallel threshold. The 74 escape
visual baselines are unchanged. The simd128 core was verified
bit-exact against the row scan under node (the harness asserts
equality on every call).

Build-flag footnote: wasm builds now pass `-C
target-feature=+simd128` -- set in `.cargo/config.toml` AND in both
`build-wasm` scripts, because `RUSTFLAGS` in the environment
REPLACES the config list (cargo's flag sources are mutually
exclusive; the scripts' env var had silently been dropping the
getrandom cfg too, now also repeated there). Cost-free for
compatibility: every WebGPU-capable browser shipped simd128 first.

Deliberately NOT taken, because they change bits: the three-squaring
reformulation (re^2, im^2, (re+im)^2 ~= 1.65 mult-equivalents vs 2)
and truncated Karatsuba both alter which carries fall at the
truncation edge, which churns every deep-zoom baseline and the orbit
store's cold==warm identity. Needs explicit sign-off if ever wanted.

**Follow-up 2026-09-01 — the SECOND visit got the same treatment.**
Profiling a realistic revisit (a stored 10.1M-iteration, 197-limb
orbit; generator + profiler live as ignored tests in `reference.rs`)
found the cached path spending ~4.9 s of single-threaded CPU before
the GPU could start, and 4.0 s of it was the BLA build — ON THE
RENDER THREAD, a UI freeze on every revisit of a deep location. All
of it was independent-per-element work, so it parallelized without
changing a single computed value:

- BLA build 3,970 → ~560 ms (level 0 and every merge level are pure
  per-entry maps; also dropped a per-level `clone()` that was
  copying the whole table a second time). Differential test pins the
  parallel build bit-identical to the serial recurrence.
- `from_bytes` 778 → ~135 ms: every DD-shadow correction is a full
  restart state, so the replay parallelizes by segment (the profiled
  orbit carries 188k corrections, median segment 54); plus a
  parallel min-rescan. The store's byte-exact roundtrip tests are
  the differential harness.
- BLA GPU packing ~125 → ~70 ms (fixed 32-byte slots, parallel
  chunks), `with_exp` conversion parallelized too — it runs holding
  the orbit worker's progress lock.

Net: the revisit stall is ~0.8 s, ~6x less, all bit-identical (74/74
escape visual baselines unchanged).

**Follow-up same day — the settle was vsync-bound, and that was the
part users actually felt.** Driving the app's exact frame path
against the real cached orbit (the `timeline_of_a_cached_revisit`
repro) showed the remaining wait was not CPU at all: the in-app loop
ran ONE ceiling-capped chunk per redraw, so a 10.1M-iteration settle
was ~530 vsync frames (~9 s) with the GPU ~80% idle in each. Chunks
now BATCH: several dispatches per redraw, filling the same 10 ms
GPU target one chunk was already allowed, each dispatch still
individually ceiling-capped (the driver-watchdog bound is per
dispatch and is untouched). Measured on that orbit: 531 frames ->
101, so the visible settle drops from ~9 s to under 2 s. Batching
engages only past a real timestamp measurement in the exact cost
regime, never on a restart frame, capped at 16, and each batched
pass gets its OWN params buffer — `Queue::write_buffer` applies at
the submission boundary, so passes sharing one buffer would all read
the last window (measured: the image self-heals through the
per-pixel resume, but the batch silently collapses into one
watchdog-length dispatch — the exact hazard the ceiling exists to
prevent). ESCAPE_CHUNK_BATCH=1 is the escape hatch; a repro test
pins batched == unbatched bit-identical.

Still open, in descending value: a persistent spin-barrier pool for
the reference build itself (rayon fork overhead caps the current
join+stripe at ~2x of the ideal ~6x; the doc above measured the
shape), interior detection for the perturbed templates (the direct
path's exact-repeat check was never ported, so interior pixels march
all of max_iter — now the next GPU-side lever once frames are
GPU-bound again), dropping the span-2 BLA level (halves table
size/build/upload but CHANGES which skips fire — baseline churn,
needs sign-off), and the bit-changing multiply options above.

Perspective on priority: the orbit store already turns the second
visit to a location into seconds, and FFORBIT6 made those files ~100x
smaller. §7 was about the FIRST visit; this follow-up was the second.

**2026-09-01 — the multibrot dip seam.** A depth-426 power-4 view
rendered a straight seam: two populations of pixels with entirely
different content split along a half-plane. Root cause, found with a
CPU mirror of the FE shader (op-for-op f32/CFe2, judged per pixel
against exact fixed-point orbits — the mirror ships as
`fe_step_survives_reference_dips`): the p >= 3 binomial step raised
the reference MANTISSA to powers assuming it O(1), but entries within
f32's range are stored raw with e = 0 — at a reference dip (|Z| ~
2^-51 mid-orbit) the mantissa's cube left f32 range and the
Z^(p-1)·w term silently flushed. Pixels whose reference index reached
the dip without a rebase took the poisoned step (~17% of pixels, ±30
iterations wrong); pixels that rebased first never executed it —
hence the straight rebase-gate boundary. The fix normalizes the
mantissa before powering (exponent folded into the term multiply) and
reads the powers in DF rather than hi-only; measured wrongness
against exact orbits fell 16.9% -> 1.2% (the DF noise floor). p = 2
never powers the mantissa, which is why deep Mandelbrot zooms never
showed it. Guards: the CPU mirror asserts both that the shipped step
stays at the floor AND that the un-normalized step still reproduces
the bug; `deep_multibrot_matches_exact_orbits` (GPU) asserts the
end-to-end render against exact orbits; the view is also a visual
baseline (`escape/multibrot-dip-seam`). Diagnostic hatches added
along the way: ESCAPE_BLA=0 renders without iteration skipping.

Noted while diagnosing, not yet done: the SCALED rung's p >= 3 step
(`delta_step_scaled_on`) powers the plain f32 reference the same way
— the same underflow class is reachable at dips within its zoom
range; audit it. And the same view answered "why no orbit-cache
file": its reference ESCAPES at iteration 2,098, so the orbit costs
~milliseconds to rebuild and the store's cost gate skips it by
design — but an escaping reference also caps every pixel at ~2,099
useful reference steps; a nucleus search wider than the view radius
(the relocation cap allows ~50 view-heights) would find the
non-escaping references such views want.

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

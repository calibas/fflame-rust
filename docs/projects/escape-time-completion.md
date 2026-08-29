# Escape-time: what is left before the branch is done

Agreed 2026-08-28, after the TDR-safety plan shipped and the NTT
project was measured and parked. Seven work items, each with what
the survey actually found rather than what it was assumed to be.

The survey's headline: **the engine is in far better shape than its
surface**. Rendering, deep zoom, orbit caching and crash-safety have
had weeks of work; scripting and the online API have had none at
all, and 20 of 23 formulas cannot zoom past the f32 direct path.

---

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

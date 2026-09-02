# Escape-time: the finished plans

Four documents whose work is done. The living design record is
[projects/escape-time-fractals.md](../../projects/escape-time-fractals.md),
which is NOT archived: it carries the dated history of how each piece
was built and what was measured, plus a **What is left** section at the
top listing the handful of things these four did not finish.

## What each one is

**escape-time-completion.md** — the per-item tracker for "what is left
before the branch is done", written 2026-08-28. Seven numbered items,
all closed:

| item | closed |
|---|---|
| 1. Accuracy against a reference | 2026-08-28 |
| 2. Perturbation tiers | 2026-09-02, for the tractable set |
| 3. UI/UX pass | 2026-08-29 |
| 4. Splitting the WASM modules | 2026-08-29 (escape-only module is 49% smaller) |
| 5. Online API support | shipped after writing; the doc's survey stands as the design |
| 6. Scripting support | 2026-08-29 |
| 7. Reference-build CPU performance | 2026-08-31 |

Item 2 is the one with a tail: eight formula families perturb, and the
transcendentals, Chebyshev's two non-unity functions and the
series-approximation measurement do not. That tail moved to the living
doc rather than staying here. Its per-tier verification table — every
family with the error measured against exact orbits — is the reason to
keep this document readable rather than delete it.

**escape-new-families.md** — research into four candidate families,
2026-08-29, sourced rather than reconstructed (the recurring bug class
in this project is a plausible-looking paraphrase of a formula). Six of
its seven items shipped: golden-spiral orbit trap, Cantor sine
bouquets, Origami Butterfly, analytic normal shading, relief shading,
Lattès maps and the sphere average. §7, temporal antialiasing and
spectral rendering, is planned and never scheduled — its costing is
still the best writeup of that feature and is referenced from the
living doc. §4 concluded that multi-scale Turing patterns and BZ are
NOT escape-time fractals, which is what started the third family.

**field-type-fractals.md** — the seed document for that third family,
superseded in planning by the four `simulation-*` documents in
projects/, which are ACTIVE and deliberately not archived (no code
exists yet). Kept because it is where the requirement came from, and
because it records the two things planning changed: the mode is named
Simulation rather than Field (escape already uses "field" for the
opposite meaning), and the Reusser quotation in it is not verbatim.

**escape-tdr-safety.md** — the device-loss plan, agreed 2026-08-28,
after field crash logs showed a TDR watchdog kill hanging the app.
Items A-D and both animation sections all shipped: the direct and
perturbed circuit breakers with their persisted tuning file, GPU
timestamp pacing, interior detection, `GpuContext::reinit()`, and
escape-mode animation playback and live preview. This one was archived
before its items were verified as complete; they now are.

## Why these are archived and the design record is not

These four are PLANS — they say what to build and how to test it, and
once built they stop being instructions. The living document is a
different kind of thing: a measurement log with the failures in it, so
that a future change gets judged against what was already tried rather
than repeating it.

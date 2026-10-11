# Escape-time: the record

The escape-time engine shipped; its reference is
[docs/main/ESCAPE.md](../../main/ESCAPE.md), and what is still not built
is listed in
[projects/escape-sim-followups.md](../../projects/escape-sim-followups.md)
("The escape engine: not built"). This folder is how it got there.

## The design record

**escape-time-fractals.md** -- the engine's dated design record, from
plan to shipped feature: the decision record (§3), the phases and what
each measured (§4), the mode A catalogue (§5), mode C's escape-time IFS
(§6, still not built), the colouring and trap catalogue (§8), latency
and the recolour cache (§8b), colouring at depth and device-loss
recovery (§8c), the panel (§8d), antialiasing (§8e), testing and the
bloat ledger. Read it before changing anything it measured: it has the
failures in it, so a change can be judged against what was tried.

**derivative-under-perturbation.md** -- carrying the derivative through
the perturbed path, so distance estimation and the terrain's distance
source work at depth. All four phases done 2026-10-04.

**escape-camera-lens.md** -- any variation as a lens on the screen
offset, the sites it reaches, the probe that chose the usable lenses.
Steps 1-4 built; bringing it to every mode is planned in
[projects/camera-lens-everywhere.md](../../projects/camera-lens-everywhere.md).

**ifs-distance-rendering.md** -- the flame as a distance field: mode D,
the Flame Attractor and its solid. Phases 0-3 built; the remaining
performance work is in the follow-ups.

## Source conversations

**pertubation-theory.md** -- perturbation, rebasing, BLA and floatexp
worked out; implemented in `src/escape/` (`reference.rs`, `bla.rs`,
`fixedpoint.rs`, `nucleus.rs`, `orbit_store.rs`).

**escape-time-ifs.md** -- the missed-fractal sweep, field-mode fractals,
the Hepting-Hart escape buffer and the chaos-game bridges; referenced in
the design record as *[ETI]*. Absorbed, except mode C.

**escape-ntt-reference.md** -- computing reference orbits on the GPU by
number-theoretic transforms. Shelved: phase 0 measured it 7.7x slower
than the CPU path.

## The finished plans

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
series-approximation measurement do not. That tail moved to
escape-time-fractals.md (archived too since; what is still open is in
[escape-sim-followups.md](../../projects/escape-sim-followups.md)). Its per-tier verification table — every
family with the error measured against exact orbits — is the reason to
keep this document readable rather than delete it.

**escape-new-families.md** — research into four candidate families,
2026-08-29, sourced rather than reconstructed (the recurring bug class
in this project is a plausible-looking paraphrase of a formula). Six of
its seven items shipped: golden-spiral orbit trap, Cantor sine
bouquets, Origami Butterfly, analytic normal shading, relief shading,
Lattès maps and the sphere average. §7, temporal antialiasing and
spectral rendering, is planned and never scheduled — its costing is
still the best writeup of that feature and is referenced from
escape-time-fractals.md. §4 concluded that multi-scale Turing patterns and BZ are
NOT escape-time fractals, which is what started the third family.

**field-type-fractals.md** — the seed document for that third family,
superseded in planning by the `simulation-*` documents, since
archived too, in [../projects/](../projects/) -- the simulation engine shipped. Kept because it is where the requirement came from, and
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

# Variation reachability census

**Status: BUILT** — phases 1–3 shipped (instrument 05deb591, runtime
9a218b15, corpus + rank in the phase-2/3 commit). Phase 4's commit
decision is resolved below, for now: **the report is not committed.**

Counts what real renders actually feed every variation, so the math
probe's divergence list can be ranked by what occurs instead of read as
438 equally-weighted curiosities.

```bash
cargo run --release --bin variation_probe -- census -i <corpus dirs> -o census.txt
cargo run --release --bin variation_probe -- rank census.txt   # census × probe diff
```

## Why it exists

The math probe answers *what can diverge*: variation × input-class
pairs where two builds disagree. It cannot say whether a real flame
ever presents that input to that variation. Every reachability question
this branch answered was answered by hand:

- `npolar` was catastrophic **because** its default parity makes
  `cosa = sina = 0` on *every call* — found by bisecting a visual diff.
- `ho` reaches `atan2(0, 0)` from a whole neighbourhood of inputs
  **because** `v*v` underflows — found by a one-off global-guard A/B.
- Subnormal *inputs* were assumed unreachable ("a chaos game spanning
  O(1) never sees 1e-38") — plausible, never measured, and this branch
  was wrong every time it substituted plausibility for measurement.

The census replaces those one-offs with one instrument.

## The trap this design avoids

**Instrumenting variation *inputs* would have missed both known bugs.**
npolar's input point was ordinary — the (0,0) was an *intermediate*
(`cosa`, `sina`). Same for ho (`v*v` underflowed internally). No
input-side census sees intermediates.

The division of labour is therefore:

| | covers | tool |
|---|---|---|
| internal singularities | probe feeds ordinary + adversarial inputs and sees the *output* diverge, whatever the internal cause | **probe** (exists) |
| which inputs occur in practice | real renders, real params, real trajectories | **census** (this) |

The probe's entries are keyed by input class, so `census × probe`
joins directly: a probe entry at input `-0+0` for variation X is
reachable iff the census saw X receive a `-0+0`-class input. The
census does **not** need to see intermediates — the probe already
folded those into "which inputs make this variation diverge".

What the join inherits: the probe evaluates defaults + one-at-a-time
parameter sweeps. A divergence that needs a parameter *combination*
is invisible to both halves. Known limit, carried honestly.

## Design

### Hook point

`build_apply_variations_2d/3d` (shader_builder_v2.rs) — the generated
dispatcher, the one place every variation call already flows through.
Per call, gated by a new `CENSUS` template flag:

1. classify the input point (per component, then as a pair/triple)
2. call the variation exactly as today
3. classify the output

Classification is **bit-based** (`bitcast<u32>`, exponent/mantissa
fields), the same reasoning as the probe's CPU classifier: bit ops are
immune to fast-math, so the instrument cannot be lied to by the thing
it measures.

### Class vocabulary

Reused from `src/probe/inputs.rs` — the census must speak the probe's
language or the join is a translation layer that drifts. Per component:
`+0, -0, subnormal, tiny` (|x| small enough that x·x underflows),
`normal, large, huge` (past the 1e32 bad-value threshold), `inf, nan`.
Pair-level flags for the patterns the probe's input list distinguishes
(both-zero, axis, mixed signed-zero, lopsided).

Input NaN/Inf *should* be impossible (bad-value recovery respawns
first) — counted anyway. If nonzero, that is a finding about the
recovery, not noise.

### Buffers and cost

One storage buffer of atomic u32 counters:
`[variation local idx 0..100] × [class ~16] × {in, out}` plus one
call counter per (xform, variation) — ~13 KB. Binding follows the
PROBE pattern; verify against Metal's 31-storage-buffer limit.

Atomic traffic: the call counter is 1 atomicAdd per variation call;
class counters fire **only when a class is interesting** (ordinary
inputs skip the atomic). Interesting inputs are rare by construction,
so the hot path adds one atomic. Estimated 1.5–3× slowdown with the
flag on. Irrelevant: this is an offline instrument.

**Flag off, the WGSL is byte-identical** — the contract PROBE and
SOLID already hold to, enforced by the canonical shader dumps.

### What the instrument must not promise

Instrumented renders are **not** promised bit-identical to normal
renders even in their math: extra code can change contraction and
scheduling under fast-math. Class-level counts are robust to that;
pixel-level comparison of census runs is not a supported use.

### CLI and report

`census` subcommand on the existing `variation_probe` binary — it
shares the report conventions, the input vocabulary, and `compare`'s
parsing. Corpus = directories of `.fflame` configs; per flame: load,
build with CENSUS, run a fixed iteration budget through the headless
render path, read back, accumulate.

Report (committed conventions: schema line, os/adapter, diffable):

```
# variation reachability census — schema 1
# corpus: assets/presets (9) + tests/visual/configs (148) + random seed 1..N
npolar   2d  calls 1.2e9   in.both_zero 0.000  out.zero 1.000   worst: apo-misc7-smoke
ho       2d  calls 3.4e8   in.axis 0.031  in.tiny 0.002        worst: ho-smoke
```

Counts, not flags: npolar-on-every-call vs once-per-billion is the
whole ranking signal.

Aggregation: per (variation, dim) across the corpus, keep the max
fraction per class and the flame that produced it — the report names a
*reproducer*, not just a number.

### `rank` — the join

Reads a probe `compare` output plus a census report; emits probe
entries that the census marks reachable, ordered by observed fraction ×
severity of the class transition (NaN-involved > zero↔finite > sign).
This is the artifact that turns "438" into a short list with evidence,
and each surviving entry becomes either a per-site WGSL guard (the
established response) or an accepted-diff entry with a reason.

## Explicitly out of scope (v1)

- **Subflames** — `subflame_iterate` evaluates nested flames' variations
  through its own machinery; instrumenting it is its own project.
- **WASM** — native offline tool, like the probe.
- **Gating** — the census is diagnostic. Whether a committed census
  becomes a gate is decided after seeing run-to-run stability; counts
  are stochastic and cross-platform counts drift by construction
  (the divergences themselves alter trajectories).

## Measured results (2026-08-04, M2 / Metal)

- Corpus: 9 presets + 148 visual configs + 100 seeded randoms = 257
  flames, 252 run (5 solid skipped), 50M iterations each, **48 s**
  total — the 10-minute estimate was off by 12x in the good direction.
- ~1,110 report rows; 367 variations exercised.
- Validation: npolar's `(normal, ±0)` output structure — the apo-misc7
  bug — appears at 52.8% + 3.0% of its calls, from orbit, on the first
  run. A count=3 observation (fdisc) sits beside it with no false rows.
- `rank` over the standing Windows↔Metal probe diff: **2,275 hard
  divergence sites → 91 REACHABLE** (with reproducer flames), 1,422
  unobserved-by-corpus, 762 not-exercised. The top of the list is
  exp-family NaN transitions at `large`/`near_threshold` inputs that a
  real corpus flame delivers.

### Stability, measured honestly

The committed-report question got three experiments:

1. **Forcing `deterministic_rng` across the corpus** (presets and
   randoms don't set it): run-to-run churn dropped ~150 → ~65 rows.
   Kept.
2. **Explicitly clearing the census tail** before the first pass: the
   tail is excluded from every ordinary clear, and relying on
   buffer-creation zeroing let recycled allocations leak the previous
   flame's counters — row counts stabilised (1112 = 1112) once
   cleared. Kept.
3. **Device-per-flame** (virgin allocations for everything): did NOT
   reduce the residual churn — the recycling hypothesis is
   insufficient. Reverted.

Residual: ~40–80 of ~1,110 rows drift between identical corpus runs,
concentrated in heavy-tailed classes (how often a walk escapes past
1e16) of a handful of flames. The same flame is **bit-deterministic
run-to-run standalone**, and our generated WGSL is process-stable (the
canonical dumps prove that), so the residual lives below our source —
naga codegen or Metal compilation/execution. Until that is understood,
the report is **generated locally and gitignored**; `rank` reads the
local file. Buckets damp most of the churn for human reading either
way. Worth re-running the two-corpus experiment on Windows/NVIDIA —
if it is byte-stable there, the committed-report convention can be
"regenerate on Windows", like the probe reports.

## Phases

1. **Instrument**: CENSUS flag, classification WGSL, buffer + readback,
   single-flame CLI. Byte-identity with flag off verified by the dumps.
   The riskiest phase — everything after is plumbing.
2. **Corpus runner**: batch over directories + random-generator seeds,
   aggregation, report writer.
3. **`rank`**: the join against probe compare output.
4. **Decide**: committed reports? gate? both deferred until the numbers
   demonstrate stability.

## Open questions

- **Corpus mix**: presets + visual configs are obvious; how many
  random-generator seeds represent "real usage"? (Proposal: 100 seeds,
  the generator exercises parameter space the curated sets do not.)
- **Iteration budget per flame**: enough to see once-per-1e8 events
  without hour-long runs. (Proposal: 100M iterations per flame,
  ~1–2 s each on the M2; corpus of ~260 flames ≈ 10 min.)
- **Cross-platform census**: run on both machines and diff, or treat
  one platform's census as representative? (Proposal: build first,
  measure stability, then decide.)

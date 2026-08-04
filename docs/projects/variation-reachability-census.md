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

### Stability: two bugs, both fixed, report committed

Getting to a byte-stable report took four experiments and found two
real bugs — one of them nowhere near the census:

1. **The random-flame generator was nondeterministic per process.**
   `enabled_variations` is a HashSet, the rng draws index into its
   collected order, and HashSet iteration is per-process random — so
   the same Pcg seed generated a DIFFERENT flame in every process. The
   in-process determinism test could never see it (one process, one
   order). Found by the curated-vs-random split measured on Windows;
   proven by dumping generated flames from two processes; fixed by
   sorting the list (randomize.rs), with a regression test that builds
   the same set in two insertion orders. Every stability measurement
   taken before this fix was confounded by it.
2. **The census tail inherited the previous flame's counters** through
   recycled allocations on a shared device — explicit clear added
   (clear_census_tail).
3. **Device-per-flame** was tried under the confounding and wrongly
   "refuted"; re-run after the generator fix, it is the remaining piece:
   shared-device corpus runs still drifted (~150 rows) while every
   flame was bit-deterministic standalone; fresh devices per flame +
   fixed generation = **two full-corpus runs byte-identical**, at no
   measurable cost (48s).

The report is therefore **committed**, and — decided at the first
Windows regeneration — **one file per platform**:
`docs/generated/variation-census-{windows,macos}.txt`. Each machine
reads and rewrites only its own; the path comes from
`default_census_path()` in the CLI, so a regeneration cannot clobber the
other platform's measurement.

That is deliberately *unlike* the probe reports, which stay single-file.
The difference is what the artifact is. A probe report is a description
of the shader math, compared against a chosen baseline; a census row is
a measurement of a specific GPU running the corpus. Measured on the
first Windows run: **424 of ~1,270 rows differ** between Windows/NVIDIA
and macOS/Metal (959 identical, 118 macOS-only, 271 Windows-only, 35
differing only in bucket). Collapsing that onto one authoring platform
would throw away the comparison the census exists to make.

Byte-stability confirmed independently on Windows after the generator
fix: three consecutive full-corpus runs byte-identical (252 flames, 50M
iterations, ~50 s each).

Open question, deliberately not chased here: what exactly the
shared-device contamination corrupts for flames run back-to-back on
one device. The census sidesteps it with fresh devices; the app and
CLI exports are insulated (one flame per renderer lifetime per
process); but it is a real observation about the stack, recorded in
the runner's comment.

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

# GPU reference-orbit computation (the FractalShark NTT direction)

Plan written 2026-08-28; scheduled AFTER the TDR-safety items and the
orbit-store compression. This is a project, not a tail item — it gets
a measurement-gated go/no-go before any real investment.

## What FractalShark demonstrates (the existence proof)

FractalShark (mattsaccount364) computes reference orbits ON the GPU:
NTT-based big-number multiplication with CRT over multiple primes,
the whole iteration loop resident in CUDA, ~10x over a multithreaded
MPIR+AVX2 CPU implementation on an RTX 4090 — at 16,384 32-bit limbs
(~158k decimal digits). Its Newton/Feature-Finder rides the same
pipeline. (FractalShark.pdf §5-6; LAv2 corresponds to our BLA, its
2x32+exponent type to our DF tier — the architectures rhyme.)

## Our baseline and the honest crossover question

CPU (`escape/fixedpoint.rs`): sign-magnitude u64 limbs, truncated
schoolbook multiply (~n²/2 limb products), measured
1.263e-9 s · iters · limbs² (the calibrated `predicted_orbit_seconds`
model; f3 = 10.1M iterations at 197 limbs = 495 s).

Two facts frame the design:

1. **The iteration is sequential** (z_{n+1} needs z_n). GPU
   parallelism exists only INSIDE one multiplication. Any design
   that dispatches per iteration dies on launch overhead; the loop
   must be resident (persistent-kernel style: one dispatch runs many
   iterations, bounded — see TDR note below).
2. **NTT is the large-n regime.** At our common depths the limb
   counts are modest: z1000 ≈ 17 limbs, z10000 ≈ 158, z100000 ≈
   1.6k. FractalShark's 10x is at 16k limbs. At ≤ a few hundred
   limbs the GPU's win (if any) comes from PARALLEL SCHOOLBOOK
   (n² products across threads, delayed-carry accumulation, log-tree
   carry resolve), not NTT. NTT becomes the rung above it.

## Phases

## PHASE 0 RAN. VERDICT: PARKED (2026-08-28)

Measured, not estimated — `src/escape/gpu_bignum_probe.rs`, a
one-workgroup parallel-schoolbook squaring kernel verified BIT-EXACT
against a Rust oracle before any timing was taken (a fast wrong
kernel measures nothing; the probe caught exactly that during
development, when an off-by-one buffer index made the shader run zero
iterations):

```
limbs  digits  CPU model/iter   GPU/iter   ratio   verdict
   64     256           5.2 us      91.4 us   0.06x   no
  197     788          49.0 us     380.4 us   0.13x   no
  512    2048         331.1 us    1230.4 us   0.27x   no
```

At the gate depth (197 limbs, the f3 reference) the GPU is **7.7x
SLOWER**, against a gate of 3x faster. The project is parked.

WHY, and why this was worth measuring rather than assuming: the
ratio climbs steadily with size (0.06 → 0.13 → 0.27), which is the
signature of parallelism starting to pay. Extrapolating the trend,
parity arrives somewhere around 2-4k limbs and a 3x win somewhere
past 10k — which is precisely the regime FractalShark operates in
(16,384 limbs, ~158k decimal digits) and precisely where NTT becomes
the right algorithm. Their 10x is real; it is just not reachable at
the depths this renderer actually visits.

Two structural facts drive the loss, and neither is a tuning
problem:

1. **One workgroup.** The orbit is sequential — iteration n+1 needs
   n — so parallelism exists only inside a single multiply. A
   30-SM GPU runs this on one SM. Splitting a multiply across
   workgroups needs a grid-wide barrier, which WGSL does not have;
   emulating it costs a dispatch per iteration (~5-10 us of launch
   overhead against a 49 us CPU iteration), or a spin-lock barrier.
2. **No 64-bit multiply in WGSL.** Every u64xu64 product the CPU
   issues as one instruction becomes sixteen 16x16→32 products here.
   The GPU must win that 16x back from parallelism before it wins
   anything at all.

Phase 4 (wasm), the phase with the best relative case, does not
survive either: the browser's CPU path is a few times slower than
native, which moves 0.13x to roughly 0.5x — still a loss.

### Corroborated by FractalShark's own report (v0.5, Dec 2025)

The author's release notes for the GPU reference orbit say the win
arrives "at a precision of 16384 32-bit limbs (~158,000 decimal
digits)" -- and, tellingly: "The only built-in View that shows a
clear benefit to the GPU-accelerated approach is View #30, which
uses 16384 32-bit limbs internally." One view, out of a built-in
set. In the project that BUILT the thing.

Converting to this project's units (we count 64-bit limbs, they
count 32-bit): their 16,384 limbs are 8,192 of ours. Fitting our
three measurements gives ratio ~ limbs^0.72, which extrapolates our
plain schoolbook kernel to about **2x** at that precision -- against
their reported 10x with NTT on an RTX 4090. Those two numbers agree
rather than conflict: NTT replaces O(D^2) with O(D log D), which is
worth roughly the remaining 5x at 8k limbs, on a much larger GPU
than the one measured here. The same fit puts our schoolbook at
parity around 3,200 of our limbs and 3x around 14,600.

The decisive part is what 8,192 limbs MEANS as a view. Precision
that deep corresponds to a zoom of roughly **2^524,000**. The
deepest location this project has ever rendered is z9,316, at 197
limbs -- and that one already takes eight minutes of reference
build. FractalShark's GPU pipeline is real, correct, and pays off
about fifty times deeper than anywhere this renderer has been.

The probe stays in the tree. It is the reproducible form of this
decision: re-run `cargo test --lib gpu_bignum -- --ignored
--nocapture` on different hardware, or after WGSL grows 64-bit
integer multiply, and the numbers answer the question again without
re-deriving any of this.

What this does NOT close: the CPU side has real headroom nobody has
spent yet — SIMD limb multiplication, Karatsuba above a few hundred
limbs, and multithreading a single multiply. Those are the moves
worth making if reference builds need to be faster.

### The original plan, for reference

**Phase 0 — measure, then decide.** WGSL prototype of parallel
schoolbook squaring at 64 / 197 / 1024 limbs (one workgroup per
multiply; u32 limbs with 16-bit-split products — WGSL has no u64 and
no mul-hi; accumulate partials in paired u32). Persistent chunked
loop (4096 iterations per dispatch, matching the worker's publish
chunk). Compare wall time against the CPU model on the same machine.
GO if ≥3x at 197 limbs; otherwise park the project and record the
numbers (the CPU path with reuse + store may simply be good enough
below ~1k limbs).

**Phase 1 — exactness.** Integer arithmetic is exact: the GPU orbit
can and MUST be bit-identical to the CPU fixed-point orbit
(truncation positions included — same INT_BITS layout, same
truncated-multiply window as `fixedpoint.rs`). This preserves every
determinism guarantee (orbit store, cold==warm, cross-run repro).
Tests: adversarial parity vs CPU — deep dips (|Z| to 2^-1000s via
the floatexp export), escape-threshold straddles, the f3 head. Note
Metal fast-math is a NON-issue here: integer ops are immune (the
shader-lint hazards are float-only).

**Phase 2 — integration.** `OrbitWorker` gains a GPU lane behind the
same progressive-publish interface (chunk in, prefix out — the
render side cannot tell). Device choice is the risk to design
around: sharing the app device contends with rendering and puts
reference compute inside the same TDR budget (a lost device then
kills BOTH; the breaker lessons apply). Prefer a second low-priority
device where the backend allows it; chunk dispatches to ≤ ~100 ms
regardless. CLI/headless uses the same lane when present.

**Phase 3 — NTT rung (≥ ~1k limbs).** Digit decomposition to 16-bit
digits, forward/inverse NTT in workgroup shared memory over 2-3
NTT-friendly 31-bit primes + CRT recombination, pointwise multiply,
carry resolve. Only reachable at zooms ≥ ~z60000 with today's zoom
model — build it when someone actually drives there, on top of the
Phase 1 exactness harness.

**Phase 4 — wasm.** The largest RELATIVE win: the browser CPU path
lowers u64xu64 through compiler-rt over 32-bit halves (the wasm
asterisk in the escape plan), so browsers pay several times the
native cost per limb product — and the SAME WGSL runs under WebGPU.
Gated on Phase 0-1 landing; no worker/COOP-COEP dependency (the GPU
lane replaces the missing worker thread rather than needing one).

## Non-goals

- Not replacing the CPU path — it remains the fallback and the
  exactness oracle.
- Not runtime per-pixel orbit decompression (FractalShark §6's other
  trick); our store compression handles disk/RAM, and the GPU orbit
  buffers stay raw hi/lo/e.
- Not GPU Newton/nucleus (Feature-Finder territory) until the
  multiply primitive exists and has soaked.

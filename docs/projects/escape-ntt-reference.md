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

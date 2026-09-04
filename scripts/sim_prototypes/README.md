# Simulation mode — CPU prototypes

Phase-0 measurement for the simulation render mode
([docs/projects/simulation-fractals.md](../../docs/projects/simulation-fractals.md)).
NumPy, CPU, no GPU needed. Nothing here ships; these exist to answer
questions the plan would otherwise have to guess at, **before** the GPU
kernel is written to a guess.

They are here rather than in a scratch folder because their answers are
the reason the shipped discretisations are what they are, and a future
change to a rule should be able to re-run the thing that justified it.

Images and tables are written to `output/sim_proto/` (gitignored —
render artifacts do not belong in the repo).

```bash
python scripts/sim_prototypes/proto_gray_scott.py
python scripts/sim_prototypes/proto_mccabe.py
```

Requires `numpy` and `pillow`.

## What each one settled

**`proto_gray_scott.py`** — validates Karl Sims' 3×3 Laplacian scheme
(D_A = 1, D_B = 0.5, dt = 1, weights −1 / 0.2 / 0.05, periodic) against
Pearson's named (F, k) classes, and measures steps-to-settle. Two
results changed the plan:

- **The clamp to [0, 1] is mandatory**, not hygiene — without it the
  field reaches NaN within a few thousand steps at some (F, k) pairs.
- **Seed size decides survival**: 12-px blobs die at the mitosis
  parameters where 24-px blobs live. The seeding options ship with that
  in mind.

It also showed the settle metric (mean |Δv| < 10⁻⁴ over 200 steps)
fires long *before* the picture is finished, because growth patterns
keep advancing into empty field at a fixed speed. So the driver's step
budget is sized from the images, not from the metric.

**`proto_mccabe.py`** — the multi-scale Turing rule with exact FFT disc
averages, which is the reference the GPU's mip-pyramid approximation
has to be compared against in phase 3. Texture is present by step 20
and developed by 100; it never settles. The 5-fold symmetric variant
costs 101 ms/step against 9 ms plain, but that is Python's rotation,
not anything intrinsic to the rule.

## Still to write

One script each for the remaining Tier-1 models, per the phase-0 gate
(a step-cost table with no entry marked "estimate"): FitzHugh–Nagumo,
Brusselator, Schnakenberg, hodgepodge, cyclic CA, spatial RPS, Eden,
ballistic deposition, Ising, percolation. Plus the Abelian sandpile's
parallel bulk-toppling round count for 2²⁰ grains — the one cost the
catalogue could not estimate at all.

The GPU side of phase 0 is done and lives elsewhere:
`src/sim_microbench.rs` measures ms/step for the shared stencil shape
(results in
[simulation-pipeline.md §10](../../docs/projects/simulation-pipeline.md)).

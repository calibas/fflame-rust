# Simulation mode — CPU prototypes

Phase-0 measurement for the simulation render mode
([docs/projects/simulation-fractals.md](../../docs/projects/simulation-fractals.md)).
NumPy, CPU, no GPU needed. Nothing here ships; these exist to answer
questions the plan would otherwise have to guess at, **before** the GPU
kernel is written to a guess.

They are here rather than in a scratch folder because their answers are
the reason the shipped discretisations and presets are what they are,
and a future change to a rule should be able to re-run the thing that
justified it. Every number they produced is recorded against the model
it belongs to in
[simulation-catalog.md](../../docs/projects/simulation-catalog.md),
with the date.

Images, JSON row sets and logs go to `output/sim_proto/` (gitignored —
render artifacts do not belong in the repo). Requires `numpy` and
`pillow`.

| script | models | modes |
|---|---|---|
| `proto_gray_scott.py` | Gray–Scott | — |
| `proto_mccabe.py` | McCabe multi-scale Turing (exact FFT reference) | — |
| `proto_reaction_diffusion.py` | FitzHugh–Nagumo, Brusselator, Schnakenberg | default: presets + settle · `--dt`: stability ladder on the active configuration · `--wavelength`: Turing wavelength vs diffusion scale |
| `proto_cellular_automata.py` | hodgepodge, cyclic CA, spatial RPS, Ising | — |
| `proto_growth.py` | Eden, ballistic deposition, percolation labelling, Packard snowflake | default · `--kpz`: growth-exponent fit |
| `proto_wolfram.py` | elementary CA | — (verifies the bit convention against binomials mod 2) |
| `proto_sandpile.py` | Abelian sandpile | — (bulk-toppling round count, 2¹² – 2²⁰ grains) |

## What they changed

Not a summary of the numbers — those live in the catalogue. The things
that would have shipped wrong without running them:

- A FitzHugh–Nagumo "Turing" preset that produces a flat field.
- Two dt caps that were estimates (Brusselator 0.04, Schnakenberg 0.02)
  and one measured on the wrong configuration (FHN from a resting
  field said 0.5; the spiral says 0.75).
- Percolation's step budget, estimated at ~10³ for 1024², measured at
  ~3,250 typical and ~5,100 worst — and a 4× sample-to-sample spread
  that means the model needs the settle reduction, not a fixed count.
- Turing feature size costing exactly what the dt scaling implies —
  steps linear in D, quadratic in wavelength (4.6× the wavelength for
  18× the steps). A first measurement said the opposite; a broken
  settle criterion was why, and it is retracted in the catalogue.
- The sandpile's round count, which the catalogue could not estimate
  at all.

## Measurement traps, all found by falling into them

They transfer straight to the shader's `settle` stage:

- **A step that changes nothing is not the end of a run.** An Eden front
  at p = 0.05 adds nothing on ~81% of steps.
- **The settle metric fires during nucleation, before a pattern
  exists.** It reported convergence on a blank field. Require an
  amplitude floor first.
- **The settle window must be counted in steps, not samples**, and the
  onset recorded — a first version dated every still ~3,800 steps late.
- **Where a model clamps, divergence detection is the wrong
  instrument.** FHN's [−3, 3] clamp turns instability into cells pinned
  at the rails; count those.
- **Probe stability on the active configuration**, not a resting one.
- **A single stochastic realisation does not give an exponent**, and
  one percolation sample does not give a step budget.
- **Periodic per axis is a choice.** A y-periodic Eden line seed grows
  into the top row on step one.

# New escape-time families, and one thing that is not one

Research for four candidates, 2026-08-29. Two are ordinary additions
to the escape engine, one is mostly already built, and one is not an
escape-time fractal at all and needs saying so before anyone starts.

Everything below is sourced. Where the source is a paper or an
implementation, the formula is quoted rather than paraphrased, because
the whole class of bug this project keeps hitting is a plausible
reconstruction that renders something adjacent to the real thing.

---

## What the engine already has

Worth stating first, because two of the four requests are partly built:

| piece | status |
|---|---|
| McMullen maps `z^n + c·z^(-m)` | **shipped** as `mcmullen` |
| Sine maps `sin(z) + c` | **shipped** as `trig` (with the `AbsIm` escape metric) |
| Orbit traps: point, cross, circle | **shipped** as `orbit_trap` (`shape` 0/1/2) |
| Per-pixel orbit averaging | **shipped** as `orbit_average`, `magnitude_average`, `stripe_average` |
| Whole-image feedback (blur, neighbours) | **nothing** — see §4 |

23 formulas, 10 colorings, 2 field formulas.

---

## 1. Golden spiral orbit trap — do this first

A fourth `shape` for the existing `orbit_trap` coloring. It is the
smallest change of the four and the only one that improves **every
formula already shipped** rather than adding a 24th.

[Nylander](https://nylander.wordpress.com/2005/03/03/golden-ratio-spiral-orbit-trap/)
gives it directly:

```
phi = (1 + sqrt 5) / 2
r   = log|z| / (4 log phi)  -  arg(z) / (2 pi)
d   = |r - round(r)|
```

`4 log phi` is the golden spiral's defining growth: a factor of phi
per QUARTER turn, so phi^4 per full turn. `r` is therefore "how many
turns out along the spiral this point sits", and the distance to the
nearest arm is how far `r` is from a whole number. Generalising the
growth factor to a parameter costs nothing and makes it a logarithmic
spiral trap of which the golden one is a default.

**Two hazards, both already documented in this repo:**

- `arg(z)` is `atan2`, and `z` reaches exactly `(0,0)` on any orbit
  that lands on the origin — which is the Metal fast-math trap in
  CLAUDE.md. There, same-sign zeros return **pi/4** (a plausible
  finite value that silently relocates the point; it cost `npolar` 73%
  of its lit pixels) and mixed-sign zeros return NaN. Use `ff_atan2`
  from `shaders/core/utilities.wgsl`, which is IEEE-exact for all four
  sign pairs.
- `log|z|` is `-inf` at the origin. Guard before the divide, or the
  trap distance is NaN for the one orbit point that most needs an
  answer.

Neither is hypothetical; both are the exact failure modes this
codebase has already paid for once.

## 2. Cantor sine bouquets — a multiplicative parameter, not our additive one

`trig` iterates `sin(z) + c`. The bouquet family is **not** that one.
[Pardo-Simón](https://arxiv.org/pdf/2209.03284) and the Devaney line of
work are specific: *the Julia set of `λ·sin(z)` with `λ ∈ (0,1)` is a
Cantor bouquet* — a Cantor set of disjoint hairs, each an arc to
infinity, on which every point escapes except the endpoints.

So the addition is a family where the parameter MULTIPLIES:

```
z <- lambda * sin(z)          (lambda = c, or a fixed parameter in Julia mode)
```

Small work: one formula, reusing `trig`'s `AbsIm` escape metric, which
is already the right one for sine maps — these orbits escape by
`|Im z|` growing, not `|z|`. Rendering the hairs well wants high
iteration counts and a distance-estimate or averaging coloring rather
than raw escape count, because a hair is one pixel wide almost
everywhere.

## 3. Origami Butterfly — our Ducks with a different fold

[McCabe's algorithm, as documented by
algorithmic-worlds](https://www.algorithmic-worlds.net/expo/work.php?work=20110204-ds7):

> Take a square, and choose a number of random lines cutting the
> square and order them. For each point of the square, compute its
> images under the sequence of mirror symmetries about the sequence of
> random lines. Then color the result by performing an average over
> all the image points.

That is per-pixel iteration plus orbit averaging — structurally
`ducks`, which is why the same page notes the results are "very
similar to the Duck-like algorithms" by a different route. It reuses
`orbit_average` / `magnitude_average` unchanged.

The lines are the only design question. `N` lines is `2N` parameters
(angle, offset), which eats the budget fast; deriving them from a
`seed` parameter through an in-shader hash costs two parameters total
(`seed`, `line_count`) and makes the whole family explorable by
scrubbing one slider. The cost is that a specific arrangement can only
be reached through its seed — acceptable for a family whose source
describes the lines as random.

Later, if it earns it: a reflection about a line is a generalised
abs-fold, so the Burning Ship `diffabs` machinery in
`assembler.rs` is the precedent for perturbing it. Not now.

## 4. Multi-scale Turing patterns and BZ — NOT escape-time

Both requests are whole-image feedback simulations. They cannot be
escape formulas, and the reason is architectural rather than a matter
of effort.

Our escape engine dispatches one thread per pixel that iterates a map
using only its own `c` and `z`. It never reads a neighbour. McCabe's
multi-scale Turing algorithm, per
[Reusser's WebGPU implementation](https://rreusser.github.io/notebooks/multiscale-turing-patterns/),
does the opposite at every step:

> Convolve the field with Gaussian or circular kernels at different
> radii to compute activator and inhibitor values … **requires
> whole-image convolutions each step. Each pixel depends on distant
> neighbours through FFT-based convolution — the receptive field spans
> the entire domain.**

The step is: convolve at `k` scales, pick per pixel the scale with the
smallest `|activator − inhibitor|`, move the field in that scale's
direction, blend colour toward that scale's colour
([McCabe's paper](https://handandmachine.org/classes/computational_fabrication/wp-content/uploads/2024/10/Cyclic_Symmetric_Multi-Scale_Turing_Patterns.pdf)).

Belousov–Zhabotinsky is the same shape: a reaction-diffusion field
(or the [hodgepodge-machine cellular
automaton](https://www.hermetic.ch/pca/bz.htm)) updated from a
neighbourhood each step, plus a per-step resample of the domain for
the "expanding space" variant.

**What they would actually be:** a THIRD render mode beside flames and
escape — `init noise → loop[multi-scale blur → select → update] →
tonemap` — whose nearest relative in this codebase is the effects
chain, which already ping-pongs full-resolution textures. Not
`src/escape/`.

Rough shape, if it is ever taken on:

- a field texture pair (ping-pong), `f16` is what the reference
  implementation uses;
- `k` blur scales per step — separable Gaussians or a mip pyramid,
  NOT an FFT, since we have no FFT on the GPU and the reference
  notes analytic kernels evaluated in-shader;
- a select-and-update pass;
- a driver that runs hundreds of steps and hands the field to the
  existing tonemap tail.

**Measure before building.** The one number that decides feasibility
is the cost of `k` full-resolution blurs per step at 1080p, times the
step count needed for the pattern to settle. That is a half-day
prototype and it is the same discipline the NTT investigation used —
which is the reason that project was parked with evidence rather than
built on a guess.

---

## Order

1. **Golden spiral trap.** Smallest, improves all 23 formulas, and
   the two hazards are known in advance.
2. **λ·sin(z).** One formula, reusing an existing escape metric.
3. **Origami Butterfly.** One formula plus a seeded line hash.
4. **Turing / BZ.** Its own plan, its own render mode, and a
   measurement before any of it.

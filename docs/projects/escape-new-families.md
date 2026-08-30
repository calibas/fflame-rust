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

## 1. Golden spiral orbit trap — DONE 2026-08-29

Shipped as `orbit_trap` shape 3 with a `growth` parameter (golden by
default). Verified against the closed form at `max_iter = 1`, where
the image IS the distance field: 0.40/255 colour spread within a
distance bin, against 68.39 for a deliberately wrong quarter-turn
factor. Details in
[escape-time-fractals.md](escape-time-fractals.md).

One thing the plan below got wrong: it says to use `ff_atan2`. That
helper belongs to the FLAME shader's `utilities.wgsl` and is not in
scope for an assembled escape shader. The escape engine's own idiom
for the same hazard is an explicit `dot(z,z) < 1e-30` branch so
`atan2` is never evaluated at a zero pair, which is what `esc_clog`
and `stripe_average` already do, and what shipped here.

### The original plan, for reference

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

## 2. Cantor sine bouquets — DONE 2026-08-29

Shipped as `lambda_sine`. Verified 0.00% against an f64 orbit at
λ = 0.5 (an additive parameter reads 14.45%), plus the map's own 2π
periodicity and mirror symmetry. The plan missed one thing: the
parameter plane cannot seed at zero, because `sin 0 = 0` makes zero a
fixed point for every λ — the plane would render one flat colour. It
seeds at the critical point π/2, as `lambda` does at 1/2. Details in
[escape-time-fractals.md](escape-time-fractals.md).

### The original plan, for reference

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

## 5. Analytic normal shading — DONE 2026-08-29

Shipped as the `normal_map` coloring, ported verbatim from the
reference C rather than from memory (the convention this plan flagged
as unverified was pinned to Wikimedia Commons'
`File:Mandelbrot_set_-_Normal_mapping.png`, the source behind
Wikibooks' bump-mapping article). Matches that reference to 1.38/255.

Two things the plan did not anticipate:

- **Where it cannot work needed a mechanism.** The perturbed rungs
  iterate no derivative and 12 formulas define none, so a
  `HAS_DERIVATIVE` constant now tells the coloring to return flat
  light rather than shade from `z/1`.
- **A bounded coloring cannot use the template's `fract`.** A value of
  exactly 1.0 wrapped to the palette's bottom and put a black seam
  through the highlight; `ColoringFeature::Bounded` clamps instead.
  The numerical check could not see it (1.74/255 with the bug, 1.38
  without) — a person looking at the image could.

Details in [escape-time-fractals.md](escape-time-fractals.md).

### The original plan, for reference

## 5. Analytic normal shading — the "fake 3D" look

Two different techniques get called this, they fail differently, and
the difference decides which one this engine should carry.

**Finite-difference slopes.** Treat the smooth iteration count as a
heightfield and light its gradient. Kalles Fraktaler describes its own
as *"the shading is based on the angle at which the iteration count is
increasing"*. Works with all 23 formulas, needs no derivative — but it
reads NEIGHBOURING PIXELS, which a per-pixel escape shader does not
have. Its home is a post-pass over the finished value field, where the
effects chain's full-resolution ping-pong is the precedent.

**Analytic normals from the derivative.**
[Chéritat](https://www.math.univ-toulouse.fr/~cheritat/wiki-draw/index.php/Mandelbrot_set)
derives it: the normal is *"the vector of coordinates (x,y,1)/√2 where
(x,y) is the normal to the potential line through the point"*, and
since the potential is `2^(-n) log|z_n|`, one *"pull[s] back the
radial direction by the derivative of c ↦ z_n"*. That is `z/dz`,
normalised, against a light direction. O(1) per pixel, no neighbours,
and **11 of 23 formulas already carry `wgsl_derivative`** with
`distance_estimate` already consuming it.

**Which one, and why it is not just architecture.** KF's changelog for
2.14.8 records adding directional DE *"used for slopes colouring with
`Analytic` differences (requires derivatives, fixing noisy texture
when jitter is enabled)"*. Finite-difference shading goes NOISY the
moment samples are jittered — neighbouring pixels stop being at
neighbouring positions, so the gradient picks up sampling noise.
Analytic normals are immune. Anyone adding the temporal sampling in §7
would hit exactly that, so the derivative version is both the
architectural fit and the one that survives what comes next.

Plan: analytic first, gated on the existing `NeedsDerivative`
coloring feature; finite-difference later as the fallback covering the
12 formulas with no derivative, as a post-pass rather than a coloring.

**Verify the convention before shipping.** The derivation above is
sourced; the exact height/normalisation convention in the common
implementations is not, and could not be fetched in the research
session (Chéritat's server refused the connection, Shadertoy returned
403, Wikibooks only describes the method). Pin it against a reference
implementation rather than writing it from memory — the whole reason
this project ports formulas by quoting sources.

## 6. Finite-difference slope shading

The fallback for the 12 formulas with no derivative, and the reason
§5 says "post-pass": it needs the finished value field, not the
per-pixel loop. Same shape as the existing effects chain.

Worth noting it also gives the classic embossed look on formulas that
DO have derivatives, so it is not purely a fallback — but it is the
one that breaks under jitter, and that is a real cost if §7 lands.

## 7. Temporal anti-aliasing and spectral rendering — PLANNED, not scheduled

Sampling TIME within a frame (motion blur), and correlating the time
and wavelength dimensions so moving detail smears into colour fringes.
On a zoom everything moves radially, so the fringing is radial.

**Nothing of this exists today** — there is no motion blur, shutter or
sub-frame sampling anywhere in the video export path. It is an
export-loop feature, not a shader one: render N sub-frames per output
frame and combine them. The cheap version puts R at `t-d`, G at `t`
and B at `t+d`; the honest version samples N wavelengths, weights them
through CIE colour-matching functions and converts XYZ to sRGB, with
the time offset a function of wavelength.

Two properties this engine brings, both worth knowing before costing
it:

- **Renders are deterministic**, so temporal sampling is pure
  supersampling in time. There is no variance to fight, unlike a
  stochastic renderer where more samples buy noise reduction.
- **Sub-frames share the expensive part.** Between sub-frames of one
  output frame the view barely moves, so the reference orbit AND the
  BLA table are reused — the orbit cache already does this. N
  sub-frames of a deep zoom therefore cost far less than N independent
  deep renders, which is not true of most deep-zoom renderers, and is
  what makes motion blur affordable here at all.

It touches the animation pipeline rather than the escape engine, so it
wants its own plan when it is scheduled. Note the interaction with §5
and §6: jittered sampling is what makes finite-difference shading
noisy, so shipping this makes the analytic path the only usable one.

## Order

1. ~~**Golden spiral trap.**~~ Done 2026-08-29.
2. **Analytic normal shading** (§5). Fits the per-pixel architecture,
   11 formulas ready, and it is the shading that survives §7.
3. **λ·sin(z).** One formula, reusing an existing escape metric.
4. **Origami Butterfly.** One formula plus a seeded line hash.
5. **Finite-difference slopes** (§6). Covers the remaining 12
   formulas, as a post-pass.
6. **Turing / BZ.** Its own plan, its own render mode, and a
   measurement before any of it.

Unscheduled: **§7 temporal / spectral**, when the animation pipeline
is next opened.

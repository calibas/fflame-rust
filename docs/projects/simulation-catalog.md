# Simulation Mode — model catalogue

**Status:** Planning, 2026-09-01. No code. Companion to
[simulation-fractals.md](simulation-fractals.md) (master plan),
[simulation-pipeline.md](simulation-pipeline.md) (GPU design) and
[simulation-integration.md](simulation-integration.md) (file checklist).

Every model the seed document ([field-type-fractals.md](../archive/escape-time/field-type-fractals.md))
mentions, plus the few its sources lead to directly. For each: the
governing rule as the source states it, the discretisation this plan
ships, parameters and starting presets, which pipeline stages it uses,
how it is coloured, and what bit it.

**Source labelling is deliberate.** `[read]` means the primary text or
page was fetched and read during this planning session (the McCabe
paper, Rafler's SmoothLife paper, Gravner–Griffeath Part III, Karl
Sims' Gray–Scott page, the mrob.com xmorphia pages, and the Wikipedia
pages named). `[verify]` means the statement is from memory of the
literature and **must be checked against the cited paper before a
preset ships** — the plan does not fabricate constants and does not
want any to be shipped on trust. Nothing below is attributed to a
person or paper the session could not confirm.

Pipeline stage names (`warp`, `pyramid`, `update`, `agents`, `color`,
`resolve`, `settle`) are those of
[simulation-pipeline.md §4](simulation-pipeline.md).

---

## 0. Summary table

| # | model | bucket | stages | neighbourhood | steps to a still (order) | tier |
|---|---|---|---|---|---|---|
| 1 | Gray–Scott | RD, dense | update, color | 3×3 | 10³–10⁴ (measured) | 1 |
| 2 | FitzHugh–Nagumo | RD, dense | update | 3×3 | 10³–10⁴ | 1 |
| 3 | Brusselator | RD, dense | update | 3×3 | 10³–10⁴ | 1 |
| 4 | Oregonator (Tyson–Fife) | RD, dense, stiff | update (sub-stepped) | 3×3 | 10⁴ | 2 |
| 5 | Schnakenberg | RD, dense | update | 3×3 | 10³–10⁴ | 1 |
| 6 | Swift–Hohenberg | PDE, 4th order | update ×2 passes | 3×3 twice | 10⁴ (dt-limited) | 2 |
| 7 | Cahn–Hilliard | PDE, 4th order | update ×2 passes | 3×3 twice | 10³–10⁴ | 2 |
| 8 | Lenia | continuous CA, large kernel | update (kernel LUT) | (2R+1)², R≈13 | 10²–10³ (never stills; animate) | 2 |
| 9 | SmoothLife | continuous CA, large kernel | update (kernel LUT) | (2r_a+1)², r_a=21 | 10²–10³ (never stills) | 2 |
| 10 | McCabe multi-scale Turing | multi-scale | pyramid, update | disc averages r ≤ 64 | 10²–10³ (measured; never stills) | 2 |
| 11 | Hodgepodge (BZ CA) | integer CA | update | 3×3 | 10²–10³ (never stills) | 1 |
| 12 | Cyclic CA | integer CA | update | R-range Moore | 10²–10³ | 1 |
| 13 | Spatial rock–paper–scissors | stochastic CA | update (RNG) | 3×3 | 10³ (never stills) | 1 |
| 14 | Abelian sandpile | integer CA | update | 3×3 (von Neumann) | 10³–10⁵ rounds (bulk toppling; measure) | 2 |
| 15 | Wolfram elementary CA | 1-D CA | update (row) | 3 | = image height | 1 |
| 16 | Kobayashi phase field | PDE, coupled | update | 3×3 | 10⁴ | 2 |
| 17 | Packard digital snowflake | integer CA, hex | update | hex 6 | ≈ radius | 1 |
| 18 | Gravner–Griffeath snowfake | mass-transfer CA, hex | update | hex 6 | 10⁴–10⁵ | 3 |
| 19 | DLA | agents | agents, update (freeze) | walkers | 10⁴–10⁶ walker steps | 3 |
| 20 | Eden | stochastic growth | update (RNG) | 3×3 | ≈ radius / p | 1 |
| 21 | Ballistic deposition | stochastic growth | update (column) | 3 | ≈ height | 1 |
| 22 | Dielectric breakdown (DBM) | Laplacian growth | update (relax ×K), growth | 3×3 | 10³ growth × K relax | 4 |
| 23 | Saffman–Taylor fingering | Laplacian growth + curvature | as DBM | 3×3 | as DBM | 4 |
| 24 | Percolation clusters | static + labelling | update (label prop.) | 3×3 | O(diameter) | 1 |
| 25 | Invasion percolation | growth | update | 3×3 | ≈ steps of threshold | 2 |
| 26 | Ising (Metropolis) | stochastic CA | update (checkerboard, RNG) | 3×3 | 10²–10⁴ sweeps | 1 |
| 27 | Physarum (Jones) | agents + field | agents, update (diffuse/decay) | 3×3 + sensors | 10²–10³ (never stills) | 3 |

Tier 1 = ships with the phase-1/2 skeleton (a 3×3 stencil or a cheap
CA), Tier 2 = needs the pyramid, a kernel LUT, or sub-stepping, Tier 3
= needs the agent stage, Tier 4 = needs a per-step Laplace relaxation
or a global selection. The phase plan in the master document follows
these tiers.

"Never stills" models are animated subjects: the export contract runs
exactly `steps` steps from the seed, so a still is reproducible, but
the natural product is a video.

---

## 1. Gray–Scott

**Sources.** J. E. Pearson, "Complex patterns in a simple system",
*Science* 261 (1993) 189–192 `[verify: the paper itself; its (F,k)
map is what the class letters refer to]`. Karl Sims, "Reaction-Diffusion
Tutorial" (karlsims.com/rd.html) `[read]` — the discretisation below
is his. Robert Munafo, xmorphia / "Pearson's classification"
(mrob.com/pub/comp/xmorphia) `[read]` — the equations and the class
table.

**Equations** (Pearson's form as given on xmorphia):

```
∂u/∂t = D_u ∇²u − u v² + F (1 − u)
∂v/∂t = D_v ∇²v + u v² − (F + k) v
```

u is the substrate ("A" in Sims), v the autocatalyst ("B"). F is the
feed rate, k the kill rate. Pearson's own parameters `[verify]`:
D_u = 2·10⁻⁵, D_v = 10⁻⁵ on a domain of size 2.5 with a 256² grid.

**Discretisation shipped** (Sims): D_A = 1.0, D_B = 0.5, dt = 1.0,
Laplacian as the 3×3 stencil with centre −1, edge 0.2, corner 0.05
(sums to zero; isotropic to second order). Cell size is 1. **Clamp
both fields to [0, 1] after every step** — measured in the NumPy
prototype: without the clamp an overshoot below zero feeds v² with the
wrong sign and the field is NaN within a few thousand steps at some
(F,k). The GPU kernel does the same.

**Parameters.** `F` (0–0.1), `k` (0.03–0.07), `dA` (0.5–1.5), `dB`
(0.1–1.0), `dt` (0.5–1.0). Only F and k are worth animating.

**Presets.** Two pairs are verified against Sims' page `[read]`:
"mitosis" F = 0.0367, k = 0.0649 and "coral growth" F = 0.0545,
k = 0.062. The prototype also ran three pairs taken from the mrob
class list `[read, but re-check each letter's pair before naming the
preset after a Pearson class]`: (0.030, 0.057), (0.046, 0.065),
(0.010, 0.041). Ship the whole Pearson row as presets once each pair
is re-read from the page, and name them by what they *do* (mitosis,
coral, worms, spots, spirals) rather than by Greek letter, since the
class boundaries are fuzzy at Sims' constants.

**Prototype findings** (NumPy, 256², six 24-px square seeds of v = 1,
periodic; `output/sim_proto/gs_sheet.png`, rows 500/5000/10000
steps):

| preset | 500 | 5000 | 10000 | note |
|---|---|---|---|---|
| mitosis (0.0367, 0.0649) | dividing spots | spot lattice growing | hexagonal spot field | textbook |
| coral (0.0545, 0.062) | branching fronts | labyrinth | labyrinth, still growing | textbook |
| (0.030, 0.057) | stripes + holes | stripe/hole maze | maze, filled | Turing stripes with negative spots |
| worms (0.046, 0.065) | worm tips | worms | worms, filled | textbook |
| (0.010, 0.041) | growing blobs | **blank** | **blank** | v died to 0 — this class (chaotic "spirals" on xmorphia) needs noisy full-field seeding, not blobs |

Cost: ~1250 µs/step in NumPy at 256² (irrelevant on the GPU; recorded
only to show what the prototype paid). Settle: the "mean |Δv| < 10⁻⁴
for 200 steps" criterion fired at 100–200 steps for coral/worms and
~2100 for the (0.030, 0.057) pair, but the images show the patterns
still *growing into empty field* at 5000 — the metric measures local
quiescence, not completion. **Sizing rule for the driver: thousands
of steps for a still at 256², scaling with grid size for growth
patterns (the front moves at a fixed speed in cells/step); tens of
steps per frame for interaction.**

Seeds matter: 12-px blobs died at mitosis; 24-px survived at every
pair tried. Ship `Blobs{count: 6, radius: 12}` (radius, so 24 px) as
the Gray–Scott default init and `Noise` for the chaotic classes.

**Stages.** `update` (one pass, two channels in `.xy`), `color`.
Boundary default Periodic.

**Colouring.** `channel` on v (the classic), `two_channel` (u → hue,
v → value), `age` (steps since v last crossed 0.25 — gives the
"growth rings" look), `hillshade` on v.

---

## 2. FitzHugh–Nagumo

**Sources.** R. FitzHugh, *Biophys. J.* 1 (1961) 445; J. Nagumo et al.,
*Proc. IRE* 50 (1962) 2061 `[verify]`. Reaction–diffusion form and
constants from Wikipedia "FitzHugh–Nagumo model" `[read]`.

**Equations** (Wikipedia's form, with diffusion added to both):

```
∂v/∂t = D_v ∇²v + v − v³/3 − w + I_ext
τ ∂w/∂t = D_w ∇²w + v + a − b w
```

with a = 0.7, b = 0.8, τ = 12.5 `[read]`. I_ext is the drive; the
excitable regime is I_ext ≈ 0.5 with a stable rest state, and a
localised kick launches a travelling pulse; spirals appear when a
pulse is broken (cut a wavefront with a line seed).

**Discretisation.** Explicit Euler, 3×3 Laplacian (Sims weights),
dt = 0.05–0.1 (the v-equation has unit timescale, so dt ≤ 0.1 is safe
for D_v ≤ 1), D_v = 1, D_w = 0 (the classic excitable-medium choice)
or D_w = D_v/… for Turing patterns. Clamp v to [−3, 3].

**Parameters.** `a`, `b`, `tau`, `I` (drive), `D_v`, `D_w`, `dt`.

**Measured 2026-09-03** (`scripts/sim_prototypes/proto_reaction_diffusion.py`,
256², NumPy):

- **`excitable_spiral` VERIFIED and shippable**: a = 0.7, b = 0.8,
  τ = 12.5, I = 0.5, D_v = 1, D_w = 0, dt = 0.1, **broken-wave seed**.
  Produces textbook counter-rotating spiral pairs: the cut wavefront's
  tips have begun to curl by step 1,000 and the spirals are fully
  formed by 4,000 (a first draft of this note said "by 1,000" without
  looking at that frame). Never stills (spatial sd 0.76 and rotating)
  — an animated subject; a still export wants `steps` ≈ 4,000 at
  dt = 0.1.
- The seed is not incidental: the same constants from a **noise** seed
  relax to the rest state (sd 0.0014, a flat field). The excitable
  regime needs a cut wavefront, so `seed_kind` must default to
  `broken_wave` for this preset.
- **Shipped 2026-09-04** as the `spiral` preset, which carries
  `SimInit::BrokenWave` as well as its numbers — a preset of numbers
  alone would render the flat field below. The GPU port reproduces the
  prototype's counter-rotating pair.
- **The Turing/labyrinth guess DOES NOT WORK and must not ship.**
  D_w = 4 with I = 0 from noise gives a spatial sd of **0.0000** — a
  perfectly flat field after 4000 steps. The catalogue's own
  instruction ("do not ship a preset without running it") was right;
  the constant set for an FHN Turing regime is still unknown.
- **dt cap = 0.75, measured on the spiral.** A first probe ran from a
  noise seed, which relaxes to rest — so it measured the stability of a
  field doing nothing and reported 0.5. Re-run on the active spiral
  for 200 units of model time: the [−3, 3] clamp means instability
  never shows as a NaN, it shows as cells pinned to the rails, and
  **0.0% of cells rail at every dt through 0.75** (spatial sd steady at
  0.36–0.41), while **dt = 1.0 rails 14.6%** of them. Cap the slider at
  0.75 and do not rely on divergence detection here.

**Stages.** `update`, `color`. Periodic or Clamp.
**Colouring.** `channel` on v, `two_channel`, `age` (time since last
excitation — spirals become rainbow rings).

---

## 3. Brusselator

**Sources.** I. Prigogine, R. Lefever, "Symmetry breaking instabilities
in dissipative systems II", *J. Chem. Phys.* 48 (1968) 1695 `[verify]`.
Wikipedia "Brusselator" `[read]`.

**Equations.**

```
∂X/∂t = D_X ∇²X + A − (B + 1) X + X² Y
∂Y/∂t = D_Y ∇²Y + B X − X² Y
```

Fixed point (X, Y) = (A, B/A). Hopf instability (oscillations) when
B > 1 + A² `[read]`. Turing instability (stationary patterns) when
B > (1 + A √(D_X / D_Y))² with D_Y > D_X `[verify — standard result,
check the sign convention before shipping the tooltip]`.

**Discretisation.** Explicit Euler, dt ≈ 0.01–0.05 (the reaction terms
are O(B) ≈ 3–5, so dt ≤ 0.05), D_X = 1, D_Y = 4–8 for Turing spots.
Clamp X, Y ≥ 0.

**Parameters.** `A` (1–3), `B` (1–5), `D_X`, `D_Y`, `dt`.

**Measured 2026-09-03**, both presets VERIFIED as patterns:

- **Turing spots** A = 1, B = 3, D_X = 1, D_Y = 8, dt = 0.01 — spots,
  **settles at 1,180 steps**, spatial sd 1.43. Shipped 2026-09-04 as
  the `turing_spots` preset; the GPU port reproduces the prototype's
  fine 2–3 cell wavelength. (A first run reported
  4,960: its settle window was counted in 20-step samples but reported
  in steps, so it demanded 4,000 quiet steps and then dated the still
  ~3,800 late. Every settle figure from that run was inflated by about
  that much.)
- **Oscillating** A = 1, B = 2.5, D_X = D_Y = 1 — never stills, as
  claimed.
- **dt cap is 0.04.** The full ladder: 0.01, 0.02, 0.03 and 0.04 all
  stable with identical spatial sd (1.43); **0.05 diverges at step
  90**, 0.1 at 35, 0.25 at 15. (A first draft of this note said 0.02
  after testing only 0.01 and 0.05 — a cap written down without running
  the rung it names. The rungs are now all run.) The upper end of the
  "0.01–0.05" estimate was one rung too far.
- Wavelength caveat below applies: at these constants the spots are
  ~2–3 cells across on a 256² grid — a real Turing pattern that reads
  as speckle.

**Stages.** `update`, `color`. **Colouring.** `two_channel`.

---

## 4. Oregonator (Tyson–Fife two-variable form)

**Sources.** R. J. Field, R. M. Noyes, *J. Chem. Phys.* 60 (1974) 1877
(the Oregonator); J. J. Tyson, P. C. Fife, *J. Chem. Phys.* 73 (1980)
2224 (the two-variable reduction) `[verify — Scholarpedia refused the
connection during this session; the equations below are from memory
and must be checked against Tyson–Fife or the Scholarpedia
"Oregonator" article]`.

**Equations** (as remembered):

```
ε ∂u/∂t = D_u ∇²u + u (1 − u) − f v (u − q) / (u + q)
  ∂v/∂t = D_v ∇²v + u − v
```

u is the activator (HBrO₂), v the oxidised catalyst. ε ≈ 0.01–0.1,
q ≈ 0.002, f ≈ 1–3; spiral waves for f ≈ 1.4, ε ≈ 0.04 `[verify]`.

**Discretisation.** Stiff in u (timescale ε): explicit Euler needs
dt ≈ ε/10 for the reaction and the diffusion limit dt ≤ 0.25 h²/D_u.
Ship as **sub-stepped**: `substeps` per `update` dispatch (a uniform
loop inside the kernel, reading its own stencil each sub-step is not
possible in one dispatch — so sub-stepping is *K dispatches per
displayed step*; the driver's step batch covers it). Tier 2 for that
reason. Guard u ≥ 0 and the denominator (u + q > 0 always if u ≥ 0).

**Parameters.** `epsilon`, `q`, `f`, `D_u`, `D_v`, `dt`.
**Stages.** `update` ×K, `color`. **Colouring.** `two_channel`, `age`.

---

## 5. Schnakenberg

**Sources.** J. Schnakenberg, *J. Theor. Biol.* 81 (1979) 389
`[verify]`.

**Equations** `[verify]`:

```
∂u/∂t = D_u ∇²u + a − u + u² v
∂v/∂t = D_v ∇²v + b − u² v
```

Fixed point u = a + b, v = b/(a+b)². Turing spots for a = 0.1, b = 0.9
with D_v/D_u ≈ 40 `[verify by prototype]`.

**Discretisation.** As the Brusselator; dt ≈ 0.01 with D_v large.
**Parameters.** `a`, `b`, `D_u`, `D_v`, `dt`. **Stages.** `update`,
`color`. **Colouring.** `channel` on u.

**Measured 2026-09-03.** a = 0.1, b = 0.9, D_u = 1, D_v = 40,
dt = 0.01 **VERIFIED**, shipped 2026-09-04 as the `turing_spots`
preset; the GPU port reproduces the prototype's ~6-cell wavelength.
Turing spots, **settles at 4,900 steps**,
spatial sd 1.17 (an independent run of the same parameters in the
wavelength sweep gave 5,100; a first run reported 8,680 through the
settle-window bug described under the Brusselator). **dt cap 0.02**, with every rung run: 0.01 and 0.02
stable, **0.03 diverges at step 486**, 0.04 at 26, 0.05 at 17.

### Turing feature size costs steps quadratically in wavelength (measured, after a retraction)

The catalogue's spot presets put the wavelength at a few cells, which
is a genuine Turing pattern that reads as speckle. Wavelength scales as
√D, so it is bought by scaling both diffusion constants — and explicit
Euler then caps dt at ~const/D, so the step count should scale linearly
with the scale factor. **It does.** Schnakenberg, 256², scaling D_u and
D_v by k and dt by 1/k, settle criterion held fixed in *model* time
(tolerance ∝ dt, window ∝ 1/dt) with an amplitude floor:

| D scale k | dt | steps to still | λ (cells) | settle in model time |
|---|---|---|---|---|
| 1 | 0.0100 | 5,100 | 6.9 | 51 |
| 4 | 0.0025 | 19,150 | 16.0 | 48 |
| 16 | 0.00063 | 93,150 | 32.0 | 58 |

Model time to settle is ~50 in every row, which is the tell: scaling D
and dt together is a change of length unit and nothing else, so the
dynamics are identical and the step count is simply ∝ k. **4.6× the
wavelength costs 18× the steps.** Feature size is not a cheap knob; a
preset that wants 32-cell spots at 256² pays ~10⁵ steps for a still,
and the alternative — a smaller grid upscaled by the resolve stage — is
the same picture for 1/16 of the work.

**A retraction.** A first version of this section reported "5.6× the
feature size for 2× the steps" and called feature size cheap. That
came from a settle criterion whose tolerance was on the per-*step*
change while dt shrank by k, so it loosened by k in rate terms and its
window shrank by k in model time — the larger-D runs were being
declared still 15× sooner than the small-D run by the same standard.
The corrected criterion above gives the linear result the physics
predicts. The wrong version stood for one commit.

One measurement trap worth carrying into the driver: the **settle
metric fires during the slow nucleation phase**, before the pattern
exists. Unguarded, it reported convergence at step 20 on a Schnakenberg
run that went on to form a full pattern, and at 10,000 steps on a
blank field at k = 16. The prototype now requires an amplitude floor
before accepting stillness, and `settle` in the shader needs the same
guard or it will stop growth models early. (Gray–Scott showed the same
failure from the other direction.)

---

## 6. Swift–Hohenberg

**Sources.** J. Swift, P. C. Hohenberg, "Hydrodynamic fluctuations at
the convective instability", *Phys. Rev. A* 15 (1977) 319 `[verify]`.
Wikipedia "Swift–Hohenberg equation" `[read]`.

**Equation.**

```
∂u/∂t = r u − (q₀² + ∇²)² u + N(u)
N(u) = −u³            (stripes / rolls)
N(u) = g u² − u³      (hexagons for g ≠ 0)
```

The textbook form has q₀ = 1; the plan exposes q₀ because at cell size
1 that gives a 2π ≈ 6.3-pixel wavelength, too fine to be attractive.
Pattern wavelength λ = 2π/q₀.

**Discretisation.** (q₀² + ∇²)² needs the Laplacian of the Laplacian
— two `update` passes per step (`passes = 2`, pipeline §4.3): pass 1
writes w = q₀² u + ∇²u into a spare channel, pass 2 reads the 3×3
stencil of w. Explicit Euler stability: with the 5-point Laplacian
the eigenvalues of −∇² lie in [0, 8] (h = 1), so the linear operator
is bounded by (8 − q₀²)² and **dt ≤ 2 / (8 − q₀²)²** — for λ = 16 px
(q₀² ≈ 0.154) that is dt ≲ 0.03. With r ≈ 0.1–0.3 the pattern grows
on a 1/r timescale, so a still needs ~10³ time units = **tens of
thousands of steps**. That is the cost of doing this explicitly; the
standard remedy is a semi-implicit spectral step, which is parked with
the FFT (pipeline §3). Tier 2, and honest about it in the tooltip.

**Parameters.** `r` (−0.5–1), `q0`, `g` (0–1), `dt`. Seed: `Noise`
(amplitude 0.1).
**Presets** `[verify by prototype]`: stripes r = 0.2, g = 0, q₀ =
2π/16; hexagons r = 0.1, g = 1.0.
**Stages.** `update` ×2, `color`. **Colouring.** `channel` (diverging
palette about 0), `hillshade`.

**Measured 2026-09-04** (`proto_pde.py`, 256², 5-point Laplacian),
shipped 2026-09-05. Three of the four things above were wrong.

- **The dt bound is CONFIRMED**: 2/(8 − q₀²)² = 0.0325 at λ = 16, and
  the ladder is stable at 0.03249 and diverges at 0.03574.
- **λ = 2π/q₀ is CONFIRMED** — the claim most at risk from the
  discretisation, since the Sims kernel the other models use is a
  Laplacian scaled by 0.3 and would have put the wavelength 83% out.
  Measured 16.5 cells against 16.0, with a spectral peak 21× the mean.
  This is why the model uses the **5-point** Laplacian; there is a
  note about it in the source and the prototype.
- **r = 0.2 is REFUTED and the parameterisation with it.** The
  band-pass is only as selective as q₀⁴: growth at the band is r,
  growth at the uniform mode is r − q₀⁴, and q₀⁴ = 0.024 at λ = 16. At
  r = 0.2 the ratio is 8.4, the uniform mode grows nearly as fast as
  the pattern, and the cubic quenches the field into ~100-cell blobs
  with no pattern at all — an attractive picture, and not this model.
  Every ratio from 0.1 to 4 gives a clean 16.5-cell pattern. The
  textbook q₀ = 1 hides the problem because q₀⁴ = 1 swamps any
  sensible r. **So the shipped model exposes `wavelength` (cells) and
  a RELATIVE `drive` = r/q₀⁴**, which is the same equation with the
  drive measured in units of the band's own selectivity — one slider
  position then means the same thing at every wavelength, which a
  literal r cannot (at λ = 64, q₀⁴ = 9.3e-5).
- **The hexagon preset is REFUTED and not shipped.** g = 1.0 does not
  give hexagons; it gives a uniform field. The quadratic g·u² competes
  with the cubic at the pattern's own amplitude ~√r, so against
  r = 0.05 a g of 1 is not a symmetry-breaking nudge but the dominant
  term, and it drives the field to the uniform fixed point near u = g.
  Sweeping g at two drives: skew (the discriminator — a hexagonal
  lattice's three modes at 120° give a skewed one-point distribution,
  stripes give 0) runs 0.00 → −0.04 → −0.11 → −0.23 → −0.43 → −1.85
  for g = 0 … 0.3, and past ~0.35 the field goes uniform (sd 0.0000).
  A focused hunt near onset — eight (drive, g) pairs at 16,000 steps —
  produced spots-and-worms mixtures, never an ordered hexagonal
  lattice. SH's hexagons are subcritical: they coexist with the flat
  state and need nucleation or a far longer anneal, not a noise seed.
  **Shipped as `spots` (g = 0.25, skew −1.02), named for what it
  actually produces**, alongside `labyrinth` (g = 0). The slider stops
  at 0.35.
- Settles at 4,600 steps (labyrinth) and 5,900 (spots) at 256².
  Wavelength costs steps steeply: r ∝ q₀⁴, so doubling the wavelength
  is 16× the steps — at 12,000 steps λ = 10 reaches sd 0.45 and
  λ = 32 only 0.012, still seed noise.

---

## 7. Cahn–Hilliard

**Sources.** J. W. Cahn, J. E. Hilliard, "Free energy of a nonuniform
system. I. Interfacial free energy", *J. Chem. Phys.* 28 (1958) 258
`[verify]`. Wikipedia "Cahn–Hilliard equation" `[read]`.

**Equation.**

```
∂c/∂t = D ∇² ( c³ − c − γ ∇²c )
```

c ∈ [−1, 1] is the composition; γ sets the interface width √γ;
domains coarsen with the Lifshitz–Slyozov law L ∝ t^{1/3} `[read]`.

**Discretisation.** Two passes: pass 1 writes the chemical potential
μ = c³ − c − γ∇²c into a spare channel, pass 2 applies D∇² to μ.
Stability (same eigenvalue argument as §6): **dt ≤ 1 / (32 D γ)** for
h = 1 — with D = 1, γ = 0.5, dt ≤ 1/16. Conservative: the mean of c is
exactly preserved by the stencil (it is a discrete divergence), which
is worth asserting in a test. Clamp c to [−1.2, 1.2] only as a NaN
guard; the dynamics should never need it.

**Parameters.** `D`, `gamma`, `dt`, `mean` (initial mean composition:
0 gives labyrinths, ±0.4 gives droplets of the minority phase). Seed:
`Noise` about `mean`.
**Stages.** `update` ×2, `color`. **Colouring.** `channel` (two-tone),
`hillshade` (the interfaces read as relief).

**Measured 2026-09-04** (`proto_pde.py`, 256²), shipped 2026-09-05.

- **The dt bound above is REFUTED, and it fails slowly enough to look
  right.** dt ≤ 1/(32 D γ) = 0.0625 keeps only the γ∇⁴ term; the
  cubic's contribution is the same order. Linearising about |c| = 1
  gives the symbol `D L (3c² − 1 − γL)` over L ∈ [−8, 0], most
  negative at the checkerboard: **dt ≤ 2/(D(16 + 64γ))** = 0.0417 at
  the defaults. Measured: stable at 0.041667, diverges at 0.045833 —
  the formula is exact. The old bound's failure is what makes this
  worth writing down: at dt = 0.05625 the run is **finite for 400
  steps and infinite by 1,000**, so a short ladder calls it stable.
  The first version of this prototype used a 400-step ladder and did
  exactly that; the ladder now runs 4,000.
- **Mean composition conserved**: 1.2e-16 over 40,000 steps in f64,
  and 3e-9 on the GPU in f32 over 4,000. The update is a discrete
  divergence, so this is a property no picture can fake — a field that
  slowly gains material still separates into plausible domains. It is
  the model's GPU test.
- **Coarsening measured**, domain size (first moment of the structure
  factor) at mean 0: 6.2 cells at step 200, 8.5 at 1,000, 13.2 at
  5,000, 18.6 at 20,000, 22.4 at 40,000 — an exponent of **0.25**
  against the Lifshitz–Slyozov 1/3. The shortfall is expected at this
  size (22 cells in a 256 box is into finite-size effects) and is
  recorded rather than explained away. Droplets coarsen more slowly
  still, 0.14 over the same range.
- Never stills: it coarsens forever, so `steps` is a choice of how
  coarse. Shipped presets use 20,000. c stays inside [−1.03, 1.02], so
  the ±4 clamp in the kernel is a NaN guard that never binds.

---

## 8. Lenia

**Sources.** B. W.-C. Chan, "Lenia: Biology of Artificial Life",
*Complex Systems* 28(3) (2019) 251–286 `[verify the page numbers]`;
formulas as stated on Wikipedia "Lenia" `[read]`.

**Rule** (Wikipedia's statement of Chan's continuous update):

```
A^{t+dt}(x) = clip( A^t(x) + dt · G( (K ∗ A^t)(x) ), 0, 1 )
```

with the kernel K a radially symmetric ring built from a core K_C on
the unit interval, scaled to radius R and normalised to sum 1:

```
K_C(r) = exp( α − α / (4 r (1 − r)) ),  α = 4          (exponential core)
K(x)   = K_C(|x| / R) / Σ K_C          (single-ring case; multi-ring uses per-ring peaks β_i)
```

and the growth mapping a Gaussian bump:

```
G(u) = 2 · exp( −(u − μ)² / (2 σ²) ) − 1
```

Conway's Life is recovered with the appropriate step kernel and
μ = 0.35, σ = 0.07 `[read]`. Orbium (the first Lenia glider): R = 13,
μ = 0.15, σ = 0.015 (Chan gives T = 10, i.e. dt = 0.1) `[verify the
exact σ against Chan's table — the session did not confirm the last
digit]`.

**Discretisation.** Direct gather: the kernel is precomputed on the
CPU into a (2R+1)² weight LUT (a small Rgba32Float or a storage
buffer; **not** the pyramid — a ring is not a box), and the `update`
kernel sums the window. Cost is (2R+1)² taps per cell per step:

| R | taps | 512² taps/step | 1024² |
|---|---|---|---|
| 13 | 729 | 1.9·10⁸ | 7.6·10⁸ |
| 21 | 1849 | 4.8·10⁸ | 1.9·10⁹ |
| 32 | 4225 | 1.1·10⁹ | 4.4·10⁹ |

At ~10¹¹ cached taps/s on a desktop GPU that is a few ms per step at
512² for R = 13 and tens of ms at 1024², which matches the interactive
budget in pipeline §5. Above R ≈ 32 the FFT is the right tool and is
parked. Boundary Periodic (Lenia's convention).

**Parameters.** `R` (5–32, integer), `mu`, `sigma`, `dt` (= 1/T),
`rings` (1–3, integer) and per-ring peaks `beta1..3`, `kernel`
(choices: exponential, polynomial, rectangular — `[verify]` the
polynomial and rectangular core formulas against Chan before adding
them).
**Presets.** Orbium (above, once σ is verified), Life (μ = 0.35,
σ = 0.07, R = 1 rectangular).
**Seeds.** `Noise` produces a soup; the Orbium *creature* needs its
specific initial pattern (a 20×20 array in Chan's supplement) —
ship it as a `Pattern` init variant later, not in phase 1.

**Stages.** `update` (LUT gather), `color`. **Colouring.** `channel`
with the Lenia-style palette, `age`.

---

## 9. SmoothLife

**Sources.** S. Rafler, "Generalization of Conway's 'Game of Life' to
a continuous domain — SmoothLife", arXiv:1111.1567 (2011) `[read]`.

**Rule** (verbatim structure from the paper): with f the field, r_i
the inner radius and r_a = 3 r_i the outer,

```
m = (1/M) ∫_{|x|<r_i} f          inner disc average ("cell" filling)
n = (1/N) ∫_{r_i<|x|<r_a} f      annulus average   ("neighbourhood")
σ(x, a, α)      = 1 / (1 + exp(−(x − a) · 4/α))
σ_n(x, a, b)    = σ(x, a, α_n) · (1 − σ(x, b, α_n))
σ_m(x, y, m)    = x · (1 − σ(m, 0.5, α_m)) + y · σ(m, 0.5, α_m)
s(n, m)         = σ_n( n, σ_m(b₁, d₁, m), σ_m(b₂, d₂, m) )
```

Discrete time: f' = s(n, m). Smooth time: ∂f/∂t = 2 s(n, m) − 1, or
Rafler's alternative f' = f + dt (s(n, m) − f) `[read — both forms
appear in the paper; ship the second, it is the one that stays in
[0,1]]`. Anti-alias the disc edges over a band of width b = 1 (the
weight ramps linearly across the boundary pixel) `[read]`.

**Glider parameters** `[read]`: r_a = 21 (r_i = 7), b₁ = 0.278,
b₂ = 0.365, d₁ = 0.267, d₂ = 0.445, α_n = 0.028, α_m = 0.147.

**Discretisation.** Two LUT gathers per cell (disc and annulus) of
(2r_a+1)² = 1849 taps at r_a = 21 — same cost class as Lenia R = 21
(table in §8). Both averages come from one window pass with two
accumulators. Periodic.

**Parameters.** `ri` (integer), `b1`, `b2`, `d1`, `d2`, `alpha_n`,
`alpha_m`, `dt`, `mode` (choices: discrete, smooth).
**Presets.** Rafler's glider set (above). **Seeds.** `Noise` at 0.5
density with blobs; the soup organises itself within ~100 steps.
**Stages.** `update`, `color`. **Colouring.** `channel`, `age`.

---

## 10. McCabe multi-scale Turing patterns

**Sources.** J. McCabe, "Cyclic Symmetric Multi-Scale Turing Patterns",
*Bridges 2010* proceedings `[read — text at output/mccabe.txt]`.
Jason Rampe (Softology) "Multi-Scale Turing Patterns" blog posts
`[verify — the radius ladders and colour blending in circulation are
from implementations, not from the paper]`. Jonathan Reusser's
implementation `[verify — the seed document's quotation of Reusser is
not verbatim; his kernels are analytic in the frequency domain inside
an FFT pipeline]`.

**Rule** (as the paper states it; single field f ∈ [−1, 1]):

For each scale i with activator radius r_a,i < inhibitor radius r_b,i
and step amount s_i:

```
a_i = disc average of f over radius r_a,i
b_i = disc average of f over radius r_b,i
v_i = |a_i − b_i|                       ("variation"; the paper also allows averaging v_i over a small radius)
```

Choose at each pixel the scale with the **smallest** v_i; then

```
f ← f + s_i   if a_i > b_i,   else   f ← f − s_i
```

and renormalise f to [−1, 1] over the whole field every step (a global
min/max — a reduction, pipeline §4.7 supplies it). **Cyclic symmetry**
(the paper's contribution): replace a_i and b_i by their average with
copies rotated about the image centre by 2πk/n, k = 1..n−1, before
comparing. The paper gives **no radius table** and does not describe
colouring; the "colour by winning scale" look is from later
implementations.

**Discretisation.** Disc averages from the `pyramid` stage: a
power-of-two box average at each mip level, trilinear between levels
for other radii (pipeline §4.2) — a box, not a disc. **Open item for
the phase-3 prototype: compare box vs. disc averages visually at one
ladder; the NumPy prototype used exact FFT discs.** Symmetry: the
rotated reads are applied *in the pyramid reads* (n rotated samples
per scale per pixel; pipeline §4.2), so n-fold symmetry costs n× the
gather.

**Prototype findings** (NumPy, 256², five scales
(1,2,.05) (2,4,.04) (4,8,.03) (8,16,.02) (16,32,.01), uniform noise
seed; `output/sim_proto/mccabe_*.png`): the characteristic nested
texture is present by step 20 and fully developed by 100; 5-fold
symmetry produces the paper's rosettes. 9.0 ms/step plain and 101
ms/step with 5-fold symmetry in NumPy (FFT discs; the symmetry cost is
the nearest-neighbour rotation in Python, not intrinsic). The field
never stills — it is an animated subject; a "still" is `steps` from
the seed.

**Parameters.** `scales` (1–6, integer), per scale `ra_i`, `rb_i`,
`amount_i` (or a generator: base radius, ratio r_b/r_a = 2, amount
falloff), `symmetry` (0–8, integer), `variation_blur` (0–2).
**Presets.** The prototype ladder; a coarse ladder (8..128); symmetric
5 and 7.
**Stages.** `pyramid`, `update`, `settle` (min/max reduction),
`color`. Periodic.
**Colouring.** `scale_mix` (hue = winning scale, value = f — the
Softology look), `channel`, `hillshade`.

---

## 11. Hodgepodge machine (Belousov–Zhabotinsky CA)

**Sources.** M. Gerhardt, H. Schuster, "A cellular automaton describing
the formation of spatially irregular structures in excitable media",
*Physica D* 36 (1989) 209–221 `[verify]`; M. Gerhardt, H. Schuster,
J. J. Tyson, "A cellular automaton model of excitable media including
curvature and dispersion", *Science* 247 (1990) 1563 `[verify]`. The
rule variants below were taken from a secondary page during the
session; `[verify against Gerhardt–Schuster 1989 before shipping]`.

**Rule** (states 0 … q; 0 healthy, q ill, 1..q−1 infected; Moore
neighbourhood; A = number of infected neighbours, B = number of ill
neighbours, S = sum of the states of the cell and its neighbours):

```
healthy:   s' = ⌊A / k₁⌋ + ⌊B / k₂⌋
infected:  s' = ⌊S / (A + B + 1)⌋ + g
ill:       s' = 0
s' = min(s', q)
```

A second variant ("bz2" on the page read) uses ⌊S/(A+1)⌋ + g for the
infected rule `[verify]`. Spirals reported for q = 200, k₁ = 2,
k₂ = 3, g = 70 `[verify — secondary source]`. Seed: uniform random
states.

**Discretisation.** Integer state in a u32 channel (bitcast into the
Rgba32Float, pipeline §3.1) or a float channel holding an integer;
3×3 gather; the sum S and counts A, B in one pass.

**Parameters.** `q` (integer), `k1`, `k2`, `g` (integers), `variant`
(choices: gerhardt_schuster, bz2).

**Measured 2026-09-03** (`proto_cellular_automata.py`, 256²): the
secondary-source parameters **q = 200, k₁ = 2, k₂ = 3, g = 70 are
CONFIRMED** — a dense field of BZ spirals and scrolls from a uniform
random seed, 129 of 201 states occupied. Shipped 2026-09-04 as the
`spirals` preset. **Not** developed by step 50
(one dominant state with scattered specks — checked, after a first
draft of this note claimed otherwise without looking); fully spiralled
by step 200. Never stills (churn plateau 0.97, i.e. almost every cell
changes every step), and the churn metric cannot date its development
because churn is ~1 from the first step — the images have to.
The source attribution stays `[verify]` — running the rule confirms the
rule, not the citation.
**Stages.** `update`, `color`. Periodic.
**Colouring.** `channel` on s/q through the palette (the classic
rainbow spirals), `age`.

---

## 12. Cyclic cellular automaton

**Sources.** R. Fisch, J. Gravner, D. Griffeath, "Threshold-range
scaling of excitable cellular automata", *Statistics and Computing* 1
(1991) 23–39 `[verify]`; D. Griffeath's Primordial Soup Kitchen pages
`[unreachable this session]`; Wikipedia "Cyclic cellular automaton"
`[read]`.

**Rule.** N states; a cell in state s advances to (s + 1) mod N when at
least T of its neighbours within range R (Moore or von Neumann) are in
state (s + 1) mod N; otherwise it stays. Written R/T/N in the
Fisch–Gravner–Griffeath notation. From random initial states the
system passes through *debris* → *droplets* → *spirals* (the spiral
cores are "demons") `[read]`. The basic 1/1/14 (R = 1, T = 1, N = 14,
von Neumann) is the textbook case `[read]`.

**Discretisation.** Integer state; range-R gather (R ≤ 5 → ≤ 121
taps). Periodic.
**Parameters.** `N` (3–24), `R` (1–5), `T` (1–R²), `neighbourhood`
(choices: moore, von_neumann).
**Presets.** 1/1/14 von Neumann; 1/3/3 Moore ("313"); R = 2..3
"turbulent" sets `[verify each on the Wikipedia/MCell lists]`.

**Measured 2026-09-03**, both shipped 2026-09-04 as presets.
**1/1/14 von Neumann CONFIRMED**: the textbook
debris → droplets → spirals sequence, fully spiralised by ~300 steps
(churn plateau 0.99), giving the characteristic 45° diamond fronts of a
range-1 von Neumann neighbourhood. All 14 states survive. **1/3/3
Moore** develops far faster — ~7 steps — and is much quieter (churn
0.12), so the two want very different default `steps`. Neither stills;
per-model defaults are the right call (open question 7).
**Stages.** `update`, `color`. **Colouring.** `channel` on s/N
(cyclic palette).

---

## 13. Spatial rock–paper–scissors

**Sources.** T. Reichenbach, M. Mobilia, E. Frey, "Mobility promotes and
jeopardizes biodiversity in rock–paper–scissors games", *Nature* 448
(2007) 1046 `[verify]` — the lattice May–Leonard model whose spirals
are the famous images. The simple CA below is folklore; no attribution
is claimed for it.

**Rule (simple stochastic CA).** Three species plus empty. Each step,
each cell picks a random neighbour: if the neighbour's species beats
the cell's (R > S, S > P, P > R) the cell takes the neighbour's
species with probability p_sel; an empty cell takes a random
non-empty neighbour's species with probability p_rep. Reichenbach's
version adds mobility (exchange with a random neighbour at rate ε),
which is what controls spiral wavelength `[verify]`.

**Discretisation.** Integer state; 3×3 gather; per-cell RNG (PCG from
`rng.wgsl`, seeded by cell index and step — deterministic per seed).
Note the parallel synchronous update differs from the sequential
random-site Monte Carlo of the paper; spirals form either way.
**Parameters.** `p_sel`, `p_rep`, `mobility`, `species` (3 or 5 —
five-species RPS-lizard-Spock forms two-level spirals).

**Measured 2026-09-03**, shipped 2026-09-04. p_sel = p_rep = 1,
three species, synchronous
parallel update: developed by ~27 steps, churn plateau 0.15, and **all
three species coexist** — the biodiversity the model is about survives
the synchronous update, which the discretisation note flagged as a
difference from the paper's sequential Monte Carlo.
**Stages.** `update` (RNG), `color`. **Colouring.** `channel` with a
categorical palette.

---

## 14. Abelian sandpile

**Sources.** P. Bak, C. Tang, K. Wiesenfeld, "Self-organized
criticality: an explanation of 1/f noise", *Phys. Rev. Lett.* 59
(1987) 381 `[verify]`. W. Pegden, C. K. Smart, "Convergence of the
Abelian sandpile", *Duke Math. J.* 162 (2013) 627 `[verify]` — the
scaling limit of the N-grain pile. Wikipedia "Abelian sandpile model"
`[read]`.

**Rule.** Integer heights h(x) ≥ 0 on the square lattice. A site with
h ≥ 4 topples: h(x) −= 4 and each of its four von Neumann neighbours
gains 1 (grains falling off the edge are lost). The final stable
configuration is independent of the toppling order (the Abelian
property) `[read]`. The picture: N = 2ᵏ grains dropped on one site,
stabilised, coloured by height 0–3 — the Pegden–Smart limit shape
with its fractal Sierpiński-like triangles.

**Discretisation.** Because order does not matter, topple **in bulk
and in parallel**: each site with h ≥ 4 sends ⌊h/4⌋ to each
neighbour in one round (legal: it is ⌊h/4⌋ consecutive single
topplings). Per round: read own h and the four neighbours' ⌊h/4⌋
(gather, no atomics). **Round count is the open cost question** —
mass must propagate radially, so rounds ≥ radius (≈ √N·c) and in
practice far more; the plan measures it in a NumPy prototype before
committing to a step budget (a 2²⁰-grain pile is the target
picture). Terminates exactly: `settle` fires when no site has h ≥ 4
(a max reduction over ⌊h/4⌋).

**Parameters.** `grains` (2¹⁰–2²⁴, log slider), `topple_at` (4;
also 8 with Moore neighbours for the variant), `sink` (choices:
edges, none-periodic — the periodic torus never stabilises with a
positive mean height above 3, so gate it).

**Measured 2026-09-03** (`proto_sandpile.py`, parallel bulk toppling,
edge sinks, mass conservation asserted at every size):

| grains | rounds | radius (cells) | rounds / radius |
|---|---|---|---|
| 2¹² | 787 | 24 | 33 |
| 2¹⁴ | 3,695 | 47 | 79 |
| 2¹⁶ | 12,837 | 94 | 137 |
| 2¹⁸ | 49,232 | 187 | 263 |
| **2²⁰** | **190,006** | **373** | **509** |

**rounds ∼ N^0.978, radius ∼ N^0.495** (theory ½). So rounds / radius
grows as √N — **rounds ∝ radius²**: the mass diffuses outward rather
than propagating at a fixed speed, and "rounds ≥ radius" above
undersold it by a factor of 500 at the target size. The 2²⁰ picture is
a 190k-round run.

On the GPU that is fine: the phase-0 microbenchmark puts a 1024²
stencil step at 0.28 ms, and this kernel is cheaper (integer, four
taps, no clamp), so **the 2²⁰ pile is ≈ 55 s** of dispatches on a GTX
1660 SUPER. The slider's upper end is not: 2²⁴ extrapolates to ~3M
rounds, a quarter of an hour, and should be capped or warned about. In
NumPy the 2²⁰ run is 23 minutes even after windowing the active region
— the pile has to be simulated on the GPU, and the prototype exists to
give the round count, not the picture.

The windowed implementation reproduces the unwindowed one exactly
(12,837 rounds at 2¹⁶ both ways), so the schedule — and therefore the
count — is the one the shader will run.
**Stages.** `update`, `settle`, `color`. Boundary Zero (edges are
sinks).
**Colouring.** `channel` categorical on h ∈ {0,1,2,3} (the classic
four-colour picture).

---

## 15. Wolfram elementary cellular automata

**Sources.** S. Wolfram, "Statistical mechanics of cellular automata",
*Rev. Mod. Phys.* 55 (1983) 601 `[verify — the rule-number paper]`;
M. Cook, "Universality in elementary cellular automata", *Complex
Systems* 15 (2004) 1 (Rule 110) `[verify]`. Wikipedia "Elementary
cellular automaton" `[read]`.

**Rule.** One-dimensional, binary, nearest-neighbour: the next state
of a cell is bit (4·left + 2·self + right) of the 8-bit rule number.
Rule 90 is left XOR right and draws the Sierpiński triangle from a
single seed; Rule 30 is chaotic; Rule 110 is Turing-complete
`[read]`.

**Discretisation.** The "field" is the space-time diagram: row t of
the grid is generation t. One `update` dispatch per row (a 1-D
workgroup over width, writing one row — the ping-pong is unnecessary
because rows are written once); `steps` = grid height. Cheap
enough to run whole images per frame; the animation axis is the rule
number (as an integer track) or the seed density.
**Parameters.** `rule` (0–255, integer — with a `choices` list for the
named ones: 30, 90, 110, 184, 54, 22, 126, 150), `seed_kind` (choices:
single, random), `density`. Periodic in x.

**Verified 2026-09-03** (`proto_wolfram.py`), shipped 2026-09-04 and
re-verified on the shader: 2,079 of 2,079 cells again, this time with
the parity taken from Kummer's theorem rather than the binomial itself,
because C(63, 29)·63 overflows u64 and wraps silently in release.

Two things a user will see and should not mistake for bugs. Rule 90 on
a PERIODIC lattice of width 2^k self-annihilates at t = 2^k, so at the
default 256 the diagram empties in its lower half — correct, and a
property of the rule. And the seed is the centre COLUMN rather than
the centre cell: the shapes are 2-D but generation 0 is one row, so
sampling the mask at the cell put a `Center` seed in a row this model
never writes, and the first render came out entirely black.

Nothing to measure here —
`steps` = grid height, exactly, by construction. What was checked
instead is the **bit convention**, which is easy to get backwards while
still producing something that looks like a cellular automaton: with
the next state taken as bit (4·left + 2·self + right) of the rule
number, rule 90 from a single seed reproduces Pascal's triangle mod 2
on **2,079 of 2,079 cells** against independently computed binomials.
All eight named rules render; live-cell fractions from a single seed
range from 0.002 (rule 184) to 0.38 (rule 54), which is a usable
smoke-test signature for the shader port.
**Stages.** `update` (row), `color`. **Colouring.** `channel` binary,
`age` (row index — a gradient down the diagram).

---

## 16. Kobayashi phase-field dendrite

**Sources.** R. Kobayashi, "Modeling and numerical simulations of
dendritic crystal growth", *Physica D* 63 (1993) 410–423 `[verify — the
equations and all constants below are from memory; the paper must be
read before any of this ships]`. Anisotropy form cross-checked on
Wikipedia "Phase-field model" `[read]`.

**Equations** (as remembered; p = phase, T = temperature):

```
τ ∂p/∂t = ∇·(ε² ∇p) − ∂/∂x( ε ε' ∂p/∂y ) + ∂/∂y( ε ε' ∂p/∂x ) + p (1 − p)(p − 1/2 + m)
  ∂T/∂t = ∇²T + K ∂p/∂t
m      = (α / π) · arctan( γ (T_e − T) )
ε(θ)   = ε̄ ( 1 + δ cos( j (θ − θ₀) ) ),   θ = angle of ∇p,   ε' = dε/dθ
```

Remembered constants: ε̄ = 0.01, τ = 0.0003, α = 0.9, γ = 10, T_e = 1,
K = 1.6 (latent heat; 1.4–1.8 range), δ = 0.02–0.04, j = 6 (six-fold),
grid 300² with dx = 0.03, dt = 0.0002 `[verify every one]`. Seed: a
small disc of p = 1 in an undercooled melt (T = 0).

**Discretisation.** Two channels (p, T); the anisotropic term needs ∇p
at the cell and the cross-derivative terms — one pass storing ε²∇p and
εε'∇p components into spare channels, a second pass taking their
divergence (`passes = 2`), then the T update uses the just-computed
∂p/∂t. dt is tiny relative to τ: ~10⁴–10⁵ steps for a fully grown
dendrite at 300² — Tier 2, batched heavily.
**Parameters.** `eps`, `tau`, `alpha`, `gamma`, `K`, `delta`, `j`
(choices 2, 4, 6, 8), `theta0`, `dt`.
**Stages.** `update` ×2, `color`. Boundary Clamp (Neumann).
**Colouring.** `channel` on p (the crystal), `two_channel` (T as the
thermal halo), `hillshade` on p.

---

## 17. Packard digital snowflake

**Sources.** N. H. Packard, "Lattice models for solidification and
aggregation", in *Science on Form* (1986) `[verify]`; J. Gravner, D.
Griffeath, "Modeling snow crystal growth I: Rigorous results for
Packard's digital snowflakes", *Experimental Mathematics* 15 (2006)
421–444 `[verify the journal reference]`.

**Rule.** Hexagonal lattice, binary. A vacant cell freezes when the
number of its six frozen neighbours is in a chosen set S ⊆ {1..6};
Packard's rules are named by that set (1, 13, 134, 1345, 1356, …).
Rule "1" (freeze with exactly one frozen neighbour) grows the classic
plate-with-branches `[verify which set gives which shape]`.

**Discretisation.** Hexagonal grid stored as an offset-row square
grid (odd rows shifted by half a cell; the 6-neighbour offsets differ
by row parity); rendered with the resolve pass's hex-aware sampling
(pipeline §4.6 lists the axial-to-pixel map). Terminates when the
crystal reaches the edge; `steps` ≈ radius. Tier 1 — and the
cheapest way to get a "snowflake" on screen while §18 is being built.
**Parameters.** `rule` (a 6-bit mask exposed as six booleans or a
`choices` list of Packard's named rules).
**Stages.** `update`, `color`. **Colouring.** `age` (freeze time as
the palette coordinate gives the growth-ring snowflake).

**Measured 2026-09-03**, shipped 2026-09-04. The GPU port keeps the
six neighbour offsets varying with row parity, which is the whole
awkwardness of an offset-row lattice; a wrong parity is not subtle, as
the six-fold symmetry collapses to four-fold and the baseline pins it.
The resolve still samples the offset grid as a square one, shearing
each cell by half a width — at the scale a snowflake is viewed this
reads as a clean hexagon, and a true axial-to-pixel resolve is a
refinement rather than a correctness gap.

Measured on the offset-row hex lattice the shader will
use (row parity changes the six neighbour offsets — the prototype does
it that way rather than pretending the lattice is square). Rules
S = {1}, {1,3} and {1,3,4} all reach the edge of a 256² grid in
**exactly 125 steps**, which is the radius: `steps ≈ radius` is exact,
not approximate, because the fastest growth direction advances one cell
per step regardless of the rule. What the rule changes is density —
45%, 57% and 66% of the disc filled respectively — so `steps` can be
derived from the grid and needs no per-rule tuning.

---

## 18. Gravner–Griffeath snowfake (mesoscopic 2-D model)

**Sources.** J. Gravner, D. Griffeath, "Modeling snow crystal growth
II: A mesoscopic lattice map with plausible dynamics", *Physica D* 237
(2008) 385–404 `[verify — not on arXiv; the session could not read it]`.
J. Gravner, D. Griffeath, "Modeling snow crystal growth III: three-
dimensional snowfakes", arXiv:0711.4020 `[read — text at
output/gg3.txt; the update rule quoted below is Part III's, whose
steps (i)–(iv) are the same algorithm on the stacked lattice T×Z]`.

**Rule** (Part III, verbatim structure, restricted here to one layer
by dropping the Z-neighbourhood terms — **the 2-D paper's own
parameterisation (its β, κ, μ tables and the vapour density ρ) must
be taken from Part II before a preset ships; the session read only
Part III**). State per site: a ∈ {0,1} attached, b ≥ 0 boundary mass,
d ≥ 0 diffusive mass; initially d ≡ ρ off a small hexagonal seed.
Each time unit, in order:

1. **Diffusion** on the unattached sites: d'(x) = (1/7) Σ_{y ∈ N_T(x)} d°(y)
   (uniform weight 1/7 on the centre and its six T-neighbours), with
   reflecting boundary conditions at the crystal — for a boundary site
   any term for an attached neighbour is replaced by d°(x). `[read]`
2. **Freezing** at boundary sites (those with an attached neighbour),
   with n_T = min(#attached T-neighbours, 3): a proportion
   1 − κ(n_T) of the diffusive mass becomes boundary mass:
   b' = b° + (1 − κ) d°, d' = κ d°. κ decreases in n_T. `[read]`
3. **Attachment**: a boundary site with b° ≥ β(n_T) attaches (a' = 1);
   a site with n_T ≥ 4 attaches automatically ("fills holes and
   makes the surface smoother"); on attachment all diffusive mass at
   the site becomes boundary mass. Attachment is permanent; β
   decreases in n_T. `[read]`
4. **Melting**: at boundary sites a proportion μ(n_T) of boundary mass
   returns to diffusive mass: b' = (1 − μ) b°, d' = d° + μ b°. μ
   decreases in n_T. `[read]`

Mass is conserved by every step. The 3-D paper's canonical seed is a
hexagon of radius 2; its Z-drift substep (1c) does not exist in 2-D.

**Discretisation.** Hex grid as in §17; three channels (a, b, d) in
one texel; the four substeps are one `update` pass each (the
diffusion pass must complete before freezing reads it, and attachment
changes what the next diffusion sees) — `passes = 4`. Everything is a
6-neighbour gather; no atomics. The paper's programs stop when the
density at the edge falls below a fraction of ρ or the crystal reaches
80% of the radius `[read]` — the same two conditions make a natural
`settle`. Simulations are long (the authors call them "barely
feasible" on 2007 PCs in 3-D; 2-D is far cheaper): plan for 10⁴–10⁵
steps at 512². Tier 3 by cost and by the unread Part II.

**Parameters.** `rho`, `kappa1..3`, `beta1..3`, `mu1..3` (or the
paper's smaller set once read), `seed_radius`.
**Stages.** `update` ×4, `settle`, `color`. Boundary Clamp (the paper
uses a finite box with the density held at the edge — `[verify]`).
**Colouring.** `two_channel` (a as the crystal, d as the vapour halo),
`age` (attachment time — the paper's own figures colour by it
`[verify]`).

---

## 19. Diffusion-limited aggregation

**Sources.** T. A. Witten, L. M. Sander, "Diffusion-limited aggregation,
a kinetic critical phenomenon", *Phys. Rev. Lett.* 47 (1981) 1400
`[verify]`; fractal dimension D ≈ 1.71 in 2-D `[read — Wikipedia
"Diffusion-limited aggregation"]`.

**Rule.** A seed particle is fixed. Walkers are launched at a radius
just outside the cluster and random-walk on the lattice; a walker that
lands adjacent to the cluster sticks (with probability p_stick;
p < 1 makes denser clusters); a walker that wanders beyond a kill
radius is relaunched.

**Discretisation** (agent stage, pipeline §4.4). Each agent is a
16-byte record (position, state); each `agents` dispatch moves every
live agent one lattice step with its PCG stream, tests the 3×3
neighbourhood of the *frozen* field texture, and on contact deposits
into the u32 atomic deposit buffer; the `update` pass then folds
deposits into the field (marking the cell frozen and stamping the
step index for age colouring) and relaunches dead agents at the
current launch radius (r_max + 5; kill at 3 r_max — both from a small
bounds reduction the settle stage supplies). Many walkers advance in
parallel, which is *not* the sequential Witten–Sander process, but
the parallel variant with a launch radius outside the cluster is the
standard GPU DLA and preserves D ≈ 1.71 as long as the walker density
near the cluster stays low (a few thousand live walkers at 512²).
Cost: a 512² picture needs ~10⁴–10⁵ stuck particles at 10²–10³ walker
steps each — 10⁷ agent-steps, i.e. seconds at 10⁹ agent-steps/s and
a natural animated subject. Square-lattice anisotropy appears above
~10⁵ particles `[verify]`; an off-lattice variant (float positions,
sticking radius) is a later option.

**Parameters.** `walkers` (10³–10⁶, integer), `p_stick`,
`seed` (choices: point, line, ring, random_points), `lattice`
(choices: square, hex — the hex is via §17's offset rows).
**Stages.** `agents`, `update`, `settle` (bounds), `color`.
Boundary Clamp.
**Colouring.** `age` (arrival order — the classic DLA rainbow),
`occupancy` (walker density, a live "vapour" halo), `channel`.

---

## 20. Eden model

**Sources.** M. Eden, "A two-dimensional growth process", *Proc. 4th
Berkeley Symposium* 4 (1961) 223 `[verify]`; interface in the KPZ
class `[read — Wikipedia "Eden growth model" / KPZ]`.

**Rule.** Starting from a seed, at each step one randomly chosen
empty site adjacent to the cluster is occupied (variants choose a
random *cluster* site and grow into a random empty neighbour). The
cluster is compact with a rough, KPZ-class interface.

**Discretisation.** Parallel stochastic version: each empty site with
an occupied neighbour becomes occupied with probability p per step.
For small p this is the Eden process in the limit; the interface
roughness class is unchanged `[verify]`. 3×3 gather + per-cell RNG.
**Parameters.** `p` (0.01–1), `seed` (choices: point, line — a line
seed gives the growing rough front that shows the KPZ scaling).
**Stages.** `update` (RNG), `color`. **Colouring.** `age` (the
growth-ring texture is the whole point).

**Measured 2026-09-03** (`proto_growth.py`, 256², steps until the
cluster reaches the edge):

| p | seed | steps |
|---|---|---|
| 1.0 | point | 127 (= the radius exactly) |
| 0.3 | point | 256 |
| 0.05 | point | 1,158 |
| 0.3 | line | 505 |

Shipped 2026-09-04. So `steps ≈ radius / p` is exact at p = 1 and
**overestimates by about 2× at small p** — at p = 0.05 it predicts 2,560 against 1,158 measured,
because the front is long and many sites get their chance each step.
The measured range is between radius/(2p) and radius/p; use radius / p
as the default, since a `steps` that is too generous costs time and one
that is too small truncates the cluster.

Two implementation notes the prototype had to get right, and the shader
will too. A step that grows **nothing** is not the end of the run: at
p = 0.05 a four-neighbour front adds nothing on ~81% of steps, so a
`settle` on "no change this step" stops the model almost immediately.
And the neighbourhood must be **periodic in x only** — with a wrapping
y a line seed on the bottom row grows straight into the top row on step
one.

---

## 21. Ballistic deposition

**Sources.** M. J. Vold, *J. Colloid Sci.* 14 (1959) 168 `[verify]`;
F. Family, T. Vicsek, *J. Phys. A* 18 (1985) L75 `[verify]`; M. Kardar,
G. Parisi, Y.-C. Zhang, *Phys. Rev. Lett.* 56 (1986) 889 — exponents
α = 1/2, β = 1/3, z = 3/2 in 1+1 dimensions `[read — Wikipedia
"Kardar–Parisi–Zhang equation"]`.

**Rule.** Particles fall vertically at a random column and stick at
the first site that touches an occupied site — below, or beside
(nearest-neighbour). Height h(i) ← max(h(i−1), h(i) + 1, h(i+1)).
Interface width w(L, t) ∼ L^α f(t / L^z).

**Discretisation.** Column-parallel: each step, every column receives
a particle with probability p (small p approximates the sequential
model); the 2-D field records the arrival step at each deposited cell
(for colouring) and the current height per column lives in row 0 or a
side buffer. A `steps` of ~grid height fills the picture.
**Parameters.** `p`, `sideways` (boolean: nearest-neighbour sticking
on/off — off gives random deposition, no correlations).
**Stages.** `update`, `color`. Periodic in x.
**Colouring.** `age` (arrival time), `channel`.

**Measured 2026-09-03**, shipped 2026-09-04. The GPU port keeps the
column heights in channel `.y` of ROW 0 and has every cell read the
three it needs from there, which keeps the rule cell-local — no
separate height buffer and no second dispatch shape, at the cost of
three extra reads per cell. 256 columns, p = 0.5, to fill the grid:
**361 steps with sideways sticking, 452 without** — so "≈ grid height"
is right to within a factor of 1.4–1.8, and lateral sticking fills
faster because it builds overhangs.

The interface width separates the two variants cleanly — **w = 2.84
with sideways sticking against 10.59 without** at their fill times, and
w(3000) = 5.9 against 28.1 on a 1024-wide interface — which is worth
keeping as a regression check: the `sideways` toggle is the difference
between a correlated interface and an uncorrelated one, and getting it
backwards would still *look* like a rough surface.

What this does **not** show is the KPZ exponent. Fitting w(t) ∼ t^β on
one realisation: random deposition gives **β = 0.507** against the
exact 1/2, steady in every time window (0.51, 0.49, 0.50, 0.48).
Lateral sticking gives 0.205 over the whole run, with local values
swinging 0.24 → 0.42 → 0.02 → 0.18 — a single sample's width
fluctuates too much to read β = 1/3 off it, and ballistic deposition
has notoriously large corrections to scaling. Direction and separation
are established; the exponent is not measured here and the catalogue
should not claim it is.

---

## 22. Dielectric breakdown model

**Sources.** L. Niemeyer, L. Pietronero, H. J. Wiesmann, "Fractal
dimension of dielectric breakdown", *Phys. Rev. Lett.* 52 (1984) 1033
`[verify]`.

**Rule.** The cluster is held at potential φ = 0, the far boundary at
φ = 1, and ∇²φ = 0 in between. Each growth step adds **one** boundary
site i chosen with probability ∝ |∇φ_i|^η (for a site adjacent to the
cluster, |∇φ_i| = φ_i since the cluster is at 0). η = 0 is the Eden
model, η = 1 is DLA, η ≈ 2 gives Lichtenberg-figure branching, η ≳ 4
is nearly one-dimensional `[verify the η → shape correspondences]`.

**Discretisation.** Two coupled problems per growth step:

1. **Relaxation**: Jacobi (or red–black Gauss–Seidel, which the
   ping-pong supports as two half-passes) sweeps of φ with the cluster
   clamped at 0 and the outer ring at 1. Full convergence from scratch
   is O(N²) sweeps on an N² grid, but after one site is added the
   warm-started potential needs only a few sweeps near the change —
   DBM implementations relax K = 5–50 sweeps per growth step. The
   pyramid (pipeline §4.2) doubles as a multigrid V-cycle restriction
   if K proves insufficient.
2. **Growth selection**: the exact rule is a *global* weighted sample
   — one site among all boundary sites. That needs a reduction (sum
   of φ^η) plus a prefix scan to locate the chosen site: a pair of
   reduction dispatches per growth step. The cheaper approximation is
   **parallel stochastic growth**: every boundary site grows
   independently with probability c · φ_i^η per step (c small). This
   is a *different* process (several sites per step, no normalisation)
   though it produces the same visual classes; the plan ships the
   approximation first and labels it `selection: parallel`, adding
   `selection: exact` (scan) as a Tier-4 refinement.

Cost: ~10³–10⁴ growth steps × K sweeps × N² — for 512² and K = 20 that
is ~10⁵ dispatches over a run, seconds to a minute, an animated
subject.

**Parameters.** `eta` (0–6), `relax_sweeps` (1–100, integer),
`selection` (choices: parallel, exact), `growth_rate` c, `seed`
(choices: point, line, ring), `boundary_potential` (choices: ring,
top_edge — the top-edge variant gives lightning from a plate).
**Stages.** `update` ×K (relax) + growth pass, `settle` (reduction
for exact selection), `color`. Boundary Clamp (φ = 1 on the frame).
**Colouring.** `age` (growth order), `channel` on φ (the potential
field around the figure is itself a beautiful thing to draw),
`hillshade` on φ.

---

## 23. Saffman–Taylor viscous fingering

**Sources.** P. G. Saffman, G. I. Taylor, "The penetration of a fluid
into a porous medium or Hele-Shaw cell containing a more viscous
liquid", *Proc. R. Soc. A* 245 (1958) 312 `[verify]`. Lattice
realisation as DLA/DBM with surface tension: T. Vicsek, "Pattern
formation in diffusion-limited aggregation", *Phys. Rev. Lett.* 53
(1984) 2281 `[verify]`.

**Physics.** Darcy flow in the viscous fluid, v = −(b²/12μ) ∇p,
incompressible so ∇²p = 0; the interface advances at v_n = −(b²/12μ)
∂p/∂n; the pressure jump across the interface is σκ (surface tension
× curvature). The less viscous fluid pushed into the more viscous one
is unstable — fingers form and tip-split.

**Discretisation.** As DBM (§22) with η = 1 and a curvature-dependent
boundary condition: the cluster boundary is held at φ = −d₀ κ instead
of 0, κ estimated from the 3×3 occupancy (the discrete curvature the
Vicsek model uses `[verify]`). The `boundary_potential: top_edge`
option gives the channel geometry of the original experiment. It is
a **modelled** Saffman–Taylor, not a Navier–Stokes one — say so in the
tooltip.

**Parameters.** DBM's plus `surface_tension` d₀.
**Stages / colouring.** As §22.

---

## 24. Percolation clusters

**Sources.** D. Stauffer, A. Aharony, *Introduction to Percolation
Theory*, 2nd ed. (Taylor & Francis, 1994) `[verify]` — D = 91/48 for
the incipient infinite cluster in 2-D. Site threshold on the square
lattice p_c ≈ 0.592746 `[verify the digits — M. E. J. Newman, R. M.
Ziff 2000 / J. L. Jacobsen 2015]`; bond p_c = 1/2 exactly, H. Kesten,
*Comm. Math. Phys.* 74 (1980) 41 `[verify]`.

**Rule.** Each site is open with probability p (site percolation);
clusters are the connected components of open sites. At p = p_c the
spanning cluster is a fractal of dimension 91/48 ≈ 1.896.

**Discretisation.** A static random field per seed, then **label
propagation**: each open site takes the minimum label among itself and
its open 4-neighbours each step, until no label changes (O(diameter)
steps — a critical cluster's diameter is the grid size, so ~10³ steps
at 1024²; pointer jumping would be logarithmic but needs random
access and is a later optimisation). The `settle` reduction detects
"no change". Alternative view: BFS distance from a seed site (the
chemical distance), one wavefront step per `update`.
**Parameters.** `p` (with p_c marked), `mode` (choices: labels,
distance_from_seed, spanning_only), `lattice` (choices: site, bond).
**Stages.** `update`, `settle`, `color`. Boundary Zero.
**Colouring.** `channel` categorical on label (hash → hue), `age` on
chemical distance.

**Shipped 2026-09-04 with PATH COMPRESSION, which changes the cost
below by an order of magnitude.** A label is a cell index, so the
shader reads the cell its own label points at and takes that label
too — a union-find "find" step, valid because the cell a label came
from is by construction in the same cluster. Measured against the
plain-propagation medians below: 53 rounds at 64², 93 at 128²,
**167 at 256² against 645**, and 491 at 512² against 1,409 — worth
3.9× and 2.9× at the two sizes there is a comparison for.

The labelling is verified against a CPU flood fill rather than a
baseline image: same component ⇒ same label and same label ⇒ same
component, checked both ways over 122 components and 2,455 open cells,
which catches a label leaking across a closed site or a component
failing to merge. Neither shows up as anything but plausible coloured
blobs.

**No settle reduction was needed.** Labels only ever decrease, so extra
steps are no-ops and over-running is safe — a settle would be an
optimisation, not a correctness requirement, and the plan's reduction
stage is deferred on that basis. The presets carry about twice the
measured count, because of the spread below.

**Measured 2026-09-03, and the estimate above is 3–5× LOW.** Label
propagation costs the longest *chemical* path in the cluster, which at
p_c is far longer than the geometric diameter. Five seeds per size:

| L | median rounds | range |
|---|---|---|
| 128 | 272 | 223 – 445 |
| 256 | 645 | 485 – 760 |
| 512 | 1,409 | 776 – 2,413 |

Median rounds ∼ L^1.19, worst ∼ L^1.22, so **L = 1024 extrapolates to
≈ 3,250 rounds typical and ≈ 5,100 worst** — against the "~10³ steps at
1024²" guessed above.

**The spread is the design constraint, not the median.** Two 256²
samples measured 332 and 1,232 rounds — a 4× swing at one size, because
at p_c the spanning cluster is critical and its longest path is not
self-averaging. A fixed `steps` cannot serve this model: it needs the
`settle` reduction to stop it, with a generous cap. Away from p_c it is
cheaper and much better behaved (p = 0.5 → 100 rounds).

---

## 25. Invasion percolation

**Sources.** D. Wilkinson, J. F. Willemsen, "Invasion percolation: a
new form of percolation theory", *J. Phys. A* 16 (1983) 3365
`[verify]`; D ≈ 1.89 without trapping, ≈ 1.82 with trapping `[read —
Wikipedia "Invasion percolation"]`.

**Rule.** Each site has a random threshold r ∈ [0,1]. The invaded
cluster grows from a seed by adding, at each step, the boundary site
with the **lowest** threshold. (With trapping, regions cut off from
the outlet cannot be invaded.)

**Discretisation.** The sequential global argmin has a parallel
equivalent for the shape: the invaded region at the moment the
running maximum threshold reaches p equals the seed's connected
component among sites with r < p. So the GPU rule is a **rising
threshold**: p(t) increases by Δp per step and a boundary site with
r < p(t) joins — a BFS-like wavefront that reproduces
invasion-percolation-without-trapping's cluster (same D ≈ 1.89 as
ordinary critical percolation `[read]`). Trapping needs global
connectivity to an outlet each step and is not planned. The invasion
order (for colouring) is the step index.
**Parameters.** `dp` per step, `seed` (choices: point, left_edge),
`stop_at_span` (boolean).
**Stages.** `update`, `settle`, `color`. **Colouring.** `age`
(invasion time — the standard picture), `channel` on r.

---

## 26. Ising model (Metropolis)

**Sources.** L. Onsager, *Phys. Rev.* 65 (1944) 117 — T_c =
2J / (k_B ln(1 + √2)) ≈ 2.269 J/k_B `[read — Wikipedia "Ising model"]`;
N. Metropolis et al., *J. Chem. Phys.* 21 (1953) 1087 `[verify]`.

**Rule.** Spins s = ±1; energy E = −J Σ_{⟨ij⟩} s_i s_j − H Σ s_i. A
proposed flip with energy change ΔE is accepted with probability
min(1, exp(−ΔE / k_B T)). At T_c the spin clusters are fractal at all
scales; below T_c domains coarsen; above they are noise.

**Discretisation.** **Checkerboard** updating: two half-passes per
sweep (all black sites, then all white), each site's ΔE depending only
on the other colour — this preserves detailed balance where a
fully-synchronous update would not `[verify — standard]`. Per-cell PCG
RNG; spins as a float channel ±1 (or bitcast u32). Periodic.
**Parameters.** `T` (0.5–5, with T_c marked), `H` (−1–1), `J`,
`algorithm` (choices: metropolis, heat_bath), `sweeps_per_step`.
**Presets.** T = T_c (critical), T = 1.5 (coarsening from noise).

**Measured 2026-09-03**, checkerboard Metropolis at 256², |m| averaged
over the last 50 of 600 sweeps — the three regimes come out in the
right order with plausible magnitudes:

| T | ⟨\|m\|⟩ | churn | developed |
|---|---|---|---|
| 1.5 (ordered) | **0.90** | 0.016 | 436 sweeps |
| 2.269 (T_c) | **0.33** | 0.207 | 27 |
| 3.5 (disordered) | **0.007** | 0.548 | 1 |

Ordered below, disordered above, in between at T_c. That validates
the checkerboard split as implemented (a broken one gives wrong
statistics at every T).

**The GPU port is checked against Onsager instead (2026-09-04), and
the observable changed for a reason.** Magnetisation is global and
equilibrates by domain coarsening, so on a 128² lattice at 600 sweeps
it measured 0.090 for T = 1.5 — *below* its own critical value, purely
because the lattice sat in a multi-domain state; left running it
reaches 0.985. The nearest-neighbour correlation is local, flat from
~100 sweeps, monotonic in T, and has an **exact** value at T_c:
1/√2 = 0.7071. Measured on the GPU at 100 sweeps: **0.952 / 0.691 /
0.332** for T = 1.5 / T_c / 3.5. That is a quantitative check against
an analytic result rather than against a previous run. It is **not** an equilibrium measurement at
T_c: 600 sweeps from a random start grows the correlation length to
~t^(1/z) ≈ 19 cells against L = 256, so the 0.33 is a coarsening
snapshot and should not be quoted as the critical magnetisation. The
`steps` default should be **per-preset**: coarsening needs ~400 sweeps
to look like anything, the critical and hot states need almost none.
**Stages.** `update` ×2, `color`. **Colouring.** `channel` binary,
`age` (time since last flip — the coarsening fronts light up),
`hillshade` on a locally averaged spin (the pyramid's level 2 gives
the magnetisation field for free).

---

## 27. Physarum transport network (Jones)

**Sources.** J. Jones, "Characteristics of pattern formation and
evolution in approximations of Physarum transport networks",
*Artificial Life* 16(2) (2010) 127–153 `[verify — cited via Sage
Jenson's "physarum" page, which was read and gives no parameter
values]`.

**Rule** (Jones' agent model, as understood from the secondary
sources):

Each agent has a position and heading. Per step:

1. **Sense**: sample the trail map at three sensors — ahead at
   distance SO, and at ±SA (sensor angle) from the heading.
2. **Turn**: rotate toward the strongest sensor by RA (rotation
   angle); if the front sensor is strongest, keep heading; if left and
   right are equal and both exceed the front, turn randomly.
3. **Move**: advance SS (step size) along the heading; if the target
   cell is outside the grid (or occupied, in Jones' exclusion variant)
   choose a random new heading instead.
4. **Deposit**: add depT to the trail map at the new position.

Then the trail map is **diffused** (3×3 mean) and **decayed**
(multiplied by 1 − decay). Jones' Table 1 defaults are remembered as
SA = 22.5°–45°, RA = 45°, SO = 9, SS = 1, depT = 5, decay ≈ 0.1
`[verify against the paper]`.

**Discretisation.** The agent stage exactly (pipeline §4.4): a 16-byte
agent (position f32×2, heading f32, spare), sensing by `textureLoad`
of the field texture (three reads), deposit via the u32 atomic buffer
(fixed-point, ×1024), then the `update` pass adds the resolved deposit,
applies the 3×3 mean and the decay in one stencil. Agents are
persistent across frames, so the model is stateful in the way §6 of
the integration doc describes. 10⁵–10⁶ agents at 512²–1024² is the
usual density (Jenson uses millions); at 10⁹ agent-steps/s that is
~1 ms per step for 10⁶ agents.
**Parameters.** `agents` (10⁴–4·10⁶), `SA`, `RA`, `SO`, `SS`,
`deposit`, `decay`, `diffuse` (boolean), `spawn` (choices: random,
ring, centre, edges), `wrap` (boolean).
**Stages.** `agents`, `update`, `color`. Periodic or Clamp.
**Colouring.** `channel` on the trail (the standard), `occupancy`
(agent density — a different, grainier image of the same network),
`age`.

---

## 28. Cross-cutting notes

- **Determinism.** Every stochastic model draws from the PCG in
  `shaders/core/rng.wgsl` seeded by (config seed, cell or agent
  index, step index) so a run is reproducible on one GPU and the
  visual baselines can be exact hashes (pipeline §8). Agent models are
  reproducible only if the deposit resolution is order-independent —
  integer atomics are, float atomics would not be; that is why the
  deposit buffer is u32.
- **Clamping.** Every RD model clamps to its physical range after the
  step (Gray–Scott [0,1], Brusselator/Schnakenberg ≥ 0, FHN a wide
  box, Cahn–Hilliard a NaN guard only). Do it with `clamp`, never with
  a self-compare (the Metal fast-math rule in CLAUDE.md applies to
  these kernels too).
- **dt is the user's foot-gun.** The explicit schemes above have hard
  stability limits (§6, §7 give them). Ship each model's `dt` slider
  with a max at the stability bound computed from the current D and
  h, and show the bound in the tooltip rather than letting the field
  explode into NaN; the kernel also clamps, so an explosion is ugly,
  not fatal.
- **Hex lattices** (§17, §18, DLA hex) share one offset-row addressing
  helper and one resolve-pass sampler; write it once.
- **Integer state** (§11, §12, §14, §24) lives in the Rgba32Float via
  `bitcast<u32>` so the float pipeline, pyramid excluded, is
  unchanged; the pyramid must never be built over an integer channel
  (a `Model.pyramid_channels` mask says which channels are averaged).
- **What was not validated.** Only Gray–Scott and McCabe were run in
  NumPy. Every other rule above is planned, not measured; the master
  plan's phase gates require a NumPy or CPU-Rust run of each model
  before its GPU kernel is written, mainly to pin the step budget and
  the seeding that actually produces the picture.

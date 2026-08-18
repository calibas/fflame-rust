# Escape-Time & Orbit-Trap Fractals

**Status:** Planning — agreed architecture, no code yet. This is the
working plan.

A second rendering mode: per-pixel iterated fractals (Mandelbrot,
Burning Ship, tetration, Kali, …) rendered by fragment shader, colored
by escape behaviour and orbit traps, sharing the app's palette, tone
mapping, effects, export and animation machinery. Deep zoom via
perturbation theory arrives in a later phase — the math groundwork is
[docs/experimental/pertubation-theory.md](../experimental/pertubation-theory.md).

---

## 1. What exists already (and what happens to it)

| thing | what it is | fate |
|---|---|---|
| `julia` color effect (`shaders/effects/color/julia.wgsl`) | Mandelbrot/Julia overlay as a post-effect: own zoom/pan, HSV coloring, 13 blend modes | Stays, as the "escape-time as garnish over a flame" tool. Its HSV coloring is superseded by palette coloring in the new mode; consider a deprecation banner once the mode ships. |
| `fract_*_wf` variations (6) | JWF escape-time *inside the chaos game* — random seed, iterate, plot by escape count | Stays untouched; different artistic object (point clouds through escape sets). Their deferred Buddhabrot mode is unrelated to this plan. |
| `mandelbrot` variation | Random-walk Buddhabrot in the chaos game | Stays untouched. |
| `littlewood` variation | The *dynamical-space* attractor A(λ) of the ±1 power-series IFS | Stays. Its module doc explicitly rules the *parameter-space* root cloud out of chaos-game scope — that side is catalog entry §4.11 here. The two views cross-check each other: at a given λ, the new mode's membership test must agree with whether the variation's attractor contains 0. |
| `docs/projects/new-shaders.md` | WGSL technique cookbook | Reference: IQ cosine palettes, domain warp — several coloring ideas start there. |
| Effects system | Ping-pong fragment chain, params→UI autogen, `EffectSource::Owned`, API-shareable | Reused **downstream** (effects compose on the escape image) and **imitated** (param-driven UI, registry shape). Not the host — see §2. |

---

## 2. Decision record

### The escape renderer is its own pipeline stage, not an effect

Effects are image *transforms*: they consume an input texture, and
their ABI is a 48-float uniform block with no storage buffers. An
escape renderer *generates* — it needs no input image, and phase 3
needs a reference-orbit storage buffer and an arbitrary-precision
center that cannot ride in 48 floats. The `julia` effect only fits the
effect mold by ignoring its input. Hosting the new mode in an effect
slot would mean either growing the effect ABI for one tenant or
lying about what the stage does.

**Rejected: copying the effects system** for the new mode — ~1400
lines duplicated in order to diverge immediately. What we actually
reuse is listed below; the chain itself is not it.

### Topology: replace the generator, share everything downstream

```
[chaos game + accumulate]   OR   [escape fragment pass]     ← the mode switch
              ↓ (same Rgba32Float HDR target)
     tonemap (Linear default for escape; log stays available)
              ↓
     color-effects chain            ← kaleidoscope on a Mandelbrot: free
              ↓
     display / PNG export / thumbnails
```

- **Zero impact on flame rendering is structural**: the only new code
  on the flame path is the top-level mode check. No shared inner-loop
  branches.
- **Tone mapping reuse** with one honest caveat: the flam3 log mapping
  is shaped for million-to-one density histograms; escape output is
  not that. `ToneMapMode::Linear` already exists and is the escape
  default. Exposure, gamma, vibrancy, Levels, curves all apply
  unchanged.
- **Palette reuse is total**: iteration count / trap distance / orbit
  average map to palette *position*; the existing palette texture,
  rotation, squeeze, reverse and log-strength controls become live
  coloring controls with zero new code.
- **Effects compose downstream for free**, which retires the "Layers?"
  question for v1: escape-over-flame garnish = the existing `julia`
  effect; escape-as-subject = the new mode; true layer compositing
  stays the separate future feature CLAUDE.md already lists.
  **Rejected for v1: layered compositing** — building a compositor as
  a prerequisite is exactly the bloat this plan avoids.

### Camera: the mode owns its own, and deep zoom forces the shape

Flame view state is f32 pan/zoom — physically incapable of holding a
deep-zoom center. The escape camera is:

- `center_re`, `center_im`: **decimal strings** in the config
  (arbitrary precision; parsed by the fixed-point module in phase 3,
  by f64 until then),
- `zoom_log2: f64` — the **log₂ zoom exponent**. A plain float
  ConfigPath, so undo, animation tracks and scripting work on it
  unmodified. A deep-zoom dive animation is "animate the exponent,
  center fixed" — which is also the orbit cache's cheap case (one
  reference orbit serves the whole dive), and the newly added
  exponential track interpolation has exactly the right feel for it,
- `rotation: f32`.

Viewport input (drag pan, wheel zoom-to-cursor, pinch) routes by
render mode. Pan at depth mutates the center *strings* (fixed-point
add — phase 3; f64 until then, which is fine at shallow zoom).

**Not animatable in v1: the center strings.** Tracks hold floats. A
center-path animation (Misiurewicz walks) is an open question for
later — see §8.

### Formula × Coloring × Traps are orthogonal axes

Ultra Fractal's proven split, and the catalog (§4) demands it anyway:
Kali-family fractals never escape and are colored by orbit *averages*;
Newton-family converge and are colored by root basin; everything else
colors by escape. One monolithic shader per fractal would duplicate
every coloring in every formula.

- A **FormulaDef registry** mirroring `VariationDef`'s shape: static
  defs, inline WGSL step body, param defs (auto-generating the panel
  UI exactly like variation params do), and feature flags
  (`NeedsPrevZ`, `NeedsComplexExpLog`, `Convergent`, `NonEscaping`,
  `AbsFold`, `SignChoice`) that gate what the shader assembler splices
  in — the same architecture pattern that already works for 647
  variations.
- A **ColoringDef registry**, same shape, consuming the per-pixel
  orbit summary `(z_final, n, escaped/converged/root, trap
  accumulator, average accumulators, |dz/dc| when enabled)`.
- **Julia mode is a toggle, not a formula list entry**: every formula
  with a `c` parameter has a Julia twin (c fixed, seed = pixel). One
  flag halves the catalog.
- Shaders are assembled per (formula, coloring, flags) on demand —
  small WGSL, fast compiles — and the **existing pipeline LRU**
  (`shader_cache`) absorbs recompiles when the user flips between
  combinations, the same way it absorbs flame shader churn.

### Arbitrary precision: our own fixed-point, no new dependencies

Decision from the dependency discussion, recorded with its reasons.

The reference orbit lives in a bounded box (|z| ≤ 2 until escape), so
the CPU side needs **fixed-point**, not arbitrary-precision floating
point: `[u64; K]` limbs, a few integer bits, implied binary point.
That deletes MPFR's actual hard parts — no exponents, no
normalization, no rounding modes. Surface: add, sub, schoolbook mul
(u64×u64→u128; Karatsuba past ~30 limbs), shifts, decimal-string
round-trip, f64 conversion, floatexp export, complex-pair wrapper.
~600–1000 lines with tests.

Throughput (schoolbook, per 1M-iteration orbit): ~30 ms at 1e-30,
~0.3 s at 1e-100, ~2–3 s at 1e-300, ~10 s at 1e-600 with Karatsuba.
MPFR's FFT multiplication only pulls ahead past several thousand bits
(zoom 1e-1000+), which is out of scope for a browser-first app — and
if it ever isn't, Toom-3 is an internal upgrade.

Why owning it beats both library options:

- **Cross-platform bit-identity is structural.** Integer limb
  arithmetic is identical on x86, ARM and wasm. A saved deep-zoom
  location must reproduce exactly on desktop and web; with a library,
  edge-rounding divergence is a test burden — with limbs it is
  impossible. Same reasoning that pinned the script RNG.
- **The GPU half is hand-rolled regardless** (WGSL has no f64): the
  floatexp / scaled-f32 shader types are ours no matter what. Owning
  the CPU side keeps one numeric story with matched semantics at the
  upload boundary.
- **Rejected: `rug` (MPFR)** — C build chain, hostile to the wasm
  build, LGPL linking care, and we would use ~1% of its surface.
- **Rejected: `astro-float` / `dashu`** — pure Rust and workable, but
  a general float tower (rounding modes, transcendentals) imported as
  audit surface for a workload that needs none of it, at a 2–5×
  penalty over MPFR anyway.

Known gap, accepted: the core set has no division. The orbit never
divides; Newton nucleus-finding (a phase-4 optimization) either uses
reciprocal-by-Newton (multiplication only) or waits.

### Config, serialization, and the contract

- `EscapeConfig` struct inside `FractalConfig`, skip-if-default, so
  every existing `.fflame` stays byte-stable and flame-mode saves
  never mention it.
- `render_mode` gains an `escape` variant. **This moves the engine
  contract's shape** (a new enum value in a pinned vocabulary — unlike
  the script-stem additions, the API's staleness pin should fire; even
  so, coordinate with the API repo deliberately rather than trusting
  the pin).
- `.flame` XML: **not written, not read** — Apophysis/JWildfire have
  no escape-time mode; this is `.fflame`-only, same policy as
  depth-density compensation.
- Scripting: escape params are ConfigPaths → `config.set("escape.…")`
  works day one; a `escape`-aware script API (formula pickers etc.) is
  deliberately later.

### UI

One new dockable panel, **Escape Fractal** (`PanelType::EscapeFractal`):
formula picker, Julia toggle + c picker, per-formula params
(auto-generated from FormulaDef, same widget path as variation
params), coloring picker + its params, trap editor, iteration budget,
and the mode's camera readout. Compact mode gets it in the Window
submenu. The View panel stays flame-only; the Colors/Tone Mapping and
Palette panels apply to both modes as-is.

Progressive refinement v1: escape frames re-render whole per change
(direct f32 is fast); if a formula × budget combination gets slow, the
iteration governor's shape (median-filtered budget ratio) is sitting
right there to adapt `max_iter` — wire only if needed.

---

## 3. Phases

**Phase 1 — direct-f32 mode (ships standalone value).**
Pipeline stage + mode switch; EscapeConfig + serialization; camera +
input routing; Escape Fractal panel; FormulaDef/ColoringDef
registries; formulas: Mandelbrot/Multibrot, Tricorn, Burning Ship
family, Kaliset (+ Julia toggle for all); colorings: escape count,
smooth iteration, basic orbit traps (point/circle/cross/line,
min-distance), orbit average for Kali; palette mapping; Linear
tonemap default; PNG export; visual-regression corpus entries per
formula; probe-style compile test over every formula × coloring pair.

**Phase 2 — catalog and coloring depth.**
Phoenix, Newton/Nova, Magnet, exponential family, tetration/Power
Tower, Deep Tetration Web, Littlewood parameter space, Ducks;
stripe-average and triangle-inequality colorings; exterior distance
estimation (derivative orbit); interior/period coloring.

**Phase 3 — perturbation (deep zoom).**
The `fixedpoint` module; CPU reference orbit on a worker thread with
progressive upload; orbit cache keyed on (center strings, precision,
maxiter) with append-on-deepen; scaled-f32 delta iteration; floatexp
WGSL type; Zhuoran rebasing; depth-based ladder direct →
scaled-f32 → floatexp. Per-formula perturbation tiers as cataloged in
§4 (clean / diffabs / hard / none). Details and math:
[pertubation-theory.md](../experimental/pertubation-theory.md).

**Phase 4 — accelerations and reach.**
BLA iteration skipping; Newton nucleus/period finding (division via
reciprocal iteration); hybrid formula loops; possible Buddhabrot
bridge to the chaos-game side. Each optional, none load-bearing.

---

## 4. The formula catalog

Per entry: iteration, test, coloring pairings, parameters beyond the
shared set (`max_iter`, `bailout`, Julia toggle + `c`), perturbation
tier for phase 3 (**clean** = complex-linear BLA works as-is,
**diffabs** = piecewise-linear case analysis, **hard** = adapted
rebase criterion needed, **none** = direct-only).

Shared machinery assumed everywhere: pixel → complex plane via the
mode camera; smooth-iteration fraction `n + 1 − log₂(log|z|)` where
escape applies; trap and average accumulators updated per iteration on
the full orbit value.

### 4.1 Mandelbrot / Multibrot — `z ← zᵖ + c`
The reference object. Integer `p ≥ 2` (p = 2 default; the classic
Multibrots at 3–8). Escape |z|² > bailout. All colorings apply.
Perturbation: **clean** (binomial expansion, precompute `Zₙᵖ⁻ᵏ`).
Param: `power` (int).

### 4.2 Tricorn / Mandelbar — `z ← z̄² + c`
Conjugation is ℝ-linear; renders the "Mandelbar" with its
tricorn-symmetric hull. Perturbation: **clean**, but BLA coefficients
become real 2×2 matrices — which the BLA storage should use from day
one so abs-family formulas share the path.

### 4.3 Burning Ship family — `z ← (|Re z| + i|Im z|)² + c` and kin
The abs-fold matrix: Burning Ship, Perpendicular
Mandelbrot/Ship/Celtic/Buffalo variants — each is a choice of which
components fold and where the conjugation lands. Catalog them as ONE
formula with a `variant` enum param rather than six entries (the step
bodies differ by two sign/abs placements). Escape as Mandelbrot.
Perturbation: **diffabs** (`|X+x| − |X|` case analysis; BLA validity
shrinks near the fold axes — single-step fallback there).

### 4.4 Phoenix — `z ← z² + c + p·z_prev`
Needs the previous iterate (`NeedsPrevZ` feature flag → the step
carries two complex registers). Classic at Julia-mode with
`c = 0.5667, p = −0.5`. Perturbation: **clean-ish** (linear part is a
2×2 block over (δₙ, δₙ₋₁)); phase 3 stretch goal, direct until then.
Params: `p_re`, `p_im`.

### 4.5 Newton / Nova — `z ← z − R·(zᵖ − 1)/(p·zᵖ⁻¹) [+ c]`
**Convergent**: orbits approach roots of zᵖ − 1; the test is
`|z − z_prev| < ε` (needs `NeedsPrevZ`) or per-root proximity.
Coloring: root-basin index → palette position, shaded by convergence
speed; escape colorings don't apply. Nova adds `+ c` per iteration
(Mandelbrot-like parameter plane over a convergent core) and a
relaxation `R`. Perturbation: **hard** (rebase criterion must be
rethought near attracting fixed points — Imagina has prior art; last
in line, possibly never). Params: `power`, `relax_re/im`.

### 4.6 Magnet I / II
Rational maps from statistical mechanics:
`z ← ((z² + c − 1)/(2z + c − 2))²` (type I; type II is the higher
cousin). Mixed behaviour: escape AND convergence to 1 both terminate
(`|z−1| < ε`). Interesting boundaries, classic catalog citizens.
Perturbation: **hard** (rational). Phase 2, direct-only until proven
wanted deeper.

### 4.7 Exponential family — `z ← eᶻ + c`, `sin z + c`, `cos z + c`
`NeedsComplexExpLog`. Escape is **Re z > threshold** (for exp; the
trig twins escape in |Im|) rather than |z|² — the escape-test slot is
per-formula, which these force. Cantor-bouquet hairs; average
colorings shine here. Perturbation: **hard** (identities exist,
exponent dynamics violent — floatexp mandatory early); direct-only
until phase 4 at the earliest.

### 4.8 Power Tower / tetration — `z ← cᶻ`
Iterated exponential from the pixel's `c` (seed z = c; equivalently
the tower c^c^c^…). Implemented as `exp(z·log c)` with `log c`
computed **once per pixel** (`NeedsComplexExpLog`). Three-way
behaviour: escape (Re(z·log c) > threshold), convergence to the
tower's fixed point (`|z − z_prev| < ε` — the classic
Shell–Thron-region interior), and periodic cycles (period-k detection
via a short ring of prior iterates, colored by k). All three need
distinct coloring hooks — this formula is the reason the orbit
summary carries a `converged/periodic` channel. Perturbation:
**none** planned (no established art; direct + the f32 depth limit is
the product).

### 4.9 Deep Tetration Web
The high-iteration filament web of the tetration map — the structure
that emerges from 4.8 at max_iter in the thousands with escape-web
coloring (iteration bands compressed, e.g. log-of-count palette
mapping, which the palette log-strength control already provides).
Catalogued separately because the *presentation* differs (huge
iteration budgets, band-compressed coloring, usually Julia-mode
slices), but it is a preset family over 4.8, not a new step function.
**Open item:** pin the exact reference images/formula convention
(sources vary on seed choice and branch cut) during phase 2
implementation — decide against pictures, not folklore.

### 4.10 Kaliset — `z ← |z| / (z·z) − c` (component abs, complex square in denominator per convention)
**Non-escaping** (`NonEscaping` flag): orbits churn forever; there is
no bailout. Colored exclusively by **orbit averages** — mean/min of a
trap function over iterations (the classic Kali glow is min-distance
to axes with exponential falloff). This entry is why average
accumulators are core machinery, not a phase-2 add-on: without them
the formula renders nothing. Cheap per iteration; budget ~50–200
iterations. Perturbation: **none** (self-similar at every scale
anyway — deep zoom is just… more of it, in f32). Convention note: the
Kali literature writes several near-identical forms
(`|z|/dot(z,z) + c` vs `− c`, component vs complex square); pick one
against reference images and name it in the formula doc.

### 4.11 Littlewood parameter space — the root cloud
For pixel λ: does some ±1 power series vanish at λ ⟺ does the
attractor of `f±(z) = (z ± 1)/λ` contain 0? Rendered escape-time-style
by iterating the **sign-choice** map from z = 0 in the inverse
direction — `w ← λ·w ± 1` choosing the sign that minimizes |w|
(greedy; `SignChoice` flag) — bounded orbit ⟹ root nearby; escape
count shades the boundary. This is the *parameter-space* twin of the
existing `littlewood` chaos-game variation, whose module doc
explicitly points here-ward ("it is root-finding, not a chaos game").
**Cross-check in tests**: at landmark λ (twin-dragon 1+i, golden
ratio), membership must agree with whether the variation's attractor
visibly contains the origin. Perturbation: **none** (bounded region,
|λ| ∈ [1, 2] — f32 forever). Params: digit set ({±1}, {0,±1},
{±1,±i} — mirroring the variation's `coeffs`).

### 4.12 Ducks / Kali-log (Monnier) — `z ← log(Re z + i·|Im z|) + c`
Samuel Monnier's "Ducks" fractal: half-fold then complex log
(`NeedsComplexExpLog`, `AbsFold`). Non-escaping like Kaliset; colored
by orbit averages (Monnier's own images use mean-of-|z| style
statistics). Spectacular with stripe-average coloring. Perturbation:
**none**. Phase 2.

### Explicitly not in the catalog (and why)

- **Buddhabrot as a fragment mode** — it's a *density* technique; the
  chaos-game side already renders it (the `mandelbrot` variation), and
  a fragment-shader Buddhabrot is a different algorithm (per-pixel
  histogram scatter) that belongs, if anywhere, in the flame pipeline.
  Phase 4 "bridge" note only.
- **3D escape fractals (Mandelbulb, Mandelbox)** — ray-marched DE
  rendering is a third pipeline (camera rays, not per-pixel planes).
  Real feature, separate plan; nothing in this design blocks it.
- **Lyapunov fractals** — sequence-driven exponent maps; plausible
  later FormulaDef but needs its own coloring semantics; parked.

---

## 5. Coloring & trap catalog

All consume the orbit summary; all map to **palette position** and
inherit palette rotation/squeeze/reverse/log-strength for free.

| coloring | consumes | notes |
|---|---|---|
| Escape count (banded) | n | the classic; log-compress via palette log-strength |
| Smooth iteration | n + fraction | continuous gradients; default for escape formulas |
| Orbit trap: point/circle/cross/line/shape | min (or avg) distance of orbit to trap geometry | trap geometry params live in the coloring, composable with any formula; the "shape" trap reads a small SDF enum |
| Orbit average (Kali glow) | running mean of a trap function | REQUIRED for NonEscaping formulas; optional everywhere |
| Stripe average | mean of sin(k·arg z) | phase 2; the classic silky filaments |
| Triangle inequality average | per-step TIA term | phase 2 |
| Root basin | root index + convergence speed | Convergent formulas |
| Period / interior | detected cycle length | tetration interior, Mandelbrot interior (phase 2) |
| Distance estimation | |z|, |dz/dc| | phase 2; needs the derivative orbit (one extra complex register, same perturbation story) |

Two-channel design worth stating: a coloring may emit **position +
intensity** (e.g. trap distance → position, iteration count →
intensity), feeding the HDR target's rgb·a shape so tone mapping has
something real to do.

---

## 6. Testing

- **Visual corpus**: one config per formula (phase 1: 5–6), landmark
  coordinates with known appearances; deterministic (no RNG in this
  pipeline at all), so baselines are exact.
- **Compile probe**: every formula × coloring × flag combination
  assembles and parses in both a shallow and a Julia-mode
  configuration — same shape as the variation probe's batch-compile
  test, and the thing that catches a registry entry missing its
  helper injection (the crackle_fast lesson).
- **Fixed-point (phase 3)**: differential vs f64 at 1–2 limbs;
  published deep-zoom reference orbits at depth; ring-axiom property
  tests; decimal round-trip; CPU-vs-GPU floatexp boundary agreement.
- **Littlewood cross-check** as in §4.11.
- **Contract**: `render_mode` vocabulary change verified against the
  API repo before release, pin or no pin.

## 7. Bloat ledger

Genuinely new: the fragment pass + bind group, EscapeConfig, two
registries + their WGSL assembler (small, patterned on the variation
builder), one panel, input routing, and in phase 3 the fixedpoint
module + orbit cache. Reused wholesale: palette, tonemap, Levels,
curves, effects chain, export paths, thumbnails, undo/ConfigPath,
animation tracks, scripting set/get, docking UI, pipeline LRU,
visual-test harness. Zero new dependencies.

## 8. Open questions

1. **Mode naming** in the UI ("Escape Fractals"? "Deep Fractals"?) —
   user-facing wording, user's call.
2. **Center-path animation** (Misiurewicz walks): needs
   string-interpolation or waypoint machinery tracks can't express
   today. Parked until someone wants it.
3. **Deep Tetration Web reference convention** — pin against images
   during phase 2 (§4.9).
4. **Kaliset formula convention** — same (§4.10).
5. **Whether the `julia` effect eventually deprecates** once the mode
   ships, or stays as the overlay tool.
6. **Iteration governor reuse** for heavyweight formula × budget
   combinations — wire only if a real case appears.

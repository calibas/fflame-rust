# The derivative under perturbation (survey P11)

Branch `derivative-under-perturbation`, started 2026-10-03.

## Why

The perturbed rungs iterate no derivative. Every colouring that reads
`dz` goes flat there:
- distance estimate;
- normal map;
- rainbow fringe;
- analytic relief.

Those are exactly the views deep zoom exists for. Measured with
`every_colouring_on_the_perturbed_paths_against_direct`, forcing the
perturbed paths at a shallow view where the direct path is exact: these
three colourings agree with the direct path on 3-43% of 8x8 blocks.
Every other colouring scores 93-100% there, against smooth's 93%.

Kalles Fraktaler carries the derivative through perturbation and BLA, and
Fraktaler 3 does it with dual numbers. Both draw distance-estimate
colourings at every depth, in pixel units.

## The convention: the derivative per render pixel

`OrbitSummary.dz` becomes dz/dc times the render pixel's size in the plane,
on every path. At escape that is about `|z| ln|z| / DE_px`, a moderate
number at any depth. The absolute dz/dc grows like 1/S, past f32's range
at about zoom 2^120, and the pixel size itself shrinks past it too.

The four consumers:
- **distance estimate:** `r ln r / |dz|` is the distance in render pixels.
  - Output pixels: divide by the supersample factor.
  - Plane units: multiply by the render pixel size.
- **rainbow fringe:** the same, in output pixels; its direction is
  unchanged.
- **analytic relief:** `d_px = r ln r / |dz|` directly.
- **normal map:** reads only the direction of z/dz, which a positive
  scale leaves alone.

The direct path keeps iterating the absolute derivative and converts
once, when it builds the summary (and so in its records, which the
recolour pass reads). Its pictures move by rounding only.

## The scaled rung

The scaled rung's delta is `w = δ/S` and its pixel offset `d0 = Δc/S`.
So `dw/dd0 = dδ/dΔc = dz/dc = D`: the absolute derivative, which fits
f32 on this rung (zoom below about 48).
- **A step:** `D' = formula_derivative(z_before, c, D, julia)`. This is the
  formula's own snippet, at the pre-step full iterate the loop already
  reconstructs.
- **A BLA skip:** `w' = A w + B d0` gives `D' = A D + B`, the same
  coefficients, with the same exponent handling as `w`.
- **The seed:** `select(DZ0, 1, julia)`, as the direct path seeds it.
- **The summary:** `dz = D * S`.
- **Chunks:** `D` resumes from the state buffer, through a state tail spliced
  only when the derivative is compiled in.

## The floatexp rung

`D` is held as an f32 complex mantissa with an i32 exponent.
- **A step:** `f'(z) D` is `formula_derivative(z, c, D.m, true)` (the
  snippet is linear in `dz`, and its Julia form drops the inhomogeneous
  term), keeping `D.e`. Then the parameter plane adds
  `f_c = formula_derivative(z, c, 0, false)` with the exponents aligned.
- **A BLA skip:** `D' = A D + B`, in extended range.
- **The summary:** `dz = D * S`, with `S = s_m 2^s_e`. That product is
  moderate at escape, and converts to f32.

## Coverage

These are the perturbable formulas that define a derivative:
- Mandelbrot and Multibrot (Power);
- Lambda;
- McMullen (Julia only);
- Tricorn.

Burning Ship, Phoenix, Manowar and the rest define none, on any path. The
panel already says so for them.

## Memory

The derivative's resume fields are spliced only when it is compiled in.
They are 8 B a pixel on the scaled rung and 16 B on the deep one. The
tiers that can carry it are all single-term (48 B, 56 B wide), so they
stay under the 80 B the render-pixel budget already assumes. No view's
antialiasing changes.

## Not in scope

- **Offset relief and Embossed at depth.** They need their neighbour
  orbits perturbed. Analytic relief, which this does bring to every
  depth, is the depth-proof relief.
- **The itinerary's address precision** (survey C8).

## Phases

1. The per-render-pixel convention on the direct path and in the
   consumers. The visual suite and the existing derivative tests are
   unchanged.
2. The scaled rung: step, BLA, resume, summary. Forced perturbed against
   direct for the four consumers, chunked against single, BLA on against
   off.
3. The floatexp rung, tested the same ways, plus depth renders at 2^60.
4. The panel's derivative-gap notes, the survey's P11, and the docs.

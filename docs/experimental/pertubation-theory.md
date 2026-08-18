
"Do you know much about rendering the Mandelbrot using pertubation theory?"

Yes — it's the technique that makes deep zooms (past ~1e-13, where f64 pixel spacing underflows) computationally feasible. The core insight, from K.I. Martin's SuperFractalThing paper (2013):

**The math.** Compute one reference orbit $Z_n$ at full arbitrary precision (MPFR or similar). For a nearby pixel $c = C + \delta_0$, write its orbit as $z_n = Z_n + \delta_n$. Substituting into $z_{n+1} = z_n^2 + c$:

$$\delta_{n+1} = 2Z_n\delta_n + \delta_n^2 + \delta_0$$

The arbitrary-precision terms cancel out, so $\delta_n$ can be iterated in plain f64 (or even f32 with rescaling), because $\delta$ is *relatively* tiny but representable — the exponent range is fine, it's the mantissa of the absolute position that isn't. One expensive orbit, millions of cheap ones.

**Glitches.** The classic failure mode: when $|Z_n + \delta_n| \ll |Z_n|$ (orbit passes near zero, i.e. near a critical point preimage), catastrophic cancellation makes the perturbed orbit diverge from truth. Pauldelbrot's heuristic detects this ($|z_n| < \varepsilon|Z_n|$, tolerance around 1e-3), and the traditional fix was choosing a new reference for glitched pixels and re-iterating — potentially recursively.

**Rebasing (Zhuoran, 2021).** The modern approach that mostly killed glitch handling: when $|Z_n + \delta_n| < |\delta_n|$, just reset — set $\delta \leftarrow Z_n + \delta_n$ and rewind the reference orbit index to 0. Works because the reference passes near the pixel's absolute position, so the full orbit value fits in low precision at that moment. One reference orbit suffices for the whole image; this is what fractalshades, Imagina, and newer renderers use.

**Skipping iterations.** Two generations:
- *Series approximation* (Martin's original): $\delta_n$ as a polynomial in $\delta_0$, valid while high-order terms stay negligible; skips thousands of iterations for all pixels at once. Finicky truncation-error control.
- *Bilinear approximation (BLA)*, Zhuoran again: precompute linear maps $\delta_{n+m} = A\delta_n + B\delta_0$ valid within a disk where $|\delta_n^2|$ is below rounding error, merge them in a binary-tree/doubling structure. More robust than SA, composes with rebasing, and each pixel skips adaptively based on its own $|\delta|$.

Also relevant: extended-range floats (f64 mantissa + i32 exponent, "floatexp") for zooms past ~1e300, and storing the reference orbit as f64 snapshots since only $2Z_n$ is needed per step.

Interesting question for your context: perturbation is inherently an escape-time technique — it exploits the smooth dependence of orbits on $c$. It doesn't map onto chaos-game IFS rendering, where there's no per-pixel orbit to linearize around. Are you thinking of adding an escape-time mode to the renderer, or is this curiosity?


"We have fragment shaders already that can render the Mandelbrot. 

I was curious about doing it all in f32 in the shaders, while Rust precomputes the reference orbit. I also want to consider rolling up our own floating point system that manages the mantissa and exponent so we can go beyond any f64 limits.

I'm also thinking about storing the precomputation or otherwise caching it if it takes a while to calculate (longer than 1 ms) live."

All three ideas are sound, and they compose into basically the architecture Fraktaler-3 (Claude Heiland-Allen's renderer — no relation) already validated: f32 deltas + extended-exponent types + CPU reference orbit. Some specifics:

**f32 in the shader — the constraint is WGSL anyway.** WebGPU has no f64, so on your stack the choice is made for you: everything GPU-side is f32 or hand-rolled. The good news is that perturbation is unusually f32-friendly. The delta iteration only needs *relative* precision (24-bit mantissa gives ~7 digits, enough for smooth coloring), and with Zhuoran-style rebasing the criterion $|Z_n + \delta_n| < |\delta_n|$ works fine in f32. The mantissa isn't the problem; the *exponent* is — at zoom 1e-40, $\delta_0$ underflows f32 immediately.

**The custom float: you want two representations, not one.**

1. *Floatexp*: `struct { m: f32, e: i32 }` with mantissa normalized to [1, 2). Mul is cheap (multiply mantissas, add exponents, conditional renorm). Add is expensive (align exponents via `exp2()` or exponent-field bit surgery with `bitcast<u32>` — do the bit surgery, `exp2` is slow and loses precision at extremes). This handles arbitrary zoom depth but costs maybe 5–10x plain f32.

2. *Scaled f32*: factor a per-frame scale out of the delta, $\delta = S \cdot d$, giving
$$d_{n+1} = 2Z_n d_n + S d_n^2 + d_0$$
where $S$ is a compile-time/uniform floatexp constant. While $S|d|^2$ underflows, you're in the linear regime and the term is legitimately negligible — the iteration is plain f32 speed. You only need floatexp when $|\delta|$ grows toward escape or after a rebase resets it to $|Z+\delta|$ (which can be tiny).

The standard structure is a hybrid inner loop: iterate in scaled f32, detect when magnitude leaves the safe band, renormalize (adjust $S$, or switch representation). Fraktaler-3 ships exactly this ladder of number types and picks per zoom depth.

**Reference orbit storage.** Only $2Z_n$ enters the linear term, so relative error in the stored orbit is what matters — f32 mantissas suffice, *except* near orbit passes close to zero, where you need the exponent range. Store the orbit as floatexp pairs in a storage buffer (16 bytes/iteration complex). With rebasing you wrap the reference index, and if you find the minibrot nucleus via Newton's method first, the orbit array length equals the *period*, which at deep zooms is vastly shorter than maxiter — this is the single biggest memory/precompute win. Worth doing: Newton root-finding on the CPU to relocate the reference to the nucleus, which also makes the reference maximally glitch-resistant.

**Precompute cost and caching.** At 1000-bit precision (zoom ~1e-300), each MPFR iteration is on the order of a few μs, so a 1M-iteration orbit is seconds — far past your 1ms bar, and it grows superlinearly with depth. So:

- Key the cache on (center coordinates at full precision — hash the decimal/binary string, precision bits, maxiter). During a continuous zoom the center is *unchanged*, so one orbit serves hundreds of frames; you only recompute on pan or when maxiter must grow, and growing maxiter is an *append* to the existing orbit, not a recompute — keep the MPFR state alive.
- Compute on a worker thread, upload to the GPU buffer in chunks, and render progressively — pixels that exhaust the uploaded prefix just come back next frame. This keeps the UI live during the multi-second deep-zoom precompute.
- Persisting to disk is worth it for bookmarked locations: the orbit serializes as the floatexp array you'd upload anyway, plus you can store the BLA table beside it. But BLA table construction from an existing orbit is fast (O(n) merges), so orbit-only persistence is probably enough.

For the Rust side, `rug` (MPFR bindings) is the fast option; `dashu`/`astro-float` if you want pure Rust and can eat ~2–5x. Newton for nucleus/period finding needs the derivative orbit too, but that's the same precision machinery.

One flame-renderer note: your existing f32 fragment-shader Mandelbrot can keep its direct path for shallow zooms — perturbation only pays past ~1e-5-ish pixel spacing where direct f32 visibly shreds. A depth-based switch between direct / perturbed-scaled-f32 / perturbed-floatexp gives you full speed at every range.


"Can this be reused for other escape time and orbit trapping fractals? I'd like to add them as a serious feature, they're currently just shaders with a few adjustable parameters."

Mostly yes — perturbation is a property of the formula, not of the Mandelbrot specifically, and orbit trapping composes with it for free. Breakdown by case:

**Trivially reusable**

- *Julia sets*: even easier than Mandelbrot — $c$ is constant so the $\delta_0$ term drops out of the iteration entirely; only the seed differs per pixel.
- *Higher powers* $z^p + c$: binomial expansion, $\delta_{n+1} = \sum_{k=1}^{p}\binom{p}{k}Z_n^{p-k}\delta_n^k + \delta_0$. Precompute the $Z_n^{p-k}$ per reference iteration. Works for integer $p$; BLA still applies (the linear coefficient is $pZ_n^{p-1}$).
- *Tricorn/Mandelbar* ($\bar{z}^2 + c$): conjugation is ℝ-linear, so the perturbed iteration just conjugates $\delta$ too. Note BLA coefficients become ℝ-linear 2×2 matrices instead of complex scalars — Fraktaler-3 handles this by storing BLAs as real matrices generally.

**Reusable with case analysis: abs-type fractals**

Burning Ship, Perpendicular variants, Celtic, Buffalo — anything built from $|x|, |y|$. The trick (worked out by laser blaster on fractalforums, implemented in Kalles Fraktaler): $|X + x| - |X|$ is piecewise linear in the perturbation — it equals $+x$, $-x$, or $\pm(2X + x)$ type expressions depending on signs, and a `diffabs(X, x)` helper with three branches computes it exactly, no precision loss. The perturbed Burning Ship iteration is just the complex-arithmetic version with `diffabs` substituted per component. Consequences:

- Slightly more reference data per iteration (you need $X_n, Y_n$ components anyway).
- BLA still works but validity radii shrink near the fold lines (axes) — a BLA step is only valid while the sign case doesn't change, so deep skipping is less effective near the "hull" details. Single-step fallback there.

**Reusable with more care**

- *Transcendental formulas* ($e^z + c$, $\sin z + c$): perturbation identities exist ($e^{Z+\delta} = e^Z(e^\delta - 1) + e^Z$, with $e^\delta - 1$ = `expm1`), but the exponent dynamics are violent — floatexp is mandatory early and glitch behavior is less well understood. Feasible, niche.
- *Convergent fractals* (Nova, Newton, Magnet's convergent regions): the perturbation algebra works (rational functions expand fine), but the *glitch/rebase criterion* changes — orbits converge toward roots rather than escaping, so "$|Z+\delta|$ small vs $|\delta|$" needs rethinking near attracting fixed points. Zhuoran-style rebasing has been adapted (Imagina does Nova), but it's the least-paved road.

**What composes for free once $\delta$ is right**

Orbit trapping, smooth iteration coloring, exterior distance estimation, stripe average / triangle inequality coloring — all of these only need the *full orbit value* $z_n = Z_n + \delta_n$, which you reconstruct each iteration with one add anyway (you already need it for the rebase test). Trap distance is computed against $Z_n + \delta_n$ at f32 precision, which is fine because trap geometry lives at O(1) scale, not pixel scale. Distance estimation additionally needs the derivative orbit $\frac{dz_n}{dc}$, which perturbs the same way — one more sequence alongside the reference.

**Architecture suggestion**

This points at a `Formula` trait with roughly:

- high-precision reference step (CPU, MPFR),
- perturbed step in scaled-f32 and floatexp flavors (WGSL — generated or hand-written per formula),
- BLA coefficient extraction (linear part as a real 2×2, since abs/conjugate formulas break ℂ-linearity) + validity radius,
- rebase/glitch criterion (escape vs convergent variant).

Then coloring/trapping is a separate orthogonal stage consuming $(z_n, \frac{dz}{dc}, n, \text{escape info})$ — which matches how you'd want the feature to feel in the UI anyway: formula × coloring algorithm × traps as independent axes, like Ultra Fractal's formula/coloring split rather than one monolithic shader per fractal.

The practical ordering: Mandelbrot + Julia first (pure complex arithmetic, BLA clean), Burning Ship family second (`diffabs`, matrix BLAs — this covers the bulk of what people actually zoom), power-N and hybrids third, convergent formulas last or never. Fraktaler-3's source is the best reference implementation for nearly all of this, including hybrid formula loops with per-phase BLAs, and it's public (AGPL — fine to read for the math, mind the license if copying code).
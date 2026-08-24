Research results first, then the missed-fractals sweep, then the bridge.

## The researched items

**Mandelbrot Cartoon (Bagula).** I pulled the actual notebook apart. The math inside is: a piecewise-linear generator on [0,1] — $f(x) = 0$ on $[0,\frac13]$, $6x-2$ on $(\frac13,\frac12]$, $-6x+4$ on $(\frac12,\frac23]$, $0$ on $(\frac23,1]$ — i.e. a single Koch "bump" as a tent function. That's Mandelbrot's *cartoon* idiom (his piecewise-linear recursive generators, as in the Brownian-motion cartoons from *Gaussian Self-Affinity and Fractals*) — hence the name. Then:

1. Periodize: $\hat f(x) = f(|x| \bmod 1)$.
2. "Minimum field": $g(x,y) = \min(\hat f(x)\hat f(y),\ \hat f(x)+\hat f(y))$.
3. Lacunary sum with the Koch dimension as scaling: $s_0 = \log 2/\log 3$, $F(x,y) = \sum_{k=0}^{20} g(3^k x, 3^k y)\,/\,3^{s_0 k}$.
4. He then plots three phase-shifted copies of $F$ as an (X,Y,Z) point cloud / surface.

So it's **not** escape-time and not chaos-game — it's a **Weierstrass-type self-affine field**, evaluated directly per point. (One caveat if you reimplement: his `gg[x_,y_]` actually passes `x` twice — `g[Mod[Abs[x],1], Mod[Abs[x],1]]` — so the published surfaces are 1D fields in disguise. Decide deliberately whether to reproduce the bug or the evident intent.)

**Besicovitch–Ursell.** Same family, and that's the punchline: B–U (1937) is the generalization of Weierstrass graphs to $\sum a_k^{-s} g(b_k x)$ with general lacunary sequences and generators, studied for graph dimension. Bagula's construction *is* a 2D Besicovitch–Ursell surface with a Koch-cartoon generator. So both research items collapse into one implementable thing, and it's a **perfect fragment-shader match**: ~20-term per-pixel sum, trivially parallel, no iteration state, no precision problems (the sum converges absolutely; f32 is plenty). This suggests a third fragment mode alongside escape-time: a **field mode** — evaluate $F(x,y)$ directly, color by value / analytic gradient (normals for hillshading come free by differentiating the sum) / contour bands / trap distance. One `FormulaDef` covers Weierstrass, Weierstrass–Mandelbrot, B–U, Riemann's function, Takagi/blancmange, and Bagula fields as generator+sequence presets.

**The tetration name cluster.** Those names (Tower Julia, Tetration Star, Schröder's Basin, Halley's Comet, Biomorph Tower, Root-finder Alloy, Oscillating Tower, Deep Tetration Web) are Daniel Geisler's — his tetration.org fractal atlas names identifiable features of the infinite-tetration fractal, the set of points where $^\infty x$ becomes periodic rather than escaping. They're gallery/feature names, not distinct algorithms. Mathematically the whole cluster is *one* formula family with axes:

- **Map**: $w \mapsto c^w = e^{w\log c}$. Pixel = $c$ (parameter space: the tetration fractal, "Tetration Star," "Deep Tetration Web" are regions/zooms of it) or pixel = $w_0$ with fixed $c$ ("Tower Julia").
- **Classification**: three-way, and this is the distinctive part — converge (multiplier test $|w_{n+1}-w_n| < \varepsilon$), escape ($\mathrm{Re}(w) \to +\infty$; test real part, not modulus), or **period-p oscillation** — Geisler's signature coloring is by detected cycle period ("Oscillating Tower"). Cycle detection = Brent's algorithm or compare against $w_{n-p}$ for small p, per-pixel in the shader.
- **Iteration scheme**: direct iteration vs. root-finders applied to the fixed-point equation $w = c^w$ — Newton, Halley (second in the class of Householder's methods, cubically convergent) ("Halley's Comet"), and Schröder's method ("Schröder's Basin"). "Root-finder Alloy" is a *hybrid* — alternating/blended root-finders, same idea as KF/Fraktaler-3 hybrid formula loops.
- **Escape condition**: "Biomorph Tower" = Pickover's biomorph trick (classify on $|\mathrm{Re}\,z|$ or $|\mathrm{Im}\,z|$ exceeding the bailout individually rather than $|z|$) applied to the tower map. Biomorphs generally are worth having as a *toggle on every formula*, not a formula — Bourke's biomorph page runs the same trick over $\sin z + e^z + c$, $z^5+c$, $z^z + z^6 + c$, etc.

So your planned "Power Tower / Deep Tetration Web" entries plus this cluster = one `FormulaDef` (complex pow with overflow guard — clamp $\mathrm{Re}(w\log c)$ before `exp`) × classification options × iteration-scheme options. f32 is fine; nobody deep-zooms these, so no perturbation machinery needed.

## Escape-time fractals you missed

Worth adding, roughly in order of value-per-effort:

- **McMullen family** $z \mapsto z^n + c/z^m$ — the biggest omission. Rational maps with Sierpiński-carpet Julia sets, heavily studied, visually distinctive, trivial in a shader, and perturbation-compatible if you ever want depth.
- **Lambda / logistic parameter plane** $z \mapsto \lambda z(1-z)$ — conformally conjugate to Mandelbrot but the $\lambda$-plane layout (period-1 disk, tangent disks) looks different and is a classic.
- **Feather fractal** $z \mapsto z^3/(1+\bar z^2\text{-ish terms}) + c$ and kin from Fractal Forums' "new theories" threads — modern community favorites alongside Burning Ship.
- **Fractint legacy set**: Spider ($z\to z^2+c$, $c\to c/2+z$), Manowar, Barnsley M1–M3 (conditional affine — escape-time renderings of IFS-like maps, directly relevant to your bridge), Frothy Basin (Loewer), Volterra–Lotka, Unity, Cactus. Cheap to add once the trait exists; big nostalgia coverage.
- **Collatz fractal** — the $\frac{1}{4}(2 + 7z - (2+5z)\cos\pi z)$ interpolation, iterated. Obscure-famous, great conversation piece.
- **Root-finder completions**: you have Newton/Nova and (via Geisler) Halley/Schröder; round out with Householder-3, secant (state = two previous iterates, still fine per-pixel), Chebyshev, and the König family — plus a complex *relaxation* parameter (generalized Newton $z - a\,p/p'$ over the $a$-plane) which is where the good Nova-like galleries live.
- **Multicorns** (higher-power tricorns) — presumably a parameter on your Tricorn entry; just make sure it's exposed.
- **Iteration-scheme axis**: the academic Mann/Ishikawa/Picard-variant fractal literature (e.g. Picard–Abbas iteration applied to $z^{k+1}+c$ Mandelbrot/Julia/biomorph sets) amounts to: replace $z\to f(z)$ with $z\to(1-\alpha)z+\alpha f(z)$ and friends. As with biomorphs, implement once as a modifier applicable to *every* formula — two float params, whole families of published variants fall out.
- **Reconsider Lyapunov**: Markus–Lyapunov fractals map stability vs. chaos of the logistic map with $r$ periodically switching between A and B, over the A–B plane. It's *purely* per-pixel (finite orbit + running $\sum \log|f'|$), needs no perturbation, no special precision — honestly one of the best fragment-shader fits in the whole zoo. The "own coloring semantics" is just: signed scalar field → diverging palette. I'd unpark it; it's cheaper than Magnet.

## Other fractal *types* that fit fragment shaders

Beyond escape-time and the field mode above:

- **Distance-estimated limit sets**: Kleinian/quasifuchsian groups via Jos Leys / knighty's Maskit-slice pseudo-DE, and Apollonian gasket / circle-inversion fractals via iterated inversion with escape or DE. Pixel-parallel, shader-native, and — note — these are the *same objects* your flame engine can render as Möbius IFS attractors, which makes them bridge exemplars (below).
- **Basin-boundary / Wada fractals**: the magnetic pendulum (per-pixel ODE integration to one of 3 attractors) — physically meaningful Wada basins, a Shadertoy classic, embarrassingly parallel.
- **FTLE / stability maps of area-preserving maps** (Chirikov standard map island structure): per-pixel finite-time Lyapunov of any 2D map. Generalizes the Lyapunov mode.
- **Domain coloring** of complex functions — not fractal per se but the identical pipeline (per-pixel complex evaluation → coloring), and iterated-function domain coloring shades into your escape-time modes anyway.

## Crossing the bridge to chaos game

The deep statement: **contractive IFS and expanding dynamics are inverse descriptions of the same object**, and each rendering paradigm samples a different measure on it.

**Direction 1 — escape-time objects into the flame engine: inverse iteration.** A Julia set is the attractor of the multivalued inverse IFS $\{\pm\sqrt{z-c}\}$. Run the chaos game on random inverse branches (IIM) and the flame pipeline renders Julia sets natively — in fact it already does: the `julia`/`julian`/`juliascope` variations *are* random-branch root maps. This generalizes to any rational map (random inverse branch of degree-d map = d-map IFS). The catch is measure: naive IIM samples the balanced measure, which starves visually important regions — the fix is MIIM (depth-limited by the derivative, Peitgen–Richter era), which in flame terms is a branch-weighting scheme. This would be a genuinely novel-ish feature: *escape-time formulas as flame variations via their inverse branches*, with density coloring instead of escape coloring.

**Direction 2 — IFS attractors per-pixel: escape-time IFS.** This is exactly the Hepting–Hart escape-buffer material you already know: for a hyperbolic, (mostly) non-overlapping IFS, iterate the expanding inverse of the union map per pixel and classify escape / measure distance. Gives you fragment-shader rendering of *arbitrary flame-style IFS attractors* with exterior distance fields — i.e., anti-aliased, solid-colored, DE-shaded versions of objects the chaos game renders as density clouds. Fractint's escape-time Sierpiński and the Barnsley types are the primitive ancestors of this.

**The exemplars already on your list**: Kaliset ($z \to |z|/\langle z,z\rangle + c$) and Ducks/Kali-log are morally escape-time renderings of expanding folded maps — they're what "inverse IFS + escape statistics" looks like when the map has folds. That's *why* they produce IFS-flavored imagery from a per-pixel loop. Littlewood is the same story from the other side: brute-force root plotting (density) vs. the Barnsley–Harrington two-map IFS characterization (chaos game) vs. potentially an escape-time rendering of the expanding dual.

**Practical bridges worth building:**
1. Shared coloring/tonemap stage — escape-time output can optionally route through the flame log-density/palette/tonemap pipeline for a unified look.
2. Escape-time → density: render orbits (not just final classification) into the flame accumulation buffer — that's Buddhabrot generalized to every formula in the catalog, and it reuses your existing histogram machinery rather than needing a per-pixel scatter (your Phase-4 note, but broader than Mandelbrot).
3. Flame → per-pixel: a "DE mode" for flames whose IFS qualifies (non-overlapping, invertible affine+variation), via Hart.
4. Post-warps: render escape-time to texture, apply flame variations as image-space warps (and conversely, escape-time fields as image traps in escape-time coloring).

If you want a unifying mental model for the whole feature: three fragment modes — **iterate-and-classify** (escape/converge/oscillate), **field evaluation** (Weierstrass/B–U/Bagula, Lyapunov, FTLE), **DE raycast-in-2D** (limit sets, escape-time IFS) — with orthogonal axes of formula × iteration scheme × classification × coloring/traps, and the bridge features living where the flame engine's measure-sampling meets the fragment engine's set-sampling.


"Does this help with the escape time IFS implementation? https://graphicsinterface.org/wp-content/uploads/gi1995-24.pdf"

Yes — quite directly, and in a way the authors couldn't have appreciated in 1995: the escape buffer *is* a texture-feedback fragment shader. They were describing GPU ping-pong rendering a decade early.

**What the paper actually contributes.** Prior escape-time-for-IFS methods were per-pixel backward algorithms: iterate inverse maps from each pixel, either choosing the right inverse via hand-designed spatial regions (Prusinkiewicz–Sandness, only workable for trivial IFSs) or traversing all $N^n$ branches with pruning (Hepting et al. 1991, which goes exponential wherever the images of the infinity circle overlap). The escape buffer replaces all of that with an image-space fixed-point iteration:

$$E(x) \leftarrow \max\big(E(x),\ \max_i\, E(S_i^{-1}(x)) + 1\big)$$

seeded with the continuous residual $\mathrm{res}_i(x) = \frac{\log R - \log\|x\|}{\log\|T_i^{-1}(x)\| - \log\|x\|}$ inside the annulus. That's their whole algorithm (their Figure 3, six lines).

**The GPU mapping is one-to-one.** Each iteration of line 3 is a fullscreen pass: sample the previous buffer at $N$ inverse-mapped positions, take max+1, write. Ping-pong two `r32float` textures. Their sequential version is Gauss–Seidel (in-place, propagation within a pass); the GPU version is Jacobi (ping-pong), which needs somewhat more passes but each pass is a single fullscreen draw at millions of pixels/pass — the trade wins enormously. Pass count is their Eq. 12: $n = \lceil \log(p/R)/\log\lambda \rceil$ for precision $p$ and max Lipschitz constant $\lambda$ — for $\lambda = 0.5$ and pixel precision at 4K, ~12–13 passes. Total cost: milliseconds. And they note the algorithm can operate solely in screen coordinates with the IFS conjugated by the window-to-viewport map, which is exactly how you'd write the shader anyway.

Three properties make it the right choice over porting the per-pixel tree traversal into WGSL:

1. **Robust to open-set violation.** An IFS that fails the open-set property adversely affects both the regional and tree-traversal methods but does not impact the escape buffer. This matters a lot for you: flame-style IFSs violate open-set constantly (overlapping transforms are the aesthetic norm). Constant cost regardless.
2. **No divergent control flow.** Per-pixel tree traversal is a warp-divergence nightmare — your SIMT research applies. The escape buffer is $N$ coherent texture fetches per pixel per pass, uniform control flow, textbook GPU-friendly.
3. **RIFS support maps to MRT/texture arrays.** The recurrent version just keeps a separate escape buffer per map, gathering only along edges $(i,j) \in G$, then maxes them at the end — that's a texture array with $N$ layers and per-layer gather masks. Xaos-style linked flames drop straight in.

**The caveats, honestly assessed:**

- *Affine-only as written.* The recurrence only needs each map to be invertible, so it extends to affine+variation transforms where the variation has a closed-form inverse (spherical, swirl, Möbius, polar do; sinusoidal and most folds don't). So this mode covers the invertible-IFS subset of your catalog, not arbitrary flames. That's still a large and interesting subset.
- *$\lambda \to 1$ blows up the pass count.* Their own benchmarks show the grid method beating the escape buffer at Lip 0.9. Eq. 12 tells you exactly when: near-isometric transforms need hundreds of passes. Your existing per-transform SVD contractivity measurement is precisely the input for deciding whether the mode is viable for a given flame — nice synergy with machinery you've already built.
- *Bounding-disk condition.* You need $R$ with $\mathcal{T}(D_R) \subset D_R$, and they note the smallest such disk isn't the smallest disk containing the attractor. With nonlinear variations you'll want a conservative bound — again your contractivity/Lyapunov prepass work.
- *WebGPU filtering wrinkle they couldn't know about:* sampling the buffer at non-integer inverse-mapped positions wants bilinear filtering, but `float32-filterable` is an optional wgpu feature. Fallback: manual 4-tap bilinear or `r16float` (escape-time values are small integers plus a [0,1) residual, so fp16 is actually sufficient).

**Two forward pointers from the paper worth following.** First, their future-work note — "if a similar forward algorithm can be constructed around distance instead of escape time, the result would greatly increase the efficiency of computing the distance transform of linear fractals" — got answered twice in the intervening decades: analytically by the KIFS/fold-space distance-estimation tradition (Knighty's kaleidoscopic IFS, the Mandelbulb-community lineage — per-pixel inverse iteration tracking accumulated scale, no multi-pass needed, arguably the modern production form of this whole idea), and computationally by jump flooding — chaos-game-render the attractor as a seed mask, JFA it into an exterior distance field in $O(\log n)$ passes. That last one is a bridge you get almost free: your flame engine produces the seeds, a JFA post-pass produces a DE field, and every escape-time coloring algorithm (DE shading, contours, traps) then applies to *arbitrary* flames, invertibility be damned.

Second, the index maps (their Plate 1): tracking *which* transformation applied at each level gives symbolic-address coloring — the escape-time analogue of flame xform color, and visually it's what makes these renders read as structured rather than flat level sets.

So: yes. I'd implement the escape buffer as the "IFS escape mode" — it's architecturally a multi-pass feedback pipeline rather than your single-pass escape-time shaders, but it's small (one shader, two textures, a loop), and it's the historically-correct ancestor of the KIFS-DE approach you'd want eventually anyway.
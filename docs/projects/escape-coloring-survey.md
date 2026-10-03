# Escape-time colouring: how other renderers do it, and what we could add

Status: **survey done, decisions made; items 1–5 of the order of work
done** (fixes, smooth count, palette mapping, accumulator and
debanding, texture layer), **item 6 in large part** (ten new or extended
colourings) and **item 7 in part** (R1–R4, R8); what remains is listed
under those items in §7 (2026-10-02).
This compares our escape-time colouring with four other programs and lists
what we could add (§5). The decisions and the order of work are in §7.

**Sources**, read from source or official documentation (nothing was built
or run):

- **techmatt** (Matt Fisher): the Rust engine in
  [fractal-wallpapers](https://github.com/techmatt/fractal-wallpapers) at
  `f0a993af` (`engine/src/mode.rs`, `iterate.rs`, `field.rs`, `coloring.rs`,
  `direct_trap.rs`, `derive.rs`). Its web explorer in
  [fractals](https://github.com/techmatt/fractals) at `32741f0a`
  (`explorer/`) compiles that engine to WebAssembly and is the program with
  the 13 modes and the Curve menu. The older
  [fractal-generator](https://github.com/techmatt/fractal-generator) has the
  same formulas in an earlier form.
- **Kalles Fraktaler 2** (KF2): `code.mathr.co.uk/kalles-fraktaler-2.git`,
  branch `kf-2.15` at `642cef4`. CPU colouring is in
  `fraktal_sft/fraktal_sft.cpp` (`SetColor`, lines 713–1163), the GLSL
  library in `gl/kf.frag.glsl`, and the manual in `README.md`.
- **Fraktaler 3** (F3): `code.mathr.co.uk/fraktaler-3.git` at `8d60912`
  (v3.1). The colouring interface is `src/colour.frag.glsl`; the per-pixel
  data is computed in `src/hybrid.cc:345-400`; examples are in `examples/`.
- **Ultra Fractal 6** (UF): the online help
  (`ultrafractal.com/help/coloring/standard.html` and its pages), plus the
  official Formula Reference, which publishes the source of every standard
  algorithm (`ultrafractal.com/formulas/reference/Standard/...`). UF's plain
  `.ucl` sources could not be fetched, so a few details exist only in the
  plug-in versions.

---

## 1. Summary

1. **Every other renderer puts a transfer step between the colouring value
   and the palette; we have none.**
   - The four curves you saw (Linear, Square Root, Log, S-curve) are
     techmatt's: `x`, `√x`, `ln(1+x)/ln 2`, `x²(3−2x)`.
   - UF has ten: Linear, Sqr, Sqrt, Cube, CubeRoot, Log, Exp, Sin, ArcTan,
     and None (solid colour).
   - KF2 has twelve "colour methods", including Log-Log, ArcTan, fourth root
     and three distance-estimate variants.
   - This is the cheapest broad improvement on the list: one step that every
     colouring passes through.

2. **Our smooth count was the plain textbook form, and our bailout was
   small.** (Both fixed since: §7, item 2.)
   - Ours was `n + 1 − log2(log2|z|)`, with no correction for the formula's
     power.
   - All four others divide by `ln(power)`.
   - They also normalise by the escape radius, `ln(ln|z| / ln R)`. An
     earlier draft of this survey said that keeps the value from moving
     when the radius changes. It does the opposite: it shifts every pixel
     by `log_p(ln R)`, so a new bailout slides the palette. What it buys is
     bands that line up with whole iterations. The unnormalised form is
     the one that settles as the radius grows (measured, below), so we
     keep it.
   - All four escape at a much larger radius:

     | Renderer | Escape radius |
     |---|---|
     | techmatt | 65,536 (2^16) |
     | KF2 "High" | 10,000 |
     | F3 | 625 |
     | UF Smooth | radius ≈ 11 (its bailout 128 is squared) |
     | UF Triangle Inequality Average | squared bailout 1e20 |
     | **Ours, new default** | 100 (squared bailout 1e4) |

   - The banding you saw at bailout 4 is the error of that formula at a small
     radius: it assumes `|z_{n+1}| ≈ |z_n|^p`, which ignores `+c`. The error
     shrinks quickly as the radius grows.
   - *Measured* (`dbg_smooth_count_error_against_bailout`): the worst
     pixel's error in iterations, against the same view escaped at a
     squared bailout of 1e15.

     | Squared bailout | 4 | 10 | 100 | 1e4 |
     |---|---|---|---|---|
     | Mandelbrot | 0.75 | 0.12 | 0.005 | 0.0000 |
     | Multibrot, p = 3 | 0.067 | 0.011 | 0.0001 | 0.0000 |
   - UF's reason for keeping the bailout as a colouring parameter is a
     conflict between colourings. Smooth colouring wants a large bailout.
     Binary decomposition, decomposition and UF's Basic want a small one (UF
     says 4). Orbit averages and traps include more iterations as the
     bailout grows, so they change too.

3. **Our orbit averages band at iteration boundaries; theirs don't.** UF's
   Triangle Inequality Average, techmatt's `deband`, KF2's TIA and F3's
   stripe window all blend two averages by the fractional part of the smooth
   count: the average with the last term and the average without it. Ours —
   stripe, triangle inequality, orbit average, magnitude average — are plain
   means, so they step at every iteration boundary.

4. **We draw one colouring at a time.** Most of techmatt's best-looking modes
   are composites: smooth, plus a texture field (stripes, curvature, lattice
   angle), blended with Screen or Add. UF builds the same looks from stacked
   layers with 20 merge modes. A second colouring layer with a blend mode
   would cover `smooth_stripe`, `smooth_curvature`, `smooth_mean_angle`,
   `smooth_angle_min` and `threads` in one feature.

5. **Relief.**
   - **Our source of slopes is the same as KF2's numeric option.** We
     difference neighbouring pixels of the colouring value. KF2's "Slopes"
     does the same in its numeric modes.
   - **Two sources we don't have.** KF2 and F3 can take the slope from an
     analytic distance-estimate vector. UF's Slope and Embossed run extra
     orbits at points offset slightly in the complex plane, and difference
     their results. That gives a relief tied to the fractal rather than to
     pixels, at 2–3× the iteration cost.
   - **What UF adds on top.**
     - A light elevation and an ambient term.
     - A choice of height source: potential, smallest |z| and its variants,
       distance estimate, smoothed iteration.
     - A transfer curve applied to the height.
     - The Embossed contour-bevel effect.
     - A grey lighting layer meant to be merged with Hard Light or Soft
       Light.
   - None of the four has specular highlights.

6. **Deep zoom loses our distance estimate; theirs doesn't.**
   - Under perturbation we iterate no derivative, so `distance_estimate` and
     `normal_map` go flat.
   - KF2 carries a Jacobian through perturbation and BLA. F3 carries
     forward-mode dual numbers through every transform. Both give a
     directional distance estimate in **pixel units** at any depth.
   - Their DE-based colouring, and KF2's analytic slopes, work at every zoom.

7. **The quick fixes come first.** §6 lists four problems in our current
   colouring, none needing new features: boundary darkening with
   supersampling, a self-contradictory light-direction comment, the
   triangle inequality reading the view centre as `c` in deep zoom, and
   colour seams where the palette wraps.

---

## 2. What we have now

From `src/escape/colorings.rs`, `src/escape/assembler.rs`,
`src/config/escape.rs` and `src/escape/renderer.rs`:

- **One colouring per render**, chosen from 14:
  - Escape Count, Smooth Iteration
  - Orbit Trap (point, cross, unit circle, log spiral; all fixed at the
    origin; minimum distance only)
  - Orbit Average (cross distance), Stripe Average, Magnitude Average,
    Position Average, Triangle Inequality, Sphere Average
  - Root Basin, Period, Distance Estimate
  - Normal Map (the classic `u = z/dz` emboss, as a colouring)
  - Position Map
- **A single `vec2` accumulator per pixel.** This limits what an orbit
  colouring can remember. Curvature, for example, needs the two previous
  iterates.
- **From value to colour:**
  - The value is `formula × scale`. It wraps with `fract`, or clamps for
    Bounded colourings.
  - A 256-entry palette table is sampled (rotation, squeeze and a log
    redistribution of the table are available), and the result is decoded
    from sRGB.
  - The shared Linear tone map then applies exposure and gamma.
  - **There is no transfer curve on the value.**
- **Auto contrast** (Off / AutoRange / Flatten) refits the value range from
  a percentile probe of the frame. It is the nearest thing we have to
  techmatt's percentile stretch. We have no histogram equalisation.
- **Interior** is drawn only by colourings flagged `ColorsInterior`; there is
  no separate inside colouring.
- **Relief** (`EscapeShading`) is a post-palette layer:
  - Central differences of the colouring's own value give the slope, with
    optional Gaussian softening.
  - The response is a signed tilt toward a light at one azimuth, with no
    elevation.
  - Shadow and highlight each have a colour, a strength and a blend mode
    (Multiply / Screen / Overlay / Mix).
  - Grain and paper noise textures.
- **Smooth count:** `n + 1 − log2(log2|z|)`, with no power correction and no
  bailout term.
- **The derivative** is iterated only on the direct path, and only for the
  12 of 26 formulas that define one. It is never iterated under
  perturbation.

---

## 3. How the others are organised

### techmatt

- **Pipeline:**
  - Iterate with bailout 2^16, updating every accumulator after each step.
  - Reduce the orbit to a scalar **field**.
  - Stretch it against the frame's own 0.5th–99.5th percentiles, or
    rank-equalise it.
  - Apply the **curve**.
  - Place it on the palette with gamma, cycles and phase.
  - Look it up in a 4,096-entry OKLab-interpolated colour map stored in
    linear light.
- **Composites:** smooth as the base, a texture field over it, a blend
  (Screen / Multiply / Add / Overlay / Min / Normal) and a weight. The blend
  happens before the palette, so both fields feed one palette lookup.
- **Debanding:** every average is debanded by the smooth fraction.
- **Direct traps** don't make a field. They paint during iteration: each
  iterate inside the trap threshold looks up a colour and composites it onto
  a start colour with Screen or Multiply, and they paint the interior.
- **Derived parameters:** the explorer derives texture weight and trap
  opacity from the view (a roughness or median-load probe).
- **No lighting**, by explicit design.

### Kalles Fraktaler 2

- **Data:** a post-process over per-pixel arrays — iteration count, smooth
  fraction, phase `T = arg(z)/2π`, and a directional distance-estimate
  vector — so recolouring never re-iterates.
- **Colour method** (a transfer applied to the smooth count or the distance
  estimate), then `/IterDiv + offset`, then phase, then a 1,024-entry palette
  built from up to 1,024 keys with cosine easing.
- **Distance estimate:** eight "Differences" modes, from finite-difference
  stencils (forward, central, Roberts cross, least squares, Laplacian) to
  analytic.
- **Slopes:** the gradient from those differences, or from the analytic
  distance-estimate vector, projected on the light, through `atan`, mixed
  toward black or white by a ratio. Normalised by image width / 640, so the
  look holds across resolutions.
- **Also:** "Infinite waves" (sinusoidal hue/saturation/brightness instead
  of a palette), image texture warp, and user GLSL colouring with the same
  inputs.

### Fraktaler 3

- **Colouring is a user GLSL function** over raw channels: iteration count,
  smooth fraction (using the power of the formula line that escaped), phase,
  a distance-estimate vector in pixel units from dual numbers, plus
  histogram access.
- **Per sample:** colour is computed for each jittered sample and averaged
  in linear float. Output is dithered 8-bit.
- **No palette system and no lighting.** The bundled shaders show the
  repertoire:
  - distance-estimate greyscale;
  - emndl (hue from smooth count, value from the distance estimate);
  - decomposition;
  - histogram;
  - stripes with a smooth window;
  - external-angle rays whose width scales by `2^−NF`, keeping lines
    continuous across bands;
  - rainbow fringe (hue from the direction of the distance-estimate vector).

### Ultra Fractal 6

- **Per tab:** a colouring returns an index; separate Inside and Outside
  tabs each have their own algorithm.
- **Index to colour:** × colour density → transfer function → + offset →
  wrap ("repeat gradient") or clamp → 400-entry gradient with per-colour
  opacity.
- **Direct colouring** returns a colour instead of an index.
- **Layers** stack with 20 merge modes and opacity, plus masks.
- **16 standard colourings:**
  - Basic, Binary Decomposition, Decomposition
  - Direct Orbit Traps, Orbit Traps
  - Distance Estimator, Exponential Smoothing
  - Gaussian Integer, Gradient, Image
  - Lighting, Emboss
  - None, Plug-In
  - Smooth, Triangle Inequality Average
- **Lighting:** Slope and Embossed are *formulas* that run extra offset
  orbits. Lighting and Emboss are colourings that turn the result into a
  grey layer.

---

## 4. Side by side

| Capability | Ours | techmatt | KF2 | F3 | UF6 |
|---|---|---|---|---|---|
| Transfer curve on the value | — | 4 | 12 | in shader | 10 |
| Smooth count corrected for power | yes (was —) | yes | yes | yes | yes |
| Smooth count normalised by bailout | — (on purpose, §1.2) | yes | yes | yes | yes |
| Default escape radius | 100 (was 2) | 65,536 | 10,000 | 625 | ≈ 11 |
| Averages debanded by smooth fraction | — | yes | TIA | stripes | TIA |
| Percentile stretch / auto range | AutoRange, Flatten | yes | "Stretched" | — | — |
| Histogram equalisation | — | rank | — | yes | — |
| Several colourings blended | — | composites | waves + palette | in shader | layers, 20 modes |
| Separate inside colouring | — | — | interior colour | in shader | Inside tab |
| Direct (per-iteration RGB) colouring | — | direct traps | — | — | Direct Orbit Traps |
| Orbit trap shapes | 4, at the origin | 8, at the origin | — | — | 19+, positioned |
| Distance estimate at deep zoom | — | — | yes | yes | — |
| Distance estimate in pixel units | — | — | yes | yes | — |
| Relief from neighbour pixels | yes | — | yes | — | — |
| Relief from analytic DE | — (normal-map colouring only) | — | yes | — | — |
| Relief from offset orbits | — | — | — | — | Slope, Embossed |
| Light elevation / ambient | — | — | — | — | yes |
| Palette interpolation | linear in sRGB values, 256 | OKLab, 4,096 | cosine keys, 1,024 | — | 400 entries |
| Phase (`arg z`) added to colour | — | — | yes | yes | Decomposition |
| Dithered 8-bit export | unchecked | — | yes | yes | — |

---

## 5. Candidates

Effort is a rough guess: **S** is a day or less, **M** a few days, **L**
needs an engine change first.

### 5.1 Pipeline changes (they improve every colouring)

| # | Candidate | What it is | From | Effort |
|---|---|---|---|---|
| P1 | **Transfer curve** | Applied to the value before the palette. Two sets to choose from: (a) techmatt's four, `x`, `√x`, `ln(1+x)/ln 2`, `x²(3−2x)`; (b) UF's ten, adding Sqr, Cube, CubeRoot, Exp, Sin, ArcTan; KF2 adds Log-Log `ln(1+ln(1+x))` and fourth root. Where it applies matters: techmatt curves a value already normalised to 0..1, while UF and KF2 curve the raw index and so need a density and offset around them. | all | S |
| P2 | **Corrected smooth count** | `n + 1 − ln(ln|z|) / ln p`, with p the formula's power, which removes the banding on power ≠ 2 formulas. The others' `ln(ln|z| / ln R)` would move the value with the bailout (§1.2). | all | S |
| P3 | **Larger default bailout for smooth colourings** | Or a bailout per colouring, as UF does, or a recommended value the panel suggests. Decomposition-style colourings keep a small one. | all | S |
| P4 | **Debanded averages** | Keep the last term and blend `mean_all` against `mean_without_last` by the smooth fraction. Needs a bigger accumulator (P6). | all | M |
| P5 | **A second colouring layer** | Base colouring + texture colouring, blended before the palette (techmatt: Screen / Multiply / Add / Overlay, weight), or two coloured layers merged after it (UF's modes). The first is cheaper and covers techmatt's composites. | techmatt, UF | M |
| P6 | **Bigger accumulator** | `vec4` or more per pixel instead of one `vec2`. It enables debanding, curvature (two previous iterates), Gaussian-integer reductions and itineraries. It costs registers and the recolour cache's memory. | — | M |
| P7 | **Histogram equalisation** | Rank-equalise the value over the frame. Our AutoRange probe already measures the frame. F3 uses a 4,096-bin CDF texture with linear filtering. Watch for flicker in animations. | techmatt, F3 | M |
| P8 | **Separate inside colouring** | Pick an interior colouring independently of the exterior one, like UF's Inside tab. | UF | M |
| P9 | **Phase add** | Palette position += strength × `arg(z)/2π`. With P2 in place the smooth fraction and phase line up into KF2's "exterior grid". | KF2 | S |
| P10 | **OKLab palette interpolation** | Plus a finer table (we resample our table nearest-index at every stage, so squeezing lowers its resolution). | techmatt | S |
| P11 | **Derivative under perturbation** | Carry dz/dc through perturbation and BLA as KF2 does, or use dual numbers as F3 does. Gives distance estimate, normal map and analytic relief at every depth, in pixel units. | KF2, F3 | L |

### 5.2 New colourings

| # | Colouring | What it computes | From | Needs |
|---|---|---|---|---|
| C1 | **Curvature average** | Mean of `|arg((z_n − z_{n−1}) / (z_{n−1} − z_{n−2}))|`, debanded. | techmatt | P6 |
| C2 | **Smooth + stripe, smooth + curvature** | Smooth base with a stripe or curvature texture, Screen-blended at weight 0.85. | techmatt | P5 (C1) |
| C3 | **Gaussian integer** | Distance from z to the nearest Gaussian integer. techmatt keeps nine reductions (min / avg / max distance, iteration at min / max, angle at min / max, mean angle, ratio). UF adds round / trunc / floor / ceil and a normalisation. Drives techmatt's `smooth_mean_angle` and `smooth_angle_min`; it also fills the interior. | techmatt, UF | P6 for most reductions |
| C4 | **Threads** | Debanded mean of `exp(−D²/σ²)` with D the distance to the axis cross, Add-blended over smooth. | techmatt | P4, P5 |
| C5 | **Exponential smoothing** | Divergent: `Σ exp(−|z|)`. Convergent: `Σ exp(−1/|z − z_prev|)`. The standard smooth colouring for Newton / Nova-type and mixed formulas; our convergent formulas only have Root Basin. | UF | — |
| C6 | **Decomposition, binary decomposition** | `arg(z)/2π` at escape; binary by sign of Im z (or of Re·Im); m-ary in F3. Wants a small bailout (P3). | UF, F3, KF2 example | — |
| C7 | **External-angle rays** | Lines where `|fract(T + ½) − ½| < w·2^−NF`; the width factor keeps lines continuous across bands. | F3 | — |
| C8 | **Itinerary** | Base-4 address of the first iterates' angular sectors, used to shift the palette phase of a rank-equalised smooth base. techmatt keeps 26 digits in f64; in f32 we'd keep about 12, or split into two floats. | techmatt | P6, P7 |
| C9 | **Richer orbit traps** | Trap position, rotation and aspect, and more shapes: ring with radius, box, rectangle, diamond, lines, hyperbola, hypercross, astroid, egg, heart, pinch, spiral, ripples, waves. Modes: closest, farthest, first, last, average, sum, product, exponential average, two closest. Colourings: distance, magnitude, real, imaginary, angle to trap, angle to origin, iteration. Ours has four shapes at the origin and minimum distance only. | UF, techmatt | some modes need P6 |
| C10 | **Direct orbit traps** | Composite a colour per iteration onto a base colour (Screen on dark, Multiply on light, opacity, threshold, closer = stronger). Paints the interior. Needs the iteration loop to look up the palette and carry RGB, which is a new output path. | UF, techmatt | L |
| C11 | **Distance-estimate variants** | KF2's Distance Linear / Log / Sqrt and "DE + Standard" (DE near the set, smooth count elsewhere); F3's `0.875 + log2|DE|/16` greyscale. Best in pixel units (P11, or at least scaled by the view). | KF2, F3 | P11 for deep zoom |
| C12 | **Rainbow fringe** | Hue = direction of the distance-estimate vector; saturation and value from its length. | F3, KF2 example | DE vector |
| C13 | **Infinite waves** | Several sinusoids of the smooth count, each driving hue, saturation or brightness, instead of a palette. | KF2 | — |
| C14 | **Basic real / imaginary / sum** | `0.05·(4 + Re z)` and its variants. A classic look; cheap. | UF | — |
| C15 | **Velocity** | Mean step length `|z_n − z_{n−1}|`. | techmatt | P6 |
| C16 | **Smoothed triangle inequality** | Ours, debanded as UF does it, and with the pixel's own c under perturbation (§6). | UF, KF2 | P4 |

### 5.3 Relief and lighting

| # | Candidate | What it is | From | Effort |
|---|---|---|---|---|
| R1 | **Light elevation and ambient** | Add an elevation angle (UF defaults to 30°) and an ambient floor. Possibly offer UF's Lambert response alongside our signed tilt. | UF | S |
| R2 | **Soft Light and Hard Light** | Add both to the shadow and highlight blends. UF recommends them for lighting layers: a flat area lands on mid-grey, which those modes leave unchanged. | UF | S |
| R3 | **Height from a different field than the colour** | Colour by stripes, relief by potential, smallest \|z\|, smallest \|Re z\| or \|Im z\|, distance estimate, or smoothed iteration (UF's Slope height values). Needs the second field computed alongside the colouring (P6). | UF | M |
| R4 | **Height transfer** | A curve applied to the height before differencing: log, sqrt, cuberoot, exp, sqr, cube, sin, cos, tan; plus pre- and post-scale. | UF | S |
| R5 | **Analytic relief** | Slope from the distance-estimate vector instead of neighbour pixels, as KF2 does in its Analytic mode. Sharper, and free of pixel-stencil artefacts. Shallow zoom only until P11. | KF2 | M |
| R6 | **Offset-orbit relief** | UF's Slope: run orbits at `c`, `c + δ` and `c + iδ`, and take the normal from the three heights. Relief belongs to the fractal, not the pixel grid, and works on any height value. Costs 3× the iteration. | UF | M–L |
| R7 | **Embossed contours** | UF's Embossed: two orbits at `c ± dr`, with `dr` scaled by 1/magnification, each giving an integer field. The field is one of: iteration count, count of iterates with Re z > 0 or Im z > 0, iteration of smallest \|z\|, trunc(ln\|z\|), or angle sector. The colour is three greys by which orbit came out higher, merged with Hard or Soft Light. Gives bevelled contour lines of constant on-screen width. | UF | M–L |
| R8 | **Differencing stencils** | KF2 offers forward, central, Roberts-cross diagonal, least squares and Laplacian, and corrects for jittered sample positions. We use central differences only. | KF2 | S each |
| R9 | **Resolution-independent strength** | KF2 scales slope strength by image width / 640, so a 4K export shades like the preview. Ours compensates only for supersampling; whether export resolution changes our relief needs checking (§6). | KF2 | S |
| R10 | **Image texture** | KF2 warps an imported image by the slope and mixes it in. We have procedural grain and paper only. | KF2 | M |

### 5.4 Output quality

| # | Candidate | What it is | From |
|---|---|---|---|
| O1 | **Ordered dither on 8-bit export** | `floor(255·s + (((x + 67c) + 236y)·119 & 255)/256)`, from pippin's a_dither. Removes palette banding in smooth gradients. Our export path needs checking. | KF2, F3 |
| O2 | **Interior detection by derivative** | F3 stops a pixel once the derivative of its orbit with respect to its start shrinks below 1/1024, an attracting-cycle test, then marks it interior. | F3 |

---

## 6. Found on the way, in our code

Read from the code, not reproduced by render, except where noted.

1. **Boundary darkening with supersampling** (confirmed by reading).
   - The downsample averages the samples' colour including the interior
     samples' zero colour, so the result is already scaled by coverage
     (`src/escape/renderer.rs` `downsample_main`).
   - The Linear tone map then composites `color × α` over the background
     again (`shaders/tonemap.wgsl:274, 536`).
   - A pixel half covered by the exterior gets a quarter of the exterior's
     colour, not half. The edge of the set darkens, or shows more
     background, whenever supersampling is on.
   - Fix: divide by coverage in the downsample, or treat escape colour as
     premultiplied in the tone map.
2. **The light direction comment contradicts itself** (confirmed by
   reading).
   - `EscapeShading::light_angle` is documented as "counter-clockwise from
     +x (east)", with the default 315 described as "north-west". Measured
     counter-clockwise from east, 315° is south-east.
   - The code builds `light = (cos a, sin a)` and compares it with the
     normal in a y-up frame, so the light comes from the screen's lower
     right.
   - Either the comment or the default is wrong. Since you're used to the
     current look, this is your call. The comment's own reasoning — relief
     lit from anywhere but the upper left reads as inverted — argues for
     changing the default.
3. **Triangle inequality under perturbation** uses the view centre as `c`,
   not the pixel's `c` (`assembler.rs` `c_f32 = params.center`). Deep views
   colour by a wrong average.
4. **Palette seam.** The value wraps with `fract`, but the palette texture
   clamps at its edges, so each cycle ends in a hard step from the last
   entry to the first. That's harmless for palettes whose ends match, and a
   visible seam otherwise. It may be intended.
5. **Stale comments and docs:**
   - The shading code says the elevation is fixed at 45°, but the code has no
     elevation term.
   - The design doc credits `normalize()` where the code uses `inverseSqrt`.
   - Code comments count formulas with a derivative as 11, 12 and 13 of 25;
     it is 12 of 26.
6. **Bounded colourings with Banded relief** store the clamped value as
   height in the iterate pass, but `fract(raw)` in the recolour pass.
7. **Relief strength depends on export resolution, but only a little.**
   - Slope is measured per pixel, and the strength is compensated for
     supersampling but not for output size.
   - *Measured* (`dbg_relief_strength_against_output_size`, smooth
     colouring near the set): at 800×600 relief moves the picture 0.79×
     as much as at 200×150, at heights 10 and 1 alike.
   - It is not the quarter a smooth field would give. The colouring
     steepens toward the set at every scale, so a larger render finds
     steeper slopes per pixel.
   - A linear correction by width, as KF2 does, would therefore overshoot
     about 3× here.
   - Left as it is. R9 is where a better-founded normalisation would go.
8. **The tone curve crushes the darkest values, flames included**
   (measured while testing item 3; not fixed, since it moves every
   render's dark pixels).
   - The curve is on by default (`use_curve`), and its 256-entry LUT
     holds `f(i/255)` in texel `i`. `tonemap.wgsl` samples it at
     `u = x` with linear filtering, which reads texel position
     `256x − 0.5`, so even the identity curve returns
     `x + (x − 0.5)/255`.
   - Near black that subtracts about 0.002 in linear light. Through an
     escape render: palette bytes below about 15 come out 0, 25 comes
     out 21, 51 comes out 49. Mid-grey is exact.
   - Fix: sample at `(255x + 0.5)/256`. The two item-3 GPU tests turn
     the curve off until then.
9. **Four IFS cache tests failed when run in parallel** and passed
   alone: they read the global `escape::diag` snapshot, which any
   other test's render rewrites between their render and their check.
   Fixed: they read their own renderer's `last_path`. A fifth,
   `a_preview_is_a_quarter_of_the_walks_and_leaves_no_trace`, times
   the walk and can still lose under parallel load.

---

## 7. Decisions and order of work

### Decisions (2026-10-02)

1. **Escape-time only for now.** No flame or Colors-panel changes. A curve
   on the colour index would be a palette-table warp for flames — what Log
   Redistribute already is — but the two engines don't share enough of the
   palette path to make the options general yet. Everything below lives in
   the escape panel.
2. **Bailout per colouring.** Each colouring declares a recommended bailout.
   The panel applies it when the colouring is picked, and the user can still
   edit it. The escape test stays one bailout per render, since it belongs
   to the formula.
3. **Transfer on the raw value** (P1), before the wrap, as UF and KF2 do.
   When Auto contrast is on, the transfer applies after its stretch, which
   gives techmatt's normalised behaviour as an option.
4. **Two kinds of curve, kept separate:**
   - **The value transfer** changes how far apart the palette cycles are
     across the picture.
   - **A palette curve** reshapes a single cycle; it is the escape-side
     counterpart of Log Redistribute.
5. **The palette's ends are the palette's business.** A user who wants the
   wrap to blend picks a palette whose ends match; there is no wrap option.
6. **Stepped palettes:** an option that draws each palette stop as a flat
   band (posterised), alongside the current blended lookup.
7. **Layers: blend before the palette** (P5, techmatt's composites). A
   second colouring and a blend mode mix into one value, which takes one
   palette lookup. Full coloured layers with merge modes stay possible
   later.
8. **Relief: all of R1–R10.**
9. **The light moves to the upper left.** The default becomes 135°
   (counter-clockwise from east), and the comment is corrected. Saved files
   keep their angle.

### Order of work

1. **Fixes** (§6). *Done 2026-10-02*, each its own commit on `main`:
   - boundary darkening (`3dc705b6`);
   - the light default and its comment (`6dbbc178`);
   - triangle inequality's `c` under perturbation (`e67e9ba7`). This
     turned out to matter in Julia mode, where the view centre is not c
     at all;
   - the Bounded / Banded height mismatch (`950bb136`);
   - stale comments (`710225e4`);
   - relief strength against export resolution: measured, and left as it
     is (§6.7).
2. **The smooth count** (P2): correct it for power, and give each
   colouring a recommended bailout (decision 2). *Done 2026-10-02*, on
   branch `escape-coloring`:
   - each formula reports its degree at infinity (`FormulaDef::escape_degree`:
     the power for Multibrot, Tricorn and McMullen, 3 for Cactus, power − 2
     for Feather). A degree of 1 or less, or none, keeps 2: linear growth
     has no log-log count. The count divides by `log2(degree)` only when
     it is not 2, so every degree-2 picture is unchanged;
   - smooth, distance estimate and normal map recommend a squared bailout
     of 1e4, which the panel sets when one of them is picked. Only for
     formulas that test `|z|^2` with no biomorph: the exponential and trig
     families test a raw `Re z` or `|Im z|`, and their presets now carry
     50 (10 for Collatz);
   - and only for a formula that is **polynomial at infinity** (it
     declares its degree): past radius 2 its orbits really escape, so a
     larger bailout refines the count without moving the escape set. Not
     so elsewhere. Magnet grows as `z^2` too, but its orbits can pass
     radius 2 and come back to converge, so a larger bailout redraws it.
     Feather at power 3 grows linearly and never reached radius 100 in
     600 iterations: a deep view rendered empty;
   - a preset that names no bailout takes its colouring's recommendation,
     or else 4, the bailout it was drawn at (it used to keep whatever the
     last picture had);
   - a new config starts at 1e4; a file without a bailout still reads as 4.
   - The escape GPU tests turned up three tests (Feather, two Magnet)
     that had failed since the default moved to 10 (`d4bfd323`): their
     exact-orbit references escape at 4 but took the default. They now
     name 4.
3. **The value transfer** (P1) and the palette curve, plus stepped palettes
   (decisions 3, 4 and 6). The design:
   - **Config:** `EscapeConfig::palette_map` (`PaletteMap`): `transfer`,
     `pivot`, `curve`, `stepped`; written only when not the default. The
     panel shows them in a *Palette mapping* section under Auto contrast.
   - **Value transfer**, on the colouring's value before it wraps:
     Linear, Square Root, Cube Root, Log, Log-Log, Square, ArcTan. Each
     is `g(v) = k·f(v/k)` with `f(1) = 1`, so the **pivot** `k` is the
     value every curve leaves unchanged: below it Square Root and Log
     stretch, above it they compress. The colouring's own scale still
     sets the density. Negative values are mirrored (`g(−v) = −g(v)`).
     `f` is techmatt's for Log (`log2(1 + u)`); KF2's for Log-Log; UF's
     for the rest. No Exp or Cube: past a few hundred they overflow f32
     or leave `fract` nothing but rounding.
   - **With Auto contrast** the transfer applies to the fitted value, in
     its 0..1 range, before the palette turns: `mix(g(raw), turns ·
     g(u), strength)`. At pivot 1 that is techmatt's normalised curve.
   - **Palette curve**, on the wrapped position within one cycle:
     Linear, Square Root, Square, Log (`log2(1 + t)`), Exp (`2^t − 1`),
     S-curve (smoothstep), Inverse S. Computed in the shader, so it
     does not lower the table's resolution as a table warp would.
   - **Relief** keeps the raw value, and Banded relief the wrapped value
     after the transfer (the band edges), not after the palette curve.
   - **Stepped:** each stop is a flat band from its position to the
     next stop (constant interpolation, as Blender's ColorRamp). A stop
     at 1.0 therefore shows only at the very top. The flame renderer
     keeps a second palette table built that way, through the same
     rotation, squeeze, log and reverse pipeline; an escape render binds
     it when stepped is on. The flame and simulation renders never see
     it.
   - **Shader:** one set of helpers (`esc_transfer`, `esc_wrap`,
     `esc_palette`) spliced into every template that looks up the
     palette, so the iterate and recolour passes cannot disagree. Two
     uniform words carry the choices; Linear and Linear take the old
     arithmetic exactly, so existing renders do not move.

   *Done 2026-10-02*, on branch `escape-coloring`, as designed, with a
   script API (`escape.transfer`, `escape.palette_curve`,
   `escape.stepped_palette`). Measured:
   - every curve, rendered through a grey ramp, lands within one 8-bit
     level of `TransferCurve::shape` / `PaletteCurve::apply` (worst
     0.004, Linear-on-Linear included);
   - a palette-map change takes the recolour path and matches a fresh
     render byte for byte: direct, perturbed, and under Auto contrast;
   - a four-stop stepped palette puts 98.5% of lit pixels on a stop
     colour (7% blended); the rest sit on band edges, which the
     sampler's filtering softens;
   - all 87 escape visual tests and the 2D flame tests pass unchanged.
4. **A bigger accumulator** (P6), then **debanded averages** (P4).
   *Done 2026-10-02*, on branch `escape-coloring`:
   - **The accumulator is a `vec4` to every colouring**
     (`coloring_map(sum, state: vec4)`, `coloring_accum(..) -> vec4`),
     but only an accumulating colouring stores all four floats: its
     records grow from 32 to 40 bytes a pixel and its perturbed resume
     state by 8. Everything else keeps the narrow layouts, so the
     deep-zoom pixel budget only shrinks while an averaging colouring
     is in use. One pre-pass per colouring resolves the width
     (`with_accum_width`); a test measures both layouts in the
     assembled shader against the Rust strides.
   - **Debanding**, on orbit, stripe, magnitude, position and
     triangle-inequality averages: each keeps (sum, count, last term)
     and blends the mean without the last term into the mean with it
     by how far through its last step the orbit escaped,
     `1 − log_p(ln|z| / ln R)`. That is Ultra Fractal's smoothing of its
     Triangle Inequality Average. Sphere average is left out: its
     stride counts calls, not terms.
   - **Saved pictures do not move.** The `deband` parameter's default
     is off, which is what a file without the key means. A fresh pick
     of the colouring turns it on, from the panel, a preset that does
     not name it, or a script (`ColoringDef::pick_params`, the same
     pattern as the recommended bailout).
   - **The bailout matters.** The blend is first order in the escape
     fraction. Measured as the step across an iteration boundary
     against the steps beside it (1 is continuous): stripes 1.22 →
     1.03 and triangle inequality 2.44 → 1.04 at bailout 1e4, but only
     3.37 → 1.62 and 16.5 → 4.9 at 4. So stripe and triangle inequality,
     whose terms are bounded, recommend 1e4 as smooth does. The
     averages of |z| and of positions do not, because a larger radius
     changes what they average.
5. **The texture layer** (P5): a second colouring blended before the
   palette. This brings techmatt's composites (C2, C3, C4). The design:
   - **Config:** `EscapeConfig::layer` (`ColoringLayer`): a colouring
     name (empty is no layer), its own parameters, a blend mode and a
     weight. Mode A only; the panel shows it under the colouring.
   - **Blend**, on the two wrapped palette positions, before the
     palette curve: Screen `1 − (1 − a)(1 − b)`, Multiply `ab`, Overlay,
     Mix `b`, each as `mix(a, blend, weight)`; and Add, `fract(a + w·b)`,
     which shifts the palette by the texture. The base's value
     transfer, contrast and relief height are the base's alone.
   - **Accumulator:** whichever of the two colourings accumulates owns
     it, so smooth + stripes, curvature or threads (techmatt's
     composites) all fit. Two accumulating colourings would need a
     second one; v1 refuses that pair (the panel greys it out, the
     renderer draws the base alone).
   - **Shader:** the layer's WGSL is spliced after the base's, with
     `coloring_map` renamed `layer_map`, `cparam` renamed `lparam` (a
     second parameter block in the uniform), and any helper function
     or constant prefixed. Period detection and the derivative orbit
     compile in when either colouring needs them. `esc_layer(t, ..)`
     is the identity without a layer, so existing renders do not move.
   - **Caches:** the layer is part of the pipeline keys and the band
     key, and of the recolour cache's identity when it accumulates or
     reads periods, exactly as the base colouring is.
   - **Script API:** `escape.layer(name, blend, weight)`,
     `escape.layer_param(name, value)`, `escape.no_layer()`.

   *Done 2026-10-02*, on branch `escape-coloring`, as designed. Measured:
   - every blend mode, rendered through a grey ramp with escape count
     as both colourings, lands within one 8-bit level of
     `LayerBlend::apply` (worst 0.004); at weight 0 a layer is
     byte-identical to none;
   - a layer that runs nothing in the loop recolours from the cache and
     matches a fresh render to the byte, including smooth over smooth.
     One that accumulates or needs the derivative orbit (distance
     estimate) re-iterates, as it must;
   - every fitting pair of the 14 colourings assembles and validates
     as base and layer, direct, perturbed and recolour; the 49 pairs of
     two accumulating colourings are refused.
6. **New colourings** (§5.2), in an order to be picked.
   *First batch done 2026-10-02*, on branch `escape-coloring`: the
   colourings that complete techmatt's composites now that layers
   exist, then the cheap classics.
   - **Curvature average** (C1), **velocity** (C15) and **threads**
     (C4), debanded like the other averages. Curvature and threads,
     whose terms are bounded, recommend a bailout of 1e4.
   - **Exponential smoothing** (C5): diverging, converging or both,
     the smooth colouring Newton and Nova lacked.
   - **Decomposition** (C6): the escape angle, or that many flat
     sectors of it; 2 is binary decomposition. Recommends the classic
     bailout of 4, as UF does, since the angle at escape depends on it.
   - **Basic** (C14): UF's real, imaginary and sum, `0.05 (4 + v)`,
     also at bailout 4.
   - **Gaussian integer** (C3): the distance to the nearest lattice
     point, reduced to its smallest, mean, largest, or the angle at the
     smallest. It colours the interior.

   Each is checked against a transcription of its definition: the
   accumulator by replaying the orbit on the CPU, the map through a grey
   ramp. Both agree within one 8-bit level. One caveat measured on the
   way: an escaping orbit's last iterates are so sensitive to c that at
   bailout 1e4 the Gaussian-integer distance of the final step is f32
   noise. f32 and f64 disagree on it, so that colouring's look at a
   large radius depends on rounding; at 4 they agree.
   Visual tests added for a curvature layer over smooth, debanded
   stripes, binary decomposition, Gaussian integer, exponential
   smoothing on Newton, and a log transfer with a stepped palette.

   *Second batch done*, same branch:
   - **Distance-estimate variants** (C11): log (as before), linear and
     square-root mappings, in plane units (as before) or pixels. In
     pixels the boundary is always about a pixel away, so the colours
     hold still under zoom, as F3 and KF2 draw it. KF2's
     "DE + Standard" is a texture layer now.
   - **Richer orbit traps** (C9), appended so a saved trap draws as it
     did: the trap's centre and rotation; a ring, box, line and
     diamond with a radius; and the reduction: closest (as before),
     farthest, mean, or first within a threshold.
   Checked the same way, within one 8-bit level; two visual tests.

   - **External rays** (C7), from F3's own example (`flying-fish.f3.toml`,
     read from source): lines where `|fract(T + ½) − ½| < w · (1/p)^NF`,
     `T` the escape angle in turns and `NF` the escape fraction, which
     F3 computes exactly as the debanding here does. 1 on a ray, 0 off,
     meant as a texture layer; a count of rays per turn generalises F3's
     one. Checked against the definition on every escaped pixel of a
     view (none disagree); one visual test.

   *Third batch: colourings that draw their own colours.*
   - **The colour path.** A colouring flagged `DirectColor` defines
     `coloring_color(sum, state, v) -> vec3` (linear light, `v` its
     value after the transfer) beside `coloring_map`. The value
     `coloring_map` returns still feeds the relief and Auto contrast.
     Every template that colours a pixel calls one spliced
     `esc_colour(raw, t, ..)`, which for every other colouring is the
     palette lookup it always was. All 99 earlier escape pictures are
     byte-identical.
     - The recolour cache serves it like any other colouring.
     - A texture layer, which blends palette positions, is refused over
       or under one, and the panel says why.
   - **Rainbow fringe** (C12), from Fraktaler 3's own example
     (`examples/rainbow-fringe.f3.toml`) and its DE vector
     (`hybrid.cc`), read from source.
     - The colour is `hsv(arg DE / 2π, 1/(1 + 4L), 2L)`, decoded to
       linear light and raised to 1/gamma (default 2).
     - `L = |z| ln|z| / |dz|` in output pixels.
     - F3's vector is the conjugate of the screen gradient of |z|²,
       since its saved rows run upward. The view rotation turns it onto
       our screen, so the hue circles the set as in F3.
     - Flat grey where no derivative is iterated (perturbation).
   - **Infinite waves** (C13), from KF2's `KF_InfiniteWaves`, read from
     source.
     - Up to six waves (KF2 allows 30), each `sin(π·iter/P)/2 + ½` on
       hue, saturation or brightness. A negative period is the
       constant `−P/100`. Each channel averages its waves, and a channel
       with none is 0, as in KF2.
     - `iter` is KF2's `n + 1 − NF`, times a scale (1/Iteration
       Division), plus an offset (Color Offset), after the value
       transfer (Color Method).
     - Stepped takes it at whole iterations. Blend mixes half the
       palette back in, a cycle per 1024, as KF2 indexes its palette.
     - The defaults are the three waves KF2's bundled `monochrome-de`
       carries (100 hue, 111 saturation, 123 brightness). None of KF2's
       77 bundled files turns waves on.

     *Measured:* against transcriptions of both sources, from the GPU's
     own records, every drawn pixel lands within one 8-bit level:
     - the fringe at two rotations;
     - the waves with KF2's defaults, and with a stepped, blended set
       holding constants.

     Retuning either recolours from the cache and matches a fresh
     render to the byte. Two visual tests.

   **Found on the way, and fixed** (`5fe6d896`): two features of this
   branch measured in render pixels, which are 1/supersample of an
   output pixel, so antialiasing changed the picture.
   - **Distance estimate in pixel units** (C11): at 2× it moved 77
     levels on average, now 5.8 (plane units: 5.6).
   - **Offset relief** (R6): the shade pass scaled its slope by the
     supersample factor, which is right only for per-render-pixel
     slopes. It was 38% stronger at 2×, and now changes by under 3%.

   The supersample factor now rides in the uniform's flag word, as F3
   divides its DE by its supersampling.

   **Not yet:** itinerary (C8) needs rank equalisation (P7); direct
   orbit traps (C10) need the palette inside the loop.
7. **Relief** (§5.3): the small ones (R1, R2, R4, R8, R9) first, then
   analytic relief (R5), offset orbits (R6) and Embossed (R7). Analytic
   relief at deep zoom waits for the derivative under perturbation (P11).
   *First part done 2026-10-02*, on branch `escape-coloring`:
   - **R1:** a Lambert lighting model beside the signed tilt, with the
     light's elevation (UF's default 30°), measured from what flat
     ground receives so the shadow and highlight scales keep their
     meaning; and an ambient floor under the shadow, for either model.
   - **R2:** Soft Light (the W3C formula) and Hard Light among the
     shadow and highlight blends.
   - **R4:** a height curve, `post · f(pre · h)`: log, square root,
     cube root, square, cube, sine, cosine. Applied as the relief reads
     the height, after any softening, so the contrast probe, which
     reads the same texture, still sees the raw value.
   - **R8:** slope stencils: central (as before), forward, Roberts
     cross, and the least-squares plane over 3×3. KF2's Laplacian is
     left out: it is a curvature, not a slope, and how KF2 lights it is
     not in what this survey read.
   - **R9 is deliberately not done.** §6.7 measured relief at 4× the
     output size moving the picture 0.79× as much, and a linear
     correction like KF2's would overshoot about 3×. A normalisation
     worth having needs a better-founded model than either.

   Every default draws what it drew before (the relief visual tests are
   unchanged). A GPU test checks the options as properties: ambient 1
   with no highlight leaves the picture as it was; a Lambert shadow
   only darkens; Soft and Hard Light with a mid-grey light change
   nothing; every stencil and curve changes the picture.

   **Found on the way, and fixed:** with Banded relief, or any field
   left on Banded, auto contrast fitted the wrapped value, because the
   probe and the relief shared one height texture. The height texture
   is now `rg32float`: red the colouring's raw value, which the probe
   reads, green the relief's own source, which the blur and the slope
   read. Measured: with the relief at zero strength, Smooth and Banded
   now draw byte-identical contrast-fitted pictures.
   - **R3**, in the form that split made cheap: a third relief source,
     **Layer**, slopes the texture layer's value, so the relief follows
     one field and the colour another. Without a layer it is the
     colouring's own value, byte for byte.

   - **R5, analytic relief**, from KF2's source (`gl/kf.frag.glsl`
     `KF_Slopes`, `fraktal_sft.h` `compute_de`): KF2 slopes at
     `1/DE`, DE in pixels, which is the gradient of `−ln d`. For a
     holomorphic map its direction is the potential's, `conj(dz/z)`.
     The iterate pass stores that gradient, in screen pixels and turned
     back by the view rotation, in the height texture's two remaining
     channels (now `rgba32float`), and the relief lights it instead of
     differencing. The derivative orbit compiles in for it. Direct
     path only, since the perturbed rungs carry no derivative (P11); the
     panel says so, and the relief is flat there, as tested. Softness,
     stencils and the height curve do not apply to it.
     *Measured:* lit from one side, analytic and numeric (smooth) relief
     move 97% of the pixels both move the same way, at rotation 0 and
     0.7; the rest is the numeric relief's stencil noise. Near the
     boundary, where the smooth count is chaotic between pixels, the
     agreement drops to 79%, which is the noise analytic slopes avoid.

   - **R6, offset-orbit relief**, a fifth relief source, **Offset
     orbits**. The direct shader's loop moved into a function,
     `esc_run(pixel)`, so it can run more than once a pixel. With offset
     relief it runs twice more, at `c + δ` along the screen's x and y,
     and the slope is the colouring's value there minus its value at
     `c`. Without it, the loop is called once and the visual suite is
     unchanged. δ is a fraction of the view's height (default 1/1024),
     and the slope is measured per 1/1024 of the view, not per render
     pixel, so the relief belongs to the fractal. At a 1024-pixel-tall
     render the two units agree, so a relief height means about the
     same in both fields there. That is R9 for this field: *measured*,
     at twice the output size the share of pixels the relief moves holds
     (×1.007), where the numeric relief's falls (×0.834). Direction:
     with the height scaled so both read the same slope per pixel,
     offset and numeric relief move 97% of the pixels both move the same
     way, at rotation 0 and 0.7. It works on any colouring's value; a
     neighbour orbit the colouring leaves undrawn (interior) counts as
     flat. The cost is three orbits a pixel, and recolouring cannot come
     from the cache while it is on, because the offset orbits need the
     loop. Direct path only: the perturbed rungs do not run offset
     orbits, so the relief is flat there, as the panel says and a test
     checks. Softness, stencils and the height curve do not apply to it.
   - **R7, Embossed**, a sixth relief source, **Embossed contours**,
     ported from Ultra Fractal's own source (Standard.ulb:
     `Standard_Embossed`, `Standard_EmbossedHelper`, `Standard_Emboss`,
     by Kerry Mitchell; read from the public formula reference). Two
     orbits, at `c − dr` and `c + dr` with `dr` toward the light, run
     in step. Each is reduced per Emboss Type: the iteration it
     escaped on, the count of iterates with Re z > 0 or Im z > 0, the
     iteration of the smallest |z|², `trunc(ln|z|)`, or the angle
     sector (1–32 sections, default 2). The pixel is a shadow where the
     orbit away from the light came out lower, a highlight where higher,
     and flat where they tie; those are Emboss's greys 0.2, 0.8 and 0.5.
     The quirks are kept:
     - An orbit that never escapes reads 0 for Iteration, Magnitude and
       Angle.
     - Neither orbit stops early, since UF requires periodicity
       checking to be off.
     - Smallest Magnitude keeps **one** minimum for both orbits, as UF's
       helpers share their owner's `fRMin`.

     The pair is its own loop, `esc_emboss`, built from the same
     generated step text as the pixel's loop (`step_lines`). It
     validates for every formula, damped or not. The step is the
     R6 offset (UF's Contour Size). Ours is a fraction of the view's
     height; UF's is `0.0065/#magn`, and how that compares depends on
     UF's view at magnification 1, which was not read. Two divergences
     are deliberate:
     - `dr` turns with the view rotation, so the light stays where the
       relief's light is. UF's `dr` is fixed in the plane.
     - The emboss is drawn only where the pixel itself has a value. The
       set's interior is the background, which the relief does not
       light.

     UF merges Emboss with Hard or Soft Light. Our black Multiply shadow
     and white Screen highlight, both at 0.6, are exactly Hard Light at
     0.2 and 0.8, with 0.5 its identity.
     *Measured:* against a CPU port of the three classes, the stored
     response agrees on 99.3–99.99% of the drawn pixels for every type,
     at rotation 0 with the light at 135° and at rotation 0.7 with it at
     30°. An unshared minimum agrees on only 11–15%, so the shared one
     is what the shader keeps. One visual test.

   **Found on the way, and fixed:** the direct path's row bands are
   sized by `width × max_iter` per row against a dispatch budget, and
   offset relief (R6) did not count its two extra orbits. A pixel can
   cost three loops, an under-estimate in the direction that risks the
   driver's watchdog; both fields now count three.

   **Held: an image texture (R10).** Image textures are planned for the
   flames as well, so loading an image, storing it with a picture and
   binding it to a pass should be designed once for both engines, not
   escape-first (decision 1 notwithstanding). What KF2 does, read from
   its source (`gl/kf.frag.glsl`: `KF_TextureWarp` and the texture
   block after the palette lookup):
   - The image is sampled in **screen space**, stretched to the frame
     (`tc = (pixel + warp) / ImageSize`).
   - The warp comes from the 3×3 neighbourhood of the iteration value:
     each difference becomes `pow(1 + d, power)`, inverted and
     sign-flipped below 1, then mapped through `(atan(x) − π/4)/(π/4)`
     and scaled by `ratio/100`; the offset is `power/64 ± power·that`.
   - It mixes into the colour, `mix(colour, image, merge)`, **before**
     the slope shading, which then lights the textured colour.
   - It covers the interior too: with a texture on, the interior colour
     is not applied.

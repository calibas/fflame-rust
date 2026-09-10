# Affine flames by distance: 2D fields and 3D raymarching in the escape engine

**Status:** plan of record, 2026-09-10, branch to be cut from `main`
after `simulation-mode` merges. **No code written yet.**

This is the plan for rendering a flame that is an affine IFS — every
transform a linear map, no folds, no nonlinear variations — **by its
distance function rather than by the chaos game.** In 2D that gives an
exterior distance field in one pass, with exact edges and a colouring
vocabulary the flame renderer cannot offer. In 3D the same function is
sphere-traced, which is what the classic high-quality fractal renders
are made of, and what the point-splatting solid mode structurally
cannot match.

It supersedes the escape-time-IFS "Mode C" of
[escape-time-fractals.md](escape-time-fractals.md) §6 for the affine
case — see D9 — and it shares its prerequisites with that plan and
with [flame-deep-zoom.md](flame-deep-zoom.md) §7, which is why phase 0
is built as common analysis.

## 0. What is being asked for

Three things, in the user's words:

1. *"I'm not quite happy with the quality [of solid rendering], it
   looks 'tacky' and that's true for about every 3D flame that uses
   solid rendering. I'm leaning towards raymarching for high-quality
   3D rendering."*
2. *"A big part is that I want to be able to use certain flames that
   match a criteria (linear only?) as 3D escape-time fractals."*
3. *"I'm interested in the 2D flame raymarching, especially if it
   improves rendering quality and/or coloring options."*

And two constraints: it lives in the escape engine, not a new render
mode; and the 3D flame mode's UI is borrowed, not the mode — *"we can
activate it whenever there's a 3D escape-time loaded."*

## 1. What is there now

Verified against the code 2026-09-10. Three of these are absences that
earlier research assumed were present.

**The escape engine has three registry pairs on one pattern** — mode A
`FormulaDef`/`ColoringDef` (26/14), mode B `FieldDef`/`FieldColoringDef`
(3/3, [src/escape/fields.rs](../../src/escape/fields.rs)) — each a
`static` with inline WGSL, spliced into a template by
[src/escape/assembler.rs](../../src/escape/assembler.rs). A field's
contract is `field_step(n, p, state) -> FieldStep` over eight floats of
per-pixel state and `field_color(sum, grad, n_terms)`. The shader
binds group 0 only (params, output, palette, and the perturbation
buffers); **nothing in the escape shader can see the flame's
transforms.**

**The flame's transforms already reach one other engine.**
`ShaderBuilder::build_layer_map`
([src/shader_builder_v2.rs](../../src/shader_builder_v2.rs)) emits
`flame_map(xform, u, seed)` — affine, variations, post-affine — bound
at group 1, for the simulation's layer warps. It emits the FORWARD map
and accepts any variation. Neither is what a distance estimate needs
(D5).

**A 3D flame transform composes to one 3×4 affine.**
[shaders/core/affine_3d.wgsl](../../shaders/core/affine_3d.wgsl): the
XY affine, then optional JWildfire YZ and ZX plane maps, then a
translation summed from three sources. Composed on the CPU that is a
3×3 matrix and a vector. Without the plane maps the 3×3 is
block-diagonal with a unit z scale — **not contractive in z**, which
matters in §2.

**Variations sum.** The dispatcher is `result += weight * f(p)`, so a
transform whose variations are all affine is affine. Today that set is
`linear`, `linear3D`, `zscale` (contributes `weight·z` to z) and
`ztranslate` (a constant). `flatten` is affine but singular.

**`address_mix` exists** ([src/escape/colorings.rs](../../src/escape/colorings.rs)):
a colouring that accumulates a per-iteration branch address as a
binary fraction, built for the origami folds, noting that the address
"is what survives zooming" while smooth quantities lose contrast. The
IFS index map is the base-N version of the same idea.

**What does NOT exist**, each assumed by earlier research:

- **No per-transform contractivity.** `mean_log_scale`
  ([src/script/api.rs](../../src/script/api.rs)) is a whole-flame mean
  of `0.5·ln|det A|`. The determinant is the area factor, so it
  averages the axes and reads a stretch as neutral — the case a
  largest singular value exists to catch. (A 2×2 largest-singular-value
  helper does exist, in `variations/analytic_blur.rs`, sizing kernels.)
- **No affine inverse.** Nothing inverts a transform's `(a,b,c,d,e,f)`,
  let alone the composed 3×4.
- **No notion of which variations are invertible or affine.**
  `VariationDef` carries no such flag.

**The shade pass** ([src/renderer/shade_pass.rs](../../src/renderer/shade_pass.rs))
has a `run_region` entry point taking a dedicated depth buffer and an
albedo image, built for the exporters. It reconstructs normals from
depth and computes occlusion in screen space — the two things that
make solid rendering look the way it does (§2). Lights, materials, fog
and temporal smoothing are separable from that.

**The visibility policy** ([src/ui/visibility.rs](../../src/ui/visibility.rs))
is per-control and exhaustive; `fly_mode_available` is a function of
render mode alone. Both are small changes to make 3D controls follow
the *config* rather than the mode.

**The density-volume experiment was retired** (solid-rendering.md,
queue item 3, 2026-07-16): a 192³ voxel grid splatted from the chaos
game and marched as density "no longer served any feature at
acceptable quality", and sparse grids defeated the march early-out.
That was a density march, not a distance march. It is the prior
result for §7's bridge and the reason that bridge is not in this plan.

## 2. The idea

### 2.1 Why the chaos game looks the way it does, and why this differs

The chaos game samples the attractor's **measure**: points land where
the IFS sends them, weighted by transform weights, and the picture is
density. Solid rendering infers a surface from that sample — nearest
depth per pixel, normals from neighbouring depths — and every symptom
in solid-rendering.md's field notes (pinholes where the measure is
thin, shell ripples on slanted surfaces, single-sample speckle
amplified by lighting, screen-space edge artifacts, temporal shimmer)
is a consequence of inferring a surface from a stochastic sample. The
backlog's fixes are all patches on that.

A distance function samples the **set**. For a point p, d(p) is how
far the nearest point of the attractor is. Edges are where d crosses
zero: exact, antialiasable by the sub-pixel value of d, deterministic,
finished in one evaluation. In 3D, a ray sphere-traces d, and the
surface normal is d's gradient — a property of the function, not a
reconstruction.

**The price is the measure.** A distance render knows nothing of
transform weights, colour speed, or density. Two flames with the same
transforms and different weights render identically. What comes back
in place of density is a different vocabulary (§2.3), and for a sparse,
clean IFS it is a better picture; for a dense, overlapping flame it is
a different picture. This plan does not pretend otherwise, and D6
records it.

### 2.2 The distance estimate

For an IFS of contractive affine maps `Sᵢ(x) = Mᵢx + tᵢ` with attractor
A and a ball B(c, R) with `Sᵢ(B) ⊂ B` for every i, Hart (1996, sphere
tracing, §on linear fractals) gives the bound by **inverse iteration**:
apply inverse maps to the query point, tracking expansion, and scale
the distance in the expanded frame back down.

```
p ← query point (after the inverse of the final transform, if any)
s ← 1
repeat K times:
    i ← the map minimising |Sᵢ⁻¹(p) − c|      (greedy branch choice)
    p ← Sᵢ⁻¹(p)
    s ← s · σ_min(Mᵢ)
    record i                                    (the index map)
    if |p − c| > R·λ⁻ᵏ: break                    (escaped: level k)
d ← s · max(0, |p − c| − R)
```

`d` is a **lower bound** on the true distance: the forward composition
contracts every displacement by at least the product of smallest
singular values, so a distance measured after k inversions is at least
that product times the true one. A lower bound is exactly what sphere
tracing needs — it never overshoots. For similarities (every σ equal)
the bound is tight; for anisotropic maps the ratio σ_max/σ_min per
level is slack, which costs march steps, not correctness.

**Greedy branch choice is the known weakness.** The inverse image
nearest the centre is not always on the branch holding the nearest
attractor point, and overlapping IFSs — the flame norm — are where it
fails. Two mitigations, both in scope: a beam of the best B branches
per level (cost B·N per level), and the escape-level output, which is
robust to the choice because it only asks *whether* the point left the
ball. Phase 2 measures how much the beam buys on real overlapping
flames.

**The escape buffer's output comes free.** The level k at which the
inverse orbit leaves the ball, plus the continuous residual within the
annulus, is Hepting–Hart's E(x) — computed per pixel in one pass, with
no multi-pass feedback. D9 draws the consequence.

### 2.3 What it renders, and how it is coloured

The per-pixel evaluation yields four quantities, and every colouring
is built from them:

| quantity | what it is | colourings |
|---|---|---|
| `d` | distance to the attractor (0 on it) | edge antialiasing; contour bands; glow / halo; the classic DE shading |
| `level` | inverse-orbit depth at escape, with residual | Hepting–Hart escape-time bands: the Fractint "escape-time Sierpiński" look |
| `address` | the branch chosen at each level, packed | symbolic colouring — the analogue of transform colour; `address_mix` generalised to base N |
| `p_K` | the point after K inversions | orbit traps in the expanded frame; image lookups, as the origami colouring does |

In 3D, `d` is traced and its gradient is the normal; the march step
count gives ambient occlusion; a second march toward the light gives
soft shadows; `level` and `address` colour the surface.

### 2.4 The criterion

A flame qualifies when **every** condition holds, and the panel says
which one fails when it does not:

1. Every variation on every transform is in the affine set:
   `linear`, `linear3D`, `zscale`, `ztranslate`. Post-affines are
   affine and compose in.
2. Every transform's composed linear part is **contractive in every
   direction**: largest singular value strictly below one. In 3D this
   excludes the Apophysis-style flame — XY affine plus a z offset — whose
   z scale is exactly one: its "attractor" is a stack of planes, not a
   solid. A 3D solid needs the JWildfire plane affines or a `zscale`
   below one.
3. No xaos. A graph-directed IFS has a distance estimate, but not this
   one; deferred (§7).
4. The final transform, if any, is affine and invertible. Applied as
   its inverse to the query point; its `σ_min` scales `d`.
5. Transform weights are ignored. They select, they do not shape.

Phase 0 reports how many shipped flame presets pass each test. That
number is the honest measure of how much of the catalogue this reaches.

### 2.5 Deep zoom

Yes — and the affine case is *easier* than Mandelbrot's, for a reason
worth understanding before phase 1 chooses its number types.

The wall is the usual one: at zoom 2⁻¹⁰⁰ a pixel's coordinate is the
centre plus an offset f32 cannot hold alongside it. The escape engine
already has the answer's two halves — a centre stored as exact decimal
strings with `zoom_log2` (the deep camera that
[flame-deep-zoom.md](flame-deep-zoom.md) §7 also adopts), and a
reference-orbit machinery that iterates the centre in high precision
and every pixel as a small delta from it.

Here the delta form is **exact, not approximate**. Write the pixel as
`p = C + δ` with C the centre. The inverse maps are affine, so

```
Sᵢ⁻¹(C + δ) = Sᵢ⁻¹(C) + Mᵢ⁻¹ δ
```

with no cross term. The reference inverse orbit `Sᵢ⁻¹(C)` and its
branch choices are computed once per view in high precision; each
pixel's δ follows by matrix products alone, in f32. Mandelbrot
perturbation carries a `δ²` term and the cancellation glitches it
causes; a linear map has neither. There is nothing to detect and
nothing to re-reference.

**The expanding dynamics do the rest.** Every inversion multiplies δ by
at least `1/σ_max`, so a pixel-scale offset reaches O(1) after about
`log(1/zoom) / log(1/σ)` levels — at σ = 0.5 and zoom 2⁻¹⁰⁰⁰, a
thousand levels, each an N-way test and an affine. Once δ is O(1) the
pixel may choose a branch the reference did not; at that point its
expanded point is itself representable, and it simply continues alone
in f32 from that level. That is the "rebase" of deep-zoom escape-time,
made trivial because the frame it rebases into is the expanded one.

So K, the iteration depth of §2.2, grows with zoom depth rather than
being fixed — which is what the escape-level test already does.

**What a deep zoom of an IFS looks like is a different matter, and it
should be said before anyone expects Mandelbrot.** A contractive IFS
attractor is self-similar: for similarity maps a zoom shows the same
figure again, exactly; for anisotropic maps it shows affinely
distorted copies. There is no new structure at depth by definition.
What changes with depth is the **address** — every level adds a digit
— so the address colouring is the one that stays alive under zoom,
exactly as the origami note on `address_mix` observed for folds: the
smooth quantities lose contrast, the symbolic one does not.

**In 3D** the ray origin is the high-precision object and the direction
is f32; sample points along the ray are `origin + t·dir`, the delta
form applies to each, and affine maps take lines to lines. Same
machinery; it is a phase-3 item, not a new one.

## 3. Decisions

- **D1 — In the escape engine, as a fourth registry kind.** Per pixel,
  one dispatch, no feedback: the escape engine's shape. Not a
  `RenderMode`: a mode costs a config version, a wire form, an API
  enum, a visibility policy, a workspace, a menu and ~55 call sites; a
  registry kind costs a definition type, a template and a panel
  section. **`IfsDef`** carries the WGSL that reads the inverse-map
  buffer (D5); `IfsColoringDef` is the colouring vocabulary of §2.3.
  A fourth pair rather than a `FieldDef` because a field has no
  external buffer, no branch address, and no 3D path.
- **D2 — 3D UI follows the config, not the mode.** When the loaded
  escape config is 3D (D8), the camera panel, fly mode and lighting
  controls are shown; `fly_mode_available` gains a config argument;
  the visibility policy gains the cases. Nothing else about the mode
  system changes.
- **D3 — 2D first, then 3D on the same distance function.** 2D needs
  no camera, no march, no shading and answers the quality question
  cheapest. Every part of the estimate (D4), the criterion (§2.4), the
  analysis (phase 0) and the colouring vocabulary carries into 3D
  unchanged; 3D adds ray generation, the march and the surface
  shading.
- **D4 — Greedy inverse iteration with the σ_min product**, a lower
  bound and therefore safe for tracing. A beam width B is a parameter
  (1 = greedy), measured in phase 2 on real overlapping flames before
  its default is chosen. K is a parameter with a preset default; the
  escape-level test bounds it in practice.
- **D5 — A purpose-built inverse-map buffer, not `build_layer_map`.**
  The shader needs per transform: the inverse affine, `σ_min`, `σ_max`,
  and the transform's colour index. All are CPU products of phase 0's
  analysis, packed into one storage buffer at group 1. The forward map
  with variations is the wrong object and its shader is far larger
  than needed.
- **D6 — This renders the set, not the measure.** Said plainly in the
  panel and the docs: weights, colour speed and density do not apply.
  A qualifying flame can be viewed either way, and the two pictures
  differ by design. The colourings of §2.3 are the replacement
  vocabulary, and the index map is the part that carries the flame's
  structure.
- **D7 — Normals and occlusion are the marcher's own.** In 3D the
  gradient of `d` is the normal and the march is the occlusion. The
  shade pass is reused for lights, materials, fog and temporal
  smoothing through an extension that accepts a normal buffer and an
  occlusion buffer when present; its screen-space reconstruction is
  the fallback, not the path. The extension improves solid flames too,
  once anything produces a normal buffer for them.
- **D8 — Projection: a pinhole camera with a field of view, on the
  escape config.** The flame's `zr = 1 − persp·z` is depth scaling,
  not a camera, and generating rays for it would be contorting the
  marcher to match a convention that exists for the chaos game's
  splats. The escape config gains a camera (position, the same three
  angles, FOV) written by the same View-panel controls and driven by
  the same fly mode, so the *interaction* is shared even though the
  projection differs. Recorded as a decision to argue with: the
  alternative is sharing the flame's camera fields outright and
  teaching the shade-pass extension the pinhole, which is less
  duplication and more coupling.
- **D9 — Mode C of the escape plan is superseded for affine IFSs.** The
  escape-level quantity of §2.2 is Hepting–Hart's E(x), per pixel, in
  one pass, from the same loop that gives the distance. The escape
  buffer's remaining advantage — robustness to overlap without a
  branch choice — is what the beam (D4) and the level colouring are
  measured against in phase 2. If they hold, Mode C is closed by this
  plan; if they do not, Mode C's multi-pass form becomes the
  overlapping-IFS path and this plan's estimate the rest. Either way
  the analysis of phase 0 is Mode C's prerequisite list, built.

## 4. What the user will see

- **Escape panel, a new source:** *"From flame"* beside the formula
  and field lists. Choosing it runs the criterion (§2.4) on the current
  flame and either enables the render or names the failing condition
  (*"transform 3 uses `spherical`"*, *"transform 1 is not contractive
  in z (σ = 1.00)"*, *"xaos is not supported"*).
- **In 2D:** the flame's attractor as a shape, at the escape view's
  centre and zoom, with the four colourings of §2.3 and their
  parameters (contour spacing, glow width, address depth, trap shape).
  Edges are exact at any zoom. It renders in one frame.
- **In 3D:** the same, sphere-traced, with the camera panel, fly mode
  and lighting controls active; analytic normals, occlusion, soft
  shadows, fog. The Menger sponge as twenty affine maps looks like a
  Menger sponge, not like a point cloud with lights on it.
- **A qualifying flame can be switched between the two engines** and
  the pictures compared. The chaos game's picture has density; the
  distance picture has edges and addresses. Neither is the other.
- **Presets:** the classical affine IFSs — Sierpiński triangle and
  tetrahedron, Barnsley fern (four affine maps), Koch, Heighway dragon,
  Menger — plus every shipped flame preset that passes the criterion.

## 5. Phases and gates

Each phase is one or a few commits and leaves every existing baseline
byte-identical.

### Phase 0 — the IFS analysis (CPU, shared)

Pure Rust, in a module both this plan and flame-deep-zoom §7 consume.

- Compose a transform to a 2×3 / 3×4 affine, post-affine included,
  from the affine variation set.
- Invert it; compute `σ_min` and `σ_max` (2×2 closed form; 3×3 via
  the eigenvalues of MᵀM). The largest-singular-value contractiveness
  measure the escape plan and the deep-zoom plan both want.
- The criterion (§2.4) as a function returning *why not*, not a bool.
- The bounding ball: a ball fixed by the IFS, grown from the
  translations and contractions (Hart's construction), conservative.
- **Gates:** unit tests against IFSs with known answers — Sierpiński's
  three maps at σ = 0.5, an anisotropic map whose determinant-based
  measure reads neutral but whose σ_max exceeds one, a 3D Apophysis
  flame that must fail condition 2 in z. A script or test that runs the
  criterion over every shipped flame preset and reports the count and
  the reasons; that number goes in this document.

### Phase 1 — 2D distance field

- `IfsDef` / `IfsColoringDef`, the template, the group-1 inverse-map
  buffer (D5), the greedy estimate (D4, B = 1), the four quantities of
  §2.3 with one colouring each.
- The view through the escape engine's deep camera from the start, so
  the pixel is `C + δ` in the shader from day one even while C is
  still within f32. Retrofitting the delta form later would mean
  rewriting the loop; writing it that way first costs nothing (§2.5).
- The escape panel's *From flame* source and the criterion message.
- Presets: the classical affine IFSs above, as flames.
- **Gates:** the Sierpiński distance matches the analytic distance to
  within tracing tolerance on a grid of test points; the classical
  presets render to their textbook pictures, inspected and baselined;
  one qualifying shipped flame preset rendered both ways, overlaid, and
  the edges coincide. Every existing baseline unchanged.

### Phase 2 — colouring, overlap, and depth

- The high-precision reference inverse orbit (§2.5): the centre's
  branch choices and expanded positions computed once per view with
  the escape engine's big-number types, K growing with `zoom_log2`.
  Gate: a Sierpiński zoom to 2⁻²⁰⁰ renders the same triangle, and the
  address colouring changes digit by digit down the zoom.
- The beam (D4, B > 1) and its cost measured; the address colouring
  at a chosen depth and as a mixed fraction; contour, glow and trap
  colourings with their parameters; edge antialiasing from `d`.
- **Gates:** three overlapping flames rendered greedy against beam,
  the difference measured and inspected; the D9 comparison — does the
  level colouring on an overlapping flame hold up against what the
  escape buffer would give — recorded with pictures. This is where
  Mode C's fate is decided, and it is decided by looking.

### Phase 3 — 3D

- The escape camera (D8), ray generation, the View panel and fly mode
  driving it (D2), the visibility cases.
- Sphere tracing the estimate; analytic normals by central
  differences of `d`; occlusion from the march; a shadow march.
- The shade-pass extension (D7): a normal buffer and an occlusion
  buffer as optional inputs; lights, materials, fog and temporal
  smoothing reused.
- Presets: Sierpiński tetrahedron, Menger as twenty maps, the 3D
  flames that pass.
- **Gates:** the Menger render agrees in silhouette with the `menger`
  variation's solid render (same camera, overlaid); the shade-pass
  extension is byte-identical for every existing solid flame when no
  normal buffer is given; a qualifying 3D flame preset rendered both
  ways, side by side, inspected — this is the picture that answers
  §0's first item.

### Phase 4 — record it

- SIMULATION.md's sibling: a topic doc for the escape engine's four
  kinds, or a section in the escape plan — whichever the escape plan's
  size argues for by then.
- The escape plan's Mode C section updated per D9's outcome.
- The Mandelbulb, KIFS and Mandelbox distance functions are **not**
  in this plan: they are `IfsDef`-shaped registry entries that need no
  flame and no phase-0 analysis, and they become catalogue work the
  day phase 3 lands. Named here so nobody plans a second marcher.

## 6. Risks

| risk | consequence | mitigation |
|---|---|---|
| Greedy branch choice fails on overlapping flames | wrong distance, visible tearing where branches cross | the beam (D4); the level colouring, which does not depend on the choice; measured in phase 2 before a default is set |
| Few shipped flames qualify | the feature reaches classical IFSs and little else | phase 0 counts them before phase 1 starts; the affine set can grow (conformal invertible variations — Möbius, spherical — are the next candidates, §7) |
| Anisotropic transforms make the bound loose | slow march in 3D, not wrong pictures | report σ_max/σ_min per transform; cap march steps; the 2D path is unaffected |
| The escape camera duplicates the flame's | two sets of camera fields drift | D8 is a decision to argue with; the View panel writes both through one path |
| Set-not-measure disappoints on a favourite flame | "it doesn't look like my flame" | D6, said in the panel; the index-map colouring is the structural part; the comparison view makes the difference legible rather than surprising |

## 7. Deliberately not here

- **Nonlinear variations.** Conformal invertible ones — Möbius,
  spherical inversion — have a scalar local scale and fit the estimate;
  they are the next step in growing the affine set, not this plan.
  Folds and non-conformal maps are Mode C's territory if Mode C
  survives D9.
- **Xaos.** A graph-directed IFS's estimate restricts the branch
  choice by the graph; the machinery is the same and the choice logic
  is not. After phase 2.
- **Arbitrary flames by voxel SDF.** Splat the chaos game to occupancy,
  3D jump-flood it to a distance field, trace that. The retired
  density-volume experiment is the caution; the difference — a
  distance march has a real early-out — is the reason it is not
  closed. An experiment after phase 3, with that prior result in front
  of it.
- **The measure.** No attempt to bring density into the distance
  render. A hybrid — the distance field as a mask or trap inside the
  chaos game — is a bridge for later.

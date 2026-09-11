# Affine flames by distance: 2D fields and 3D raymarching in the escape engine

**Status:** plan of record, 2026-09-10, on branch `ifs-distance`.
**Phase 0 built 2026-09-10** (§5, with the census result recorded
there). **Phase 1 built 2026-09-10** (§5, with its three
findings); phases 2–4 not started. §8 is the roadmap past affine,
written 2026-09-10 before phase 1 could foreclose it.

This is the plan for rendering a flame that is an affine IFS — every
transform a linear map, no folds, no nonlinear variations — **by its
distance function rather than by the chaos game.** In 2D that gives an
exterior distance field in one pass, with exact edges and a colouring
vocabulary the flame renderer cannot offer. In 3D the same function is
sphere-traced, which is what the classic high-quality fractal renders
are made of, and what the point-splatting solid mode structurally
cannot match.

**On the two names.** The escape plan's **Mode C** is the
Hepting–Hart escape buffer, planned there and not built; this
plan's registry kind is **mode D**, the fourth after formulas and
fields. D9 is about whether the first is still needed once the
second exists, so both appear in the same sentences and they had
better not share a letter.

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
`ztranslate` (a constant). `flatten` is affine but singular. Affine is
the *only* class closed under this sum, which is why phase 0 could
ignore the sum entirely and §8.4 cannot.

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
   `linear`, `linear3D`, `zscale`, `ztranslate`, and **`affine3D`** —
   JWildfire's general 3D affine with per-axis scale, six shears, a
   rotation and a translation. Phase 0 added the last one after the
   census showed it is the only shipped variation that can make a
   transform contractive in z at all. Post-affines are affine and
   compose in.
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

**This exactness is a property of affine maps, not of this plan.**
A nonlinear map has a second-order term and gets Mandelbrot's
problem back in full. §8.5 draws the consequence: deep zoom and a
wider variation set are separate capabilities that do not arrive
together.

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

  **Settled 2026-09-10, and for a reason the original argument did not
  have: DEEP ZOOM.** The flame's camera fields are `f32`, which is the
  same wall mode D's view centre hit — it renders somewhere else past
  2²². An escape camera can carry its position the way the escape
  view already carries its centre, as exact decimal strings with a
  `zoom_log2`, and hand the marcher a high-precision ray origin with an
  f32 direction (§2.5's last paragraph). Sharing the flame's fields
  would cap 3D at a zoom 2D passed this morning, so the duplication is
  not the cost — it is the feature.
- **D9 — Mode C of the escape plan is superseded for affine IFSs.** The
  escape-level quantity of §2.2 is Hepting–Hart's E(x), per pixel, in
  one pass, from the same loop that gives the distance. The escape
  buffer's remaining advantage — robustness to overlap without a
  branch choice — is what the beam (D4) and the level colouring are
  measured against in phase 2. If they hold, Mode C is closed by this
  plan; if they do not, Mode C's multi-pass form becomes the
  overlapping-IFS path and this plan's estimate the rest. Either way
  the analysis of phase 0 is Mode C's prerequisite list, built.

  **Decided 2026-09-10: Mode C is closed for the affine case**, with
  the residual recorded below. See phase 2.

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

**Built 2026-09-10** — [src/scene/ifs_analysis.rs](../../src/scene/ifs_analysis.rs),
nine tests. Two things the tests taught that the plan had not said:

- **A plane *rotation* does not make an Apophysis flame contractive.**
  A 90° YZ rotation on an XY half-scale map sends the unit z scale into
  y, and σ_max is exactly 1. The plane map must scale as well as
  rotate. (The determinant would have called the rotated map
  contractive.)
- **`g` survives the flat path scaled**, because the affine that
  carries it runs before the variation sum that scales z.

**The census** (`how_many_shipped_flames_are_affine_ifss`, prints
rather than asserts, since the number is the deliverable):

| | |
|---|---|
| shipped flames examined | 163 — 13 from the preset library, 150 visual-regression configs |
| qualify as a **planar** affine IFS | **7** — 3 smoke tests for `affine3D` and `zscale`, plus the 4 classical IFSs phase 1 ships as presets |
| 3D with `preserve_z` on (solid candidates) | 34 |
| qualify as a **solid** affine IFS | **0** — every candidate uses a non-affine variation |

The four presets qualify by construction and say nothing about the
catalogue; read against what shipped before phase 1, the number is
**3 of 159**. Not one flame from the preset library qualified. The commonest reasons
are `spherical` (13), `quaternion_linear` (6, a 4D affine whose fourth
coordinate carries across iterations — its 3D shadow is not a 3D IFS,
unless its w-coupling is zero, a refinement not made), `flatten` (5,
affine but singular), `subflame_wf` (5), a non-affine final (4).

So the risk named in §6 is real and now measured: as shipped, this
feature reaches the classical affine IFSs and flames built for it, not
the existing catalogue. That was always the honest expectation — the
flame aesthetic is the nonlinear variation — and it makes the §7
extension to conformal invertible maps (Möbius, spherical inversion)
the thing that would change the number. `spherical` alone is 13 of
159.

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
- The σ product accumulated **in the shader loop**, not folded from
  a precomputed per-level constant — identical arithmetic for affine
  maps, and the one line that would otherwise have to be rewritten to
  reach §8's ladder.
- **Gates:** the Sierpiński distance matches the analytic distance to
  within tracing tolerance on a grid of test points; the classical
  presets render to their textbook pictures, inspected and baselined;
  one qualifying shipped flame preset rendered both ways, overlaid, and
  the edges coincide. Every existing baseline unchanged.

**Built 2026-09-10** — the estimator
([src/scene/ifs_estimate.rs](../../src/scene/ifs_estimate.rs), generic
over dimension so phase 3 inherits it), the registry pair and the four
colourings ([src/escape/ifs.rs](../../src/escape/ifs.rs)), the template
and `assemble_ifs`, and the renderer's group-1 map buffer. Not yet
built: the panel's *From flame* source and its criterion message, and
the presets as shipped assets — the classical IFSs live in the gallery
test for now.

**Five things the pictures taught, none of which the plan had said**
(the third of them is why the beam is in this phase at all)**:**

- **§2.2's pseudocode draws the bounding balls.** Breaking out of the
  loop at the first escape returns `σ·(|q − c| − R)`, which goes to
  **zero at the ball's surface** — so every level's ball is reported as
  part of the attractor and renders as a bright ring around the set.
  Plainly visible on a Sierpiński. Every level's value is a lower
  bound, so the walk now runs on and keeps the LARGEST; the value
  converges (each inversion multiplies the radius by ~`1/σ` and the
  running product by `σ`), so there is a far cutoff rather than a
  budget. Measured on the unit square, whose distance is exact: the
  worst estimate/exact ratio over the exterior grid goes from **0.448
  to 1.0000**. The rings were never a property of the set.

- **Hart's ball is far looser than the estimate needs.** His radius
  bounds a ball each map sends into ITSELF; the walk only needs the
  attractor to be inside one. Since `A ⊆ B` implies `A = ∪Sᵢ(A) ⊆ ∪Sᵢ(B)`,
  the images' bounding ball is another valid ball, and iterating
  contracts it toward the attractor's own. The dragon's went from 1.707
  to about its true circumradius; the unit square's converges to
  exactly 0.7071, the circumscribed circle.

- **Greedy's cost is now measured, on a classical IFS.** Every address
  bounds the distance to ITS piece and the truth is the minimum over
  all of them, so following one address can only read too LARGE — and
  too large renders as "far from the set", which erodes the picture.
  Against exhaustive search over every address at depth 10
  (`greedy_erodes_a_just_touching_ifs_and_exhaustive_does_not`):

  | IFS | pieces meet | greedy loose at | worst ratio |
  |---|---|---|---|
  | Sierpiński gasket | at points | 10 / 576 | 1.27× |
  | Heighway dragon | along a boundary | 288 / 576 | **62×** |

  So the split is not "overlapping flames" versus the rest, as §6 row 1
  frames it — it is how the pieces MEET. Disjoint or point-touching
  IFSs are essentially exact under greedy; a just-touching attractor
  with positive area is not.

  **The beam moved into phase 1 because of this**, D4's schedule
  notwithstanding: phase 1's own gate is that the classical presets
  render to their textbook pictures, and the dragon did not. Widths,
  against exhaustive search at depth 10:

  | beam | loose at | worst ratio | walk, 512² |
  |---|---|---|---|
  | 1 | 288 / 576 | 62.5× | 17 ms |
  | 2 | 33 / 576 | 6.2× | 31 ms |
  | 3 | 1 / 576 | 1.3× | — |
  | 4 | **0 / 576** | **1.00×** | 60 ms |
  | 8 | 0 / 576 | 1.00× | 114 ms |

  Those times are the walk alone. The first version of this table
  reported the whole `render_with` round trip and put beam 8 at "68%
  more than greedy", which was wrong by a lot: a mode-A Mandelbrot
  through the same harness costs 363 ms before any walk happens, so
  the measurement was almost entirely floor. Against a control, beam 8
  is **7×** beam 1. Any timing claim here needs a control; this one
  did not have one.

  Beam 4 already agrees with exhaustive search on that grid, but the
  picture needs 8: of 16 384 points ON the dragon, beam 4 still reads
  141 as off it — a scatter of holes through a solid region — and beam
  8 reads none. Depth does not buy it (24, 40 and 80 all leave the same
  141), so the shipped default is **8**. An IFS with disjoint pieces is
  exact at 1 and pays the 7× for nothing, which is why it is a
  parameter.

  Two things about the ranking, both found by measurement. Candidates
  are ranked by **distance to the ball's centre**, not by their bound:
  the bound is a running maximum, so once a path grazes the ball's edge
  every descendant inherits the same value and the siblings cannot be
  told apart — loose at 568 of 576 gasket points that way against 10.
  Adding the bound as a tiebreak moves no measurement at all, so the
  key is one number, which matters because shifting it is the shader's
  inner loop. And the beam is *ranked* by position but *answered* by
  bound: pruning asks which piece the point is in, the estimate asks
  which surviving address is nearest.

  **The selection is two passes, and that is not a micro-optimisation.**
  Building each child in full and insertion-sorting it into the
  keep-list shifts a fourteen-register candidate up to `beam` times for
  each of `beam × maps` children. Ranking on the key alone and
  rebuilding only the `beam` survivors moves an f32 and a u32 instead,
  and descends `beam` times rather than `beam × maps`.

- **The address and the trap need a distance halo to show the set.**
  Both quantities are defined for every point the walk touches, so a
  colouring that lights the whole plane by them draws the EXTERIOR's
  branch partition — big flat wedges, and the attractor invisible
  inside them. Fading by distance from the set (a reach in pixels) is
  what turns the address into the picture §2.3 promised: the
  Sierpiński's three sub-gaskets and the Koch curve's four
  sub-curves, each its own colour, with exact edges. That picture is
  the one the chaos game cannot make, because it colours by branch
  ADDRESS rather than by which transform happened to fire.

- **A colouring whose default is palette position 0 draws nothing.**
  `ifs_distance` puts the set at a palette position and the exterior at
  the background, and its "interior" parameter defaulted to 0 -- which
  in most palettes, the shipped Fire included, is black. The dragon
  preset rendered a completely empty frame, and read as a broken
  distance function rather than a bad default. It is 0.5 now. The same
  trap sits under any colouring that maps a constant into a palette.

**The classical IFSs, rendered** (the gallery test writes them to
`output/ifs/`): Sierpiński, the four-map filled square, Koch and — with
the beam — the Heighway dragon all render to their textbook pictures
with exact antialiased edges. Four ship as presets (gasket, carpet,
dragon, Koch), framed by the bounding ball the analysis already
computes, so a preset cannot point somewhere the set is not; a test
reads them back out of `assets/presets.fflame`, re-checks the criterion
and the framing, and renders each one to confirm it is not blank.

That last check earned its place immediately. A config serialised with
`serde_json` directly carries no `version`, so loading it runs the
v2 to v3 migration -- which lifts `render_mode` out of the nested
`flame` object and OVERWRITES the top-level `escape` with the flame's
absent default. The presets came back as ordinary 2D flames and drew a
few hundred pixels of chaos game: a plausible picture, of the right
fractal, from the wrong engine. Generating them through
`FractalConfig::to_json`, which stamps the version, is the fix.

**Mode D is the one escape formula that is not a function of the escape
config alone**, and that has a consequence worth stating: every path
driving an `EscapeRenderer` has to hand it the analysed flame, and a
path that forgets draws an empty frame — correctly, quietly, and
indistinguishably from a flame that does not qualify. The app's own
viewport was exactly that until a source-scanning test went in beside
the two call sites. A flame edit also has to mark the escape image
dirty, which nothing else needed to do.

**The panel** gains mode D as a third group in the formula dropdown,
the def's parameters and colourings, and the criterion: it lists
*every* reason a flame fails rather than the first, says plainly that
the render draws the set and not the measure (D6), and offers a Frame
Attractor button.

**What phase 2 still owns**: the high-precision reference orbit and the
deep-zoom path (§2.5), the remaining colouring parameters, and the D9
comparison against Mode C's escape buffer. The beam is no longer among
them, but its cost on an *overlapping* flame — as opposed to a
just-touching one — is still unmeasured, because no shipped flame
overlaps and qualifies.

### Phase 1b — the 1080p hang

Reported straight after phase 1 landed: the presets rendered, but
loading one left the app barely usable, and a pan or zoom usually
froze or killed it with `STATUS_STACK_BUFFER_OVERRUN`.

That exit code names no GPU, and it is not a stack overflow. It is what
Windows reports when the driver resets under a compute dispatch that
overran its watchdog — and the dispatch was the whole image at once.

**The cause was the chunk estimate, not the shader.** The direct path
sizes each dispatch as `budget / (width · per-pixel cost)`, and mode D
declared its per-pixel cost as `levels · maps`. That leaves out the
beam (8× at the default) and, worse, it is stated in mode A's unit: a
walk step is not an iteration, and measured against the shared budget
it is about 250× heavier. The estimate came out so large that
`rows ≥ height` — one dispatch, every pixel, several seconds.

The adaptive breaker that exists for exactly this could not help. It
halves the budget when a band survives but runs slow, or when the
device is lost — and it only arms while a render is BANDED. An
estimate that never bands never arms it, so the first frame is also
the fatal one.

Three fixes, in order of what they were worth:

- **A budget of its own**, `IFS_DISPATCH_BUDGET`, in walk steps
  (`levels · beam · maps` per pixel), calibrated from the measured
  1.5e9 steps/s to a ~300 ms band. The row arithmetic is a pure
  function with tests that assert a 1080p view bands, that no band at
  any supported size and depth exceeds the breaker's 700 ms, that the
  breaker's halvings still reach it, and that a small cheap view is
  NOT chopped up for nothing.
- **The two-pass selection** above, which took beam 8 from 152 ms to
  114 ms of walk.
- **The presets stopped asking for 2× supersampling**, which was four
  times the walk for nothing: mode D's edge is antialiased
  analytically, from the sub-pixel value of the distance, and its other
  colourings are smooth fields that do not alias.

A 1080p render of every shipped preset now finishes in about a second,
in three bands, and there is an end-to-end test that says so — the row
arithmetic can be gated by a unit test, but a driver reset cannot.

**A pass has to hold still while it runs.** Reported next: rotating the
palette made horizontal bands of different colours as the render
scanned down. A band is a complete render of its own rows dispatched in
its own frame, and it samples the palette texture as it goes — so an
edit part-way through leaves the rows already drawn in the old colours.
The band cursor already restarts when its key changes; the palette
simply was not in that key, because it lives in the flame renderer's
texture rather than the escape config. Neither was the flame, which
only mode D reads and which the app re-analyses every frame. Both are
in the key now, via a palette generation counter bumped at the single
point the texture is written.

Restarting is the honest answer, because a band cannot be re-coloured
after the fact: the walk that produced it is gone. Mode A escapes this
through its recolor cache — per-pixel records that re-colour without
re-iterating — and it is why mode A's palette editing was never
affected. Mode D has no such cache, so a palette edit costs it a full
re-render, and during a slider drag only the top band gets drawn. That
is the first thing phase 2 fixes.

**One more hazard, found while looking rather than reported.** Mode D
is the only escape formula that depends on the flame, so the app
re-analyses it every frame and marks the escape image dirty when the
result differs. That comparison was `PartialEq` on packed `f32`s — and
a NaN never equals itself, so a single NaN anywhere in the packed data
(a transform colour is enough) answers "changed" on every frame,
forever. The view would re-render a band per frame and never settle:
a permanent redraw loop that looks exactly like the engine being too
slow, which is the same symptom as the hang and would have survived
its fix. The comparison is by BYTES now, which asks the question
actually being asked — is this the same buffer we already uploaded —
and a test pins it.

### Phase 2 — colouring, overlap, and depth

- The high-precision reference inverse orbit (§2.5): the centre's
  branch choices and expanded positions computed once per view with
  the escape engine's big-number types, K growing with `zoom_log2`.
  Gate: a Sierpiński zoom to 2⁻²⁰⁰ renders the same triangle, and the
  address colouring changes digit by digit down the zoom.

  **The wall is measured, 2026-09-10** — `where_the_f32_walk_stops_agreeing_with_the_reference`
  renders a Sierpiński centred on a deep attractor point at rising
  zoom and compares the classes against the f64 reference:

  | zoom | agreement |
  |---|---|
  | 0 – 2²⁰ | 100% |
  | 2²² | 95.6% |
  | 2²⁴ | 82.4% |
  | 2²⁶ | 55.7% |
  | 2³⁰ and past | ~60%, which is chance |

  Two limits, and they are not the same limit:

  - **Depth, which is a parameter.** At `levels = 24` the walk resolves
    structure down to σ²⁴ only, so past 2²⁶ every pixel reads as
    interior — on BOTH sides, reference included. Scaling depth with
    zoom (one level per bit at σ = ½) restores 100% agreement, and the
    interior/exterior pixel counts then come out *identical* at every
    zoom, which is the self-similarity §2.5 predicts, seen.
  - **The f32 centre, which is the real wall.** `params.center` is a
    `vec2<f32>`, quantised to ~6e-8 near 0.28: at 2²² that is 6% of the
    view, at 2²⁶ the whole view. The picture is not coarse past there,
    it is somewhere else.

    A dyadic centre hides this completely — 0.25 agrees at every zoom
    because there is nothing to round, and the first version of this
    measurement used 0.25 and reported no wall at all. The fixture is
    a point reached by forty maps for that reason.

  `DEEP_ZOOM_LIMIT` records the claim as a constant the test enforces,
  so raising it is what "deep zoom works" means.

  **The design the measurement points at.** The delta half of the split
  is already exact and f32 holds it to zoom ~2¹²⁰ before underflow; the
  reference half is what needs precision, and only until δ grows past
  the reference's own rounding. So the CPU walks the beam from the
  centre in high precision for k₀ levels — where k₀ ≈ log(R/δ₀)/log(1/σ)
  — and hands the shader each surviving candidate's position, its
  accumulated inverse-linear map **scaled by the view** (so it is O(1)
  rather than σ⁻ᵏ), its σ product and its address. That is about nine
  floats per candidate, seventy-two for a beam of eight, which fits in
  the `fdata` block the params already carry: no new buffer.

  One consequence to take with it: the distance has to become
  **pixel-relative** rather than world-absolute, or it underflows f32
  long before the delta does. That also makes the contour bands
  zoom-invariant, which they are not today.

  **The CPU half is built, 2026-09-10** — `seed_beam` and
  `estimate_seeded` in [ifs_estimate.rs](../../src/scene/ifs_estimate.rs).
  The seeding walk is a faithful PREFIX of the walk, not an
  approximation of one: same ranking, same bound, same escape
  recording, stopping only when a delta reaches a quarter of the ball's
  radius. Three gates:

  - a seeded walk answers what a direct one does, over two IFSs, five
    pixels and zooms to 2³⁰ — as far as the direct walk can be trusted
    as a reference, since it forms `C + δ` at full magnitude and f64's
    ulp is 1.5% of the view by 2⁵⁰;
  - the handover level tracks the zoom, one level per bit at σ = ½
    (measured: level 0 at the home view, 56 at 2⁶⁰);
  - **a deep zoom sees the same figure.** Around the fixed point of a
    half-scale map the attractor is exactly invariant under halving,
    so the distance field in PIXELS at zoom Z and at Z+1 must be the
    same field. It is, to **zero difference, at every zoom up to
    2¹⁶⁰** — a deep-zoom gate with no reference to lose precision,
    because it compares the machinery against itself one octave apart.

  **Connected 2026-09-10, and `DEEP_ZOOM_LIMIT` is 40.** The seeds pack
  into the `fdata` block the params already carry — four `vec4`s each,
  fifteen slots for a beam of eight, no new buffer — and the shader
  takes a pixel's *normalised offset* rather than a position, because
  at a deep zoom there is no position an f32 could hold. Measured, the
  same sweep as before:

  | zoom | before | after |
  |---|---|---|
  | 2²⁰ | 100% | 100% |
  | 2²⁴ | 82.4% | 100% |
  | 2³² | chance | 100% |
  | 2⁴⁴ | chance | 100% |

  A million times deeper, and the gasket at 2⁴⁴ renders clean.

  **The reference/delta split ended up entirely on the CPU, and the
  shader got simpler for it.** The handover happens exactly where the
  delta has grown to a quarter of the ball's radius, and at that size
  f32 holds the sum with nothing left to lose — so the shader carries
  one combined point instead of a reference and a delta, and the
  candidate shrank from fourteen registers to twelve. The final
  transform left the shader too: the seeding walk applies it, so by the
  time the shader starts there is nothing to send.

  **The centre went arbitrary-precision the same day, and
  `DEEP_ZOOM_LIMIT` is 200.** The seeding walk is generic over its
  position type (`SeedPoint`): only the POSITION needs more than f64,
  and only until the handover. The maps' coefficients stay f64, the
  basis and σ product stay f64 — they run like 2^±zoom, which an f64
  exponent holds past 2¹⁰²³ — and the distance to the ball is O(1), so
  f64 answers it however precise the point is. What needs precision is
  the centre, and the reason is cancellation: after k levels the walk
  has computed `A_k·C + b_k` with `A_k ~ 2ᵏ` and an O(1) answer, so k
  bits of C are spent getting there. The `BigFloat` impl lives in the
  escape module because `scene` compiles without the escape engine.

  **§2.5's gate is met**: a Sierpiński zoom to 2⁻²⁰⁰ renders the same
  triangle, at 100% class agreement with the high-precision reference,
  and the same at every zoom below it. The ceiling is now the depth
  parameter (256) rather than precision.

  **Three fixtures in a row hid the wall, and each was found only by
  asking what the fixture was rather than reading the result.**

  - The first centre was **0.25** — f32-exact, so it agreed at every
    zoom and reported no wall at all.
  - The first deep gate centred on the **origin** — f64-exact, same
    thing. It still passes to 2¹⁶⁰ and is worth keeping, but what it
    proves is the walk, not the centre.
  - The control written to prove the second one honest asked whether an
    **f64 centre's picture is self-similar**, and it IS: every f64 is a
    dyadic rational, and a dyadic centre's inverse orbit runs out of
    fractional bits and lands exactly on a fixed point. Self-similar
    for a completely degenerate reason.

  So the gate that actually establishes the claim asks the question
  directly, at 2¹²⁰: **more limbs must not change the picture** (it is
  converged — 22 limbs and 44 limbs agree exactly) and **f64 must
  change it** (the precision is load-bearing — f64 differs completely).
  A gate can pass for the wrong reason, and here three did.

  The working fixture is **(5/14, 1/7)**: the fixed point of
  `m₀∘m₁∘m₂`, so the view keeps finding structure however deep it
  goes, and a decimal that repeats forever, so nothing about it is
  exactly representable.
- ~~The beam (D4, B > 1) and its cost measured~~ — **moved into phase
  1**, because the dragon needed it to render at all (§5, phase 1).
- **A mode-D recolor cache — built 2026-09-10, and taken first.**
  The address colouring at a chosen depth, contour, glow and trap
  colourings with their parameters: all of that is unusable to TUNE
  while every slider drag costs a full re-walk, and after phase 1b a
  palette edit restarts the banded pass outright. So the cache came
  before the colourings it exists to make adjustable.

  The walk writes its four quantities into the 32-byte-per-pixel
  records buffer the escape engine already allocates — the same
  stride and the same binding mode A's `IterResult` uses, so the two
  share one buffer — and a mode-D recolor template runs the same
  colouring def over them. Measured on the dragon at 1080p, against a
  mode-A control to remove the harness floor: **433 ms of walk becomes
  nothing measurable.** Gated by rendering each colouring two ways —
  through the cache, and from scratch on a renderer that has never
  seen the view — and requiring the two images to be byte-identical,
  plus a check that the cached path was actually taken, because equal
  images prove nothing if both sides walked. A second test asserts the
  two templates declare the same record, since a field added to one
  alone shifts every field after it into plausible wrong colours with
  no validation error to point at.
- The address colouring as a mixed fraction; the remaining colouring
  parameters; edge antialiasing from `d` beyond what phase 1 has.
- **D9, decided 2026-09-10.**

  The catalogue has no flame that both overlaps and qualifies — phase
  0's census found three qualifying flames and all three are variation
  smoke tests — so the three are constructed: a Sierpiński whose maps
  are 0.6 instead of 0.5 so the pieces lap over each other, two
  half-covers of the square at 0.7 sharing a wide band, and two
  similarities at 0.46 **turned against each other**, so the overlap is
  not axis-aligned and the nearest-centre rule has no symmetry to lean
  on.

  **The escape buffer did not have to be built to be measured
  against.** Its whole advantage is that it needs no branch choice: it
  iterates the IMAGE through the maps, so a point is inside at level k
  exactly when SOME address of length k holds it. That is the maximum
  over addresses — computable directly by exhaustive search, and that
  is the answer the buffer would give.

  How often the walk's level reaches it, against exhaustive search at
  depth 12 over a 21×21 grid:

  | IFS | beam 1 | beam 4 | beam 8 |
  |---|---|---|---|
  | fat gasket | 99.5% | 100% | 100% |
  | overlapping band | 100% | 100% | 100% |
  | turned pair | 98.6%, short by ≤6 | 98.9% | **99.3%, short by ≤2** |

  **The verdict: closed.** The buffer's remaining advantage is worth
  0.7% of points, by at most two levels, on the hardest case that could
  be constructed — and the shortfall has a direction. The walk follows
  real addresses, so any level it reports is one some address explains:
  it can only ever fall SHORT, never over-claim. The artifact is a
  point drawn very slightly further out than it is, which is
  conservative and invisible. That does not buy a second multi-pass
  renderer.

  **A measurement error worth keeping, because it inverted a result.**
  The first version of this compared the *winning* candidate's level —
  the one that minimises the distance — against the buffer's. On that
  quantity a wider beam was sometimes WORSE (the band read 99.1% at
  beam 8 against 100% at beam 4), which is not a property a beam should
  have. It was the comparison, not the beam: the escape buffer computes
  the deepest surviving address, and `level` was answering a different
  question. `Estimate::deepest_level` answers the buffer's question,
  and on it the beam is monotone and the anomaly is gone. The shader
  reports the deepest too, for the same reason.

  **And the pictures** (`render_the_overlapping_ifss_for_d9`, written
  to `output/ifs/`). The level colouring over an overlapping IFS is the
  Hepting–Hart escape-time look the plan promised, and it is the
  prettiest thing mode D draws. The distance colouring is where the
  beam shows: on the turned pair, greedy renders a broken, speckled
  curve missing whole lobes, and beam 8 renders it continuous — 5 304
  lit pixels against 8 473.

### Phase 3 — 3D

**The tracer is built, 2026-09-10** — `ifs_flame_3d`, a second `IfsDef`
with `solid: true`, which selects a second template. The two share the
walk's shape and **all four colourings**: a colouring maps one of the
four quantities to a palette position, and that is the same question in
either dimension.

**The walk needed no new algorithm.** `estimate` was written generic
over `IfsSpace` in phase 1 and `Affine3` already implemented it, so the
3D walk is the 2D one with 3D arithmetic — and it is EXACT: against the
analytic distance to a solid cube (eight half-scale maps whose images
tile it), the worst estimate/exact ratio over 6 584 points is 1.0000.
The 3D twin of the unit-square gate, and the only 3D case with a
closed-form answer.

One thing the walk answers differently, and the marcher is why: a plane
render wants the distance in PIXELS, because that is what an edge and a
halo are measured in and because world units underflow f32 under a deep
zoom. A marcher steps BY the distance, so it has to be a length in the
space the ray is crossing.

**What the pictures show.** A Sierpiński tetrahedron viewed down an
axis presents as a gasket, with sharp edges and no speckle. The Menger
sponge as twenty affine maps looks like a Menger sponge — §4's picture,
rendered. Normals are central differences of `d`, so they are a
property of the field rather than a reconstruction from neighbouring
depths; occlusion is how hard the march had to work, which is free and
also a property of the field. Neither can speckle, because neither is
inferred from a stochastic sample. That is §2.1's argument, seen.

**A layout bug worth recording, because it did not fail.** The first
solid row used a `vec3<f32>` for the translation. A `vec3` aligns to
sixteen bytes in WGSL and four in Rust, so the struct was eighty bytes
on one side and ninety-six on the other, and the shader read every map
but the first from the wrong offset. It did not error: it rendered a
plausible noisy blob that still looked vaguely like something. The row
is four `vec4`s now with no `vec3` anywhere, and a test asserts both
the size and that the shader's own declaration contains no `vec3` —
the half a size assertion cannot see.

**The geometry is checked on the CPU, not read off the render.** Both
shipped solids have their middle cell removed, so the centre of the
unit cube must be a hole; the test asserts it (Menger: 0.236 from the
set). A low-contrast colouring can hide a hole and a wrong map layout
can fake one, so the picture is not the evidence.

**The camera is built, 2026-09-11** (D8). It has the shape a deep zoom
wants: the **target** holds still and carries the precision — three
exact decimal strings, like the 2D centre and for the same reason —
while `zoom_log2` shortens the eye's distance around it and two angles
orbit. An empty target means the attractor's own centre, so a flame
you have just switched to is framed without being told where it is.
`solid_camera` is the one place the angles mean anything, so the
marcher, the panel and anything that later flies it cannot disagree.

The controls follow the **config, not the mode** (D2): escape mode is
not three-dimensional, one formula in it is, so gating on the mode
would show a camera over a Mandelbrot and hide it over the thing it
steers.

**And it does not reach a deep zoom, which is worth stating plainly
rather than implying otherwise.** The target carries digits but the
marcher is handed an ABSOLUTE eye position in f32, and near a target of
0.5 that is quantised to 6e-8 — so past **2¹³** a pixel is smaller than
the eye's own rounding and the ray starts somewhere else. That is the
wall the plane hit before §2.5's reference orbit, moved one step along,
and the answer is the same one: stop sending a position and send an
offset. `a_solid_deep_zoom_is_limited_by_the_eye_not_the_target` pins
the number so raising it is a deliberate act.

2¹³ is lower than a guess would put it, and the reason is worth
keeping: the eye's rounding has to stay under a PIXEL, which is the
view span over a thousand-odd, so it binds about ten bits earlier than
"under the view" would. The first version of that test asserted 2²⁰–2²⁸
and was simply wrong.

**The solids ship as presets, 2026-09-11** — a Sierpiński tetrahedron
and a Menger sponge, beside the four planar ones. They had to be BUILT
rather than found, and phase 0's census is why: none of 34 shipped 3D
candidates qualifies as solid, because an Apophysis-style transform is
a perfectly good planar map and has unit scale in z. Half-weight
`linear3D` is the simplest thing that contracts in all three. They
frame themselves — an empty camera target means the attractor's own
centre — so neither preset has to know where its own solid is.

**The beam's default is 1 for a solid, against 8 for the plane**, and
that is a measurement rather than a nerve. A march pays the beam on
every STEP, not once per pixel, and both shipped solids TILE — the
sponge's twenty sub-cubes are disjoint, the tetrahedron's four meet at
points — so greedy is exact on them for the same reason it is on the
gasket and Koch. Measured at 256²: beam 8 costs 2.8× beam 1 on the
sponge and changes **two bytes** of a quarter-megabyte image. At 1080p
the default took the sponge from **11.1 s to 1.09 s**, byte-identical.
The parameter stays for a solid whose pieces overlap.

**And the chunk model has the march in it now.** It counted one walk
per pixel while a marcher walks once per STEP — the same class of
mistake that hung a 1080p planar view, and it survived the first solid
render because the beam change hid it. `IFS_SOLID_BUDGET` is the planar
budget times the default step count, so the defaults band exactly as
before (three bands, about a second) while raising the march or the
beam shrinks the bands instead of silently lengthening them. The ratio
is a calibration, not a worst case: most rays never take their full
allowance, and modelling the ceiling would band a 1080p view into
one-row dispatches for nothing.

**Still to come in this phase**: soft shadows, the shade-pass extension
(D7), and **3D seeding**, which the camera has now given a shape. A ray
marches THROUGH space, so what a handover carries is per-ray rather
than per-pixel and how far along the ray a sample sits is part of the
offset — and the ray origins are all the same point, the eye, which is
what makes a per-view handover possible at all.


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
- The escape plan's Mode C section updated per D9's outcome — which
  is "closed for the affine case", with the 0.7%/≤2-level residual and
  its direction recorded.
- The Mandelbulb, KIFS and Mandelbox distance functions are **not**
  in this plan: they are `IfsDef`-shaped registry entries that need no
  flame and no phase-0 analysis, and they become catalogue work the
  day phase 3 lands. Named here so nobody plans a second marcher.

## 6. Risks

| risk | consequence | mitigation |
|---|---|---|
| Greedy branch choice fails on overlapping flames | wrong distance, visible tearing where branches cross | the beam (D4); the level colouring, which does not depend on the choice; measured in phase 2 before a default is set |
| Few shipped flames qualify | the feature reaches classical IFSs and little else | phase 0 counts them before phase 1 starts; the affine set can grow, and §8 is the ladder for growing it |
| Anisotropic transforms make the bound loose | slow march in 3D, not wrong pictures | report σ_max/σ_min per transform; cap march steps; the 2D path is unaffected |
| The escape camera duplicates the flame's | two sets of camera fields drift | D8 is a decision to argue with; the View panel writes both through one path |
| Set-not-measure disappoints on a favourite flame | "it doesn't look like my flame" | D6, said in the panel; the index-map colouring is the structural part; the comparison view makes the difference legible rather than surprising |

## 7. Deliberately not here

- **Nonlinear variations.** Not in this plan, but the intended
  direction, and §8 records what they require — including the one
  thing phase 1 must not foreclose. Folds and non-invertible maps
  stay Mode C's territory if Mode C survives D9.
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

## 8. Growing the set: what a reversible variation has to supply

The census in §5 says what was expected: the flame aesthetic *is* the
nonlinear variation, so an affine-only criterion reaches almost none of
the catalogue. The intent is to support a good portion of the
variations eventually. This section records what that takes — written
before phase 1, so phase 1 does not build something that has to be torn
out to get there.

### 8.1 Three things per variation, not one

Invertibility is necessary and not sufficient. The estimate of §2.2
applies inverse maps until the point escapes the ball, then divides the
escape distance by the accumulated contraction. So each variation must
supply:

1. **A closed-form inverse**, as WGSL. Not a proof that a preimage
   exists — a computation of it.
2. **A lower bound on local scale**: σ_min of the Jacobian at a point,
   or over a region. For affine maps this is a constant and phase 0
   precomputes it on the CPU. For anything else it is a function of
   position, and must be accumulated *inside* the shader loop, at the
   orbit points the walk actually visits.
3. **A branch rule** where the map is many-to-one. Choosing a branch
   chooses which preimage the walk follows; §2.2's greedy choice
   generalises, and so does its weakness (§6, row 1).

### 8.2 RNG is not the disqualifier — what the RNG *does* is

`julia` ([defs/advanced.rs](../../src/variations/defs/advanced.rs)) is
√z with a random sign. It is **already an inverse-iteration map**, and
its RNG selects between two preimages, which is exactly what the
distance walk does deliberately. `blur`, in the same file, never reads
`p` at all: it returns a uniform disc point. That is not a function
with a hard inverse, it is a *measure*, and it sits outside the
framework rather than at the far end of it.

The sorting question is whether the RNG **selects a branch** or
**manufactures a point**. Only the second is categorically out — along
with `NeedsAccum` and per-thread-state variations, whose map depends on
iteration history and so is not an IFS map at all.

### 8.3 The ladder

Four classes, each with σ_min in closed form, each more shader work
than the last, each reaching further into the catalogue.

| class | Jacobian | σ_min | examples |
|---|---|---|---|
| **affine** | constant | a constant, CPU-side | `linear`, `zscale`, `affine3D` — phase 0 |
| **conformal** | scalar × rotation | `\|f′(z)\|`, exact rather than bounded | `spherical` (`p/(r²+ε)`, an involution — its own inverse), `mobius`, `julia`, `power`, `exp`/`log` |
| **radial** | `r ↦ f(r)`, θ fixed | `min(f′(r), f(r)/r)`, invertible where f is monotone | the disc family by formula shape — `bubble`, `fisheye`, `hyperbolic` — **not audited, listed as candidates** |
| **general invertible** | full 2×2 | smaller singular value, per point | whatever else has a closed-form inverse |

The conformal rung is the best value for the work: one scalar per
point, exact rather than a bound, and `spherical` alone was 13 of the
159 flames in the census.

### 8.4 The weighted sum is a second gate

A transform's variations are **summed**: `result += weight · f(A p)`. A
weighted sum of invertible maps is not generally invertible, and has no
closed-form inverse when it is. Affine is special here not because it
is easy but because it is the only class closed under weighted sums —
which is why phase 0 never had to notice this.

Past affine the practical rule is **one Normal-phase variation per
transform**, with the pre- and post-affines composing around it: the
inverse of `w · v(A p)` is `A⁻¹(v⁻¹(q/w))`.

So "supporting a good portion of the variations" does not translate
into "supporting a good portion of the flames". The sum structure is an
independent constraint, and what it costs should be measured the way §5
measured the first one — a second census column, not an assumption.

### 8.5 Deep zoom does not come along

§2.5's exactness is an affine property. `S⁻¹(C + δ) = S⁻¹(C) + M⁻¹δ`
has no cross term because the map has no second-order term. A nonlinear
map has curvature, and a perturbed nonlinear IFS inherits the whole
Mandelbrot problem — glitch detection, rebasing, all of it. The escape
engine's perturbation machinery knows how to do that, but it is not
free and it is not automatic.

**The two capabilities decouple.** A build that renders `spherical` is
not thereby a build that deep-zooms `spherical`. Affine stays exactly
deep-zoomable; every rung above it gets the hard version.

### 8.6 Where the data belongs

[`AFFINE_VARIATIONS`](../../src/scene/ifs_analysis.rs) is a `&[&str]`
const. That is right at five entries and wrong at a hundred: it keeps
knowledge about a variation somewhere other than the variation, so
every addition edits a central list instead of one file.

The shape that scales mirrors what the registry already does — a
`VariationDef` carrying its own inverse the way it carries `wgsl_2d`,
`features` and `parameters`: an inverse **kind** (affine / conformal /
radial / general), an inverse WGSL body, a scale WGSL body. All
optional, absent by default, so the other 640-odd definitions do not
move and the append-only registration order is untouched. Then:

- the criterion asks the registry whether a variation has an inverse,
  rather than consulting a list;
- the shader builder splices inverse bodies exactly as it splices
  forward ones, through the per-flame local index map that already
  exists;
- D5's inverse-map buffer stays the affine fast path and gains a
  spliced-shader path beside it, rather than being replaced;
- the census becomes a progress meter instead of a verdict.

None of this is phase 1's work. Phase 1's only obligation is not to
foreclose it, which costs one line (§5, phase 1).

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
A and a ball B(c, R) with `Sᵢ(B) ⊂ B` for every i, the estimate is by
**inverse iteration**: apply inverse maps to the query point, tracking
expansion, and scale the distance in the expanded frame back down. See
§9 for where each half of that comes from — the escape time is old and
well attributed, the distance scaling is ours until someone finds it a
source.

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
| `level` | inverse-orbit depth at escape, with residual | escape-time bands, the Hepting–Hart quantity: nested shells of constant depth around the set |
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
  gradient of `d` is the normal and the occlusion is asked of `d` too.
  (Written as "the march is the occlusion", and built that way first:
  one minus the fraction of the step allowance a ray used. That
  measured almost nothing — a ray reaching a flat face and one reaching
  the floor of a recess both converge in a handful of steps — and it is
  five samples along the normal now. See the phase 3 record.) The
  shade pass is reused for lights, materials, fog and temporal
  smoothing through an extension that accepts a normal buffer and an
  occlusion buffer when present; its screen-space reconstruction is
  the fallback, not the path. The extension improves solid flames too,
  once anything produces a normal buffer for them.

  **Half-taken, 2026-09-11.** The extension is built; mode D is not its
  consumer. By the time it existed the marcher had its own rig, and
  the pass's remaining advantage was that rig alone — forty lines —
  against four full-image buffers, a depth encoding and a second pass
  to reach the same pixels, with no way to express a shadow PER LIGHT
  through one occlusion channel. So mode D reads the same
  `SolidShadingSettings` and lights itself. The vocabulary is shared,
  which was the point; the pixels are not, which was the mechanism.
  See the phase 3 record.
- **D8 — Projection: a pinhole camera with a field of view, on the
  escape config.** The flame's `zr = 1 − persp·z` is depth scaling,
  not a camera, and generating rays for it would be contorting the
  marcher to match a convention that exists for the chaos game's
  splats. The escape config gains a camera (position, the same four
  angles, FOV) written by the same View-panel controls and driven by
  the same fly mode, so the *interaction* is shared even though the
  projection differs. (Built with two angles first; the other two and
  the pan arrived 2026-09-12 — see the phase 3 record.) Recorded as a decision to argue with: the
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
  translations and contractions, conservative. The CONDITION
  `Sᵢ(B) ⊆ B` is Hepting–Hart's (8); the radius that satisfies it,
  `R ≥ |S(c) − c| / (1 − L)`, is the elementary contraction-mapping
  bound and is not attributed to anyone.
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

**Re-run 2026-09-13, after the per-space rule (§8.7):** 165 shipped
flames (the two solid presets joined the library), **12** qualify as
planar — the six presets plus six configs, three of them newly:
`Flatten 3D Smoke`, `Simple 3D Zcone`, `ZCone 3D Smoke`. No shipped
flame fails on `flatten`, `zcone` or `zblur` in the plane any more.
Solid is still 0 of 34. The preset library alone reads **6 of 15**,
and the census now says what each of the other nine needs:
`spherical` (3 presets), `julian` (4), `bubble` (3), `blur`/`pre_blur`/
`noise` (4 — measures, never), `disc`, `blob`, `hemisphere`,
`julia3Dz`. The §8.4 column: 140 of the 153 non-qualifying flames
have one nonlinear variation per transform, but the corpus is mostly
one-variation smoke tests, so that number flatters; the preset list
above is the honest one.

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

- **The fixed ball is far looser than the estimate needs.** Its radius
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
depths; occlusion is asked of the field directly (see the shadow
record below — the first answer, "how hard the march had to work", was
free and measured almost nothing). Neither can speckle, because
neither is inferred from a stochastic sample. That is §2.1's argument,
seen.

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

**Two staleness bugs, reported from the app and both about a pass
being allowed to continue when it should start over.** The camera
sliders did nothing and a resize fixed it; changing the depth stacked
frames of different depths on top of each other.

- **The recolor cache's key had no camera in it.** So a camera change
  HIT the cache and re-coloured the old geometry — the picture could
  not change until something else invalidated the key, and a resize is
  exactly that. The same omission was in the band key. Both carry it
  now. This is the third time a key has been missing an input the
  picture depends on (the palette, the flame, now the camera), and the
  shape is always the same: the input does not live in `EscapeConfig`'s
  view fields, so it was not in the format string.
- **A solid walk wrote a record only where a ray HIT.** Every pixel
  that missed kept the previous view's record, so a re-colour painted
  the last frame's solid into this frame's empty space. Misses write a
  record now, with bit 1 of `escaped` meaning "no surface here", which
  the recolor pass turns back into absence rather than paint.

And a third that only surfaced once the first two were fixed: **a
cached recolour of a solid came back flat.** Lighting cannot be
recomputed from a record — the normal is central differences of the
distance function, which needs the walk — so the shade is kept in the
record. The word it uses held `depth`, which was stored and never
read: no colouring takes it and the recolor pass only copied it back.
A test now requires all THREE templates to agree about the record's
declaration, since the solid walk is the one that writes the shade.

**Soft shadows, 2026-09-11.** A second march, from the surface toward
the light, with the penumbra falling out of it for nothing: a sphere
trace already knows how CLOSE it passed at every step, and that
clearance over the distance travelled IS the angle by which the
blocker missed the light. A shadow map has to be filtered into looking
soft; this is soft because the geometry is. Four parameters — the
light's azimuth and elevation (the cartographer's convention the
hillshade colouring already uses), the shadow's strength, and the
sharpness of its edge. Strength 0 skips the march, which is the whole
cost: measured at +23% on the tetrahedron and +23% on the sponge.

The band model counts it. A shadowed pixel is charged for **two**
marches rather than one, which halves the band. The measured cost is a
fifth more, not twice — but what a band size protects against is the
driver's watchdog, and that is a ceiling question, not an average one.
Phase 1b is what happens when it is answered with an average.

**Three things had to be right before a shadow was visible at all, and
none of them was the shadow.** Each was found by measuring rather than
by looking, and each would have read as "shadows do not work".

1. **The instrument was clipping, and then compressing.** At the
   default exposure a lit surface saturates the 8-bit output, and a
   saturated pixel cannot show a shadow — eight times less light is
   still past 255. Fixing the exposure was not enough: at the default
   **gamma of 4** the eightfold drop from a lit surface to the ambient
   floor lands inside two deciles of output. Gamma 4 is a curve for
   DENSITY, an accumulation being rescued from the dark. A solid
   render is not that; it is already an image, and the shader hands
   the tonemap linear light — it decodes the palette with
   `pow(srgb, 2.2)` precisely so the lighting multiplies in linear. So
   the display curve a solid wants is the matching sRGB encode, and
   the two solid presets ship with **gamma 2.2**. The shadow tests
   measure at gamma 1, so that a threshold is a statement about the
   shadow rather than about the tone curve.

2. **Ambient occlusion was on the wrong factor.** The lighting read
   `0.12 + 0.88·λ·sun·ao`, so the occlusion was darkening the DIRECT
   light — which the shadow march is already answering for — while the
   ambient term, the one the name is about, was unoccluded. Swapped,
   it is `0.12·ao + 0.88·λ·sun`. This is not pedantry about names: with
   the old arrangement a fully shadowed recess and an unshadowed flat
   face both landed on the same constant ambient, so on the sponge the
   sub-squares DISAPPEARED into the face at exactly the moment they
   should have gone darkest. Turning shadows on made the picture
   flatter.

3. **The occlusion itself measured almost nothing.** It was one minus
   the fraction of the march's step allowance a ray used — free, and a
   real property of the field, but not this property: a ray reaching a
   flat face and a ray reaching the floor of a recess both converge in
   a handful of steps, so it read about one everywhere. It became five
   samples along the normal, comparing how far the sample moved with
   how far the surface then is — and then, when that turned out to be
   blind sideways, the twelve-probe hemisphere described below. The
   five-sample version was Quilez's form; the hemisphere is not his and
   is no longer credited to him.

**And the presets were framed down their own symmetry axes.** A
Sierpiński tetrahedron seen down an axis is exactly the planar gasket;
a solid renderer whose whole claim is the third dimension should not
ship a preset that renders a flat emblem. Both solid presets now carry
chosen angles rather than the defaults.

**The gates are two populations, not one difference.** "The picture
changed" is passed by a march that starts on the surface and therefore
reports every point as shadowing itself — a uniform dimmer. So the
test requires BOTH: lit pixels the light still reaches untouched, and
lit pixels it does not, each at least a twentieth of the surface, with
no pixel brighter than before, since a shadow can only take light
away. A second gate holds the penumbra to the sharpness knob: the
partially-lit population must shrink as the edge hardens, which a
uniform darkening cannot fake. Both check the render was not clipping
before they believe anything they measured.

**3D seeding, 2026-09-11 — a solid zoom reaches 2⁸⁰.** Measured against
a CPU reference marching the same rays: agreement was 100% to 2¹² and
fell to 92% by 2²⁰; it is now **100% through 2⁸⁰**, and degrades
gracefully rather than cliffing past that (95% at 2⁹⁶, 85% at 2¹¹²).

The idea is §2.5's, with one structural difference that comes from what
a MARCHER asks. A plane render hands over once: every pixel sits in the
same shrinking view, so a single level serves them all. A ray asks
about a LINE — its samples run from a pixel's width at the target out
to the far side of the bounding sphere, which at a deep zoom is the
whole zoom in span — so no one level is the handover for all of them.
The handover is therefore a **chain**: the beam's state at every level
from the target outward, and each sample takes the deepest link whose
matrix still carries its delta no further than the cap. A sample near
the target takes a deep link and one out at the sphere takes a shallow
one, which is the same statement as "the address prefix containing a
point is shorter the further away the point is". One chain serves the
whole view because every ray starts at the same place.

The CPU half is `seed_chain3`/`estimate_seeded3`, gated by
`a_seeded_3d_walk_holds_a_target_f64_cannot_express`: `S₁∘S₂∘S₃` is a
similarity of ratio ⅛ fixing `(6,5,3)/7`, so `d(T+δ)·8ᵏ` is a constant
and the gate needs no ground truth of its own. It holds to nine digits
at `|δ| = 5e-42`, where the naive f64 walk is out by twenty-six orders
of magnitude — and the naive walk is REQUIRED to fail, because without
that control the constancy would also be passed by a chain that never
left f64.

**Three faults stood between 2¹² and 2⁸⁰, and each was found by
printing a quantity rather than by reasoning about it.**

1. **An epsilon floor of `1e-7` on the march's hit test.** It was a
   guard against an absolute position's own f32 resolution — the right
   scale while the marcher worked in absolute coordinates. Seeded, it
   is not: past about 2²⁰ a pixel is smaller than that floor, so the
   surface was found a fixed distance early and the picture stopped
   sharpening with the zoom. Removing it took agreement from 2¹² to
   2⁴⁸. The same floor sat in the test's own CPU reference at `1e-12`,
   where it made every ray hit at `t = 0` once the whole view was
   smaller than it — the reference had the bug it was there to find.

2. **`eye − target` computed as a subtraction.** The camera carried an
   absolute f64 eye and the offset was derived from it, which is the
   subtraction of two nearly equal numbers — exactly the cancellation
   the seeded path exists to avoid. Past 2⁴⁸ the difference rounded to
   **zero**, so every ray in the frame started at the target itself.
   The camera now carries `eye_rel = −forward·distance` directly,
   products and sums of numbers its own size. This was visible the
   moment the quantity was printed and invisible before.

3. **`length()`, which squares before it adds.** By 2⁶⁴ a delta is
   around `1e-19` and its square is `1e-38`, which is where f32 stops
   having normal numbers — so `length(delta)` returned 0 in the link
   choice, every link then qualified, and the walk started from a
   prefix whose piece the sample was not in. Not a blurred surface: an
   address that is simply WRONG, which reads as holes through the
   interior. The disagreement being mostly INTERIOR rather than at the
   silhouette is what said "structural" rather than "precision", and
   that split is now printed by the test. The link choice compares the
   largest component instead, which is within √3 of the length and
   needs no square. That took 2⁶⁴ to 2⁸⁰.

`what_limits_a_solid_deep_zoom_now_the_eye_is_an_offset` measures all
three ceilings and pins them: the offset resolves a pixel at **every**
zoom (both sides shrink together, so the ratio is a constant — that is
the whole point of carrying an offset), it stays a normal f32 to 2¹²⁷,
and its SQUARE only to 2⁶⁴. The last number is why nothing on this path
may take a length, and it is the same 2⁶⁴ the picture broke at.

**What is packed.** `IfsLinkGpu` is six `vec4`s — reference position,
the accumulated inverse matrix split into a unit-ish part and a binary
exponent applied with `ldexp`, the σ product, the running bound, the
address fraction and the escape state. The exponent split is what lets
a matrix running like 2^level and a delta running like 2^−zoom both be
f32 when only their product is O(1). The reach is stored as a LOGARITHM
for the same reason. The chain is a second storage binding in group 1,
bound for the planar walk too at one empty link — a layout that
differed by dimension would need two pipeline layouts for one shader
family, and the planar template simply never reads it.

**The shade-pass extension, 2026-09-11 (D7, first half).**
`run_region` takes a `ShadeGeometry` now: a normal texture, an optional
occlusion texture, and the half-angle of a pinhole projection. Supplied,
they replace the screen-space reconstruction *and* the chain that builds
it — the normals pass reads a depth field to recover what a marcher
already knows exactly, so running it would be work done to reach a worse
answer. Absent, every existing caller takes the path it always took.

The division of labour is the point, and it is not "the marcher does
lighting" versus "the shade pass does lighting". A distance marcher
knows the gradient of its own field and how enclosed a point is, both
exactly and both as properties of the field rather than of neighbouring
pixels; it can also fold in a shadow it actually traced, which SSAO has
no way to express. What it does not have is the rig: four coloured
lights, a material, fog, and the temporal smoothing that keeps a
progressive render from strobing. Each side keeps what it is better at.

`reconstruct` gained the pinhole because that is the only place a pixel
and a depth become a position — D8's projection difference is confined
to five lines rather than spread through the pass.

**The gate is asserted of the parameter bytes, not of the pixels, and
that is the only honest form it can take.** A solid flame is not
bit-reproducible — the in-batch depth race, recorded in
`solid-rendering.md` — so two renders of the same flame differ whether
or not anything changed, and a pixel comparison could not tell the two
cases apart. What the extension could actually break is the block of
bytes the shader reads, and that IS deterministic: with nothing
supplied the two former padding slots must still read as padding and
the occlusion bit must be clear, and with geometry supplied the same
call must differ in exactly those fields and nowhere else. The six
`solid-*` visual baselines are the end-to-end half.

**The rig, 2026-09-11 (D7, second half — and a divergence from how D7
said to get there).** A solid walk now lights itself from
`SolidShadingSettings`: the Solid Rendering panel's own four lights,
ambient, diffuse, specular, shininess and occlusion strength, plus the
scene's depth fog. Same controls, same panel, same vocabulary — the
panel is simply available in escape mode when the FORMULA is solid,
which is D2 again and is why `visibility::panel` grew a `Solid`
argument rather than another `matches!` on the mode.

**What it does not do is send the pixels through the shade pass, and
that is a deliberate departure from D7 as written.** D7 assumed the
marcher had geometry and needed a rig. By the time it was built the
marcher had a rig too — one white key light, traced shadows, real
occlusion — so the question became what routing the pixels would
actually buy. Measured against what it would cost:

- The shade pass's advantage over the marcher is **entirely** the rig:
  four coloured lights, a material, fog, temporal smoothing. That is
  about forty lines of WGSL.
- Its geometry is the half the marcher already does better and
  exactly — screen-space normals against an analytic gradient, an
  eight-tap SSAO against the distance field itself, splat-resolution
  shadow maps against a traced ray.
- Routing would cost four full-image buffers (~100 MB at 1080p, on an
  engine that already has an OOM scope for exactly this), four texture
  writes a pixel, a depth buffer in the splat encoding, and a second
  full-screen pass — to arrive at the same picture.
- And a single occlusion channel **cannot express a shadow per light**.
  Four lights with one shadow term means light 1's shadow darkens
  light 3. The marcher gets per-light shadows by tracing one each.

So the extension stands (it is built, gated, and the right shape for
the next generator that wants it), and mode D is not its first
consumer. The vocabulary is shared, which was the point; the pixels
are not, which was the mechanism.

**What it costs, measured.** Lights are free and shadows are not:

| | 1 light | 4 lights |
|---|---|---|
| Shadows off | 83.5 ms | 83.3 ms |
| Shadows on | 112.7 ms | 185.4 ms |

Blinn-Phong per light is a handful of arithmetic against a
hundred-walk march, so the count costs nothing on its own. A traced
shadow is one more march per light, +35% for the first and about
25 ms each after — sub-linear because a surface facing away from a
light skips that light's march entirely. The knob is **Shadows**, not
the light count, and the tooltip says so.

**The recolour cache carries the lighting as one scalar, and that is
exact only while the lighting IS a multiplier.** A record is
thirty-two bytes with every one spoken for, so what it holds is what a
new albedo would be multiplied by. No specular (which adds a term
rather than scaling one), white lights (a coloured one is three
different multipliers) and no fog (which mixes toward a colour).
Outside those the cache is REFUSED and a colouring change re-walks —
slower, and correct, which is the way round this has to fail. It is
also why the two solid presets ship with white lights and no
specular: a palette drag would otherwise cost about 590 ms a frame at
1080p against 20, and both are one click away for a still.

**An untouched panel means a default key light, not no light.** For a
flame, `shading_strength = 0` is classic emissive and a real picture;
for a solid it is a flat silhouette. Untouched is the whole struct
being default rather than the strength being zero, and the difference
matters: a user who sets the strength to zero deliberately is asking
for the unlit silhouette, and a rule keyed on the strength alone would
leave no way to ask for it.

**The fourth key.** Lighting had to go into the band and chunk keys, or
a light change would have shown as bands of different lighting
scrolling down the frame. That is the fourth input to arrive without
one, after the palette, the flame and the camera.

**The occlusion was wrong twice, reported 2026-09-11 as "hard shadows
with Shadows at zero, and bright pixels inside the dark lines".** Both
faults were mine and both were found by printing numbers off the
render rather than by looking at it.

1. **The normaliser saturated.** It was `1 − 2·occ/reach`, and the 2
   was a guess. Measured on a Menger crease whose probes read four
   tenths of their height, that expression returns −0.13 and clamps to
   **zero** — so a moderately concave surface came out as *fully
   enclosed*. The normaliser is accumulated now (`Σ h·w`, the reading
   a point would get if every probe landed inside the set), which pins
   open space at exactly 1 and full enclosure at exactly 0 with no
   constant left to be wrong. The same crease reads 0.4.

   Zero occlusion mattered because **occlusion multiplied the direct
   term as well**, so a face that plainly pointed at a light rendered
   pure black. 2 317 pixels of the sponge, all of them in the creases
   where the cubes meet — and since a black pixel next to a lit one
   reads as a hole, that is what "bright pixels inside the dark lines"
   was: the lit pixels were the surface, and the dark ones were the
   bug.

2. **The probe was blind sideways.** It sampled along the NORMAL
   alone, which is not wrong so much as useless for this shape: from
   the floor of a Menger shaft the normal points straight up an open
   shaft, so it read unoccluded — correctly for that one ray. The
   walls are to the side. It is twelve probes over the hemisphere now
   (the normal plus a ring at 55°, two distances each), normalised
   against what an unobstructed HALF-SPACE would return rather than
   against the weights — a flat face is exactly 1 either way, but
   dividing by the weights reads a plane as 0.68 and makes every
   surface in the picture look dirty.

**And a thing that is not a bug, recorded because it looks like one.**
Correct occlusion on a Menger sponge is mild: a shaft is wider than
the reach, so its walls really are open at that scale. The dramatic
black shafts the first renders had came *from* the saturation. A
cavity goes properly dark because the LIGHT cannot get into it, which
is Shadows, not Occlusion — the tooltip says so now, and the default
reach moved from 0.06 to 0.15, which is comparable to the features it
is being asked to shade.

The recolour comparison is no longer byte-identity, and for an
arithmetic reason rather than a staleness one: the fresh walk sums a
term per light while the recolour multiplies by the single factor the
record carries. Both compute the same number and round differently in
the last place, so the gate bounds how many bytes may differ AND by
how much — a stale record is whole regions of the previous view, which
no last-place bound admits.

**Two faults reported 2026-09-11, and neither was what it looked
like.**

**"A slight zoom difference and a whole section goes dark; it happens
sooner at higher supersampling."** The shadow ray's START offset is a
few pixels — the surface's position is only known to about that — while
its HIT threshold was a fixed fraction of the attractor, chosen
deliberately so that light arriving at a glancing angle would not call
every textured face blocked. The two scale differently, and where they
CROSS the function fails completely: a surface close enough to the eye
gets a start offset smaller than the threshold, so every ray is blocked
on its first sample by the surface it started on, and the frame goes
uniformly to its ambient floor. It spreads from the centre outward as
the eye closes in, and supersampling brings it on sooner because that
shrinks the pixel too — which is why the pairing in the report was the
diagnosis. The threshold is capped at a tenth of the offset now, so
where the pixel is large nothing changes and where it is not the
threshold follows the offset down. The gate renders the same approach
at two pixel sizes and asks both that the picture keeps its variation
and that the two sizes agree about its brightness; either alone would
be passed by a fault that darkened both equally.

**"Sections disappear when they are mostly off-screen. Is there
culling?"** There is no culling. It was the handover, and the mechanism
is worth stating because it is the same shape as a bug §2.5 already
warned about in another form: **everything a seed carries is inherited
by every pixel** — the bound as a running MAXIMUM that nothing later
can lower, and the escape as a level nothing later revisits. Both were
scored at the view CENTRE's own position. That is sound only while the
view is small enough for the difference not to matter, and the
handover's whole job is to run until it nearly does.

Pan until the centre leaves the bounding ball and its positive bound is
inherited by pixels INSIDE it. A point sitting exactly on the attractor
then reports the centre's distance to the ball — measured at **7.333
against a true zero** for a centre eight units out — and a point that
reports itself far from the set renders as empty space. The set was not
being culled; it was being told it was somewhere else.

Both walks now score at `r − reach`, the nearest point the view (or, in
3D, the link) can reach, which is a real lower bound on every pixel's
own distance and therefore a sound bound and a sound escape test for
all of them. The continuation raises it again per pixel, which is what
the continuation is for. The gate probes a point ON the set, kept near
the frame's edge but inside it — outside the frame is outside what the
seeding promises — and requires it to read zero at spans from 8 down to
1/16.

**Performance, 2026-09-12: a solid render at 2.8× for the same
pixels.** A review of where the time goes, with the two largest levers
measured and pulled. The sponge at 512² with the shipped rig:
419 ms → 150 ms. The shipped 1080p presets: the tetrahedron
652 → 521 ms, the sponge 1180 → 623 ms *before* the second lever,
which the 512² figure includes.

**Where the time went.** Switching each part off in turn: shadows
29%, occlusion 17%, the primary march and normal 54% — and within the
march, `steps` is not a lever at all (96 → 32 changed 3%; the sphere
trace converges in far fewer) while `levels` is the whole of it. A
walk ran every level it was given, its only early exit being a
candidate a trillion radii away.

**1. The walk stops when the picture cannot see the rest.** Measured
on the CPU: at 1080p home zoom, the sponge's distance is within one
pixel of its level-40 value by **level 6**, at every distance from the
set, against a default of 24. The reason is the bound's shape — it is
a running maximum, and once a candidate has left the ball each further
level can move it by at most about `σ_k·R`. So the walk takes the
precision its caller needs and marks a candidate done when `σ_k·R`
drops below it. The marcher's steps, the normal, the occlusion probes
and the shadow rays each pass their own tolerance; the single
evaluation at the hit point that feeds the colourings passes zero,
because the level and the address are only known when a candidate
escapes and stopping first would report it as interior.

The tolerance that matters is the march's own, and it is a
**hundredth** of a pixel rather than a pixel: a pixel of slack in the
distance is a pixel of slack in where the ray lands, and on structure
that is itself a pixel across — the sponge's pits at 512 — that is a
different face and a different shade. Measured against the full-depth
render: at one pixel 2 882 pixels changed by more than 24 of 765 and
253 flipped between hit and miss; at a hundredth, 18 and 2, which is
f32 noise, for 15% more time. The normal, occlusion and shadow
tolerances are free — with only the march at full depth the picture
was byte-identical. 419 → 245 ms.

A consequence worth more than the speed: **`levels` is a ceiling now,
not a setting.** The walk goes as deep as the pixel needs and no
deeper, so a deep zoom no longer requires raising it by hand, and
raising it costs nothing when the pixel does not ask.

**2. The beam is compiled in.** The walk keeps two arrays of
candidates, a dozen scalars each, and they are registers because that
is what a function-scope `var` is. Sized for the widest beam, they
cost the same at a beam of one — where the walk never touches slot
two — and what they cost is occupancy. `assemble_ifs` takes the
config's beam and substitutes the array bound; the pipeline is keyed
on it. Same bytes out, because nothing in the algorithm changes.
245 → 150 ms on the solid; the planar walk at beam one went from about
16 ms over the harness floor to about 2.

The deep-zoom gate now distinguishes what a precision change can touch
from what it cannot: a handful of silhouette pixels may land on the
other side of a hit against a full-depth reference — measured at one
to five of 9 216 per zoom — and the tolerance INSIDE the surface is
zero, because that is where a wrong link or a squared length shows.

**Where this stops.** The early-exit criterion is measured on
similarity maps (every shipped solid) and stated for them; for an
anisotropic map, `σ_min` understates how fast the bound can still move,
and nothing here has measured by how much. The planar walk is
untouched — it produces all four quantities in one pass for the record
cache, and an early exit would leave the level and address wrong for a
later colouring switch.

**The geometry cache, 2026-09-12: a lighting edit is a relight, not a
walk.** The solid walk no longer shades. It writes two records a pixel
— the four colouring quantities, as before, and sixteen bytes of what
it found at the surface: the normal and raw occlusion as f16, the four
raw per-light shadow terms as unorm8, the hit depth as f32 — and a
**relight pass** owns the picture. That pass is the recolor kernel
with the rig spliced in, and it runs over the whole frame after every
band of a walk and again on any change that is not a geometry input.
It contains no walk, so at 512² it costs about 2 ms over the harness
floor against 150 for the walk it replaces; measured on a warm
renderer, a light-intensity edit is 36 ms and a light-direction edit
163.

**What is geometry and what is not** — the whole design is these two
lists. The walk reads a light's direction and whether it is on (they
decide which shadow rays are traced), whether shadows are traced at
all, the sharpness, and the occlusion reach; those are in both keys.
Intensity, colour, ambient, diffuse, specular, shininess, occlusion
strength, shadow strength, fog, the palette and the colouring are
applied by the relight and are in neither. The gate asserts both
lists: each relight-only input must hit the cache and come out
byte-identical to a fresh walk, and each geometry input must miss,
re-walk, and still match.

Three consequences beyond the speed:

- **The scalar cache is gone and so is its exception.** A record
  carried the lighting as one number, exact only while the lighting
  was a multiplier, so a coloured light, a specular or a fog refused
  the cache. Now the cache holds what the lighting is COMPUTED from,
  and all three are on the exact list.
- **A light edit part-way through a banded pass cannot stripe the
  frame.** Every band relights everything walked so far with the
  lighting of now; there is no band with old lighting baked in.
- **Rows the walk has not reached read as absent**, because the
  geometry buffer is cleared on a pass restart and a zero depth means
  no surface. That is what lets the relight run over the whole frame
  rather than needing a second uniform for a row range — which a
  single params buffer written once per submission could not carry
  anyway.

The rig is one text, `IFS_RIG`, spliced into the walk template and the
relight template at the same marker. That they cannot drift is the
basis of the cache being exact, and the factoring caught a drift on
its first use: the rewritten rig had dropped `ao` from the direct
term, and the before/after comparison showed it as 9% of bytes
differing by up to 69. With it restored, 0.33% differ by exactly one —
the f16 and unorm8 packing, at the last place.

One more key fault surfaced by the gate: the walk's parameter key was
built from whatever the map happened to hold, so a parameter absent
(the default) and the same parameter set explicitly to its default
were different keys, and the first touch of any slider re-walked. It
is built from the def's parameter list with defaults filled in now.

The recolor path's byte-identity test on solids had been relaxed to a
last-place tolerance because the old scalar multiplied where the walk
summed; with both paths ending in the same relight pass, the
comparison is byte-identical again.

**The interaction tier, 2026-09-12: a drag is a quarter-resolution
preview, and the full render lands when it stops.** A mode-D walk at
1080p is hundreds of milliseconds, and a drag that re-walks at every
mouse event is a slideshow. Inside a 250 ms window measured from the
LAST edit, the walk computes one pixel in each 2×2 block and the
relight (or, for the plane, the walk itself) fills the block from it;
when the edits stop, one full render lands. Measured at 384²: solid
102 → 54 ms, planar 57 → 37, each including about 34 ms of harness
floor, so the walks themselves are ~3.4× and ~6× cheaper.

It is a stride, not a resize. The buffers stay full-size and the tail
is untouched; the walk dispatches a quarter of the threads at
`(2x, 2y)`, the band edges land on block boundaries so no block
straddles two bands, and the readers index block-aligned. The stride
is in BOTH render identities, so a preview's records never masquerade
as a full pass's and turning the preview off is a cache miss that
re-walks — which is also why the app has to keep the frame loop
turning until the window lapses: the last preview frame settles, and
nothing else would ask for the full one.

**Shadows and occlusion stay on during the preview, deliberately.**
They were the obvious extra ~1.75×, but they change what the picture
IS, and the full render landing would pop — a hole going dark a
quarter-second after you let go of the slider. A resolution change
only sharpens. If the extra speed is wanted it is one condition in
the walk, and it should be a choice rather than a default.

The relight lights each of the block's four pixels with its OWN ray,
so a solid preview is blockier rather than blocky: the surface is
sampled at half rate, the shading is not.

**Beam default and budget model, 2026-09-12.** See §10 items 4 and 5
for the measurement. 1080p presets across the day's work: gasket
437 → 250 ms, carpet 401 → 284, tetrahedron 652 → 173, sponge
1180 → 301; the dragon and the Koch keep beam 8 and their times.

**The handover stops where the view stops agreeing, 2026-09-12.**
Reported from use: sections disappear when zooming in or out, the
structure warps rather than magnifying, regions in 2D seem to swap
which is drawn above the other, and Beam Width changes it while no
setting fixes it. The suspicion was the handover, and the measurement
confirmed it. The CPU walks the beam from the view CENTRE and every
pixel continued from the centre's surviving branches; a pixel whose
own nearest piece had been pruned from that beam read a distance to
some other piece and rendered as exterior. Which pixels that hit
depended on how deep the handover went, which depended on the zoom.
Measured against each pixel's own unseeded walk at the same world
point (exact at these zooms, and it knows nothing of any centre),
with disagreement counted past a pixel or one percent: the gasket at
z8 disagreed on 15% of the frame, the dragon at z8 beam 2 on **41%**,
the dragon at z14 beam 1 on 6%, and 0% in the cells between. That is
the zoom dependence the report describes, and the beam dependence.

The rule: a branch may be pruned only if every pixel in the view
would prune it too. The ranking key is a distance to the ball centre
in the inverse-iterated frame, and a pixel's own key for a candidate
is within the candidate's REACH of the centre's (the view basis
pushed through the candidate's inverse maps in 2D; the chain cap in
3D, since a sample sits within the cap of its link's reference by
construction). So the view agrees on the beam exactly when the most
optimistic key of the best pruned candidate is still worse than the
most pessimistic key of the worst kept one, and the first level at
which that fails is not taken. After: 0% in every cell of both
tables — five zooms, four beams, the gasket and the overlapping
dragon; five zooms, three beams, the tetrahedron and the sponge.

Two things it costs, both accepted. The handover ends earlier than
the reach alone would have taken it — 53 levels for 60 bits of zoom
on a generic point of the gasket, where it was 60 — and the shader
walks the difference in f32, eight bits of its twenty-four spent on
the frame before the pixel; the deep-zoom gates (2D to 2²⁰⁰ against
the reference, solids to 2⁸⁰) are unchanged. And on a set whose
branches TIE, the chain ends at the tie: the tetrahedron and the
sponge tile, so at beam 2 or 4 their two nearest branches are
equidistant from every sample and the chain is one or two links —
the unseeded f32 walk, and the 2¹³ wall again. A beam of one is exact
for a tiling set and is the solid default, so nothing shipped pays;
an overlapping solid wanting both a wide beam and a deep zoom has no
one asking for it yet. The same tie stops a 2D handover at a centre
the set is exactly symmetric about — the test target that was the
ball centre pushed through thirty maps came back to the ball centre
after thirty levels and stalled there — which a typed centre of
exactly zero on a symmetric set could reach; from the tie down the
shader is on its own in f32, which is the wall of §2.5 measured from
that level rather than from the top. Not measured on a real view,
because no preset centres on one; the exact answer for that case is
below.

What was tried first and is not the answer: carrying the ambiguous
branches too — a closure wider than the beam, up to eight seeds or
eight slots a link, ranked once per pixel at the handover by the
pixel's own position. It restored the 3D depth at beam 2 and it is
not the same walk: greedy selection is level by level, and a lineage
that ranks best at the handover level need not be the one greedy
would have followed from the level it was pruned at, so the seeded
distance differed from the pixel's own — on the gasket by a pixel or
two along a halo edge, on the dragon at z8 beam 2 by half — and a
seeded walk that differs from the per-pixel one is, by construction,
one whose picture depends on the centre, which is the artefact.
Replaying the selection per level per pixel from the carried tree
would be exact and about eight times the CPU cost at depth; it is the
answer to the tie case if anyone hits it, and the wide-closure
packing (empty-slot flags, insertion ranking on the GPU) is kept
because it is what that replay would hand over.

**The solid camera has all four angles, and a pan, 2026-09-12.**
Reported from use, three at once: Pitch and Yaw but no Bank; panning
changed `center_re`/`center_im`, which a solid does not read, so the
drag did nothing; and the view's Rotation did nothing either. The
3D flame's camera was offered as the reference — it pans in the
rotated frame and has all four angles.

The frame is the flame's chain now: `Rz(roll)·Rx(pitch)·Ry(bank)·
Rz(−yaw)`, the same factorisation as `build_camera_matrix` in
`utilities.wgsl` and `CameraMatrix::build` in fly mode, with `cam_bank`
in the bank slot and the view's `rotation` in the roll slot — the
outermost factor, so it turns the screen and leaves the other three
alone, which is what "rotates the viewport independently" asks for.
Bank sits between pitch and yaw as it does on a flame, and does there
what it does here (`the_solid_chain_is_the_flames_camera_matrix`
transcribes the WGSL and compares). Two conventions are the escape
engine's rather than the flame's, and both are deliberate. The
solid's pitch is measured from the horizon, the flame's from looking
straight down, and every shipped preset and saved solid carries angles
in the solid's terms — so pitch and yaw are re-expressed on the way
into the chain, and with bank and rotation at zero the frame is
EXACTLY the old one (`the_frame_is_what_it_was_with_the_new_angles_at_zero`);
the presets did not move. And the flame draws its y axis DOWN the
screen, Apophysis's way, so its frame is the mirror of a physical
camera's, while the plane draws Im up and the solid always looked the
physical way, right = forward × up — the chain's screen-x row is
negated for that, and the roll is applied as `Rz(−rotation)` so that
a positive rotation turns the solid's picture the way it turns the
plane's (clockwise on screen; measured on the rendered sponge). The
pitch clamp short of the poles is gone with the cross product that
needed it: the chain has an up at every angle
(`the_frame_is_a_frame_at_every_angle_including_the_poles`).

A pan slides the target across the screen plane at the target's own
depth: `dx · right − dy · up` pixels, each a step of `2·tan(fov/2)·
distance / height` in world units, so the surface under the cursor
follows the cursor. The step is a mantissa and a power of two added
to the decimal target in fixed point — the plane's
`escape_pan_delta_symbolic` again — because a solid's zoom shrinks
the distance without limit and an f64 step underflows past ~2¹⁰⁶⁰
while the target's digits do not; gated at 2¹²⁰⁰. A zoom towards the
cursor moves the target by the offset's change of scale, so the point
of the target's plane under the cursor stays under it. Right and up
are the camera's rolled ones, so the pan follows the rotation. An
empty target — the attractor's own centre — becomes explicit the
moment the camera moves. A drag re-runs the IFS analysis to get the
ball and the camera: measured at 0.26 ms per event on the sponge's
twenty maps, in release.

Not done, and known: fly mode still does not drive this camera (D2
asks for it; the chain being the fly camera's makes it a matter of
wiring), and the field of view's doc string said "horizontal" when
`ifs_ray` spreads it vertically — corrected in passing.

**Phase 3 is done.** A ray
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
const (a per-space role function since §8.7, still a central match).
That is right at a dozen entries and wrong at a hundred: it keeps
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

### 8.7 The first rung, taken 2026-09-13: affine per space

Asked from use: how does the list of five grow, and why is `flatten`
rejected in the plane when the plane never sees z?

The list was a name allowlist, and a name is the wrong unit: what a
variation contributes depends on the SPACE being analysed. `flatten`
is post-phase `z ← 0`; in 2D its body returns its input, and the
chaos game in 2D never had a z to zero. Same for `zcone` and `zblur`
(2D bodies return zero, summed — nothing) and for the four
`pre/post_rotate_x/y` (2D bodies return their input). So
[`AFFINE_VARIATIONS`](../../src/scene/ifs_analysis.rs) became
[`affine_role`](../../src/scene/ifs_analysis.rs): per name and per
space, *nothing*, a *summed* affine, a *pre-phase* affine or a
*post-phase* affine. The composition mirrors the shader's phase order
— affine, pre-variations replacing the point, the sum, post-variations
replacing the result, post-affine — and its ORDER: pre/post
variations compose in the flame's first-occurrence order
(`resolve_phase_buckets` walks `active_variation_names_ordered`), not
the transform's, which matters the moment two rotations about
different axes meet (`two_pre_rotations_compose_in_the_flames_order`).

What that adds, exactly and deep-zoomably:

- **Plane:** `flatten`, `zcone`, `zblur`, `pre/post_rotate_x/y` are
  nothing and accepted. Three shipped configs newly qualify (§5's
  re-run).
- **Solid:** `pre/post_rotate_x/y` are rotations by the weight in
  radians about the named axis — affine isometries, composed in their
  phase, no singular value changed. `flatten` is affine and
  **singular** there, and the criterion now says *singular* rather
  than *non-affine*.

What it does not add: any nonlinear variation. The next rungs, from
the census: the `julia` family (single-valued inverse `z ↦ zⁿ`, no
branch rule — the forward map's RNG picks a branch the inverse never
needs — with the escape-time DE in place of the σ-product; 4 presets),
`spherical` (an involution; 3 presets), `bubble` (3). The `VariationDef`-
carries-its-inverse shape of §8.6 is the vehicle for those; growing
`affine_role` further is not.

### 8.8 The second rung: the julia family (plan, 2026-09-13)

The first nonlinear maps. `julia` is `julian` with power 2 and
distance 1; `julian` is `q = |z|^{d/|n|}·e^{i(θ + 2πk)/n}`, `k` drawn
at random from `[0, |n|)`. What follows is the plan as it will be
built, with its decisions numbered so the record can say which ones
held.

**J1 — The inverse is one map, and the RNG is not in it.** Every
branch `k` of the forward map is undone by the same
`z = |q|^{|n|/d}·e^{i·n·arg q}`. So `q ∈ T(A)` iff
`A_pre⁻¹(P(A_post⁻¹(q)/w)) ∈ A` with `P` that single map: inverse
iteration needs no branch rule for a root, and the walk's shape —
one child per map — is unchanged. (`juliascope`'s mirror branches
have TWO preimages, `±n·arg q`; not this rung.)

**J2 — What the walk draws is the FILLED set.** A root map expands
near the origin, so it is not a contraction and its chaos-game set
is a repeller — the Julia set, a curve or a dust — not a Hutchinson
attractor. The walk reports distance 0 for a point it never pushes
out of the ball, which for a root map is the filled set: the same
picture mode A draws for `z² + c`, with the DE halo outside it. The
flame draws the boundary. The two agree exactly when the set is
totally disconnected, which is the usual flame case (a `julian` at
weight w with a translation is a dust whenever the pre-affine's
image misses the critical value's basin); for a connected set the
distance render fills what the flame outlines. The panel says so,
the way D6 says "the set, not the measure".

**J3 — The scale is the chain rule at the orbit point, and the bound
is an estimate.** Per level, `σ` multiplies by the forward map's
local σ_min at the point the walk is at: `σ_min(A_post)·w·σ_min(A_pre)·
(min(d,1)/|n|)·|v|^{1 − |n|/d}`, `v` the point before the root's
inverse. That is the product-of-norms form the affine walk already
uses, made local. It is not a proven lower bound past affine —
Koebe's ¼ is the slack for a univalent map, and near a critical
point (`v → 0`, where the root's derivative is infinite) the
estimate is large where the set is not far. The running maximum
over levels is kept: past escape the estimate DECAYS by a factor
`|n|` per level (`r ↦ rⁿ` against a derivative `n·rⁿ⁻¹`), so the
maximum lands a level or two after the escape, which is where a
Green-function DE would put it up to that factor. Measured, not
argued: gate 1 below.

**J4 — One root per transform, alone in the normal phase.** A root
summed with an affine (`0.5·julian + 0.5·linear`) has no closed-form
inverse. The pre-affine, pre-phase affines, post-phase affines and
the post-affine compose around it; any summed variation beside it
is `NotAffine::MixedSum`. `n = 0` or `d = 0` is `NotAffine::
Degenerate`. The final transform stays affine.

**J5 — The ball is found numerically.** No fixed-point formula and
no global σ_max. The centre is the mean of a short CPU chaos game
(the forward maps with a random branch); the radius starts at that
sample's extent and grows until every map sends the sampled disc
(boundary circle and interior rings) into the disc, then takes a
5% margin. A radius that does not settle in sixty rounds is
`Disqualification::NoBall`. For an affine IFS the existing
closed-form ball is used unchanged.

**J6 — No deep zoom.** §8.5: the reference/delta split is affine. A
nonlinear IFS hands over at level 0 with the pixel's absolute f32
position, and the zoom wall is the one the plane had before §2.5
(~2¹⁶–2²⁰, resolution-dependent). The seeding code takes its affine
path or stops; nothing in it is generalised.

**J7 — On the GPU the map row grows to sixty-four bytes** — the
post-inverse (with `1/w` folded in), the pre-inverse, the kind, `n`,
`d`, the constant part of σ — and `ifs_inv_point` becomes a kernel
switch on the kind. Kind 0 is the affine row and its arithmetic is
exactly today's, so every affine preset renders byte-identically:
gate 3. The kernels are a fixed switch in the walk template for this
rung — two kinds — and move to the per-`VariationDef` splice of §8.6
when there are enough of them to be worth a splice.

**J8 — On the CPU the 2D map is an enum**, `Map2::{Affine, Root}`,
and the walk's step is `(point, local σ_min) = map.step(q)`. The
affine arm returns what it returns today. `Ifs2` is `Ifs<Map2, ..>`;
the 3D types do not move (J9).

**J9 — Not this rung:** `julia3D`, `julia3Dz` (solids), `juliascope`
(two preimages), any second nonlinear kind. Each is a kind and a
kernel once J7 exists.

**Gates:**

1. *The math:* one `julia` transform with pre-translation `−c` IS the
   inverse-iteration system of `z² + c`. On a grid of exterior points
   the walk's distance is within a factor of 4 of the classic
   escape-time DE `|z|·ln|z| / |z′|` at depth, and membership (never
   escapes within L levels) agrees with the escape-time test at the
   same L, on a connected `c` (the rabbit) and a dust `c`.
2. *The transcription:* the GPU walk agrees with the CPU estimate on
   a two-transform julia flame, the way it does on the gasket.
3. *Nothing moved:* the four planar affine presets render
   byte-identically before and after the row change.
4. *Shipped:* at least one julia preset, rendered and inspected, with
   J2's caveat visible in the picture and stated in the panel.
5. *The census*, re-run, with the presets that now qualify named.

**Built 2026-09-13. Every decision held; the gates read:**

1. `a_julia_walk_agrees_with_the_classic_distance_estimate`: on the
   rabbit, a dust and the basilica, 96² points each, membership
   agrees on every point (late escapes on either side of the ball's
   radius versus the classic bailout excluded, eight levels each way),
   and the walk's distance over the classic estimate reads
   **min 0.36, median 0.40–0.54, max 0.61** — below it throughout, by
   a bounded factor, which is J3's mechanism exactly: no `ln|z|`, and
   the maximum over levels landing a level or two past the escape.
   Gated at [0.25, 1.0], what was measured with room for a different
   `c` and none for a different mechanism.
2. `the_gpu_walk_agrees_with_the_cpu_reference_on_a_julia_pair`:
   5510 interior and 2967 exterior pixels of 96², agreement above 97%
   by the same test the gasket passes.
3. The six shipped affine presets (four planar, two solid) at 512²,
   **byte-identical** before and after the row change, compared as
   files.
4. Three presets ship: **Douady Rabbit** (`ifs_level` — the filled
   set with escape bands, J2 in plain sight), **Julia Dendrite**
   (`ifs_distance`, `c = 0.36 + 0.1i`, thin enough that the picture
   is its outline) and **Cubic Pair** (two `julian` roots of power 3
   under `ifs_address`, a set no single Julia set is). Chosen from
   seven candidates rendered side by side
   (`render_the_julia_candidates_for_inspection`). The panel says
   "N maps, M of them julia roots" and states J2 under it.
5. Census: **15 of 168** planar (the three new presets); the preset
   library reads **9 of 18**. The nine that still fail need, with
   the roots now taken: `bubble` (Grand Julian, JuliaN Bubble,
   Bubbles), `disc` (Julian Disc), `blob` (Flower), `spherical`
   (Plastic, Spherical3, Cup), `hemisphere` (Cup), `julia3Dz`
   (Bubbles), and the measures. So `bubble` and `spherical` are the
   rungs that would move the library next, three presets each.

What it cost: the map row is sixty-four bytes instead of thirty-two
(J7), one `power == 0` branch per inverse step on an affine row, and
`pow`, `atan2` and `sincos` per step on a root row. Not measured as
time; the byte-identity gate says the affine arithmetic did not move.

What is recorded and not fixed: an `Ifs2` is `Ifs<Map2, ..>` and the
3D types are untouched (J9), so the solid formula still rejects
`julia3D`; seeding hands over at level 0 for a root IFS (J6) and a
deep zoom into a julia set stops where f32 does — the panel's note
says so; the walk's `last_sigma` for the level residual is a root
row's constant part, which is a coarser residual than an affine
row's, visible as slightly uneven band widths under `ifs_level` and
not worth a second word in the row.

### 8.9 The third rung: `spherical` and `bubble` (plan, 2026-09-13)

The two variations the census names next, three presets each, taken
together because they share the machinery the julia rung built and
differ from it in one thing each: `spherical` has no rigorous ball,
`bubble` has two preimages. Bodies, with `w` the weight and the
affines composing around as in J4: `spherical(p) = w·p/(|p|² + 10⁻⁶)`,
`bubble(p) = w·4p/(|p|² + 4)`.

**S1 — The kernel is a slot in the row, and a transform may fill
more than one.** `RootMap2` becomes `NonlinearMap2` with a `Kernel`
(`Root{n, d}`, `Spherical`, `Bubble`) and a `branch`. One map of the
IFS is one (transform, branch): a `bubble` transform contributes TWO
maps that share its colour and differ in the branch, so the walk's
loops, the beam and the address (base = number of maps) do not
change shape. The address colouring's base grows with the branches;
the first digit still names the transform.

**S2 — `spherical` is an involution, and the ε is dropped from the
inverse.** With `v = post⁻¹(q)/w`, the inverse is `v/|v|²`; the
flame's `10⁻⁶` keeps its chaos game off a division by zero and makes
the forward map 2-to-1 inside a radius of 10⁻³ of the pre-origin,
whose image is a radius of ~500w — outside any ball the render uses,
so the second preimage is never inside the ball and never a branch.
The local scale is exact: the map is conformal with `|f′| = w/|p|²`,
so the factor on the constant part is `|v|²`.

**S3 — `spherical` has no invariant ball, and gets a measured one.**
A set built from inversions is unbounded through the pre-origin:
whatever ball is drawn, some point of it maps outside. The ball for
an IFS with a spherical map is the bulk of a chaos-game sample — its
99.5th percentile radius about the sample mean, with a 30% margin —
and is NOT a proof: the sparse tail beyond it is drawn as exterior,
and the distance near the tail is not a bound. The forward map used
for the sample keeps the ε, as the flame does, so the sample is the
flame's. Roots and bubbles keep the invariance search of J5, which
bubble makes trivial: its image is the unit disc.

**S4 — `bubble` folds, and its two preimages are both followed.**
`|v| = 4|p|/(|p|² + 4)` is at most 1, reached at `|p| = 2`, and every
`|v| < 1` has an inner preimage `|p| = (2 − 2√(1−|v|²))/|v|` and an
outer one with `+`. Both are branches (S1). A `v` outside the unit
disc has no preimage: that branch's child is placed at infinity,
which the walk already treats as an escape of that piece. The local
scale is the map's TANGENTIAL derivative, `|v|/|p|`, and not its
smallest singular value: the radial derivative vanishes on the fold
circle, and a lower bound honest about that reads zero on every
preimage of the fold — a dark ring per transform per level, drawn
where the set is not. The tangential choice is an estimate that
reads too LARGE across the fold, which erodes the halo there and
nothing else: membership is the escape test and does not depend on
σ at all.

**S5 — Not this rung:** the 3D bodies (`spherical` in 3D is the same
map on xy with z through, so it is not a contraction in z; `bubble`
in 3D writes z); `disc`, `blob`, `hemisphere`; the measures.

**Gates:**

1. *Soundness, measured:* for a spherical flame and a bubble flame,
   on a grid, the walk's distance never exceeds the distance to a
   dense chaos-game sample of the set by more than the sample's own
   spacing — the module's `estimate_never_exceeds_a_sampled_upper_bound`,
   applied to maps with no closed form. This is what catches an
   over-read: S3's tail and S4's fold, both of which are predicted to
   show only in the halo.
2. *Round trips:* each kernel's inverse undoes each of its branches.
3. *The transcription:* GPU agrees with CPU on a spherical flame and
   on a bubble flame.
4. *Nothing moved:* the nine shipped presets byte-identical across
   the row change.
5. *Catalogue:* the shipped flames the census names (`Spherical3`,
   `JuliaN Bubble`) rendered as flames and as distance fields, side
   by side, inspected; whichever of them qualifies is the first
   catalogue flame this feature reaches.
6. *The census*, re-run.

**Built 2026-09-13. The kernels are right and the pictures are
discs.** The gates, then what they showed:

1. `nonlinear_walks_never_exceed_a_sampled_upper_bound`, a 40×40 grid
   over each set's ball against a 200 000-point chaos-game sample.
   **Bubble: 34 of 1600 over, 2 of 316 in the inner half, worst
   22×** — and before one more mechanism was found, 824. A point
   outside a bubble's image disc has no preimage, and reporting that
   piece as *infinitely* far was the 824: the piece is not far, it is
   just not reachable by inversion. Its distance is at least
   `(|v| − 1)·w·σ_min(post)`, the **image gap**, which the walk now
   records for the piece without following a child, and the answer
   is the minimum over all pieces, entered or not (S4, amended). The
   34 that remain sit on the images of the fold circle, where S4's
   tangential scale reads too large, as predicted; gated at the
   measurement. **Spherical: 946 of 1600, 24 of 316 in the inner
   half**, and S3's "shows only in the halo" was wrong in degree: a
   set point whose address passes through the tail leaves the
   measured ball, so membership itself is wrong wherever the set is
   built through the pre-origin — 7.6% of the inner half. Recorded,
   not gated; the render of an inversion IFS is the escape-time set
   relative to a measured ball, and its halo is not a distance.
2. `every_kernel_inverse_undoes_each_of_its_branches`: round trips
   on both bubble branches, the inversion (to 4·10⁻⁶ — the flame's
   ε, dropped from the inverse, S2) and a negative-distance root.
3. `the_gpu_walk_agrees_with_the_cpu_reference_on_spherical_and_bubble`:
   above 97% by the gasket's test, on both.
4. The nine shipped presets byte-identical across the row change
   (eighty bytes now, a kind and a branch and a vec4 of kernel
   parameters).
5. `render_the_kernel_candidates_for_inspection`, six candidates —
   two inversion pairs, three tangent circles with and without a
   seed, two bubble pairs, and the catalogue's **JuliaN Bubble**,
   which qualifies now (its `julian` distances are −1, a root of the
   inverted radius, handled like an inversion's ball) — each as a
   distance field beside itself as a flame. **Every distance field
   is a filled disc.** The never-escaping set of an IFS with an
   inversion or a bubble is a whole region: inversion swaps the
   inside and outside of a circle, bubble folds the plane onto a
   disc, and points bounce inside the union of those discs without
   leaving.
6. Census: **17 of 168** planar; the preset library **10 of 18**,
   JuliaN Bubble the first catalogue flame reached.
   `Spherical3` stays out for a different reason: two of its
   transforms are pure translations, σ_max = 1, and a set that
   contains its own translate is unbounded.

**The disc was the distance, not the picture — corrected the same
day.** The paragraph that stood here concluded from gate 5 that
there was nothing worth shipping and that the ladder should stop.
It was drawn from ONE colouring per candidate, which is the mistake
the reader should take from this section: a rung was nearly
abandoned on a single sample of a four-way choice, and the user's
"I suspect that sheet doesn't give us the whole story" is what
caught it.

The distance IS a disc, and that part of the measurement stands:
the set is a region and the walk reports every interior point as
being on it. But the walk produces FOUR quantities (§2.3) and three
of them are defined everywhere inside a region — which branch was
taken (the address), how deep the walk went (the level), and where
the inverse orbit ended (the trap). They carry the structure the
distance cannot:

- **Three tangent circles under the orbit trap** draw an Apollonian
  circle packing — the limit set of the Schottky group those
  inversions generate, which is what an inversion IFS is. With a
  contracting seed transform beside them it is the classical
  gasket, discs within discs to the resolution.
- **JuliaN Bubble under the branch address** draws a spiral
  partition with nested copies of itself at two of its arms; the
  catalogue flame the census names, and a picture in its own right
  rather than a diagram of one.
- The inversion PAIR stays noisy under every colouring, which is
  S3's measured ball showing as it should: its set is unbounded
  through the pre-origin and the ball is a percentile.

`render_every_colouring_of_the_kernel_candidates` is the gate that
should have existed: four colourings × two parameter settings × six
candidates, one sheet. And the obvious objection is answered —
`does_the_interior_structure_depend_on_the_walks_depth` compares
24, 48 and 96 levels and finds **0.02% then 0.00%** of pixels
changing, so the structure is the set's and not the parameter's.

What this rung settles about the ladder, restated: past affine, the
question is still what the picture IS, but "the distance field" is
not the answer for every set. An IFS whose maps fold or invert has
a region for its never-escaping set, and its structure lives in the
address and the trap. That reframes the variations that would move
the library next (`disc`, `blob`, `hemisphere`): they fold the
plane too, so their distance will be a disc too — and that is no
longer a reason not to take them.

### 8.10 The fourth rung: `disc`, `blob`, `hemisphere` (plan, 2026-09-13)

The three the census names after §8.9, taken on §8.9's corrected
footing: their distance will be a region and their picture is in the
address and the trap. Bodies, `θ = atan2(x, y)` (Apophysis's angle
from the +y axis) and `r = |p|`:

- `hemisphere(p) = p / √(r² + 1)` — onto the open unit disc, 1-to-1.
- `disc(p) = (θ/π) · (sin πr, cos πr)` — polar coordinates read the
  other way: the output's RADIUS is the input's angle over π, its
  angle (from +y) is `πr`. Onto the unit disc, and periodic in `r`
  with period 2, so every image point has a ring of preimages.
- `blob(p) = r · s(θ) · (cos θ, sin θ)`, `s(θ) = low + (high − low)/2 ·
  (sin(waves·θ) + 1)` — a reflection in the diagonal, since
  `(cos θ, sin θ)` is the swap of `(sin θ, cos θ)`, times a radial
  scale that depends on the angle. Onto the plane when `s > 0`.

**D1 — hemisphere is a diffeomorphism onto the disc, with an honest
σ_min.** Inverse `p = v / √(1 − |v|²)`; image gap `|v| − 1` as
bubble's. Singular values `(1−|v|²)^{3/2}` radially and
`(1−|v|²)^{1/2}` tangentially, both exact, no fold: the smaller is
taken. It goes to zero at the rim because the far plane compresses
there, and that reads as a distance too SMALL near the rim, which is
the conservative side.

**D2 — disc has a ring of preimages, and the ball says how many.**
With `ρ = |v|` and `φ` the angle of `v` from +y, the preimages are
`r = φ/π + m` for every integer `m` with `r ≥ 0`, at `θ = +πρ` for
even `m` and `−πρ` for odd. Rings beyond the ball's pre-image cannot
hold a set point, so the branch count is `⌊r_max⌋ + 2` with
`r_max = |pre(c)| + σ_max(pre)·R`, capped at twelve — which makes the
ball a prerequisite of the branch expansion, and the analysis now
finds the ball on the unexpanded maps first. A branch whose `r`
comes out negative has no preimage and its child lands at infinity;
that is not an image gap (other branches serve) and contributes
nothing. Image gap `|v| − 1` for the unit disc. Singular values are
exact and orthogonal: `π|v|` along the input's radius and `1/(πr)`
along its angle; the smaller is taken, and it vanishes at the
output origin (the +y axis collapses there), the conservative side
again.

**D3 — blob is a reflection times an angular radial scale, and the
scale must stay positive.** Inverse `p = swap(v) / s(θ)`, `θ` the
angle of `v` from +x. With `low ≤ 0` or `high ≤ 0` the scale can
vanish or change sign and the preimage count changes with the
angle; that is `Degenerate`. Jacobian in the (radial, tangential)
frame is `[[s, s′], [0, s]]`, whose smaller singular value is closed
form; `s′ = (high − low)/2 · waves · cos(waves·θ)`. Onto the plane,
no gap. The defaults (`high = low = 1`) make it the pure reflection,
an isometry.

**D4 — Nothing else changes shape.** Three more kinds in the row
(4, 5, 6), blob's three parameters in the row's vec4, and `branch`
carrying disc's `m`. The julia and spherical/bubble rows are
untouched and the nine presets stay byte-identical.

**Gates:** as §8.9's, with §8.9's lesson: (1) round trips per kernel
and per branch; (2) the sampled upper bound, measured on all three
and gated where the ball is rigorous (all three are); (3) GPU agrees
with CPU on each; (4) byte identity; (5) every colouring of every
candidate on one sheet, and the depth check, BEFORE any conclusion
about the picture; (6) the census — Julian Disc, Flower and Cup are
the presets this reaches.

**Built 2026-09-13. One thin set among the three, and it ships.**

1. `the_fold_kernels_undo_each_of_their_branches`: every point comes
   back on exactly one of a disc's twelve rings and on the one
   branch of the others, and each local σ_min is below the forward
   stretch by finite differences. A blob with `low = 0` is
   `Degenerate`, and the message now says what that means rather
   than "power or distance".
2. `nonlinear_walks_never_exceed_a_sampled_upper_bound`, the same
   grid and sample as §8.9: **hemisphere 0 of 1600, disc 0 of 1600**
   — rigorous balls and honest σ_min, and it shows. **Blob 22 of
   1600, 3 inner, worst 1.56×**: small, and not explained — the
   Jacobian's singular values check against finite differences and
   the ball is invariant, so the product-of-parts bound should hold;
   pinned at thirty and left as a question. Bubble re-measured at
   27 / 9 inner / 44× after the ball search changed (below); the
   count moves with the ball because the fold band does, the regime
   does not.
3. GPU agrees with CPU on all five kernels by the gasket's test.
4. The nine presets byte-identical — after a detour. The ball
   search's plain fixed-point iteration `radius = reach` climbs to
   its limit from below and need not satisfy `reach ≤ radius`
   exactly: a blob IFS was still creeping at 0.51 after sixty
   rounds and reported `NoBall`. An overshoot of 5% past the reach
   settles it in a round — and moved the three julia presets' balls,
   which the byte gate caught (three of nine differed). It now
   overshoots only past sixty plain rounds, so the balls the julia
   presets were framed on are found exactly as before. The search
   also restarts a lost chaos-game point off the origin, because
   `disc` sends the origin to itself and a root of a negative
   distance sends it to infinity; Julian Disc is exactly those two
   maps and had no ball at all.
5. Every colouring of every candidate, then the depth check, before
   a word about the picture (§8.9's lesson). **Blob Flower** is the
   find: the one thin set among the fold kernels, whose distance
   field IS the picture — contour bands radiating from a dark set —
   and 0.42% then 0.27% of its pixels move across 24 → 48 → 96
   levels. It ships, under `ifs_distance` with contours on and a
   dark interior. **Julian Disc** (a `disc` and an inverted-radius
   `julian` of power 50) draws a spoked disc under the address
   colouring, depth-converged at 0.00%; reached, not shipped. The
   hemisphere pair, the disc spiral and **Cup** are discs, and where
   their trap colourings showed structure it moved **29% and 37%**
   of pixels with the depth — the walk's, not the set's, and the
   depth check is what keeps that from being called a picture. Cup's
   measured ball is radius 110 on a set whose bulk is a unit or two:
   S3's percentile with a heavy tail, recorded.
6. Census: **20 of 169** planar; the preset library **13 of 19** —
   Julian Disc, Cup and JuliaN Bubble reached, Blob Flower added.
   Flower stays out on its own terms: its blob has `low = 0` and its
   second transform is a rotation at scale 2. The rest are measures
   (`blur`, `noise`, `pre_blur`), `julia3Dz`, and Spherical3's
   translations.

What the three rungs together say about the ladder now: the
question was never "invertible", and it is not "region or not"
either. Roots draw filled Julia sets; inversions draw regions whose
structure is in the address and the trap; a blob with a positive
scale draws a thin set with an honest distance field. Each kernel
had to be rendered under every colouring and checked against the
depth before anyone could say which, and that — not the inverse — is
the cost of the next one.

### 8.11 Towards quaternions: the road, and its first step (plan, 2026-09-13)

The eventual goal is 3D quaternion sets. Three steps, each a thing
on its own:

1. **A quaternion Julia solid that needs no flame** — this step.
   `q ↦ qⁿ + c` in ℍ on a 3D slice, rendered by the escape-time
   distance estimate of Hart, Sandin and Kauffman (1989), which is
   the paper sphere tracing comes from (§9). It is an `IfsDef` with
   `needs_flame: false`, the shape phase 4's note reserved for the
   Mandelbulb and its kin: its WGSL supplies `ifs_walk3` and the
   solid template supplies everything else — the camera, the march,
   the normals, the shadows, the occlusion, the relight cache, the
   four colourings. The chaos game already has this set as
   `quaternion_julia_set`, and the def takes its conventions: `c` as
   `(cx, cy, cz, cw)` with the scalar last, the power, the bailout,
   a slice axis and a slice value.
2. **3D kernels on the flame side** — `Map3::{Affine, Nonlinear}` as
   §8.8–8.10 built for the plane, starting with `julia3D` (a root
   whose radius is the 3D radius with `z` scaled by `1/|n|`, whose
   elevation is kept and whose azimuth is divided), so that a flame
   of several 3D root transforms is a solid the way a julia pair is
   a plane.
3. **The quaternion IFS** — several `quaternion_julia` transforms
   with the scalar part carried, which is where step 2's kernels
   and step 1's arithmetic meet, and where the 4D caveat that
   variation's own docs record (the inverse root collapses onto a
   plane) has to be faced rather than inherited.

**Q1 — The distance is the classic estimate, and it is an
estimate.** With `dq` the scalar `|∂q_k/∂q_0|`, which under
`q ↦ qⁿ + c` multiplies by `n·|q|ⁿ⁻¹` each step because the
quaternion norm is multiplicative, `d = ½·|q|·ln|q| / dq` once `|q|`
is large. The half is Hart's, for the march's sake. The iteration
runs past the bailout to `|q| > 10⁴` or the depth, whichever first,
so the estimate is taken where the logarithm has settled; membership
is `|q|` never exceeding the bailout within the depth.

**Q2 — The four quantities.** Distance as above; level the smooth
escape count, rising toward the set as the walk's does; address the
escaped quaternion's azimuth in `[0, 1)`, which is the binary
decomposition; the trap point its `xy`. So every colouring works
unchanged.

**Q3 — The wiring is the flame-less packing.** `pack_standalone`
makes a `PackedIfs` with no maps and a ball of the bailout's radius
at the origin, so `set_ifs`, the camera, the globals and the keys
all work as they do; the map count in the globals is at least one
so the template's "no qualifying flame" guard does not fire; the
handover chain is skipped for a map-less IFS. The walk works in
`delta + target_offset`, f32 and absolute: no deep zoom in this
step, as §8.5 says of anything nonlinear.

**Gates:** (1) every mode-D combination compiles, which the new def
joins by being in `IFS_DEFS`; (2) *the math:* with `c = 0` the set
is the unit ball, so its rendered silhouette is a disc whose pixel
radius the camera predicts, gated within a pixel; (3) a preset —
Bourke's `c = (−1, 0.2, 0, 0)` on the `k` slice — rendered and
inspected before it ships; (4) the preset gate learns that a
flame-less preset has no criterion to pass.

**Step 1 built 2026-09-13.** `quaternion_julia_solid` is in
`IFS_DEFS`, every colouring compiles over it, and the wiring is what
Q3 said: `pack_for` chooses the flame's analysis or the def's own
`pack_standalone`; the globals report at least one map; a map-less
IFS gets no chain. Two WGSL rules learned on the way — `smooth` is a
reserved word and `let _ = x;` is not a statement — cost one compile
gate each.

The unit-ball gate measured **57.44 px against a predicted 55.47**,
and the two-pixel excess is the estimate doing exactly what Q1 says:
with `c = 0` the orbit is `|q₀|^(2ᵏ)` and Hart's half makes
`d = ½·|q₀|·ln|q₀|` at every depth — half the true distance — so a
march that stops at one pixel of estimated distance stops at two of
true distance, and the silhouette is two pixels wide. Gated at
`[−0.5, +2.5]` with that written beside it; taking the half out would
land the ball exactly and cost the march its safety on a real set.

Bourke's constant on the `k` slice, `c = (−1, 0.2, 0, 0)`, renders
as the ringed lobes everyone knows, lit and shadowed, under the
level, distance and address colourings alike — the `i` slice, a
dendrite-like `c` and a fully general one all render
(`render_the_quaternion_julia_for_inspection`) — and ships as
**Quaternion Julia**, framed at `zoom_log2 = 0.7` because the ball
is the bailout's radius and the set is half of it. Relight, preview
stride, the geometry cache and the four-angle camera all work
unchanged, having been written against `ifs_walk3` and not the
flame. Not in this step: deep zoom (the walk is `delta +
target_offset` in f32), and CPU agreement — there is no CPU twin of
this walk, and the unit ball is the check that stands in for one.

## 9. Where this comes from

Written after the fact, because the first version of §2.2 cited a
section of a paper that does not exist. The rule in CLAUDE.md about
not inventing attributions applies to papers as much as to variation
authors, and a plausible-sounding citation is worse than none — it
looks checked.

**Verified, by reading the papers:**

- **Escape time for a linear fractal, by inverse maps.** Prusinkiewicz
  and Sandness, *Koch curves as attractors and repellers*, IEEE
  Computer Graphics and Applications 8(6):26–40, 1988. Hepting and
  Hart's Definition 4.2 is explicitly "based on the one given by
  Prusinkiewicz and Sandness", and reads
  `DE(x) = 1 + maxᵢ DE(Tᵢ⁻¹(x))` inside the disk and 0 outside — "the
  maximum number of inverse transformations necessary to iterate x to
  a point outside D_R". That MAX over the maps is what this plan calls
  `deepest_level`, and the disk condition `T(D_R) ⊂ D_R` is our
  bounding ball.
- **The N-ary tree with pruning**, which is what the walk without a
  beam is: Hepting, Prusinkiewicz and Saupe, *Rendering methods for
  iterated function systems*, in Fractals in the Fundamental and
  Applied Sciences, 1991 — cited for exactly that by Hepting and Hart.
- **The escape buffer**, the FORWARD algorithm this plan repeatedly
  contrasts itself with: Hepting and Hart, *The Escape Buffer:
  Efficient Computation of Escape Time for Linear Fractals*, Graphics
  Interface '95, pp. 204–214. Its own §7.1 names the gap this plan
  fills: "If a similar forward algorithm can be constructed around
  DISTANCE instead of escape time, the result would greatly increase
  the efficiency of computing the distance transform of linear
  fractals."
- **Sphere tracing**, which is what the solid marcher does, step for
  step: Hart, *Sphere tracing: a geometric method for the antialiased
  ray tracing of implicit surfaces*, The Visual Computer
  12(10):527–545, 1996. Including the stopping rule — the paper works
  in terms of "the radius of a pixel", which is our `px_at * t`.

**Cited but NOT read, so nothing is claimed about its contents:** Hart
and DeFanti, *Efficient antialiased rendering of 3-D linear fractals*,
Computer Graphics 25(3):91–100, 1991. It is the obvious place for a
distance bound on a linear fractal to live and it is where Hart 1996
gets its pixel radius, but this plan does not assert what is in it.

**Not attributed, because no source was found:** scaling the escape
excess by the product of the maps' minimum singular values to turn an
escape time into a DISTANCE — §2.2's `σ·(r − R)` and its running
maximum over levels. It is the natural Lipschitz analogue of the
escape time above and it may well be in the 1991 paper; until someone
checks, it is unattributed rather than mis-attributed.

**Also borrowed, and verified:**

- **Soft shadows by the narrowest passage.** Inigo Quilez,
  *Soft shadows in raymarched SDFs*,
  <https://iquilezles.org/articles/rmshadows/> — the
  `shade = min(shade, k·d/t)` accumulation `ifs_shadow` runs. His `k`
  is "related to the inverse of the light source's size, since larger
  lights create softer shadows", which is what our **Shadow Sharpness**
  is and what its tooltip now says.
- **The attractor as the fixed point of `A = ∪ Sᵢ(A)`**, which the ball
  refinement is one line of: Hutchinson, *Fractals and self-similarity*,
  Indiana University Journal of Mathematics 30(5):713–747, 1981.
- **Blinn–Phong**, the per-light shading mode D shares with the splat
  pipeline's shade pass: Phong 1975, Blinn 1977. Naming it is the
  credit; the arithmetic is deliberately the shade pass's, so the two
  engines describe light the same way.
- **The Barnsley fern**, one of phase 0's fixtures, is from Barnsley's
  *Fractals Everywhere* (1988), which is also where the IFS vocabulary
  this plan uses comes from.

**Named only to put them out of scope** (§7): the Mandelbulb, Mandelbox
and KIFS distance functions. Those are community constructions — the
Mandelbulb is generally credited to Daniel White and Paul Nylander, the
Mandelbox to Tom Lowe, KIFS to the demoscene handle "Knighty" — and
this plan neither uses nor reproduces them, so the names are here to
say what this is NOT.

**Ours, and said so rather than left ambiguous:** the twelve-probe
hemisphere occlusion (Quilez's along-the-normal form is what it
replaced, for being unable to see a well); the beam, its
distance-to-centre ranking and the two-pass selection; the seed chain
and its per-level handover; and the reference/delta split applied to
inverse maps.

**Unchecked, and flagged rather than asserted.** An earlier version of
§2.3's table called the level colouring "the Fractint 'escape-time
Sierpiński' look". Fractint does ship an IFS type and escape-time
rendering of linear fractals was in the literature by then, but nobody
here has opened Fractint to confirm it produces that picture, so the
claim is gone rather than dressed up.

**What was wrong.** §2.2 read "Hart (1996, sphere tracing, §on linear
fractals)". That paper has no such section — its sections are
Introduction, Sphere Tracing, Antialiasing, Results, Conclusion — and
it contains no occurrence of "IFS", "iterated function", "attractor"
or "contractive"; its Appendix F on fractals is about noise and
hypertexture. The sphere-tracing half of the attribution was right and
the linear-fractal half was invented. `Hart's inverse iteration`
appeared in three source files as well and is now named for what it
is.

## 10. Performance backlog

From the 2026-09-12 review, ranked by measured or estimated gain
against effort. Items are struck through here when they land, with
the commit.

1. ~~**Geometry cache and relight pass.**~~ Landed 2026-09-12; see the
   phase 3 record. A light-intensity edit is 36 ms against 163 for a
   walk at 512², and coloured lights, specular and fog are on the
   exact list.
2. ~~**Interaction tier.**~~ Landed 2026-09-12 as a stride, not a
   resize; see the phase 3 record. Shadows and occlusion stay on
   during the preview so the full render does not pop; switching them
   off would be one more condition in the walk, worth ~1.75×, and is
   left as a choice rather than a default.
3. **Shadow-only re-march.** With item 1 in place, a light DIRECTION
   change still re-walks everything; a third kernel that re-marches
   only the shadows from cached hit and normal would make that cost
   ~25% per light instead. Needs the walk spliced into a banded
   relight-like pass.
4. ~~**Planar beam default.**~~ Measured 2026-09-12 on the three D9
   overlap fixtures, the dragon and the gasket at 384²: **nearly all
   of the beam's cost is the last doubling** (8 → 4 is 60–77 ms → ~41)
   and 4 matches 8 exactly on three of five sets, 16 pixels short on
   the dragon and 47 on the turned pair. Below 4 the dragon loses
   thousands, and even the gasket loses 178 at beam 1 where its pieces
   touch. Default is 4. The dragon and the Koch presets are pinned at
   8 because a preset is a still and their pieces meet (the Koch
   moved 358 pixels of 305 000 at 1080p at the default). Auto-detection
   from disjoint image balls was not pursued: the gasket's pieces have
   overlapping bounding circles and are exact at 4, so the proxy
   would say the wrong thing about the commonest case.
5. ~~**Chunk budget model.**~~ Counts every walk a pixel can cost now:
   the primary march, one march per shadowed light, twelve occlusion
   probes, six for the normal.
6. **Deeper 3D zoom.** The remaining wall is f32's exponent on the
   delta (~2¹²⁶); a global 2ᵉ scale applied with `ldexp` lifts it to
   the chain cap — ~2²⁵⁰ at σ=½, ~2⁴⁰⁰ at σ=⅓. Only matters past 2⁸⁰,
   which nobody has asked for yet.
7. ~~**Tetrahedron normals.**~~ Not done, on the numbers: two walks of
   the forty-odd a lit pixel costs, each already six levels deep, is
   under 5% — and it changes the stencil, so the picture moves for
   it. Not worth a picture change.
8. ~~**Address colouring at depth.**~~ Not a bug, on analysis. The
   fraction is an f32 with a 24-bit mantissa, so digits past about
   the twelfth (base 4) cannot change it whatever the scale does; the
   underflow past 2⁶⁰ only makes explicit a limit the type already
   had. The colouring resolves ~24 bits of address by construction.

**Added 2026-09-12, from use:**

9. ~~**Glitchiness under zoom.**~~ Sections disappear when zooming in
   or out; the structure warps rather than staying perceptually fixed
   the way the Mandelbrot does; in 2D, regions appear to swap which is
   drawn "above" the other. Affected by Beam Width, and no setting
   fixes it. Found and fixed 2026-09-12: the handover pruned branches
   the view did not agree on. See the phase 3 record; the two
   measurement tables are in `does_the_seeded_walk_agree_with_each_pixels_own`
   and `does_the_seeded_chain_agree_with_each_samples_own`, gated by
   `the_seeded_walk_is_each_pixels_own` and
   `the_seeded_chain_is_each_samples_own`.
10. ~~**Solid camera: no Bank.**~~ Pitch and yaw only; the flame's 3D
    camera has the third axis. Landed 2026-09-12: the frame is the
    flame's four-angle chain now, `cam_bank` in the flame's slot. See
    the phase 3 record.
11. ~~**Solid camera: panning does nothing.**~~ The View panel's pan
    wrote `center_re`/`center_im`, which the solid camera does not
    read — it orbits `cam_target`. Landed 2026-09-12: a drag, the
    arrow keys and a zoom towards the cursor move the target across
    the screen plane at its own depth, through the camera's rolled
    right and up, in fixed point.
12. ~~**Solid camera: rotation does nothing.**~~ Landed 2026-09-12: the
    view's rotation is the chain's outermost factor, a roll of the
    screen and nothing else, turning the same way it turns the plane.

**Growing the set, 2026-09-13:** §8.7 (affine per space), §8.8 (the
julia family, three presets), §8.9 (`spherical` and `bubble`: their
DISTANCE is a disc, and their structure is in the address and the
trap — an Apollonian packing among it. The first conclusion there
was drawn from one colouring per candidate and was wrong; the
correction is recorded beside it), §8.10 (`disc`, `blob`,
`hemisphere`: Blob Flower ships, three catalogue flames reached,
and the depth check separates a picture from a parameter), §8.11
(towards quaternions: step 1, a flame-less quaternion Julia solid on
Hart's estimate, ships as a preset; steps 2 and 3 planned).

Not on the list, and why: the planar walk has no early exit because it
produces all four quantities in one pass for the record cache, and
stopping early would leave the level and address wrong for a later
colouring switch; and `steps` is not a lever (96 → 32 changed 3%).

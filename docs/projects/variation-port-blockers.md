# Variation Port Blockers

Inventory of the variations we haven't ported and what feature(s) would
need to be added to unblock them.

This document is the companion to
[`variation-bulk-port.md`](variation-bulk-port.md), which tracks what
*has* been ported. As of `jwf-variations-batch3` (2026-06-05, +3
variations: `crackle`, `dc_crackle_wf`, `dc_perlin`), the registry
holds 516 variations.

For the focused JWF "script vars" subset, see
[`jwf-common-variations-port.md`](jwf-common-variations-port.md):
187 / 190 implemented (98%); the 3 remaining (`metaballs3d_wf`,
`post_colormap_wf`, `szubieta`) all hit blockers in this doc.

## Unsupported features

These are the framework features whose absence is blocking one or more
variations. Some are hard architectural limits; others are "we just
haven't built it yet" and could be added with focused work.

### Hard architectural limits

1. **JIT-compiled user expressions** — `custom_wf` and the dc_*
   variations that take a user-typed math expression / GLSL fragment
   and compile it at flame-load time. We have a precompiled WGSL
   shader; runtime user code generation requires a totally different
   pipeline (e.g. Naga-as-library or dynamic dispatch through a
   bytecode interpreter). The cpp framework does this through Java's
   in-memory compilation.

2. **Texture / image / SVG sampling** — variations that read from a
   user-supplied image (bump map, displacement map, color map, SVG
   path raster). Requires an upload path for image bind groups and
   per-variation texture handles.

3. **~~Mid-iteration accumulator reads~~** — RESOLVED for the
   intra-iteration case (2026-05-04). Variations declare `needs_accum`;
   the codegen passes the running `result` value as a `vec2<f32>` /
   `vec3<f32>` parameter, giving them visibility into contributions
   from prior variations *in the same iteration*. Doesn't kill
   parallelism since each thread is already sequential through its
   variation chain. The original framing was wrong — the parallelism
   killer is *cross-thread* accumulator reads (one pixel reading
   another's state), which still doesn't exist in any audited
   variation. See
   [`intra-iteration-state-and-accum.md`](intra-iteration-state-and-accum.md).

4. **Pre-affine input read in post phase** — `post_depth` and similar
   need access to `pAffineTP` (the pre-affine input) inside a post-
   phase variation. Our post-phase variations only see the variation
   output `p`. Would require widening the post-phase calling
   convention.

5. **~~Color-register reads in non-color path~~** — RESOLVED. The
   `vc: ptr<function, f32>` parameter passed to `writes_color`
   variations is a true read/write pointer; WGSL has no write-only
   pointer type, and the codegen passes the same pointer through
   every variation call in an iteration. A variation can read `*vc`
   for any spatial purpose — drive Z from TC, scale by color, etc. —
   the same way a variation reads any other input. The "write-only
   from the variation perspective" framing was incorrect after the
   `vc` parameter landed; this row predated that work. Confirmed
   empirically by the `dc_carpet3D` full port (2026-05-29): changing
   `color_a..color_f` with the transform's `direct_color` slider at
   0 changes the 3D structure (proves read+spatial path) without
   changing the rendered color (proves direct_color gating still
   works). See blocker #11 below — same mechanism, same resolution.

### Soft blocks (could be implemented)

6. **~~Persistent variation state~~** — RESOLVED for per-thread,
   per-dispatch case (2026-05-04) by `var<private> thread_state` with
   per-(xform, variation) baked offsets and optional `wgsl_state_init`
   for thread-start initialization. State persists across the inner
   iteration loop within a single shader invocation; re-initializes
   each compute dispatch. See
   [`intra-iteration-state-and-accum.md`](intra-iteration-state-and-accum.md).
   The remaining gap is *cross-dispatch* persistence (state surviving
   between frames), which only mandelbrot's point-cache appears to
   actually need from the audited cases — and even there, re-init each
   dispatch with random sampling is probably visually equivalent.

7. **Primitives infrastructure** — turtle paths, line/triangle/polygon
   plotting against precomputed shapes (Hilbert curve, Koch snowflake,
   Tree fractal, Voronoi cells, regular polygons). Would need
   CPU-side shape generation per primitive type, upload as buffers,
   and a per-shape sampler.

8. **Abstract base classes** — `DC_BaseFunc`, `AbstractColorMapWFFunc`,
   `AbstractDisplacementMapWFFunc`, `AbstractFalloff3Func`. Each is a
   shared spatial/color skeleton with virtual-method overrides per
   derived variation. `DC_BaseFunc` was previously listed as gated
   on #11 (the spatial transform reduces to `linear` or `uniform
   blur` once color writes are dropped, so the 34 derivatives would
   gain no spatial distinctiveness). **`DC_BaseFunc` is now
   unblocked** as of the 2026-05-29 #5/#11 resolution: the
   derivatives can be ported as `writes_color: true` with a color
   body, and they recover their distinctiveness through the color
   register the same way the `dc_carpet3D` port does. The other three
   bases (`AbstractColorMapWF`, `AbstractDisplacementMapWF`,
   `AbstractFalloff3`) remain gated on texture/displacement sampling
   (#2) or on the falloff coefficient table.

9. **Subflames** — `subflame_wf` and friends invoke an entire other
   flame as a substep. Requires nested-flame execution infrastructure
   (recursive shader dispatch or precomputed subflame point cache).

10. **~~16-slot per-variation budget~~** — RESOLVED 2026-05-04 by the
    packed-variation-params buffer (see
    [`packed-variation-params.md`](packed-variation-params.md)). Each
    variation now allocates exactly its slot count (user + init), with
    per-flame compile-time offsets baked into the WGSL `get_param`
    switch. Quaternion at 93 slots and the other 7 slot-blocked
    variations are now ported. Remaining variations under this row
    are blocked only by additional features (#11, #12, etc.), not by
    slot count.

11. **~~Color-write affecting spatial output~~** — RESOLVED 2026-05-29
    by the `dc_carpet3D` full port. Same `vc: ptr<function, f32>`
    mechanism as #5: a variation can `let cur = *vc; *vc = next;`
    then use `next` (or `cur`) in its spatial output, all in the
    same body. The 2026-05-29 dc_carpet3D port writes the color and
    then re-reads it to compute `dz = vc · scale_z + offset_z`
    exactly as the JWildfire Java source does. Empirical
    confirmation: with the transform's `direct_color = 0`,
    color-param changes shift the 3D structure while leaving the
    rendered color unchanged — proving the spatial output reads vc
    and uses it independently of color rendering. The 34
    `DC_BaseFunc` derivatives (#8 below) are now portable as
    standard `writes_color: true` variations.

12. **Prepost (priority-2) execution** — variations like
    `prepost_circlize` and `prepost_mobius` run *both* a pre-variation
    body and a post-variation body in the same iteration, typically
    inverses of each other, forming a non-linear sandwich around the
    normal variations + post-affine. We only have pre xor post phases.
    A single-phase compromise port (running just the post half as a
    normal-phase variation) was tried and reverted — it loses the
    sandwich semantics entirely and is indistinguishable from plain
    `circlize` / `mobius`. Proper port needs a `VariationPhase::PrePost`
    that registers two bodies and runs them in both slots; deferred
    pending architectural decision on whether the family is worth the
    plumbing.

13. **Unbounded / data-dependent loops** — some variations have
    `do { } while (cond)` rejection sampling or accumulator-state-
    dependent loops where the iteration count varies per call.
    Already-shipped compromise: cap at 32 iterations and emit the
    last sample. Some variations need higher caps that affect
    performance.

14. **~~Complex math runtime~~** — RESOLVED 2026-05-05 by
    [`shaders/core/complex.wgsl`](../../shaders/core/complex.wgsl)
    (~90 LoC). Provides `cadd, csub, cmul, cdiv, cconj, cmag2, csquare,
    csqrt, cmul_real` and a `CMat2` type with `cmat2_make, cmat2_apply`
    (Möbius transformation), `cmat2_inverse_sl2`. Injected before
    `utilities.wgsl` in main / tiled / export / init shaders.
    Transcendental complex functions (`cexp, clog, csin/cos/tan, cpow,
    csinh, asinh/acosh/...`) NOT included — adds opportunistically as
    future variations need them. Unblocked `klein_group` (Indra's
    Pearls Kleinian limit-set chaos game). See
    [`complex-math-and-klein-group.md`](complex-math-and-klein-group.md).

### Special case: complete duplicates

Three variations in the cpp set are exact (or near-exact) duplicates
of variations we already have in the registry:

- `linear3d` — same as `linear` (handles 3D when active)
- `cylinder_apo` — same as `cylinder`
- `log_apo` — same as `log` (we already have a base parameter)

These are listed below for completeness but are not "missing" in any
practical sense.

## Unported variations (165)

Format: `name` — *primary blocker*. Some have multiple blockers; we
list the one most likely to be the easiest unlock.

### Persistent state (#6) — 19 remaining (3 ported, 2026-05-04)

Per-thread state + intra-iteration accum-reads infrastructure (blocker
#6 + #3) shipped on the `port-stateful-variations` branch. Ported as
validation:

| Variation | Slots | Notes |
|---|---|---|
| `curliecue2` | 4 state | Sosa walker; first state-only port |
| `farblur` | 5 state + accum | zephyrtronium; first needs_accum port |
| `macmillan` | 3 state + accum + writes_color + state_init | First port using all four new mechanisms |
| `hexaplay3D` | 3 state + accum + replacement-style accum write | Berlin 2009 hex snowflake (2026-05-05) |
| `hexnix3D` | 3 state + accum + replacement-style + smooth/3-mode majplane | Berlin 2009 animated-friendly variant (2026-05-05) |
| `klein_group` | 1 state + 16 init + complex math + state_init | Indra's Pearls Kleinian chaos game; 7 recipes (Grandma/Maskit/Jorgensen/Riley/+modified) (2026-05-05) |

Still pending (originally listed under #6 but mostly blocked by other
features once you read the cpp body carefully):

| Variation | Notes |
|---|---|
| `arctruchet` | `_tiltArray` precomputed grid table — actually #7 (primitives) and incomplete cpp port |
| `brownian_js` | Brownian-path canvas — actually #7 (primitives) |
| `curliecue` | `_x0/_y0/_theta/_phi` accumulators — actually #7 (`primitives.add(...)`) |
| `dc_dmodulus` | `_oldColor` accumulator (also DC_BaseFunc #8) |
| `dc_cracklep_wf` | Crackle algorithm state (also #8) — sibling of the landed `dc_crackle_wf` but with extra persistent state we don't yet support. |
| `nblur` | Rejection-sampling state |
| `pre_stabilize` | `x[64]/y[64]/c[64]` plus `start` flag — needs custom thread-init |
| `recurrenceplot` | `_oldx/_oldy` + 30+ helper functions across 879 lines — own project |
| `scrambly` | 626-element scrambled lookup table |
| `sphtiling3v2` | `xy/uv` accumulators across iterations |
| `ztwister` | Reads `FPz` accumulator (`ez = twist*FPz`) |

### JIT-compiled custom code (#1) — ~28 variations

| Variation | Notes |
|---|---|
| `colordomain` | `cf_runner.f(z)` user-defined complex function |
| `custom_wf` | User-typed math expression |
| `custom_wf_full` | Same |
| `pre_custom_wf` | Same |
| `post_custom_wf` | Same |
| `dc_code` | Code-based palette function |
| `ducks` | `cf_runner.f(z, c)` user-defined fractal iter |
| `glsl_` | Bare GLSL JIT base |
| `glsl_acrilic` | GLSL fragment + `DC_BaseFunc` |
| `glsl_apollonian` | GLSL fragment + `DC_BaseFunc` |
| `glsl_circlesblue` | GLSL fragment + `DC_BaseFunc` |
| `glsl_circuits` | GLSL fragment + `DC_BaseFunc` |
| `glsl_code` | GLSL fragment + `DC_BaseFunc` |
| `glsl_fractaldots` | GLSL fragment + `DC_BaseFunc` |
| `glsl_grid3d` | GLSL fragment + `DC_BaseFunc` |
| `glsl_hoshi` | GLSL fragment + `DC_BaseFunc` |
| `glsl_hyperbolictile` | GLSL fragment + `DC_BaseFunc` |
| `glsl_kaleidocomplex` | GLSL fragment + `DC_BaseFunc` |
| `glsl_kaleidoscopic` | GLSL fragment + `DC_BaseFunc` |
| `glsl_kaliset` | GLSL fragment + `DC_BaseFunc` |
| `glsl_kaliset2` | GLSL fragment + `DC_BaseFunc` |
| `glsl_mandala` | GLSL fragment + `DC_BaseFunc` |
| `glsl_mandelbox2d` | GLSL fragment + `DC_BaseFunc` |
| `glsl_randomoctree` | GLSL fragment + `DC_BaseFunc` |
| `glsl_squares` | GLSL fragment + `DC_BaseFunc` |
| `glsl_starsfield` | GLSL fragment + `DC_BaseFunc` |

### `DC_BaseFunc` derivatives — infrastructure unblocked, per-variation porting remains

These extend `DC_BaseFunc` and override `getRGBColor(uV)`. The base's
spatial transform is `linear` (when `colorOnly=1`) or `uniform blur in
[-0.5, 0.5]` (when `colorOnly=0`, the default) — that part is trivial.
The distinctive content is in the color output, computed by each
derivative's `getRGBColor()`.

**Status:** With the 2026-05-29 #5/#11 resolution, the *color-pipeline*
gate is gone — these can write `*vc` and the spatial output can read it
back, same as `dc_carpet3D`. **But the original blockers framing
("trivial color body ports") was misleading.** Each derivative is a
real port:

- `getRGBColor()` is typically 100–200 lines of GLSL-style procedural
  pattern math (modulo, trig, multi-octave iteration, distance fields,
  noise hashing) referencing JWildfire's `js.glsl.G` namespace.
- Several need infrastructure we don't have yet: Worley/Voronoi cell
  sampling (`dc_voronoise`), more transcendental complex functions on
  top of the Klein-group baseline in
  [`shaders/core/complex.wgsl`](../../shaders/core/complex.wgsl)
  (`dc_apollonian`, `dc_mandbrot`, `dc_mandelbox2d`, `dc_kaliset`,
  `dc_kaliset2`, etc.). The Perlin/simplex infra is now in place via
  `shaders/core/noise.wgsl` (table-free Gustavson port) — `dc_perlin`
  itself landed in batch 3, and any other Perlin-dependent dc_*
  derivative can reuse the same helper.
- Several use time-based animation params (`time`, elapsed-ms walls)
  that we don't track per-iteration today.
- The base class's `gradient=0` and `gradient=1` modes inject
  `pVarTP.{red,green,blue}Color` directly, bypassing the palette.
  Our `vc: f32` register only carries a palette index; the
  `gradient=2` mode (greyscale luminance → palette index) maps to
  `*vc` cleanly, but mode-0 and mode-1 would need widening the
  accumulator to carry RGB — a separate shader-side feature.

Realistic per-variation cost: 2–4 hours for simpler derivatives like
`dc_squares` or `dc_rotations`; longer for those needing new primitives.
Total for the set: tens of hours minimum.

Listed below until ported. Pick individually or in thematic batches as
useful flames need them.

`dc_acrilic`, `dc_apollonian`, `dc_base`, `dc_circlesblue`,
`dc_circuits`, `dc_ducks`, `dc_escher`, `dc_fractaldots`, `dc_gnarly`,
`dc_grid3d`, `dc_hexagons`, `dc_hoshi`, `dc_hyperbolictile`,
`dc_kaleidocomplex`, `dc_kaleidoscopic`, `dc_kaleidotile`, `dc_kaliset`,
`dc_kaliset2`, `dc_layers`, `dc_mandala`, `dc_mandbrot`,
`dc_mandelbox2d`, `dc_menger`, `dc_randomoctree`,
`dc_rotations`, `dc_squares`, `dc_starsfield`, `dc_tesla`, `dc_tree`,
`dc_tritile`, `dc_truchet`, `dc_turbulence`, `dc_voronoise`.

(`dc_perlin` was on this list until batch 3 — landed via
`src/variations/defs/dc_perlin.rs` on top of `shaders/core/noise.wgsl`.)

### Other abstract base classes (#8) — 7 variations

| Variation | Base class |
|---|---|
| `colormap_wf` | `AbstractColorMapWFFunc` (texture sampler #2 too) |
| `post_colormap_wf` | `AbstractColorMapWFFunc` |
| `displacemap_wf` | `AbstractDisplacementMapWFFunc` (#2 too) |
| `post_displacemap_wf` | `AbstractDisplacementMapWFFunc` |
| `falloff3` | `AbstractFalloff3Func` |
| `pre_falloff3` | `AbstractFalloff3Func` |
| `post_falloff3` | `AbstractFalloff3Func` |

### Primitives infrastructure (#7) — 14 variations

| Variation | Primitive type |
|---|---|
| `dragon_js` | Turtle path (Heighway dragon) |
| `gosperisland_js` | Turtle path (Gosper island) |
| `gpattern` | Generic `Primitive` dispatch (Line/Triangle/etc.) |
| `hexes` | Voronoi cell (`VORONOI_MAXPOINTS = 25`) |
| `hilbert_js` | Turtle path (Hilbert curve) |
| `htree_js` | Turtle path (H-tree fractal) |
| `inversion` | `shape.getCurvePoint(theta)` from precomputed shape |
| `koch_js` | Turtle path (Koch snowflake) |
| `lsystem_js` | Turtle path (L-system rewriter) |
| `mandala` | `Ngon` polygon |
| `mandala2` | `Ngon` polygon |
| `nsudoku` | `Ngon` polygon (sudoku grid) |
| `rsquares_js` | Random-squares primitive |
| `sunflower` | Spiral-arrangement primitive |
| `sunvoroni` | Voronoi cell layout |
| `szubieta` | Custom primitive |
| `taprats` | Tap-rats tile primitive |
| `tree_js` | Tree-fractal turtle path |
| `triantruchet` | `Tile`/`Triangle` rotation + `plotTriangle` |

### Subflames (#9) — 5 variations

`subflame_wf`, `pre_subflame_wf`, `ringsubflame`, `glynns3subfl`,
`starfractal`.

### Texture / image sampling (#2) — 6 variations

| Variation | Notes |
|---|---|
| `post_bumpmap_wf` | Bump-map texture displacement |
| `text_wf` | Text glyph point cache (font rasterizer) |
| `svg_wf` | SVG path rasterizer + sampler |
| `primitives_wf` | Renderable primitive bank |
| `displacemap_wf` | Displacement map texture (also #8) |
| `post_displacemap_wf` | Same |

### Color-register read — unblocked 2026-05-29 (was #5) — 4 variations

Previously gated on blocker #5, now portable with the standard
`writes_color: true` + `*vc` read pattern (see `dc_carpet3D` for the
template). Listed here until ported.

| Variation | Notes |
|---|---|
| `pre_dcztransl` | Reads `TC` to compute pre-affine z |
| `dc_ztransl` | Reads `*pColor` to compute z |
| `colorscale_wf` | Reads `TC` for z scaling |
| `post_colorscale_wf` | Same |

### Pre-affine input read in post phase (#4) — 1 variation

| Variation | Notes |
|---|---|
| `post_depth` | Reads both `pVarTP.z` and `pAffineTP.x/y/z` in post phase |

### Mid-iteration accumulator read (#3) — 5 variations

| Variation | Notes |
|---|---|
| `cubic3d` | Reads `FPx/FPy/FPz` as `(FPx + FTx)` |
| `cubiclattice_3d` | Same `(FPx + FTx)` pattern |
| `roundspher3d` | Reads `FPz` (`tempPZ = FPz` branch) |
| `scry_3d` | Reads `FPz` (`Foopzee = FPz` branch) |
| `post_smartcrop` | Reads `*pFPx/*pFPy/*pFPz` via stored pointers + persistent state |

### Over 16-slot budget (#10) — 2 remaining (11 ported)

Packed-variation-params buffer eliminated the per-variation 16-slot
ceiling. Ported across `packed-variation-params` (8) and
`port-easy-moderate-batch` (3):

| Variation | Slots | Notes |
|---|---|---|
| `vibration2` | 26 user | Two-wave directional vibration (FarDareisMai) |
| `gridout3D` | 26 user | 8-region grid offset (Faber & Faber, Java-recovered) |
| `jubiq` | 24 user + 2 init | Quaternion Möbius / julian2 mix |
| `superShape3d` | 16 user + 10 init | Two-axis Gielis super-shape (cpp's `M_2_PI = 2/π` preserved) |
| `z` | 13 user + 7 init | Faber Lost Variations radial boost |
| `w` | 14 user + 7 init | Faber Lost Variations angle-rotate-and-clip |
| `quaternion` | 92 user + 1 init | zephyrtronium / Stefanov 13-subfunction mega-variation |
| `xtrb` | 6 user + 22 init | Zueuk's TriBorders trilinear hex variation |
| `harmonograph_js` | 18 user | Sosa damped-pendulum harmonograph (2026-05-05) |
| `rhodonea` | 15 user + 5 init | CozyG rose curves with 7×7 mode switch (2026-05-05) |
| `complex` | 64 user | cothe / Stefanov 14-subfunction 2D analog of quaternion (2026-05-05) |

Still pending — additional non-slot blockers prevent porting:

| Variation | Slot count | Other blockers |
|---|---|---|
| `pre_recip` | 15 user + complex math | #14 transcendentals (`Complex.AsinH/AcosH/AtanH/AsecH/AcosecH/AcotH`) — basic ops now available, transcendentals still needed |
| `prepost_affine` | 15 user + 18 init | #12 prepost — needs phase compromise |

### Mandelbrot / fractal-iteration family — 2 remaining

Originally listed as 8 — six of them turned out to be straightforward
ports once we read JWildfire's own `getGPUCode()`, which hand-translates
each iterator to CUDA. Those six (Dragon, Julia, Mandelbrot, Meteors,
Pearls, Salamander) ship in `jwf-variations-batch2`.

The remaining two extend `AbstractFractFormulaWFFunc` (not the simpler
`AbstractFractWFFunc` the others use) and are marked
`NotDesiredForGPURendering` in JWildfire itself — they take a
user-typed math expression and walk it via a stack-based interpreter
per iteration. That's blocker #1 (JIT-compiled user expressions).

| Variation | Notes |
|---|---|
| `fract_formula_julia_wf` | User-typed formula Julia. Blocker #1 (JIT user expressions). |
| `fract_formula_mand_wf` | User-typed formula Mandelbrot. Blocker #1. |

### Prepost (#12) — 1 remaining

| Variation | Notes |
|---|---|
| `prepost_affine` | Also #10 over budget (15 user + 18 init) |

### Other / multi-blocker — 13 variations

| Variation | Notes |
|---|---|
| `bwrands` | bwraps cousin with bytemix/byteshf bit-mixing — moderate effort, no hard blocker |
| `crob` | Needs investigation (7 user, body unread) |
| `gdoffs` | 8 user + helper functions (`fosc`, `fclp`, `flip`) — no hard blocker, moderate effort |
| `checkerboard_wf` | TC color writes + 2D plane + `getDisplacement`/`getColor` helpers; tractable but writes_color compromise affects spatial output |
| `plane_wf` | Similar to checkerboard_wf — uniform plane sample with color/z driven by `setColor`/`getDisplacement` |
| `grid3d_wf` | 16 user + branching structure with TC writes — at slot limit + writes_color |
| `hexes` | Voronoi cells (#7) + abstract base for `dc_hexes_wf` (#8) |
| `dc_hexes_wf` | Same Voronoi infrastructure as `hexes` |
| `knots3d` | 12 user + complex 3D knot parametrization — likely tractable, needs investigation |
| `polylogarithm` | Riemann zeta function table + factorial overflow handling + dual series (Crandall/Erdelyi); doable but substantial |
| `prose3d` | Many params + Java-only `Math.copySign` idioms |
| `supershape3d` | 16 user (at slot limit) + complex super-shape body |
| `synth` | 1149 lines, synthesizer-style waveform compositor — way beyond scope |
| `dla_wf` / `dla3d_wf` | DLA = diffusion-limited aggregation, needs precomputed cluster (#7) |
| `maurer_lines` | 36 user params (#10 over budget) + meta-mode/coset/tangent/sampling subsystems (huge) |

### Already covered in registry (duplicates) — 3 variations

| Variation | Equivalent |
|---|---|
| `linear3d` | `linear` (handles 3D when active) |
| `cylinder_apo` | `cylinder` |
| `log_apo` | `log` (we have a base parameter) |

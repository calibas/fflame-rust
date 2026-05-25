# Variations — Running TODO List

Notes worth keeping but not addressing right now. Accumulated as we work
through the bulk metadata review (Phase 4 of
[VARIATIONS_BULK_METADATA_IMPORT.md](VARIATIONS_BULK_METADATA_IMPORT.md)).

Two buckets: things that belong to the metadata-import project itself,
and things we'll defer to other branches when we hit them. Add freely;
prune when something lands.

---

## In scope (variations-bulk-metadata branch)

### Author attribution research

Variations encountered with no obvious author. Need a research pass
(JWildfire history, Apophysis docs, original `.cpp` headers) before we
can fill in `# Authors`. Leave the section omitted on the static until
the answer is known — that's the convention for "unknown" per
[VARIATIONS_BULK_METADATA_IMPORT.md §3.3](VARIATIONS_BULK_METADATA_IMPORT.md).

- `rings2` ([advanced.rs](../../src/variations/defs/advanced.rs))
- `log` ([advanced.rs](../../src/variations/defs/advanced.rs))
- `zcone` ([depth3d.rs](../../src/variations/defs/depth3d.rs))
- `flatten` ([depth3d.rs](../../src/variations/defs/depth3d.rs))
- `zscale` ([depth3d.rs](../../src/variations/defs/depth3d.rs))
- `pre_rotate_x` ([rotation3d.rs](../../src/variations/defs/rotation3d.rs))
- `pre_rotate_y` ([rotation3d.rs](../../src/variations/defs/rotation3d.rs))
- `post_rotate_x` ([rotation3d.rs](../../src/variations/defs/rotation3d.rs))
- `post_rotate_y` ([rotation3d.rs](../../src/variations/defs/rotation3d.rs))
- `hemisphere` ([full3d.rs](../../src/variations/defs/full3d.rs))
- `zblur` ([blur.rs](../../src/variations/defs/blur.rs))
- `blur3d` ([blur.rs](../../src/variations/defs/blur.rs))
- `pre_blur` ([blur.rs](../../src/variations/defs/blur.rs))
- `pre_zscale` ([pre_phase.rs](../../src/variations/defs/pre_phase.rs))
- `pre_ztranslate` ([pre_phase.rs](../../src/variations/defs/pre_phase.rs))
- `pre_bwraps` ([pre_phase.rs](../../src/variations/defs/pre_phase.rs))
- `pre_falloff2` ([pre_phase.rs](../../src/variations/defs/pre_phase.rs))
- `post_bwraps` ([post_phase.rs](../../src/variations/defs/post_phase.rs))
- `post_falloff2` ([post_phase.rs](../../src/variations/defs/post_phase.rs))
- `post_curl3d` ([post_phase.rs](../../src/variations/defs/post_phase.rs))
- `ztranslate` ([extended.rs](../../src/variations/defs/extended.rs))
- `falloff2` ([extended.rs](../../src/variations/defs/extended.rs))
- `bwraps` ([extended.rs](../../src/variations/defs/extended.rs))
- `julia3dz` ([extended.rs](../../src/variations/defs/extended.rs))
- `curl3d` ([extended.rs](../../src/variations/defs/extended.rs))
- `blur_circle` ([extended.rs](../../src/variations/defs/extended.rs))
- `blur_zoom` ([extended.rs](../../src/variations/defs/extended.rs))
- `blur_pixelize` ([extended.rs](../../src/variations/defs/extended.rs))
- `tangent` ([shapes.rs](../../src/variations/defs/shapes.rs))
- `tangent3d` ([shapes.rs](../../src/variations/defs/shapes.rs))
- `secant2` ([shapes.rs](../../src/variations/defs/shapes.rs))
- `cosine` ([shapes.rs](../../src/variations/defs/shapes.rs))
- `pie3d` ([shapes.rs](../../src/variations/defs/shapes.rs))
- `butterfly3d` ([shapes2.rs](../../src/variations/defs/shapes2.rs))
- `spherical3d` ([numbered.rs](../../src/variations/defs/numbered.rs))
- `square` ([numbered.rs](../../src/variations/defs/numbered.rs))
- `square3d` ([numbered.rs](../../src/variations/defs/numbered.rs))
- `disc2` ([heavy_init.rs](../../src/variations/defs/heavy_init.rs))
- `blob3d` ([numbered_extras.rs](../../src/variations/defs/numbered_extras.rs))
- `super_shape` ([shapes3.rs](../../src/variations/defs/shapes3.rs))
- `spirograph` ([parametric_curves.rs](../../src/variations/defs/parametric_curves.rs))
- `cylinder2` ([stub_recoveries.rs](../../src/variations/defs/stub_recoveries.rs))
- `pulse` ([stub_recoveries.rs](../../src/variations/defs/stub_recoveries.rs))
- `ptransform` ([misc_extras.rs](../../src/variations/defs/misc_extras.rs))
- `squircular` ([watchlist_misc.rs](../../src/variations/defs/watchlist_misc.rs))
- `bi_linear` ([classic_blades_misc.rs](../../src/variations/defs/classic_blades_misc.rs))
- `twoface` ([classic_blades_misc.rs](../../src/variations/defs/classic_blades_misc.rs))
- `unpolar` ([classic_blades_misc.rs](../../src/variations/defs/classic_blades_misc.rs))
- `power` ([apo_misc.rs](../../src/variations/defs/apo_misc.rs))
- `exp2` ([simple_classics.rs](../../src/variations/defs/simple_classics.rs))
- `invpolar` ([simple_classics.rs](../../src/variations/defs/simple_classics.rs))
- `nPolar` ([apo_misc7.rs](../../src/variations/defs/apo_misc7.rs))
- `hyperbolicellipse` ([apo_misc8.rs](../../src/variations/defs/apo_misc8.rs))
- `swirl3` ([apo_misc11.rs](../../src/variations/defs/apo_misc11.rs))
- `invsquircular` ([apo_misc11.rs](../../src/variations/defs/apo_misc11.rs))
- `rings` ([apo_misc12.rs](../../src/variations/defs/apo_misc12.rs))

**Likely shared author** — all are JWildfire ports of complex-plane
inverse hyperbolic functions (the plain ones in `hyperbolic.rs` and
the `sqrt(z)`-prefixed siblings in `sqrt_hyperbolic.rs`). Finding the
author of any one should resolve the rest of the group.

- `acosech` ([hyperbolic.rs](../../src/variations/defs/hyperbolic.rs))
- `arcsech` ([hyperbolic.rs](../../src/variations/defs/hyperbolic.rs))
- `arcsech2` ([hyperbolic.rs](../../src/variations/defs/hyperbolic.rs))
- `arcsinh` ([hyperbolic.rs](../../src/variations/defs/hyperbolic.rs))
- `arctanh` ([hyperbolic.rs](../../src/variations/defs/hyperbolic.rs))
- `sqrt_acoth` ([sqrt_hyperbolic.rs](../../src/variations/defs/sqrt_hyperbolic.rs))
- `sqrt_acosh` ([sqrt_hyperbolic.rs](../../src/variations/defs/sqrt_hyperbolic.rs))
- `sqrt_acosech` ([sqrt_hyperbolic.rs](../../src/variations/defs/sqrt_hyperbolic.rs))
- `sqrt_asech` ([sqrt_hyperbolic.rs](../../src/variations/defs/sqrt_hyperbolic.rs))
- `sqrt_asinh` ([sqrt_hyperbolic.rs](../../src/variations/defs/sqrt_hyperbolic.rs))
- `sqrt_atanh` ([sqrt_hyperbolic.rs](../../src/variations/defs/sqrt_hyperbolic.rs))

**Apophysis Plugin Pack** — currently attributed to the pack itself
([output/apo-plugins/](../../output/apo-plugins/), sourceforge.net/projects/apo-plugins).
Each variation in the pack has a real original author we haven't dug
up yet; resolve attribution one-by-one and replace the placeholder
when found.

- `bent2` ([misc_extras2.rs](../../src/variations/defs/misc_extras2.rs))
- `bipolar` ([advanced.rs](../../src/variations/defs/advanced.rs))
- `bipolar2` ([numbered_extras.rs](../../src/variations/defs/numbered_extras.rs)) — also credited to Brad Stefanov
- `boarders` ([boarders.rs](../../src/variations/defs/boarders.rs))
- `butterfly` ([shapes2.rs](../../src/variations/defs/shapes2.rs))
- `cell` ([shapes2.rs](../../src/variations/defs/shapes2.rs))
- `circlize` ([singleton_misc.rs](../../src/variations/defs/singleton_misc.rs))
- `cpow` ([advanced.rs](../../src/variations/defs/advanced.rs))
- `curve` ([erf_misc.rs](../../src/variations/defs/erf_misc.rs))
- `edisc` ([erf_misc.rs](../../src/variations/defs/erf_misc.rs))
- `elliptic` ([advanced.rs](../../src/variations/defs/advanced.rs))
- `escher` ([advanced.rs](../../src/variations/defs/advanced.rs))
- `foci` ([advanced.rs](../../src/variations/defs/advanced.rs))
- `lazysusan` ([advanced.rs](../../src/variations/defs/advanced.rs))
- `loonie` ([advanced.rs](../../src/variations/defs/advanced.rs))
- `modulus` ([singleton_misc.rs](../../src/variations/defs/singleton_misc.rs))
- `ngon` ([advanced.rs](../../src/variations/defs/advanced.rs))
- `oscilloscope` ([misc_extras2.rs](../../src/variations/defs/misc_extras2.rs))
- `pie` ([shapes.rs](../../src/variations/defs/shapes.rs))
- `polar2` ([advanced.rs](../../src/variations/defs/advanced.rs))
- `popcorn2` ([numbered.rs](../../src/variations/defs/numbered.rs))
- `scry` ([advanced.rs](../../src/variations/defs/advanced.rs))
- `separation` ([extended.rs](../../src/variations/defs/extended.rs))
- `split` ([misc_2d.rs](../../src/variations/defs/misc_2d.rs))
- `splits` ([advanced.rs](../../src/variations/defs/advanced.rs))
- `stripes` ([misc_2d.rs](../../src/variations/defs/misc_2d.rs))
- `wedge` ([extended.rs](../../src/variations/defs/extended.rs))
- `wedge_julia` ([wedge_extended.rs](../../src/variations/defs/wedge_extended.rs))
- `wedge_sph` ([wedge_extended.rs](../../src/variations/defs/wedge_extended.rs))
- `whorl` ([misc_extras4.rs](../../src/variations/defs/misc_extras4.rs))

### Enum-candidate parameters

Confirmed during review — currently declared as `Integer` but
semantically picks among a few labeled modes. Convert during the Phase 3
type-correction pass.

**Apophysis-import compatibility:** the Phase 3 conversion has to
preserve the meaning of values coming in from `.flame` XML written by
Apophysis / JWildfire, which store these parameters as raw floats. For
most candidates the mapping is mechanical:

- `Integer [0, 1]` → `Boolean`: read as `value >= 0.5` (or `!= 0.0`).
  Should be safe to automate across all entries below that fit this
  shape.
- `Integer [0, N-1]` → `Enum` with N variants: round to nearest, clamp
  to `[0, N-1]`, look up by index. Also safe to automate.

The tricky ones are parameters where the wire-format value space does
**not** discretely line up with the enum's variant count — most
visibly `hypercrop.zero`, which is currently `unlimited_float [0, 2]`
but semantically tri-state via threshold comparisons (`> 1.5`,
`> 0.5`, else). A naive enum mapping (e.g. `0/1/2`) round-trips fine
for newly-saved values, but an imported flame with `zero = 0.7`
currently means "collapse to origin" — under enum semantics that value
has no slot, so we'd need an explicit per-parameter import shim that
reproduces the original threshold dispatch when reading legacy XML.
Decision pending: either keep this one as `Float` with documented
thresholds, or add a `legacy_import` hook on the param def.

- `falloff2.type`, `pre_falloff2.type`, `post_falloff2.type` — 3
  branches (0 = uniform, 1 = triangular, 2 = gaussian). Same enum
  across all three phase variants. See
  [extended.rs](../../src/variations/defs/extended.rs),
  [pre_phase.rs](../../src/variations/defs/pre_phase.rs),
  [post_phase.rs](../../src/variations/defs/post_phase.rs).
- `hole2.shape` — 10 distinct radial-formula branches (shape 0-9). See
  [standalone_exotics.rs](../../src/variations/defs/standalone_exotics.rs).
- `hole2.inside` — declared `Integer` with `[0, 1]` range, used as a
  binary toggle (`r1 = w/r1` vs `r1 = w·r1`). Should be `Boolean`.
  See [standalone_exotics.rs](../../src/variations/defs/standalone_exotics.rs).
- `crop3d.zero` — declared `Integer` with `[0, 1]` range, used as a
  binary toggle (collapse-to-origin vs scatter-to-edge). Should be
  `Boolean`. See
  [parametric_curves.rs](../../src/variations/defs/parametric_curves.rs).
- `hypercrop.zero` — declared `unlimited_float` with `[0, 2]` range,
  dispatched as 3 modes via threshold comparisons (`> 1.5` snap to
  corner, `> 0.5` collapse to origin, else scatter). See
  [maurer_hyper.rs](../../src/variations/defs/maurer_hyper.rs).
- `chunk.mode` — declared `Integer` with `[0, 1]` range, used as a
  binary toggle (keep r ≤ 0 vs keep r > 0). Should be `Boolean`. See
  [misc_extras.rs](../../src/variations/defs/misc_extras.rs).
- `ptransform.use_log` — declared `Integer` with `[0, 1]` range, used
  as a binary toggle (linear vs log-polar ρ). Should be `Boolean`. See
  [misc_extras.rs](../../src/variations/defs/misc_extras.rs).
- `tile_reverse.vertical` — declared `Integer` with `[0, 1]` range,
  used as a binary axis selector (horizontal vs vertical). Should be
  `Boolean`. See
  [misc_extras.rs](../../src/variations/defs/misc_extras.rs).
- `tile_reverse.reversal` — declared `unlimited_float` with `[-10, 10]`
  range, but only checked for `== 1.0` vs anything else (binary mirror
  toggle). Type is misleading; semantics are Boolean. See
  [misc_extras.rs](../../src/variations/defs/misc_extras.rs).
- `corners.logmode` — declared `Integer` with `[0, 1]` range, used as
  a binary formula selector (pow vs log-pow). Should be `Boolean`. See
  [singleton_misc.rs](../../src/variations/defs/singleton_misc.rs).
- `atan.mode` — declared `Integer` with `[0, 2]` range, 3-mode axis
  selector (Y only / X only / both). Should be `Enum`. See
  [singleton_misc.rs](../../src/variations/defs/singleton_misc.rs).
- `tqmirror.type` — declared `Integer` with `[0, 1]` range, used as a
  binary outer-boundary branch selector (swap x↔y vs pass through).
  Should be `Boolean`. See
  [stub_recoveries2.rs](../../src/variations/defs/stub_recoveries2.rs).
- `ripple.fixed_dist_calc` — declared `Integer` with `[0, 1]` range,
  used as a binary distance-formula selector (Euclidean vs upstream-
  quirk product). Should be `Boolean`. See
  [apo_misc13.rs](../../src/variations/defs/apo_misc13.rs).

### Numerical edge-case divergence from upstream

Across the variations we systematically guard against NaN/inf with
`max(x, 1e-30)` (before `log`/`pow`/`sqrt`) and
`select(u, 1e-30, abs(u) < 1e-30)` (before division). Upstream cpp
generally doesn't bother — it lets the float arithmetic produce NaN
or ±inf, which propagates and typically gets discarded by the
histogram-write step.

For most cases this is a wash: our extreme-finite values either land
off-canvas or get clamped, same as NaN. But a few cases produce
visibly different output:

- **`disc3` sqrt-clamp** — at
  [stub_recoveries2.rs:84](../../src/variations/defs/stub_recoveries2.rs#L84)
  we do `sqrt(max(x²·d·e + y²·f·g, 0.0))`. When `d·e < 0` or
  `f·g < 0`, the argument can be negative; upstream cpp returns NaN
  (which propagates to discarded points), while our clamp returns
  `sqrt(0) = 0`, yielding `sin(0) = 0, cos(0) = 1` and a real plotted
  point near the origin that wouldn't exist upstream.

The general guard pattern is probably fine, but it's worth a render
comparison on a flame with parameters that exercise the edge cases
(e.g. `disc3` with `d = -1, e = 1`) to confirm we're not introducing
spurious origin artifacts. If we are, we may need to special-case the
`sqrt` clamp (and similar) to propagate a sentinel that the
dispatcher recognizes as "drop this point."

### Init-slot optimization opportunities

Variations whose bodies recompute values that depend only on user
parameters (not the per-iteration input `p` or the per-iteration
transform weight). Moving these to `wgsl_init` removes the work from
the hot path — same model as `circus`, `modulus`, `spligon`, etc.
that already do this.

- `murl` — three precomputable per-flame values: `c` (rescaled
  `c_user / (power − 1)` when `power ≠ 1`), `p2 = power / 2`, and
  `vp = c + 1`. Upstream cpp stores these in its `Variables` struct
  (`_c`, `_p2`, `_vp`) and the module header on
  [singleton_misc.rs:33-36](../../src/variations/defs/singleton_misc.rs#L33-L36)
  flagged-then-dismissed them as "per-iteration"; only the trig
  follow-ups (`_a`, `_sina`) actually are. 3 init slots, no behavior
  change. See
  [singleton_misc.rs:639-646](../../src/variations/defs/singleton_misc.rs#L639-L646).

---

## Out of scope (defer to other branches)

### Direct-color (DC) port decisions to verify

Two deliberate divergences from upstream C++ in
[dc.rs](../../src/variations/defs/dc.rs) that warrant later review,
especially as the broader DC corpus gets ported:

- **Color from weighted post-variation position.** Both `dc_linear` and
  `dc_bubble` (and likely most DC variations) compute color from
  `weight * unweighted_output` to mimic the C++ `FPx` accumulator.
  This matches the C++ exactly **only when the DC variation is the
  only normal-phase variation in its transform** (the typical case).
  Mixed with other normal variations, the C++ sums `FPx` across all
  variations before reading it for color — our model uses just the DC
  variation's own contribution. Worth a render comparison on
  mixed-variation transforms once we have more of the DC corpus
  ported.

- **dc_bubble follows JWildfire Java, not the C++ port.** The
  Chaotica/JWildfire C++ port has an apparent porter typo
  (`FPx += FPx + r4_1 * FTx;` doubles FPx instead of incrementing
  once); the C++ Z formula has the same bug. We follow the
  single-add Java original on XY and pass Z through unchanged. Both
  decisions deliberate — flag if a flame imported from a C++-based
  Apophysis renders visibly different from the same flame in
  JWildfire.

When more `dc_*` variations land, apply the same logic and add to
this entry rather than spawning new ones.

### cpp-vs-Java port divergences — recurring pattern

**Five variations confirmed so far** all show the same family of cpp/Java divergence: the upstream cpp port swapped the order of `atan2` arguments (or equivalently, swapped the output X/Y components) relative to JWildfire's Java. Each renders mirrored across `y = x` (or with some additional sign/parameter twist) relative to the Java equivalent. We follow cpp; JWildfire flame import would need affine pre-compose or per-variation in-body swaps to render imported flames as the artist intended.

The affected entries below: `power`, `nPolar`, `flower_db`, `swirl3`, `rings`. Each has its own per-variation analysis since the downstream effect varies (some involve parameter sign flips or radius-modulation reshaping that aren't pure coordinate swaps). If we add a sixth, time to consolidate into a single section.

### `power` follows cpp (Apophysis), not Java (JWildfire)

The standard `power` variation has two upstream forms:

- **Java (JWildfire):** exponent is `sinA = y/r`, output is `(r·cosA, r·sinA) = pow(r, y/r) · (x/r, y/r)`.
- **cpp (Chaotica / Apophysis ports):** exponent is `cosA = x/r`, output is `(r·sinA, r·cosA) = pow(r, x/r) · (y/r, x/r)`.

These collapse to a single identity: **`cpp_power(x, y) ≡ java_power(y, x)`** — i.e. cpp's output is Java's output with the input transposed across the diagonal `y = x`. Visually this is a mirror, not a rotation, and the angular structure of the output really does differ between the two (because the exponent itself depends on angle). We follow cpp; see [apo_misc.rs](../../src/variations/defs/apo_misc.rs).

Worth flagging once we add JWildfire flame import — any imported flame whose intended look came from Java's `power` will render mirrored unless we either (a) pre-compose a `y↔x` swap into the affine that feeds the variation (doesn't compose cleanly when multiple variations share a transform), or (b) add a per-variation in-body input swap gated on an import-source flag. Either path is a real change, so make the decision deliberately rather than just "do what JWildfire does."

### `nPolar` follows cpp (Apophysis), not Java (JWildfire) — even-parity branch only

`nPolar` has the same flavor of cpp-vs-Java divergence as `power`, but the divergence only manifests when `|parity|` is even (the log-polar mid-step branch). The cpp uses `atan2(FTx, FTy)` in two places where the Java uses `atan2(FTy, FTx)` — see [apo_misc7.rs:475](../../src/variations/defs/apo_misc7.rs#L475) (intermediate `x` in the even branch) and [apo_misc7.rs:491](../../src/variations/defs/apo_misc7.rs#L491) (closing `fy`).

Using `atan2(X, Y) = π/2 − atan2(Y, X)` and the variation's `vvar = w/π` factor, each cpp atan2 site collapses to **`cpp_value = w/2 − java_value`** — an additive shift *and* a sign flip relative to Java. And because the intermediate `x` feeds the downstream `angle = atan2(y, x) / nnz` and `radial = pow(x²+y², cn)` calculations, the reflection cascades through the rest of the body. The odd-parity branch (which keeps the input in cartesian and skips the log-polar mid-step) is unaffected.

Same remediation options as `power` if we add JWildfire imports — affine pre-compose vs in-body swap gated on import-source flag — with the additional wrinkle that the swap is only needed when `|parity|` is even, so the body-level fix is conditional.

### `flower_db` follows cpp (Apophysis), not Java (JWildfire)

Same pattern again — cpp uses `atan2(FTx, FTy)` swapped from Java's `atan2(FTy, FTx)` at the angle-extraction step. See [apo_misc9.rs:273](../../src/variations/defs/apo_misc9.rs#L273).

The downstream uses two effects rather than one:

1. **Output XY swap.** With `t_cpp = π/2 − t_java`, `sin(t_cpp) = cos(t_java)` and `cos(t_cpp) = sin(t_java)`. The output lines `(sin(t)·r, cos(t)·r)` therefore evaluate to `(x, y)·(r/|r_xy|)` in cpp but `(y, x)·(r/|r_xy|)` in Java — i.e., Java's XY output is transposed relative to cpp's. Visible diagonal-mirror.
2. **`r` modulation differs non-trivially.** The radius modulator `|spread + sin(petals·t)| · cos(petal_split·petals·t)` evaluates `sin/cos` at `petals · t`, which under the π/2 shift becomes `sin(petals·π/2 − petals·t_java)` — a function of `petals` parity that mixes sin and cos. Unlike the output swap, this isn't a simple coordinate exchange; the petal pattern itself reshapes when ported.

Same JWildfire-import remediation question as `power`/`nPolar` (affine pre-compose vs in-body swap). The radius-modulation effect means even fixing the angle won't perfectly match Java for arbitrary `petals` — but for integer `petals` the result is still recognizably the same flower with a rotation/reflection.

### `swirl3` follows cpp (Apophysis), not Java (JWildfire)

Same pattern, fourth case in this codebase. The cpp uses `atan2(FTx, FTy)` in [apo_misc11.rs:66](../../src/variations/defs/apo_misc11.rs#L66), swapped from Java's `atan2(FTy, FTx)`.

Working through the substitution `atan2(X, Y) = π/2 − atan2(Y, X)`:

- **Java:** `ang_java = θ_java + log(r) · shift`, output is `r · (cos ang_java, sin ang_java)`.
- **cpp:** `ang_cpp = π/2 − θ_java + log(r) · shift`. Then `cos(ang_cpp) = sin(θ_java − log(r) · shift)` and `sin(ang_cpp) = cos(θ_java − log(r) · shift)`.

So cpp emits `r · (sin(θ_java − log(r)·shift), cos(θ_java − log(r)·shift))` while Java emits `r · (cos(θ_java + log(r)·shift), sin(θ_java + log(r)·shift))`. Two changes combined:

1. **Output XY swap** (sin ↔ cos in the output components).
2. **Effective `shift` sign flip** (`+ log(r)·shift` becomes `− log(r)·shift` inside the trig functions).

So an imported Java flame would need both the affine pre-compose (or in-body swap) and a sign-flip on the `shift` parameter to look identical under cpp semantics. Same family of remediation question as the other entries.

### `rings` follows cpp (Apophysis), not Java (JWildfire)

Simplest of the cluster. The body computes `r0 = sqrt(x²+y²)`, then a modular `r`, then emits the output:

- **Java:** `(r · cosA, r · sinA) = r · (x/r0, y/r0)`.
- **cpp:** swapped — `(r · sinA, r · cosA) = r · (y/r0, x/r0)`.

Pure diagonal mirror, no parameter-sign flips or radius-modulation effects. An imported Java flame using `rings` would need only the affine pre-compose (or in-body XY swap) to render correctly. See [apo_misc12.rs:71](../../src/variations/defs/apo_misc12.rs#L71).

### Zero-weight variations should still count as "present"

A variation with weight 0 is currently treated as if it doesn't exist
in some code paths:

- **Animation system**: zero-weight variations don't appear in the
  target list, so they can't be picked as animation targets.
- **Shader builder** (suspected, needs verification): the generated
  WGSL may skip emitting calls for zero-weight variations, which
  means animating the weight up from zero wouldn't take effect on the
  fly.

The intended contract: **if a variation is part of a flame, plan on
it being used.** A weight of 0 is a valid resting state — the user
may want to animate to/from it, or set it conditionally — and
shouldn't make the variation invisible to the rest of the pipeline.

Investigate both call sites; either bring the behavior in line with
"present means used" or document the cases where dropping zero-weight
variations is intentional (likely none, but worth confirming before
ripping the optimization out).

### Stray `weight: f32` parameter in some WGSL bodies — why does it work?

Several 3D variations declare their WGSL function with a trailing
`weight: f32` parameter that the shader builder doesn't pass:

- All three in [depth3d.rs](../../src/variations/defs/depth3d.rs)
  (`zcone`, `flatten`, `zscale`).
- All four in [rotation3d.rs](../../src/variations/defs/rotation3d.rs)
  (`pre_rotate_x/y`, `post_rotate_x/y`) — confirmed working in
  practice despite the apparent signature mismatch.

Per the signature contract in
[VARIATIONS_WIRE_FORMAT.md §4](VARIATIONS_WIRE_FORMAT.md), with
`parameters: &[]`, `needs_rng: false`, `needs_transform: false` (or
true with `(xform_id, variation_id)` already covering it),
`writes_color: false`, `needs_accum: false`, the only argument should
be `p: vec3<f32>`. The extra `weight: f32` shouldn't link — but
rotation3d demonstrably renders correctly.

Possibilities:
- WGSL is silently tolerant of an unused trailing parameter in
  declarations the caller doesn't reference.
- The shader builder has special-case handling we haven't traced.
- The function is being inlined/elided before the linker sees the
  mismatch.

Investigate, then either remove the stale `weight: f32` parameters
from the WGSL bodies (they're unused even where they appear), or
document the actual mechanism. Correctness/cleanup task, not a
metadata task.

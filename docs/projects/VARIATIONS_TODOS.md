# Variations — Running TODO List

Notes worth keeping but not addressing right now. Accumulated as we work
through the bulk metadata review (Phase 4 of
[VARIATIONS_BULK_METADATA_IMPORT.md](VARIATIONS_BULK_METADATA_IMPORT.md)).

Two buckets: things that belong to the metadata-import project itself,
and things we'll defer to other branches when we hit them. Add freely;
prune when something lands.

---

## Related variation project docs

The other active project docs in this directory that touch the
variations system. Completed projects have been moved to
[`docs/archive/`](../archive/).

- [VARIATIONS_BULK_METADATA_IMPORT.md](VARIATIONS_BULK_METADATA_IMPORT.md)
  — the parent project plan for this branch (bulk doc-comments + per-param
  descriptions + author attribution). Most phases complete; this TODO doc
  is the deferred-work residue.
- [VARIATIONS_WIRE_FORMAT.md](VARIATIONS_WIRE_FORMAT.md) — client/API
  wire contract for variations served from the fractalsforall API. Pure
  reference; update if you change either side.
- [variation-bulk-port.md](variation-bulk-port.md) — the long-tail
  porting log (JWildfire/Chaotica → our registry). ~491 ported as of
  the most recent batch; ~67 candidates remain.
- [variation-port-blockers.md](variation-port-blockers.md) — numbered
  index of architectural blockers that prevent specific variations from
  being ported. Cross-referenced from several entries below.
- [subflames.md](subflames.md) — architecture reference for the
  `subflame_wf` variation (v1 shipped). Subflame v2 (real param/state
  slots for variations inside subflames) shipped 2026-05-17 — see
  [`docs/archive/subflame-variations-v2.md`](../archive/subflame-variations-v2.md).

Run `python scripts/audit_variations.py` to sanity-check the corpus
(defined/registered/documented counts + author normalization). Catches
drift introduced by new variation additions.

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
- `pre_spin_z` ([spin_phase.rs](../../src/variations/defs/spin_phase.rs))
- `post_spin_z` ([spin_phase.rs](../../src/variations/defs/spin_phase.rs))
- `pre_blur3D` ([apo_misc15.rs](../../src/variations/defs/apo_misc15.rs))
- `spirograph3D` ([apo_misc17.rs](../../src/variations/defs/apo_misc17.rs))
- `glynnlissa` ([glynnlissa_misc.rs](../../src/variations/defs/glynnlissa_misc.rs))
- `glynnspiro` ([glynnspiro_misc.rs](../../src/variations/defs/glynnspiro_misc.rs))
- `glynnSShape` ([glynnsshape_misc.rs](../../src/variations/defs/glynnsshape_misc.rs))
- `xheart_blur_wf` ([apo_misc18.rs](../../src/variations/defs/apo_misc18.rs))
- `circleLinear` ([apo_misc19.rs](../../src/variations/defs/apo_misc19.rs))
- `cannabiscurve_wf` ([apo_misc20.rs](../../src/variations/defs/apo_misc20.rs)) — curve documented by Eric W. Weisstein at [mathworld.wolfram.com](https://mathworld.wolfram.com/CannabisCurve.html); WF implementation author unknown
- `spherical3D_wf` ([apo_misc20.rs](../../src/variations/defs/apo_misc20.rs))
- `swirl3D_wf` ([apo_misc20.rs](../../src/variations/defs/apo_misc20.rs))
- `heart_wf` ([apo_misc21.rs](../../src/variations/defs/apo_misc21.rs))
- `post_ztranslate_wf` ([apo_misc21.rs](../../src/variations/defs/apo_misc21.rs))
- `post_mirror_wf` ([apo_misc21.rs](../../src/variations/defs/apo_misc21.rs))
- `dc_carpet` ([apo_misc22.rs](../../src/variations/defs/apo_misc22.rs))
- `post_point_symmetry_wf` ([apo_misc22.rs](../../src/variations/defs/apo_misc22.rs))
- `epispiral_wf` ([wf_curves.rs](../../src/variations/defs/wf_curves.rs))
- `cloverleaf_wf` ([wf_curves.rs](../../src/variations/defs/wf_curves.rs))
- `rose_wf` ([wf_curves.rs](../../src/variations/defs/wf_curves.rs))
- `bubble_wf` ([wf_curves.rs](../../src/variations/defs/wf_curves.rs))
- `dinis_surface_wf` ([waves_wf_family.rs](../../src/variations/defs/waves_wf_family.rs)) — surface documented at [mathworld.wolfram.com/DinisSurface](https://mathworld.wolfram.com/DinisSurface.html); WF implementation author unknown
- `dc_triangle` ([dc_misc.rs](../../src/variations/defs/dc_misc.rs))
- `dc_cube` ([dc_misc2.rs](../../src/variations/defs/dc_misc2.rs))
- `pre_rect_wf` ([dc_misc2.rs](../../src/variations/defs/dc_misc2.rs))
- `post_axis_symmetry_wf` ([post_axis_symmetry_misc.rs](../../src/variations/defs/post_axis_symmetry_misc.rs))
- `pre_wave3D_wf` ([pre_wave3d_misc.rs](../../src/variations/defs/pre_wave3d_misc.rs))
- `circleRand` ([circle_rand_misc.rs](../../src/variations/defs/circle_rand_misc.rs))
- `CircleTrans1` ([circle_rand_misc.rs](../../src/variations/defs/circle_rand_misc.rs))
- `waveblur_wf` ([waveblur_misc.rs](../../src/variations/defs/waveblur_misc.rs))
- `macmillan` ([macmillan_misc.rs](../../src/variations/defs/macmillan_misc.rs)) — McMillan map itself is from Edwin McMillan's 1971 nonlinear-dynamics work; variation porter unknown

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
  Safe to automate.
- `Integer [0, N-1]` → `Enum` with N variants: round to nearest, clamp
  to `[0, N-1]`, look up by index. Also safe to automate.

**Status:**
- **35 binary `Integer [0, 1]` → `Boolean`** conversions landed in
  commit `4aa4290` (full list in `scripts/convert_int_to_bool.py`).
- **10 multi-mode `Integer [0, N-1]` → `Enum`** conversions landed in
  commit `567e860` (full list in `scripts/convert_int_to_enum.py`):
  `falloff2.type` + pre/post variants, `atan.mode`,
  `post_axis_symmetry_wf.axis`, `pre_wave3D_wf.axis`,
  `mobius_strip.width_mode/radial_mode`, `spirograph3D.mode`,
  `klein_group.recipe`.
- Description cleanup (drop now-redundant "0 = X, 1 = Y" enumerations)
  landed in `scripts/cleanup_enum_bool_descriptions.py`.

Wire format unchanged across all of these — values still serialize as
`f32`; only the UI control changed. The remaining work splits into the
two buckets below.

#### Enum (`Integer [0, N-1]` → `Enum`) — needs design decisions

These weren't included in the mechanical batch because each has a
question that needs answering before conversion.

- `hole2.shape` — 10 distinct radial-formula branches (shape 0-9).
  Mechanically convertible, just needs short semantic labels per
  shape (read the WGSL body to name each formula). See
  [standalone_exotics.rs](../../src/variations/defs/standalone_exotics.rs).
- `iconattractor_js.preset_id` — 17-mode selector for Field &
  Golubitsky's symmetric-icon preset table. Same shape as `hole2`:
  mechanically convertible, but labels would need shape descriptors
  per preset (read the WGSL or the original paper). See
  [iconattractor_misc.rs](../../src/variations/defs/iconattractor_misc.rs).
- `butterfly_fay.outer_mode`, `butterfly_fay.inner_mode` — 6-mode
  output-formula selector. Shares the spread-formula family with
  `rhodonea.inner_mode/outer_mode` (modes 0-4 match exactly); best
  done together for consistent labels. See
  [butterfly_fay_misc.rs](../../src/variations/defs/butterfly_fay_misc.rs).
- `rhodonea.inner_mode`, `rhodonea.outer_mode` — 7-mode spread/mask
  behavior selector. Same 0-4 spread family as `butterfly_fay`; modes
  5/6 are mask hide/pass-through with **inverted semantics between
  inner and outer** (5 hides for inner / passes for outer; 6 passes
  for inner / hides for outer). Need two separate enum variant lists
  to label each correctly. See
  [rhodonea_misc.rs](../../src/variations/defs/rhodonea_misc.rs).
- `jac_asn.jac_asn_type` — 8-mode selector that's really
  `2 × 4` (function-kind × swap-modulus-and-phi). Decision: convert
  to a flat 8-variant enum to match the wire format, or split into
  two separate params (a 4-variant enum + a Boolean)? Splitting is
  cleaner UX but changes the param list — would need a legacy_import
  shim. See
  [jac_asn_misc.rs](../../src/variations/defs/jac_asn_misc.rs).
- `subflame_wf.color_mode` — declared `Integer` with `[-1, 4]` range,
  6-mode color-handling selector: -1 = Off (default), 0 = Direct
  (overwrite parent's `vc` with subflame's color), 1-4 = JWildfire's
  CM_RED/GREEN/BLUE/BRIGHTNESS modes. Two issues: the `-1` baseline
  is awkward for `Enum` (which expects `[0, N-1]`); and modes 1-4 are
  currently declared but **silently no-op'd** (only Off and Direct
  are implemented). Either finish the port for 1-4 or drop them from
  the range. Remapping to `0 = Off, 1 = Direct, ...` would change the
  wire format and need a legacy_import shim. See
  [subflame.rs](../../src/variations/defs/subflame.rs).

#### Float-threshold dispatch — keep as Float (or add legacy_import shim)

These don't cleanly fit an `Enum` because the float value space carries
information beyond just selecting a mode. A naive enum mapping (e.g.
`0/1/2`) round-trips fine for newly-saved values, but an imported flame
with a value like `zero = 0.7` (currently meaning "collapse to origin"
via threshold dispatch) has no slot under enum semantics. Decision
pending: either keep these as `Float` with documented thresholds, or
add a per-parameter `legacy_import` hook that reproduces the threshold
dispatch when reading legacy XML.

- `hypercrop.zero` — declared `unlimited_float` with `[0, 2]` range,
  dispatched as 3 modes via threshold comparisons (`> 1.5` snap to
  corner, `> 0.5` collapse to origin, else scatter). See
  [maurer_hyper.rs](../../src/variations/defs/maurer_hyper.rs).
- `tile_reverse.reversal` — declared `unlimited_float` with `[-10, 10]`
  range, but only checked for `== 1.0` vs anything else (binary mirror
  toggle). Type is misleading; semantics are Boolean — but the float
  range is preserved in the wire format. See
  [misc_extras.rs](../../src/variations/defs/misc_extras.rs).
- `cpow3_wf.discrete_spread` — declared `unlimited_float` with
  `[0, 1]` range, but only checked `>= 1.0` (binary toggle between
  discrete-integer and continuous angular branches). Same pattern as
  `tile_reverse.reversal`. See
  [apo_misc22.rs](../../src/variations/defs/apo_misc22.rs).
- `waves2b.pwx`, `waves2b.pwy` — declared `unlimited_float` but
  semantically tri-state via threshold comparisons: `pw ∈ [0, 1e-4)`
  → Jacobi `sn` mode, `pw ∈ (-1e-4, 0)` → Bessel `J1` mode, else →
  power-mode sine with `pw` as the actual exponent. The "power-mode"
  arm uses `pw` as a continuous parameter, so the value space cannot
  cleanly split into an `(enum mode, float power)` pair without
  losing the smooth handoff at the boundaries. See
  [waves2b_misc.rs](../../src/variations/defs/waves2b_misc.rs).
- `hexaplay3D.majp`, `hexnix3D.majp` — both declared
  `unlimited_float` but dispatch on `|majp|` thresholds, with the
  value above the threshold also feeding a continuous `boost` term.
  `hexaplay3D` is two-mode (`≤ 1` single plate, `> 1` split planes
  with `boost = (|majp| - 1) · 0.5`); `hexnix3D` is three-mode
  (`≤ 1`, `1-2`, `≥ 2`) plus additional negative-`majp` branches
  for animation Z-flips. See
  [hexaplay3d_misc.rs](../../src/variations/defs/hexaplay3d_misc.rs)
  and
  [hexnix3d_misc.rs](../../src/variations/defs/hexnix3d_misc.rs).

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

- `affine3D` — six precomputable per-flame values: `sin(rx·π/180)`,
  `cos(rx·π/180)`, and likewise for `ry` and `rz`. Upstream cpp
  caches all six in `_sinX/cosX/sinY/cosY/sinZ/cosZ`; our port
  skipped this because the historical 16-slot per-variation budget
  couldn't fit them alongside the 15 user params. That budget is
  gone — the shader builder now packs variation params contiguously
  with no fixed stride (see
  [shader_builder_v2.rs:799-842](../../src/shader_builder_v2.rs#L799-L842)),
  so 6 init slots cost only what they cost. Worth revisiting now
  that the constraint is lifted. See
  [affine3d_misc.rs:10-14](../../src/variations/defs/affine3d_misc.rs#L10-L14)
  for the original justification.

  More broadly: any variation whose module header cites the
  "16-slot budget" as the reason for inlining derived values is a
  candidate for the same revisit — `maurer_hyper.rs`,
  `stub_recoveries2.rs`, `sosa_attractors2.rs`, `apo_misc22.rs`,
  and `parametric_curves.rs` all mention it as of this commit.

- `murl` — three precomputable per-flame values: `c` (rescaled
  `c_user / (power − 1)` when `power ≠ 1`), `p2 = power / 2`, and
  `vp = c + 1`. Upstream cpp stores these in its `Variables` struct
  (`_c`, `_p2`, `_vp`) and the module header on
  [singleton_misc.rs:33-36](../../src/variations/defs/singleton_misc.rs#L33-L36)
  flagged-then-dismissed them as "per-iteration"; only the trig
  follow-ups (`_a`, `_sina`) actually are. 3 init slots, no behavior
  change. See
  [singleton_misc.rs:639-646](../../src/variations/defs/singleton_misc.rs#L639-L646).

### Declared-but-unused user parameters

Parameters listed in `parameters: &[...]` that the WGSL body never
reads. Two subcategories:

**Intentional (kept for cpp/Java interface parity):**

- `harmonograph_js.seed` — upstream cpp re-seeds `GOODRAND` per
  Prepare; we use the per-thread RNG, so the seed value has no
  effect. Kept so `.flame` XML round-trips and so users importing
  from JWildfire don't see a "missing param" warning. See
  [harmonograph_misc.rs:13-15](../../src/variations/defs/harmonograph_misc.rs#L13-L15).

(For DC variations whose params are kept-for-parity because the
color-write side is dropped — `dc_carpet3D.color_a..color_f`,
`scale_z`, `reset_z`, `origin` — see the *Direct-color (DC) port
decisions* section below rather than duplicating here.)

**Possibly port omissions (needs upstream verification):**

- `macmillan.N` — the body hardcodes exactly two iterations of the
  McMillan map per call; `N` is declared (`unlimited_float [-10, 10]`,
  default `1.0`) but never read. Upstream cpp may use it as an inner-
  loop bound (`for (i = 0; i < N; i++) { ... }`) that the Rust port
  collapsed. Verify against
  `output/jwildfire-vars/output/macmillan.cpp` and either rewire it
  or drop the param. See
  [macmillan_misc.rs:36-42](../../src/variations/defs/macmillan_misc.rs#L36-L42).

---

## Out of scope (defer to other branches)

### Prepost (priority-2) variations — needs architectural decision

Reverted the single-phase compromise port of `prepost_circlize` and
`prepost_mobius` (removed
`src/variations/defs/prepost_compromise.rs` and the corresponding
smoke test). The compromise ran just the post-half as a normal-phase
variation, which collapses to the same behavior as plain `circlize` /
`mobius` (neither of which exist yet either) — the entire point of
the "prepost" family is the **sandwich**: a pre-phase warp and a
post-phase inverse-warp bracketing the rest of the iteration.

To port these properly we need a new `VariationPhase::PrePost` that
takes two WGSL bodies (pre and post) and registers them in both phase
slots of the iteration pipeline. The pre-phase body typically does
the inverse of the post-phase body, so coordinates flow:

```
input → pre_affine → PREPOST.pre_body → normal_variations →
        PREPOST.post_body → post_affine → output
```

Upstream JWildfire candidates (from
[variation-upstream-only.txt](variation-upstream-only.txt) and
[variation-port-blockers.md](variation-port-blockers.md) blocker
\#12): `prepost_affine`, `prepost_circlize`, `prepost_mobius`. The
research task: enumerate JWildfire's full `prepost_*` corpus, check
which ones are popular in real flames (Apophysis import frequency,
JWildfire community presets), and decide if the user-visible payoff
justifies the phase-system plumbing. If yes, add the
`VariationPhase::PrePost` variant and port them properly. If no,
document the family as permanently skipped and move on.

### Direct-color (DC) port roadmap — what "fully ported" requires

The DC story is split across several docs and per-file headers. This
section is the index — points at where the open work lives and
distinguishes infrastructure that exists-but-isn't-used from infra
that's actually missing.

**What works today:**

- `writes_color: true` variations get a `vc` color-register pointer
  passed by the shader builder (see
  [shader_builder_v2.rs:1272-1340](../../src/shader_builder_v2.rs#L1272-L1340)).
  The *write* side is fully wired up. `dc_linear`, `dc_bubble`,
  `macmillan` and others use this today.
- `needs_accum` + per-thread state (resolved 2026-05-04) cover the
  common pattern of computing TC from `FPx + FPy` after the
  variation's own contribution. See blocker #5 in
  [variation-port-blockers.md](variation-port-blockers.md).

**What's missing — needs architectural work** (each is a blocker in
[variation-port-blockers.md](variation-port-blockers.md)):

- **#5 — TC reads in the spatial path.** Variations that read the
  current point's color value to drive *spatial* output (e.g.
  `dc_ztransl` reads TC for Z displacement). Distinct from computing
  TC from an accumulator, which works.
- **#11 — Color-write coupled to spatial output.** Variations where
  color *is* the spatial differentiator (e.g. `dc_carpet3D`'s
  `dz = color · scale_z + offset_z`). Without a write-then-read color
  pipeline the spatial output collapses to a constant or to linear/blur.
- **#8 — `DC_BaseFunc` + 31 derivatives.** Unlocked once #11 lands;
  without DC writes the derivatives don't gain spatial distinctiveness
  over plain linear/blur.

**What's missing — per-variation grunt work:**

- Variations ported with `writes_color: false` as a deliberate
  compromise: the geometry was ported but the color write was dropped
  even though the infrastructure would support it. Examples include
  `dc_carpet3D` and the `direct_color` toggle cases in `truchet`,
  `truchet2`, and `waveblur_wf`. Most of these would just need the
  flag flipped + the color body added once they get attention. The
  carpet-style cases (spatial output also depends on reading the color
  back) are gated by #11 above, not the write itself.

Per-variation interface-parity decisions (color-from-position
approximations, Java-vs-C++ porter-bug choices, parity-only param
retention) live in the section below.

### Direct-color (DC) port decisions to verify

Deliberate divergences from upstream C++ in
[dc.rs](../../src/variations/defs/dc.rs) and related DC files that
warrant later review, especially as the broader DC corpus gets ported:

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

- **`dc_carpet3D`: color-coupled Z dropped, ancillary params kept for
  parity.** Upstream couples Z output to color (`dz = color · scale_z +
  offset_z`, then `z = dz` or `z += dz`); a `reset_z` flag optionally
  re-randomizes Z from the same color-driven term. With color writes
  dropped, the entire color→Z pipeline collapses, so the Rust port
  emits just `z + w · offset_z` (constant Z bump) and the
  `color_a..color_f`, `scale_z`, `reset_z`, and `origin` params are
  retained only for interface parity (they're declared but ignored in
  the body). If/when DC writes get a proper port, this needs to be
  revisited — the color-coupled Z is presumably the variation's
  defining 3D feature. See
  [dc_carpet3d_misc.rs](../../src/variations/defs/dc_carpet3d_misc.rs).

When more `dc_*` variations land, apply the same logic and add to
this entry rather than spawning new ones.

### cpp-vs-Java port divergences (atan2-swap family)

Eleven variations so far show the same family of divergence: the
upstream cpp port swapped the order of `atan2` arguments (or, in
`power`'s case, swapped sin↔cos directly in the output) relative to
JWildfire's Java. Mechanically this collapses via the identity
`atan2(X, Y) = π/2 − atan2(Y, X)`. Each variation renders mirrored
across `y = x` (sometimes with additional sign/parameter twists)
relative to the Java equivalent. We follow cpp throughout.

The per-variation forms — keep these so we can match Java's output
exactly if we ever need to:

- **`power`** ([apo_misc.rs](../../src/variations/defs/apo_misc.rs))
  — exponent and output components both swapped. Collapses to
  **`cpp_power(x, y) ≡ java_power(y, x)`** — a clean diagonal mirror
  of the input. Angular structure differs (because the exponent
  itself depends on angle), so it's a real mirror, not just a rotation.

- **`nPolar`** ([apo_misc7.rs:475](../../src/variations/defs/apo_misc7.rs#L475),
  [:491](../../src/variations/defs/apo_misc7.rs#L491)) — atan2 swap
  in two places, **`|parity|` even branch only**. With the variation's
  `vvar = w/π` factor, each cpp atan2 site collapses to
  **`cpp_value = w/2 − java_value`** (additive shift + sign flip).
  The reflection cascades through `angle = atan2(y, x) / nnz` and
  `radial = pow(x²+y², cn)`. Odd-parity branch (cartesian, skips
  log-polar mid-step) is unaffected.

- **`flower_db`** ([apo_misc9.rs:273](../../src/variations/defs/apo_misc9.rs#L273))
  — two compounded effects: (1) output XY swap from `sin(t_cpp) =
  cos(t_java)` etc., and (2) the radius modulator `|spread +
  sin(petals·t)| · cos(petal_split·petals·t)` reshapes
  non-trivially because `sin(petals·(π/2 − t_java))` mixes sin and
  cos depending on `petals` parity. The petal pattern itself
  changes when ported, not just its orientation.

- **`swirl3`** ([apo_misc11.rs:66](../../src/variations/defs/apo_misc11.rs#L66))
  — two effects: (1) output XY swap (sin ↔ cos), and (2) effective
  `shift` parameter sign flip: cpp emits `r · (sin(θ_java − log(r)·shift),
  cos(θ_java − log(r)·shift))` vs Java's
  `r · (cos(θ_java + log(r)·shift), sin(θ_java + log(r)·shift))`.

- **`rings`** ([apo_misc12.rs:71](../../src/variations/defs/apo_misc12.rs#L71))
  — simplest. Direct output swap: Java emits `r · (x/r0, y/r0)`, cpp
  emits `r · (y/r0, x/r0)`. Pure diagonal mirror.

- **`pre_disc3d`** ([spin_phase.rs:178](../../src/variations/defs/spin_phase.rs#L178),
  [:190](../../src/variations/defs/spin_phase.rs#L190))
  — cpp's `vv = w · atan2(x, y) / pi` becomes `vv = w/2 − java_vv`
  (additive shift + sign flip). Because `vv` multiplies all three
  output components (`vv · sr, vv · cr, vv · r · cos(z)`), the
  whole-output magnitude differs, not just the angular direction.

- **`swirl3D_wf`** ([apo_misc20.rs:176](../../src/variations/defs/apo_misc20.rs#L176),
  [:185](../../src/variations/defs/apo_misc20.rs#L185))
  — cpp's `ang = atan2(x, y)` gives output XY swap (`(rad·sin θ_java,
  rad·cos θ_java)` vs Java's identity `(rad·cos θ_java, rad·sin θ_java) =
  (x, y)`). The Z output `sin(6·cos(rad) − n·ang)` picks up both a
  sign flip on the `n·θ` term and an additive `−n·π/2` shift inside
  the sine, so the Z modulation is genuinely different (not just
  rotated).

- **`cpow3_wf`** ([apo_misc22.rs:230](../../src/variations/defs/apo_misc22.rs#L230),
  [:264](../../src/variations/defs/apo_misc22.rs#L264))
  — cpp's `ai = atan2(x, y)` cascades through a branch (`if ai <
  0.0: n += 1.0`), so the π/2 shift changes which iterations get
  `n + 1` — that's a *branch-selection* divergence, not just a
  coordinate flip. The shifted `ai` then feeds the radial term
  `ri = exp(half_c · lnr2 − d · ai)` and the angular output
  `ang2 = c · ai · half_d · lnr2 · ang · (...)`, so both the
  radius and angle of the output differ from Java's.

- **`epispiral_wf`, `cloverleaf_wf`, `rose_wf`** (three at once;
  see [wf_curves.rs](../../src/variations/defs/wf_curves.rs)) — all
  three are polar curves of the form `r = f(a)` with output `(sin a ·
  r, cos a · r)`, where cpp uses `a = atan2(x, y)` (swapped from
  Java's `atan2(y, x)`). The output angular direction matches Java's
  thanks to the `sin/cos` swap cancelling out, but the radius is
  evaluated at `π/2 − a_java` instead of `a_java`. For polar curves
  this is a reflection across the π/4 line; the exact visible effect
  depends on `f`. For rose-style `cos(waves · a)` the result is a
  rotation by `waves · π/2` (a different orientation per integer
  `waves` parity).

**Remediation if we add JWildfire flame import:** two general
options. Either (a) pre-compose a `y↔x` swap into the affine that
feeds the variation — doesn't compose cleanly when multiple
variations share a transform — or (b) add a per-variation in-body
input swap gated on an import-source flag. For variations with
additional parameter-side effects (`swirl3`'s sign flip,
`flower_db`'s radius reshape, `pre_disc3d`'s magnitude shift), the
in-body fix has to handle those too — not just the coordinate swap.
Make the decision deliberately rather than just "do what JWildfire
does."

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

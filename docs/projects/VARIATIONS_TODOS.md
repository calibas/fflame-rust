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
- `invsquircular` ([apo_misc11.rs](../../src/variations/defs/apo_misc11.rs))
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
- **16 multi-mode `Integer [0, N-1]` → `Enum`** conversions landed in
  commits `567e860` and follow-ups (full list in
  `scripts/convert_int_to_enum.py`): `falloff2.type` + pre/post
  variants, `atan.mode`, `post_axis_symmetry_wf.axis`,
  `pre_wave3D_wf.axis`, `mobius_strip.width_mode/radial_mode`,
  `spirograph3D.mode`, `klein_group.recipe`, `hole2.shape`,
  `butterfly_fay.outer_mode/inner_mode`,
  `rhodonea.outer_mode/inner_mode`, `jac_asn.jac_asn_type`.
- Description cleanup (drop now-redundant "0 = X, 1 = Y" enumerations)
  landed in `scripts/cleanup_enum_bool_descriptions.py`.

Wire format unchanged across all of these — values still serialize as
`f32`; only the UI control changed. The remaining work splits into the
two buckets below.

#### Enum candidates — deferred (not urgent)

Wire format is unchanged after the Phase 3 conversions, so old
configs round-trip cleanly through both the Boolean and Enum cases
that landed. The two remaining candidates aren't urgent — they're
both Integer sliders today, which is functional. Revisit when the
underlying questions get answered.

- `iconattractor_js.preset_id` — 17-mode selector for Field &
  Golubitsky's symmetric-icon preset table. Blocker: the WGSL preset
  table is just raw `(degree, a, b, g, o, l)` tuples with no
  per-preset comments, so labels would have to come from rendering
  each preset and matching against the named figures in *Symmetry in
  Chaos*. Degree-based labels (5 presets share `degree=5`, 6 share
  `degree=3`) wouldn't disambiguate enough to be worth the
  conversion. Plain numeric `Preset 0..16` (still as Integer) is
  honestly more useful than a dropdown of `D5 #1, D5 #2, …`.
  Revisit once someone renders the 17 presets and picks book-derived
  names. See
  [iconattractor_misc.rs](../../src/variations/defs/iconattractor_misc.rs).
- `subflame_wf.color_mode` — declared `Integer` with `[-1, 4]` range,
  6-mode color-handling selector: -1 = Off (default), 0 = Direct
  (overwrite parent's `vc` with subflame's color), 1-4 = JWildfire's
  CM_RED/GREEN/BLUE/BRIGHTNESS modes. Blockers: the `-1` baseline is
  awkward for `Enum` (which expects `[0, N-1]`); and modes 1-4 are
  currently declared but **silently no-op'd** (only Off and Direct
  are implemented). Revisit when either CM_* gets implemented (and
  the range can stay) or we decide to shrink the range to `[-1, 0]`
  and document the no-op'd modes as removed. See
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
- **`wave3` artifacts** — user-reported visible difference; root cause
  not yet investigated. Likely the same clamp-vs-NaN family. Worth
  pinning down before any architectural decision about a point-discard
  sentinel.

The general guard pattern is probably fine, but it's worth a render
comparison on flames that exercise the edge cases (e.g. `disc3` with
`d = -1, e = 1`; plus reproducing the `wave3` artifact) to confirm
we're not introducing spurious origin artifacts. If we are, we may
need to special-case the `sqrt` clamp (and similar) to propagate a
sentinel that the dispatcher recognizes as "drop this point."

### Init-slot optimization opportunities

Variations whose bodies recompute values that depend only on user
parameters were moved to `wgsl_init` after the packed-variation-params
layout retired the historical 16-slot per-variation ceiling. Same
model as `circus`, `modulus`, `spligon`, etc. that already did this.

**Done** (this branch):

- `affine3D` — 7 init slots: 6 sin/cos of rotation angles +
  `_hasShear`. Matches upstream cpp's `_sinX/cosX/sinY/cosY/sinZ/cosZ`
  + `_hasShear`. ~14 ops/iter saved.
- `murl` — 3 init slots (`_c`, `_p2`, `_vp`). Matches upstream
  `Variables` struct. ~5 ops/iter saved.
- `maurer_rose` — 10 init slots: `k`, `step_size`, `safe_step`,
  `cycles`, three sampling thresholds, three thicknesses. ~12
  ops/iter saved.
- `hypercrop` — 4 init slots (`coef`, `a0`, `len`, `d`). ~6 ops/iter
  saved.

**Audited and skipped** — files that cited the 16-slot budget but
either had no per-iter constants worth moving, were citing overflow
not inlining, or already had appropriate init usage:

- `stub_recoveries2.rs` — the 16-slot mention was about gridout3d's
  26-param overflow (different problem). `disc3`, `projective`,
  `tqmirror` bodies all per-iter (every op depends on `p`).
- `sosa_attractors2.rs` — 16-slot mention is overflow context, not
  inlining. `lorenz_js` has 1 already-dead init slot (`1/scale`,
  for cpp parity); `woggle_js` has marginal candidates (~3 ops/iter,
  not worth a slot).
- `apo_misc22.rs` — `cpow3_wf` already has 7 init slots.
  `dc_carpet` and `post_point_symmetry_wf` have only marginal
  candidates.
- `parametric_curves.rs` — `crop3d` already has 6 init slots; could
  add 3 more (`w/h/l_range = (xmax-xmin)*0.5*scatter`) but the
  savings are ~6 ops/iter for 3 slots — not worth it unless the
  variation becomes a bottleneck.

### Declared-but-unused user parameters

Parameters listed in `parameters: &[...]` that the WGSL body never
reads. All currently in the "intentional / dead upstream too"
category — kept for cpp/Java interface parity so saved flames
round-trip cleanly:

- `harmonograph_js.seed` — upstream cpp re-seeds `GOODRAND` per
  Prepare; we use the per-thread RNG, so the seed value has no
  effect. Kept so `.flame` XML round-trips and so users importing
  from JWildfire don't see a "missing param" warning. See
  [harmonograph_misc.rs:13-15](../../src/variations/defs/harmonograph_misc.rs#L13-L15).
- `macmillan.N` — declared (`unlimited_float [-10, 10]`, default
  `1.0`) but never read. Verified against `output/jwildfire-vars/
  output/macmillan.cpp` and JWildfire's Java `MacMillanFunc.java`:
  **N is dead in upstream too** — both declare it as a `VAR_REAL`
  but neither reads it in `PluginVarCalc`. The body is hardcoded to
  exactly two McMillan iterations regardless of N. We're being
  faithful to upstream — not a port omission. See
  [macmillan_misc.rs:36-42](../../src/variations/defs/macmillan_misc.rs#L36-L42).

(`dc_carpet3D`'s previously-parity-only `color_a..color_f`,
`scale_z`, `reset_z`, and `origin` are now all live as of the
2026-05-29 full port — they drive the color mix and color-coupled
Z output. The remaining `writes_color: false` compromises in
`dc_cube`, `dc_cylinder`, `dc_cylinder2`, `dc_triangle`,
`truchet`, `truchet2`, `waveblur_wf` follow the same pattern;
see the *Direct-color (DC) infrastructure* section below.)

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

### Direct-color (DC) infrastructure — complete

**Status as of 2026-05-29:** the DC infrastructure is feature-complete
for the palette-index color path. The `vc: ptr<function, f32>` parameter
passed to every `writes_color: true` variation is a true read/write
pointer; the same pointer is threaded through every variation call in
an iteration (normal phase and linked-chain). A variation can both
write to `*vc` and read from `*vc` in the same body, and a later
variation in the chain sees the writes from earlier ones. Blockers #5
(TC reads driving spatial output) and #11 (color-write coupled to
spatial output) — previously framed as architectural — turned out to
have been resolved when the `vc` parameter landed; the docs and TODOs
just hadn't caught up. See the [`dc_carpet3D` promotion](../../src/variations/defs/dc_carpet3d_misc.rs)
and [`dc_tc_read`](../../src/variations/defs/dc_tc_read.rs) for
canonical write-then-read and pure-read patterns respectively.

**Completed on the `variations-dc-port` branch (2026-05-29):**

- **Existing compromises promoted to full ports** (geometry was
  ported, color writes had been dropped pending the false belief that
  #5/#11 were architectural):
  - `dc_carpet3D` — proof of #11 (write-then-read in a single body).
  - `dc_cube`, `dc_cylinder`, `dc_cylinder2`, `dc_triangle`.
  - `truchet`, `waveblur_wf` — `direct_color` toggle now wires
    through to actual `*vc` writes.
  - (`truchet2` was originally in this list, but turned out to have
    no color logic in cpp/Java at all — a docs error.)
- **TC-read variations** — previously listed under blocker #5:
  - `dc_ztransl`, `pre_dcztransl`, `colorscale_wf`, `post_colorscale_wf`.

**Remaining DC work — not on this branch:**

- **`DC_BaseFunc` derivatives (~34 variations).** Previously the
  blockers doc framed these as "linear/blur spatial + color body"
  trivial ports gated only on #11. **They aren't trivial.** Each
  derivative is a GLSL-style procedural pattern generator (typically
  100–200 lines of Java + internal helpers, references to
  JWildfire's `js.glsl.G` namespace, time-based animation params,
  occasional infrastructure dependencies — Perlin permutation
  tables for `dc_perlin`, Apollonian-circle recursion for
  `dc_apollonian` (most of that math already lives in
  `shaders/core/complex.wgsl` thanks to Klein group), etc.).
  Realistic per-variation cost: 2–4 hours for simpler ones, more for
  those needing new shader-side primitives.

  There's also a **shader-side RGB-direct path** still missing — the
  base class's `gradient=0` and `gradient=1` modes inject RGB
  directly into `pVarTP.{red,green,blue}Color` and bypass the
  palette. Our color register is `f32` (palette index) only, so
  these modes can't be reproduced without widening the accumulator
  to carry RGB. `gradient=2` (greyscale luminance → palette index)
  is fully supportable today via `*vc`.

  Per-batch porting of the derivatives is genuine separate work,
  not the finishing touch implied by the original blockers framing.

- **Other unported `dc_*` gated by separate blockers:**
  - `dc_crackle_wf`, `dc_cracklep_wf` — Crackle algorithm state (#7).
  - `dc_dmodulus` — `_oldColor` accumulator (#3-style; may already
    be expressible via `needs_accum`, worth a re-audit).
  - `dc_code` — JIT-compiled user expressions (#1, impossible).
  - `dc_hexes_wf` — Voronoi primitive (#7).

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

- **~~`dc_carpet3D`: color-coupled Z dropped~~** — RESOLVED 2026-05-29.
  Promoted to a full port: `writes_color: true`, the color mix uses
  `color_a..color_f` + `origin` (via `H = 0.1 · origin`) + the
  `x0^y0` corner parity, and Z is driven by the freshly-written
  color value (`dz = new_vc · scale_z + offset_z`, with `reset_z`
  controlling override vs accumulate). Empirical proof of #11
  unblock — changing `color_a..color_f` with `direct_color = 0`
  shifts the 3D structure without changing the rendered color. See
  [dc_carpet3d_misc.rs](../../src/variations/defs/dc_carpet3d_misc.rs).

- **`colorscale_wf` reset_z limitation.** Cpp's `reset_z > 0` case
  sets `pVarTP.z = dz` outright, discarding any prior Z
  contribution from other normal-phase variations in the same
  transform. Our `needs_transform` outer-multiplier model can only
  add (`result.z += w · nz`), not override. The port emits `nz =
  dz/w` regardless of `reset_z`, which matches upstream when the
  variation is the only normal-phase Z contributor (the typical
  use). Mixed cases differ from cpp; revisit with `needs_accum` if
  a real flame needs it. `post_colorscale_wf` is unaffected (post
  phase has direct `p.z` access). See
  [dc_tc_read.rs](../../src/variations/defs/dc_tc_read.rs).

- **`colorscale_wf` Java vs cpp TC source.** Java reads
  `pAffineTP.color` (the input color); cpp reads `pVarTP.color`
  (the running color register). These differ only when a prior DC
  variation in the same chain has written to the color register.
  Our `*vc` follows the cpp semantics. Flag if this produces a
  visible diff in a real flame.

When more `dc_*` variations land, apply the same logic and add to
this entry rather than spawning new ones.

### cpp-vs-Java port divergences (atan2-swap family)

Eleven variations were originally flagged because our `chaotica-
apophysis-plugins-from-jwildfire`-derived cpp ports use a different
`atan2` argument order from the corresponding JWildfire Java source.
**Reframing**: the mwegner converter repo isn't a canonical upstream;
it's an automated Java→cpp conversion that sometimes introduces bugs.
The actual canonical references per ecosystem are:

- **Apophysis 7X**: original `.pas` (built-ins) and `.c` (plugin)
  source at https://github.com/xyrus02/apophysis-7x
- **JWildfire**: `<Name>Func.java` source at https://github.com/thargor6/JWildfire

Both ecosystems may legitimately diverge for a given variation, in
which case they're functionally different. The framing for "which is
correct" is: match the canonical source of whichever ecosystem the
variation originated in. For `_wf` variants (JWildfire-side), Java is
canon. For non-`_wf` variants (Apo-side, mostly), Apo 7X is canon.

**Per-variation verdicts:**

#### Verified — matches canon, no change

- **`epispiral_wf`, `cloverleaf_wf`, `rose_wf`** — JWildfire Java
  uses `atan2(x, y)` (via `getPrecalcAtan()`, which returns the
  swapped order). The swap is **deliberate JWildfire convention**;
  our port faithfully matches. Output `(sin a · r, cos a · r)` also
  matches Java. See JWildfire `EpispiralWFFunc.java`,
  `CloverLeafWFFunc.java`, `RoseWFFunc.java`. No fix needed.
- **`nPolar`** — Apo 7X's `npolar.c` uses `atan2(FTx, FTy)`
  (swapped) in both places. The swap is **canonical Apo 7X
  behavior**; our port matches. See `xyrus02/apophysis-7x:
  src/Plugin/npolar.c`. No fix needed.
- **`epispiral`** (plain, not `_wf`) — Apo 7X's
  `varEpispiral.pas` uses standard `atan2(FTy, FTx)`; our port
  matches. Author Joel Faber (added to authors block).
- **`power`** ([apo_misc.rs](../../src/variations/defs/apo_misc.rs))
  — one of Scott Draves's original 27 from the *Fractal Flames*
  paper. flam3's `var19_power` and JWildfire's `PowerFunc.java`
  both use the swapped-angle convention via flam3's
  `precalc_sina = x/r` / `precalc_cosa = y/r` (which are sin/cos
  of `atan2(x, y)`, not the standard angle) and Java's
  equivalent `getPrecalcSinA()` / `getPrecalcCosA()`. Our port
  produces `r^sin(a) · (cos a, sin a)` where `a = atan2(x, y)`,
  matching both upstreams. The TODO's earlier "cpp_power(x,y) ≡
  java_power(y,x)" claim was wrong — the swapped variable names
  in flam3 hide that the underlying convention is identical.
- **`rings`** ([apo_misc12.rs](../../src/variations/defs/apo_misc12.rs))
  — also one of Scott Draves's original 27. flam3's `var21_rings`
  and JWildfire's `RingsFunc.java` both produce output
  `(r · cosA, r · sinA)` where `cosA = y/r0`, `sinA = x/r0` in
  their swapped-angle precalc convention. Our `(r · y/r0,
  r · x/r0)` matches both. The doc-comment's earlier claim about
  "cpp's xy-output swap (Java uses cosA for x, sinA for y; cpp
  swaps)" was misleading: Java does use `cosA` for x, but with
  the swapped precalc `cosA = y/r0`, so Java output equals our
  output. No swap relative to upstream — the "swap" is just the
  canonical math of this variation. Author Scott Draves added to
  the file.
- **`pre_disc3d`** ([spin_phase.rs](../../src/variations/defs/spin_phase.rs))
  — JWildfire-only (no Apo 7X analog). Java's `PreDisc3DFunc`
  calls `atan2(pAffineTP.x, pAffineTP.y)` directly — i.e. `y_arg
  = x`, `x_arg = y`, giving the swapped-angle convention. Our cpp
  port uses the same `atan2(p.x, p.y)`; identical. The
  doc-comment previously claimed Java used `atan2(y, x)`, which
  was wrong. Z output `vv · r · cos(z)` also matches. Author
  gossamer light was already in the file.

#### Fixed — converter bug, matches Java now

- **`swirl3D_wf`** ([apo_misc20.rs](../../src/variations/defs/apo_misc20.rs))
  — JWildfire `Swirl3DWFFunc.java` uses `getPrecalcAtanYX()` (standard
  `atan2(y, x)`), but our cpp port had the swap. **Fixed**: body now
  uses `atan2(p.y, p.x)`. Match verified against Java source.
- **`cpow3_wf`** ([apo_misc22.rs](../../src/variations/defs/apo_misc22.rs))
  — JWildfire `CPow3WFFunc.java` uses `getPrecalcAtanYX()` (standard);
  our cpp port had the swap. **Fixed**: body now uses
  `atan2(p.y, p.x)`. The `if ai < 0: n++` branch selection now
  reflects Java's iteration distribution.
- **`swirl3`** ([apo_misc11.rs](../../src/variations/defs/apo_misc11.rs))
  — Author Zy0rg. Both Fractorium (Ember) source and JWildfire GPU
  code use standard `atan2(y, x)` with `+ log(r)·shift` and
  `(cos, sin)` output. Our cpp port had `atan2(x, y)` (swapped),
  which algebraically equals `(sin(θ - log·shift), cos(θ - log·shift))`
  — the form the TODO previously described. **Fixed**: body uses
  `atan2(p.y, p.x)`; added Zy0rg to authors block.
- **`flower_db`** ([apo_misc9.rs](../../src/variations/defs/apo_misc9.rs))
  — JWildfire `FlowerDbFunc.java` uses `getPrecalcAtanYX()` (standard
  `atan2(y, x)`) per the source embedded in our cpp file; JWildfire's
  GPU code uses `t = __theta` (also standard); our cpp port had the
  swap. **Fixed**: body uses `atan2(p.y, p.x)`. Fractorium has a
  separately-evolved simpler `flowerdb` (3 params, no stem/fold)
  which is functionally a different variation; documented in the
  module comment.

**Resolved.** All originally-flagged variations are now classified
— see the *Verified* and *Fixed* sections above. Net result: 4
converter-bug fixes (`swirl3D_wf`, `cpow3_wf`, `swirl3`,
`flower_db`); the remaining flags were canonical-by-design (either
upstream deliberately uses the swap, or the precalc helpers
already bake it in).

**For variations where Apo 7X and JWildfire genuinely differ** (if
any are discovered in future work), the recommended approach is to
**add a sibling variation** (e.g. `power_jw`) rather than runtime
convention flags. They're functionally different math; modeling
them as separate variations keeps the internal convention singular.

### Apo-only artifacts we deliberately don't reproduce

Some flames render with extra visible structure in Apophysis 7X that
neither flam3-derived implementation (ours nor JWildfire) produces.
The math is identical — the divergence is an Apo-specific bug.

Pattern recognized so far:

- **Blur/noise `Randomize`-per-call**. Apo's `TXForm.Blur` /
  `TXForm.Noise` (and the other blur-class procedures) each begin
  with `Randomize`, which reseeds Pascal's PRNG from `GetTickCount`
  on every call. The Pascal authors annotated this with
  `// HACK! Fix me...` and never did. Effect: within a 1ms window
  (~1000 IFS iterations) every blur/noise call returns the same
  `(theta, r)`. The orbit gets pushed by a constant vector for
  ~1000 iterations, then flips. Over many ms-ticks, the per-tick
  drift directions accumulate into structured patterns (visible
  rings, ghost copies of the attractor).

  flam3 ported these without the `Randomize` call. JWildfire
  inherited the fixed version. We did too. So our output and
  JWildfire's agree; Apo is the outlier.

  **Reproducibility test for `Plastic-colortest.flame`** (the flame
  this came up on): the `dense ring inside the blur disc` structure
  the user saw in Apo is absent in JWildfire's render of the same
  flame. Confirmed Apo-bug.

  **Tells that a flame may be relying on this**: blur and/or noise
  with a non-trivial weight, especially combined with multiplicative
  variations (spherical, julia, julian) whose iteration dynamics
  amplify small per-call perturbations. The visible artifact tends
  to be a high-contrast inner ring, repeated ghost copies, or
  starburst patterns through a uniform-fill region.

  **What we do**: nothing. Document the divergence and move on.
  Faithful emulation on GPU is expensive (would need ms-resolution
  seed sharing across threads) and would copy a known bug into our
  pipeline. If a user reports a specific Apo flame that doesn't
  match, point them at this section.

### ~~Zero-weight variations should still count as "present"~~ — RESOLVED

Resolved on the `variations-param-type-cleanup` branch. Both
suspected call sites had been silently dropping zero-weight
variations:

- **Animation system** (commit `6b0883a`): the target selector
  filtered out variations with `weight == 0.0` in all three pool
  builders (Transform, LinkedTransform, FinalTransform). Created a
  catch-22 — the variation was invisible until it had weight, but
  you couldn't give it weight via animation without selecting it as
  a target. Filter removed.
- **Shader builder** (commit `7a990eb`): `extract_active_variations`
  filtered with `weight.abs() > 1e-6`, so variations at weight 0
  never got compiled into the shader. Animating from 0 → nonzero
  triggered a shader recompile mid-animation (visible as a hitch on
  the threshold-crossing frame). Filter removed — variations
  present in any transform's variations map now compile into the
  shader regardless of weight.

Contract is now consistent: **if a variation is in the flame, plan
on it being used.** The user controls what's in the flame; explicit
adds-then-zero are respected as "include this in the shader".

### ~~Stray `weight: f32` parameter in some WGSL bodies~~ — RESOLVED

Resolved on the `variations-init-slot-and-macmillan` branch. The
mechanism turned out to be hand-inlined special cases in
`shader_builder_v2.rs` for `zcone`, `zscale`, `ztranslate`, and
`flatten` that bypassed the standard `variation_NAME(...)` dispatch
entirely — the function definitions were never called, so their
signature mismatches were harmless.

Cleanup: removed the four special-case branches, fixed
`variation_flatten`'s body to produce equivalent math under standard
dispatch (`return vec3(p.x, p.y, 0)` instead of the stale
`vec3(0, 0, -p.z)`), and dropped the stale `, weight: f32` parameter
from the zcone/flatten/zscale signatures. The rotation3d variations
already had clean `(p, xform_id, variation_id)` signatures — no
change needed there. Verified bit-identical pixel output via 3D
smoke tests for zcone, flatten, and zscale.

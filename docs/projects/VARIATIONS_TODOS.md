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

(For DC variations whose params are kept-for-parity because the
color-write side is dropped — `dc_carpet3D.color_a..color_f`,
`scale_z`, `reset_z`, `origin` — see the *Direct-color (DC) port
decisions* section below rather than duplicating here.)

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

#### Pending research

The remaining four are harder — they're not standard Apo 7X plugins,
and Apo 7X core variations like `power`/`rings` live inline in the
Pascal renderer, not in separate `.pas` files. Need to consult
non-Apo-7X canonical sources per variation:

- **`power`** ([apo_misc.rs](../../src/variations/defs/apo_misc.rs)) —
  one of Scott Draves's original 27 from the *Fractal Flames* paper.
  Canon = flam3 source ([scottdraves/flam3](https://github.com/scottdraves/flam3)).
- **`rings`** ([apo_misc12.rs](../../src/variations/defs/apo_misc12.rs)) —
  also from the original 27. Canon = flam3.
- **`flower_db`** ([apo_misc9.rs:273](../../src/variations/defs/apo_misc9.rs#L273))
  — DarkBeam plugin (`_db` suffix). Not in Apo 7X repo. Canon = DarkBeam's
  original deviantart post (hard to locate) or use JWildfire as
  canon-by-proxy.
- **`pre_disc3d`** ([spin_phase.rs](../../src/variations/defs/spin_phase.rs))
  — JWildfire-only (Apo has `pre_disc` 2D, no 3D version). Canon =
  JWildfire `PreDisc3DFunc.java`.

**For variations where Apo 7X and JWildfire genuinely differ** (if
any), the recommended approach is to **add a sibling variation**
(e.g. `power_jw`) rather than runtime convention flags. They're
functionally different math; modeling them as separate variations
keeps the internal convention singular.

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

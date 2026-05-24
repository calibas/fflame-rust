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
- `loonie` ([advanced.rs](../../src/variations/defs/advanced.rs))
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
- `wedge` ([extended.rs](../../src/variations/defs/extended.rs))
- `bwraps` ([extended.rs](../../src/variations/defs/extended.rs))
- `julia3dz` ([extended.rs](../../src/variations/defs/extended.rs))
- `curl3d` ([extended.rs](../../src/variations/defs/extended.rs))
- `blur_circle` ([extended.rs](../../src/variations/defs/extended.rs))
- `blur_zoom` ([extended.rs](../../src/variations/defs/extended.rs))
- `blur_pixelize` ([extended.rs](../../src/variations/defs/extended.rs))
- `separation` ([extended.rs](../../src/variations/defs/extended.rs))

### Enum-candidate parameters

Confirmed during review — currently declared as `Integer` but
semantically picks among a few labeled modes. Convert during the Phase 3
type-correction pass.

- `falloff2.type`, `pre_falloff2.type`, `post_falloff2.type` — 3
  branches (0 = uniform, 1 = triangular, 2 = gaussian). Same enum
  across all three phase variants. See
  [extended.rs](../../src/variations/defs/extended.rs),
  [pre_phase.rs](../../src/variations/defs/pre_phase.rs),
  [post_phase.rs](../../src/variations/defs/post_phase.rs).

---

## Out of scope (defer to other branches)

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

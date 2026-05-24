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

---

## Out of scope (defer to other branches)

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

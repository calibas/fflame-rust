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

---

## Out of scope (defer to other branches)

### `depth3d.rs` WGSL signature has an unused `weight: f32` argument

All three Z-only variations in
[depth3d.rs](../../src/variations/defs/depth3d.rs) (`zcone`, `flatten`,
`zscale`) declare their WGSL function with a trailing `weight: f32`
parameter that the shader builder never passes.

Per the signature contract in
[VARIATIONS_WIRE_FORMAT.md §4](VARIATIONS_WIRE_FORMAT.md), with
`parameters: &[]`, `needs_rng: false`, `needs_transform: false`,
`writes_color: false`, `needs_accum: false`, the only argument should
be `p: vec3<f32>`. The extra `weight: f32` either makes the WGSL
silently fail to link in 3D mode, or there's special-case handling for
legacy Z-only variations somewhere in the codebase.

Investigate, then either:
- Remove the stale `weight: f32` parameter from the three WGSL bodies
  (they don't use it), or
- Document the special case if there's a real reason these need it.

Either way, it's a correctness/cleanup task, not a metadata task.

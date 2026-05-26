# Packed variation parameter buffer

## Goal

Replace the fixed `100 variations × 16 params` parameter buffer with a
per-flame packed layout where each variation occupies exactly the
slots its definition declares (`parameters.len() + init_param_count`).

This removes the 16-slot per-variation hard ceiling, cuts the
parameter buffer's typical waste from ~80% to ~5%, and unblocks ~13
currently-blocked variations whose param count exceeds 16
([variation-port-blockers.md](variation-port-blockers.md) §10).

## Non-goals

- **Runtime per-variation slot resizing.** Slot count is set at
  registration via `VariationDef`; changing it at runtime is not
  supported.
- **Per-variation slot-count UI.** This is a backend / shader-builder
  change. The user-facing parameter UI is unaffected.
- **Cross-flame parameter sharing.** Each flame still owns its own
  parameter buffer; we're just packing it tighter.
- **Reducing the per-flame 100-variation cap.** Untouched.

## Current layout

[`src/gpu/buffers.rs:127`](../../src/gpu/buffers.rs#L127):

```rust
pub const MAX_PARAMS_PER_VARIATION: usize = 16;

pub struct GpuVariationParams {
    /// Flat array indexed by: variation_id * MAX_PARAMS_PER_VARIATION + param_slot
    pub params: [f32; 1600],  // 100 variations × 16 params
}
```

[`shaders/core/utilities.wgsl:7`](../../shaders/core/utilities.wgsl#L7):

```wgsl
fn get_param(xform_id: u32, variation_id: u32, param_slot: u32) -> f32 {
    let idx = variation_id * 16u + param_slot;
    return variation_params[xform_id].params[idx];
}
```

Per flame: `MAX_TRANSFORMS (32) × 1600 floats × 4 bytes ≈ 200 KB`.
Most variations use 3-8 of the 16 slots, so 50-80% of that buffer is
zeros that nobody reads.

## Design

### Packed buffer with per-flame compile-time offsets

Each variation's slot footprint is a build-time constant. The shader
builder already regenerates the WGSL per flame based on the active
variation set ([`src/shader_builder_v2.rs`](../../src/shader_builder_v2.rs)),
so it can also generate a packed `get_param` with offsets baked in.

For each active variation in registration order, compute its slot
count `parameters.len() + init_param_count` and accumulate. The
builder emits:

```wgsl
fn get_param(xform_id: u32, variation_id: u32, param_slot: u32) -> f32 {
    var offset: u32 = 0u;
    switch (variation_id) {
        case 0u: { offset = 0u; }      // linear: 0 slots
        case 1u: { offset = 0u; }      // sinusoidal: 0 slots
        // ... non-active variations skip
        case 24u: { offset = 0u; }     // julian (active): 2 slots, starts at 0
        case 25u: { offset = 2u; }     // blob (active): 3 slots, starts at 2
        case 73u: { offset = 5u; }     // glynnSShape (active): 14 slots, starts at 5
        // ...
        default: { offset = 0u; }
    }
    return variation_params[xform_id].params[offset + param_slot];
}
```

The `variation_params` buffer shrinks to `MAX_TRANSFORMS × total_active_slots`
floats. For a typical 5-variation flame the per-transform footprint
goes from 1600 floats to ~30-50 floats.

### Total buffer sizing

The per-transform buffer size is `total_active_slots × 4` bytes, where
`total_active_slots = sum of (active variation slot counts)`. We need
to pick a buffer ceiling that fits all reasonable flames:

| Scenario | Total slots |
|---|---|
| Typical flame (5 variations, avg 6 slots) | ~30 |
| Heavy flame (15 variations, avg 8 slots) | ~120 |
| Quaternion-using flame (1 var × 120 + 9 × 6) | ~174 |
| Worst case (100 active, avg 16 slots — current cap) | 1600 |

Keep the existing 1600-float buffer as the worst-case ceiling and
treat it as a "soft cap on `sum(active slot counts)`". The shader
builder errors out at flame compile time if the active set exceeds it
(in practice this never happens — current variations average ~6 slots,
so 100 active variations would need to average 16 to fill it).

### Per-variation slot count

Stored on `VariationDef` (already present, just used differently):

```rust
pub struct VariationDef {
    pub parameters: &'static [VariationParamDef],
    pub init_param_count: usize,
    // ...
}

impl VariationDef {
    /// Total slots this variation occupies in the packed param buffer.
    pub fn slot_count(&self) -> usize {
        self.parameters.len() + self.init_param_count
    }
}
```

No data migration needed — we already know each variation's slot
count at registration time.

### Host-side packing

[`src/gpu/buffers.rs`](../../src/gpu/buffers.rs) `update_variation_params`
currently does:

```rust
let buffer_idx = local_idx * MAX_PARAMS_PER_VARIATION + param_idx;
params[buffer_idx] = value;
```

Becomes:

```rust
// Per-flame: variation_offsets is a Vec<u32> sized to the active
// variation count, where offsets[i] = sum of slot_counts before var i.
let buffer_idx = variation_offsets[local_idx] + param_idx;
params[buffer_idx] = value;
```

The `variation_offsets` Vec is built once when the active set changes
(same trigger as shader rebuild). Both host and shader use it.

### Shader rebuild trigger

Today's shader rebuild conditions
([`src/renderer/compute_kernel.rs`](../../src/renderer/compute_kernel.rs))
already include "active variation set changed". This change adds no
new triggers — slot counts only change when the active set changes,
which already rebuilds.

What does change: the rebuild now generates a different `get_param`
based on offsets. That's a tiny addition to the existing code-gen.

## Implementation plan

1. **Add `slot_count()` to `VariationDef`** — derived from existing
   fields, no data changes.
2. **Build `variation_offsets` in shader builder** — when computing
   the active variation list, walk it and accumulate slot counts.
   Store the offsets alongside the active list.
3. **Generate packed `get_param` in shader builder** — emit the
   `switch` statement with one case per active variation and its
   offset. Replace the current `utilities.wgsl` `get_param` with a
   build-time-injected version.
4. **Update host packing** — `update_variation_params` reads the
   offsets from the same source the shader builder used and packs
   accordingly. The `GpuVariationParams.params` array stays at
   `[f32; 1600]` (no struct-size churn).
5. **Remove `MAX_PARAMS_PER_VARIATION = 16` constant.** Add a soft
   total-slots-per-flame check at shader-build time (panic / error
   with a clear message if sum exceeds 1600).
6. **Port at least one previously-blocked variation** to validate
   end-to-end. `vibration2` (24 user params) is a good first target —
   simple body, just over the old budget. `quaternion` (~120 params)
   would be the dramatic stress test.
7. **Visual regression** — run the existing visual-regression suite;
   no test outputs should change since active variations and their
   slot counts are unchanged for ported flames.

## Performance notes

- **Shader switch cost**: WGSL `switch` over u32 with up to 100 cases
  compiles to either a jump table or a small branch tree depending on
  the backend. Each case is a single `offset = constant; break;`. The
  measurement I'd want before shipping: render time delta on a 5-var
  and a 50-var flame between current and packed.

- **Buffer transfer cost**: smaller buffer = faster `write_buffer`
  uploads. Negligible at current sizes (200 KB → ~50 KB) but free.

- **Cache friendliness**: packed slots improve L2 hit rate for the
  variation params read inside the per-iteration loop. Probably noise
  in benchmarks but directionally good.

- **No extra runtime indirection**: offsets are baked into the
  generated shader; no uniform table read per `get_param` call.

## What this doesn't unblock

The "over 16-slot" tier in
[variation-port-blockers.md](variation-port-blockers.md) §10 lists 13
variations. After this change, all of them become slot-budget-OK. But
some have *additional* blockers:

- `prepost_affine` — also needs prepost (#12)
- `pre_recip` — also needs complex-math toolkit extensions for the 11
  remaining complex hyperbolic inverse functions (#1, sort of)
- `complex` — same complex-math extension
- `harmonograph_js` — possibly tractable on its own once 18 params fit
- `vibration2`, `gridout3d`, `xtrb`, `quaternion`, `w`, `z`,
  `rhodonea`, `supershape3d`, `grid3d_wf` — pure-slot blockers,
  unlocked

So expect ~9 immediate ports unlocked, plus 4 partial unblockers.

## Migration path

This is an additive, build-system-only change. No file-format
migration, no preset compatibility break, no UI work. Existing flames
load and render identically (same active variations, same slot
counts, same parameter values; only the in-memory buffer layout
changes).

The risk surface is the shader code-gen path. The visual regression
suite covers the existing rendered outputs; new assertions would be
warranted around the generated `get_param` switch (e.g. "for an active
set of [linear, glynnSShape, circleLinear], the generated switch has
exactly 3 cases with offsets [0, 0, 14]").

## Future work (out of scope)

- **Reduce the 1600-float worst-case ceiling** if telemetry shows
  flames never approach it. Could be a per-platform tuning knob.
- **Per-variation init slot reuse**. Currently each variation's init
  slots are uniquely owned. If two variations would compute the same
  init expression, they could share — but this is a micro-optimization
  unlikely to matter in practice.
- **Move `transforms.variations` indices to a packed sparse format**.
  Today the `variations` map upload is the dominant cost; this would
  be a follow-up.

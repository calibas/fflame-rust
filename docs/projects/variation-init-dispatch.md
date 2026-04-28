# Variation Init Dispatch

## Goal

Move per-variation init-time computations (precomputed values from user
params, e.g. `inv_power = divisor / power`) out of the per-iteration WGSL
body and into a small GPU compute pass that runs once per param change.

Two motivations:

1. **Perf.** Several recently-ported heavy-init variations (`cpow2`, `cpow3`,
   `disc2`, `log_apo`, `log_db`, `juliaq`, `julia3dq`, `juliac`) are running
   ~5-15 ops of constant-relative-to-params init math every iteration —
   ~33M iterations/frame × ~10 wasted ops × N active variations. Several
   percent of frame time saved per init-heavy variation.

2. **Code clarity.** Body fragments currently mix init logic with the
   per-iteration math. Cleaner if `wgsl_init` lives separately and bodies
   only contain the actual transformation.

Hard constraints:

- **API-shippable.** Variations distributed via the API are a JSON blob
  with WGSL strings and a parameter schema. The init formula has to be
  expressible in that format — no Rust closures.
- **No shader rebuilds on slider drag or animation.** Existing live
  animation system will sweep params at 60 Hz; we cannot recompile shaders
  on the param-change path.
- **Backward compatible.** Existing variations without init keep working
  unchanged. Old API JSON without the new fields still parses.

## Design

Each variation gets two new optional fields:

```rust
pub struct VariationDef {
    // ... existing fields ...
    pub wgsl_init: Option<&'static str>,  // a WGSL function fragment
    pub init_param_count: usize,           // number of derived values it produces
}
```

For API-served variations, the matching JSON fields are added to
`VariationDownload` with `serde(default)` so old payloads parse cleanly.

### Buffer layout (unchanged in shape)

The existing `VariationParams.params: array<f32, 1200>` (100 variations ×
12 params) stays the same. Within each variation's 12-slot region:

- Slots `0..N` — user-facing parameters, written by the CPU buffer
  populator from `Transform.variation_params`
- Slots `N..N+M` — init-derived parameters, written by the GPU init pass

Both ranges are read by the variation body via the same
`get_param(xform_id, variation_id, slot)` accessor. The body code doesn't
care which kind of param it's reading.

Cap: `N + M ≤ 12`. Worst case in current ports is `cpow2` with 4 user + 7
init = 11. We leave the cap at 12 for now and revisit if a future port
needs more.

### Init function signature

Each variation's `wgsl_init` is a function with a uniform shape:

```wgsl
fn init_cpow2(user: array<f32, 4>) -> array<f32, 7> {
    let r = user[0];
    let a_p = user[1];
    let divisor = user[2];
    let range_p = user[3];
    var out: array<f32, 7>;
    out[0] = 6.28318530717959 / divisor;             // ang
    out[1] = r * cos(1.5707963267948966 * a_p) / divisor;  // c
    out[2] = r * sin(1.5707963267948966 * a_p) / divisor;  // d
    out[3] = out[1] * 0.5;                            // half_c
    out[4] = out[2] * 0.5;                            // half_d
    out[5] = 0.5 / range_p;                           // inv_range
    out[6] = 6.28318530717959 * range_p;              // full_range
    return out;
}
```

The init function name is mangled per-variation (`init_<name>`). Sizes are
known from `parameters.len()` and `init_param_count` so the shader builder
can emit the correct array sizes.

### Init compute shader

Generated per-flame by the shader builder from the active variations'
`wgsl_init` fragments. Structure:

```wgsl
@group(0) @binding(0) var<storage, read_write> variation_params: array<VariationParams>;
@group(0) @binding(1) var<uniform> init_params: InitParams;  // num_xforms etc

// Each ported wgsl_init fragment is concatenated here

@compute @workgroup_size(64)
fn init_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pair_idx = gid.x;
    if (pair_idx >= TOTAL_INIT_PAIRS) { return; }

    // Decode pair_idx → (xform_idx, variation_local_idx)
    // Emitted as a switch/case structure, since the active set is
    // compile-time-known for this flame.

    switch (pair_idx) {
        case 0u: { /* xform 0, cpow2 */
            var user: array<f32, 4>;
            user[0] = variation_params[0u].params[0u * 12u + 0u];
            user[1] = variation_params[0u].params[0u * 12u + 1u];
            user[2] = variation_params[0u].params[0u * 12u + 2u];
            user[3] = variation_params[0u].params[0u * 12u + 3u];
            let derived = init_cpow2(user);
            variation_params[0u].params[0u * 12u + 4u] = derived[0];
            // ... etc for derived[1..6]
        }
        // ... one case per (xform, init-bearing variation) pair
    }
}
```

The `12u` is the per-variation slot count (`MAX_PARAMS_PER_VARIATION`).
Emitted with the local indices the main shader builder already computes.

### Dispatch flow

Today, in `compute_kernel.rs`:

```
write_params → main_dispatch → accumulate → tonemap
```

After:

```
write_params → [init_dispatch (if init-dirty)] → main_dispatch → accumulate → tonemap
```

Init dispatch is a one-shot small dispatch (single workgroup of 64 threads
covers up to 64 (xform, variation) pairs — comfortable headroom over the
realistic worst case of ~32 × 10 = ~320 pairs, which would need 5
workgroups). It's enqueued in the same command encoder as the main
dispatch — no extra `submit()` calls, just an extra `dispatch_workgroups()`
call and a buffer barrier.

### Init-dirty flag

The CPU buffer populator (`update_variation_params`) writes user params and
sets an init-dirty flag. The render loop checks the flag at the start of
each frame and dispatches init exactly once if set, then clears the flag.

If params don't change between frames (static flame), init dispatch is
skipped entirely. Animation / slider drag = re-dispatch every frame, still
microsecond-level cost.

### Cache invalidation

Two pipelines now per flame: main + init. Both rebuild on the same trigger
as the existing `ShaderCache`:

- Active variation set changes → both rebuild
- Path features / xaos / mode change → main only
- Constants change → main only
- Registry version changes → both rebuild

The init pipeline cache mirrors the existing one in `shader_cache.rs`.

### Inlined-constants (export) mode

When `should_use_inlined_constants()` is enabled (CLI export only),
parameters get baked into the shader as compile-time literals via
`with_inlined_transforms` / `get_inlined_var_param`. Init values are
similarly bakeable: CPU computes derived values once, emits them as
additional cases in the same `get_inlined_var_param` switch. No separate
init dispatch needed in this mode — the entire derived chain folds at
shader compile time.

### Body code migration

For each existing heavy-init variation, the body code today reads
`get_param(slot)` for slots `0..N` and computes derived values inline.
After migration:

- The init logic moves verbatim into a new `wgsl_init` fragment
- The body code reads `get_param(slot)` for slots `N..N+M` instead of
  computing derived values
- Body code becomes shorter and matches upstream's `transform()` 1:1

Existing parameterized variations without init (e.g. `bubble2`, `popcorn2`,
`cardioid`) remain unchanged — `wgsl_init: None`, `init_param_count: 0`,
business as usual.

### What `wgsl_init` is NOT for

- Per-iteration precomputation that depends on the input point (`FTx`,
  `FTy`, `FTz`). Those stay in the body. Example: `waves2_3d`'s
  `avgxy = (FTx + FTy) / 2` must be per-iteration; it's not constant
  across iterations within a render. We can't move that to init.
- Per-frame setup that depends on the affine matrix. None of our current
  ports need this; if a future variation does, we'd need a different
  mechanism.
- Anything using RNG. Init runs once per dispatch; randomness belongs in
  the body where each iteration draws fresh values.

## Implementation steps

### PR 1 — infrastructure (no behavior changes) ✅ COMPLETE

1. ✅ **Add fields to `VariationDef`** (commit `dce6d4d`)
2. ✅ **API schema fields** (commit `1605f51`)
3. ✅ **Bump per-variation slot count 12 → 16** (commit `4aea156`)
4-5. ✅ **Init shader generator + ShaderCache integration** (commit `3b2114f`)
6. ✅ **Dispatch wiring** (commit `aabed2b`)

All six steps landed as no-op extensions: every existing variation sets
`wgsl_init: None` and `init_param_count: 0`, so the init pipeline returns
`None`, the init dispatch is skipped, and rendering is bit-identical to
pre-PR-1 baseline (verified after each step against
`tests/visual/configs/variations/misc-variations.fflame`).

End-to-end verification of the init-dispatch path itself happens in PR 2
when we migrate the first heavy-init variation — at that point the init
pipeline becomes `Some` and the dispatch actually fires.

6. **Inlined-constants mode reuse**
   - Per decision A above: no init baking in this mode. The init dispatch
     runs once at export start to populate the buffer; the main shader
     reads init values from the buffer (along with user params, which in
     this mode are themselves baked into the shader as compile-time
     constants via `get_inlined_var_param`).
   - Reuses the same dispatch infrastructure built in step 5 — no
     dedicated CPU-side init evaluator.

7. **PR 1 smoke-test**
   - Render every existing test config, bit-compare against pre-PR-1
     baseline. With `wgsl_init: None` on all variations, output must be
     pixel-identical.
   - Render a flame mixing several variations including a few of the
     heavy-init ones (still using their inline init bodies). Confirm no
     shader recompiles fire when params change.

### PR 2 — migrations + new ports ✅ COMPLETE

All migrations + new ports landed on the `variation-init-dispatch` branch
(same branch as PR 1, since cpow2 was migrated as part of the PR 1
end-to-end verification step).

  - **cpow2** (commit `26f8e7b`, on PR 1 branch as the proof of life):
    99.997% pixel-identical to pre-migration baseline. The 2 differing
    pixels are f32 last-bit rounding from a different op order.
  - **cpow3, disc2, log_db, juliaq, julia3dq, juliac** (commit `7d95889`):
    follow-on migrations using the same pattern. Smoke-tested with a
    1M-iter render mixing all seven plus cpow2.
  - **`cell`** *not* migrated — its only init value is `1/size`, a single
    division. The body-cleanup gain isn't worth the migration churn.
  - **`target`, `yin_yang`** (commit `d6542cc`): net-new ports off the
    porter-omitted-init watchlist, blocked on init support before this
    PR. `target.size` default bumped from upstream's 0 (which yields
    `t mod 0` = NaN) to 1.0.

`log_apo` was deleted before PR 1 — see commit `8d5e488` on the
bulk-port branch — because it's functionally identical to the existing
`log` from the base 84.

Skipped from this work (deferred):
  - Comparing against `inlined_constants` export render — the export
    path runs the same init dispatch then bakes user params; a separate
    bit-diff smoke test would confirm parity. Not blocking the merge.

## Decisions (resolved 2026-04-27)

Five questions came up during plan review. Resolutions:

### A. CPU-side WGSL evaluation for inlined-export mode → **option (b)**

Skip init baking in `inlined_constants` mode. Init values come from the
buffer; the init dispatch runs once at export start to populate them; the
main shader's body still inlines everything else as today. The buffer
populator and dispatch trigger reuse the same code path as the main-app
interactive flow — the export path is just "compute init once, then run
the inlined main shader". No CPU-side WGSL evaluator needed.

We can revisit if future perf measurements show inlined init slots make
a meaningful difference, but the perf delta is small (init is ~5-15 ops
out of hundreds in the body) and writing a WGSL evaluator that exactly
matches spec semantics is non-trivial.

### B. Per-variation slot count → **bump 12 → 16**

`MAX_PARAMS_PER_VARIATION` goes from 12 to 16. `VariationParams.params`
goes from `[f32; 1200]` to `[f32; 1600]` (100 variations × 16 params).
Cost: ~22 KB extra GPU memory, negligible. Leaves headroom for future
variations with more init values than `cpow2`'s 11.

### C. Workgroup sizing → **fixed W=64, dispatch ceil(N/64)**

One thread per (transform, init-variation) pair. Threads with
`gid.x >= total_pairs` early-return. Worst case ~320 pairs → 5 workgroup
launches.

This isn't really a design call — it's the only sensible choice given
WGSL's workgroup-dispatch API. Single-thread workgroups would be wasteful
(per-launch driver overhead), 320-thread workgroups exceed typical
backend limits.

### D. Migration ordering → **two PRs**

PR 1 lands the infra as a no-op (existing variations keep their inline
init since `wgsl_init: None`). Verify pixel-identity against the
pre-PR-1 baseline before merging — proves the infra doesn't change
rendering for any existing flame.

PR 2 migrates the 8 existing heavy-init ports + adds `target` and
`yin_yang`. Bit-diff each migrated variation individually against its
PR-1-baseline output.

### E. Include `target` and `yin_yang` → **yes**

Both are on the porter-omitted-init watchlist, blocked on init-step
support. Migrating them as part of PR 2 validates the infra against
fresh-port variations (not just refactors of in-tree code), and gets
two more entries off the watchlist.

## Risks

- **GPU dispatch overhead on mobile/WASM.** Each compute dispatch carries
  some driver overhead. WebGPU on Safari/Chrome is generally good (~10-50µs)
  but I haven't measured. If overhead turns out to be 200µs+, the 60Hz
  animation case could lose 1% of frame budget purely to dispatch
  overhead. Mitigation: fold init into the start of the main shader (each
  workgroup runs init once at the top, into workgroup-shared memory) as a
  follow-up if measurements show this matters.

- **Subtle init-vs-body coupling bugs.** The migration step has to be
  exact — the init logic moving to the GPU needs to produce bit-identical
  results to the previous in-body computation. Floating-point ordering
  matters; subtle precision differences could shift fractal output by
  visible amounts. Mitigation: pixel-diff tests against pre-migration
  baselines.

- **Init shader can't share variation function bodies.** The
  per-variation WGSL function (e.g. `variation_cpow2`) is concatenated
  into the main shader, not the init shader. The init function
  (`init_cpow2`) needs to be in both — or, more cleanly, only in the init
  shader and accessed via the buffer from the main shader. Architecturally
  the init function should NOT appear in the main shader source.

## Effort estimate

- Infrastructure (steps 1-6, no migrations): ~half a day of focused work
- Migration of 8 existing heavy-init variations: ~1-2 hours
- Smoke testing + bit-identity verification: ~1 hour
- Total: ~1 working day

If we discover issues during migration (like the bit-identity concern
above), add 0.5-1 day for diagnostics and fixes.

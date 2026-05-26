# Per-thread variation state + intra-iteration accumulator reads

## Goal

Add two coupled features to the variation framework:

1. **Per-thread variation state** — let a variation declare a
   `state_count` of mutable f32 slots. State is per-(thread, xform,
   variation), persists across the inner iteration loop within one
   shader invocation, and re-initializes at the start of each main()
   call (each compute dispatch). Backed by a single module-level
   `var<private> thread_state` array with per-flame baked offsets,
   accessed via generated `get_state` / `set_state` switches.

2. **Intra-iteration accumulator reads (`needs_accum`)** — let a
   variation see the running result of *prior* variations in the same
   iteration. This is what cpp variations get from `FPx/FPy/FPz`. When
   the flag is set, the variation gains an `accum: vec2/vec3<f32>`
   parameter and the codegen passes the current accumulator value.

The two features are coupled because most cpp variations that need
one need the other — they read FP* to compute a contribution that
also depends on persistent state.

## Background

### What FP*/FT* are in cpp

In JWildfire/Apophysis cpp (which is the source of all our porting):
- `FTx, FTy, FTz` — *From Transform*, the input to a variation after
  the affine transform was applied. Equivalent to our `p` parameter.
- `FPx, FPy, FPz` — *Final Point*, a running accumulator. The
  iteration's variations contribute via `FPx += weight * x_part`. So
  when variation N reads FPx mid-body, it sees the sum of contributions
  from variations 0..N−1 in this iteration.
- `VVAR` — current variation's weight (= `xform.variations[idx]`).
- `TC` — iteration's color register.

### What our generated `apply_variations` already does

The shader builder emits, per flame, code roughly equivalent to:

```wgsl
var result = vec2<f32>(0.0, 0.0);
if (xform.variations[0] != 0.0) { result += w0 * variation_a(temp); }
if (xform.variations[1] != 0.0) { result += w1 * variation_b(temp); }
return result;
```

`result` here is exactly cpp's `FPx/FPy/FPz`. Our variations don't
currently see it — that's what `needs_accum` adds.

### Why this isn't full "persistent state"

In cpp, the `Variables` struct lives for a flame's lifetime and is
shared across millions of point iterations on a single thread. Our
GPU model: ~32K threads run in parallel, each iterates 256 times per
dispatch, then exits. State here is per-thread and re-initializes
each dispatch.

Most observed cpp uses converge fast enough that re-init each
dispatch is visually indistinguishable from full persistence. The
audit caught no variations among the 8 candidates that actually need
cross-dispatch state (mandelbrot does, but it's separately blocked
by primitives infrastructure).

## Non-goals

- **Cross-dispatch persistence.** State resets each main() call. If a
  variation later turns out to need true cross-dispatch state, it
  needs a separate (storage-backed, atomically-managed) project.
- **Cross-thread shared state.** Each thread has its own
  `var<private>` allocation; threads never see each other's state.
- **Pre-phase accum reads.** Pre-phase variations modify the input
  `temp`, not the output `result`. cpp pre-phase variations don't read
  FP* (and if they did, FP* would be 0). `needs_accum` is gated to
  normal + post phases.
- **Solving blocker #2 (color register reads in non-color path)**
  generally. We do solve macmillan/recurrenceplot's specific case,
  because those compute TC from `FPx + FPy` *after* their own
  contribution is added — that's representable as
  `accum + own_contribution`, both available locally.
- **Solving blocker #11 (`Complex` math runtime).** Klein_group still
  blocked.
- **Solving blocker #4 (pre-affine input in post phase).** sphtiling3v2
  still blocked.

## Audit (what motivates this project)

Of 8 cpp variations originally categorized as "persistent state" in
[variation-port-blockers.md](variation-port-blockers.md) §6, a careful
re-read showed:

| Variation | State | Reads FP* / TC | Verdict in this project |
|---|---|---|---|
| `curliecue2` | _x0/_y0/_theta/_phi/_s | no | unlocked (state only) |
| `farblur` | _r[4] + _n | yes (FP*) | unlocked (state + accum) |
| `hexaplay3D` | rswtch/fcycle/bcycle | yes (FP*) | unlocked (state + accum) |
| `hexnix3D` | rswtch/fcycle/bcycle | yes (FP*) | unlocked (state + accum) |
| `macmillan` | _xa/_x/_y | yes (FP+FP for TC) | unlocked (state + accum) |
| `recurrenceplot` | _y1/_y2/_oldx/_oldy | yes (FP* for TC) | unlocked (state + accum) |
| `klein_group` | prev_matrix | no | still blocked (Complex math) |
| `sphtiling3v2` | _xy/_uv | reads FT in post-phase | still blocked (#4) |

Total unlocked: **6**.

Likely additional unlocks once both features ship and we re-audit
other rows — the original blockers doc has many entries marked
"persistent state" that are really FP*-read variations that happen to
also have state. Conservative estimate: 3–8 additional unlocks from
re-audit.

## Design

### `VariationDef` additions

Three new fields, all defaulting to "off":

```rust
pub struct VariationDef {
    // ... existing fields ...

    /// Number of f32 state slots this variation owns per (xform,
    /// variation) instance. Slots are zero-initialized each shader
    /// invocation and persist across the inner iteration loop within
    /// that invocation. Variations access their slots via the
    /// generated `get_state` / `set_state` accessors. Default 0 (no
    /// state).
    pub state_count: usize,

    /// Optional WGSL fragment that runs once at thread start to
    /// initialize this variation's state slots beyond zero-fill. The
    /// fragment runs inside main() before the iteration loop with
    /// `xform_id`, `variation_id`, and `set_state` in scope. Default
    /// None (zero-init is sufficient).
    pub wgsl_state_init: Option<&'static str>,

    /// Whether the variation reads the running variation accumulator
    /// (cpp's FPx/FPy/FPz). When true, the function signature gains
    /// `accum: vec2<f32>` (or `vec3<f32>` in 3D). The shader builder
    /// passes the current `result` value. Only effective in normal
    /// and post phases — pre-phase variations don't see an
    /// accumulator. Default false.
    pub needs_accum: bool,
}
```

### Per-thread state layout

State is keyed on `(xform_idx, variation_local_id)` — two transforms
both using `farblur` get independent state. Layout is computed once
per flame, baked into the generated accessors, and matches the offset
walking in the host (no GPU upload — state lives in `var<private>`).

`scene/transforms.rs`:

```rust
pub struct PackedStateEntry {
    pub xform_idx: u32,
    pub variation_local_id: u32,
    pub variation_name: String,
    pub offset: u32,
    pub state_count: u32,
}

pub fn compute_state_layout(
    flame: &Flame,
    local_map: &HashMap<String, u32>,
    registry: &VariationRegistry,
) -> Vec<PackedStateEntry>;

pub fn total_state_slots(...) -> u32;
```

Layout walks transforms in order; for each xform it walks the active
variations in local-index order; for each (xform, var) where the
variation has `state_count > 0`, appends an entry whose `offset`
follows the previous entry's. Total slots = last entry's `offset +
state_count`.

### Generated WGSL

Module level (only when `total_state_slots > 0`):

```wgsl
var<private> thread_state: array<f32, TOTAL>;

fn get_state(xform_id: u32, variation_id: u32, slot: u32) -> f32 {
    var offset: u32 = 0u;
    let key = xform_id * 100u + variation_id;
    switch (key) {
        case 0u: { offset = 0u; }    // xform 0, var 0: 4 slots
        case 5u: { offset = 4u; }    // xform 0, var 5: 6 slots
        case 100u: { offset = 10u; } // xform 1, var 0: 4 slots
        // ...
        default: { offset = 0u; }
    }
    return thread_state[offset + slot];
}

fn set_state(xform_id: u32, variation_id: u32, slot: u32, value: f32) {
    // same switch, then:
    thread_state[offset + slot] = value;
}
```

Switch key `xform_id * 100 + variation_id` works because
`MAX_VARIATIONS_PER_FLAME = 100` and `MAX_TRANSFORMS = 32`, so the
key fits in u32 with no collisions.

State init runs in main() before the iteration loop:

```wgsl
fn main(...) {
    // RNG, current, etc.

    // State init (only when any active variation declares
    // wgsl_state_init; otherwise zero-init from var<private> is enough)
    {
        // emitted per-(xform, var) with custom init
        set_state(0u, 5u, 0u, /* fragment-supplied expr */);
        // ...
    }

    for (var i = 0u; i < params.iterations_per_thread; i++) {
        // ...
    }
}
```

### Variation function signature

Three new combinations on top of the existing
`(needs_rng × has_params × writes_color)` matrix. Total signature
matrix grows from 16 (today's combinations) to 32. Codegen handles
this in one place — `generate_3d_wrapper` and the call-site
arg-builder in `build_2d`/`build_3d`.

When `needs_accum`:
- 2D variations gain `accum: vec2<f32>` after `p` and before
  `xform_id`.
- 3D variations gain `accum: vec3<f32>` after `p` and before
  `xform_id`.

Stateful variations don't change signature — they use the
module-level `get_state`/`set_state` accessors.

### Codegen change in `apply_variations`

Today's normal-phase emit:

```wgsl
result += w * variation_foo(temp [, xform_id, var_id] [, rng] [, vc]);
```

With `needs_accum`:

```wgsl
result += w * variation_foo(temp, result [, xform_id, var_id] [, rng] [, vc]);
```

Note: passing `result` *before* the `+=` means the variation sees the
sum of prior variations, matching cpp semantics. We don't need to
copy `result` to a local first — the compiler will sequence the right
expression.

For post-phase (variations that mutate `result` directly), we pass
`result` for both `temp` and `accum`:

```wgsl
result = variation_post(result, result [, ...]);
```

### Soft cap on state slots

Add `MAX_STATE_SLOTS_PER_FLAME = 1024` (4 KB per thread). Worst case
of all 8 audited variations active in 4 transforms each: ~64 slots.
1024 leaves 16× headroom. Soft-cap log warning if exceeded; clamp.

## Implementation plan

### Phase 1: Field plumbing (no behavior change)
1. Add `state_count`, `wgsl_state_init`, `needs_accum` to
   `VariationDef`.
2. Mirror on `VariationInfo`.
3. Update `from_def`, `from_download`, `param!` macro defaults.
4. Build clean.

### Phase 2: State layout
5. `compute_state_layout` + `total_state_slots` in
   `scene/transforms.rs`.
6. Unit test: empty layout for stateless flame; correct offsets for a
   flame with 2 stateful variations across 2 xforms.

### Phase 3: Shader builder — state accessors
7. `build_state_accessors` in `shader_builder_v2.rs`. Returns the
   `var<private> thread_state` declaration + `get_state`/`set_state`
   switch, or empty string when total slots == 0.
8. Inject before `utilities.wgsl` in main / tiled / export shaders
   (same insertion point as `build_packed_get_param`).
9. Build and run an existing flame — should be byte-identical (no
   active variation declares state yet).

### Phase 4: Shader builder — accum threading
10. Update arg-builder in `build_2d`/`build_3d` for normal + post
    phases: when `info.needs_accum`, prepend `result` (or
    `temp`/`result` for post) to the call args.
11. Update `generate_3d_wrapper` to forward accum.
12. Build and run — should be byte-identical (no active variation
    declares `needs_accum`).

### Phase 5: Validate with curliecue2 (state only)
13. Port `curliecue2` with `state_count = 6`. Smoke config + render.

### Phase 6: Port accum variations
14. `farblur` (state + accum, smoke)
15. `hexaplay3D` (state + accum, smoke)
16. `hexnix3D` (state + accum, smoke)
17. `macmillan` (state + accum, computes TC locally; smoke)
18. `recurrenceplot` (state + accum + writes_color, smoke)

### Phase 7: Documentation
19. Update [variation-port-blockers.md](variation-port-blockers.md):
    mark blocker #6 partially resolved (per-thread cases unlocked,
    cross-dispatch deferred), blocker #1 partially resolved
    (intra-iteration unlocked, cross-thread/cross-iteration still
    hard architectural limits), blocker #2 partially resolved
    (FP*-derived TC writes unlocked).
20. Re-audit other rows in the blockers doc for incidental unlocks.

### Phase 8: PR
21. Smoke + visual regression suite.
22. Single squashed PR or 3 commits (infra / curliecue2 / accum
    variations).

## Risks and open questions

- **`var<private>` size limits.** WGSL spec doesn't impose one but
  driver implementations may. 1024 f32 = 4 KB/thread is well within
  the typical 32 KB per-thread limit on desktop and mobile GPUs. WASM
  WebGPU should also accept this.

- **Switch performance.** A switch with 50–100 cases is compiled to a
  jump table on most targets; lookup is O(1). If profiling shows it's
  hot, switch to a precomputed offset array uploaded as a uniform
  (mirrors what we'd do for packed params).

- **`accum` semantics ambiguity in post phase.** In cpp post-phase,
  FP* before the post variation runs is the variation-output of the
  current iteration. In our model, `result` at the time the post
  variation runs already holds that output. So passing `result` as
  `accum` (and as `temp` since post variations modify in place)
  matches. Confirm with a unit test or visual smoke.

- **Variation ordering.** Variations are called in registry order
  (= local index order). cpp uses declaration order. For preset
  configs that depend on ordering of FP*-reading variations, our
  output may differ from cpp. The blockers doc already notes this for
  other features. Document it; do not block.

## File touches (estimated)

```
docs/projects/intra-iteration-state-and-accum.md      (new, this file)
docs/projects/variation-port-blockers.md              (mark partial unlocks)
src/variations/definition.rs                          (~30 LoC)
src/variations/mod.rs                                 (~30 LoC)
src/scene/transforms.rs                               (~80 LoC)
src/shader_builder_v2.rs                              (~250 LoC)
src/variations/defs/curliecue2.rs                     (new, ~150 LoC)
src/variations/defs/farblur.rs                        (new, ~200 LoC)
src/variations/defs/hexaplay3d_misc.rs                (new, ~250 LoC)
src/variations/defs/hexnix3d_misc.rs                  (new, ~280 LoC)
src/variations/defs/macmillan.rs                      (new, ~150 LoC)
src/variations/defs/recurrenceplot.rs                 (new, ~250 LoC)
src/variations/defs/mod.rs                            (~10 LoC entries)
tests/visual/configs/variations/<6 smoke configs>     (~120 LoC each)
```

Total: ~1300 LoC + ~700 LoC of test configs.

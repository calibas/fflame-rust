# Subflame variations v2: real parameter and state slots

## Goal

Make every variation work correctly inside a subflame, not just the
parameter-less / state-less ones. Today, the chaos game runs fine for
linear / sinusoidal / spherical / swirl / etc., but anything that
needs `get_param`, `get_state`, an init shader, or a direct
`transforms[xform_id]` read renders broken (zeros / NaN / nothing).
Notable affected variations: **julian / julian_n** (power, dist
params), **blob** (high/low/waves), **klein_group** (16-slot init +
1 state slot + reads its own weight), and most of the ~60 variations
with `needs_transform: true`.

## Where we are today

Subflame transforms use a synthetic `xform_id = 128 + pool_offset`
that's deliberately outside the parent's id space. The variation
system's per-xform lookups all key off `xform_id`:

| Lookup | Where it reads | Subflame behavior |
|---|---|---|
| `get_param(xform_id, var_id, slot)` | `variation_params[xform_id].params[…]` | OOB → 0 |
| `get_state(xform_id, var_id, slot)` | `thread_state[…]` (built per parent xform set) | OOB → 0 |
| `transforms[xform_id].variations[var_id]` | parent `transforms` buffer | OOB → garbage |
| Init shader dispatch | pair list built over parent xforms only | subflame xforms never initialized |

The synthetic id was a v1 shortcut documented in
[`shaders/core/subflame.wgsl:27-37`](../../shaders/core/subflame.wgsl#L27-L37)
and
[`docs/projects/subflames.md`](subflames.md). The shape of the fix is
"give subflame xforms real slots in the buffers, just like parent
xforms have."

## Design: unified xform_id space

The cleanest fix unifies subflame xforms into the same `xform_id`
space as parent xforms. Subflame xforms get real slots immediately
after parent xforms in every per-xform buffer.

```
xform_id layout (unified):

  [0 .. P)                          parent normals + linkeds + finals (P = parent total)
  [P + SF[0].offset .. P + SF[0].end)        subflame 0's normals + linkeds + finals
  [P + SF[1].offset .. P + SF[1].end)        subflame 1's normals + linkeds + finals
  ...

  P = flame.transforms.len()
    + flame.linked_transforms.len()
    + flame.final_transforms.len()

  SF[i] = byte-relative offset into the unified array for subflame i
```

`SubflameMeta` records the unified-array offset for each subflame so
the subflame dispatch can compute `xform_id = sf_meta.xform_id_base +
picked_within_pool` instead of `128u + …`.

### Why unify the namespace

Three alternatives considered:

1. **Unify** (this design) — one xform_id space; all per-xform
   buffers grow to cover both parent and subflame xforms. Every
   existing variation lookup (`get_param`, `get_state`,
   `transforms[xform_id]`) works for subflame xforms with zero
   per-variation changes.
2. **Parallel buffers** — keep parent and subflame buffers separate,
   add `get_param_subflame` / `get_state_subflame` /
   `subflame_transforms` accessors, route based on context. Requires
   editing all 60+ variations that read `transforms[xform_id]` —
   widespread, error-prone, doubles the maintenance surface.
3. **Pass everything by argument** — change every variation signature
   to take `weight`, `params`, `state` as arguments. Same problem as
   (2) plus invasive churn on every variation function.

Unify wins on every dimension except "biggest buffer-size change up
front", which is bounded and manageable (see Memory below).

## Buffer changes

### transforms

`array<GpuTransform, N>` where N is currently `MAX_TRANSFORMS = 128`.
Subflame xforms append after parent xforms in the same buffer. New
cap: `MAX_TRANSFORMS_UNIFIED = MAX_TRANSFORMS + MAX_SUBFLAME_TRANSFORMS_TOTAL`.

The existing `subflame_transforms` buffer is redundant — its content
moves into `transforms` at the new extended slots. Either:
- Delete `subflame_transforms_buffer` entirely (and remove the bind
  group entry), or
- Keep it as a write-through alias for compatibility with code that
  references it (preferred for v2 stability — easy to drop later).

I'd default to **delete it**: it's only read by one shader function
(`subflame_iterate`), and that function needs updating anyway.

### variation_params

`array<GpuVariationParams, MAX_TRANSFORMS>` where each entry is
`[f32; 1600]`. Parallel-indexed with `transforms`. Grows the same
way: `MAX_TRANSFORMS_UNIFIED`.

Per-flame packed layout (`compute_packed_layout`) is keyed only on
the variation set, not xform count, so the *offset* table doesn't
need to change — every xform (parent or subflame) reads its params
out of its own row at the same per-variation offsets.

### thread_state (per-thread `var<private>`)

Currently sized as `array<f32, total_state_slots>` where
`total_state_slots = sum over (xform_id, variation) of state_count`.
Built per active variation set. Subflame xforms participate in the
active variation set already (via
[`extract_active_variations`](../../src/scene/transforms.rs#L1463-L1494)),
so the **state layout** in `shader_builder_v2` just needs to count
subflame xforms in the per-xform slot allocation.

The compiled `get_state` / `set_state` switch must include the
extended xform_id range.

### subflame_transforms_buffer (delete)

Per above — fold into the unified `transforms` buffer.

## Init shader

The init compute shader runs once at flame compile time and writes
derived parameters back into `variation_params`. Currently it iterates
parent xforms only ([`build_init_shader`](../../src/shader_builder_v2.rs#L872)).

Extend the dispatch list to include subflame xforms (klein_group's
16-slot init shader is the immediate motivator — without it, the
Möbius generator matrices stay zero).

```rust
let mut pairs: Vec<(u32, String, u32)> = Vec::new();
// existing parent xform walk...
let mut xform_idx = P;  // start at parent total
for sf in &flame.subflames {
    for x in &sf.transforms { emit(x, xform_idx, ...); xform_idx += 1; }
    for x in &sf.linked_transforms { emit(x, xform_idx, ...); xform_idx += 1; }
    for x in &sf.final_transforms { emit(x, xform_idx, ...); xform_idx += 1; }
}
```

## Subflame iteration changes

[`shaders/core/subflame.wgsl`](../../shaders/core/subflame.wgsl):

```diff
- let sub_xform_id = 128u + sf_meta.normals_offset + picked;
+ let sub_xform_id = sf_meta.xform_id_base + picked;
```

`SubflameMeta` gains a `xform_id_base` field (the offset in the
unified `transforms` / `variation_params` arrays where this
subflame's xforms start). Set during `update_subflames` based on
walk order.

Drop the separate `subflame_transforms[…]` reads — read from
`transforms[sub_xform_id]` directly.

## Memory budget

Going to `MAX_TRANSFORMS_UNIFIED = 256` (128 parent + 128 worth of
subflame headroom — generous for v2):

| Buffer | Was | Is | Delta |
|---|---|---|---|
| `transforms` | 128 × ~440 B = 56 KB | 256 × ~440 B = 113 KB | +57 KB |
| `variation_params` | 128 × 1600 × 4 = 819 KB | 256 × 1600 × 4 = 1.6 MB | +819 KB |
| `attachments` | 128 × 264 B = 33 KB | (unchanged — attachment lists are parent-only) | 0 |
| `subflame_transforms` | 128 × 440 B = 56 KB | **deleted** | −56 KB |
| **Per-flame net** | | | **+820 KB** |

Per-thread `thread_state` grows in proportion to the active variation
set's total state slots, summed over total xforms (parent +
subflame). For a typical flame with ~5 stateful variations and 30
total xforms (10 parent + 20 across subflames): 5 × 30 = 150 f32 =
600 bytes per thread. At 8192 threads in flight (typical workgroup
config): ~5 MB. Compare to the current ~2 MB. Acceptable.

The hard upper bound is governed by `MAX_VARIATION_PARAM_SLOTS = 1600`
per xform (unchanged). Most flames stay well below.

## Validation / caps

| Cap | Current | After |
|---|---|---|
| `MAX_TRANSFORMS` (parent xforms across normals+linkeds+finals) | 128 | 128 (split off as `MAX_PARENT_TRANSFORMS`) |
| `MAX_SUBFLAMES` (number of subflames) | (existing) | unchanged |
| `MAX_SUBFLAME_TRANSFORMS_TOTAL` (sum across all subflames) | (existing) | unchanged |
| `MAX_TRANSFORMS_UNIFIED` (size of transforms / variation_params buffers) | n/a (was 128) | 256 (= parent + subflame_total caps) |

`update_variation_params` and `update_transforms` learn to walk
subflame xforms after the parent walk; the existing OOB check
`total_transforms > MAX_TRANSFORMS` becomes
`parent + subflame_total > MAX_TRANSFORMS_UNIFIED`.

## Files touched

| File | Change |
|---|---|
| [`src/gpu/buffers.rs`](../../src/gpu/buffers.rs) | Bump variation_params + transforms buffer sizes; `update_variation_params` and `update_transforms` walk subflame xforms; delete `subflame_transforms_buffer`; `SubflameMeta` gains `xform_id_base`. |
| [`src/shader_builder_v2.rs`](../../src/shader_builder_v2.rs) | `build_packed_get_param` switch covers extended xform_id range (actually unchanged — switch is on `variation_id`, not `xform_id`); `build_state_machinery` slot allocation includes subflame xforms; `build_init_shader` dispatches over subflame xforms too. |
| [`shaders/core/subflame.wgsl`](../../shaders/core/subflame.wgsl) | `sub_xform_id = sf_meta.xform_id_base + picked`; read xform from `transforms[…]` instead of `subflame_transforms[…]`. Update header comment to reflect new model. |
| [`shaders/core/header.wgsl`](../../shaders/core/header.wgsl) | Remove `subflame_transforms` binding; bump array sizes for `transforms` / `variation_params` to new cap. |
| [`src/gpu/pipelines.rs`](../../src/gpu/pipelines.rs) | Update bind group layout to drop subflame_transforms binding. |
| Tests | Update visual regression baselines (subflame renders should change because previously-zero params now use real values — same flame, different output). New unit test: load a subflame with `julian` (power=3, dist=2), verify the render isn't blank. |

## Test plan

- **Regression**: existing subflame visual tests
  ([`tests/visual/configs/2d/subflame-smoke-2d.fflame`](../../tests/visual/configs/2d/subflame-smoke-2d.fflame),
  [`tests/visual/configs/3d/subflame-smoke.fflame`](../../tests/visual/configs/3d/subflame-smoke.fflame))
  still render. They use linear-only subflames so behavior is unchanged;
  the test exists to catch buffer-layout regressions.
- **New positive tests** (`.fflame` configs):
  - Subflame using `julian` with non-default `power` / `dist` —
    verify output matches Apophysis reference.
  - Subflame using `blob` with non-default high/low/waves.
  - Subflame using `klein_group` — verify Mumford/Series Indra's
    Pearls limit-set renders. This is the headline test; it's the
    most complex variation in the registry, exercising init shader +
    parameters + per-thread state + own-weight read all at once.
- **Memory check**: largest test flame's GPU buffer footprint stays
  under a sensible budget (target: <10 MB per flame).
- **Render-correctness check**: replace a subflame's flame mid-session
  (PR `b75bb1c`), verify the chaos game uses the new params (not
  cached zeros).

## Scope

~400-600 LOC across buffer plumbing, shader builder, and
shader sources. Most of the LOC is mechanical extension of existing
loops to cover subflame xforms. Risk concentrates in:

1. **Bind group layout change** — adding/removing bindings forces
   pipeline rebuild; need to verify all render paths (interactive,
   export, tiled high-res) rebuild correctly.
2. **Init shader ordering** — derived params must be in place before
   the first render-frame dispatch reads them. Already works for
   parent xforms; pattern extends but ordering must be re-verified.
3. **Per-thread state slot allocation** — growing it changes the
   register pressure on the compute shader. Watch for compile-time
   regressions on weak GPUs (mobile / integrated).

## Out of scope

- **Nested subflames** (subflame_wf inside a subflame). Still
  excluded; would require recursive xform_id allocation or a
  bounded-depth flattening pass. Separate project.
- **Per-subflame palettes**. Each subflame's color blend still uses
  the parent's palette — independent palettes are tracked in
  [`docs/projects/subflames.md`](subflames.md).
- **Xaos in subflames**. Still weight-only selection.

## Phasing

One PR. Cleanly separable into commits:

1. `SubflameMeta` adds `xform_id_base`; subflame iteration switches
   to the new field (still using synthetic ids that match the old
   `128 + offset` for now, just behind the new field name). No
   behavior change — refactor only.
2. Buffer expansion: bump variation_params and transforms sizes;
   `update_variation_params` walks subflames and writes their params
   to the extended slots. Subflame iteration switches to real
   `xform_id_base = MAX_PARENT_TRANSFORMS + offset`. Variations that
   only need `get_param` (julian, blob) now work.
3. Thread state allocation extended to subflame xforms; klein_group
   and other stateful variations now work in subflames.
4. Init shader dispatch covers subflame xforms; klein_group's
   matrices populate correctly.
5. Delete `subflame_transforms_buffer` and its bind-group entry;
   subflame iteration reads from unified `transforms` buffer. Update
   shader header.
6. New visual regression tests for julian / blob / klein_group in
   subflames.

Each commit is independently testable: after (2), julian works; after
(3), klein_group reads its own weight correctly; after (4),
klein_group fully renders.

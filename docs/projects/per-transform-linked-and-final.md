# Per-transform Linked and Final transforms

## Goal

Replace the current "global Final transform" model with first-class
per-transform attachments: every normal transform can have N attached
**Linked transforms** (deterministic dynamics extension) and M attached
**Final transforms** (post-iteration view filter). Linked + Final each
get their own GPU buffer pool (32 of each, plus the existing 32
normals).

Independent of (and not affecting) the existing xaos system, which
stays as-is for chaos-game routing among normal transforms. The old
xaos-pattern "Linked transforms" UI feature in
[`linked-transforms.md`](linked-transforms.md) is unrelated and
explicitly out of scope here — separate plans for that.

## Background

### Current model (one global Final)

Today's iteration loop:
1. Pick a normal transform N (chaos game, optionally xaos-weighted).
2. Apply N's affine + variations → produces `current`.
3. **If a global Final exists**, apply it to produce `final_pos`.
4. Plot `final_pos`; next iteration's input is `current` (the
   pre-Final position).

The Final's color writes are discarded, opacity is ignored — it
inherits both from the normal that fired (see
[main_template.wgsl:198](../../shaders/core/main_template.wgsl#L198)).

This is sufficient for classical Apophysis-style flames but limits:
- Only **one** Final transform per flame.
- Final is **global** — every normal transform feeds the same
  post-processing.
- No way to deterministically chain transforms without abusing xaos.

### New model

Each normal transform N owns two ordered lists of attachments:

```
each iteration:
  1. select normal transform N (chaos game)
  2. apply N's affine + variations              → P_normal
  3. for each linked L in N.linked_attachments:
        apply L's affine + variations           → P_linked  (sequential)
  4. for each final F in N.final_attachments:
        apply F's affine + variations           → P_final   (sequential)
  5. plot whichever is last in the chain (P_final, else P_linked, else P_normal)
  6. next iteration's input = P_linked (post-linked, pre-final)
```

**Linked is part of dynamics**: feeds forward into the next
iteration, contributes to color flow, contributes to speed-mode
displacement.

**Final is a view filter**: shapes only what gets plotted. Color
writes from a Final are discarded; opacity inherits from the normal.
Doesn't affect dynamics. Matches today's global-Final semantics
exactly.

This is essentially "the global Final, but per-transform and
optionally chained, plus a parallel concept (Linked) for
deterministic dynamics extension."

## Non-goals

- **Reworking xaos.** Xaos still routes among normal transforms only.
  Linked/Final are out of the chaos game.
- **The old xaos-pattern Linked UI feature.** Stays as-is. Separate
  plans.
- **Per-attachment color or opacity.** Inherit from the normal that
  fired. Final color writes still discarded; Linked color writes
  affect `color_index`.
- **Branching inside chains.** Linked and Final are flat sequential
  lists, not trees. If you want branching dynamics, use multiple
  normals + xaos.
- **Mid-chain plotting.** Only the chain end gets plotted. One
  iteration = one plotted point.

## Design

### Iteration semantics

Confirmed earlier in conversation; recap for completeness:

| Step | What happens | Affects dynamics? | Affects plot? |
|---|---|---|---|
| 1. Normal | Affine + variations | Yes (defines P_normal) | Yes (chained downstream) |
| 2. Linked (each) | Affine + variations | Yes (P_linked feeds forward) | Yes (chained downstream) |
| 3. Final (each) | Affine + variations | No (output discarded for IFS) | Yes (last one is plotted) |

**Plot point**: end of chain (P_final if any, else P_linked, else
P_normal). Always plot something.

**Next-iter input**: `P_linked` (after all linkeds, before any
finals).

**Color flow** (per iteration):
- `c_base` computed from `color_index` and the normal's `color_speed`.
- `vc` initialized to `c_base`.
- Variations from N + each Linked write to `vc` (DC variations
  only).
- After Linked chain completes, Step-3 lerp updates `color_index`
  from `c_base` and the final `vc`, weighted by N's
  `direct_color`.
- Final variations may write to `vc` but the writes are discarded
  for `color_index` purposes; only the plotted color uses
  `color_index` from before the Final chain.

**Speed-mode color**: `length(P_linked - old_pos)`. (`old_pos` is
the previous iteration's `current`, which equals that iteration's
`P_linked`.)

**Plot opacity check**: `rng_nextf < N.opacity`. Linked and Final
opacities are unused.

### Storage model

Three separate top-level arrays in `Flame`:

```rust
pub struct Flame {
    pub transforms: Vec<Transform>,             // normal — drives chaos game
    pub linked_transforms: Vec<Transform>,      // pool, referenced by index
    pub final_transforms: Vec<Transform>,       // pool, referenced by index
    // ... other existing fields ...
}

pub struct Transform {
    // ... existing fields (a..g, post_a..post_g, weight, opacity,
    //                       color, color_speed, direct_color,
    //                       variations, variation_params) ...

    // NEW (only meaningful on transforms in `flame.transforms`;
    // ignored on those in `flame.linked_transforms` /
    // `flame.final_transforms`).
    pub linked_attachments: Vec<usize>,   // indexes into flame.linked_transforms
    pub final_attachments: Vec<usize>,    // indexes into flame.final_transforms
}
```

Index references mean **multiple normals can share the same Linked
or Final** instance. Editing `linked_transforms[3]` affects every
normal that references it. (See "Sharing" risk below.)

### Limits

- `MAX_TRANSFORMS = 128` — a **single combined cap** across all three
  pools. Originally scoped as 32-of-each (96 total) but flipped to a
  single 128-cap during Phase 2 implementation when the trade-off
  analysis (below) showed the "intent-revealing" benefit of separate
  buffers wasn't worth the codegen cost of per-pool variation
  function duplication. 128 leaves headroom for any reasonable
  combination (e.g., 50 normals + 50 linkeds + 28 finals).
- `MAX_ATTACHMENTS_PER_TRANSFORM = 32` — per-normal chain length cap,
  independent of the total transform budget.

Per-flame GPU footprint at the worst case:
- Transforms (128 × 480 B): **60 KB**
- Variation params (128 × 6,400 B): **819 KB**
- Attachments (128 × 264 B): **34 KB**
- **~915 KB per flame** worst case. Trivial at modern GPU scales.

### GPU buffer layout

**Single concatenated transforms buffer** + single concatenated
variation_params buffer. Layout:

```
slots 0..N            — normals (N = flame.transforms.len())
slots N..N+L          — linkeds (L = flame.linked_transforms.len())
slots N+L..N+L+F      — finals  (F = flame.final_transforms.len())
slot  N+L+F           — LEGACY copy of flame.final_transform (Phase 2-3
                         transitional; removed in Phase 4)
```

`xform_id` in the shader is a global index into this single array.
Per-pool CPU indexes (in `Transform::linked_attachments` /
`final_attachments`) get translated to global xform_ids during
attachment-list packing — see `GpuAttachmentList::from_transform`.

Why single buffer over three separate? With three buffers each
variation function would need per-pool variants (e.g.,
`variation_linear_normal/linked/final`) because `get_param(xform_id,
var_id, slot)` reads from `variation_params[xform_id]` and the
buffer reference can't be parameterized at WGSL function call sites
without major signature surgery across all 489 variations. A single
buffer makes `get_param` work uniformly across all pools — same
function body, just bigger array. The "intent-revealing" function
names (`apply_normal/linked/final`) can be expressed in the main
loop's chain-execution code without forcing per-pool variation
duplication.

WGSL bindings — only ONE new binding (attachments):

```wgsl
struct AttachmentList {
    linked: array<u32, 32>,        // GLOBAL xform_ids
    linked_count: u32,
    final_: array<u32, 32>,        // GLOBAL xform_ids
    final_count: u32,
}
@group(0) @binding(10) var<storage, read> attachments: array<AttachmentList>;
```

Existing `transforms` (binding 0) and `variation_params` (binding 5)
unchanged in shape — just bigger arrays.

Apply chain in the main loop directly (no need for separate
apply_normal/linked/final functions — they all use the same
`apply_variations` since variation behavior doesn't depend on which
pool a transform is in):

```wgsl
let normal_idx = select_transform_const(rand_val);
let p_normal = apply_variations(transforms[normal_idx], normal_idx, affine_p, &rng);

var p_linked = p_normal;
let attach = attachments[normal_idx];
for (var li = 0u; li < attach.linked_count; li++) {
    let lid = attach.linked[li];  // global xform_id
    let aff = apply_affine(transforms[lid], p_linked);
    p_linked = apply_variations(transforms[lid], lid, aff, &rng);
}

// color_index updates here

var p_final = p_linked;
for (var fi = 0u; fi < attach.final_count; fi++) {
    let fid = attach.final[fi];
    let aff = apply_affine(transforms[fid], p_final);
    var final_vc = color_index;  // discarded after Final chain
    p_final = apply_variations(transforms[fid], fid, aff, &rng, &final_vc);
}

current = p_linked;  // feed forward
plot(p_final);
```

### Main loop changes

Today's loop body (paraphrased):
```wgsl
let xform = transforms[xform_idx];
let affine_p = apply_affine(xform, current);
current = apply_variations(xform, xform_idx, affine_p, &rng);
// ... post-affine, color flow ...
if (HAS_FINAL_TRANSFORM) {
    let final_xform = transforms[FINAL_TRANSFORM_INDEX];
    final_pos = apply_variations(final_xform, ..., &rng);
}
plot(final_pos);
```

New loop body:
```wgsl
let normal = normal_transforms[xform_idx];
let p_normal = apply_normal(xform_idx, apply_affine(normal, current), &rng, &vc);

// Linked chain (deterministic, in-order)
var p_linked = p_normal;
let lcount = attachments[xform_idx].linked_count;
for (var li = 0u; li < lcount; li++) {
    let lid = attachments[xform_idx].linked[li];
    let lxform = linked_transforms[lid];
    let aff = apply_affine_pool(lxform, p_linked);
    p_linked = apply_linked(lid, aff, &rng, &vc);
}

// color_index updates here from c_base + vc
// next-iter input = p_linked

// Final chain (deterministic, in-order; doesn't update color or feed forward)
var p_final = p_linked;
let fcount = attachments[xform_idx].final_count;
for (var fi = 0u; fi < fcount; fi++) {
    let fid = attachments[xform_idx].final[fi];
    let fxform = final_transforms[fid];
    let aff = apply_affine_pool(fxform, p_final);
    var final_vc = color_index;  // discarded after Final chain
    p_final = apply_final(fid, aff, &rng, &final_vc);
}

current = p_linked;  // feed forward
plot(p_final);
```

`apply_affine_pool` is just `apply_affine` rebound for the linked or
final pool — same WGSL function body, different buffer access.

### State / accum keying

Our per-thread state and accum infrastructure currently keys on
`(xform_id, variation_local_id)` via `xform_id * 100 + variation_id`.
With three buffers, the key needs a buffer-kind tag:

```
key = (kind << 16) | (xform_id << 8) | variation_id
   // kind:        0 = normal, 1 = linked, 2 = final
   // xform_id:    0..32
   // variation_id: 0..100
```

(Or encoded as `kind * 3200 + xform_id * 100 + variation_id` — same
information.) The state-accessor switch tables grow proportionally
but it's still a constant-time lookup.

This matters because a stateful Linked transform (e.g., `curliecue2`
used as a Linked) needs its own state slots independent of the
normal that triggered it. With shared-by-index references, two
normals that share `linked_transforms[3]` will also share its
state — the walker advances on whichever normal calls it. Probably
fine; flagging.

### File format / migration

JSON schema additions:
```json
{
    "flame": {
        "transforms": [...],            // unchanged shape, gains attachment fields
        "linked_transforms": [...],     // new array
        "final_transforms": [...],      // new array
        "final_transform": {...}        // legacy: optional, only on old files
    }
}
```

Each normal Transform gains:
```json
{
    "linked_attachments": [0, 2],      // indexes into linked_transforms
    "final_attachments": [0]           // indexes into final_transforms
}
```

**Migration on load** (in the `FractalConfig` deserializer):
1. If `flame.final_transform` is present (legacy), push it into
   `flame.final_transforms[0]` and add `0` to every normal
   transform's `final_attachments`. Drop the legacy field.
2. If `linked_transforms` / `final_transforms` arrays are missing,
   default them to empty.
3. If a normal transform has no `linked_attachments` /
   `final_attachments` field, default to empty.

This preserves visual behavior of all existing flames.

### Apophysis XML import

The Apophysis XML format has a `<finalxform>` element (singular,
global). Maps to the same migration path: import as
`final_transforms[0]` attached to every normal. No change to the
import semantics; just an extra translation step.

### UI changes

Each normal transform's editor gains two collapsible sections:
- "Linked attachments" — list of attached Linked transforms with
  reorder / detach buttons + an "Add linked" dropdown showing the
  pool.
- "Final attachments" — same shape.

Two new pool-management panels (or sections within the existing
transforms panel):
- "Linked transforms pool" — list of all
  `flame.linked_transforms`, with create / delete / select-to-edit.
- "Final transforms pool" — same.

**Sharing UX**: when a Linked or Final is referenced by more than
one normal, the editor shows a `[shared by N transforms]` badge.
Editing it changes the shared instance. Add a "Clone before edit"
button for users who want to fork a shared instance into a unique
one.

**Visual representation in the transform list**: each normal shows
its attachment chain inline, e.g., `T1 → [L0, L2] → [F0]`. Clicking
an attachment opens its editor.

### Per-pixel iteration cap, burn-in, max_iterations

Each `for i in 0..iterations_per_thread` loop iteration runs the
full chain (one normal + its linked + its finals) and plots once.
So the existing burn-in count, per-pixel iteration cap, and
max_iterations all measure "iterations" the same way today's loop
does. No changes.

The `speed_multiplier` system (frame-rate quality control) likewise
unchanged.

## Implementation plan

### Phase 1: Data model + serde + migration

1. Add `linked_transforms: Vec<Transform>` and
   `final_transforms: Vec<Transform>` to `Flame`. Add
   `linked_attachments: Vec<usize>` and
   `final_attachments: Vec<usize>` to `Transform`.
2. serde defaults for new fields (empty vec).
3. Migration in `FractalConfig::deserialize` (or a post-deserialize
   hook): legacy `final_transform` → push to
   `final_transforms[0]`, attach to every normal.
4. Update `Flame::has_final_transform`,
   `Flame::final_transform_index` callers — they need new semantics
   (probably "any normal has a non-empty `final_attachments`").
5. Unit tests: load a few existing `.fflame` files, confirm
   migration produces equivalent flame structure.

### Phase 2: GPU buffer split

6. New `MAX_LINKED_TRANSFORMS` / `MAX_FINAL_TRANSFORMS` constants
   in `gpu/buffers.rs`.
7. Create three separate transform storage buffers + three
   `GpuVariationParams` buffers + an `AttachmentList` buffer. Bind
   group layout grows.
8. CPU-side packers: `GpuTransform::from_flame` splits across pools.
9. Confirm initial render of an existing flame is byte-identical
   (no Linked, single legacy Final imported into final_transforms
   pool).

### Phase 3: Shader builder updates

10. `build_apply_variations_*d` parameterized by buffer kind. Emit
    `apply_normal_*d`, `apply_linked_*d`, `apply_final_*d` per flame.
11. State / accum accessor keying extended with `kind` tag.
    `build_state_accessors` walks all three buffers.
12. `build_state_init_block` emits init blocks for every
    (kind, xform, variation) triple with `wgsl_state_init`.
13. `build_init_shader` (variation init dispatch) handles all three
    buffers.

### Phase 4: Main loop changes

14. Update `main_template.wgsl`, `main_tiled.wgsl`, `main_export.wgsl`
    with the new chain structure (Linked loop + Final loop +
    plotting from chain end).
15. Color flow: `color_index` update moves to after the Linked loop;
    Final's `vc` writes discarded.
16. Speed-mode color: displacement from `old_pos` to `P_linked`.
17. Confirm sanity render of imported legacy flame matches old
    output exactly (no Linked, identical Final behavior).

### Phase 5: UI

18. Transform editor gains attachment sections (add / remove /
    reorder).
19. Linked + Final pool management panels (or sections).
20. Sharing UI: badges + Clone-before-edit.
21. Inline chain display in transform list.
22. Drag/drop reordering of attachments.

### Phase 6: Surrounding system updates

23. Apophysis XML import: same migration path as JSON.
24. Random flame generator: how does it use the new feature? Default
    to no Linked/Final, but optionally seed a Final from a curated
    library?
25. Preset library: existing presets all use the legacy global
    Final; they'll auto-migrate. Future presets can showcase the new
    feature.
26. Undo/redo system: attachment add/remove/reorder become delta
    operations.
27. Animation system: keyframable attachment lists? Probably not v1
    — out of scope.

### Phase 7: Testing + docs

28. Visual regression suite — confirm no diffs on existing flames
    after migration.
29. New visual regression configs that exercise the chain
    (multi-Linked, multi-Final, shared attachments).
30. Update [`docs/main/TRANSFORMS.md`](../main/TRANSFORMS.md) and
    [`docs/main/UI.md`](../main/UI.md) with new model.
31. Mark this project doc complete; PR.md.

## Risks and open questions

- **Sharing UX surprise.** Shared-by-index Linked/Final means
  editing one place affects multiple normals. Without clear UI
  badges and Clone-before-edit, this will confuse users.
  Mitigations in Phase 5.

- **State semantics with shared attachments.** A stateful Linked
  shared by 2 normals has *one* state slot pool. Within an iteration,
  whichever normal triggers it advances the shared state. Probably
  fine for most use cases; pathological for things like macmillan
  used as a shared Linked (which would couple two normals' chaos
  games together). Document the gotcha.

- **MAX_TRANSFORMS = 32 per pool may feel restrictive**. With 32
  normals, 32 unique linked, 32 unique finals you have plenty of
  shared-attachment combinations. But if someone wants 50 unique
  Finals, they're stuck. Bump cap if that becomes a real
  complaint — buffer size is not the constraint.

- **Backwards compatibility on save**. Once a user opens a legacy
  flame in the new build and re-saves, the file gains
  `linked_transforms` / `final_transforms` fields and the legacy
  `final_transform` is dropped. Older builds can't read it back.
  Acceptable for a major-version bump; document it.

- **Visual regression baseline** for legacy flames must remain
  byte-identical after migration. Catch-all test: render every
  existing visual config, compare hashes pre- and post-migration.

- **Init shader dispatch count** scales with the total
  (normal-count × init-bearing-variations + linked-count × ... +
  final-count × ...) pairs. Up to ~3× larger but each pair is
  cheap. Confirm no perf regression.

- **Animation interpolation** for chain shape (add/remove
  attachments mid-animation) is out of scope. Static attachment
  lists per keyframe; transitions are step-functions.

- **Solo mode and per-pixel iteration cap** still meaningful?
  Solo mode hides a normal — still works (just a flag on the
  normal). Per-pixel iteration cap counts iterations of the chain,
  not individual transform applications. Unchanged semantics.

## File touches (estimated)

```
docs/projects/per-transform-linked-and-final.md  (new, this file)
docs/main/TRANSFORMS.md                          (update with new model)
docs/main/UI.md                                  (UI section update)

src/scene/transforms.rs                          (~150 LoC: new fields, migration)
src/config/fractal_config.rs                     (~30 LoC: serde migration)
src/config/delta.rs                              (~80 LoC: ConfigPath for attachments)
src/gpu/buffers.rs                               (~150 LoC: bump cap to 128, concat layout, attachments)
src/shader_builder_v2.rs                         (~80 LoC: emit chain in main loop)
src/scene/randomize.rs                           (~30 LoC: don't break)
src/renderer/compute_kernel.rs                   (~50 LoC: track linked/final counts)
src/apophysis_xml.rs, src/api/sync.rs            (~20 LoC: call migrate_legacy_final)

shaders/core/header.wgsl + 2 variants            (~10 LoC: AttachmentList + binding 10)
shaders/core/main_template.wgsl                  (~80 LoC: chain loop)
shaders/core/main_tiled.wgsl                     (~80 LoC: same)
shaders/core/main_export.wgsl                    (~80 LoC: same)

src/ui/transforms.rs                             (~400 LoC: attachment UI)
src/ui/triangle_editor.rs                        (~50 LoC: handle pool transforms)
src/ui/menu_bar.rs                               (~20 LoC: pool management menu)

tests/visual/configs/...                         (~5 new configs exercising chains)
```

Total: ~1,500 LoC + ~600 LoC of UI + test configs.

## Future extensions (not in this PR)

- Per-attachment color override (opt-in per Linked/Final, default
  inherit).
- Conditional attachments (only fire if some predicate holds).
- Branching chains (Linked → fork into multiple Finals).
- Animation interpolation across chain-shape keyframes.
- Pool transforms shared *across flames* (library of common
  filters).

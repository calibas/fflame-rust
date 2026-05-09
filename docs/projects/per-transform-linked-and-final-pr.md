# Per-Transform Linked and Final Transforms (PR)

Branch: `per-transform-linked-and-final`

Replaces the global single-Final-transform model with first-class per-transform attachments. Every normal transform now owns ordered lists of **Linked** (dynamics extension) and **Final** (view filter) attachments. Linked and Final live in dedicated GPU pools; the chain is dispatched from a per-normal attachment list.

Project doc: [per-transform-linked-and-final.md](./per-transform-linked-and-final.md).

## Summary

- New `flame.linked_transforms: Vec<Transform>` and `flame.final_transforms: Vec<Transform>` pools alongside the existing `transforms` pool.
- Each normal transform carries `linked_attachments: Vec<usize>` and `final_attachments: Vec<usize>` (ordered indices into the corresponding pool).
- The legacy `flame.final_transform: Option<Transform>` field is gone. Old `.fflame` files migrate at deserialize time: the singular Final becomes `final_transforms[0]` with auto-attachment on every normal.
- GPU pipeline: a single concatenated transform buffer (normals + linkeds + finals, capped at 128) plus an `attachments` storage buffer (one entry per normal, holding linked + final index lists, each capped at 100 entries).
- Shader chain logic: `apply_normal → for L in linked: apply L → plot point starts at last_linked → for F in finals: apply F → plot point becomes last_final; next-iter input = post-linked, pre-final`.
- UI: three-section panel (Transforms / Linked / Final) with per-pool Add buttons; per-normal Advanced section gains "Linked XForms" and "Final XForms" subsections with toggle checkboxes and ↑/↓ reorder buttons.
- Triangle Editor handles all three pools (selector lists Normal / Linked / Final; canvas draws all three with distinct color tints; "Edit Triangle" button on every pool member).
- Animation system targets per-pool members (target selector lists each Linked/Final pool member as its own subcategory; track editor and animation export apply to the right pool entry).

## Iteration semantics

| Step | What happens | Feeds next iteration? | Plotted? |
|---|---|---|---|
| 1. Normal | Affine + variations → `P_normal` | Yes (defines start) | Yes (if no chain) |
| 2. Linked (each, in order) | Affine + variations → `P_linked` | **Yes** | Yes (if no finals) |
| 3. Final (each, in order) | Affine + variations → `P_final` | **No** (discarded) | Yes (last final) |

Linked is *part of dynamics*: its output feeds the next iteration. Final is a *plot-time filter*: its output is plotted but the next iteration's input is `P_linked` (or `P_normal` when there's no Linked chain).

Color writes from a Final are discarded; opacity inherits from the firing normal — same as the old global-Final semantics.

## What changed (by area)

### Data model (`src/scene/transforms.rs`)

- `Transform` gains `linked_attachments: Vec<usize>` and `final_attachments: Vec<usize>`.
- `Flame` gains `linked_transforms: Vec<Transform>` and `final_transforms: Vec<Transform>`. Loses `final_transform: Option<Transform>`.
- `Flame::migrate_legacy_final(Option<Transform>)` consumes a legacy singular Final and migrates it into the pool with auto-attachments. Called from Flame's custom `Deserialize` and from external migration sites (Apophysis XML import, API DTO conversion).
- `Flame::total_gpu_transform_slots()` returns `normals.len() + linkeds.len() + finals.len()`.
- `compute_state_layout` (variation-state ABI) walks all three pools in `[normals, linkeds, finals]` order to match the GPU buffer layout.

### Config (`src/config/`)

- `ConfigPath` gains:
  - `LinkedTransformAffine{index, param}`, `LinkedTransformPostAffineEnabled{index}`, `LinkedTransformPostAffine{index, param}`, `LinkedTransformVariation{index, variation}`, `LinkedTransformVariationParam{index, variation, param}` for the Linked pool.
  - `PoolFinalTransformAffine`, `PoolFinalTransformPostAffineEnabled`, `PoolFinalTransformPostAffine`, `PoolFinalTransformVariation`, `PoolFinalTransformVariationParam` (same shape) for the Final pool.
  - All the legacy `FinalTransform*` (no-index) variants stay as compat aliases that route to `final_transforms[0]` via `manager.rs`. Animation tracks saved against the legacy variants keep working.
- New `TransformKind` enum (`Normal | Linked | Final`) and `TransformRef` enum (`Normal(usize) | Linked(usize) | Final(usize)`). `TransformRef` provides path builders (`affine_path`, `post_affine_path`, `variation_path`, `variation_param_path`) and `get`/`get_mut` helpers, so the same UI render fns can drive any pool member.
- `ConfigManager` get/set handlers for the new variants source from / write to the right pool. Variation-weight semantics (NaN = remove, 0.0 = kept) and variation-param writes route through shared helpers (`apply_variation_weight`, `apply_variation_param`) so all pools behave identically.

### GPU (`src/gpu/buffers.rs`, `src/renderer/compute_kernel.rs`, shaders)

- `MAX_TRANSFORMS` raised to 128. `MAX_ATTACHMENTS_PER_TRANSFORM` set to 100 (was 32).
- New `GpuAttachmentList` struct (per normal): `linked: [u32; 100]`, `linked_count`, `final_: [u32; 100]`, `final_count`. Padded to MAX_TRANSFORMS in the GPU buffer.
- `GpuTransform::from_flame` / `GpuVariationParams::from_flame` concatenate `[normals, linkeds, finals]` into a single buffer; CPU pool indices are translated to global xform_ids when packing the attachment lists.
- New bind-group entry `attachments: array<AttachmentList>` in all three shader headers (`header.wgsl`, `header_export.wgsl`, `header_tiled.wgsl`).
- Shader chain logic in `main_template.wgsl` / `main_export.wgsl` / `main_tiled.wgsl`: after normal-apply, run linked chain in order; in the burn-in-gated plot block, run final chain on a copy and plot the result.

### UI (`src/ui/`)

- New three-section Transforms panel (`transforms.rs`):
  - Top: existing per-normal list, now with attachment subsections.
  - Below: "Linked Transforms (N)" header + Add button + per-pool list.
  - Below: "Final Transforms (N)" header + Add button + per-pool list.
  - Add Final auto-attaches to every normal; deleting a pool member auto-detaches and reindexes all attachments.
- Per-normal Advanced section gains "Linked XForms" / "Final XForms" subsections — one row per pool member with a toggle checkbox + execution-order reorder buttons (↑/↓). Pool order is fixed; only per-normal attachment order is editable.
- Pool-member rendering uses one shared `render_pool_member_block`. Per-pool customization (`PoolMemberOptions`) decides whether to show top-level weight, color, color dynamics, solo, or attachment subsections. Linked + Final hide weight + color + opacity (Linked sequential, Linked/Final inherit from triggering normal).
- Triangle Editor (`triangle_editor.rs`):
  - Selection switched from `Option<usize>` to `TransformRef`.
  - Combobox lists Normals, Linkeds, Finals (separators between groups).
  - Canvas draws all three pools; Linked uses a cool grey-blue tint, Final uses a warm grey-tan tint. Selected pool member highlights normally; others are dimmed.
  - All edits (drag, quick-action buttons, coord/coefficient sliders, reset to identity) dispatch via `xref.affine_path()` / `post_affine_path()` so they emit the right ConfigPath for whichever pool member is selected.
  - "Edit Triangle" button on every Linked/Final pool member opens the editor with that selection.

### Animation (`src/ui/target_selector.rs`, `src/ui/track_editor.rs`, `src/animation/export.rs`)

- Target selector replaces the single "Final Transform" category with one category per Linked / Final pool member. Each lists affine, post-affine, variations, and variation-params.
- Track editor `get_current_value` reads any pool member via the new variants. Affine + post-affine reads share a `read_affine` / `read_post_affine` helper.
- Animation export `apply_config_value` writes any pool member via the new variants. Affine writes share `apply_affine_param` / `apply_post_affine_param` helpers.
- Legacy `FinalTransform*` (no-index) animation paths keep working by routing through `final_transforms.first()` everywhere they're consumed.

### Misc consumers updated to the new pool model

`apophysis_xml.rs`, `api/sync.rs`, `scene/randomize.rs`, `variations/mod.rs::missing_variations_in`, `shader_builder_v2.rs`, `shader_cache.rs`, `export/high_res.rs`, `gpu/buffers.rs::update_*` panic messages.

## Backwards compatibility

- Old `.fflame` files load unchanged. The custom Flame deserializer accepts either `final_transform` (legacy singular) or `final_transforms` (new pool) JSON keys. Legacy singular gets migrated to `final_transforms[0]` with auto-attachment on every normal.
- Old animation tracks targeting the legacy `FinalTransform*` ConfigPath variants keep working — those variants now read/write `final_transforms[0]` everywhere.
- Apophysis XML imports flow into the new pool with auto-attachment, matching prior visual behavior.

## What's deferred

- Renaming `PoolFinalTransform*` → `FinalTransform*` (would collide with the legacy compat-alias variants). The Pool prefix is descriptive in the meantime; the rename can happen once the legacy compat aliases are no longer needed.
- Inline-shader path (`shader_builder_v2`, used for some specialized rendering) still treats `final_transforms[0]` as the singular Final. Multi-final in inline mode is a follow-up — the main attachment-driven chain works in all standard render paths.
- The dead `has_final_transform` flag in `GpuParams` (no longer read by shader chain logic) is left for binary layout stability; can be removed when GpuParams gets its next intentional layout change.

## Test plan

- `cargo test --lib` — 190 tests passing (3 new Flame migration tests).
- Build clean on desktop (Windows/MSVC). WASM not separately verified in this PR but no platform-specific code touched.
- Manual smoke-test:
  - Three-section panel renders, Add buttons create pool members, attachment checkboxes/reorder buttons mutate per-normal lists.
  - Triangle Editor selects/edits Linked + Final pool members.
  - Old `.fflame` presets loaded from `assets/presets/` render visually identical.
- Visual regression and benchmark suites not re-run for this PR; should be run before merge.

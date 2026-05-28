# API v2 — config v2 schema migration

## Why

The shipped API serializes `FractalConfig` into a flat typed schema so the server can index, search, and migrate individual fields rather than treat flames as opaque blobs. Client `FractalConfig` has since gained subflames, multiple final transforms, linked transforms, several tonemap fields, and palette pipeline extensions. The wire format silently drops all of them today.

v2 catches the wire format up. No client release has shipped yet, so **this is a clean break** — no back-compat shims, no version field on the wire, no aliases for renamed fields. If we ever need versioning later we'll URL-version (`/v2/flames`).

## Locked decisions

| Decision | Choice | Notes |
|---|---|---|
| Back-compat | None | No serde aliases, no `config_version` field, no migration matrix. Pre-release. |
| Subflame storage | Same `flames` table with `parent_flame_id` FK | Recursive. Subflames are not first-class entities; they round-trip with their parent. |
| Subflame submission | Inline in parent's request | Single round-trip — `CreateFlameRequest.subflames: Vec<SubflameRequest>` is created/updated transactionally with the parent. |
| Subflame request shape | Distinct `SubflameRequest` type | Subflames are nested `Flame` instances; tonemap, palette, and visibility live on `FractalConfig`, not on `Flame`. The wire format mirrors that — subflames don't carry those fields at all. See "Wire format" below. |
| Subflame visibility | Inherited from parent | No `visibility` on the wire. Server gates access via the parent's `visibility`; subflame row's own `visibility` column is set to the parent's value on insert but never read directly. |
| Subflame tonemap / palette | Inherited from parent | No tonemap / palette fields on the wire. Subflame DB rows retain the columns with default values; they're never read. |
| Subflame nesting depth cap | 4 | Engine currently uses at most depth 1; design for 4. Reject deeper at create/update. |
| Subflames per parent cap | 8 | Engine's `subflame_id` parameter has range `0..=7`; flames with more than 8 subflames are unrenderable. Reject at create/update. |
| Final-transform model | Plural | `final_transforms: Vec<...>` instead of `final_transform: Option<...>`. |
| Linked transforms | Separate pool | `linked_transforms: Vec<...>` per flame. |
| Active toggling | Derived | A linked/final transform is "active" iff at least one normal transform's attachment array references its index. No `enabled` column. |
| Palette dual scheme | Dropped | `palette_id` only. Embedded palettes get uploaded by the client to `/api/palettes` first (content-addressable storage dedupes) and referenced by ID. No `embedded_palette` JSONB column. |
| Tonemap presets | Not in v2 | Skipped per direction. |

## Schema changes

### `flames` table

Additions:
- `parent_flame_id UUID REFERENCES flames(id) ON DELETE CASCADE` — nullable; `NULL` for top-level flames, set for subflames.
- `subflame_order INTEGER` — nullable; order within parent's `subflames` array. Only meaningful when `parent_flame_id IS NOT NULL`.
- `highlight_mode TEXT NOT NULL DEFAULT 'clip'` — `clip` / `max_norm` / `reinhard` / `filmic`.
- `white_level REAL NOT NULL DEFAULT 200.0`.
- `palette_squeeze_mode TEXT NOT NULL DEFAULT 'linear'` — `linear` / `geometric`.
- `palette_squeeze_falloff REAL NOT NULL DEFAULT 0.5`.
- `palette_log_strength REAL NOT NULL DEFAULT 0.0`.
- `palette_reverse BOOLEAN NOT NULL DEFAULT false`.
- `flame_name TEXT` — internal `.fflame` name, distinct from the cloud-library `name`.

Subflames are still `flames` rows so they carry every column the table defines. On insert the server sets `visibility` to the parent's value and lets tonemap/palette columns take their column defaults; none of these are read for rows where `parent_flame_id IS NOT NULL`. Document the rule in `src/models/flame.rs`.

Two additional indexed columns aggregate across the whole subflame tree (recomputed on every write):

- `has_3d BOOL` — true if **the root flame or any nested subflame** has a 3D render mode. The current per-flame `has_3d` becomes a tree-aware aggregate. A user searching for 3D flames expects to find any flame that renders in 3D, including via a subflame.
- `variation_names TEXT[]` — already exists; now collected by walking the whole subflame tree, not just the root's transforms. Same reasoning: `?variation=zcone` should match a flame whose subflame uses `zcone`.

Both are recomputed inside `create_flame_with_children` / `update_flame_with_children` after the entire tree has been validated; subflame inserts happen in the same transaction so the values are accurate when the parent row is finalized.

**Catalog filtering — every catalog query needs `WHERE parent_flame_id IS NULL`.** Specifically:
- `flames::list_by_user`, `list_by_user_public`, `list_public`, `list_featured`
- `routes::explore::*` (relies on the above)
- `routes::search::search_flames`
- `repositories::favorites::list_by_user`
- `repositories::collections::get_collection_flames`
- `repositories::user_limits::check_limit("flames")` count — subflames don't count toward `max_flames`
- `repositories::user_limits::get_usage`'s `flames.used` count — same

**Direct-access guards.** Subflames can't be fetched, forked, favorited, tagged, thumbnailed, or added to a collection on their own. The route-layer guard is: load the row, check `parent_flame_id IS NULL`, else return 404. Apply to:
- `GET /api/flames/{id}` (and the OptionalAuthUser variant)
- `POST /api/flames/{id}/fork`
- `POST /api/flames/{id}/favorite`, `DELETE`, and the `check_favorite` GET
- `PUT/GET/DELETE /api/flames/{id}/thumbnail`
- `GET /api/flames/{id}/tags`, `PUT`
- `POST /api/collections/{id}/flames` (the `flame_id` in the body)
- `POST /api/animations` and `PUT /api/animations/{id}` (the `flame_id` body field — already gated by `check_flame_owned_by_caller`; add a "must not be a subflame" check there too)

### `transforms` table

- Add `transform_kind TEXT NOT NULL DEFAULT 'normal'` — values: `normal` / `linked` / `final`.
- Add `linked_attachments JSONB NOT NULL DEFAULT '[]'::jsonb` — array of `int`, indices into the flame's linked-pool.
- Add `final_attachments JSONB NOT NULL DEFAULT '[]'::jsonb` — array of `int`, indices into the flame's final-pool.
- Migrate existing rows: `UPDATE transforms SET transform_kind = CASE WHEN is_final_transform THEN 'final' ELSE 'normal' END`.
- Drop `is_final_transform` column.

Both attachment arrays are only meaningful on `kind='normal'` rows. Linked/final transforms still carry the columns for schema symmetry, but the route layer rejects writes where `kind != 'normal'` and either array is non-empty.

`sort_order` continues to apply per kind — i.e., normal transforms have their own ordering, linked transforms have their own, finals have their own.

### `variations` table

**Prerequisite migration A — `writes_color`:** add a `writes_color BOOLEAN NOT NULL DEFAULT false` column to `variations` before the v2 work lands. The engine's variation registry has this flag (decides shader builder behavior); our catalog is currently missing it. Mirrors `needs_rng` / `needs_affine` in shape — additive on both `Variation` and `VariationListItem`. Defaults all existing rows to false; subsequent migrations set it true for the variations that actually write color (small set; check `defs/*.rs` for `writes_color: true`).

**Prerequisite migration B — `aliases`:** add an `aliases TEXT[] NOT NULL DEFAULT '{}'` column to `variations`. Holds foreign-app names (Apophysis 7X / JWildfire / Chaotica) that resolve to this variation on `.flame` XML import — e.g. `linear`'s `aliases` is `'{linear3D}'` because those apps split linear/linear3D where we don't. Additive on both `Variation` and `VariationListItem`. Defaults all existing rows to empty; subsequent migrations populate where needed (initially just `linear`; expand as the import path encounters dropped variation names). Reasoning and client-side handling in [VARIATIONS_WIRE_FORMAT.md §10](VARIATIONS_WIRE_FORMAT.md#10-aliases-for-foreign-app-name-compatibility).

After both, add the engine's subflame variation to the catalog so flames using it pass the unknown-variation check. Values come from `fflame-rust/src/variations/defs/subflame.rs`:

```sql
INSERT INTO variations (name, aliases, display_name, category, phase, needs_rng, needs_affine, writes_color)
VALUES ('subflame_wf', '{}', 'Subflame', 'plugin', 'normal', true, false, true);
```

`shader_2d` / `shader_3d` stay NULL — the renderer special-cases this variation (samples a subflame rather than computing a transform).

## Wire format changes

All struct names below are in `src/models/flame.rs` / `src/models/transform.rs`.

### `CreateFlameRequest` (and `UpdateFlameRequest` alias)

Add:
- `flame_name: Option<String>` — internal `.fflame` name; distinct from `name` (cloud library title).
- `final_transforms: Vec<CreateTransformInput>` — new plural, replaces `final_transform: Option<CreateTransformInput>`.
- `linked_transforms: Vec<CreateTransformInput>` — new pool.
- `subflames: Vec<SubflameRequest>` — recursive (`SubflameRequest` nests itself); empty by default. See struct definition below.
- `highlight_mode: ApiHighlightMode` (new enum). Default `Clip`.
- `white_level: f32`. Default `200.0`.
- `palette_squeeze_mode: ApiSqueezeMode` (new enum). Default `Linear`.
- `palette_squeeze_falloff: f32`. Default `0.5`.
- `palette_log_strength: f32`. Default `0.0`.
- `palette_reverse: bool`. Default `false`.

Drop:
- `final_transform: Option<CreateTransformInput>` — gone, replaced by plural.

### `SubflameRequest` (new)

Mirrors the client-side `Flame` struct (not `FractalConfig`) — subflames don't carry tonemap, palette, visibility, or any other render-state field, because those live on `FractalConfig` and are inherited from the parent at render time. This keeps the wire honest about what's meaningful per row and removes the awkward "server silently ignores 15 fields" rule.

```rust
pub struct SubflameRequest {
    pub flame_name: Option<String>,
    pub render_mode: RenderMode,                       // 2d / 3d
    pub perspective_strength: f32,
    pub solo_transform: Option<i32>,
    pub xaos: Option<Vec<Vec<f32>>>,
    pub transforms: Vec<CreateTransformInput>,         // normal pool
    pub linked_transforms: Vec<CreateTransformInput>,  // linked pool
    pub final_transforms: Vec<CreateTransformInput>,   // final pool
    pub subflames: Vec<SubflameRequest>,               // recursive
}
```

All other flame columns on a subflame row get their column defaults (or, for visibility, the parent's value at insert). Validation should reject any input that tries to set tonemap/palette/visibility fields on subflame rows by virtue of the type system — `SubflameRequest` simply doesn't have them.

### `CreateTransformInput` (and `TransformResponse`)

Add:
- `linked_attachments: Vec<usize>` — default `[]`.
- `final_attachments: Vec<usize>` — default `[]`.

Keep on response only:
- `transform_kind: ApiTransformKind` — enum `Normal` / `Linked` / `Final`. Present on `TransformResponse` for read-side, **not** on `CreateTransformInput`. The server assigns the kind from which array field the transform arrived in (`transforms` vs `linked_transforms` vs `final_transforms`).

Drop:
- `is_final_transform` field.

### Implementation note: pool wiring

`CreateFlameRequest` (and `SubflameRequest`) carry three transform arrays:

```rust
pub transforms: Vec<CreateTransformInput>,           // normal pool
pub linked_transforms: Vec<CreateTransformInput>,    // linked pool
pub final_transforms: Vec<CreateTransformInput>,     // final pool
```

The server assigns `transform_kind` from which array each transform arrived in. No `transform_kind` field on `CreateTransformInput`. `sort_order` is assigned per pool, in array order.

### `FlameResponse`

Mirrors `CreateFlameRequest` plus computed fields. Specifically:
- `subflames: Vec<FlameResponse>` — recursive, populated by `assemble_flame_response` via a new repo function `find_subflames(parent_id)`.
- `transforms`, `linked_transforms`, `final_transforms` — three arrays, populated from `transforms` rows bucketed by `transform_kind`.

The current `FlameResponse.transforms` and `FlameResponse.final_transform` fields both go away; replaced by the three pools.

### Search

Extend `routes::search::SearchFilters`:
- `has_subflames: Option<bool>`
- `has_linked: Option<bool>`
- `final_transform_count_min / max: Option<i32>`
- `highlight_mode: Option<ApiHighlightMode>`

These map to indexed columns extracted at insert time:
- `flames.has_subflames BOOL` — recomputed on write from `subflames.is_empty()`.
- `flames.has_linked BOOL` — recomputed from `linked_transforms.is_empty()`.
- `flames.final_transform_count INTEGER` — recomputed from `final_transforms.len()`.

Add to the relevant migrations alongside the structural changes.

## Validation rules (route layer)

Apply in `create_flame` and `update_flame`, before `repositories::flames::create_flame_with_children`. Walk the whole subflame tree, not just the root:

1. `subflames.len() <= 8` at every level. Reject with `Validation("subflames_per_parent_exceeded")`. Engine's `subflame_id` parameter is `0..=7`.
2. Subflame nesting depth ≤ 4 (root is depth 0). Reject with `Validation("subflame_too_deep")`.
3. For every normal transform (at every depth), each entry in `linked_attachments` must satisfy `0 <= idx < linked_transforms.len()` **of the same flame the transform belongs to**, not the root's. Same for `final_attachments`. Reject with `Validation("attachment_out_of_range: linked")` / `final`.
4. Linked-pool and final-pool transforms must have empty attachment arrays. Reject with `Validation("attachment_on_non_normal_transform")`.
5. Existing `reject_unknown_variations` already covers `subflame_wf` once the migration adds it to the catalog. Extend the helper to walk subflames so unknown variations nested in a subflame also reject.
6. Subflame `flame_name` and other string fields go through length-cap validation. Reuse `validate_flame_input` (split out a sub-helper if needed so it can run on `SubflameRequest` too).
7. `SubflameRequest` is enforced by the type system — clients can't send tonemap/palette/visibility fields on subflames. No runtime check needed.

## Implementation order

Each step is a separate migration + code change + tests.

1. **Prerequisite migration A: variations.writes_color** — Add `writes_color BOOLEAN NOT NULL DEFAULT false` to `variations`. Add to `Variation` and `VariationListItem` mirroring `needs_rng` / `needs_affine`. Tests confirm presence in list and detail. (Independent of the rest; can ship on its own.)
2. **Prerequisite migration B: variations.aliases** — Add `aliases TEXT[] NOT NULL DEFAULT '{}'` to `variations`. Add to `Variation` and `VariationListItem` (`Vec<String>`, `#[serde(default)]`). Backfill: `UPDATE variations SET aliases = '{linear3D}' WHERE name = 'linear';` plus any other rename/alias entries identified by the upstream-name comparison pass (see [VARIATIONS_WIRE_FORMAT.md §10](VARIATIONS_WIRE_FORMAT.md#10-aliases-for-foreign-app-name-compatibility)). Independent; can ship alongside or after migration A.
3. **Migration C — flames structural** — Add `parent_flame_id`, `subflame_order`, `has_subflames`, `has_linked`, `final_transform_count`, `flame_name`, and the six tonemap columns to `flames`. Backfill `has_subflames` / `has_linked` to `false`, `final_transform_count` to `(SELECT COUNT(*) FROM transforms WHERE flame_id = flames.id AND is_final_transform = true)`. Existing `has_3d` and `variation_names` get recomputed by application code on next write (no SQL backfill needed since old flames have no subflames).
4. **Migration D — transforms structural** — Add `transform_kind`, `linked_attachments`, `final_attachments` to `transforms`. `UPDATE transforms SET transform_kind = CASE WHEN is_final_transform THEN 'final' ELSE 'normal' END`. Drop `is_final_transform`.
5. **Migration E — subflame_wf catalog entry** — INSERT the row per the spec above.
6. **Code** — update Rust models (introduce `SubflameRequest`), repo, route, OpenAPI registrations in `src/lib.rs`. New enums `ApiHighlightMode`, `ApiSqueezeMode`, `ApiTransformKind`. Drop singular `final_transform` from `CreateFlameRequest` and `FlameResponse`.
7. **Repo updates** — `create_flame_with_children` walks `subflames` recursively, calling itself for each child with `parent_flame_id` set. The aggregate `has_3d` and `variation_names` are computed across the entire tree before the root row is inserted/updated. `update_flame_with_children` rewrites the subflame set wholesale (delete subflames where `parent_flame_id = $id`, re-insert from the request — matches the existing "delete children and reinsert" pattern for transforms). `assemble_flame_response` calls a new `find_subflames(parent_id)` to populate the nested array, recursively assembling each subflame.
8. **Catalog filter pass** — every site in the impact list above gets `WHERE parent_flame_id IS NULL`. Add an `assemble_flame_response`-level guard that returns NotFound if `parent_flame_id IS NOT NULL` and the caller isn't recursing internally.
9. **Direct-access guards** — fork, favorite, thumbnail, tags, collection-add: reject with 404 when the target is a subflame.
10. **Tests** — round-trip a flame with subflames, with multiple finals, with linked transforms, with all six new tonemap fields. Regression test: subflames don't appear in `/api/explore`, `/api/flames` (list), `/api/users/{id}/flames`, or `/api/search/flames`. Regression test: GET `/api/flames/{subflame_id}` returns 404. Regression test: a parent with 3 subflames costs 1 against `user_limits.max_flames`, not 4. Regression test: deleting a parent cascades to subflames via the FK. Regression test: search by `variation=zcone` matches a parent whose subflame uses zcone. Regression test: subflames-per-parent cap (9 rejected, 8 accepted) and depth cap (5 rejected, 4 accepted).

## Client-side impact (palette flow)

The "palette_id only" decision is one sentence on the server but a meaningful shift on the client. Today's `FractalConfig.palette` is an embedded `Palette { name, stops }`. v2 means:

- **Save flame** — if the active palette has no `api_palette_id` cached yet, the client first `POST /api/palettes` with the embedded data, gets back an `id`, caches it on `FractalConfig.palette` (or alongside it), then `POST /api/flames` with that `id`. Two round trips; both can fail independently — design the save UX accordingly.
- **Load flame** — `GET /api/flames/{id}` returns `palette_id`. Client then `GET /api/palettes/{palette_id}` to reconstitute embedded palette data. Two round trips again.
- **In-memory palettes** — palettes the user has authored but never saved need a "not yet uploaded" sentinel until the first save. Existing `FractalConfig.palette` doesn't have that state.
- **Content-addressable dedup is free** — the server hashes palettes by content (SHA-256) and returns the existing row on conflict, so the client shouldn't worry about flooding the catalog with near-duplicates.

The active toggling for linked/final transforms is similarly an editor concern: with no `enabled` column, the only way to "deactivate" a linked or final transform is to detach every attachment. The editor either needs explicit attach/detach gestures or users will be surprised when removing the last attachment makes a transform vanish from the active set. Worth verifying in the client design.

## Open items

All major decisions captured. Minor things to confirm or pin down during implementation:

- Whether `subflame_order` should have a `NOT NULL` constraint when `parent_flame_id IS NOT NULL` (CHECK constraint) — leaning yes, but trivially enforceable in app code if SQL is awkward.
- Confirm the engine's `writes_color` flag for `subflame_wf` — the proposed `true` matches `defs/subflame.rs` as of this writing; double-check at implementation time in case the engine value drifted.

## Out of scope

- Real schema versioning beyond a future URL bump (`/v2/flames`).
- Random generator and audio settings (device-local, never synced).
- Animation track schema versioning (animations stay opaque).
- Subflame thumbnails, fork tracking, favorites — subflames aren't first-class.

# API v2 — server-side plan

Paired with the client's [docs/api-v2.md](api-v2.md). That doc describes the
wire format and client responsibilities; this one covers the database schema,
read/write paths, validation surface, and migration.

> **Status:** design, not yet implemented. All seven previously-open questions
> are now resolved (§"Resolved design choices"). Ready to coordinate with the
> client before writing migrations.

---

## Architecture summary

- **`flames.config JSONB`** — opaque blob holding everything *except*
  root-flame transforms. Server never parses it. Includes the full
  subflame tree (and nested subflames, up to depth 5), all non-transform
  flame fields, and a client-owned `config_version`.
- **`transforms` table** — canonical, slim. One row per **root-flame**
  transform. Subflame transforms live inside the blob and are not
  individually queryable.
- **`palettes` flow unchanged** — content-addressable via `palette_data`,
  inline on flame requests, denormalized hash + name on the flame row.
  See migration `20250101000063` and
  [docs/CLIENT_MIGRATION_PALETTES.md](CLIENT_MIGRATION_PALETTES.md).

Result: a flame is **one row in `flames` + N rows in `transforms` + one
`palette_data` row**. No `transform_variations`, no
`transform_variation_params`, no `config_effects`, no `parent_flame_id`
subflame rows, no `subflame_path` addressing.

---

## Schema

### `flames` (slim)

```sql
CREATE TABLE flames (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,         -- the only title; covers both cloud-library and .fflame
    visibility      visibility NOT NULL DEFAULT 'private',

    -- The opaque blob: everything except root-flame transforms.
    -- Includes the full subflame tree (depth ≤ 5).
    config          JSONB NOT NULL,

    -- Palette (unchanged from shipped redesign)
    palette_hash    TEXT REFERENCES palette_data(hash) ON DELETE SET NULL,
    palette_name    TEXT,

    -- Cached search facets. Only the two that can't be cheaply derived
    -- from the transforms table (or that require blob inspection at
    -- read time) live here.
    render_mode     render_mode NOT NULL DEFAULT '2d',  -- extracted from blob once at save
    has_subflames   BOOLEAN NOT NULL DEFAULT FALSE,     -- derived from config.subflames[] at save

    -- Community / engagement (trigger-maintained, unchanged from today)
    forked_from      UUID REFERENCES flames(id) ON DELETE SET NULL,
    featured_at      TIMESTAMPTZ,
    fork_count       INTEGER NOT NULL DEFAULT 0,
    favorite_count   INTEGER NOT NULL DEFAULT 0,
    view_count       INTEGER NOT NULL DEFAULT 0,
    animation_count  INTEGER NOT NULL DEFAULT 0,

    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_flames_user_id     ON flames(user_id);
CREATE INDEX idx_flames_visibility  ON flames(visibility);
CREATE INDEX idx_flames_render_mode ON flames(render_mode);
CREATE INDEX idx_flames_featured_at ON flames(featured_at DESC) WHERE featured_at IS NOT NULL;
CREATE INDEX idx_flames_created_at  ON flames(created_at DESC);
```

**Dropped from today's `flames` table:**
- The ~40 config fields (`perspective_strength`, `solo_transform`, `xaos`,
  view, camera, DOF, fog, rendering, color, tonemap, levels, options) →
  all into `config` blob.
- `parent_flame_id`, `subflame_order` → subflames live in blob.
- `flame_name` → merged into single `name` column.
- `has_3d`, `has_linked`, `final_transform_count` → derivable from
  transforms table cheaply; search uses `render_mode = '3d'` directly
  for the 3D case.
- `transform_count`, `variation_names` → derivable from transforms
  table; catalog reads pick them up via a batched aggregation query.
- `avg_color_*`, `dominant_hue` → unused.

### `transforms` (slim, root-only)

```sql
CREATE TABLE transforms (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    flame_id        UUID NOT NULL REFERENCES flames(id) ON DELETE CASCADE,
    kind            TEXT NOT NULL,             -- 'normal' | 'linked' | 'final'
    sort_order      INT NOT NULL,
    variation_names TEXT[] NOT NULL,           -- non-zero-weight only (client-filtered)
    data            JSONB NOT NULL,            -- affines, weights, params, attachments
    CONSTRAINT chk_transform_kind CHECK (kind IN ('normal', 'linked', 'final'))
);

CREATE INDEX idx_transforms_flame_id          ON transforms(flame_id);
CREATE INDEX idx_transforms_flame_id_kind     ON transforms(flame_id, kind);
CREATE INDEX idx_transforms_variation_names   ON transforms USING GIN(variation_names);
CREATE UNIQUE INDEX uq_transforms_sort_order
    ON transforms(flame_id, kind, sort_order);
```

What lives in `data JSONB`:
- Affine `a..g`, post-affine `post_a..post_g`, `post_affine_enabled`
- `weight`, `color`, `color_speed`, `opacity`, `direct_color`
- `variations` — `{ name: weight, ... }` map (including zero-weight if
  client opts to round-trip them). The `variation_names` column is the
  non-zero subset, client-filtered.
- `variation_params` — `{ "var.param": value, ... }`
- `linked_attachments`, `final_attachments` — index arrays

Nothing about the transform row is queryable except `flame_id`, `kind`,
`sort_order`, and `variation_names`. Everything else is opaque.

### Tables that **stay** unchanged
`palette_data`, `user_palettes`, `users`, `user_profiles`, `user_limits`,
`thumbnails`, `favorites`, `animation_favorites`, `tags`, `flame_tags`,
`animations`, `collections`, `collection_flames`, `translations`,
`variations`, `effects`.

The `variations` and `effects` tables still serve their catalog endpoints
(`GET /api/variations` for shader data, etc.) but are **no longer
authoritative** for flame storage — flames can reference any name,
plugin or not.

### Tables that **disappear**
- `transform_variations` — content folds into `transforms.data.variations`
- `transform_variation_params` — content folds into `transforms.data.variation_params`
- `config_effects` — content folds into `config.density_effects` /
  `config.color_effects` arrays inside the blob

### Subflames

No table rows. Subflames live as nested objects inside the parent's
`config.subflames[]` array, recursive up to depth 5. Each subflame
carries its own non-transform config **plus its own transform pools**
(normal/linked/final) inline.

Subflame transforms are **not** in the `transforms` table and **not**
in the flame's `variation_names` search corpus. A flame whose root has
no variation X but whose subflame uses X will not match a
`?variation=X` search query. Accepted trade-off: subflames are pure
config; search facets reflect the root pool only.

---

## Wire format

```jsonc
// POST /api/flames
{
    "name": "...",
    "visibility": "private",
    "palette": { "hash": "...", "name": "...", "color_data": [...] },
    "config": {
        "config_version": 2,
        // ...all non-transform flame fields...
        "subflames": [
            {
                // subflame's own non-transform fields + its transforms inline
                "render_mode": "2d",
                "transforms": [...],
                "linked_transforms": [...],
                "final_transforms": [...],
                "subflames": [...]    // nested, depth ≤ 5
            }
        ]
    },
    "transforms": [
        {
            "kind": "normal",
            "sort_order": 0,
            "variation_names": ["splits", "linear"],
            "data": { "a": 1.0, ..., "variations": {...}, ... }
        },
        { "kind": "final", "sort_order": 0, ... }
    ]
}
```

**Client does NOT send derived metadata.** No `render_mode`, no `has_3d`,
no `variation_names`, no `transform_count`. The server populates
`render_mode` and `has_subflames` on the flame row from the blob on save
(see §"Validation surface"); everything else is derived live from the
`transforms` table by the catalog/search queries.

`GET /api/flames/{id}` mirrors this back plus server-owned fields
(`id`, `user_id`, timestamps, `display_name`, `thumbnail`, `animations`,
`fork_count`, `favorite_count`, `view_count`, `animation_count`,
`featured_at`, `forked_from`).

---

## Validation surface

Server validates only what it has to. Everything else is the client's
problem.

### Server enforces

1. **DoS caps.**
   - `config` JSONB size ≤ **256 KB** (final). This is the universal
     limit — implicitly caps subflame depth and breadth too, since each
     nested subflame is at minimum a few hundred bytes of structure.
     Also covers any effects arrays inside the blob (no
     `MAX_EFFECTS_PER_STAGE` enforced server-side anymore).
   - `transforms.data` JSONB per row ≤ **16 KB**. Bounds the worst
     case for a single transform; without this, transforms aren't
     covered by the 256 KB flame blob cap (they live in their own table).
   - Total transform rows per flame ≤ `MAX_TRANSFORMS * 3` = 300
     (per-pool cap × 3 pools). Unchanged from today.
   - `xaos` and `palette` size caps unchanged from today.
   - **No `MAX_SUBFLAME_DEPTH` / `MAX_SUBFLAMES_PER_PARENT` /
     `MAX_EFFECTS_PER_STAGE`** — client owns these. The 256 KB blob
     cap is the only enforcement.

2. **Structural sanity on transform rows.**
   - `kind ∈ {normal, linked, final}` (CHECK constraint)
   - `sort_order ≥ 0`
   - No duplicate `(flame_id, kind, sort_order)` (unique index)

3. **Blob inspection on save (narrow, fixed paths).**
   - `config.render_mode` → `flames.render_mode` (typed column for search).
   - `jsonb_array_length(config -> 'subflames') > 0` → `flames.has_subflames`.

   These are the only two blob reads on the write path. Both are
   one-shot key accesses, not parses of arbitrary fields.

### Server does NOT enforce

- **Variation/effect name registry** — plugin names round-trip without
  interference. `reject_unknown_variations` is deleted. The catalog
  endpoints (`GET /api/variations`, `GET /api/effects`) still exist for
  shader/effect lookup, but they're no longer authoritative for storage.
- Numerical field ranges, enum-string correctness, defaults, field
  renames inside `config`, attachment index validity, anything else
  inside the blob.

### Server derives at read time (from the transforms table)

`FlameListItem` needs `transform_count` and `variation_names` for the
catalog UI. Both come from a single batched aggregation per page:

```sql
SELECT flame_id,
       COUNT(*) AS transform_count,
       COALESCE(array_agg(DISTINCT v ORDER BY v) FILTER (WHERE v IS NOT NULL), '{}') AS variation_names
FROM transforms
LEFT JOIN LATERAL unnest(variation_names) AS v ON TRUE
WHERE flame_id = ANY($1::uuid[])
GROUP BY flame_id;
```

One extra query per page; result map zipped onto the `FlameRow` results
by `flame_id`. No cache, no write-time maintenance, no spot-validation —
the transforms table is the only source of truth for these.

---

## Search / catalog queries

`/api/search/flames` reads `render_mode` + `name` from `flames`, and pushes
the variation and transform-count filters down to `transforms`:

```sql
SELECT f.* FROM flames f
WHERE f.user_id = $1
  AND f.render_mode = $2
  AND f.name ILIKE $3
  -- variation filter (uses GIN on transforms.variation_names)
  AND EXISTS (
      SELECT 1 FROM transforms t
      WHERE t.flame_id = f.id AND $4 = ANY(t.variation_names)
  )
  -- transform_count BETWEEN $5 AND $6
  AND f.id IN (
      SELECT flame_id FROM transforms
      GROUP BY flame_id HAVING COUNT(*) BETWEEN $5 AND $6
  )
ORDER BY f.created_at DESC;
```

The `transform_count BETWEEN` filter is the only one that pays a cost
without a cached column — it aggregates the whole transforms table.
Acceptable for low-traffic search; if it becomes hot, promote
`transform_count` back to a cached column on `flames`.

New queries the `transforms` table unlocks:

```sql
-- Final transform uses julia
SELECT DISTINCT flame_id FROM transforms
WHERE kind = 'final' AND 'julia' = ANY(variation_names);

-- Same transform has blur+bubble (or pre_blur+bubble)
SELECT DISTINCT flame_id FROM transforms
WHERE variation_names @> ARRAY['blur', 'bubble']
   OR variation_names @> ARRAY['pre_blur', 'bubble'];

-- One transform with splits, a DIFFERENT transform with elliptic
SELECT t1.flame_id FROM transforms t1
JOIN transforms t2 ON t1.flame_id = t2.flame_id AND t1.id <> t2.id
WHERE 'splits' = ANY(t1.variation_names)
  AND 'elliptic' = ANY(t2.variation_names);
```

All three are GIN-indexable; the route layer joins back to `flames` for
the `FlameListItem` projection.

---

## Migration plan

In-place break (not a `/v2/` parallel route). Same call as the palette
redesign — client + server ship together.

**Phase 1 — additive schema (no behaviour change yet)**
1. `ALTER TABLE flames ADD COLUMN config JSONB`.
2. Create the new slim `transforms_v2` table.
3. Backfill in a single migration script:
   - For each flame row, rebuild `config` from its current columns plus
     its subflame tree (recursively walked; subflame transforms inlined
     under each subflame's `transforms`/`linked_transforms`/`final_transforms`
     arrays). Migrate `config_effects` into the blob too.
   - Migrate root-flame transforms (the rows in `flames` with
     `parent_flame_id IS NULL`) into `transforms_v2`, joining
     `transform_variations` + `transform_variation_params` into the
     `data.variations` / `data.variation_params` JSONB.
   - Populate `flames.render_mode` and `flames.has_subflames` from the
     blob on each flame.
4. `ALTER TABLE flames ALTER COLUMN config SET NOT NULL`.

**Phase 2 — code switchover**
- Repo + route layer reads from `config` + `transforms_v2`. Old columns
  no longer written.
- Wire format flips to the new shape (single sharp cut).

**Phase 3 — verify**
- Confirm `flames.render_mode` matches the blob's top-level `render_mode`.
- Confirm `flames.has_subflames` matches `jsonb_array_length(config -> 'subflames') > 0`.
- Round-trip every flame: client load → re-save → byte-identical config.

**Phase 4 — destructive cleanup**
1. `DROP TABLE transform_variations`
2. `DROP TABLE transform_variation_params`
3. `DROP TABLE config_effects`
4. `DROP TABLE transforms` (old), `ALTER TABLE transforms_v2 RENAME TO transforms`
5. Drop the ~40 superseded columns on `flames`
6. Drop `parent_flame_id`, `subflame_order`, `flame_name`
7. Drop `has_3d`, `has_linked`, `final_transform_count`,
   `transform_count`, `variation_names` (cached), `avg_color_*`,
   `dominant_hue`. (Keep `has_subflames`.)
8. Drop `WHERE parent_flame_id IS NULL` filters from every catalog query
9. Delete the recursive `assemble_flame_response` Pin<Box<dyn Future>>
   — subflame assembly is now a JSON read
10. Delete `reject_unknown_variations` and the effect-name validation

---

## Resolved design choices

| | Decision |
|---|---|
| `avg_color_*`, `dominant_hue` | Drop |
| `config_version` | Lives in blob, client-owned. Server never reads it. |
| Subflame addressing (`subflame_path`) | Drop. Subflames live entirely in `config` blob, including their transforms. Not in the search corpus. |
| Route versioning | In-place break (no `/v2/` parallel) |
| Variation/effect name validation | Drop entirely. Server stores names verbatim; plugins round-trip without interference. |
| Client-sent derived metadata | None. Server extracts `render_mode` + `has_subflames` from fixed paths in the blob on save; `transform_count` and `variation_names` derived live from the transforms table at read time. |
| `has_3d` propagation through subflames | Drop. Search uses `render_mode = '3d'` directly. Subflame 3D doesn't surface to catalog. |
| `flame_name` vs `name` | Merge to single `name`. |

---

## What we're trading

| | New plan | Today |
|---|---|---|
| Migration when a config field is added/renamed | None (in blob) | ALTER + mapping code + tests |
| Migration when a per-transform field is added/renamed | None (in `data` JSONB) | ALTER + repo + tests |
| Per-transform search (`final transform uses julia`, `same transform has X+Y`) | Yes, GIN on `transforms.variation_names` | Hard |
| Variation/effect plugin support | Free — names round-trip opaquely | Blocked by registry FK + reject check |
| Server-side field validation | Drop (move to client) | Comprehensive |
| Search visibility into subflames | None — root pool only | Today's tree-walked `variation_names[]` |
| Flame GET row count | 1 flame + N transforms + 1 palette | Many across 6 tables |
| Flame WRITE complexity | flame UPSERT + delete+insert transforms | Today's multi-table cascade |
| Code size | Much smaller — drop recursive assembly, all the child-table plumbing, two validation passes | Today's |

The two explicit givebacks: subflame variations not searchable (rare),
and server-side field-level validation (client already has it).

---

## Adjacent surfaces — unchanged

The flame redesign doesn't touch these. All reference flames by UUID
and remain valid:

- Animations (`animations` table, `/api/animations` routes)
- Collections (`collections`, `collection_flames`, `/api/collections`)
- Favorites + animation favorites
- Tags + `flame_tags`
- Thumbnails
- User profiles + `user_limits`
- Variation + effect catalog endpoints (`/api/variations`, `/api/effects`)
  — still serve their data, just no longer authoritative for flame storage.

---

## Coordination with client

Before writing code, client confirms:
- `to_api_request` strips root-flame transforms out and sends them in
  the top-level `transforms[]` array; everything else (including the
  full subflame tree with its inlined transforms) goes into `config`.
- `from_api_response` reconstructs the in-memory flame by re-attaching
  root transforms to the deserialized config.
- Client filters zero-weight variations from each transform's
  `variation_names` field (used for search visibility).
- Client carries `config_version` inside the blob and handles
  forward-compat reads itself.
- Client no longer sends derived metadata (`variation_names`, `has_3d`,
  etc.) — server computes them.
- Plugin variation/effect names round-trip without server interference,
  but won't be queryable via the catalog endpoints unless the client
  also publishes them through the variations registry.

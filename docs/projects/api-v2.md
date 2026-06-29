# API v2 — client-side plan

Paired with [docs/projects/api-v2-server.md](api-v2-server.md). That doc owns the
database schema, read/write paths, validation surface, and migration; this one
owns the **wire format from the client's side**, the `FractalConfig ↔ request`
conversion, and the **version-keyed config migration** the client is responsible
for.

> **Supersedes** the old granular client/server plans. The previous `api-v2.md`
> (per-field typed schema) and `api-v2-server-side.md` (granular server schema)
> are both retired — delete `api-v2-server-side.md`.

## Why

The shipped API mirrors every `FractalConfig` field as a wire field + a DB
column + a `sync.rs` mapping. That's a **drift treadmill**: each config change
needs three coordinated edits, and the wire format is currently ~15 fields
behind (`camera_x/y/bank`, `image_size`, the Flame depth/symmetry/`preserve_z`
set, the Transform `variation_order`/`variation_priorities`/3D-affine set — all
silently dropped on save, defaulted on load).

v2 stops the treadmill: the flame config becomes an **opaque JSONB blob that is
the same JSON a `.fflame` file holds**. One serialization for local files and
the cloud; new config fields need zero API/DB work.

## Architecture (client view)

A saved flame is, on the wire:

- **`config`** — opaque blob: the whole `FractalConfig` **minus the root flame's
  transforms and minus the palette**, including the full subflame tree (each
  subflame carries its own non-transform fields *and* its own inline transform
  pools). Carries `config_version`.
- **`transforms[]`** — the **root flame's** transforms only, sent as a
  flat array of `{ kind, sort_order, variation_names, data }`. These become rows
  in the server's slim `transforms` table (GIN-indexed `variation_names` powers
  per-transform search). Subflame transforms stay inside `config`.
- **`palette`** — inline content-addressable `ApiPalette` (unchanged flow); the
  server stores `(hash, name)` on the flame row.
- **`name`, `visibility`** — the only typed flame metadata. **Single `name`** —
  the old `flame_name`/cloud-title split is gone; `Flame::name` round-trips
  through `name`.

The client sends **no derived metadata**: the server extracts `render_mode` and
`has_subflames` from two fixed blob keys on save, and derives
`transform_count`/`variation_names` live from the transforms table at read.

## Wire format

```jsonc
// POST /api/flames  (PUT is the same shape)
{
  "name": "My Flame",
  "visibility": "private",
  "palette": { "name": "...", "color_data": [/* u32 RGB */] },   // server hashes
  "config": {
    "config_version": 2,
    // every non-transform FractalConfig field, skip-if-default:
    "zoom": 2.0, "camera_x": 1.5, "image_size": [3840, 2160],
    "highlight_mode": "max_norm", "preserve_z": true, /* ... */
    "subflames": [
      { "render_mode": "2d",
        "transforms": [/* inline */],
        "linked_transforms": [], "final_transforms": [],
        "subflames": [] }      // recursive, depth ≤ 5
    ]
  },
  "transforms": [              // ROOT flame transforms only
    { "kind": "normal", "sort_order": 0,
      "variation_names": ["splits", "linear"],   // non-zero weight, client-filtered
      "data": { "a": 1.0, /* affines, post-affines, weight, color, opacity,
                  direct_color, variations, variation_params,
                  linked_attachments, final_attachments, 3D coefs, ... */ } },
    { "kind": "final", "sort_order": 0, "variation_names": [...], "data": {...} }
  ]
}
```

`GET /api/flames/{id}` returns this mirrored back plus server-owned fields
(`id`, `user_id`, timestamps, `thumbnail`, `animations`/counts, `featured_at`,
`forked_from`).

### What goes where (root transform split)

`to_api_request(config: &FractalConfig)`:
1. Run the palette through the existing inline content-addressable flow; remove
   it from the config value.
2. Pull `config.flame.transforms` / `linked_transforms` / `final_transforms`
   (the **root** pools) out into the flat `transforms[]` array, tagging each
   with `kind` + `sort_order` (array order, per pool) and a client-filtered
   `variation_names` (non-zero-weight entries). Each transform's full state goes
   in `data`.
3. Serialize the rest of `FractalConfig` — including `config.flame.subflames`
   with their transforms left **inline** — to `config`, with `config_version`.

`from_api_response(resp)`:
1. Deserialize `config` (through the migration hook below) into a
   `FractalConfig` whose root flame has empty transform pools.
2. Rebuild the root pools from `transforms[]` (bucket by `kind`, order by
   `sort_order`).
3. Reattach the palette from `palette`/`palette_hash`.

Subflame transforms need no special handling — they ride inside `config`.

## Version-keyed config migration (the one new mechanism)

Stripping defaults is already what `.fflame` does
(`#[serde(skip_serializing_if = default)]` + `#[serde(default = ...)]`), and the
blob *is* that JSON. The subtlety: serde recovers absent fields with the
**current code's** default, not the default at the version the blob was written.
So if a default ever *changes* across a version bump (e.g. `white_level`
200 → 220), an old blob that omitted the field because it equalled the *old*
default silently re-renders with the *new* one.

To honor "recover via the defaults **for the config version**" we read through a
version-keyed migration:

```rust
// pseudocode — runs on EVERY config read (cloud blob AND local .fflame)
fn load_config(mut v: serde_json::Value) -> FractalConfig {
    let version = v.get("config_version").and_then(Value::as_u64).unwrap_or(1) as u32;
    migrate_config(version, &mut v);   // v..=CURRENT, step by step
    serde_json::from_value(v)          // serde fills still-absent fields w/ current defaults
}

// each bump that CHANGES a default (or shape) adds one arm; fields whose
// default is unchanged need no entry — serde's current default is already right.
fn migrate_config(from: u32, v: &mut Value) {
    if from < 2 { /* v1 → v2: e.g. if white_level absent, set it to the v1 default */ }
    // if from < 3 { ... }  // future
    v["config_version"] = json!(CURRENT_CONFIG_VERSION);
}
```

- **Bump `CURRENT_CONFIG_VERSION` 1 → 2.** For the v1→v2 step specifically, no
  defaults change, so the arm is empty for now — we're building the **hook**,
  not migration entries. The point is that every future default change lands as
  one arm here instead of silently altering old flames.
- **This also fixes local `.fflame` loading**, which has the same latent bug
  today (load goes through the same path).
- **Forward-compat** (old client reading a *newer* blob) stays best-effort:
  serde drops unknown fields; the older client renders with what it understands.
  The migration hook only handles backward (new client, old blob), which is the
  case that matters.

## Client implementation (this repo)

1. **`src/api/types.rs`** — collapse to the shape above. Keep `ApiPalette`,
   `ApiRenderMode`, `ApiVisibility`, animation types, variation-catalog types.
   **Remove** the per-field `CreateFlameRequest`/`FlameResponse` body
   (`SubflameRequest`, and the `ApiHighlightMode`/`ApiSqueezeMode`/
   `ApiTransformKind`/`ApiPath*` enums — those now live as strings inside the
   blob). Add a small `ApiTransformWire { kind, sort_order, variation_names,
   data: Value }` for the root-transform array.
2. **`src/api/sync.rs`** — `to_api_request`/`from_api_response` become the
   palette-strip + root-transform-split + blob (de)serialize described above,
   threaded through `load_config`/migration. Delete `transform_to_api`,
   `transform_from_api`, `flame_to_subflame_request`,
   `flame_from_subflame_response`, and every per-field flame mapping — including
   all the "API doesn't carry X yet" default sites. The 15-field drift closes
   for free.
3. **`migrate_config` hook** + bump `CURRENT_CONFIG_VERSION` to 2. Route local
   `.fflame` deserialization through the same hook so both paths agree.
4. **Helpers** — `kind`/`sort_order`/filtered-`variation_names` extraction for
   the root pools (reuse existing pool/weight accessors).
5. **Tests**
   - Round-trip `FractalConfig → request → response → FractalConfig` byte-equal
     `config` for: a 3D flame (camera_x/y/bank, preserve_z, depth fades),
     subflames with their own transforms, multiple finals + linked +
     attachments, post-symmetry, `variation_order`, a custom palette.
   - Migration: a synthetic v1 blob missing a field whose default we pretend
     changed → asserts the v1 default is restored, not the current one. Guards
     the hook itself.
   - `.fflame` load goes through the same migration (no separate path).

## What stays / what we trade

Storage, search SQL, validation caps (256 KB blob, 16 KB per-transform,
`MAX_TRANSFORMS*3` rows), the dropped variation/effect registry checks, and the
DB migration phases are all in [api-v2-server.md](api-v2-server.md). The two
explicit givebacks: **subflame variations aren't searchable** (root pool only —
a real regression from today's tree-walk, accepted) and **server-side
field-level validation is dropped** (the client already validates). In return:
no migration when a config or per-transform field is added/renamed, free plugin
variation/effect round-trip, smaller client and server, and one format for local
and cloud.

## Out of scope

- Random-generator and audio-analysis settings — device-local
  (`SystemSettings`), never synced.
- Animation track schema — already opaque `serde_json::Value`.
- URL versioning (`/v2/flames`) — only for a future breaking change to the
  request *envelope*, not the config inside it.

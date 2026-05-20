# API v2 — catch the wire format up to FractalConfig

## Why

The shipped API ([docs/projects/api-integration.md](api-integration.md))
serializes `FractalConfig` into a flat typed schema —
[`CreateFlameRequest`](../../src/api/types.rs) /
[`FlameResponse`](../../src/api/types.rs) — so the server can index,
search, and migrate individual fields rather than treat flames as
opaque blobs. That's the right shape; we keep it for v2.

What it can't do today: round-trip flames that use anything added to
`FractalConfig` since the API shipped. The gap accumulated through
linked transforms, subflames, the tonemap rework, palette pipeline
extensions, and a few other landings. Saving such a flame to the
cloud silently drops information; loading defaults the missing
fields. That's the v2 work: extend the typed schema to cover the
current `FractalConfig` shape, bump the version, and document
back-compat behavior on both sides.

## Scope decisions (already made)

- **Path A — granular typed schema, not a blob.** Server keeps
  per-field columns so search and server-side migrations stay
  possible. ([api-integration.md §"Save Format"](api-integration.md)
  showed the temptation of a `config: Value` blob; rejected because
  it loses queryability.)
- **Stable IDs stay session-only.** `Transform.id`, `Flame.id`,
  `EffectInstance.id` are all `#[serde(skip)]`; the wire format uses
  array indices, matching `.fflame`. No transport for IDs needed.
- **Animations stay opaque.** `tracks`/`generators`/`base_config` are
  already `serde_json::Value` in
  [`CreateAnimationRequest`](../../src/api/types.rs) and that's
  correct — animation tracks are highly variable and target by index,
  and the server doesn't need to introspect them.
- **Effects don't have per-transform targeting.** The two effect
  chains (density / color) are global to the flame; the existing
  `CreateEffectInput { effect_name, params, enabled }` shape is
  enough.

## The gap

Fields present in `FractalConfig` /
[`Flame`](../../src/scene/transforms.rs) / `Transform` that the wire
format drops today. Each entry: client field → API field (or
`missing`) → default for back-compat on receive.

### Flame structure (the structural changes)

| Client | API today | Plan |
|---|---|---|
| `Flame.final_transforms: Vec<Transform>` | `final_transform: Option<CreateTransformInput>` (**singular**) | Rename to plural; `Vec<CreateTransformInput>`. Old clients sending the singular form: server accepts via serde alias and stores as a 1-element pool. |
| `Flame.linked_transforms: Vec<Transform>` | missing | Add `linked_transforms: Option<Vec<CreateTransformInput>>`. Default `None` ≡ empty pool. |
| `Transform.linked_attachments: Vec<usize>` | missing | Add to `CreateTransformInput` and `TransformResponse`. Default `[]`. |
| `Transform.final_attachments: Vec<usize>` | missing | Same. |
| `Flame.subflames: Vec<Flame>` | missing | Add `subflames: Option<Vec<CreateFlameRequest>>` (recursive). Subflame names default to "Untitled" client-side; the API doesn't need them but should preserve them for round-trip. |

A note on the singular→plural final-transform migration: existing
flames stored in the DB have a single `final_transform` row. The
server's v2 migration should treat that as a one-element
`final_transforms` pool. New flames written by v2 clients land in the
pool directly. Old clients reading new flames see the *first* final
in `final_transform` for back-compat — or, since old clients also
ignore unknown fields, see no final at all. Either is acceptable;
need to pick one in API implementation.

### Tonemap / color (additive)

| Client | API today | Plan |
|---|---|---|
| `highlight_mode: HighlightMode` (`Clip`/`MaxNorm`/`Reinhard`/`Filmic`) | missing | Add enum `ApiHighlightMode` + field. Default `Clip`. |
| `white_level: f32` | missing | Add field. Default `200.0` (`DEFAULT_WHITE_LEVEL`). |
| `palette: Palette` (embedded `{ name, stops }`) | `palette_id: Option<String>` only | Add `palette: Option<ApiPalette>` carrying name + stops. **Both fields coexist** — `palette_id` references the library, `palette` carries embedded custom data. If both present, `palette` wins client-side (matches `.fflame` behavior). |
| `palette_squeeze_mode: SqueezeMode` (`Linear`/`Geometric`) | missing | Add enum + field. Default `Linear`. |
| `palette_squeeze_falloff: f32` | missing | Add. Default `0.5`. |
| `palette_log_strength: f32` | missing | Add. Default `0.0`. |
| `palette_reverse: bool` | missing | Add. Default `false`. |

### Effects

`CreateEffectInput.params: Option<serde_json::Value>` is currently
opaque on the wire. Client `EffectInstance.params: HashMap<String,
f32>` is typed. The two are compatible (HashMap serializes to a JSON
object), but worth making explicit:

| Client | API today | Plan |
|---|---|---|
| `EffectInstance.params: HashMap<String, f32>` | `params: Option<serde_json::Value>` | Keep as Value on the wire (effect param schemas are per-effect-type, no point typing centrally), but document the convention: object with string keys and number values. |

Server can extract effect-type-specific indexed columns later if
search-by-effect-param becomes a thing.

### Other (verify, don't assume gaps)

- `Flame.name` vs top-level `CreateFlameRequest.name` — currently two
  different concepts. The request's `name` is the cloud-library
  title; `Flame.name` is the internal name in the .fflame. v2 should
  preserve `flame.name` separately. Add `flame_name:
  Option<String>` to the request.

## Wire-format versioning

Add a top-level integer field to `CreateFlameRequest` /
`FlameResponse`:

```rust
#[serde(default = "default_config_version")]
pub config_version: u32,  // 1 or 2

fn default_config_version() -> u32 { 1 }  // server-side default
```

Bump [`CURRENT_CONFIG_VERSION`](../../src/config/fractal_config.rs)
from 1 to 2 client-side. The client writes 2; the server stores it
on the row. The version is informational — it tells consumers what
fields to expect. Real schema enforcement still happens via column
presence + serde defaults.

**Why not Accept-Version headers**: per-endpoint header-based
versioning works for API URL versioning (`/v1/flames` vs
`/v2/flames`) but creates separate code paths server-side. An
integer field on the body keeps a single endpoint and lets old
clients gracefully degrade.

## Migration matrix

| Reader | Writer | Behavior |
|---|---|---|
| v1 client | v1 server | unchanged (today) |
| v2 client | v1 server | new fields default to client-side defaults; round-trip *loses* new fields on save until server is updated |
| v1 client | v2 server | new fields ignored (serde skips unknown); client renders as if they were defaults |
| v2 client | v2 server | full round-trip ✓ |

Forward-compat works because all new fields are added with
`#[serde(default)]` and `Option<T>` is used on Create requests.
Backward-compat works because old clients ignore unknown fields by
default.

No hard cutover required.

## Implementation order (client side)

1. **Define new Api* types in [`src/api/types.rs`](../../src/api/types.rs)**:
   - `ApiHighlightMode`, `ApiSqueezeMode`, `ApiPalette` (name + stops),
     `ApiColorStop`, `ApiTransform`-with-attachments,
     `ApiSubflame` (recursive wrapper around the existing flame
     schema).
2. **Extend `CreateTransformInput` + `TransformResponse`**: add
   `linked_attachments`, `final_attachments`. Both `Option<Vec<usize>>`
   on input, default `[]` on response.
3. **Extend `CreateFlameRequest` + `FlameResponse`**: add the gap
   list above plus `config_version` plus `linked_transforms`,
   `final_transforms` (Vec<>, deprecate singular `final_transform`),
   `subflames`, `flame_name`. Preserve the old `final_transform`
   field on the response for back-compat reads.
4. **Update [`src/api/sync.rs`](../../src/api/sync.rs)** —
   `to_api_request` and `from_api_response` learn the new fields.
   The serde defaults already in place on `FractalConfig` cover the
   missing-on-read case.
5. **Bump `CURRENT_CONFIG_VERSION` to 2.** Client always emits
   `config_version: 2`.
6. **Tests** — at least one round-trip test per new field. Use the
   existing visual regression baselines (with `highlight_mode =
   MaxNorm`, with subflames, with a custom palette, etc.) to confirm
   nothing is silently dropped.

## API-side checklist (handoff for server work)

Implementation details for the server team. Schema migrations are
ordered.

1. **`flames` table**: add columns
   - `config_version INT NOT NULL DEFAULT 1`
   - `flame_name TEXT` (separate from cloud-library title)
   - `highlight_mode TEXT NOT NULL DEFAULT 'clip'`
     (`clip`/`max_norm`/`reinhard`/`filmic`)
   - `white_level REAL NOT NULL DEFAULT 200.0`
   - `palette_squeeze_mode TEXT NOT NULL DEFAULT 'linear'`
   - `palette_squeeze_falloff REAL NOT NULL DEFAULT 0.5`
   - `palette_log_strength REAL NOT NULL DEFAULT 0.0`
   - `palette_reverse BOOLEAN NOT NULL DEFAULT false`
   - `embedded_palette JSONB` (nullable, custom palette data when
     `palette_id` is NULL)

2. **`transforms` table**: change shape
   - Add `transform_kind TEXT NOT NULL DEFAULT 'normal'`
     (`normal`/`linked`/`final`). Migrate existing
     `is_final_transform = true` rows to `transform_kind = 'final'`,
     `false` → `'normal'`.
   - Drop `is_final_transform` after migration (or keep as computed
     view for back-compat).
   - Add `linked_attachments JSONB` (array of int) — indexes into
     the parent flame's `linked_transforms` pool.
   - Add `final_attachments JSONB` (array of int).

3. **New `subflames` table** (or denormalized JSONB column on
   `flames`):
   - Subflames are recursive `Flame` instances. Easiest first cut:
     `subflames JSONB` column on `flames` holding the array of
     nested flame JSON. Indexing into the subflame pool isn't a
     common query; revisit if it becomes one.
   - Tradeoff: blob in a relational schema. Acceptable here because
     subflames are referenced by the `subflame_wf` variation by
     index, the parent doesn't query into them.

4. **Variations table — `subflame_wf`**: ensure this variation is
   registered. It exists client-side; the server's variations table
   should list it as a known variation name.

5. **Indexed metadata extraction** (extracted at write time, used by
   search):
   - `has_subflames BOOL` — true if `subflames` array non-empty
   - `has_linked BOOL` — true if `linked_transforms` non-empty
   - `final_transform_count INT` — array length
   - All extracted server-side from the JSON on insert/update.

6. **API endpoints — no URL changes.** Existing `/api/flames` POST
   accepts both v1 and v2 bodies. `config_version` is informational
   for the server; the actual contract is "all v2 fields are
   `Option<>` in the request, defaults applied at write time."

7. **Validation rules**:
   - `linked_attachments[i]` and `final_attachments[i]` must be in
     range of the respective pools. Reject with 400 on out-of-range.
   - `subflames` recursion depth: cap at, say, 4. Reject deeper.
   - `highlight_mode` must be one of the four valid strings.

8. **Search params** — extend
   [`SearchFlamesParams`](../../src/api/types.rs) on both sides
   with optional filters:
   - `has_subflames: Option<bool>`
   - `has_linked: Option<bool>`
   - `highlight_mode: Option<ApiHighlightMode>`

## Open questions

- **Singular `final_transform` deprecation**: keep the old field on
  responses indefinitely, or sunset after some grace period? I'd
  lean keep — the cost is one nullable column, the cost of removing
  is breaking old WASM bundles in the wild.
- **Palette: embedded vs library reference precedence**: if both
  `palette_id` and `palette` are present, which wins? Client picks
  embedded (matches `.fflame`). Worth documenting on the server too.
- **Subflame storage**: JSONB blob inside `flames` row vs separate
  `subflames` table with foreign key. Blob is simpler, table is
  more relational. Pick before implementing.

## Out of scope (v2 doesn't tackle these)

- Real schema versioning beyond `config_version: u32`. If we ever
  need *breaking* migrations (rename, remove a field), we'll need
  URL versioning (`/v2/flames`) at that point. Not now.
- Random generator settings — not in `FractalConfig`, lives on
  `SystemSettings` (device-local), not synced.
- Audio analysis settings — same, device-local.
- Animation track schema versioning — animations are opaque, will
  evolve independently.

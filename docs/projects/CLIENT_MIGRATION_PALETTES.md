# Client Migration: Palette Redesign

Tracks the breaking changes to the palette API. The API server has shipped
migration `20250101000063_palette_inline_redesign.sql` which:

- drops the per-user `palettes` wrapper table,
- denormalizes `palette_hash` + `palette_name` onto every flame row,
- introduces a `user_palettes` bookmark table (no effect on rendering).

Palettes are now content-addressable, anonymous, and embedded inline in
flame requests/responses. There is no per-palette ownership or visibility.

---

## 1. Flame payloads

### Before
```json
{
  "name": "my-flame",
  "palette_id": "8c8e0a3b-...uuid...",
  ...
}
```

### After
```json
{
  "name": "my-flame",
  "palette": {
    "hash": "abc123...sha256",         // optional if color_data/stops are sent
    "name": "Sunset",                  // optional flame-specific display name
    "color_data": [1, 2, 3, ...],      // optional; required if hash is absent or unknown
    "stops": { ... }                   // optional
  },
  ...
}
```

**Server resolution rules** (POST/PUT `/api/flames`):

1. If `palette` is omitted or `null` → flame has no palette.
2. If `color_data` and/or `stops` are present → server computes the canonical
   SHA-256 hash, upserts `palette_data`, and stores `(hash, name)` on the
   flame. Any client-supplied `hash` is **ignored** in this case.
3. If only `hash` is provided → server verifies it exists in `palette_data`;
   if not, returns `400 Validation error: palette_hash_unknown`.

**Recommendation:** When saving a flame, send `palette.hash` if you already
know the palette is registered; otherwise send the full content. The server
will deduplicate by hash on the backend.

### Flame response
The `palette_id` field is gone. Flames now embed:

```json
{
  "palette": {
    "hash": "abc123...sha256",
    "name": "Sunset",
    "color_data": [1, 2, 3, ...],
    "stops": null,
    "avg_color_r": 0.5,
    "avg_color_g": 0.3,
    "avg_color_b": 0.1,
    "dominant_hue": 25.0,
    "color_count": 256
  }
}
```

`palette` is `null` when the flame has no palette assigned.

---

## 2. Palette endpoints

| Old endpoint                          | New endpoint                                  | Notes                          |
| ------------------------------------- | --------------------------------------------- | ------------------------------ |
| `GET /api/palettes`                   | `GET /api/users/me/palettes`                  | Personal library only          |
| `GET /api/palettes/{uuid}`            | `GET /api/palettes/{sha256}`                  | Public, no auth                |
| `POST /api/palettes`                  | `POST /api/palettes`                          | Submit content; see below      |
| `PUT /api/palettes/{uuid}`            | `PUT /api/users/me/palettes/{sha256}`         | Library nickname only          |
| `DELETE /api/palettes/{uuid}`         | `DELETE /api/users/me/palettes/{sha256}`      | Removes library entry only     |

### `GET /api/palettes/{hash}`
**Public**, no auth required. Returns the palette content (no name, no owner):

```json
{
  "hash": "abc123...",
  "color_data": [...],
  "stops": null,
  "avg_color_r": ...,
  ...
}
```

### `POST /api/palettes`
**Authenticated.** Submit content; optionally bookmark it in caller's library.

```json
// Request
{
  "color_data": [...],            // required (at least one of color_data/stops)
  "stops": { ... },               // optional
  "nickname": "Sunset"            // optional; bookmarks in caller's library
}

// 201 Created (new content) or 200 OK (already existed)
{
  "palette": { "hash": "...", "color_data": [...], ... },
  "added_to_library": true        // only true when nickname was set AND it was a new bookmark
}
```

`added_to_library` is `false` if the caller didn't pass a nickname OR the
palette was already in their library (existing nicknames are NOT overwritten
by `POST /api/palettes` — use `PUT /api/users/me/palettes/{hash}` to rename).

### `GET /api/users/me/palettes`
**Authenticated.** Lists the caller's bookmarked palettes. Pagination via
`?page=` and `?per_page=`.

```json
[
  {
    "hash": "...",
    "nickname": "Sunset",         // user's personal name; null if untitled
    "added_at": "2026-05-22T...",
    "color_data": [...],
    "stops": null,
    "avg_color_r": ...
  }
]
```

### `PUT /api/users/me/palettes/{hash}`
**Authenticated.** Adds or updates a library entry. `404` if the hash is not
in `palette_data` yet (submit content via `POST /api/palettes` first).

```json
// Request
{ "nickname": "Inferno" }     // optional; absent leaves existing nickname unchanged
```

### `DELETE /api/users/me/palettes/{hash}`
**Authenticated.** Removes the caller's library entry. `204` on success,
`404` if no entry. Never deletes the underlying `palette_data` — content is
shared and reference-counted by flames.

---

## 3. Removed concepts

The following concepts no longer exist on the palette API. Strip any client
code that depends on them:

- `palette.id` (UUID) — replaced by `palette.hash` (SHA-256 hex string).
- `palette.owner_id` — palettes have no owner.
- `palette.visibility` — palettes are addressable by hash to anyone who has
  the hash. "Private" palettes are now just private flames that happen to
  reference them.
- `palette.metadata` — dropped from the schema. If clients were storing
  structured metadata, fold it into your client-side state.
- `palette_id` field on `CreateFlameRequest` / `FlameResponse`.

---

## 4. Naming model

Three layers — none of them affect the hash (which is purely the colors):

1. **`flame.palette.name`** — the flame's display name for its palette.
   Travels with the flame; what's shown when rendering a flame card.
2. **`user_palettes.nickname`** — the caller's personal label in their
   library. Affects only the `/api/users/me/palettes` listing.
3. **No global canonical name.** The hash is identity; everyone is free to
   call the same palette whatever they want.

---

## 5. Validation error codes the client should handle

| Code                              | Source                              | Meaning                                                       |
| --------------------------------- | ----------------------------------- | ------------------------------------------------------------- |
| `palette_content_required`        | `POST /api/palettes`                | Neither `color_data` nor `stops` provided                     |
| `palette_hash_unknown`            | Flame create/update with hash-only  | The referenced hash is not in `palette_data`; send content    |
| `color_data_too_large`            | `POST /api/palettes`, flame create  | `color_data` exceeds `MAX_PALETTE_BYTES` (65,536)             |
| `stops_too_large`                 | `POST /api/palettes`, flame create  | `stops` JSON exceeds `MAX_JSON_VALUE_BYTES` (32,768)          |
| `nickname_too_long`               | `POST /api/palettes`, `PUT library` | `nickname` exceeds `MAX_NAME_LEN` (128)                       |

---

## 6. Suggested client-side rollout

1. **Saving flames:** switch to embedding `palette: { hash?, name?, color_data?, stops? }`.
   Always send `color_data`/`stops` on the first save of a palette new to the
   server; afterwards just send the `hash`.
2. **Loading flames:** read `response.palette.{hash, name, color_data, stops}`
   instead of looking up by `palette_id`. No separate palette fetch is
   needed for display.
3. **Library UI:** repoint browse/list to `GET /api/users/me/palettes`,
   add/remove via `PUT`/`DELETE /api/users/me/palettes/{hash}`. Drop any UI
   for palette visibility or ownership transfer.
4. **Shared palettes:** to share a palette with another user, just share the
   hash (or share a flame that uses it). The other user can fetch content
   from `GET /api/palettes/{hash}` and optionally bookmark it.

# The Catalog's API: the route table (phase 0)

> **Status 2026-11-16: a proposal for the user to agree.** Nothing is
> built. Phase 0 of [catalog.md](catalog.md): the ten principles there
> made into routes. Once agreed, it is built on an ffapi branch (with
> ffapi's permission matrix and tests) and the client moves onto it in
> phase 3. The current routes are from the ffapi survey of 2026-11-16
> (catalog.md, *In the API*).

## How old and new live together

**New routes under `/api/v2/`; every current route stays where it is.**
Released apps keep calling the current routes, unchanged. The new app
calls `/api/v2/`. The two are separate questions:

- **`/api/v2/`** is the API's *shape*: paths, the list envelope, the
  query words, the errors.
- **`?reader=N`** stays what a client can *open* (config versions,
  script kinds, multi-pass effects). Every v2 route takes it as v1 routes
  do. The app moves to reader 8 when it starts using v2.

This replaces principle 10's "new shapes behind reader 8": one path
meaning two things by a parameter would be harder to read, document and
test than two prefixes. **Decision 1.**

A v2 route is a thin layer over the same handlers and repositories as
its v1 route, so nothing is stored twice and v1 cannot drift from v2's
data.

## Conventions every v2 route keeps

**Paths.** For each resource `R`:

| Route | Who | What |
|---|---|---|
| `GET /api/v2/R` | anyone | public items: browse and search |
| `GET /api/v2/users/me/R` | signed in | your own, every visibility |
| `GET /api/v2/users/{id}/R` | anyone | someone's public items |
| `GET /api/v2/R/{id}` | as the item allows | one item, with its content |
| `POST /api/v2/R` | signed in | create |
| `PUT /api/v2/R/{id}` | owner | replace |
| `PATCH /api/v2/R/{id}` | owner | name, description, visibility |
| `DELETE /api/v2/R/{id}` | owner | delete |
| `GET /api/v2/R/{id}/thumbnail` | as the item allows | an image, where `R` has them |

**One list envelope**:
`{"items": [...], "total": 123, "page": 1, "per_page": 50}`.

**One query vocabulary** on every list:

| Word | Meaning |
|---|---|
| `q` | search: name, title and description, case-insensitive |
| `kind` | one of the kinds below (where a resource has kinds) |
| `sort` | `recent` (created), `updated`, `name`, `popular` (favourites), `views`, `featured`; each resource says which it takes |
| `featured` | `true`: featured items only |
| `page`, `per_page` | from 1; `per_page` 1-100, default **50** everywhere |
| `engine`, `scene`, `dimension`, `type`, `role`, `version` | the catalog filters, as now, on fractals |

An unknown word's value (a sort, a kind) is a **400**, as an unknown
filter is now; nothing silently falls back.

**One kind vocabulary**, the scripts' 13 and the built-in-only one:
`generator`, `modifier`, `library`, `formula`, `coloring`, `field`,
`field_coloring`, `solid`, `solid_coloring` (built-in only), `variation`,
`effect`, `sim_model`, `sim_coloring`, `track`. A reader too old for a
kind does not see items of it, as now.

**One item shape.** Every item has `id`, `name`, `owner` (`{"id",
"display_name"}`, absent for built-ins), `visibility` (where a resource
has it), `created_at`, `updated_at`; and, where a resource has them,
`kind`, `description`, `authors`, `version`, `has_thumbnail`. One owner
object replaces `display_name` (fractals, animations) and
`owner_display_name` (scripts, rooms).

**One error shape**: `{"error": "kind_invalid", "message": "Unknown kind
'x'"}` -- a stable code to branch on, and text to show. v1 keeps
`{"error": "Validation error: kind_invalid"}`. **Decision 2.**

**One conflict.** A stale `version` on a PUT is a **409**
`{"error": "conflict", "message": ..., "current_version": 4,
"updated_at": ...}`, for every resource that has versions (scripts now).

## The routes

### Fractals (flames, escape-time and simulation configs)

| v2 | Replaces (v1) | Notes |
|---|---|---|
| `GET /v2/flames` | `GET /explore`, `GET /explore/featured` | public; `q`, the catalog filters, `render_mode`, `sort` = `recent`/`popular`/`views`/`featured`, `featured` |
| `GET /v2/users/me/flames` | `GET /flames`, `GET /search/flames` | your own; the same words, so search stops being a separate route |
| `GET /v2/users/{id}/flames` | `GET /users/{id}/flames` | |
| `GET /v2/flames/{id}` | `GET /flames/{id}` | |
| `POST`, `PUT`, `PATCH`, `DELETE /v2/flames/{id}` | the same | |
| `GET`, `PUT`, `DELETE /v2/flames/{id}/thumbnail` | the same | the PUT takes no size (the client sent one the server ignored) |
| `GET /v2/flames/{id}/animations` | the same | enveloped |
| favourites, tags, fork | the same paths under `/v2` | |

The list item gains `has_thumbnail` and `description` (**decision 3**:
a fractal has no description today; the Catalog shows one where it
exists).

### Animations

| v2 | Replaces (v1) | Notes |
|---|---|---|
| `GET /v2/animations` | `GET /explore/animations` | public; gains `q` |
| `GET /v2/users/me/animations` | `GET /animations` | |
| `GET /v2/users/{id}/animations` | none | new, as for every resource |
| item routes, favourites, fork | the same paths under `/v2` | |

### Palettes

Palettes stay content-addressed: the hash is the content's id, and the
app computes it (`api::sync::palette_content_hash`). What is new is the
**published palette** (principle 6): your library entry for a palette
gains a name, a description and a visibility, and a public one is
listed. **Decision 4.**

| v2 | Replaces (v1) | Notes |
|---|---|---|
| `GET /v2/palettes` | none | public entries: `q` on name and description, `sort` = `recent`/`name`/`popular`; each item the hash, name, description, owner, colours |
| `GET /v2/users/me/palettes` | `GET /users/me/palettes` | your library, every visibility |
| `GET /v2/users/{id}/palettes` | none | someone's public entries |
| `GET /v2/palettes/{hash}` | `GET /palettes/{hash}` | the content |
| `POST /v2/palettes` | `POST /palettes` | submit content; with a name, it joins your library |
| `PATCH /v2/users/me/palettes/{hash}` | `PUT /users/me/palettes/{hash}` | name, description, visibility (publishing is making it public) |
| `DELETE /v2/users/me/palettes/{hash}` | the same | leaves the content |

The same palette published by two people is two entries with one hash.

### Rhai scripts (every kind)

| v2 | Replaces (v1) | Notes |
|---|---|---|
| `GET /v2/scripts` | `GET /search/scripts` | public; `q`, `kind`, `sort` = `recent`/`updated`/`name`/`featured`, `featured` |
| `GET /v2/users/me/scripts` | `GET /scripts` | |
| `GET /v2/users/{id}/scripts` | none | |
| `GET`, `POST`, `PUT`, `DELETE /v2/scripts/{id}` | the same | PUT's 409 is the conflict above |
| `POST /v2/scripts/{id}/fork`, `publish`; `GET /v2/scripts/resolve/{ref}` | the same | CPU kinds, as now |

### Variations and effects

| v2 | Replaces (v1) | Notes |
|---|---|---|
| `GET /v2/variations` | `GET /variations` | enveloped, paged (the app walks the pages: ~650 is 7 pages of 100); `q`, `category`, `sort` = `name`/`updated`; takes the reader; items gain `authors`, `description_plain`, `created_at`, `updated_at` |
| `GET /v2/variations/{name}` | the same | |
| `GET /v2/effects` | `GET /effects` | the same treatment; `category` is the stage |
| `GET /v2/effects/{name}` | the same | |

Their `version` is the shader's; a prose edit moves `updated_at` only.

### Built-ins: the engine's types

| v2 | Replaces (v1) | Notes |
|---|---|---|
| `GET /v2/types` | `GET /types` | **every** built-in the engine catalog names (formulas, colourings, fields, solids and their colourings, simulation models and colourings, variations, effects), with its display name and description, and the scripts public configs use; each with its public-config `count` (0 for unused, where v1 leaves it out). `kind`, `engine`, `origin` (`builtin`, `script`), `q`, `sort` = `name`/`popular` |
| `GET /v2/types/{kind}/{key}` | none | one type, with the configs that use it (`GET /v2/flames?type=...` for the full list) |

The descriptions come from the engine catalog, which takes them from
the twins' `INFO.description` (the next client step).

### The browser app's resources

Collections, rooms, gallery items, favourites and tags follow the same
conventions under `/v2` -- own lists under `/users/me`, envelopes, the
query words -- but nothing in the Catalog calls them; they move when the
browser app moves.

## The database

- `user_palettes`: `description`, `visibility` (default private),
  `updated_at`; `nickname` read as the name.
- `flames`: `description` (nullable) -- if decision 3 is yes.
- `variations`, `effects`: `created_at`, `updated_at`.
- Counting for `total`: the list queries' `COUNT(*) OVER ()`, one round
  trip.

## Fixed on the client meanwhile (phase 3)

- `search_public_flames` calls the own-only search without auth and has
  no callers: removed; public search is `GET /v2/flames?q=`.
- The thumbnail PUT's ignored size parameters: dropped.

## Decisions for the user

1. **`/api/v2/` beside the current routes**, rather than reader-gated
   shapes on the same paths. Recommended.
2. **The error shape** `{"error": code, "message": text}` in v2.
   Recommended.
3. **A description on fractals** (nullable, set from Save Online and the
   Catalog later). Recommended: yes -- the Catalog and the browser show
   prose for every other kind.
4. **Published palettes as library entries with a visibility** (one
   palette, many named entries). Recommended.
5. **`per_page` capped at 100 for everything**, the app walking the pages
   of the variation and effect catalogs, rather than a larger cap for
   those two. Recommended: one rule.

# The Catalog's API: the route table (phase 0)

> **Status 2026-11-16: agreed; building** on ffapi branch
> `catalog-api`. Phase 0 of [catalog.md](catalog.md). The first draft put new
> routes under `/api/v2/` beside the current ones; the user then said
> the API has **not launched** -- `?reader=N` exists so it never needs a
> `/v2` for content changes -- so this draft reshapes the routes **in
> place**, before launch, and both clients (this app and the browser app,
> `ffbrowser`) move with the server in one release. An independent review
> of the first draft against ffapi's code (2026-11-16) found what would
> have forced a breaking change after launch; its findings are folded in
> and cited where they change the design.
>
> Agreed in the first round and kept: the error shape, a description on
> fractals, `per_page` capped at 100, markdown descriptions. The first
> round's decision 4 (published palettes as library rows) is reversed
> here -- see *Palettes* -- and needs the user's agreement again.

## The aim

Launch once, then only add. Everything below is chosen so that what the
Catalog needs later -- browsing every kind, your own resources edited
from two clients, previews, sync -- arrives as new fields, values and
routes, never as a changed one.

## The stability contract

What the API promises from launch, written down so both clients can
rely on it:

1. **Additive only.** After launch a route, field, value or meaning is
   never removed or changed. New fields, new enum values, new routes and
   new optional parameters may appear at any time.
2. **Clients ignore what they do not know**: unknown fields are skipped,
   unknown enum values are kept and shown as unknown (this app's
   `wire_enum!` already does: `Unknown(String)`, `src/api/types.rs`). The
   browser's code may not switch on an enum without a default branch.
3. **New content arrives behind `reader`.** A new kind, render mode,
   visibility or any value an older client could not open appears only to
   clients whose `?reader=N` is high enough, as config versions and script
   kinds already do. A new resource kind is a new route and a new `kind`
   value, behind a reader.
4. **The server refuses what it does not understand**: an unknown query
   word (other than `reader` and `locale`) or an unknown value of a known
   word is a **400**. Today both are ignored (serde's `Query`) or fall
   back silently (`repositories/flames.rs`'s `_ =>` sort arm), so a client
   asking for a filter the server lacks gets an unfiltered list and cannot
   tell.
5. **Error codes are an open set.** A client branches on the HTTP status,
   then on `error`; an error may carry more members than its code needs.
6. **404 means unavailable**: deleted, private and moderated look alike,
   by design. A client keeps what it adopted and marks it unavailable.
7. **`/api/admin` is outside the promise**: it serves the browser's admin
   pages only, and may change with them.

**The path carries a version from launch: `/api/v1/`** (**decision A**).
It costs nothing now; if a `/v2` is ever needed, it sits beside `/v1`
instead of displacing unversioned routes. `reader` stays the content
gate; `/v1` would only ever change for a reshaping on this scale.

## Conventions

### Paths

For each resource `R`:

| Route | Who | What |
|---|---|---|
| `GET /api/v1/R` | anyone | public items: browse and search |
| `GET /api/v1/users/me/R` | signed in | your own, every visibility |
| `GET /api/v1/users/{id}/R` | anyone | someone's public items |
| `GET /api/v1/R/{id}` | as the item allows | one item, with its content |
| `POST /api/v1/R` | signed in | create |
| `PUT /api/v1/R/{id}` | owner | replace |
| `PATCH /api/v1/R/{id}` | owner | name, description, visibility |
| `DELETE /api/v1/R/{id}` | owner | delete |

Every resource has every row that applies; the tables below list only
what differs or is new. **`{id}` is always the item's `id` field**: a
UUID, or a name for the built-in catalogs (variations, effects), never
something else in the path than in the body.

### Lists

```json
{ "items": [...], "next": "opaque-cursor-or-null", "total": 123 }
```

- **`next`** is an opaque cursor; a client pages by sending it back as
  `cursor=`, and stops at `null`. Clients must page this way. It lets
  public feeds move to keyset paging later without a change; `page` is
  accepted too, for jumping.
- **`total`** is optional and may be capped (documented as "at least").
  A count over a public feed materialises every matching row
  (`COUNT(*) OVER ()` defeats the index's `LIMIT`, and runs the
  readability check a row), so the server may leave it out.
- **Every sort ends with `id`**, so equal keys have one order: today's
  public sorts tie (`created_at`, `favorite_count, created_at`), and the
  built-in scripts were imported in one transaction with one
  `created_at`, so offset paging over them already repeats and skips.
  Clients de-duplicate by `id` all the same.
- `per_page` 1-100, default 50, everywhere.

### Query words

| Word | Meaning |
|---|---|
| `q` | search the name and the description's plain text, case-insensitive |
| `kind` | one of the kinds below |
| `sort` | `recent` (became public), `updated`, `name`, `popular` (favourites), `views`; each resource lists which it takes |
| `featured` | `true`: featured items only (a filter; there is no `sort=featured`) |
| `cursor`, `page`, `per_page` | paging, above |
| `updated_since` | items changed after a time, for sync |
| `ids` | up to 100 ids, for fetching many at once |
| `locale` | the language of names and prose; the reply says which it served |
| `engine`, `scene`, `dimension`, `type`, `role`, `version` | the catalog filters, as now (fractals, types) |

`recent` is "became public" (a script's `published_at` where it has
one), not "created": a script made private long ago and published today
belongs at the top.

**Kinds**: `generator`, `modifier`, `library`, `formula`, `coloring`,
`field`, `field_coloring`, `solid`, `solid_coloring`, `variation`,
`effect`, `sim_model`, `sim_coloring`, `track` -- one vocabulary for
scripts and types alike (today 13 script kinds and 9 catalog kinds).

### Items

Every item has:

| Field | Meaning |
|---|---|
| `id` | the path key |
| `origin` | `builtin` or `user` -- explicit, because the built-in scripts are rows owned by the admin account and a client must not learn that account's id |
| `owner` | `{ "id", "display_name" }`, absent for built-ins (one object for today's `display_name` and `owner_display_name`) |
| `name` | |
| `description` | markdown (below), possibly empty |
| `description_plain` | its plain text, made by the server, with every description of every kind |
| `created_at`, `updated_at` | |
| `revision` | a mutable item's revision, for writes (below) |
| `thumbnail_url` | a URL, or null; it changes when the image does, so it can be cached forever |

and, where a resource has them: `kind`, `authors`, `license` (a user
item's, below), `version` (a built-in
catalog item's content version, a different thing from `revision`),
`visibility`, `published_at`, `forked_from`, `tags`, and -- in the
owner's own view only -- `moderation_state` (own lists include moderated
items today, but the item does not say so, and the owner's GET is a
403).

Clients use `thumbnail_url` and never build a thumbnail URL: today
fractals have `GET /flames/{id}/thumbnail` and scripts a third shape
(`preview_item_hash`, `preview_has_thumbnail`); a URL lets thumbnails
move to a CDN, gain sizes, and appear for kinds that have none yet (the
Catalog's phase 6) without new routes.

### Writes

- **Every mutable item has a `revision`.** `PUT` and `PATCH` take it
  (in the body, or `If-Match`); a stale one is a **409**. Optional on the
  wire, so a client that does not send it overwrites, as today; but the
  token exists from launch, because phase 4 edits from two clients and
  requiring a token later would be a change. Today only scripts check
  versions; fractals, animations and palettes are last-writer-wins.
- **Deleting keeps a tombstone** of the id: an id is never reused. Today
  a deleted GPU script frees its id, anyone may then publish another
  under it (`script_id_taken` is "the first to publish it owns it"), and
  `link_catalog` points every public config's `script:<id>` at the new
  owner's code. Lists asked with `updated_since` also return the ids
  deleted since (`"deleted": [...]`).
- **Conditional requests**: lists and items send an `ETag`; a request
  with `If-None-Match` gets a **304**. Nothing sends one today.

### Errors

```json
{ "error": "kind_invalid", "message": "Unknown kind 'x'" }
```

A stable code and text to show. A **409** adds what the client needs to
resolve it: `{"error": "conflict", "message": ..., "current_revision": 7,
"updated_at": ...}`; every taken name or id is a 409
(`script_name_taken` is a 400 today, `script_id_taken` a 409). A **429**
(`rate_limited`) carries `Retry-After`; there is no limiting in the
server today, only nginx's, but the reply's shape is fixed now.

### Markdown

Every description is markdown, and this is the contract (the app's
`src/markdown.rs` and `src/ui/markdown.rs` implement it):

- **CommonMark, with tables and strikethrough**, nothing else.
- **Raw HTML is stored but never interpreted**: it reads as text.
- **Images show their alternative text only** -- a user's description
  must not become a tracking pixel.
- **Links are clickable only when `http` or `https`**, and carry
  `rel="nofollow ugc noopener"`; any other is shown as text.
- **At most 4,096 characters** (as `validation.rs` counts).
- **`description_plain`** is made by the server with the same parser and
  options (ffapi gains `pulldown-cmark`), and `q` searches it.

The browser renders with `react-markdown`, which skips raw HTML and
`javascript:` links by default but **draws images**: it must disallow
`img` and filter links to the contract.

### Discovery

`GET /api/v1/meta?reader=N` says what this server offers that client:
the kinds, each resource's sorts and filters, the limits (`per_page`, a
description's length), the markdown flavour. A new kind or filter then
reaches the clients' lists without a code change.

## The routes

### Fractals (flames, escape-time and simulation configs)

| Route | From today's | Notes |
|---|---|---|
| `GET /flames` | `GET /explore`, `GET /explore/featured` | public; `q`, the catalog filters, `render_mode`, `sort` = `recent`/`popular`/`views`, `featured` |
| `GET /users/me/flames` | `GET /flames`, `GET /search/flames` | your own; the same words, so search is not a separate route. The own search's `min_transforms`, `max_transforms` and `has_3d` stay as filters; `variation=X` becomes `type=variation:X` |
| `GET /users/{id}/flames` | the same | |
| `GET /flames/{id}` and the writes | the same | `PATCH` takes name, description, visibility |
| `PUT`, `DELETE /flames/{id}/thumbnail` | the same | the PUT takes no size (the app sent one the server ignored); reading is `thumbnail_url` |
| `GET /flames/{id}/animations` | the same | enveloped |
| favourites, tags, fork | the same | |

A fractal gains `description` (agreed).

### Animations

| Route | From today's | Notes |
|---|---|---|
| `GET /animations` | `GET /explore/animations` | public; gains `q` |
| `GET /users/me/animations` | `GET /animations` | |
| `GET /users/{id}/animations` | none | as for every resource |

### Palettes

A palette's **content** stays content-addressed -- the hash is the
content's id, which the app computes (`api::sync::palette_content_hash`)
-- but a **published palette is its own item** (**decision B**,
reversing the first round's decision 4). As a library row keyed
`(user_id, palette_hash)` with no id of its own, a published palette
could not keep its name, description, favourites or reports through an
edit of its colours (an edit changes the hash), could not say "update
available" to someone who adopted it, and could not be reported or
moderated (reports take a UUID; `admin::table_for` has no palette arm;
the row has no `moderation_state`) -- public user text with no
moderation path.

| Route | From today's | Notes |
|---|---|---|
| `GET /palettes` | none | public palettes: `q`, `sort` = `recent`/`name`/`popular`; `hash=` finds the entries for one content |
| `GET /users/me/palettes` | `GET /users/me/palettes` | yours, every visibility |
| `GET /users/{id}/palettes` | none | |
| `GET /palettes/{id}` | none | one palette: name, description, owner, `content_hash` and the colours |
| `POST /palettes`, `PUT`, `PATCH`, `DELETE /palettes/{id}` | `POST /palettes`, `PUT`/`DELETE /users/me/palettes/{hash}` | a `PUT` of new colours keeps the id |
| `GET /palette-data/{hash}` | `GET /palettes/{hash}` | the content alone |
| favourites | none | keeping someone else's palette is a favourite, as for fractals |

A palette has a UUID, owner, name, description, visibility,
`moderation_state`, `content_hash`, `forked_from`, timestamps and a
revision. Today's library rows become private palettes, their nicknames
their names.

### Rhai scripts (every kind)

| Route | From today's | Notes |
|---|---|---|
| `GET /scripts` | `GET /search/scripts` | public; `q`, `kind`, `sort` = `recent`/`updated`/`name`, `featured` |
| `GET /users/me/scripts` | `GET /scripts` | |
| `GET /users/{id}/scripts` | none | |
| `GET`, `POST`, `PUT`, `PATCH`, `DELETE /scripts/{id}` | the same, and a new `PATCH` | the 409 above |
| `POST /scripts/{id}/fork`, `GET /scripts/resolve/{ref}` | the same | |
| `GET /scripts/{id}/versions` | none | the snapshots `script_snapshots` already keeps |
| `POST`, `DELETE /museum/scripts/{id}` | `POST`, `DELETE /scripts/{id}/publish` | the museum's pin, renamed: **"publish" means only making something public**, as it does for every other resource; today's `publish` snapshots a CPU script for the museum and refuses GPU kinds |

### Variations and effects

| Route | From today's | Notes |
|---|---|---|
| `GET /variations` | `GET /variations` | enveloped; `q`, `category`, `sort` = `name`/`updated`, `updated_since`; takes the reader; items gain `authors`, `description_plain`, timestamps |
| `GET /variations/{name}` | the same | |
| `GET /effects`, `GET /effects/{name}` | the same | `category` is the stage |

**`id` is the name** for both. Today they serve a UUID `id` made per
database (`gen_random_uuid()`, migration 7), which reseeding replaces
(migration 110 deletes removed effects outright); it is not served.
`version` is the shader's; a prose edit moves `updated_at` only. With
`updated_since` and `ETag`, the app's session start is a 304, not a walk
through seven pages.

### Built-ins: the engine's types

| Route | From today's | Notes |
|---|---|---|
| `GET /types` | `GET /types` | every built-in the engine catalog names, with its display name and description, and every script and source type public configs use; each with its public-config `count` (0 for unused, which today's leaves out). `kind`, `engine`, `origin` (`builtin`, `script`, `source`), `q`, `sort` = `name`/`popular` |
| `GET /types/{type}` | none | one type, spelled as the `type` filter spells it (`formula:mandelbrot`, `script:<id>`, `source:<hash>`) -- a kind and key alone are ambiguous across origins |

The descriptions come from the engine catalog, which takes them from the
twins' `INFO.description` (written 2026-11-16).

### The browser app's resources

Collections, rooms, gallery items, favourites, tags and reports are
reshaped **in this pass**, by the same conventions (**decision C**): in
place, launching with two conventions would mean fixing the second after
launch. Reports gain palettes (and every kind with public text).

## The database

- `palettes` (new): the published-palette item above; `user_palettes`
  rows migrate into it as private palettes.
- `flames`: `description`; `revision` on flames, animations, palettes,
  collections, rooms.
- Tombstones: a table of deleted ids by resource, for id reuse and
  `updated_since`'s `deleted`.
- `variations`, `effects`: `created_at`, `updated_at`.
- A trigram index for `q` on names and plain descriptions.

## The clients

- **This app** (phase 3): every call moves to `/api/v1`; it pages by
  `next`, sends `revision`, reads `thumbnail_url`; `search_public_flames`
  (own-only search without auth, no callers) goes, as does the thumbnail
  PUT's ignored size.
- **The browser app** moves in the same release. It is built on today's
  shapes: `hasMore` from `items.length === PER_PAGE` (Explore,
  ExploreAnimations, admin Audit), error strings matched as text
  (`client.ts`, `PublishPanel.tsx` strips `"Validation error:"`), and
  `display_name`/`owner_display_name` in about 85 places.

## Decided (2026-11-16)

A-E as recommended ("You can continue on the other items"):

A. **`/api/v1/` from launch.**
B. **Published palettes as their own items** (a UUID, moderation,
   favourites), reversing the first round's decision 4.
C. **The browser's resources reshaped in this pass**, `/api/admin`
   outside the promise.
D. **Paging by `next`**, `total` optional.
E. **A `revision` on every mutable item**, optional on writes.
F. **The terms of publishing**, the user's: "the options to be
   Copyright, CC-BY, CC-BY-NC, and CC0. v3.0 for all the Creative
   Commons licenses." CC0 has only a 1.0 (Creative Commons published no
   3.0 of it), so the four are:

   | `license` | Meaning |
   |---|---|
   | `copyright` | all rights reserved (the default, until the user says otherwise: it grants nothing the author did not choose) |
   | `cc-by-3.0` | Creative Commons Attribution 3.0 |
   | `cc-by-nc-3.0` | Creative Commons Attribution-NonCommercial 3.0 |
   | `cc0-1.0` | Creative Commons CC0 1.0 (public domain dedication) |

   Every user item that can be made public carries `license` (fractals,
   animations, palettes, scripts, collections, rooms, gallery items); the
   author chooses it, and may change it. Built-ins carry none: many are
   ports whose terms are their sources' own, which the API does not
   assert. A new licence would be a new value, behind a reader (the
   contract's rule 3).

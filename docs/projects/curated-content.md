# Curated content

**Status: step 1 built 2026-11-18** on ffapi branch `curated-content` (migration 119; what was built and decided is ffapi's `docs/curated-build.md`); every decision made (D1-D11). Both repositories: the
client (this one) and the API server (`c:/projects/ffapi`). Built from a
survey of both on 2026-11-18. The survey's findings are summarised in
*What exists*, with file references where they matter.

## The goal

A large online library of admin-approved content, the online equivalent
of what ships in the app (the presets, the palette packs, the built-in
variations, effects, formulas and scripts). It covers:

- 2D and 3D flame presets
- palettes
- a canonical list of variations by name, which can grow
- escape-time formulas, colourings and presets
- simulation models, colourings and presets
- scripts
- animation tracks
- shader effects

When the app browses online it sees two things: **curated** content and
the **account's own**. Anyone else's public item is opened by its id or
link. This matters most for the kinds that run code.

## Decisions (the user's, 2026-11-18)

| # | Question | Decision |
|---|---|---|
| D1 | What happens when a curated item is edited? | **Curated downloads are static. Editing makes a copy.** |
| D2 | Browsing another user's public items? | It belongs in the API (`/users/{id}/...` already exists). It is not an app feature yet. |
| D3 | How does content get into the curated set? | A submission process, **later**. Until then the admin picks. |
| D4 | Seed what ships in the app as curated rows? | **Yes, automated.** Many items were already imported by hand on production; there are many more still to import. |
| D5 | Canonical names? | **A canonical name is part of curation, for every kind.** |
| D6 | Seed by direct database import, or through the API? | **Through the API.** It is safer than a direct database import. The curator account is the admin account, `31f422c2-e70f-4cd9-a704-85e2f65f044b` (the built-in scripts' owner already). |
| D7 | Palette packs online? | **Whatever the current systems support**, even if packs have to work differently in the app. |
| D8 | Variations and effects: migrations or the import? | **One system for everything**, with a way to track and update items already migrated: **hash-based syncs for every kind.** |
| D9 | Can configs name curated items? | **Yes. Configs use canonical names for everything**, and the API resolves a name that is missing locally. |
| D10 | Files for offline use? | **Save as offers "include curated sources"**, off by default. A carried source whose hash matches its reference is used without a request. |
| D11 | When does an existing config's carried source become a reference? | **On save only.** Loading never changes a file's meaning: a source that matches a curated hash is trusted on load, and is written as a reference the next time the config is saved. |

## What exists

**The server has no notion of curation.**

- Visibility is `private | unlisted | public` (migration 016).
- Moderation is after the fact only: `visible | hidden | removed` (084).
- There is no pending, approved or official state anywhere.

"Featured" is built three different ways:

| Where | How it is set |
|---|---|
| `flames.featured_at` | No setter. Hand-written SQL only. |
| Rooms | `PUT /admin/rooms/{id}/featured` |
| Scripts | `PUT /admin/scripts/{id}/museum` (featured plus a `slug`), CPU kinds only |

Palettes, animations and the GPU script kinds have no curation path.

**Where built-in content lives on the server (three sources of truth):**

- The `variations` table (647, WGSL) and the `effects` table (18, WGSL).
  Content arrives only through generated, review-gated migrations
  (`tools/generate_variation_import.py`, `generate_effect_import.py`).
  These tables are curated in practice. Their `name` is the canonical
  name and `version` is the client's cache key.
- Escape-time and simulation built-ins have no database rows. They exist
  only in the embedded `engine-catalog.json`.
- The 18 built-in scripts are admin-owned rows with `origin='builtin'`.
  A hand-run SQL file imports them (`tools/generated/import_builtin_scripts.sql`).
- Flames, palettes and animations always report `origin='user'`. Nothing
  the app ships exists on the server, except what was imported by hand
  on production.

**User content:**

- Flames carry escape-time and simulation presets too, told apart by
  `render_mode`.
- `palettes` are content-addressed by `content_hash`.
- One `scripts` table holds every Rhai kind.
- An animation's tracks can embed Rhai script source.

Lookup by name exists only for `GET /variations/{name}` and
`GET /effects/{name}`. Both are exact matches: aliases are not resolved,
and there is no batch form.

GPU script ids go to whoever publishes first (`script_id_taken`). Only
CPU stems are reserved.

**The client:**

- Online scopes are `Scope::{Public, Mine}`, shown as Public | Account.
- `Origin::Api` and `Provenance::Api` mean "came from the server", not
  "reviewed".
- The client types do not bind `featured_at`, `origin` or
  `moderation_state`.

**The code trust boundary is only partly in place:**

- **WGSL variations and effects** arrive only through migrations, so the
  automatic fetch by name is curated by construction.
- **Rhai a config carries** compiles automatically as `Provenance::Local`
  with no prompt (`FractalConfig::ensure_sources`). That includes custom
  variations, escape and simulation formulas, and effects.
- **Animation script tracks** run without the untrusted restrictions.
- **Downloaded CPU scripts** are the only content that gets those
  restrictions (`ScriptHost::with_untrusted`).
- **What the risk is:** GPU code cannot read files, but it can hang or
  reset the GPU. CPU Rhai is bounded by an operation budget.

**Duplication on the client:**

- `app/variation_fetch.rs` and `app/effect_fetch.rs` are near-copies
  whose rules have drifted apart:
  - one pauses the render while it fetches and the other does not;
  - they cache and register in opposite orders;
  - only variations refuse a name already used locally;
  - the variation fetch sends an empty `Bearer` header on desktop.
- There are three request plumbings (`api::job` plus hand-written
  slots).
- There are three paths that load a flame by id.
- Seven API methods have no app caller (`list_my_flames`,
  `search_flames`, `list_my_scripts`, `search_public_scripts`,
  `list_my_palettes`, `get_palette_data`, `rename_palette`).
- Variation and effect names go into the URL path without
  percent-encoding.

One thing already fits D1: a user script's registry key
(`scripted::key`) is its id plus the first 8 hex of its source's
SHA-256, so a config already pins the exact version of the Rhai it
carries.

## The model

### A curated item is a frozen copy (D1)

The admin's action **curate** does not flag a user's item. It makes a
**new row, owned by the curator account**, with:

- `curated_at` and `curated_by`;
- `canonical_name` (D5);
- `content_hash`, the hash of exactly what is served;
- `forked_from`, pointing at the original;
- `authors`, crediting the original's owner. Flames, palettes and
  animations gain this column; scripts, variations and effects already
  have it.

The original stays the owner's, editable as before. Curating never takes
anything away from the owner.

A curated row is **static**. A database trigger refuses any change to
its content columns (`config`, `content`, `source`, `tracks`, the
shaders). Prose may still change: the description, translations and
tags. This is the variations table's existing rule that prose edits do
not bump the version.

**Editing makes a copy, on both sides:**

- On the server, an owner endpoint on a curated row answers 409
  `curated_static`. Forking it gives the user an ordinary copy.
- On the client, a curated download that is edited becomes Custom: a
  local copy of the user's own. Loaded configs already behave this way.
  The change is for adopted scripts and entries: an edit drops the
  curated mark and keeps a "copied from <canonical name>" note.

### Names and versions (D5)

- **One namespace for each kind.** The kinds are fractal, palette,
  variation, effect, formula, coloring, solid, field, field_coloring,
  sim_model, sim_coloring, track, animation, and the CPU script kinds.
- **The variation namespace is shared** by the `variations` table and
  curated Rhai variations. Effects work the same way. Curating a Rhai
  variation under a name the WGSL table already holds is refused. That
  is how the canonical list grows: a curated Rhai variation **is** a
  canonical variation.
- The name is a slug: `^[a-z0-9][a-z0-9_-]{2,63}$`. The museum's `slug`
  becomes this name. Variations and effects keep their existing names.
  Aliases resolve for every kind; today only variations have them, and
  only as foreign spellings for `.flame` import.
- **A name points at its current version.** Every version is its own
  frozen row, which keeps D1. Publishing a fix means curating a new
  frozen row under the same name, and the name moves to it. Old versions
  stay fetchable by hash, so a config that pinned one still renders the
  same. WGSL variations already work this way with `version`. Rhai
  entries already pin by hash in the registry key.
- **Retiring a name** tombstones it (the `tombstones` table): the name
  is never reused for something else.

### Browsing and opening

- **In the app:** the Online scopes become **Curated | Account**.
  Curated is `GET /{kind}?curated=1`, which works for a guest too.
  Account is `GET /users/me/{kind}`.
- **Open by id or link** is a field in the Online view, on desktop and
  web. It accepts:
  - a UUID;
  - a link (`?flame=`, `?animation=`, an item URL);
  - a canonical name, written `kind/name`.

  This replaces the three id-loading paths with one. The web's
  `?flame=` and `?animation=` links go through it, and the desktop gains
  the same entry point.
- **In the API:** public lists without `curated=1` and `/users/{id}/...`
  stay as they are, for the browser app and for D2 later.
- **The Catalog gains an Animations family.** It is in the Catalog's
  scope, and there is no public browse of animations today.

### Trust

- **Curated code is trusted by hash.** The client asks
  `POST /curated/resolve` with `{kind, hash}` pairs: is this exact
  source curated? It caches the answers.
  - A config whose Rhai sources all resolve opens without a prompt.
  - So does a WGSL download, which is curated by construction.
- **Uncurated code asks first.** This covers an uncurated config that
  carries Rhai, an animation with script tracks, and an adopted GPU
  script. Each asks once: "contains code by X that hasn't been
  reviewed". The answer is remembered by hash, for that session or
  always.
- **Script tracks run under `with_untrusted`** unless curated.
- **`Provenance::Api { version }` becomes `Provenance::Api { version,
  curated: Option<name> }`** (or a separate `Curated` variant). The
  Catalog's origin filter gains Curated.

## API changes (ffapi)

**Schema:**

- `curated_at`, `curated_by`, `canonical_name` and `authors` where they
  are missing, on `flames`, `palettes`, `scripts` and `animations`.
- `content_hash` on flames and animations, computed over a canonical
  JSON. Scripts already have `script_snapshots.hash`; palettes have
  `content_hash`.
- A unique index on `(kind, canonical_name)` for current rows, and a
  name-history table for older versions.
- The static trigger.
- The three "featured" mechanisms fold into this (**deferred** at step 1:
  `featured_at` and the `featured` word stay as they were, since the
  browser app may use them; folding waits until it moves to `curated`):
  - flames' `featured_at` is migrated;
  - the scripts museum keeps its owner pin (`published_*`), but the
    admin feature becomes curate;
  - rooms are not content and keep theirs.

**Endpoints:**

| Endpoint | What it does |
|---|---|
| `GET /{kind}?curated=1` | Curated rows only. Sorts include name. |
| `GET /curated/{kind}/{name}` | The current version, resolving aliases. `?hash=` asks for a specific version. |
| `POST /curated/resolve` | Batch: `[{kind, name}]` or `[{kind, hash}]`. Replaces one request per missing variation, and answers the trust question. |
| `POST /admin/{entity}/{id}/curate` | Takes `{name, authors?}`. Makes the frozen copy and moves the name. Audited (`moderation_actions` gains `curate` and `retire`). |
| `DELETE /admin/curated/{kind}/{name}` | Retire: tombstones the name. |
| `POST /admin/curated/import` | The seeding import (D4), below. Idempotent, with `?dry_run=1`. |

`GET /variations/{name}` and `GET /effects/{name}` stay for old readers.
They resolve aliases from now on.

## Seeding and syncing (D4, D6, D8)

**One system for every kind, synced by hash.** All of these go through
one sync:

- the app's shipped content;
- the WGSL variations and effects, which today come in by migration;
- the built-in scripts, which today come in by a hand-run SQL file.

That one sync is a manifest the client generates, imported through the
API as the admin account. Every curated row it owns records the hash of
its content, so the next sync knows what changed.

### The manifest

A new binary, `export_curated_json`, writes
`output/curated-manifest.json`. It replaces `export_variations_json`,
`export_effects_json` and `tools/generate_*_import.py` as the source of
built-in content.

Each record carries `kind`, `canonical_name`, `display_name`, `authors`,
`content_hash`, the content, and its prose. It covers:

- the 84 presets in `assets/presets.fflame` (16 2D, 6 3D, 45
  escape-time, 17 simulation), as fractals;
- the 728 palettes in the packs (D7, below);
- the 647 variations and 18 effects. WGSL comes from the registry, as
  the corpora do today;
- the shipped scripts and Rhai examples;
- the Rhai twins of built-ins, where curating them is wanted.

Names come from the item's own name, made into a slug. Variations and
effects keep theirs. Duplicates are numbered and reported.

`content_hash` is the SHA-256 of the content's canonical JSON. For Rhai,
it is the source as `scripted::normalized` writes it, so it agrees with
the registry key's hash.

### The import

`POST /admin/curated/import` receives the manifest in batches. A small
tool, `tools/sync_curated.py`, posts it with the admin account's token.
Nothing writes to the database directly (D6).

**Which rows belong to the sync.** The server marks the curated rows the
sync owns with `origin='builtin'`. Scripts already have that column;
the other tables get it. Each record is matched in this order:

1. **By `(kind, canonical_name)` among the sync's rows.**
   - Same hash: *unchanged*.
   - Different hash: a *new version*. A new frozen row, the name moves
     to it, and the old one stays fetchable by hash. For variations and
     effects, the `version` is bumped as well, for old readers.
2. **By `content_hash` among the admin account's rows.** This finds the
   items imported by hand on production. They are *taken over*: they get
   the name and `origin='builtin'`, and become static.
3. **By display name among the admin account's rows.** This catches a
   hand import made from an older file, whose hash differs. These are
   listed as *needs a decision*. Nothing is written until the record is
   confirmed (`?accept=<name>,...`), and then it becomes a new version
   of that row.
4. Otherwise *created*.

**Removals.** A sync-owned name that is missing from the manifest is
listed as *gone*. It is retired, with its name tombstoned, only with
`?retire=1`.

**The dry run.** `?dry_run=1` lists every record's outcome: unchanged,
new version, taken over, needs a decision, created, gone. The user reads
it before the real run.

**Curated user items are never touched.** Items the admin curated from
users (`origin='user'`, `curated_at` set) are outside the sync.

**Migrations.** The first sync replaces the reseed migrations: no more
078/097/104-style variation migrations or 081/103/110-style effect
migrations. Their rows are taken over by hash in step 1, because the
manifest gives them the same names. The variation tombstones move to the
same retire path.

### Syncing on the client, by hash too

Cached downloads (variations, effects, curated Rhai) are keyed by name
and hash, no longer by name and `version`. The catalog list carries each
curated name's current hash. An update is offered when a cached item's
hash differs from it. As before, the user starts the update.

### Palette packs (D7)

The server has no grouping for palettes:

- `tags` and `collections` exist for flames only (`flame_tags`,
  `collection_flames`).
- Palettes have neither.

So, with what exists:

- **Online:** curated palettes are a flat list of named palettes,
  searchable. The manifest records each palette's pack, and the import
  writes it into the description ("From the Fire and Ice pack"), so a
  search for the pack finds them.
- **In the app:** the shipped packs stay the local grouping. The Online
  place shows curated palettes flat. A pack can also be offered as
  "download these into a local pack": a search plus Save to Library.
- **Later:** if palettes gain tags, the pack becomes a tag and the app's
  packs could be built from it. That is a server change, not part of
  this plan.

## Names in configs (D9)

**Today**, a config refers to things in four different ways:

- **By name:** variations and effects, the WGSL ones fetched when
  missing.
- **By built-in id:** escape formulas and colourings, simulation models
  and colourings.
- **By carrying the source:** Rhai of every kind, keyed
  `u<id>_<hash8>`, including script tracks.
- **By carrying the colours:** palettes.

**After this plan**, a config refers to any curated item by its
**canonical name**, pinned by hash.

- **The config records each curated item it uses.** A new skip-if-default
  table on `FractalConfig` lists `{kind, name, hash}`, and the name is
  the key wherever the item is used: a transform's variation, the effect
  chain, the escape formula and colouring, the simulation model, a script
  track.
- **Built-in names are reserved.** The canonical namespaces include the
  built-ins (the engine catalog's keys and the registered variations and
  effects), so a curated item can never shadow a built-in. The server
  refuses such a name; the contract already ships the list.
- **Resolving a config's names when it loads:**
  1. Look in the local registry: the built-ins, the cache, and what is
     already registered.
  2. Ask for the rest in one request, `POST /curated/resolve` with
     `[{kind, name, hash}]`.
  3. Download each answer, register it as `Provenance::Api` curated, and
     cache it.

  This generalises today's missing-variation fetch, rendering pause
  included. Without the network, or for a name the server does not know,
  the item is reported missing, as a missing variation is today.
- **Pinning by hash.** With the hash, the config gets exactly the version
  it was made with, because curated versions are static (D1). Without one
  (old configs, which name WGSL variations with no hash) it gets the
  current version, as today.
- **What a config carries:**
  - Curated items are carried **by reference only**. Like a WGSL
    variation, the source is not in the file.
  - The user's own and uncurated Rhai is still carried as source.
  - Palettes are still carried as colours, with the name and hash
    recorded so the Catalog knows the palette is curated.
  - For offline use, Save as offers "include curated sources" (D10).
- **Existing configs.** A carried source that matches a curated hash
  stays as source on load and becomes a reference when the config is
  next saved (D11).
- **What else changes:**
  - The `.flame` export writes curated Rhai variations by name, as it
    writes built-ins.
  - PNG metadata carries the same table.
  - The engine contract and catalog gain the curated kinds' name format.
  - A server-side summary names a curated component by canonical name,
    not as `script:<uuid>`.

## Client changes

- **Online scopes.** Curated | Account, then Open by id or link, and the
  Animations family (`ui/inventory_online.rs`).
- **Names in configs.** The config's curated table, resolution on load
  (generalising `report_or_fetch_missing`), and references written on
  save (D9).
- **Trust.** Resolve by hash, the prompt, and script tracks under
  `with_untrusted`. This touches `config/fractal_config.rs`
  `ensure_sources`, `animation/script.rs` and `provenance.rs`.
- **One fetch by name.** `variation_fetch.rs` and `effect_fetch.rs`
  become one resolver over `POST /curated/resolve`, for every kind. It:
  - follows one rule set: pause, cache then register, refuse a local
    name;
  - keys the cache by hash;
  - percent-encodes names;
  - sends no empty `Bearer` header.
- **The manifest.** `export_curated_json` replaces the corpus exporters.
- **One request plumbing.** Everything moves onto `api::job`.
- **Cleanups.** Delete the seven uncalled methods. Bind `origin`,
  `canonical_name`, `curated_at` and `content_hash` in the list types.
- **Edits make copies.** An adopted curated entry that is edited drops
  its curated mark and becomes the user's own, carried as source
  (`storage/links.rs`, `ui/rhai_editor.rs`).

## Order of work

1. **API schema and curate.**
   - The columns, `origin` on every table, `content_hash` and the static
     trigger.
   - `POST /admin/.../curate` and `?curated=1` on the lists.
   - Fold flames' `featured_at` into it (deferred: see *API changes*).
   - **Built 2026-11-18**, ffapi branch `curated-content`. Decided while
     building: a curated animation freezes its fractal with it (an
     unnamed static copy); a curated script's copy takes the canonical
     name as its stem, an older version's stem moving aside to
     `<name>~<hash8>`; thumbnails and tags of a curated fractal are
     static too; `q` searches canonical names.
   - Reserve built-in names.
2. **API resolve and import.**
   - `/curated/{kind}/{name}` and `POST /curated/resolve`, with aliases
     and hashes.
   - `POST /admin/curated/import`: dry run, accept, retire.
   - Variations and effects onto it.
3. **The manifest and the first sync.**
   - `export_curated_json` and `tools/sync_curated.py`.
   - A dry run against production, which the user reviews: in
     particular the hand imports under *needs a decision*.
   - Then the sync.
4. **Client browsing.** Curated | Account, Open by id or link, and the
   Animations family.
5. **Client names in configs and trust.** The curated table, resolution
   on load, the prompt, and untrusted tracks.
6. **Client fetch unification, the cache by hash, and cleanups.**
7. **Later:**
   - the submission process (D3);
   - browsing another user's items (D2);
   - palette tags (D7).

Steps 1-3 are the API and data; 4-6 are the client. Step 4 needs step 1,
and step 5 needs step 2.

## Open questions

None. D1-D11 are decided; the plan is ready to build.

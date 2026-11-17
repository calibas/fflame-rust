# The Catalog: one place for every resource

> **Status 2026-11-16: phases 0-2 merged** (the API reshaped in place on
> ffapi, its `docs/catalog-api-build.md`, and this app's client moved to
> it; the model and the panel; the picker, PR #167). **Phase 3, online,
> designed and decided** (*Phase 3: online*, below), being built on branch
> `catalog-online`. The two surveys this plan stands on (the app's
> sources, the API's routes) were taken 2026-11-16 and are summarised
> under *What exists*; the online survey under *Phase 3*.

## The goal

The user, 2026-11-16: "a "Catalog", a panel where the user can browse
every built-in, plug-in, and resource downloaded from the API", with an
option "to browse and download online items from the API". It "should
show info about the resource, like a preview, authors, description and
where it came from", and "Resources should be selectable, so other parts
of the UI can see what's chosen in the Catalog."

Every kind: palettes, fractals, variations, effects, escape formulas
and colourings, simulation models and colourings, and the creation and
modification Rhai scripts. It replaces the Palette Library, takes the
Fractal Browser's presets and online flames, and is where the user's own
resources are created, edited, deleted and published.

Today the same questions -- where did this come from, is there a newer
one, can I download it -- are answered in six places, each its own way:
the Palette Library (packs, Cloud Palettes), the Fractal Browser
(Presets, Online), the Variations panel (server catalog, updates, Clear
Cache), the Effects panel (the same for effects), the Scripts panel and
the Formula Editor (online search, publish, adopt, plug-in marks), and
the escape and simulation pickers (plug-in groups). It closes two
backlog items: "A combined library panel" and "Formula library in the
Fractal Browser".

## Decided (2026-11-16)

1. **The name the user sees is "Catalog".** The code uses another name
   (*Decided after the survey*, 1): `catalog`, `library` and `resources` are taken.
2. **Text first.** Previews for kinds that have none (variations,
   effects, colourings, models, scripts) come later, one kind at a time.
   Palettes and fractals keep the previews they have.
3. **The API is reviewed and changed as needed**, endpoints made
   consistent: "I'd like to tighten up the API, so it's a perfect time to
   review and update as needed."
4. **Search, filters, lazy rows and lazy previews**, all of them.
5. **A separate phone layout**, planned as its own phase.
6. **Selection is not application.** A click selects and shows the
   entry's info; a **Use** action applies it. Palettes are the
   exception: a click applies (undoable; auditioning palettes by clicking
   is how the library is used today). Loading a fractal is undoable too
   (`App::load_config_with_undo` → `ConfigManager::load_config`, one
   undo step), so a click loads it (*Decided after the survey*, 5).
7. **The Fractal Browser splits**: Presets and Online move to the
   Catalog; Random and Files -- results of this session, not resources --
   stay in a slimmer Fractal Browser.
8. **One Rhai Editor panel.** The Formula Editor and the Scripts panel's
   editing become one editor; formulas and scripts are chosen and managed
   (new, rename, delete, import, plug-in flag, publish, adopt) in the
   Catalog.
9. **A compact Catalog embedded in pickers, if it makes the code easier**
   -- e.g. the Transforms panel's variation picker, from which a variation
   can be downloaded and installed. It does (see *The picker*), so it is
   planned.

## What exists (surveyed 2026-11-16)

### In the app

| Kind | Local source, key | Metadata at runtime | Online today | Picked in |
|---|---|---|---|---|
| Palettes | `PaletteLibrary` (`scene/palette.rs`): embedded starter pack, `assets/palettes/packs/*.json`, loose `.palette` files ("Local Files"), Custom (`custom_palettes.json`); web packs lazily. Looked up by name today; **its id is its content hash**, which the API already keys palettes by (computed by the server, not yet by the app) | name, stops, pack name and description; no author, version or provenance | Cloud Palettes: your library only (`GET /users/me/palettes`, remove); no publish -- a palette travels inside a flame | Palette Library (packs, click applies), Colors panel combo (no search) |
| Fractals | `PresetLibrary` (`assets/presets.fflame`, embedded fallback); **key: `flame.name`**; gallery items keyed by config hash | name, category from the view kind, thumbnail; **no author or description field on `FractalConfig`** | your flames, your-flames search, load, delete; publish via Save Online | Fractal Browser (Presets, Online tabs), `FractalConfigGallery` |
| Variations | `VariationRegistry`: 647 built-ins, API downloads (cached), Rhai variations (`Provenance::Local`); key: name | display name, category, phase, `Provenance`; **descriptions and authors only in `defs/` doc comments** (parsed for the export corpus, not at runtime) or from the server catalog | catalog (cached), fetch missing, updates, Clear Cache | Transforms Add popup (search, by category), Variations panel (catalog, prose, no search), escape lens popup, Random Generator |
| Effects | `EffectRegistry`: built-ins (WGSL, Rhai twins, Rhai built-ins), downloads, Rhai effects; key: name | display name (locale), category, `Provenance`; prose only from the server catalog | catalog, fetch, updates, Clear | Effects panel's two Add combos (+ plug-ins), catalog section |
| Escape formulas, colourings, fields, solids | static `FORMULAS` (26), `COLORINGS` (28), `FIELDS`, `FIELD_COLORINGS`, `IFS_DEFS`, `IFS_COLORINGS`; Rhai entries; key: name | name, display name, features, parameters, presets; **no description, author or version** | only as Rhai entries (scripts resource) | escape panel combos, registry order, no search, plug-in groups |
| Simulation models, colourings | `sim::MODELS` (31), `sim::COLORINGS` (13), shipped Rhai twins; key: name | display name, **one-line description**; no author or version | only as Rhai entries | sim panel combos (description tooltips), plug-in groups |
| Rhai scripts and entries | CPU: `script::library::EMBEDDED`, `assets/scripts`, `scripts/`; GPU entries: `formulas/` (`.rhai`, `.ufm`, `.ucl`, `.frm`); keys: stem / file name | `INFO` title, description, author, id; `storage::links::Link` (cloud id, version, owner, from others, plug-in); CPU scripts' prose from the header comment | `ScriptCloudRequest`: list mine, search public, publish, update (409 conflicts), adopt, unpublish | Scripts panel (picker, params, seed, run, batch, online), Formula Editor (file list, Use, plug-in, publish) |

Building blocks to reuse:
- **`storage::library::scan()`** -- one entry per user or shipped Rhai
  file (kind, title, description, author, id, version, origin, link,
  source). Built for this panel in rhai-plugins P3; **no production
  caller**, and its shipped set misses the escape, variation and effect
  twins (`escape::twins`, `variations::twins`, `effects::custom::RHAI_BUILTINS`).
- **`storage::catalog::merge_state`** -- built-in, yours, downloaded,
  update available, available, not downloadable. Implemented for
  variations and effects only; it extends to the rest.
- **`provenance::{Origin, Provenance}`**, **`storage::links`**,
  **`ScriptCloudState`**, the thumbnail cache, `FractalConfigGallery`,
  and **`ui::params`** (one parameter renderer for every kind).

Gaps: the app does not compute a palette's content hash (the server
does); presets have no stable id; palettes and presets have no origin or
version; escape built-ins have no description; no built-in has authors
at runtime; the pickers each sort, group and search their own way.

### In the API (ffapi, 110 migrations)

What a client can do per kind:

| Kind | Public list | Search | Own list | Fetch, content | Write own | Visibility | Thumbnail | Prose |
|---|---|---|---|---|---|---|---|---|
| Fractals | `/explore`, `/users/{id}/flames` | **own only** (`/search/flames`, signed in) | `/flames` | yes | yes | yes | JPEG | owner, timestamps; no description or authors |
| Animations | `/explore/animations` | no | `/animations` | yes | yes | yes | no | owner, timestamps |
| Palettes | **no** | no | `/users/me/palettes` (bookmarks) | by hash | submit, nickname | **no** | no | nickname only |
| Variations, effects | full list (unpaginated) | no | n/a | by name | no | n/a | no | authors, description, version; no timestamps |
| Built-in escape and sim kinds | only `/types`, counting those public configs use | `/types?q` | n/a | **no** | n/a | n/a | no | key, display name |
| Rhai scripts (13 kinds) | `/search/scripts` | `q` on names | `/scripts?kind=` | with source | yes, optimistic `version` | yes | published CPU scripts only | authors, description, version, timestamps |

Inconsistencies to resolve:
- **Paths**: your own lists at the root (`/flames`, `/scripts`) or under
  `/users/me/` (palettes, gallery items); public lists under `/explore`,
  `/search/scripts`, or their own root.
- **Search**: `q=` (scripts, types) or `name=` (flames, own only); none
  for variations, effects, palettes, animations.
- **Lists**: always a bare array, never a total; page sizes default to 20
  or 50; several lists are unpaginated (variations, effects, collections,
  types at 500, tags).
- **Sort** vocabularies differ, and an unknown value silently falls back.
- **Kind** vocabularies differ: 9 catalog kinds, 13 script kinds.
- **409 bodies** differ; only scripts have optimistic versions.
- **Owner name**: `display_name` (flames, animations) or
  `owner_display_name` (scripts, rooms).
- **Reader gating**: the variations list takes no reader, its detail does.
- The engine catalog (`engine-catalog.json`) is compiled into the server
  but no route serves it.
- Client-side: `search_public_flames` calls the own-only route without
  auth and has no callers; `PUT /flames/{id}/thumbnail` sends a size the
  server ignores.

## The design

### The model (`inventory`)

One entry type over every source:

```rust
pub struct Entry {
    pub kind: Kind,          // Palette, Fractal, Variation, Effect, EscapeFormula,
                             // EscapeColoring, Field, FieldColoring, Solid, SimModel,
                             // SimColoring, Generator, Modifier, ScriptLibrary, Track, ...
    pub key: EntryKey,       // stable per kind (below)
    pub title: String,
    pub description: String,
    pub authors: Vec<String>,
    pub version: Option<String>,
    pub origin: Origin,      // built-in, plug-in, yours, downloaded, online only
    pub state: State,        // installed, update available, available, not downloadable
    pub group: Option<String>, // pack, category, preset category
    pub link: Option<Link>,  // cloud id, owner, visibility
    pub preview: Preview,    // palette stops, config thumbnail, none
}
```

- **Sources are adapters**, one per kind, over what exists: the
  registries, `storage::library::scan` (completed with the twins), the
  palette and preset libraries, and the cached server catalogs. Local
  sources are synchronous; online sources are background requests, as
  `ScriptCloudRequest` is (generalised to a catalog request).
- **Keys**: a registry name; **a palette's content hash**, the API's id
  for it (`palette_content_hash`, ffapi migration 26: SHA-256 of `c` and
  the colour bytes, or `C`, then `s` and the stops' JSONB text, or `S`),
  computed by the app the way the client encodes a palette
  (`api::sync::encode_palette_data`: 256 evenly spaced stops are colour
  bytes, anything else stops), and checked against vectors the server's
  function produced -- the same palette in two packs is one palette
  shown in both; a preset's name within its category; a store file; a
  cloud id for online-only rows.
- **Order is never HashMap order**: by group, then registration or file
  order, as the variation lists are.
- **What can be done to an entry** (use, open in the editor, download,
  update, rename, duplicate, delete, publish, visibility, plug-in flag) is
  computed from its kind, origin and state, so the panel and the picker
  draw the same buttons.

### Prose the app ships

Text-first means the text must exist offline:
- **Variations**: the `defs/` doc comments (descriptions and authors,
  already parsed by `variations/docs.rs` for the corpus) become a
  generated table compiled into the app, kept current by a staleness test
  as the contract is. No attribution beyond what the source says.
- **Escape formulas, colourings, fields, solids**: a `description` on
  each def (every registry is append-only; a field added to the def
  struct is not a registration change). Written per entry.
- **Simulation**: the one-line descriptions exist.
- **Effects**: built-in descriptions from the locale, as their names are.
- **Palettes and presets**: a pack's description; a preset's name.

### The panel

- **Left**: search; filters by kind, origin (built-in, plug-ins, yours,
  downloaded, online) and state (updates, available); the list grouped
  (packs, categories), with lazy rows (`show_rows`).
- **Right, the info pane**: title, kind, description, authors, version,
  where it came from and its online state; the actions; for a script, its
  parameters and Run (*Decided after the survey*, 2).
- **Palettes** grouped by pack, with each pack's enable checkbox and load
  state (the Palette Library's features, Cloud Palettes folded in).
- **Fractals**: the presets (and online) as `FractalConfigGallery` grids.
- **The selection** is shared: one `(kind, key, source)` any panel reads,
  plus each picker's own. **Use** raises a `UiResponse`, handled in
  `ui_handlers.rs` by the code that applies each kind today
  (`ConfigPath::Palette`, `load_config_with_undo`, a transform's
  variation path, the effect snapshots, `render_mode::choose`, the sim
  paths, a script run).
- **Every mode**: the panel is drawn in every mode; whether Use acts is
  `visibility.rs`'s answer, with its reason, as for every control.
- **Saved layouts**: `PanelType` is serialised, so `PaletteLibrary`
  deserialises to the Catalog (a serde alias), and the Palette Library's
  buttons (Colors panel, Palette Editor) open the Catalog on Palettes.

### The picker

The same list as the panel's left side, given a filter, in a popup or a
panel section: search, groups, the entry's description as a tooltip,
**download** for an available entry (pickable once installed). It
replaces the hand-written pickers, which each sort, group and search
their own way and none of which can download:

| Picker | Filter |
|---|---|
| Transforms: Add variation | variations; Only3D only in 3D; not already on the transform |
| Escape lens | variations, as the lens takes them |
| Effects: Add | effects of one stage (colour, density) |
| Escape formula, colourings, layer colouring | escape kinds; colourings that suit the formula (`coloring_suits_formula`) |
| Simulation model, colourings | sim kinds |
| Colors: palette | palettes of enabled packs (click applies) |

Each picker keeps a small "use this" handler of its own; the Random
Generator's variation multi-select stays as it is.

### The Rhai Editor

The Formula Editor and the Scripts panel's editor become one panel that
edits any Rhai kind: the open entry's text, Save, Revert, Use and live
re-apply, Edit as Rhai. Choosing, creating, renaming, deleting, importing,
the plug-in flag, publishing and adopting move to the Catalog, which opens
an entry in the editor. Generator and modifier scripts run from the Catalog's info pane
(*Decided after the survey*, 2).

## The API review

Principles to agree before the ffapi branch (compatibility: *Decided after the survey*, 3):
1. **One shape per resource**: `GET /api/{resource}` browses public
   items; `GET /api/users/me/{resource}` lists your own;
   `GET /api/users/{id}/{resource}` someone's public ones;
   `GET /api/{resource}/{id}` one item; writes as now.
2. **One list envelope**: `{items, total, page, per_page}`.
3. **One search and sort vocabulary**: `q`, `kind`, `sort`
   (`recent|popular|featured|name`), `page`, `per_page`; an unknown sort
   is a 400, as an unknown filter is.
4. **One kind vocabulary** across scripts and the catalog.
5. **One owner field** (`owner_display_name`), **one 409 body**.
6. **Palettes become publishable**: a named palette with an author,
   description and visibility, searchable, with your library of bookmarks
   kept. (Packs online: later, if wanted.)
7. **Fractal search goes public**: search public flames by name and
   catalog filters; your own stays.
8. **Built-ins served**: the engine catalog (formulas, colourings, sim
   models, with descriptions) for clients that do not ship them -- the
   browser app; the desktop app ships its own.
9. **Variations and effects**: timestamps, reader on the list, search and
   paging available (the client may still fetch all ~650 and filter).
10. **Old clients keep working**: released apps call the current routes,
    so they stay, gated or aliased by `reader`, and new shapes are what
    reader 8 asks for.

## Phases

0. **The API review** (ffapi), in parallel with phase 1: the principles
   above turned into a route table ([catalog-api.md](catalog-api.md)),
   agreed, then built on an ffapi branch with the permission matrix and
   tests updated.
1. **The model and the panel, local only**: `inventory` over every local
   source, the prose the app ships, the panel (search, filters, groups,
   info pane, selection, Use), palettes by pack. **Retires the Palette
   Library**; presets move from the Fractal Browser.
2. **The picker**, replacing the pickers above one at a time, the
   Transforms variation picker first.
3. **Online**: browse and search every kind, download, adopt, update,
   Clear Cache; the Variations and Effects panels' catalog sections, Cloud
   Palettes, the Fractal Browser's Online tab and the Scripts panel's
   online section move in. Needs phase 0.
4. **Your own resources**: create, rename, duplicate, delete, import,
   publish, visibility, the plug-in flag; **the Rhai Editor** replaces the
   Formula Editor and the Scripts panel.
5. **The phone layout**: list and info as two screens.
6. **Previews**, one kind at a time: a variation on a reference flame,
   an effect before and after, a colouring or formula's default view, a
   model after N steps, a generator at a fixed seed; rendered lazily and
   cached in the thumbnail cache.

Each phase is a branch, merged before the next starts, except 0 and 1,
which run side by side.

## Phase 3: online

### What the app does online today (surveyed 2026-11-16)

Five sections, each written its own way:

| Section | Shows, offers | State, requests | Account |
|---|---|---|---|
| Variations panel, catalog (`render_catalog_summary`) | counts; stale downloads with Update / Update all; the first 40 available names, no buttons; Clear Cache | `App.variation_catalog` (cached on disk, fetched once a session, every page by cursor); `variation_update_requested` → `trigger_variation_fetches`, **pausing rendering** until done or 30 s | guest |
| Effects panel, catalog (`render_effect_catalog`) | the same for effects (strings untranslated) | `App.effect_catalog`; `effect_update_requested`; no pause | guest |
| Cloud Palettes (`render_cloud_palettes_section`, in the Catalog's palettes) | your library: name, gradient, click applies, a delete without confirmation | `CloudPaletteState` on the UI layer; its own thread and slot; page 1 of 100 | account |
| Fractal Browser, Online tab | **your** flames as text rows (no thumbnails), name search, mode filter, load, delete | panel fields, three slots; page 1 of 100; the only section that signs out on a 401 | account |
| Scripts panel, online (`render_cloud`) | Public scripts: search, Open (adopt); Published by you; publish, update, unpublish, conflicts. The Formula Editor: publish the open file | `App.script_cloud`, `ScriptCloudRequest` → `run_request`; first page only | search, open: guest; the rest: account |

Found along the way:
- **No shared request helper.** Every section writes the same
  thread-or-`spawn_local`, slot and per-frame poll (32 `spawn_local` sites
  in 14 files); panel-owned threads never ask for a redraw.
- **An expired sign-in** signs out only from the Fractal Browser; Cloud
  Palettes and the script cloud show the 401 as a status line.
- **Never wired**: public flames (`search_public_flames` has no caller),
  public palettes (no client function), `/types`, `/meta`, someone's
  public items, thumbnails (`thumbnail_url` is not read; nothing
  downloads one). `FlameListItem` lacks the owner, visibility,
  description and `thumbnail_url` the API sends.
- **A signed-out desktop cannot load a public flame**:
  `load_flame_with_visibility` requires a token.
- **A failed Catalog download** says *Downloading…* until restart (the
  session's asked list is never cleared).

### Decided (2026-11-16)

The user's picks, all as recommended:
1. **Online is a place in the top bar** (*On this device* | *Online*), not
   an origin in the filter: a server query pages and searches unlike a
   local filter.
2. **Online fractals as a thumbnail grid**: the server's thumbnails
   (`thumbnail_url`), downloaded as they scroll into view and cached on
   disk by URL -- the app's first thumbnail download.
3. **Publishing waits for phase 4**: publish, update, unpublish and
   conflicts stay in the Scripts panel and the Formula Editor until the
   Rhai Editor moves them, so they move once.

### The design

**Two places in the Catalog's top bar: *On this device* and *Online*.**
On this device is the list as built in phases 1 and 2 (the cached
variation and effect catalogs' downloads stay in it, as now). Online
lists what the server holds for the family, asked of it page by page:

| Family | Online asks | Use |
|---|---|---|
| Palettes | `GET /palettes`; Mine: `GET /users/me/palettes` | applies a copy (as a click on a library palette); Mine: delete (confirmed) |
| Fractals | `GET /flames`; Mine: `GET /users/me/flames` (with the mode filter); a grid of the server's thumbnails (decision 2) | loads it, one undo step; Mine: delete (confirmed) |
| Variations, effects | the cached catalog (it is the whole list) | Download, Update |
| Escape, simulation, scripts | `GET /scripts?kind=` for each of the family's kinds, a group each with its own paging; Mine: `GET /users/me/scripts` | Open: adopt into your files (as the Scripts panel's Open; the warning about running someone's code kept) |

- **Public for everyone, Mine signed in.** A guest browses and opens
  public items, as a guest already searches public scripts; Mine, and
  deleting, need the account (ONLINE.md's table gains the rows).
- **Search is the server's** (`q`, sent 400 ms after the typing stops);
  the sort is each resource's (`recent`, `popular`, `name`, ...);
  **paging is lazy**: reaching the end of a group asks for its `next`.
- **Rows are entries** (`EntryKey::Online(Kind, id)`, `Origin::Api`),
  with the owner and the time, drawn and selected as every row, so the
  info pane, the search box and the phone layout to come are the list's;
  an online row that the app already holds (a variation by name, a
  palette by content hash, a script by its link) says so and offers its
  local entry.
- **Updates on this device**: `Entry` gains the newer version the cached
  catalog offers; an *Updates (n)* chip shows them, each with Update, and
  Update all. **Clear Cache** moves to the Variations and Effects
  families' top bar.

**One request helper** (`api::job`): spawn (a thread on desktop,
`spawn_local` on the web), a slot, a repaint when done, and a 401 that
signs out the session whoever asked. The Catalog's requests use it, and
the sections that move in are rewritten on it; the rest move to it when
next touched.

**What moves out**, each leaving a button that opens the Catalog there:
the Variations panel's catalog summary and Clear Cache; the Effects
panel's catalog section and Clear Cache; Cloud Palettes; the Fractal
Browser's Online tab (it keeps Random and Files); the Scripts panel's
Public scripts and Published by you.

**Fixed on the way**: loading a public flame signed out; a failed
download says why and offers Download again; every 401 signs out.

### Order of work

1. `api::job`, and the client: one browse call per resource (`q`,
   `sort`, `cursor`, `kind`, the scope), public palettes, the item types'
   new fields, `load_flame` without a token for a public flame.
2. The online list model: entries from pages, per (family, scope, kind,
   query, sort), with paging, loading and error state, matched to the
   local entries.
3. The panel's Online place: the scope, the sort, the lazy groups, the
   info pane's online actions; the fractal grid and its thumbnail
   downloads and disk cache.
4. Updates and Clear Cache on this device; failed downloads.
5. The move-outs, one section at a time, each with its replacement in
   place first.
6. Docs (CATALOG.md, ONLINE.md, UI.md), the plan's status, tests.

## Decided after the survey (2026-11-16)

"I agree on all decisions" -- the five below, as recommended:

1. **The code name.** **`inventory`** for the model
   (`src/inventory/`) and the panel (`ui/inventory.rs`,
   `PanelType::Inventory`, shown as "Catalog"). `catalog` already means the
   engine catalog in both repositories (`crate::catalog`,
   `engine-catalog.json`, ffapi's catalog fields), so renaming that is
   cross-repository churn; `library` is the Rhai scan and the script
   library; `resources` is HTTP and file fetching.
2. **Where scripts run.** **In the Catalog's info pane** --
   a generator's or modifier's parameters (`ui::params`), seed, Run and
   Batch -- since running is a script's Use; the editor is for editing,
   with a Run of its own for the file being written.
3. **API compatibility.** First agreed as new shapes behind reader 8;
   superseded the same day: the API has not launched, so the routes are
   reshaped in place and both clients move with the server, under a
   written stability contract ([catalog-api.md](catalog-api.md), second
   draft).
4. **Public palettes.** A published palette is a named, authored,
   visible item (principle 6); packs online later.
5. **A fractal click** **loads it**, as the Fractal Browser does today --
   one undo step -- with the info pane showing the selection.

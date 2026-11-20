# The Catalog

The Catalog panel lists every resource the app knows -- palettes,
fractals, variations, effects, escape-time formulas and colourings,
simulation models and colourings, and Rhai scripts -- to browse, read
about and use. It replaced the Palette Library and holds the presets the
Fractal Browser's Presets tab held; its list is also every picker's
(*The picker*, below), and its **Online** place is what the server holds
(*Online*, below): the Fractal Browser's Online tab, Cloud Palettes, the
Scripts panel's public scripts and the Variations and Effects panels'
catalog sections moved there. It manages your own resources -- Rhai
files, palettes, online fractals -- and runs scripts (*Your own
resources*, below); the Rhai Editor edits the file it opens. On a phone
its list and info are two screens (*The phone layout*), and its entries
show previews (*Previews*). Built in phases 1 to 6 of
[docs/archive/projects/catalog.md](../archive/projects/catalog.md), the plan.

- Model: [src/inventory/](../../src/inventory/) (`inventory::scan`,
  `Entry`, `EntryKey`). The code says `inventory` because `catalog` is the
  engine catalog in both repositories (`crate::catalog`,
  `engine-catalog.json`).
- Panel: `PanelType::Inventory`, drawn by `InventoryPanel`
  ([src/ui/inventory.rs](../../src/ui/inventory.rs)). Saved layouts that
  named `PaletteLibrary` open it (a serde alias).
- Picker: `ui::picker::Picker` ([src/ui/picker.rs](../../src/ui/picker.rs)).
- Your own resources: [src/ui/inventory_manage.rs](../../src/ui/inventory_manage.rs),
  the operations [src/storage/manage.rs](../../src/storage/manage.rs);
  running a script [src/ui/script_run.rs](../../src/ui/script_run.rs);
  the editor [src/ui/rhai_editor.rs](../../src/ui/rhai_editor.rs).
- Previews: [src/ui/inventory_preview.rs](../../src/ui/inventory_preview.rs).

**See also:** [PALETTE_LIBRARY.md](PALETTE_LIBRARY.md) (packs),
[FRACTAL-BROWSER.md](FRACTAL-BROWSER.md) (the presets file, the gallery
widget), [VARIATIONS.md](VARIATIONS.md), [ESCAPE.md](ESCAPE.md),
[SIMULATION.md](SIMULATION.md), [SCRIPTING.md](SCRIPTING.md).

---

## Opening it

- **The Catalog menu**, in the menu bar after Rendering (and the compact
  menu's Catalog submenu): a row a family opens it there; **Online...**
  opens its Online place at the family it was on.
- The Colors panel's **Browse Palettes** and the Palette Editor's
  **Browse Palettes in the Catalog** open it on Palettes.
- **File ▸ From Preset Library** opens it on Fractals.

It is drawn in every mode.

## The model

`inventory::scan(palettes)` reads every source into one list of
`Entry`s: kind, key, title, description, authors, version, origin
(`provenance::Origin`), group and link record. The list is
sorted by kind, then group, then each source's own order -- never a
map's. `inventory::revision(palettes, files)` hashes what changes while
the app runs (the palette library's generation and packs, the variation
and effect registries, the stores' change count, the online library's
`library_generation`, the curated scripts fetched); the panel scans again
when it changes.

| Kind | Source | Key | Group |
|---|---|---|---|
| Palette | the app's `PaletteLibrary`: every loaded pack | the content hash | the pack |
| Fractal | `PresetLibrary` | the preset's name (a repeat gains ` (2)`) | `PresetCategory` |
| Variation | `VariationRegistry`: built-ins, downloads | registry name | its category |
| Effect | `EffectRegistry`: built-ins, downloads | registry name | colour, density |
| Formula, Coloring, Field, Field Coloring, Solid, Solid Coloring | `escape::FORMULAS`, `COLORINGS`, `fields::FIELDS`, `FIELD_COLORINGS`, `ifs::IFS_DEFS`, `IFS_COLORINGS` | registry name | the kind |
| Simulation Model, Coloring | `sim::MODELS`, `sim::COLORINGS` | registry name | the kind |
| Rhai entries and scripts | `storage::library::scan`: the shipped examples and starter scripts, the user's files, what they adopted | store, file, shipped | Examples, Custom, Adopted |

- **A palette's key is its content hash**, the API's id for it.
  `api::sync::palette_content_hash` reproduces ffapi's
  `palette_content_hash` (migration 26) over what the client sends for a
  palette (`encode_palette_data`): SHA-256 of `c` and the colour bytes (or
  `C`), then `s` and the stops as Postgres prints them as `jsonb` (or
  `S`) -- keys by length, `": "` and `", "`, numbers as `numeric` prints
  them, never with an exponent. Checked against vectors the server's
  function produced (`a_palettes_id_is_the_servers_content_hash`). The same
  palette in two packs is one palette, listed in each.
- **The user's own registry entries are not listed twice**: a compiled
  file is its file, and a source a config carries is the config's. A
  built-in's Rhai twin is the built-in's own source, not an entry of its
  own.
- **One list, no plug-ins** ([one-list.md](../projects/one-list.md)): a
  Rhai file of a kind a list uses -- a shipped example, a file of the
  user's, an adopted one -- is in the pickers as it is in the Catalog,
  compiled the first time a list or a preview asks
  (`ui::inventory::compiled` -> `library::compile_file`, once per file and
  version, a file with no `INFO.id` under one derived from its name).
  **Never inside a draw**: a list's ask is queued and compiled before the
  next frame's panels (`inventory::compile_requested`), as a panel may
  hold a registry's read guard while it draws a picker and registering
  takes the write lock (it hung the Add Variation picker:
  `a_pickers_filter_never_compiles_under_a_registry_guard`). Meanwhile
  the row is offered and a preview shows its spinner.
- **In a picker, what can be downloaded comes last**, after every kind's
  own groups.
  Where a thing came from is its origin and link, nothing else: the
  *Load at start* flag (`Link::plugin`) is retired and ignored in old
  `_links.json` files.
- **Prose comes from where it is written** (`inventory::prose`): a
  variation's twin opens with its built-in's doc block, so 640 of the 647
  have their description and authors offline; a built-in's Rhai twin's
  `INFO` (or the doc block it opens with) for the escape kinds, effects and
  simulation kinds; a simulation def's line; a file's `INFO`; a CPU
  script's header summary (`script::parse_doc`). The cached server catalog
  covers downloads and the seven variations without a twin. Nothing is
  attributed that its source does not say.
- **Every escape-time built-in and every built-in effect has a
  description** (2026-11-16): in its twin's `INFO.description` -- the 26
  formulas, 28 colourings, 3 fields and their 3 colourings, 4 solids
  (`assets/formulas/twins/`) and 18 effects (`assets/effects/`) -- so it
  travels with the twin when it is opened or published; written in ASCII
  (`z -> z^2 + c`), as the fonts draw no superscripts or arrows. The
  seven with no twin -- the two flame attractors and the solids' five
  colourings -- take theirs from the locale
  (`inventory.descriptions.<kind>.<name>`, `prose::untwinned`). A
  twin's text is part of its registered name, so the engine catalog's
  twin table gained the new names (append-only) and its examples were
  regenerated.

## The panel

- **The top**: a tab a family (Fractals, Palettes, Variations,
  Escape-Time, Simulation, Effects, Scripts; it opens on Fractals) with
  its count; a search over the
  title, the name, the description and the authors; an origin filter
  (every origin, built-in, custom, from others); and, in a family
  of several kinds, a chip a kind.
- **The list**: the family's entries in collapsing groups with counts;
  a row is the title and its origin's mark (` *` yours, ` ↓` somebody
  else's), its description's first line as the tooltip.
- **Palettes**: pack by pack, each with its enable checkbox, its load
  state and a Retry (`palette_library::pack_section`), a row each: the name
  and a gradient (`palette_preview`, made once a row is on screen, kept
  under its hash). Searching leaves out packs with no match. Signed in,
  a line after the packs shows the account's online palettes (Online,
  Account).
- **Fractals**: the presets as `FractalConfigGallery` grids, a tab a
  category, previews made for the gallery shown (the app's preview loop
  drives the Catalog's galleries beside the Fractal Browser's). A search
  or an origin filter shows rows instead.
- **The info pane** (at the right from 560 points wide, under the list
  when narrower): the title, kind and group, where it came from, the
  version, the authors, a palette's gradient, the description, then the
  actions.

## Selecting and using

A click selects; the info pane's **Use** -- or a double click -- applies.
A palette is applied by its click, and a fractal loaded by its click (one
undo step each). `InventoryPanel::selected` gives the selection to any
panel.

What Use does is what each kind's own picker does, through the same
functions:

| Kind | Use |
|---|---|
| Palette | the config's palette, an editable copy (`ConfigPath::Palette`) |
| Fractal | loads it (`load_config_with_undo`, as the Fractal Browser did) |
| Variation | adds it at weight 1 to the transform the editors are on (`transforms::editing_transform`: the Triangle Editor's selection, else the first), its source carried for a Rhai one (`transforms::add_variation`) |
| Effect | adds it at the end of its chain, a file's source carried (`effects_panel::add_effect`) |
| Formula, Field, Solid | `escape_panel::choose_formula` / `choose_field` / `choose_solid`: the default view, entering Escape |
| Coloring | `escape_panel::pick_coloring`: what a fresh pick sets |
| Field or Solid Coloring | `escape_panel::choose_named_coloring` |
| Simulation Model | the first layer's (`sim_panel::choose_first_layer_model`) |
| Simulation Coloring | the base colouring (`sim_panel::choose_base_coloring`) |
| A Rhai file that compiles | as its kind, by its compiled key (`escape_panel::choose_plugin`, ...) |
| A CPU script | runs it: the info pane's parameters, seed, Generate / Apply, Batch / Mutate (`ui::script_run`) |
| Another Rhai file (one that does not compile), a UF or Fractint file, a track | **Edit** opens it in the Rhai Editor (`RhaiEditorPanel::open_file`); Use says why it cannot apply |

**Not installed.** What the server offers and the app lacks -- a
variation or an effect in the cached catalog (`inventory::Online`, the
catalogs the app last fetched), downloadable, under a name the registry
does not hold -- is listed under **Available to download**, marked with
an arrow, `Entry::installed` false. Its Use is **Download**
(`inventory::request_download`, drained once a frame into the fetches
that install and update variations and effects); once installed it is an
ordinary entry. A row asked for says **Downloading…**.

Use is disabled, with the reason on hover, when it cannot apply:
`visibility::catalog_use` answers for the mode (a variation needs a flame
mode, a 3D-only one 3D); the engines' own rules for the rest (an escape
colouring must draw the current formula, `coloring_suits_formula`; a
field's colouring needs a field, a solid's a solid that takes it,
`coloring_applies`; a variation needs a transform).

---

## The picker

Every hand-written picker became the Catalog's list given a filter
(`ui::picker::Picker`): a button showing the current choice opens a popup
with a search (focused on opening), the Catalog's groups, **Download** for
an entry not installed, and greyed rows where the caller refuses one.
Beside the list, an info pane says what the entry under the pointer is --
the Catalog's own summary (`inventory::entry_summary`), a refused entry's
reason, Download -- keeping the last one pointed at while the pointer
crosses to it, and the current choice on opening. A screen too narrow for
both (a phone) shows the list alone, descriptions as tooltips. A popup, not a
ComboBox: a combo closes on any click inside it, its search box included
(`the_picker_stays_open_while_it_is_used`). The list is read once a frame
before any panel draws (`InventoryPanel::refresh`, from the UI's frame) and
shared (`inventory::shared`), so every list of a kind is the same list.

**Previews in pickers**: the info pane shows the entry's preview above
its summary, as the Catalog's does; and a picker whose kinds have
previews (`inventory_preview::has_previews`) offers **List / Grid** beside
its search, remembered per picker (egui memory, `<picker id>/grid`). The
grid is the Catalog's cells at 96 px, four across (the list widens to
fit); a cell asks for its picture only while on screen; one that cannot
be picked here is greyed with the reason on hover, one not installed is
greyed with its Download in the pane. The Catalog and the pickers share
one set of previews (`inventory_preview::with`, a thread-local store):
its frame begins in the UI's frame before any panel draws, and the app
makes one a frame of what was asked, so a closed Catalog does not stop
a picker's (`the_picker_grid_asks_for_the_previews_on_screen`).

| Picker | Filter | A pick |
|---|---|---|
| Transforms: Add variation | variations; Only3D only in 3D; not already on the transform nor superseded; the fractal's own Rhai variations after (`picker::own_variations`) | `add_variation` |
| Escape lens | None, the recommended lenses, then the measured set (Show all: every variation, downloads too) | `lens_choice` |
| Escape transformations | variations with a 2D body (a solid: a 3D body) | the entry's variation |
| Effects: Add effect… | effects of the stage (an uninstalled one by its catalog `category`) | `add_effect` |
| Escape formula | formulas, fields, solids, Rhai files of those kinds that compile | `inventory::use_entry` |
| Escape colouring | the colourings that draw the formula (`coloring_suits_formula`), Rhai colouring files | `use_entry` (`pick_coloring`) |
| A field's or a solid's colouring | its registry's (a solid: `coloring_applies`), Rhai files of the kind | `use_entry` |
| Texture layer | None, the colourings that draw the formula; one the base refuses greyed (`layer_refusal`) | `pick_layer` |
| Simulation model, colourings | models, colourings, Rhai files of the kind, in every layer and slot | the slot's own choice |
| Colors: palette | the palettes of enabled packs | the config's palette, an editable copy |

A picker admits what it can use (`inventory::pickable`): an installed
built-in or download, a file that compiles; a UF or Fractint file, a file
that does not compile, is not offered (L2). A file is picked by its
compiled key (`inventory::applied_name`); the Random Generator's variation
multi-select stays as it is.

**Downloads in pickers** (L3): the pickers also offer what is not
installed -- the server's variations and effects (as the Catalog's
*Available to download*), and the **curated online scripts** of the kinds
the pickers use (`sources::curated_scripts`; fetched once a session by
the panel, `Scope::Curated`; the app's own shipped content the server
holds, `origin` "builtin", left out; one already adopted left out). A
curated row's Download adopts it (`ScriptCloudRequest::Adopt`, one at a
time, as the Online place adopts); once adopted it is a file and is
picked. The Catalog's own list leaves those rows to its Online place.
**Offer downloads in pickers** in the Catalog menu
(`SystemSettings::picker_downloads`, on unless turned off) offers only
what is installed when off.

---

## Online

The top's first row chooses the place: **On this device** (everything
above) or **Online** (`Place`), what the server holds for the family
([src/ui/inventory_online.rs](../../src/ui/inventory_online.rs), its
entries made by `inventory::online`):

| Family | Online lists | Use |
|---|---|---|
| Palettes | `GET /palettes?curated=true`; Account: `GET /users/me/palettes` -- each a row with its gradient | a click applies a copy; Account: Delete... (confirmed) |
| Fractals | `GET /flames?curated=true`; Account: `GET /users/me/flames` -- a grid of the server's thumbnails, a render-mode filter (All, 2D, 3D, Escape Time, Simulation); and **Animations**, a kind of the family (the user's choice, 2026-11-18): `GET /animations?curated=true`, Account: `GET /users/me/animations`, as rows | a fractal: a click loads it, one undo step, its online id kept so Save Online updates it; Account: Delete... (confirmed). An animation: **Load**, with its fractal, as the Animation panel's list loads one |
| Variations, Effects | the cached catalog first (downloaded, offered), then people's Rhai ones (`GET /scripts?kind=variation`, `effect`) | Download, Update; a Rhai one: Open |
| Escape, Simulation, Scripts | `GET /scripts?kind=` for each of the family's kinds, a group each (the kind chips narrow it to one) | Open: adopt into your files (`ScriptCloudRequest::Adopt`), under the restrictions somebody else's code runs with |

- **Curated or Account** ([curated-content.md](../projects/curated-content.md)).
  Curated is the curated collection -- reviewed, static copies, each
  under its canonical name (`Scope::Curated`: a list with
  `curated=true`) -- a guest's too; Account (the signed-in account's own
  items, every visibility) needs the account (the button says so when
  signed out). Anyone else's public item is not browsed: it opens by its
  id or link. A curated item's info says **Curated, as** its name (and
  when it is an older version), and credits whose work it is.
- **Open by ID or link**, under the scopes: an item's id, a link that
  holds one (the web app's `?flame=` / `?animation=`, an API path, a
  page of the browser app's: the resource named last before the id
  decides), or a canonical name `kind/name` (resolved by
  `GET /curated/{kind}/{name}`). It does what a click on the item's row
  would: a fractal or an animation loads, a palette applies, a script is
  opened (adopted). An id that names no resource is looked for as a
  fractal, an animation, a palette, then a script; a built-in
  variation's or effect's name says it is in the app already
  (`inventory::online::parse_open`, `OnlinePlace::open`). Code nobody
  reviewed asks first: a fractal's or an animation's when it loads, a
  script before it is adopted ([ONLINE.md](ONLINE.md), *Trusting code
  from the server*).
- **Search** is the server's: the panel's search box, asked
  (`q`) once the typing has stopped for 0.4 s. **Sort**: the resource's
  (Newest, Most liked, Most viewed, Recently changed, Name; each
  resource takes some).
- **Refresh** (⟳, the panel's top-right corner) does everything at
  once, in either place: the Online lists and the pickers' curated
  scripts are asked for again, the files on disk are read again (a file
  dropped into a store appears), and the app fetches the variation and
  effect catalogs again (`InventoryPanel::refresh_all`). Palette packs
  are still read only at startup.
- **A failed list** says why once (`FetchError`'s `Display` names a
  network error; the client's message does not repeat it). **Try again**
  asks for the page that failed; and when another list's page comes
  back, every failed list drops its error, so it is asked again when
  shown (`OnlinePlace::forget_failures`).
- **Paging** is lazy, by the server's cursor: reaching a list's end asks
  for its next page. Lists are kept per (kind, scope, search, sort,
  mode), so going back to an earlier search shows what it found; a
  repeated id is shown once.
- **What the app holds**: an online script you opened (its file's link
  names the id) or a palette of the same colours says so and offers
  **Show it on this device**.
- **Thumbnails** (a fractal's `thumbnail_url`, used as given: it changes
  when the image does) download four at a time, decode off the frame,
  and are kept in the app's one thumbnail cache (`ThumbnailCache`, keyed
  by the URL's hash; on disk on desktop, within its limit). A path is fetched from the API with the token (a
  private fractal's is its owner's); a whole URL elsewhere is fetched
  without it.
- Every request is an `api::job` (a thread on desktop, `spawn_local` on
  the web); a 401 to one asked signed in signs the session out.
  Signing out forgets your own lists; Save Online forgets them all.

**Updates and Clear Cache** (on this device, Variations and Effects): a
download the cached catalog has a newer version of carries
`Entry::update` and the mark ` ↑`; the **Updates (n)** chip lists them,
**Update All** asks for each, and the info pane's **Update** for one --
through the fetches that install. **Clear Cache** removes the downloads
and the cached catalog. A download that fails (an HTTP error, a refused
effect, the fetch's timeout) is asked for no more: its info pane says why
(`inventory::download_failure`) and offers Download again.

**Where the old sections went**: each panel says so and shows the
Catalog there (`inventory::request_open`, drained once a frame): the
Variations and Effects panels (their online sections), the Fractal
Browser (**Online...**), the Scripts panel (its public scripts and
Published by you; its Publish, Update, Unpublish and conflicts stay
until phase 4).

---

## Your own resources

The Catalog manages; the Rhai Editor edits (the plan's decision 8). The
info pane offers, for

| Entry | Actions |
|---|---|
| A Rhai file of yours (either store) | **Edit**, **Rename...**, **Duplicate**, **Delete...** (confirmed), **Export...**, and **Online**: Publish (private, public), Update, Make Public / Make Private, Unpublish; a conflict's Overwrite / Load theirs, shown with the file it names |
| A shipped file, a built-in with a Rhai twin | **Edit as Rhai**: a copy among your files (`inventory::twin_source`), opened in the editor |
| A palette of yours (the Custom pack) | **Rename...**, **Delete...**, Publish (private, public): a palette item of its own online |
| Online, a fractal or palette of yours | **Rename...**, Make Public / Make Private, **Delete...** |

and the top of a family that holds Rhai files offers **New** (a kind,
from `inventory::template`: the language's starting text for a formula
and a variation, else the kind's first shipped example) and **Import...**
(a Rhai, UF or Fractint file, into the store its text belongs in).

The operations are `storage::manage`'s, over both stores' rules (a
formula file keyed by its name and extension, a script by its stem; a
shipped stem reserved):

- **Rename** keeps the file's link (cloud id, origin); it compiles again
  under the new name; a taken name is refused.
- **Duplicate** is a new file of the user's: no cloud id, an entry's
  `INFO.id` replaced (two files with one id would be one script online
  and in the lists).
- **Delete** forgets its compiled record and drops the link with the
  file; a published copy stays online.
- **Import** and **New** never overwrite: a taken name gains `-2`, `-3`.
- Somebody else's file, as adopted, is not the user's to publish
  (`manage::publish_refusal`); an edit or a copy of it is (decision 3,
  below).
- Every change bumps `manage::generation`, which the Catalog's list, the
  editor and `script_run`'s library watch beside the script cloud's
  adoptions.

A saved Rhai file that does not parse stays listed, as an **Unreadable
File** among the scripts, so it can be opened and fixed.

**Running a script** is its Use (the plan's survey decision 2): the
info pane of a generator, modifier or library draws `ui::script_run` --
its parameters, the seed and Reroll, Generate / Apply, Batch / Mutate
(the original first), a library's import line, errors with their line --
on the host every script shares (the whole library, so one script calls
or imports another; somebody else's restricted). A flame, a batch or an
animation loads as the Scripts panel's did.

**The Rhai Editor** (`PanelType::RhaiEditor`; saved layouts naming
`FormulaEditor` or `Scripts` open it) edits the one file open, of either
store: its name, kind and an unsaved mark, **Save**, **Revert**, **Show
in Catalog**; for a GPU kind its checks as it is typed, **Use** per kind
and **Live**; for a CPU script **Run** (`script_run` again, on the text
being written); for a UF or Fractint file its dialect and Edit as Rhai.
An unsaved buffer -- a built-in's twin opened by a panel's Edit as Rhai,
a converted entry, a shipped example -- becomes a new file of yours on
Save. A file renamed or deleted in the Catalog leaves the editor's text
unsaved.

**An edited adopted file** (decision 3): a GPU entry gets an id of its
own and is the user's; a CPU script gets free of that online script --
it can be published as the user's -- but keeps running restricted:
editing a line is not reading the rest.

---

## The phone layout

Under 560 points wide (a phone's compact layout, a narrow dock) the list
and the info are two screens. The list fills the panel; the selected row
carries a **›** that opens its info full height, with **‹ Back** at its
top. A tap still selects -- a palette still applies, a fractal still
loads -- so auditioning palettes stays a tap each. Where the list is a
gallery or a grid (the presets, online fractals) the selection and its
› sit above it. The Online place, the picker (its info pane gives way to
tooltips there) and the management actions work unchanged inside the two
screens.

---

## Previews

An entry's preview is the config that shows it, built through what Use
does (a scratch `ConfigManager` and `use_entry`) and rendered by the
gallery's machinery -- `renderer::render_preview`, at 256 points, one a
frame on desktop, one in flight on the web -- then kept in the shared
`ThumbnailCache` (its limit, its eviction) under the configs' hash and the
entry's version, so an update or an edit makes a new one:

| Kind | Its preview |
|---|---|
| Variation | its image of the plane: four transforms filling the square [-1, 1]^2 evenly, coloured by quarter, the variation alone as the final transform (through `add_variation`, so a Rhai one carries its source); framed so the square fills half the picture; a variation that colours (direct colour, the GLSL generators) runs in the four transforms instead, as a final's colour writes are discarded; a pre or post one passes the point through linear |
| Effect | a reference fractal (the first 2D preset) before and after |
| Formula, field, solid | its default view |
| Colouring (escape, field, solid) | the default formula drawn with it |
| Simulation model, colouring | its first layer or base colouring, run 400 steps at most |
| Generator | its run at seed 1; a modifier: the reference fractal, modified at seed 1 |
| A Rhai file of a list's kind | as its kind |
| Palette, fractal | their gradient, the gallery, the server's thumbnail, as before |

A Rhai file of a kind a list uses -- an example, a file of yours, an
adopted one -- previews through what compiling it makes
(`inventory::plugin_of`, the same record the pickers use). A library, a
track and an unreadable file have none. Previews
that take the default palette use Fire without its black end, which on
the black background hid whatever was drawn at index 0; a colouring
starts from its own parameters, not the formula preset's. The info pane shows the selected entry's preview above
its description (a spinner while it is made). A family with previews can
be a **Grid** (beside the origin filter): each cell its entry's preview,
asked for as it comes on screen -- the first look at 647 variations fills
in as they scroll by, later looks come from the cache.
`render_previews_for_inspection` (ignored) renders a sample through the
preview path into `output/previews/` for a look.

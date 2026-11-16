# The Catalog

The Catalog panel lists every resource the app knows -- palettes,
fractals, variations, effects, escape-time formulas and colourings,
simulation models and colourings, and Rhai scripts -- to browse, read
about and use. It replaced the Palette Library and holds the presets the
Fractal Browser's Presets tab held. Built in phase 1 of
[docs/projects/catalog.md](../projects/catalog.md), the plan, which also
says what comes next (online browsing, the picker, your own resources,
one Rhai editor, the phone layout, previews).

- Model: [src/inventory/](../../src/inventory/) (`inventory::scan`,
  `Entry`, `EntryKey`). The code says `inventory` because `catalog` is the
  engine catalog in both repositories (`crate::catalog`,
  `engine-catalog.json`).
- Panel: `PanelType::Inventory`, drawn by `InventoryPanel`
  ([src/ui/inventory.rs](../../src/ui/inventory.rs)). Saved layouts that
  named `PaletteLibrary` open it (a serde alias).

**See also:** [PALETTE_LIBRARY.md](PALETTE_LIBRARY.md) (packs),
[FRACTAL-BROWSER.md](FRACTAL-BROWSER.md) (the presets file, the gallery
widget), [VARIATIONS.md](VARIATIONS.md), [ESCAPE.md](ESCAPE.md),
[SIMULATION.md](SIMULATION.md), [SCRIPTING.md](SCRIPTING.md).

---

## Opening it

- **Window ▸ Catalog** (also in the compact Window menu).
- The Colors panel's **Browse Palettes** and the Palette Editor's
  **Browse Palettes in the Catalog** open it on Palettes.
- **File ▸ From Preset Library** opens it on Fractals.

It is drawn in every mode.

## The model

`inventory::scan(palettes)` reads every source into one list of
`Entry`s: kind, key, title, description, authors, version, origin
(`provenance::Origin`), plug-in flag, group and link record. The list is
sorted by kind, then group, then each source's own order -- never a
map's. `inventory::revision(palettes, files)` hashes what changes while
the app runs (the palette library's generation and packs, the variation
and effect registries, the plug-ins, and the stores' change count, the
online library's `library_generation`); the panel scans again when it
changes.

| Kind | Source | Key | Group |
|---|---|---|---|
| Palette | the app's `PaletteLibrary`: every loaded pack | the content hash | the pack |
| Fractal | `PresetLibrary` | the preset's name (a repeat gains ` (2)`) | `PresetCategory` |
| Variation | `VariationRegistry`: built-ins, downloads | registry name | its category |
| Effect | `EffectRegistry`: built-ins, downloads | registry name | colour, density |
| Formula, Coloring, Field, Field Coloring, Solid, Solid Coloring | `escape::FORMULAS`, `COLORINGS`, `fields::FIELDS`, `FIELD_COLORINGS`, `ifs::IFS_DEFS`, `IFS_COLORINGS` | registry name | the kind |
| Simulation Model, Coloring | `sim::MODELS`, `sim::COLORINGS` | registry name | the kind |
| Rhai entries and scripts | `storage::library::scan`: the shipped examples and starter scripts, the user's files, what they adopted | store, file, shipped | Examples, Yours, Adopted |

- **A palette's key is its content hash**, the API's id for it.
  `api::sync::palette_content_hash` reproduces ffapi's
  `palette_content_hash` (migration 26) over what the client sends for a
  palette (`encode_palette_data`): SHA-256 of `c` and the colour bytes (or
  `C`), then `s` and the stops as Postgres prints them as `jsonb` (or
  `S`) -- keys by length, `": "` and `", "`, numbers as `numeric` prints
  them, never with an exponent. Checked against vectors the server's
  function produced (`a_palettes_id_is_the_servers_content_hash`). The same
  palette in two packs is one palette, listed in each.
- **The user's own registry entries are not listed twice**: a plug-in is
  its file (marked a plug-in), and a source a config carries is the
  config's. A built-in's Rhai twin is the built-in's own source, not an
  entry of its own.
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

- **The top**: a tab a family (Palettes, Fractals, Variations, Effects,
  Escape-Time, Simulation, Scripts) with its count; a search over the
  title, the name, the description and the authors; an origin filter
  (every origin, built-in, yours, from others, plug-ins); and, in a family
  of several kinds, a chip a kind.
- **The list**: the family's entries in collapsing groups with counts;
  a row is the title and its origin's mark (` *` yours, ` ↓` somebody
  else's, `(plug-in)`), its description's first line as the tooltip.
- **Palettes**: pack by pack, each with its enable checkbox, its load
  state and a Retry (`palette_library::pack_section`), a row each: the name
  and a gradient (`palette_preview`, made once a row is on screen, kept
  under its hash). Searching leaves out packs with no match. With the
  online account on, **Cloud Palettes** follows the packs.
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
| Effect | adds it at the end of its chain, a plug-in's source carried (`effects_panel::add_effect`) |
| Formula, Field, Solid | `escape_panel::choose_formula` / `choose_field` / `choose_solid`: the default view, entering Escape |
| Coloring | `escape_panel::pick_coloring`: what a fresh pick sets |
| Field or Solid Coloring | `escape_panel::choose_named_coloring` |
| Simulation Model | the first layer's (`sim_panel::choose_first_layer_model`) |
| Simulation Coloring | the base colouring (`sim_panel::choose_base_coloring`) |
| A plug-in file | as its kind, by its registered key (`escape_panel::choose_plugin`, ...) |
| A CPU script | selects it in the Scripts panel, which opens |
| Another Rhai file, a UF or Fractint file, a track | **Open in Formula Editor** (`FormulaEditorPanel::open_file`) |

Use is disabled, with the reason on hover, when it cannot apply:
`visibility::catalog_use` answers for the mode (a variation needs a flame
mode, a 3D-only one 3D); the engines' own rules for the rest (an escape
colouring must draw the current formula, `coloring_suits_formula`; a
field's colouring needs a field, a solid's a solid that takes it,
`coloring_applies`; a variation needs a transform).

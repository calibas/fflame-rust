# One Catalog view

**Status:** planned 2026-11-20, open questions below; nothing built.
Branch: `curated-content` (where the Catalog work is), unless the user
says otherwise.

## Why

The Catalog draws the same job several ways:

- **Local vs Online** are two separate sub-panels
  (`InventoryPanel::render_list` vs `OnlinePlace::render_list`). The
  online one lacks Grid/List and the size slider.
- **Fractals on this device** are a third: the presets drawn as
  `FractalConfigGallery` tabs, with their own search, Grid/List, size
  slider and in-list expandable details. The panel's search above it
  switches to a fourth, plain grouped rows.
- **The info pane** follows the selection, never the pointer. A row's
  click only selects; Use or a double click applies; in the Fractals list
  only the `›` button opens the details.
- **The Origin selector** (All / Built-in / Custom / From others) sits
  next to the search, and Local / Online sits above everything.

The goal is one panel with one set of controls, laid out the same way for
every catalog, each catalog adjusting only the filters that make sense
for it.

## The design

### The top of the panel

One row of selects, then one row of view options:

| Control | Values | Default |
|---|---|---|
| **Catalog** | Fractals, Palettes, Variations, Escape-time, Simulation, Effects, Scripts (one dropdown; replaces the seven tabs) | Fractals |
| **Category** | per catalog (below) | All |
| **Source** | All, Online, This device (replaces the Local / Online switch) | All |
| **Owner** | All, Curated, Yours, Others (replaces the Origin selector) | All |
| **Type** | per catalog, only where a catalog has several (Fractals: Fractal, Animation) | All |
| View | Grid / List | Grid (where there are previews) |
| Search | one box, for everything shown | |
| Size | the grid's cell size (Grid only) | |
| Group by | Category, Source, Owner, Type, Name | Category |

The Refresh icon stays in the corner. *Open by ID or link* (online) and
the downloads bar (Update all, Clear Cache) stay as rows under the
filters, shown only when Source admits online, or when there is something
to update.

### One list

Every catalog draws ONE list, the device's entries and the server's pages
merged:

- **Search** filters the device's entries on the client and is sent to
  the server as `q` (debounced as now). No second search box anywhere;
  `FractalConfigGallery`'s own toolbar is not drawn in the Catalog.
- **Duplicates are merged.** An online item the device holds — a curated
  copy of a preset or palette, an adopted script, a downloaded variation
  — is one row, the device's, which knows it is online too (`held`
  already finds most of these; fractals need a match by curated name or
  content hash).
- **Grouping** is a heading per group (collapsible, as now). Group by
  Name means no groups, A–Z.
- **Paging:** online pages are asked for as the end of the list comes
  into view, for every online list the filters admit. Groups fill in as
  pages arrive.
- **Grid** cells use the device's preview (rendered) or the server's
  thumbnail (online), at the Size slider's side. **List** rows are the
  name and a small preview, never expandable.

### Clicking and hovering (every catalog)

- **Hover** shows an entry in the info pane.
- **Click** selects it and loads or applies it (a fractal loads; a
  palette, formula, model or colouring applies). See question 3 for the
  additive kinds (variations, effects).
- When the pointer leaves the list, the pane goes back to the selected
  entry. So the pane's buttons (Download, Rename, Delete, Add to
  transform) always act on what you clicked, which is what the pane shows
  once your pointer is over it.
- **Everything the Fractals list showed when expanded moves to the
  pane** (`list_summary` / the expanded details: mode, transforms,
  variations, formula, model, size, ...). The `›` button goes. On a
  narrow screen (a phone) there is no hover; see question 7.

### Each catalog

| Catalog | Category | Type | Group by | View |
|---|---|---|---|---|
| **Fractals** | 2D Flame, 3D Flame, 2D Escape, 3D Escape, Simulation (`PresetCategory`) | Fractal, Animation | Category, Source, Owner, Type, Name | Grid/List, Size |
| **Palettes** | — | — | Pack, Source, Owner, Name (question 4) | list as now (name + gradient); no Grid, no Size |
| **Variations** | the variation categories (Basic 2D, Advanced 2D, ...) | — | Category, Source, Owner, Name | Grid/List, Size; a **transform selector** |
| **Escape-time** | Formula, Coloring, Field, Field Coloring, Solid, Solid Coloring, Formula File | — | Category, Source, Owner, Name | Grid/List, Size |
| **Simulation** | Model, Coloring | — | Category, Source, Owner, Name | Grid/List, Size |
| **Effects** | the effect stages (question 8) | — | Category, Source, Owner, Name | Grid/List, Size |
| **Scripts** | Generator, Modifier, Library, Track | — | Category, Source, Owner, Name | List |

**Owner** maps as:

| Owner | On the device | Online |
|---|---|---|
| Curated | built-in (`Origin::Builtin`) | curated items |
| Yours | your files, your saved palettes (`Origin::Local`) | your account's items (`Scope::Mine`) |
| Others | what you downloaded or adopted from someone (`Origin::Api`, an owner not you) | see question 2 |

### Variations: the transform selector

A selector in the pane (or above the list), listing Normal, Linked and
Final transforms together, as the triangle editor's does. **Add to
transform** adds to the selected one. Recommended: the same selection as
the triangle editor and Transforms panel (egui memory
`triangle_editor_selected_transform`), so picking a transform anywhere
picks it here.

### Simulation previews

A model's preview renders the model's **first preset** (its parameters,
time step, boundary, initial field, steps and its colouring, all of
which `SimPreset` carries) instead of a default field and colouring. A colouring's preview renders
on a model that suits it: recommended, the first model of the colouring's
state kind, at that model's first preset, instead of Gray-Scott for
everything.

## Order of work

1. **The shell:** the Catalog dropdown, the filter row and view row, the
   Origin selector and Local/Online switch removed, the merged list model
   (device + online, duplicates merged, group by, paging), click and hover,
   the pane absorbing the expanded details. Fractals first, end to end;
   the presets' `FractalConfigGallery` no longer drawn in the Catalog
   (the Fractal Browser keeps it).
2. **Escape-time, Simulation, Effects, Scripts** on the same shell.
3. **Palettes** (Group by, pack checkboxes).
4. **Variations** and the transform selector.
5. **Simulation previews.**
6. The narrow layout (question 7), tests, docs (CATALOG.md rewritten for
   the new layout, UI.md).

Each step is committed on its own and left working; the old views are
deleted as each catalog moves over, not kept beside the new one.

## Open questions

1. **2D vs 3D escape online.** The server filters fractals by
   `render_mode` (2d, 3d, escape, simulation) and has no 2D/3D-escape
   filter. Filtering escape pages on the client leaves pages half empty.
   Options: (a) ask ffapi for a filter (e.g. `view=escape3d`, or a
   meaning for `has_3d` on escape items); (b) online, the two escape
   categories both ask for `escape` and the client sorts them, accepting
   uneven pages. **Recommended: (a), with (b) until ffapi has it.**
2. **Owner "Others" online.** The curated-content plan decided not to
   browse other people's public items: they are opened by id or link.
   Online, should Others (a) browse everyone's public items
   (`Scope::Public`), or (b) mean only what you downloaded or adopted
   from someone? **Recommended: (b) for now; (a) is a policy change for
   the site** (moderation, what the Catalog shows by default).
3. **Click on additive kinds.** For a fractal, palette, formula, model or
   colouring, click = load/apply (it replaces something, and undo takes
   it back). For a **variation** or an **effect**, a click that adds
   means browsing adds things to your fractal by accident. **Recommended:
   click selects (pane shows it, sticky), and Add to transform / Add
   effect in the pane adds.** The alternative is click = add.
4. **Palettes' Group by.** The pack checkboxes (enabled / disabled) live
   on the pack headings. With Group by Source, Owner or Name there are no
   pack headings. **Recommended: add Pack to Group by, as the default;
   Source / Owner / Name as asked; the checkboxes appear only when
   grouped by Pack.**
5. **The online Sort** (Newest, Most liked, Most viewed, Recently
   changed, Name) is not in the new controls. Drop it (rows sorted by
   name within groups), or keep a Sort select beside Group by?
   **Recommended: keep it, shown when Source admits online; it orders
   rows within each group.**
6. **"Yours" or "Account".** A guest's own files are theirs too, with no
   account. **Recommended: "Yours"** (local files you made, and online
   your account's items).
7. **Narrow screens (phones)** have no hover. **Recommended: a tap
   selects and loads (as on desktop), and a Details button in the top row
   (not per row) shows the selected entry's pane.**
8. **Effects and Scripts** were not in the request. **Recommended: the
   same shell; Effects' categories are their stages (density, colour,
   ...), Scripts' their kinds (Generator, Modifier, Library, Track),
   List only for Scripts (no previews).**
9. **Variations' Type.** One kind, so no Type select. Same for the other
   single-kind catalogs. (Stated so it is not a surprise.)

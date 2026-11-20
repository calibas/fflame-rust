# One Catalog view

**Status:** planned 2026-11-20; every question decided the same day
(*Decisions*, below). Branch `one-catalog`. Steps 1-3 and 6 built
2026-11-20 (one shell for every catalog, see *Order of work*); steps 4
(the transform selector) and 5 (simulation previews) to do.

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
| Group by | Category, Source, Owner, Type, Name (also the order: no Sort select, D5) | Category |

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
  Name means no groups, A–Z. Group by is also the order: there is no
  Sort select (D5); online pages are asked for in the server's name
  order (`sort=name`), so a page joins the list where it belongs.
- **Paging:** online pages are asked for as the end of the list comes
  into view, for every online list the filters admit. Groups fill in as
  pages arrive.
- **Grid** cells use the device's preview (rendered) or the server's
  thumbnail (online), at the Size slider's side. **List** rows are the
  name (a palette's: and its gradient), never expandable; the preview is
  the pane's. (Built so: a preview in every row would render one for
  each of 647 variations as the list scrolls, where the list was the
  cheap view.)

### Clicking and hovering (every catalog)

- **Hover** shows an entry in the info pane.
- **Click** selects it and loads or applies it (a fractal loads; a
  palette, formula, model or colouring applies). The additive kinds
  (variations, effects) select instead (D3).
- When the pointer leaves the list, the pane goes back to the selected
  entry. So the pane's buttons (Download, Rename, Delete, Add to
  transform) always act on what you clicked, which is what the pane shows
  once your pointer is over it.
- **Everything the Fractals list showed when expanded moves to the
  pane** (`list_summary` / the expanded details: mode, transforms,
  variations, formula, model, size, ...). The `›` button goes. On a
  narrow screen (a phone) there is no hover (D7).

### Each catalog

| Catalog | Category | Type | Group by | View |
|---|---|---|---|---|
| **Fractals** | 2D Flame, 3D Flame, 2D Escape, 3D Escape, Simulation (`PresetCategory` on the device; online the API's `engine` and `dimension`, D1) | Fractal, Animation | Category, Source, Owner, Type, Name | Grid/List, Size |
| **Palettes** | — | — | Pack (default), Source, Owner, Name (D4) | list as now (name + gradient); no Grid, no Size |
| **Variations** | the variation categories (Basic 2D, Advanced 2D, ...) | — | Category, Source, Owner, Name | Grid/List, Size; a **transform selector** |
| **Escape-time** | Formula, Coloring, Field, Field Coloring, Solid, Solid Coloring, Formula File | — | Category, Source, Owner, Name | Grid/List, Size |
| **Simulation** | Model, Coloring | — | Category, Source, Owner, Name | Grid/List, Size |
| **Effects** | Density, Color (`EffectCategory`, D8) | — | Category, Source, Owner, Name | Grid/List, Size |
| **Scripts** | Generator, Modifier, Library, Track | — | Category, Source, Owner, Name | List |

**Owner** maps as:

| Owner | On the device | Online |
|---|---|---|
| Curated | built-in (`Origin::Builtin`) | curated items |
| Yours | your files, your saved palettes (`Origin::Local`) | your account's items (`Scope::Mine`) |
| Others | what you downloaded or adopted from someone (`Origin::Api`, an owner not you) | nothing: other people's public items are not browsed (D2); they open by id or link |

### Variations: the transform selector

A selector in the pane (or above the list), listing Normal, Linked and
Final transforms together, the triangle editor's own list. **Add to
transform** adds to the selected one. It is the same selection as the
triangle editor's (egui memory `triangle_editor_selected_transform`), so
picking a transform in either picks it in both (decided).

### Simulation previews

A model's preview renders the model's **first preset** (its parameters,
time step, boundary, initial field, steps and its colouring, all of
which `SimPreset` carries) instead of a default field and colouring. A colouring's preview renders
on a model that suits it: the first model of the colouring's state
kind, at that model's first preset, instead of Gray-Scott for everything
(decided). Later, models and colourings may say which go together; not
in this plan.

## Order of work

1. **The shell:** the Catalog dropdown, the filter row and view row, the
   Origin selector and Local/Online switch removed, the merged list model
   (device + online, duplicates merged, group by, paging), click and hover,
   the pane absorbing the expanded details. Fractals first, end to end;
   the presets' `FractalConfigGallery` no longer drawn in the Catalog
   (the Fractal Browser keeps it). **Built 2026-11-20** -- for every
   catalog at once: the filters and the list are generic
   (`ui::inventory_view`), so steps 2 and 3 came with it.
2. **Escape-time, Simulation, Effects, Scripts** on the same shell.
   **Built** (step 1).
3. **Palettes** (Group by, pack checkboxes). **Built** (step 1).
4. **Variations** and the transform selector.
5. **Simulation previews.**
6. The narrow layout (D7), tests, docs (CATALOG.md rewritten for
   the new layout, UI.md). **Built** with step 1.

Each step is committed on its own and left working; the old views are
deleted as each catalog moves over, not kept beside the new one.

## Decisions (2026-11-20)

- **D1. 2D vs 3D escape online: already in the API.** The catalog spec
  ([api-catalog-spec.md](api-catalog-spec.md), built 2026-11-11) gave
  `GET /flames` the filters `engine` (`flame`, `escape`, `simulation`)
  and `dimension` (`2`, `3`), and its list items the fields `engine`,
  `scene`, `dimension`; `render_mode` stays for old clients (its `3d`
  means a 3D flame only, and `has_3d` the same). The app still asks by
  `render_mode`. The categories ask: 2D Flame `engine=flame&dimension=2`,
  3D Flame `engine=flame&dimension=3`, 2D Escape `engine=escape&dimension=2`,
  3D Escape `engine=escape&dimension=3` (solids and terrains),
  Simulation `engine=simulation`. `FlameListItem` gains the three fields.
  Items written before the summariser ran have none; the server
  backfills them, so a row without them falls back to `render_mode`.
- **D2. Owner "Others"** is only what you downloaded or adopted from
  someone else. Other people's public items are not browsed.
- **D3. A click on a variation or an effect selects** (the pane shows
  it); the pane's Add to transform / Add effect adds. Every other kind
  loads or applies on a click.
- **D4. Palettes group by Pack** by default, then Source, Owner, Name;
  the enable checkboxes show only when grouped by Pack.
- **D5. No Sort select.** Group by is the order, to keep the controls
  few.
- **D6. "Yours"**, not "Account": your files on the device, your
  account's items online.
- **D7. Narrow screens:** a tap selects and loads; one Details button in
  the top row shows the selected entry's pane.
- **D8. Effects and Scripts** get the same shell. Effects' categories
  are Density and Color; Scripts' are Generator, Modifier, Library and
  Track, List only (no previews).
- **D9. A catalog of one kind has no Type select** (Variations, Palettes,
  ...): its Category says it.

# Fractal Browser

The Fractal Browser panel browses whole configs — the built-in presets,
a generated batch, the configs in a file, and the flames saved online —
as thumbnails or as a list. Clicking one loads it as a single undo
entry ("Load Preset" in the History panel).

- Panel: `PanelType::FractalBrowser`, drawn by `FractalBrowserPanel`
  ([src/ui/fractal_browser.rs](../../src/ui/fractal_browser.rs)).
- Gallery widget: `FractalConfigGallery`
  ([src/ui/fractal_gallery.rs](../../src/ui/fractal_gallery.rs)), one per
  local tab, and one per preset category.
- Thumbnail cache: `ThumbnailCache`
  ([src/storage/thumbnail_cache.rs](../../src/storage/thumbnail_cache.rs)).
- Thumbnail render: [src/renderer/thumbnail.rs](../../src/renderer/thumbnail.rs).
- Presets: `PresetLibrary` ([src/scene/presets.rs](../../src/scene/presets.rs))
  loading [assets/presets.fflame](../../assets/presets.fflame) through
  [src/resources/presets.rs](../../src/resources/presets.rs).

**See also:** [UI.md](UI.md) (panels and layouts), [ONLINE.md](ONLINE.md)
(the online tab's API), [CONFIG.md](CONFIG.md) (the `.fflame` format).
Design record: [fractal-browser.md](../archive/projects/fractal-browser.md).

---

## Opening it

- **File ▸ From Preset Library** opens it on the Presets tab.
- **Window ▸ Fractal Browser**.
- The **Scripting** workspace docks it on the right.
- It opens by itself, on the right tab, when the Random Generator or the
  Scripts panel produces a batch, or when File ▸ Open reads a file that
  holds more than one config (the first is loaded, the rest wait in the
  browser).

It is available in every render mode.

## Tabs

| Tab label | `BrowserTab` | What it holds |
|---|---|---|
| Presets | `Presets` | The built-in presets, a tab per category (see [Presets](#presets)) |
| Random | `Batch` | The last batch from the Random Generator or a Scripts batch, until the next one replaces it. The label shows the count |
| Files | `Files` | The configs from one file: **Open File...** on the tab, a multi-config `.fflame` or multi-flame `.flame` opened with File ▸ Open, or a multi-flame `.flame` from Import Apophysis. The label shows the count |
| Online | `Online` | The signed-in user's flames from the API. Shown only in online mode |

The code calls the second tab `Batch`; its label (`browser.tab_batch`)
is "Random".

### Files

**Open File...** opens a `.fflame` picker (desktop: a native dialog;
web: the browser's file picker, which hands back the text). A file may
hold one config or a JSON array of them; `FractalConfig::from_json_multi`
reads either. A failed read or parse is shown in red above the gallery.

### Online

The tab lists flames saved through Save Online
([ONLINE.md](ONLINE.md)):

- It fetches on the first visit when signed in, and again on
  **Refresh**. With no filter it lists the user's own flames (the first
  100); a name search or a render-mode filter (All / 2D / 3D / Escape
  Time) uses the search endpoint instead (also 100). The name is
  percent-encoded into the query. There is no Simulation filter: the
  server's `render_mode` enum does not know that value yet.
- Each row is a button reading `name — mode | NT | variations`, where
  `N` is the transform count and the variations are the first two
  names plus a count; an escape-time or simulation row, which has no
  transforms, reads `name — mode` (`online_row_label`). Clicking
  loads the full config together with its online metadata (id,
  visibility, owner, linked animations). The red **X** asks for
  confirmation, then deletes the online flame and refreshes the list.
- An authentication error (401) starts the sign-out flow; a 403 is
  shown as an error and does not sign you out.

The online tab does not use the gallery widget: no thumbnails, no grid,
no list toggle.

---

## The gallery

`FractalConfigGallery` holds a list of `GalleryItem`s (a config plus
its thumbnail hash) and draws:

- **A toolbar**: Grid / List, a **Search** field that filters by name
  (a case-insensitive substring of `config.flame.name`; the escape-time
  presets keep their names there too), and in Grid view a **Size** slider for
  the thumbnails, 64 to 256 points (default 128). Below 480 points of
  panel width the three are stacked rather than wrapped.
- **Grid view**: cards wrap to the panel width; a card shrinks to fit a
  panel narrower than the chosen size. A card shows the thumbnail (a
  grey placeholder until it exists) and the name, truncated to fit.
  Clicking a card loads it.
- **List view**: one collapsible row per config, with a **>** button
  that loads it, the name, and a summary line that depends on the
  engine (`list_summary`, `list_details`):

  | Mode | Summary | Expanded |
  |---|---|---|
  | 2D / 3D flame | `N transforms, mode, variations` (up to three variation names, or the first two and a count) | Zoom, Max Iterations, Color Mode |
  | Escape-time | `mode, formula, coloring` | Zoom as `2^zoom_log2`, Max Iterations |
  | Simulation | `mode, model, colouring` | Steps |

  Expanded, every row also shows a 64-point thumbnail.

## Thumbnails

### Rendering

Every thumbnail is a real render through the unified render API
(`renderer::render` with a `RenderJob`): 512 × 512 pixels, 50 million
iterations, 256 iterations per thread (`THUMBNAIL_SIZE`,
`THUMBNAIL_ITERATIONS`, `THUMBNAIL_ITERATIONS_PER_THREAD`). The render
dispatches by mode, so escape-time and simulation configs render
through their own engines.

- **Desktop**: one thumbnail a frame, blocking
  (`render_thumbnail`, which wraps the async function in
  `pollster::block_on`). A gallery queues its missing thumbnails when it
  is first drawn, so a preset category's are made when it is opened. The
  shown gallery's queue goes first, then any other that was shown and is
  not finished (`next_pending_config`, a `GalleryId` each). While a
  gallery has thumbnails pending it shows a "Generating Thumbnails..."
  window with a progress bar and the name being rendered, instead of its
  contents.
- **Web**: every missing thumbnail is spawned as an async render
  (`render_thumbnail_async`), and finished ones are picked up on later
  frames. The gallery stays usable meanwhile. A preset category's are
  spawned the first time it is shown.

Save Online uploads a thumbnail rendered by the same function, at the
same 512 pixels, as a JPEG.

### The disk cache (desktop)

| | |
|---|---|
| Key | FNV-1a 64-bit hash of the config's JSON, as 16 hex digits (`ThumbnailCache::config_hash`). FNV-1a rather than `DefaultHasher`, which is seeded randomly per run. Any change to a config is a new key, so a stale thumbnail is never shown |
| Format | `{hash}.jpg`: downscaled to 256 × 256 (Lanczos3, the largest size the gallery shows), RGB JPEG at quality 88 |
| Location | the platform cache directory for "Fractals for All / Fractal Art Editor", in `thumbnails/` (on Windows `%LOCALAPPDATA%\Fractals for All\Fractal Art Editor\cache\thumbnails\`) |
| Index | one for every gallery (`shared_thumbnail_cache`), built on first use by scanning the folder; a file with a 16-hex-digit name and any other extension (the old PNG cache) is deleted. A load that finds its file gone forgets it, so the gallery renders that thumbnail again |
| Size limit | 1000 files (`MAX_CACHE_SIZE`, about 25 MB), least recently used evicted first: a load sets the file's modified time |

The web build has no disk cache: thumbnails are rendered again every
session.

---

## Presets

[assets/presets.fflame](../../assets/presets.fflame) is a JSON array of
complete configs. `global_preset_library()` loads it once per run:

- **Desktop** reads the file at run time (so it can be edited without a
  rebuild) and falls back to the copy embedded at compile time
  (`resources::presets::PRESETS_JSON`) when the file cannot be read.
- **Web** uses the embedded copy.
- If the configs fail to parse, or the array is empty, the library holds
  a single "Default" preset (one transform, `linear`). On desktop a file
  that is present but malformed lands here too; it does not fall back to
  the embedded copy.

**The Presets tab has a tab per category** (`PresetCategory`), each with
its count and its own gallery, so opening the browser makes one
category's thumbnails, not all 84.
A category with no presets has no tab, and the browser opens on the first
that has any. `PresetCategory::of` sorts by the camera a config's picture
is seen through (`camera::gesture::view_kind`): a flame by its render
mode; an escape-time config 3D when its formula is a solid -- a
built-in, or its own Rhai solid, told from the source because nothing
has compiled it when the browser sorts the presets -- or it draws a
terrain; every simulation one category, terrains too.

The file holds 84 presets:

| Category | Count | Presets |
|---|---|---|
| 2D Flame | 16 | Grand Julian, Julian Disc, Plastic, Spherical3, Square Tile, Flower; Chladni, Chladni Blur, Cymatics3D Slice, Chladni Disc; Moran: Triangle of Ferns, its four-level form, Carpet of Dragons, Dragon of Carpets, Carpet and Levy C, Sturmian, Dragon and Levy C, Sturmian (the level-dependent example variation) |
| 3D Flame | 6 | Cup (3D), JuliaN Bubble (3D), Bubbles (3D), Cymatics Water; Halayka Escape Paths: Z^2 + C, sin Z + C sin Z |
| 2D Escape | 12 | Ducks; Frothy Basin, Volterra-Lotka, Unity (Fractint's, written in Rhai: each carries its formula; ESCAPE.md, *The formulas*); Sierpinski Gasket, Sierpinski Carpet, Heighway Dragon, Koch Curve, Douady Rabbit, Julia Dendrite, Cubic Pair, Blob Flower (formula `ifs_flame`) |
| 3D Escape | 33 | Quaternion Julia, Lacquered Julia (`quaternion_julia_solid`); KIFS Menger, Tetrahedron, Octahedron, Bloom, Columns, Icosahedron, Icosahedral Flake, Dodecahedron (`kifs_solid`); Mandelbox, Mandelbox Scale 3, Mandelbox Inverted, Mandelbox Julia, Lacquered Mandelbox (`mandelbox_solid`); Mandelbulb, Mandelbulb Power 4, Mandelbulb Julia, Gold Mandelbulb (`mandelbulb_solid`); Halayka's six quaternion sets, the Bristorbrot and its Julia sets, the Quaternionbrot's four (Rhai solids) |
| Simulation | 17 | Gray-Scott Coral, Gray-Scott Mitosis, Brusselator Beans, Cahn-Hilliard Labyrinth, Cyclic Cellular Automaton, Hodgepodge Machine, FitzHugh-Nagumo Vortex, Oregonator Wave Trains, Lichtenberg Figure, Diffusion-Limited Aggregation, Snow Crystal, GG Snowflake, GG Snowflake (3D), Physarum Network, Abelian Sandpile, McCabe Multiscale Turing, Coral Terrain (most began as the visual suite's simulation configs, `tests/visual/configs/sim/`) |

To add a preset, save the config (File ▸ Save As) and append its JSON
object to the array. The desktop app picks it up on the next launch;
the web build needs a rebuild to embed the new file.

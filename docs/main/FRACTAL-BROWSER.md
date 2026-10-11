# Fractal Browser

The Fractal Browser panel browses whole configs — the built-in presets,
a generated batch, the configs in a file, and the flames saved online —
as thumbnails or as a list. Clicking one loads it as a single undo
entry ("Load Preset" in the History panel).

- Panel: `PanelType::FractalBrowser`, drawn by `FractalBrowserPanel`
  ([src/ui/fractal_browser.rs](../../src/ui/fractal_browser.rs)).
- Gallery widget: `FractalConfigGallery`
  ([src/ui/fractal_gallery.rs](../../src/ui/fractal_gallery.rs)), one per
  local tab.
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
| Presets | `Presets` | The built-in presets (see [Presets](#presets)) |
| Random | `Batch` | The last batch from the Random Generator or a Scripts batch, until the next one replaces it. The label shows the count |
| Files | `Files` | The configs from one file: **Open File...** on the tab, or a multi-config `.fflame` opened with File ▸ Open. The label shows the count |
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
  100); a name search or a render-mode filter (All / 2D / 3D) uses the
  search endpoint instead (also 100).
- Each row is a button reading `name — mode | NT | variations`, where
  `N` is the transform count and the variations are the first two
  names plus a count; an escape-time row reads `name — Escape`. Clicking
  loads the full config together with its online metadata (id,
  visibility, owner, linked animations). The red **X** deletes the
  online flame and refreshes the list.
- An authentication error (401/403) starts the sign-out flow.

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
  that loads it, the name, and a summary line
  `N transforms, mode, variations` (up to three variation names, or the
  first two and a count). Expanded, a row shows Zoom, Max Iterations,
  Color Mode and a 64-point thumbnail.

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
  `pollster::block_on`). The current tab's queue goes first, then the
  other local tabs'. While a gallery has thumbnails pending it shows a
  "Generating Thumbnails..." window with a progress bar and the name
  being rendered, instead of its contents.
- **Web**: every missing thumbnail is spawned as an async render
  (`render_thumbnail_async`), and finished ones are picked up on later
  frames. The gallery stays usable meanwhile.

Save Online uploads a thumbnail rendered by the same function, at the
same 512 pixels, as a JPEG.

### The disk cache (desktop)

| | |
|---|---|
| Key | FNV-1a 64-bit hash of the config's JSON, as 16 hex digits (`ThumbnailCache::config_hash`). FNV-1a rather than `DefaultHasher`, which is seeded randomly per run. Any change to a config is a new key, so a stale thumbnail is never shown |
| Format | `{hash}.jpg`: downscaled to 256 × 256 (Lanczos3, the largest size the gallery shows), RGB JPEG at quality 88 |
| Location | the platform cache directory for "Fractals for All / Fractal Art Editor", in `thumbnails/` (on Windows `%LOCALAPPDATA%\Fractals for All\Fractal Art Editor\cache\thumbnails\`) |
| Index | built at startup by scanning the folder; a file with a 16-hex-digit name and any other extension (the old PNG cache) is deleted |
| Size limit | intended at 200 files, oldest-modified first (a load touches the file). See [Known gaps](#known-gaps) |

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

The file holds 20 presets:

| Mode | Count | Presets |
|---|---|---|
| 2D flame | 6 | Grand Julian, Julian Disc, Plastic, Spherical3, Square Tile, Flower |
| 3D flame | 3 | Cup (3D), JuliaN Bubble (3D), Bubbles (3D) |
| Escape-time | 11 | Sierpinski Gasket, Sierpinski Carpet, Heighway Dragon, Koch Curve, Douady Rabbit, Julia Dendrite, Cubic Pair, Blob Flower (formula `ifs_flame`); Sierpinski Tetrahedron, Menger Sponge (`ifs_flame_3d`); Quaternion Julia (`quaternion_julia_solid`) |
| Simulation | 0 | |

To add a preset, save the config (File ▸ Save As) and append its JSON
object to the array. The desktop app picks it up on the next launch;
the web build needs a rebuild to embed the new file.

---

## Known gaps

Recorded from the code as it stands; the escape-time and simulation
cases were added to a browser built for flames.

- **The online filter offers only All, 2D and 3D**
  (`online_search_render_mode` is 0, 1 or 2). There is no Escape or
  Simulation filter, although the API's `ApiRenderMode` has both.
- **A simulation row in the online list reads `name — Simulation | 0T | `.**
  Only escape rows drop the transform and variation columns, and a
  simulation, like an escape fractal, is stored with no transforms and
  no variation names.
- **The list view shows flame details for every config.** The summary's
  transform count and variations come from `config.flame`, and the
  expanded Zoom, Max Iterations and Color Mode are the flame's fields.
  For an escape-time or simulation config only the mode label describes
  the picture (an `ifs_flame` escape preset does render its flame's
  transforms, so its transform count is meaningful).
- **A multi-flame `.flame` (XML) file opened with File ▸ Open lands in
  the Random tab**, not Files: the app loads it with
  `load_batch_into_fractal_browser`, while the comments beside both call
  sites say Files.
- **The disk cache never shrinks.** `enforce_cache_limit` lists `.png`
  files, but the cache now writes `.jpg`, so it finds nothing to
  evict and the folder grows past 200 files. (`ThumbnailCache::clear`,
  unused, also deletes `.png` names.)
- **Grid-card name truncation slices bytes.** The card cuts a long name
  with `&name[..n]` on a byte count, which panics if the cut falls
  inside a multi-byte character (a non-ASCII name longer than the card).

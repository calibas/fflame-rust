# UI Architecture (egui_dock)

**Overview:** All UI is dockable panels built on egui + egui_dock. A
panel can be rearranged, detached, docked to any edge, or closed and
reopened from the Window menu.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system design and module organization
- [I18N.md](I18N.md) - Internationalization support
- [RENDERER.md](RENDERER.md) - Rendering pipeline
- [TRANSFORMS.md](TRANSFORMS.md) - Transform editing
- [../archive/projects/ui-render-modes.md](../archive/projects/ui-render-modes.md) - the
  render-mode project: the survey the current design came from, the
  decisions, and the bugs it found but did not fix

---

## Panels

`PanelType` in [src/ui/workspace.rs](../../src/ui/workspace.rs) defines
**29 panels**. Each renders from its own `src/ui/*.rs` file;
`src/ui/mod.rs` coordinates docking and bubbles cross-panel results
through `UiResponse`.

The Fractal Viewport is the picture and is effectively always present.
The rest group by what they edit:

| Group | Panels |
|---|---|
| The flame | Transforms, Triangle Editor, Variations, Xaos Editor, Subflames, Path Editor, View, Solid & Lighting |
| The other engines | Escape Fractal, Simulation |
| Appearance, shared by every engine | Colors, Palette Editor, Palette Library, Effects |
| Workflow, mode-independent | Fractal Browser, History, Animation, Signals, Scripts, Random Generator, Export, Config Import/Export, Rendering, Performance, Help, Keyboard Shortcuts, Account, Save Online |

**Which of these is available depends on the render mode** — see
[Render modes and the UI](#render-modes-and-the-ui) below. Do not add a
`matches!(render_mode, ...)` to a panel; add a case to
`src/ui/visibility.rs`.

### Workspace layouts

`WorkspaceLayout` (same file) holds the presets: Standard, Animation,
Scripting, Escape Time, Simulation, Compact. Two of them are per-mode
and are applied automatically (below).

- `Workspace::apply_layout` **forces** a rebuild from code. This is
  what "Reset Workspace" and the Window ▸ Workspace Layout menu mean.
- `Workspace::switch_layout` **remembers**: it stashes the dock state
  it is leaving and restores the one it is entering, falling back to
  `apply_layout` the first time a layout is entered. Mode changes use
  this, so switching modes does not throw away an arrangement.

The stash is session-only. Nothing about the dock tree is persisted, so
a restart opens on the built-in layouts. Persisting would need `serde`
on `DockState<PanelType>` plus a versioned `SystemSettings` field.

---

## Menu bar

[src/ui/menu_bar.rs](../../src/ui/menu_bar.rs) draws the desktop bar;
[src/ui/compact_menu.rs](../../src/ui/compact_menu.rs) draws the mobile
hamburger equivalent. They share `MenuActions` / `MenuState`
([menu_context.rs](../../src/ui/menu_context.rs)) — a menu sets a flag
or an `Option`, and `src/ui/mod.rs` acts on it after the frame is drawn.

| Menu | Holds |
|---|---|
| File | New, Open, Save As, Export Flame XML, preset browser, random flame / batch, Save Online, Export PNG, config import/export, Quit |
| Edit | Undo, Redo (Preferences is a disabled stub) |
| View | Reset View, Zoom In / Out |
| **Mode** | The four render modes as radio rows, built from `RenderMode::ALL` |
| Rendering | Pause, Reset Accumulation, Iterations per Thread, Reset to Defaults. **Hidden entirely outside the flame modes** — every item configures the chaos game |
| Window | Reset Workspace, Mobile View, the panel rows, Workspace Layout presets |
| Help | Help panel, Keyboard Shortcuts, Report a Bug, About |

Right-aligned, above 500 points of width: the language picker,
connectivity and account status, and the Fly Mode button (3D only).

**The panel rows in both Window menus come from one table**,
`visibility::WINDOW_MENU`. The compact menu keeps its own order —
touch priority, transforms first — in `COMPACT_WINDOW_MENU`, but takes
its labels from the shared table, and a test asserts it is a subset.
Add a panel row there, not in either menu.

---

## Render modes and the UI

There are four render modes — `RenderMode::{TwoD, ThreeD, Escape,
Simulation}` in [src/scene/transforms.rs](../../src/scene/transforms.rs)
— and they are peers, not a flame with two add-ons. `render_mode` lives
on `FractalConfig`, and the `flame`, `escape` and `sim` sub-configs all
exist at once, each preserved while inert, so switching modes and back
round-trips.

`RenderMode::ALL` is the enum in wire order, kept in step by an
exhaustive match and a length assertion. **Iterate it** rather than
listing four modes by hand.

### Changing the mode

[src/ui/render_mode.rs](../../src/ui/render_mode.rs) is the only place
that writes `ConfigPath::RenderMode`, and a test
(`nothing_outside_this_module_writes_the_render_mode`) scans the source
tree to keep it that way. Whole-config load paths are exempt by name:
they carry a mode in from a file rather than switching.

`switch_render_mode` exists because the switch is not just a
field write. Both non-flame engines produce a unit-range image, and a
flame's Log-calibrated exposure renders that black — so entering
either from a flame mode resets tone mapping to Linear with default
exposure and gamma, batched with the mode change into one undo entry.
Switching between the two non-flame modes does not reset, and **leaving
does not restore**, which is a known wart recorded in the project doc.

Also in that module:

- `layout_for(mode)` — the workspace and editor panel a mode wants.
  `App::follow_loaded_render_mode` consults it; it does not keep a copy.
- `fly_mode_available(mode)` — 3D alone. It used to be asked as "not
  2D", which left Fly Mode live in Escape and Simulation.
- `keeps_escape_engine` / `keeps_sim_engine` — which engine's GPU state
  a mode needs resident (below).
- `mode_label_key` / `mode_tip_key` — the `mode.*` locale keys.

The app compares the mode every frame, so a change from the Mode menu,
a panel, a script or an **undo** all bring the workspace with them.

### What each mode makes available

[src/ui/visibility.rs](../../src/ui/visibility.rs) is the single
answer, and it is exhaustive on purpose: there is no `_` arm, so a new
panel or a new mode will not compile until someone decides what it
means.

```rust
pub enum Vis { Show, Grey(&'static str), Hide }
pub fn panel(p: PanelType, m: RenderMode) -> Vis;
pub fn control(c: Control, m: RenderMode) -> Vis;
pub fn gated(ui, c: Control, m: RenderMode, body) -> Option<R>;
```

The `&'static str` is a locale key naming the **reason**, shown on
hover, so a user who goes looking for a control learns why it is not
there instead of wondering whether it exists.

**No panel is ever `Hide`.** Hiding a panel's menu row would make it
unreachable *and* undiscoverable, so an unavailable panel is greyed in
the Window menu and its body says the same thing. `Hide` is for whole
sections inside a panel.

The rule of thumb the tables encode:

- **Flame-only** (greyed in both non-flame modes): View, Xaos Editor,
  Subflames, Path Editor, and Random Generator, whose output would
  leave the mode.
- **Transform editors** (Transforms, Triangle Editor, Variations):
  available in Simulation too, because the flame's transforms are the
  simulation's per-layer warps when `sim.use_transforms` is on. Greyed
  in Escape.
- **3D only**: Solid & Lighting.
- **Each engine's own panel** hides the other's.
- **Everything else is available everywhere**, because both non-flame
  engines write an image in the flame accumulator's layout and go
  through the same density-effects → tonemap → colour-effects tail.

`Control` groups controls that share a fate rather than naming each
widget. In the non-flame modes it hides the chaos-game controls, the
tone-map presets, Reset Colors, the spatial filter, density levels and
the colour-mode selector, and greys the tone-map mode selector, the
logarithmic-only sliders and the alpha-blend pair.

### Engine lifetimes

Both non-flame renderers are created lazily and are released when the
mode changes away from them (`App::release_inactive_engines`). The two
cases differ, and the difference matters:

- The **escape** renderer rebuilds itself from the config, so returning
  costs only the re-render. It holds 16 bytes per pixel of
  supersampled area at minimum — about 506 MB for a 1080p viewport at
  4x antialiasing — which is why this is worth doing.
- The **simulation** grid *is* its state, so returning restarts it from
  the seed at step 0.

---

## Panel reference

Every panel renders from its own file. `PanelViewer::render_panel`
([panel_viewer.rs](../../src/ui/panel_viewer.rs)) is the dispatch, and
it is the authoritative list — the table below is a map, not a
contract.

| Panel | File |
|---|---|
| Fractal Viewport | `panel_viewer.rs` (`render_fractal_viewport`) |
| Transforms | [transforms.rs](../../src/ui/transforms.rs) |
| Triangle Editor | [triangle_editor.rs](../../src/ui/triangle_editor.rs) |
| Variations | [variations.rs](../../src/ui/variations.rs) |
| Xaos Editor | [xaos_editor.rs](../../src/ui/xaos_editor.rs) |
| Subflames | [subflames.rs](../../src/ui/subflames.rs) |
| Path Editor | [path_editor.rs](../../src/ui/path_editor.rs) |
| View | [view.rs](../../src/ui/view.rs) |
| Solid & Lighting | [solid_panel.rs](../../src/ui/solid_panel.rs) |
| Escape Fractal | [escape_panel.rs](../../src/ui/escape_panel.rs) |
| Simulation | [sim_panel.rs](../../src/ui/sim_panel.rs) |
| Colors / Tone Mapping | [tone_mapping.rs](../../src/ui/tone_mapping.rs) |
| Palette Editor | [palette_editor.rs](../../src/ui/palette_editor.rs) |
| Palette Library | [palette_library.rs](../../src/ui/palette_library.rs) |
| Effects | [effects_panel.rs](../../src/ui/effects_panel.rs) |
| Rendering | [settings.rs](../../src/ui/settings.rs) |
| Performance | [performance.rs](../../src/ui/performance.rs) |
| History | [undo_history.rs](../../src/ui/undo_history.rs) |
| Animation | [animation_panel.rs](../../src/ui/animation_panel.rs), [track_editor.rs](../../src/ui/track_editor.rs) |
| Signals | [signal_panel.rs](../../src/ui/signal_panel.rs) |
| Scripts | [scripts_panel.rs](../../src/ui/scripts_panel.rs) |
| Fractal Browser | [fractal_browser.rs](../../src/ui/fractal_browser.rs) |
| Random Generator | [random_generator.rs](../../src/ui/random_generator.rs) |
| Export | [export_panel.rs](../../src/ui/export_panel.rs) |
| Config Import/Export | [config_dialog.rs](../../src/ui/config_dialog.rs) |
| Help, Keyboard Shortcuts | [help.rs](../../src/ui/help.rs) |
| Account, Save Online | [login_dialog.rs](../../src/ui/login_dialog.rs), [save_online_dialog.rs](../../src/ui/save_online_dialog.rs) |

Three panels have mechanics worth knowing before editing them.

### Triangle Editor

Visual affine editing: each transform is drawn as a triangle whose
vertices are the affine basis (O, X, Y). Dragging a vertex converts
screen space → fractal space → affine coefficients and writes them back.

Its accumulation behaviour is deliberate: GPU parameters update
*during* the drag for live feedback, and the iteration reset happens
on release, so a drag does not restart the render on every mouse move.
In 3D it offers an XY / YZ / ZX plane selector; in 2D there is only XY.

### Rendering

Almost everything here configures the chaos game — max iterations,
iterations per thread, burn-in, deterministic RNG, the blend controls,
pause and reset accumulation — so it is hidden outside the flame modes
(`Control::ChaosGame`). What survives is VSync and the frame cap, which
are `SystemSettings` device preferences rather than fractal parameters,
plus escape's deep-zoom orbit cache in Escape mode.

Note that pause is read in two places that both already exclude the
non-flame modes, so it was inert there long before it was hidden.

### Colors

Four sections: tone mapping, tone curve, density levels, and colour &
appearance. Both non-flame engines go through this same tail, so most
of it applies to them — but the logarithmic branch never runs there,
density levels are suppressed in the frame loop, the spatial filter
lives inside a compute pass that is never dispatched, and the preset
dropdown would set a Log-calibrated look that renders a unit-range
image black. `Control::*` in `visibility.rs` encodes which is which.

---

## UI Response System

### UiResponse Struct
**Purpose:** Communicate UI changes back to main app for processing

**Location:** [src/ui/mod.rs](../../src/ui/mod.rs)

**Fields:**
```rust
pub struct UiResponse {
    pub flame_changed: bool,          // Transform/variation/weight changed
    pub view_changed: bool,           // Zoom/pan/rotation changed
    pub palette_changed: bool,        // Palette selected
    pub camera_changed: bool,         // Camera pitch/yaw changed (3D mode)
    pub reset_requested: bool,        // Manual reset button
    pub config_export: Option<PathBuf>, // Save config to file
    pub config_import: Option<PathBuf>, // Load config from file
    pub palette_export: Option<PathBuf>, // Save palette to file
    pub palette_import: Option<PathBuf>, // Load palette from file
    pub export_png: Option<PathBuf>,  // Export opaque PNG
    pub export_transparent_png: Option<PathBuf>, // Export with alpha
    pub preset_changed: bool,         // Preset selected
    pub add_transform: bool,          // Add new transform
    pub delete_transform: bool,       // Remove current transform
    pub undo: bool,                   // Undo requested (Ctrl+Z)
    pub redo: bool,                   // Redo requested (Ctrl+Y)
}
```

**Usage Pattern:**
```rust
// In render() function:
let ui_response = egui_layer.render_ui(&mut flame, ...);

// Handle responses:
if ui_response.flame_changed {
    renderer.update_flame(&flame);
    renderer.reset();  // Clear accumulation
}

if ui_response.view_changed {
    renderer.update_iterations(...);
    renderer.reset();
}

if ui_response.preset_changed {
    let config = preset_library.get(preset_index);
    app.import_config(config);  // Includes undo capture
}
```

**Code:** [src/app/mod.rs](../../src/app/mod.rs) - `render()` function handles all responses

---

## Input Handling

### Keyboard Input
**Location:** [src/app/input.rs](../../src/app/input.rs) - `handle_keyboard()`

**Shortcuts:**
- **Arrow Keys** - Pan view (rotation-aware)
  - Up: Pan in rotated "up" direction
  - Down: Pan in rotated "down" direction
  - Left: Pan in rotated "left" direction
  - Right: Pan in rotated "right" direction
- **+/=** - Zoom in (1.1x)
- **-/_** - Zoom out (0.9x)
- **Ctrl+Z** - Undo
- **Ctrl+Y** - Redo
- **R** - Reset view (zoom=1, pan=0, rotation=0)

**Rotation-Aware Panning (Added 2025-10-24):**
```rust
// Convert screen delta to fractal space
let cos_r = rotation.cos();
let sin_r = rotation.sin();
let fractal_dx = screen_dx * cos_r - screen_dy * sin_r;
let fractal_dy = screen_dx * sin_r + screen_dy * cos_r;
```

**Behavior:**
- Only processed if egui doesn't consume event
- Sets `view_changed_by_keyboard` flag
- Triggers reset on next frame

### Mouse Input

#### Mouse Button
**Location:** [src/app/input.rs](../../src/app/input.rs) - `handle_mouse_button()`

**Left Button:**
- **Press** - Start drag (if not over egui)
- **Release** - End drag

**State Tracking:**
```rust
pub struct App {
    dragging: bool,
    last_mouse_pos: Option<PhysicalPosition<f64>>,
    // ...
}
```

#### Mouse Move
**Location:** [src/app/input.rs](../../src/app/input.rs) - `handle_mouse_move()`

**Behavior:**
- If `dragging == true`:
  - Calculate delta from `last_mouse_pos`
  - Apply rotation-aware panning (same as keyboard)
  - Update pan_x, pan_y
  - Set `view_changed_by_keyboard` flag
- Update `last_mouse_pos`

**Panning Formula:**
```rust
// Screen space delta (pixels)
let delta_x = (pos.x - last.x) as f32;
let delta_y = (pos.y - last.y) as f32;

// Convert to fractal space (normalized + rotation-aware)
let scale = 2.0 / (zoom * height as f32);
let screen_dx = delta_x * scale;
let screen_dy = delta_y * scale;

// Apply inverse rotation
let fractal_dx = screen_dx * cos_r - screen_dy * sin_r;
let fractal_dy = screen_dx * sin_r + screen_dy * cos_r;

// Update pan (invert Y for screen coordinates)
pan_x += fractal_dx;
pan_y -= fractal_dy;
```

#### Mouse Wheel
**Location:** [src/app/input.rs](../../src/app/input.rs) - `handle_mouse_wheel()`

**Behavior:**
- Zoom toward cursor position (not screen center)
- Zoom factor: 1.1x per scroll unit
- Adjusts pan to keep cursor position fixed

**Zoom-to-Cursor Formula:**
```rust
// Cursor position in fractal space (before zoom)
let cursor_world_x = (cursor_screen_x - width/2) / (zoom * height/2) - pan_x;
let cursor_world_y = (cursor_screen_y - height/2) / (zoom * height/2) - pan_y;

// Apply zoom
zoom *= 1.1;  // or 0.9 for zoom out

// Cursor position in fractal space (after zoom) should be same
// Adjust pan to compensate:
let new_cursor_world_x = (cursor_screen_x - width/2) / (zoom * height/2) - new_pan_x;
// Solve: cursor_world_x = new_cursor_world_x
new_pan_x = pan_x + (cursor_world_x - new_cursor_world_x);
```

**Code:** Both App keyboard handler and egui's Settings window arrow buttons use identical rotation logic

---

## State Management Integration

**See [CONFIG.md](CONFIG.md)** for complete ConfigManager documentation.

### Delta-Based State Management (Added 2025-10-31)

The UI uses **ConfigManager** for all parameter updates, replacing the old flag-based approach.

**Location:** [src/config/manager.rs](../../src/config/manager.rs)

**Key Principles:**
1. All parameter changes flow through ConfigManager
2. Automatic undo/redo with delta tracking
3. Type-safe ConfigPath identification
4. Selective GPU updates via UpdateType
5. Lazy undo throttling for continuous controls

**Modern UI Pattern (Slider Helpers):**
```rust
use crate::config::slider::{lazy_slider, config_slider};
use crate::config::delta::{ConfigPath, UpdateType};

// View Window (lazy undo for smooth dragging)
let update_type = lazy_slider(ui, config_manager, ConfigPath::Zoom, 0.1..=10.0)
    .text("Zoom")
    .show();

// Tone Mapping Window (immediate undo)
let update_type = config_slider(ui, config_manager, ConfigPath::Exposure, 0.1..=5.0)
    .text("Exposure")
    .suffix("x")
    .show();

// Handle updates in App::render()
match update_type {
    UpdateType::View => {
        let config = config_manager.active_config();
        renderer.update_view(config.zoom, config.pan_x, config.pan_y, config.rotation);
        renderer.reset();
    }
    UpdateType::ToneMap => {
        let config = config_manager.active_config();
        renderer.update_tonemap(/* ... */);
        // No reset for tone mapping
    }
    _ => {}
}
```

**Legacy Pattern (Being Phased Out):**
```rust
// OLD: Manual flags and capture_state()
if ui.add(egui::Slider::new(&mut value, 0.0..=1.0)).changed() {
    app.capture_state();
    flame_changed = true;
}
// NEW: ConfigManager with automatic undo
let update_type = config_slider(ui, config_manager, ConfigPath::SomeParam, 0.0..=1.0).show();
```

**Undo/Redo:**
- **Automatic:** ConfigManager captures undo states on parameter changes
- **Throttled:** LazyUndoHelper prevents spam during slider drags (500ms minimum)
- **History:** 50 states (circular buffer, oldest states dropped)
- **Keyboard:** Ctrl+Z (undo), Ctrl+Y (redo)
- **Visual History:** Undo History window shows all deltas with descriptions

**Code:**
- [src/config/manager.rs](../../src/config/manager.rs) - ConfigManager implementation
- [src/config/slider.rs](../../src/config/slider.rs) - UI helpers (lazy_slider, config_slider)
- [src/ui/undo_history.rs](../../src/ui/undo_history.rs) - Undo history window

### Accumulation Reset
**When to reset:**
- ✅ Flame changed (transforms, variations, weights, colors)
- ✅ View changed (zoom, pan, rotation)
- ✅ Camera changed (pitch, yaw) - 3D mode
- ✅ Palette changed
- ✅ Global render params changed (density_scale, accumulation controls)
- ✅ Manual reset button
- ❌ NOT during triangle editor drag (only on release)

**Code:**
```rust
// In renderer/compute_kernel.rs:
pub fn reset(&mut self) {
    // Clear accumulation textures to black
    // Reset sample counter to 0
    // Does NOT modify GPU params
}
```

---

## UI Features

### Key Features (Summary)
- **Menu Bar** - File, Edit, View, Mode, Rendering, Window, Help
- **Collapsible Sections** - All sections can be collapsed to save space
- **Real-time Updates** - Most changes update immediately without reset
- **Smart Accumulation** - Triangle editor only resets when dragging stops
- **Undo/Redo** - Ctrl+Z/Ctrl+Y for all state changes
- **Variation Parameters** - Float/Integer/Angle sliders appear below active variations (Added 2025-10-22)
- **Rotation-Aware Panning** - All input methods respect view rotation (Added 2025-10-24)
- **Zoom to Cursor** - Mouse wheel zooms toward cursor position
- **Visual Transform Editing** - Triangle editor with drag-to-update

### UI Ordering Rules

**Variation Display Order:**
- ✅ Sort by category first (Basic 2D, Advanced 2D, 3D Depth, etc.)
- ✅ Within category, sort by registration order (from VariationRegistry)
- ❌ NEVER use HashMap iteration (random order!)

**Correct Implementation:**
```rust
// In ui/mod.rs:
for name in registry.ordered_names() {
    let info = registry.get(name);
    if info.category == current_category {
        // Render variation slider
    }
}
```

---

## Common UI Modification Tasks

### Add a panel
1. Add a variant to `PanelType` ([workspace.rs](../../src/ui/workspace.rs)) and a title arm in its `Display` impl.
2. Add a case to `visibility::panel` — the match is exhaustive, so this is not optional.
3. Add a row to `visibility::WINDOW_MENU` (and to `COMPACT_WINDOW_MENU` if it belongs on a phone).
4. Add a dispatch arm in `PanelViewer::render_panel` ([panel_viewer.rs](../../src/ui/panel_viewer.rs)).
5. Add the `menu.window_*` and `panels.*` locale keys.
6. Update the panel count in the tests that assert it.

### Add a control that only some modes can use
1. Add a variant to `visibility::Control`, or reuse one that shares its fate.
2. Answer it in `visibility::control`.
3. Wrap the widget in `visibility::gated(ui, Control::X, mode, |ui| { ... })`, or check `control(...) != Vis::Hide` for a whole section.
4. Add the reason's locale key under `visibility.*`.

Do **not** write `matches!(config.render_mode, ...)` inside a panel.
That is what the policy module replaced, and scattered copies are how
the menus and the dispatcher came to disagree.

### Add New Control/Slider (cross-panel results only)
Most controls write straight through `config_manager.update_param`.
`UiResponse` is for results another part of the app must act on.

1. Add field to `UiResponse` struct
2. Add UI widget in appropriate window section
3. Set response field when value changes
4. Handle response in `app.rs` `render()` function
5. Update GPU buffers if needed
6. Capture state for undo if needed

### Add Keyboard Shortcut
1. Add case to `handle_keyboard()` in [src/app/input.rs](../../src/app/input.rs)
2. Set appropriate flag (`view_changed_by_keyboard`, etc.)
3. Handle flag in next frame's `render()` function

### Modify Triangle Editor
1. Edit [src/ui/triangle_editor.rs](../../src/ui/triangle_editor.rs)
2. Vertex dragging converts screen space to fractal space to affine coefficients
3. Smart accumulation: update GPU params during the drag, reset on release

---

## Internationalization

**Framework:** rust-i18n v3.1 with YAML translation files in `locales/`

**Language Selector:** the menu bar's right-hand strip (the globe), when
the window is at least 500 points wide
- Native language names, applied immediately without a restart
- Loads the matching font via `ensure_font_for_locale`

**Current Support:**
- English (en) is the reference and is complete
- Spanish, Japanese and Simplified Chinese are partial: roughly 270
  lines each against English's 2,200, so most new keys are
  English-only in practice

**Translation Coverage:**
- All menu items and panel titles
- Transform and variation controls
- Color and rendering settings
- Tooltips and help text
- Error messages and notifications

**Font Support (egui default):**
- ✅ Full: Latin scripts, Cyrillic, Greek
- ⚠️ Limited: CJK (Chinese, Japanese, Korean) - basic characters only
- ❌ No support: Arabic/Hebrew (RTL languages)
- For full CJK: Add Noto Sans CJK font via egui FontDefinitions

**See:** [I18N.md](I18N.md) for complete translation guide

---

**Last Updated:** 2025-11-13
**Related Documentation:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system design
- [I18N.md](I18N.md) - Internationalization guide
- [RENDERER.md](RENDERER.md) - Rendering pipeline (not yet created)
- [TRANSFORMS.md](TRANSFORMS.md) - Transform editing (not yet created)

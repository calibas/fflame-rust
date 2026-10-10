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
| The camera, every mode's | View (its camera, image, atmosphere, depth of field, and lighting & material; see [The View panel](#the-view-panel)) |
| The flame | Transforms, Triangle Editor, Variations, Xaos Editor, Subflames, Paths |
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
| Rendering | Pause, Reset Accumulation, Iterations per Thread, Frame-Time Governor (on/off) and the Workgroups a frame dispatches while it is off, Reset to Defaults. **Hidden entirely outside the flame modes** — every item configures the chaos game |
| Window | Reset Workspace, Mobile View, the panel rows, Workspace Layout presets |
| Help | Help panel, Keyboard Shortcuts, Report a Bug, About |

Right-aligned, above 500 points of width: the progress bar
(rightmost), the language picker, connectivity and account status, and
the Fly Mode button (any 3D view: `MenuState::fly_available`, from
`visibility::fly_mode`).

There is no Solid & Lighting row in either Window menu: its controls
are the View panel's Lighting & Material section. The panel type stays
so a saved layout that docks it still loads, and draws that section.

### The progress bar

One bar for every rendering task
([src/ui/render_progress.rs](../../src/ui/render_progress.rs)). The app
picks the one task it reports each frame (`App::render_progress`), in
this order:

| Task | The bar |
|---|---|
| A PNG or video export | Fills to the export's fraction. Its headline and detail are the hover text. The export takes the bar over, as it pauses the viewport's render |
| Animation playback | Sweeps: it runs without end in every mode |
| Flame (2D / 3D) | Fills to `total_iterations / max_iterations`; grey when finished or paused |
| Escape | Sweeps while the render refines (unsettled, preview window, texture in flight); full and grey once settled |
| Simulation | Fills to `step / Max Steps` while short of the cap; sweeps when running past it (or with no cap); full and grey when paused at the cap |

It replaced the export overlay, and it costs nothing it can avoid:
- It reads counters the frame loop already keeps, with no readback.
- It never requests a frame. Every state in which it sweeps is one in
  which the app is already redrawing.
- Its text is built only while the pointer is on it.

Two event-loop rules exist for it:
- **An export redraws at 10 Hz.** It used to redraw at the display
  rate, competing with the export thread for the GPU.
- **A stale bar gets one more frame.** The UI is built before the
  frame's render runs, so the bar can show a frame-old state: an escape
  render that has just settled, a simulation that has just paused. When
  the state differs from what was drawn (`progress_drawn`), the loop
  draws once more and then sleeps.

The compact menu shows the same bar as a strip under its button, only
while something runs.

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
pub fn panel(p: PanelType, m: RenderMode, solid: Solid) -> Vis;
pub fn control(c: Control, m: RenderMode, tone: ToneMapMode) -> Vis;
pub fn fly_mode(kind: ViewKind) -> Vis;
```

The `&'static str` is a locale key naming the **reason**, shown on
hover, so a user who goes looking for a control learns why it is not
there instead of wondering whether it exists.

**No panel is ever `Hide`.** Hiding a panel's menu row would make it
unreachable *and* undiscoverable, so an unavailable panel is greyed in
the Window menu and its body says the same thing. `Hide` is for whole
sections inside a panel.

The rule of thumb the tables encode:

- **Every mode's**: the View panel, the camera of whatever the
  viewport shows.
- **Flame-only** (greyed in both non-flame modes): Xaos Editor,
  Subflames, Paths, and Random Generator, whose output would
  leave the mode.
- **Transform editors** (Transforms, Triangle Editor, Variations):
  available in Simulation too, because the flame's transforms are the
  simulation's per-layer warps when `sim.use_transforms` is on. Greyed
  in Escape.
- **3D views only**: Solid & Lighting (`Solid::of` -- a 3D flame, a
  solid, a terrain), and fly mode (`fly_mode(ViewKind)`, a question
  about the camera rather than the mode).
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

### The timeline and the simulation grid

`sim.steps` means two things, and **whoever is driving decides which**.
To the transport it is Max Steps, a CAP: the run stops there once, and
`0` means uncapped. To the animation timeline it is a TARGET: the
picture at a given time is the state at that step count. The exporter
has always read it the second way; the app read it the first way only,
which is why a `Sim.Steps` track used to do nothing in the app but
move a cap.

**The timeline owns the step count** when it is playing an animation
that has a `Sim.Steps` track, or when a target it committed has not
been reached yet (`App::timeline_owns_sim`). While it does:

- the grid is advanced toward the track's value, **budgeted** per
  display frame from the measured cost of a step
  (`SimRenderer::steps_in`), so a two-thousand-step jump is walked
  over a few frames rather than freezing the window — a scrub can ask
  for one on every slider event;
- Run / Pause / Step and the spacebar are inert, and the panel greys
  them. Run is *disengaged* when playback starts rather than merely
  ignored: left engaged it would resume the instant the timeline let
  go, and the run would carry on under a button nobody pressed;
- playback is **paced by the grid**, as escape playback is by
  settling — the controller is held while the grid is short of the
  frame's target, then advanced by everything that elapsed. A heavy
  model plays slower rather than showing a lagging state, so what
  plays is what exports.

An animation with **no** step track leaves all of this alone: the
transport keeps working and the run keeps going underneath, so
animating a colouring parameter over a free-running simulation still
does what it always did.

**Going backwards.** The rule is not invertible, so reaching a lower
step count means reseeding and re-running. A falling target is
therefore HELD while the motion is continuous and applied only on a
discrete event — a scrubber release, a `Loop` wrap, or a `PingPong`
turnaround at t = 0 (`sim::timeline_target_applies`,
`app::animation_update::playback_motion`). Without the hold, dragging
left would restart the run on nearly every slider event, ping-pong
would restart once per frame for half of every cycle, and a track
written to count down would do the same.

So a track written 2000 → 0 plays in-app as a still of the highest
state reached, with the panel saying why and pointing at the
scrubber; **export renders every frame of it correctly**, because a
video frame is the state at its time whatever the direction. Both
rules are pure functions in [src/sim/mod.rs](../../src/sim/mod.rs)
with tests that need neither a GPU nor an `App`.

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
| Paths | [paths_panel.rs](../../src/ui/paths_panel.rs) |
| View | [view.rs](../../src/ui/view.rs), [view_controls.rs](../../src/ui/view_controls.rs) |
| Solid & Lighting (kept for saved layouts) | [solid_panel.rs](../../src/ui/solid_panel.rs) |
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

## The View panel

One panel, in every mode (docs/projects/camera-unification.md, C5):
[view.rs](../../src/ui/view.rs) lays it out by the shown camera's
`ViewKind`, from the shared widgets in
[view_controls.rs](../../src/ui/view_controls.rs).

- **Camera** (2D views, and a 3D flame's projected picture):
  `camera_2d` -- zoom, centre (the escape centre as exact decimals),
  rotation, an arrow pad and Reset, through `camera::gesture` as the
  keys and the mouse are.
- **3D Camera** (every 3D view): `camera_3d` -- position, target, zoom
  or distance, pitch, yaw, bank, roll, field of view or perspective,
  Reset, and fly mode with its settings. A camera that stores a target
  shows its eye as a readout; a terrain's target has a lift.
- **Lens and Image**: an escape view's lens, antialiasing and
  downsampling; a simulation's resolve filters, fit and tiling.
- **Atmosphere, Depth of Field, Lighting & Material** (every 3D view):
  fog, a flame's depth weighting, a terrain's far and haze; a flame's
  focus and blur or the path tracer's lens; the render tier and the
  path tracer, a terrain's shadows, the lights, and the picture's one
  material (docs/projects/materials.md).
- **The flame's own**: deep zoom, the 2D/3D switch, Preserve Z, post
  symmetry.

Angles show in degrees on a −180..180 slider that takes typed values
beyond (`view_controls::angle_slider`); the config stores radians.

---

## Input Handling

One set of controls drives every camera (camera-unification C3). The
gestures are pure functions on the config in
[src/camera/gesture.rs](../../src/camera/gesture.rs) and
[src/camera/fly.rs](../../src/camera/fly.rs), each returning a
`CameraEdit` (the writes and their history entry) that the caller
applies; `gesture::view_kind` decides which camera they move.

| | 2D view | 3D view | 3D, fly mode |
|---|---|---|---|
| Drag | pan | pan (a terrain: along the ground) | look (about the eye) |
| Alt+drag, right drag | rotate (grab and twist) | orbit the target (a turntable) | look |
| Wheel | zoom toward the cursor | dolly toward the cursor (a flame: its 2D zoom) | flying speed |
| Shift | finer steps | finer steps | sprint |
| W A S D Q E | -- | -- | fly (Q/E: screen up in FreeLook, world up in FPS) |
| Arrows, + / − | pan, zoom | pan, dolly | -- |
| One finger | pan (rotate with the Turn toggle) | pan (orbit with the Turn toggle) | look |
| Long press, then drag | rotate | orbit | look |
| Two fingers | pinch zoom, twist rotate, drag pan | pinch dolly, twist orbit, drag pan | -- |

- **The viewport** ([panel_viewer.rs](../../src/ui/panel_viewer.rs)):
  `view_drag` and `view_scroll` are the one input path, used by the
  viewport body and the tab-bar strip above it alike.
- **Touch**: `TouchTracker`, because egui's own multi-touch does not
  work on the web. A finger held still within 10 points for half a
  second is a **long press -- the touch's right button**: held, it reads
  the path in PathMap (as holding the right button does); dragged, it
  turns. The **Pan/Turn toggle** at the viewport's bottom left is the
  touch's **Alt**: a sticky choice of what a one-finger drag does. It
  shows on the compact layout and once a touch has been seen.
- **Keys** ([src/app/input.rs](../../src/app/input.rs),
  `handle_keyboard`): arrows pan 5% of the view a press and + / −
  zoom by 1.5, through the same gestures; Ctrl/Cmd+Z and +Y undo and
  redo; Space plays the animation, or runs the grid in Simulation mode
  (`render_mode::space_runs_the_simulation`; inert while the timeline
  owns the step count); F toggles full screen; **F2 toggles fly mode**
  in any 3D view. Keys go to the app only when egui has not consumed
  them -- except a fly key's release, which always lets go, as does
  losing the window's focus.
- **History**: one entry name per gesture, all coalescing -- pan, zoom,
  rotate, orbit, and the fly camera's -- and Reset View as its own entry.
  During animation playback a gesture writes silently.
- **Path readback**: in PathMap mode only, holding the right button (or
  a long press) queries the path under the pointer.

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
2. A key that moves the view goes through `camera::gesture` (as the
   arrows and + / − do), so it moves whichever camera is shown
3. Otherwise set the appropriate flag (`view_changed_by_keyboard`,
   etc.) and handle it in the next frame's `render()`
4. List it in the Help panel's controls ([help.rs](../../src/ui/help.rs))

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

# UI Architecture (egui_dock - Migrated 2025-11-13)

**Overview:** The fractal flame renderer uses egui with egui_dock for its flexible docking panel system. All windows have been migrated to dockable panels that can be rearranged, detached, and docked anywhere.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system design and module organization
- [I18N.md](I18N.md) - Internationalization support
- [RENDERER.md](RENDERER.md) - Rendering pipeline (not yet extracted)
- [TRANSFORMS.md](TRANSFORMS.md) - Transform editing (not yet extracted)

---

## Panel Layout (Docking System)

**Migration Status:** ✅ Complete (2025-11-13)
- All windows converted to dockable panels (1:1 mapping)
- egui_dock integration complete
- Users can rearrange, detach, and dock panels anywhere
- Future: Save/restore workspace layouts

The UI consists of a menu bar plus 7 dockable panels:

```
┌────────────────────────────────────────────────────────────────┐
│ Menu Bar: File  Edit  View  Fractal  Rendering  Window  Help  │
└────────────────────────────────────────────────────────────────┘
                             │
                             │  ┌─────────────────────┐
                             │  │  Fractal Viewport   │
                             │  │  (always visible)   │
┌──────────────────┐        │  │                     │
│    Settings      │◄───────┼─►│  Main rendering     │
│                  │        │  │  display with live  │
│ - File & Project │        │  │  fractal output     │
│ - Rendering      │        │  │                     │
│ - Export         │        │  └─────────────────────┘
│ - Preferences 🆕 │        │
│   └─ Language 🆕 │        │  ┌─────────────────────┐
└──────────────────┘        │  │    Transforms       │
                            └─►│                     │
┌──────────────────┐           │ - Add/Delete        │
│ Triangle Editor  │           │ - Affine params     │
│                  │           │ - Variation weights │
│ - Visual editing │           │ - Parameters        │
│ - Drag handles   │           └─────────────────────┘
│ - Real-time      │
└──────────────────┘           ┌─────────────────────┐
                               │    View             │
┌──────────────────┐           │                     │
│  Tone Mapping    │           │ - Zoom, Pan, Rot    │
│  & Colors        │           │ - 3D Camera         │
│                  │           │ - Projection        │
│ - Color mode     │           └─────────────────────┘
│ - Palette        │
│ - Tone curve     │           ┌─────────────────────┐
│ - Background     │           │  Palette Editor     │
└──────────────────┘           │                     │
                               │ - Gradient preview  │
┌──────────────────┐           │ - Color stops       │
│  Undo History    │           │ - Import/export     │
│                  │           └─────────────────────┘
│ - Visual browser │
│ - Jump to state  │
│ - Delta preview  │
└──────────────────┘

All panels can be:
- Dragged to rearrange
- Detached into floating windows
- Docked to any edge
- Closed/reopened via Window menu
```

### Menu Bar (Enhanced 2025-11-13)
**Location:** Top of screen

**Contents:**
- **File**: New, Open, Save, Import/Export, Recent Files, Quit
- **Edit**: Undo, Redo, Preferences
- **View**: Reset View, Fit to Window, Zoom, 2D/3D Mode
- **Fractal**: Transform operations, Palette operations
- **Rendering**: Pause/Resume, Reset, Speed, Iterations
- **Window**: Panel visibility toggles, Layout presets
- **Help**: Documentation, Keyboard Shortcuts, About

**Note:** Most menu actions are future work - currently shows structure

**Code:** [src/ui/menu_bar.rs](../../src/ui/menu_bar.rs)

### Performance Window
**Purpose:** Real-time performance monitoring and global render settings

**Sections:**
1. **Performance Metrics**
   - FPS (frames per second)
   - Frame time (ms)
   - Total iterations rendered
   - Iterations per second

2. **Preset Selection**
   - Dropdown list of built-in + loaded presets
   - Loads complete FractalConfig (flame + view + rendering + colors)

3. **Render Mode** (Added 2025-10-21)
   - Toggle: 2D (classic) / 3D (pseudo-3D with depth)
   - Dynamically switches compute shader pipeline

4. **Camera Controls** (3D mode only)
   - Camera Pitch slider (-180° to 180°) - Up/down orbit
   - Camera Yaw slider (-180° to 180°) - Left/right orbit
   - Reset button (pitch=0, yaw=0)

5. **Projection Type** (3D mode only)
   - Toggle: Orthographic (flat) / Perspective (depth-aware)
   - Perspective Strength slider (0.0 to 10.0)

**Code:** [src/ui/mod.rs](../../src/ui/mod.rs) - `render_ui()` Performance section

### Transforms Window
**Purpose:** Edit individual transforms (affine + variations + color + weight)

**Header:**
- Transform selector dropdown (Transform 0, Transform 1, etc.)
- "➕ Add Transform" button - Creates new default transform
- "🗑 Delete Transform" button - Removes current transform (min 1 required)

**Affine Transform Section:**
- 6 sliders: a, b, c, d, e, f (2x2 matrix + translation)
- Range: -2.0 to 2.0
- Formula: `[x', y'] = [[a, b], [c, d]] * [x, y] + [e, f]`

**Z Offset Section** (3D mode only):
- g slider (-2.0 to 2.0) - Offset in Z axis

**Variations Section:**
- Collapsible categories:
  - Basic 2D (Linear, Sinusoidal, Spherical, Swirl, Horseshoe)
  - Advanced 2D (Polar, Handkerchief, Heart, Disc, Spiral, Hyperbolic, Diamond, Ex, Julia, Bent, Waves)
  - 3D Depth (Zcone, Flatten, ZScale) - 3D mode only
  - 3D Full (Hemisphere) - 3D mode only
  - 3D Rotation (PreRotateX/Y, PostRotateX/Y) - 3D mode only
- Weight sliders (0.0 to 2.0) for each variation
- Parameter sliders appear below active variations (Float, Integer, Angle types)
  - Example: JuliaN shows "Power" (integer) and "Distance" (float)
  - Example: Blob shows "High", "Low", "Waves" (all floats)

**Color Section:**
- RGB sliders (0.0 to 1.0)
- Color speed slider (0.0 to 1.0) - Blend rate with previous color

**Weight Section:**
- Weight slider (0.01 to 10.0) - Transform selection probability

**Code:** [src/ui/mod.rs](../../src/ui/mod.rs) - `render_ui()` Transforms section

### Settings Window
**Purpose:** Global view, rendering, color, and export settings

**View Controls:**
- Zoom slider (0.1 to 10.0) with Reset button
- Pan X/Y sliders (-10.0 to 10.0) with Reset buttons
- Rotation slider (-180° to 180°) with Reset button
- Arrow buttons for pan (respects rotation angle)
- Center View button (zoom=1, pan=0, rotation=0)

**Rendering Settings:**
- **System Settings** (device-specific, persist across sessions):
  - VSync checkbox (desktop only) - Toggle vertical sync (locks FPS to monitor refresh)
  - Target FPS slider (desktop only, when VSync off) - 10 to 1000 Hz
  - WASM: VSync always enabled (WebGPU Fifo mode required), controls hidden
- Workgroups slider (1 to 512) - Parallel compute units
- Iterations/Thread slider (16 to 4096) - Samples per thread
- Speed multiplier (1x/2x/4x/8x/16x) - Frame rate or chunking
- Density Scale slider (0.01 to 10.0) - Alpha multiplier

**Accumulation Controls:**
- Blend Rate slider (0.01 to 1.0) - Exponential blend speed
- Dynamic Blend Mode toggle - Exponential vs fixed rate
- Low-Density Smoothing slider (0.0 to 1.0) - Noise reduction
- Density Compression slider (0.0 to 100.0) - Bright area detail
- Target Iterations/Pixel slider (0 to 1M) - Per-pixel limit

**Histogram Settings:**
- Color Scale slider (1.0 to 1000.0) - U32 encoding precision

**Color Settings:**
- Color Mode: Transform / Palette / Speed
- Palette dropdown (built-in + loaded palettes)
- Background color RGB sliders (0.0 to 1.0)
- Speed Factor slider (0.0 to 2.0) - Speed mode sensitivity

**Tone Mapping:**
- Tonemap Mode: Logarithmic / Linear
- Use Curve toggle - Apply S-curve adjustment
- Tonemap Curve slider (0.0 to 10.0) - S-curve strength
- Exposure slider (0.1 to 10.0)
- Gamma slider (0.5 to 3.0)

**Export:**
- "Export PNG" button - Save current viewport to file
- "Export Transparent PNG" button - Save with alpha channel

**Config Import/Export:**
- "Save Config" button - Export .fflame file
- "Load Config" button - Import .fflame file
- "Copy Config to Clipboard" button - JSON export
- "Import Config from Clipboard" button - JSON import

**Undo/Redo:**
- "Undo" button (Ctrl+Z) - Revert last change
- "Redo" button (Ctrl+Y) - Restore undone change

**Code:** [src/ui/mod.rs](../../src/ui/mod.rs) - `render_ui()` Settings section

### Triangle Editor Window (Added 2025-10-21)
**Purpose:** Visual editing of transform affine parameters via dragging triangles

**Display:**
- 600×600 canvas showing unit square bounds
- Each transform rendered as colored triangle (3 vertices)
- Bounding boxes showing transform output range
- Grid lines at unit intervals
- Current transform highlighted in red

**Interaction:**
- **Left-click + drag vertex** - Move triangle point
  - Updates affine matrix (a,b,c,d,e,f) in real-time
  - No accumulation reset while dragging (smooth updates)
- **Release mouse** - Trigger accumulation reset
- **Hover vertex** - Highlight in white
- **Click outside** - Deselect

**Smart Accumulation:**
- Updates GPU params every frame during drag
- Only resets accumulation when drag completes
- Provides immediate visual feedback without flickering

**Math:**
```rust
// Identity triangle vertices (before transform)
v0 = [0.0, 0.0]
v1 = [1.0, 0.0]
v2 = [0.5, 0.866]  // equilateral

// After affine transform
v0' = [e, f]
v1' = [a + e, c + f]
v2' = [b/2 + a/2 + e, 0.866*d + c/2 + f]

// Inverse: dragging v' updates [a,b,c,d,e,f]
```

**Code:** [src/ui/mod.rs](../../src/ui/mod.rs) - `render_triangle_editor()`

### Palette Editor Window (Added 2025-10-20)
**Purpose:** Create and edit color palettes with gradient stops

**Sections:**

1. **Palette Library**
   - Dropdown list of built-in + loaded palettes
   - Applies selected palette to current flame

2. **Gradient Preview**
   - 400×40 color bar showing interpolated gradient
   - Visual representation of full 256-color palette

3. **Color Stops** (editable)
   - List of stops with position (0.0-1.0) and RGB color
   - Position slider for each stop
   - RGB sliders (0.0 to 1.0) for each stop
   - "Remove Stop" button (min 2 stops required)

4. **Add Stop**
   - "Add Color Stop" button - Insert new stop at 0.5

5. **Import/Export Palette**
   - "Export Palette" button - Save .palette file
   - "Copy Palette to Clipboard" button - JSON export
   - "Load Palette" button - Import .palette file
   - "Import Palette from Clipboard" button - JSON import
   - Imported palettes automatically added to library

**Palette Format:**
```json
{
  "name": "My Palette",
  "stops": [
    { "position": 0.0, "color": [1.0, 0.0, 0.0] },
    { "position": 0.5, "color": [0.0, 1.0, 0.0] },
    { "position": 1.0, "color": [0.0, 0.0, 1.0] }
  ]
}
```

**Code:** [src/ui/mod.rs](../../src/ui/mod.rs) - `render_ui()` Palette Editor section

### Undo History Window (Added 2025-10-31)
**Purpose:** Visual browser for undo/redo history with human-readable delta descriptions

**Sections:**

1. **Undo Stack**
   - Scrollable list of all undo states (up to 50)
   - Each entry shows ConfigPath description (e.g., "Transform 2 → Affine a")
   - Clickable to jump directly to any past state
   - Current position highlighted

2. **Redo Stack**
   - Scrollable list of all redo states
   - Appears after undoing changes
   - Cleared when new change is made

**Features:**
- **Jump to State:** Click any delta to jump directly to that configuration
- **Visual Indicator:** Current position shown with highlighted entry
- **Human-Readable:** ConfigPath::Display generates descriptions like:
  - "Exposure" (simple parameter)
  - "Transform 2 → Affine a" (indexed affine parameter)
  - "Transform 1 → Linear variation" (variation weight)
  - "Transform 3 → JuliaN power" (variation parameter)
- **Real-Time Updates:** Automatically updates as changes are made

**Example Display:**
```
Undo History
┌──────────────────────────────────┐
│ • Transform 2 → Affine a         │ ← Current
│   Transform 2 → Affine d         │
│   Transform 1 → Linear variation │
│   Exposure                       │
│   Zoom                           │
│   ...                            │
└──────────────────────────────────┘

Redo Stack
┌──────────────────────────────────┐
│   Gamma                          │
│   Background Color               │
└──────────────────────────────────┘
```

**Code:** [src/ui/undo_history.rs](../../src/ui/undo_history.rs)

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
- **Menu Bar** - Toggle window visibility (Added 2025-10-21)
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

### Add New Window
1. Add show/hide boolean to App struct
2. Add menu bar checkbox in `render_menu_bar()`
3. Add window rendering in `render_ui()`
4. Add response fields to `UiResponse` if needed
5. Handle responses in `app.rs` `render()` function

### Add New Control/Slider
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
1. Edit `render_triangle_editor()` in [src/ui/mod.rs](../../src/ui/mod.rs)
2. Vertex dragging logic converts screen → fractal space → affine matrix
3. Smart accumulation: update GPU params during drag, reset on release

---

## Internationalization (Added 2025-11-13)

**Framework:** rust-i18n v3.1 with YAML translation files

**Language Selector:** Settings → Preferences section
- Dropdown with native language names (e.g., "English (English)")
- Changes apply immediately (no restart required)
- Persists via `set_locale()`

**Current Support:**
- English (en) - Complete with 200+ strings
- Ready for community translations

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

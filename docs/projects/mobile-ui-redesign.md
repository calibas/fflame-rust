# Mobile UI/UX Redesign

**Status:** Planning
**Priority:** High — prerequisite for iOS and Android native apps
**Depends on:** Nothing
**Blocks:** [iOS Native App](ios-native-app.md), [Android Native App](android-native-app.md)

## Goal

Make the existing UI work well on mobile screens and touch input. This is not a separate mobile app or a code fork — it's a new `WorkspaceLayout::Compact` preset and a floating menu button that replaces the top menu bar. The same egui_dock code runs everywhere; only the layout and entry point differ.

## Compact Mode Detection

No OS detection needed. Use logical window size on first frame:

```rust
let logical_width = ctx.screen_rect().width();
// screen_rect() returns logical points (physical pixels / native_pixels_per_point)
// Phone at 1080px physical with 3x scaling = 360 logical
// Tablet at 2x = ~500-640 logical
let is_compact = logical_width < 600.0;
```

- Check once at startup (first frame), not per-frame
- Save result to `SystemSettings` so it persists across sessions
- User can manually switch layout via the menu (e.g., tablet user who prefers desktop layout)
- No re-evaluation on resize — layout is sticky once chosen

## Layout: `WorkspaceLayout::Compact`

### Full-Screen Viewport

Add a new `WorkspaceLayout::Compact` variant to the existing `WorkspaceLayout` enum. The layout creates a `DockState` with just `PanelType::FractalViewport` filling the entire main surface. No side panels, no top menu bar.

```rust
fn create_compact_layout() -> DockState<PanelType> {
    DockState::new(vec![PanelType::FractalViewport])
}
```

### Floating Menu Button

Instead of `egui::TopBottomPanel::top("menu_bar")`, render a small floating button via `egui::Area` anchored to the top-right corner. Tapping it opens a popup/dropdown with the same menu items as the current top menu bar (File, Edit, View, etc.).

- Button is semi-transparent, small enough to not obstruct the viewport
- Fades out after 5 seconds of inactivity (no touch, click, drag, or keypress)
- Reappears on any input event
- Menu popup appears below the button on tap
- Opening a panel from the menu docks it based on orientation

### Orientation-Aware Panel Docking

When a panel is opened from the floating menu, dock position depends on orientation:

- **Portrait** (height > width): `split_below` at ~0.67 — panel takes bottom 1/3
- **Landscape** (width ≥ height): `split_right` at ~0.67 — panel takes right 1/3

In both cases:
1. Fractal viewport automatically resizes to fill the remaining 2/3
2. Opening a different panel replaces the current panel (or adds as a tab)
3. Closing the panel returns to full-screen viewport

Orientation is checked each time a panel is opened (not on resize), so rotating the device affects where the *next* panel docks.

This uses standard egui_dock APIs — no custom panel system needed.

### egui_dock on Mobile

egui_dock works on mobile — tab dragging, dividers, and docking all function with touch input. The UX isn't ideal (small drag targets, thin dividers), but it's functional. The compact layout avoids these issues by keeping things simple, while still allowing power users to rearrange panels if they want to.

## Touch Gesture Mapping

egui has built-in multi-touch support. Wire these to the fractal viewport:

| Gesture | egui API | Fractal Action |
|---------|----------|----------------|
| One-finger drag | Mouse emulation (built-in) | Pan |
| Pinch | `InputState::zoom_delta()` | Zoom in/out |
| Two-finger rotate | `InputState::rotation_delta()` | Rotate view |
| Two-finger pan | `InputState::translation_delta()` | Pan (alternative) |
| Long press (0.6s) | `Response::secondary_clicked()` | Context menu |
| Double tap | Detect manually | Reset view / fit to window |

These APIs already exist in egui — the work is connecting them to the camera/view system.

## UI Scaling

- Larger slider handles, bigger buttons
- More padding between interactive elements

## DPI / High-DPI Screens

egui handles this automatically:

- `native_pixels_per_point` is set by the platform (2.0-3.0 on mobile)
- All widgets are sized in logical points
- No special handling needed beyond the zoom factor adjustment above

## Features to Hide in Compact Mode

- Keyboard shortcut references in menus
- Top menu bar (replaced by floating menu button)

## Triangle Editor

The Triangle Editor relies on precise mouse dragging of triangle vertices. On a small touch screen:
- Triangles are too small to interact with accurately
- Fingers obscure the triangle being dragged
- Multi-touch conflicts with pinch/zoom gestures

This needs a dedicated touch-friendly rework — larger drag handles, possibly a different interaction model (e.g., select triangle first, then adjust with sliders). This is a separate sub-task, not part of the initial compact layout work. The Triangle Editor can be hidden from the compact menu initially.

## Implementation Order

1. Add `WorkspaceLayout::Compact` variant and `create_compact_layout()`
2. Add compact mode detection (logical width < 600 on first frame, save to SystemSettings)
3. Create floating menu button with 5-second fade-out on inactivity
4. Implement orientation-aware panel docking (bottom in portrait, right in landscape)
5. Wire touch gestures to viewport (pinch zoom, two-finger rotate)
6. Set zoom factor to 1.4 in compact mode
7. Conditionally hide desktop-only features (keyboard shortcuts)
8. Test on mobile browsers (WASM) before native apps
9. (Later) Touch-friendly Triangle Editor rework

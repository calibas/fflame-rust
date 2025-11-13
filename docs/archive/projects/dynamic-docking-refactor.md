# Dynamic Docking Refactor

## Goal
Convert from fixed left/right SidePanel layout to fully dynamic docking system where users can drag tabs anywhere and dock to any edge (left, right, top, bottom).

## Current Architecture

**Layout:**
```
[Menu Bar]
[Left SidePanel] [Fractal Center] [Right SidePanel]
```

**Problems:**
- Two separate `DockState` instances (left and right)
- Can't drag tabs between left and right docks
- Fixed panel locations
- Duplicate `PanelContext` creation code

## Target Architecture

**Layout:**
```
[Menu Bar]
[DockArea with transparent background covers screen]
[Fractal renders first, then egui on top with gaps showing fractal]
```

**Benefits:**
- Single `DockState` - all tabs draggable anywhere
- Users can dock to any edge (left, right, top, bottom)
- Simpler code (one DockState, one PanelViewer creation)
- More flexible for users

## Technical Approach

### Option A: Full-Screen DockArea with Transparent Styling

Use `DockArea::show()` directly without `SidePanel`, with custom styling:

```rust
egui::CentralPanel::default()
    .frame(egui::Frame::none()) // No background
    .show(ctx, |ui| {
        egui_dock::DockArea::new(&mut workspace.dock_state)
            .style(transparent_dock_style()) // Custom style
            .show(ctx, &mut panel_viewer);
    });
```

**Key Styling:**
- Transparent tab bar backgrounds
- Transparent panel backgrounds
- Only tab content has backgrounds

**Challenge:** Need to ensure fractal shows through empty areas.

### Option B: Empty CentralPanel + DockArea Windows

Don't put DockArea in CentralPanel at all - just let it manage floating windows:

```rust
// Menu bar at top
menu_bar::render_menu_bar(...);

// Empty central area shows fractal
egui::CentralPanel::default()
    .frame(egui::Frame::none())
    .show(ctx, |_ui| {
        // Empty - fractal shows through
    });

// DockArea manages windows that can dock to edges
egui_dock::DockArea::new(&mut workspace.dock_state)
    .show(ctx, &mut panel_viewer);
```

`DockArea::show()` (without `show_inside()`) renders windows that can:
- Float anywhere
- Dock to screen edges
- Be dragged between docked and floating states

**This is the recommended approach** - it's what egui_dock is designed for.

### Option C: Keep SidePanels but Merge DockStates

Simplest change - just merge left and right into one:

```rust
egui::SidePanel::right("dock_panel")
    .show(ctx, |ui| {
        egui_dock::DockArea::new(&mut workspace.dock_state)
            .show_inside(ui, &mut panel_viewer);
    });
```

**Pros:**
- Minimal code change
- Fractal guaranteed to show on left
- All tabs in one dock = draggable between

**Cons:**
- Can't dock to left edge (panel is fixed right)
- Can't dock to top/bottom
- Less flexible than Options A/B

## Recommended Implementation: Option B

**Why:**
- Leverages egui_dock's full capabilities
- Most flexible for users
- Clean separation: fractal background, dock windows on top
- Natural behavior users expect from IDE-style docking

**Implementation Steps:**

### 1. Update Workspace Structure
```rust
// src/ui/workspace.rs
pub struct Workspace {
    /// Single dock state for all panels
    pub dock_state: DockState<PanelType>,
    /// Active layout preset
    pub current_layout: WorkspaceLayout,
}
```

**Changes:**
- Remove `left_dock_state` and `right_dock_state`
- Add single `dock_state`
- Update layout functions to use single state
- Update `panel_exists()` to check single state
- Update `open_floating_panel()` to use single state

### 2. Update Layouts

**Beginner:**
```rust
fn create_beginner_layout() -> DockState<PanelType> {
    let mut state = DockState::new(vec![PanelType::Colors]);
    state.push_to_focused_leaf(PanelType::Transforms);
    state
}
```

**Standard:**
```rust
fn create_standard_layout() -> DockState<PanelType> {
    let mut state = DockState::new(vec![PanelType::Transforms]);
    state.push_to_focused_leaf(PanelType::Colors);
    state.push_to_focused_leaf(PanelType::View);
    state.push_to_focused_leaf(PanelType::Rendering);
    state.push_to_focused_leaf(PanelType::History);
    state
}
```

All panels start in one tabbed group - users can split and rearrange.

### 3. Update UI Rendering

**Before (src/ui/mod.rs):**
```rust
// Left SidePanel with left_dock_state
egui::SidePanel::left("left_dock_panel").show(ctx, |ui| {
    DockArea::new(&mut workspace.left_dock_state)
        .show_inside(ui, &mut panel_viewer);
});

// Right SidePanel with right_dock_state
egui::SidePanel::right("right_dock_panel").show(ctx, |ui| {
    DockArea::new(&mut workspace.right_dock_state)
        .show_inside(ui, &mut panel_viewer);
});
```

**After:**
```rust
// Empty central panel shows fractal
egui::CentralPanel::default()
    .frame(egui::Frame::none())
    .show(ctx, |_ui| {
        // Fractal renders first (before egui), shows through here
    });

// DockArea manages dockable windows on top
egui_dock::DockArea::new(&mut workspace.dock_state)
    .show(ctx, &mut panel_viewer);
```

**Only create PanelViewer once** - huge code simplification.

### 4. Test Styling

May need to adjust `DockArea` style to ensure proper transparency:

```rust
use egui_dock::Style;

let mut style = Style::from_egui(ctx.style());
// Ensure tab bars have some background so they're visible
// But keep content areas with appropriate backgrounds
workspace.dock_state.style = style;
```

### 5. Handle Edge Cases

- **Initial window placement:** Start with reasonable default positions
- **Save/restore layout:** Consider serializing window positions
- **Empty state:** If all windows closed, show hint to open from Windows menu

## Migration Plan

### Phase 1: Refactor Workspace (1 file)
- Update `workspace.rs` to use single `DockState`
- Update all layout functions
- Update helper methods (`panel_exists`, `open_floating_panel`)

### Phase 2: Update UI Rendering (1 file)
- Modify `ui/mod.rs` to use single `DockArea`
- Remove duplicate `PanelViewer` creation
- Test that fractal shows through

### Phase 3: Test & Polish
- Test all panels can be opened
- Test drag-and-drop between positions
- Test docking to different edges
- Verify fractal visibility
- Check for any styling issues

### Phase 4: Update Documentation
- Update workspace layout comments
- Update CLAUDE.md if needed

## Estimated Effort

- **Phase 1:** 30-45 minutes
- **Phase 2:** 15-30 minutes
- **Phase 3:** 30-60 minutes (testing/polish)
- **Phase 4:** 10 minutes

**Total:** 1.5-2.5 hours

## Risks

1. **Fractal visibility:** CentralPanel with `Frame::none()` should work, but need to verify
2. **Window positioning:** DockArea might not have sensible defaults for window positions
3. **User confusion:** Free-form docking is more flexible but less structured

## Fallback

If Option B doesn't work well, Option C (single right SidePanel) is a safe fallback that still achieves most goals:
- Single DockState
- All tabs draggable within the panel
- Guaranteed fractal visibility
- 30 minutes to implement

## Open Questions

1. Should we save/restore window positions between sessions?
2. What's the default layout for floating windows?
3. Do we want any panels to be "always on top" or have special positioning?

## Decision

**Proceed with Option B** - full dynamic docking with `DockArea::show()` at top level. If issues arise, fall back to Option C.

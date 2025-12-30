# App Module Refactoring

**Status:** In Progress
**Created:** 2025-12-29
**Priority:** High (blocking other work)

## Problem

`src/app/mod.rs` is 2,137 lines (108KB) with a single `render()` function of ~1,640 lines. This causes:
- Edit tool reliability issues (file modification detection)
- Difficult to navigate and understand
- Hard to test individual components
- Merge conflicts likely

## Current Structure

```
src/app/
├── mod.rs          # 2,137 lines - THE PROBLEM
├── config.rs       # 10KB - config import/export helpers
├── export.rs       # 9KB - headless export
├── input.rs        # 5KB - keyboard/mouse input
└── render_mode.rs  # 10KB - FSM for render modes
```

## Target Structure

```
src/app/
├── mod.rs              # ~400 lines - App struct, new(), orchestration
├── config.rs           # existing
├── export.rs           # existing
├── input.rs            # existing
├── render_mode.rs      # existing
├── ui_handlers.rs      # NEW: ~500 lines - UI response processing
├── animation.rs        # NEW: ~200 lines - animation update logic
└── gpu_updates.rs      # NEW: ~300 lines - GPU buffer updates
```

## Extraction Plan

### Phase 1: Extract UI Response Handlers

Create `ui_handlers.rs` with functions for each category:

```rust
// src/app/ui_handlers.rs

impl App {
    /// Handle all UI responses - main dispatcher
    pub(super) fn handle_ui_responses(&mut self, ui_response: &UiResponse) {
        self.handle_viewport_resize(ui_response);
        self.handle_config_operations(ui_response);
        self.handle_transform_operations(ui_response);
        self.handle_palette_operations(ui_response);
        self.handle_file_operations(ui_response);
        self.handle_undo_redo(ui_response);
        self.handle_panel_requests(ui_response);
        self.handle_animation_responses(ui_response);
        self.handle_path_filters(ui_response);
    }

    fn handle_viewport_resize(&mut self, ui_response: &UiResponse) { ... }
    fn handle_config_operations(&mut self, ui_response: &UiResponse) { ... }
    fn handle_transform_operations(&mut self, ui_response: &UiResponse) { ... }
    fn handle_palette_operations(&mut self, ui_response: &UiResponse) { ... }
    fn handle_file_operations(&mut self, ui_response: &UiResponse) { ... }
    fn handle_undo_redo(&mut self, ui_response: &UiResponse) { ... }
    fn handle_panel_requests(&mut self, ui_response: &UiResponse) { ... }
    fn handle_animation_responses(&mut self, ui_response: &UiResponse) { ... }
    fn handle_path_filters(&mut self, ui_response: &UiResponse) { ... }
}
```

**Lines to extract:** ~600-1630 (approximately 1,000 lines)

### Phase 2: Extract Animation Logic

Create `animation.rs` (or rename/extend existing animation module):

```rust
// src/app/animation.rs

impl App {
    /// Update animation state and apply values to config
    /// Returns whether animation is currently playing
    pub(super) fn update_animation(&mut self, delta_time: f64) -> bool {
        let was_fsm_animating = self.render_mode.is_animating();
        let is_controller_playing = self.animation_controller.state == PlaybackState::Playing;

        self.handle_animation_start(is_controller_playing, was_fsm_animating);
        self.handle_animation_stop(is_controller_playing, was_fsm_animating);

        if is_controller_playing {
            self.advance_animation(delta_time);
        }

        is_controller_playing
    }

    fn handle_animation_start(&mut self, playing: bool, was_animating: bool) { ... }
    fn handle_animation_stop(&mut self, playing: bool, was_animating: bool) { ... }
    fn advance_animation(&mut self, delta_time: f64) { ... }
}
```

**Lines to extract:** ~1633-1717 (approximately 85 lines)

### Phase 3: Extract GPU Update Logic

Create `gpu_updates.rs`:

```rust
// src/app/gpu_updates.rs

impl App {
    /// Process pending actions and update GPU buffers
    pub(super) fn process_gpu_updates(&mut self, view_changed_by_keyboard: bool) {
        let actions = self.config_manager.get_pending_actions();

        if self.needs_gpu_update(&actions, view_changed_by_keyboard) {
            self.execute_gpu_updates(&actions, view_changed_by_keyboard);
        }

        self.update_overwrite_mode(&actions);
        self.config_manager.clear_pending_actions();
    }

    fn needs_gpu_update(&self, actions: &UpdateAction, keyboard: bool) -> bool { ... }
    fn execute_gpu_updates(&mut self, actions: &UpdateAction, keyboard: bool) { ... }
    fn update_overwrite_mode(&mut self, actions: &UpdateAction) { ... }
}
```

**Lines to extract:** ~1718-1860 (approximately 140 lines)

### Phase 4: Slim Down render()

After extractions, `render()` becomes:

```rust
fn render(&mut self, window: &Window) -> Result<(), SurfaceError> {
    // Setup (surface, encoder, timing)
    let surface_output = self.gpu.surface.get_current_texture()?;
    let delta_time = self.calculate_delta_time();

    // PHASE 1: Render UI
    let ui_response = self.render_ui(window, &surface_output);

    // PHASE 2: Handle UI responses
    self.handle_ui_responses(&ui_response);

    // Animation update (before GPU updates)
    let is_animating = self.update_animation(delta_time);

    // GPU updates
    self.process_gpu_updates(self.view_changed_by_keyboard);
    self.view_changed_by_keyboard = false;

    // PHASE 3: Render fractal
    self.render_fractal(&surface_output, is_animating)?;

    // Present
    surface_output.present();
    Ok(())
}
```

## Execution Order

1. Create `ui_handlers.rs` - extract UI response handling
2. Create `gpu_updates.rs` - extract GPU update logic
3. Create `app/animation.rs` - extract animation logic
4. Clean up `mod.rs` - simplify render()

## Notes

- All new files use `impl App` with `pub(super)` visibility
- Keep `App` struct definition in `mod.rs`
- Move helper functions along with their callers
- Run `cargo check` after each extraction to catch errors

## Phase 1 Tracking: ui_handlers.rs ↔ mod.rs

| Handler Function | mod.rs Lines | Status | Verified Same? |
|------------------|--------------|--------|----------------|
| `handle_config_operations` | 600-653 | ✅ Done | ✅ Yes |
| `handle_transform_operations` | 655-729 | ✅ Done | ✅ Yes |
| `handle_random_flame` | 731-751 | ✅ Done | ✅ Yes |
| `handle_palette_operations` | 753-771, 917-1073 | ✅ Done | ✅ Yes |
| `handle_file_operations` | 773-915 | ✅ Done | ✅ Yes |
| `handle_undo_redo` | 1075-1081 | ✅ Done | ✅ Yes |
| `handle_panel_requests` | 1083-1091 | ✅ Done | ✅ Yes |
| `handle_animation_requests` | 1093-1178 | ✅ Done | ✅ Yes |
| `handle_preset_selection` | 1180-1187 | ✅ Done | ✅ Yes |
| `handle_animation_seek` | 1599-1621 | ✅ Done | ✅ Yes |
| `handle_path_filters` | 1623-1632 | ✅ Done | ✅ Yes |

**Phase 1 Complete!**
- `mod.rs`: 2,137 → 1,517 lines (620 lines removed, 29% reduction)
- `ui_handlers.rs`: 678 lines (new)

**NOT extracted (staying in mod.rs):**
- PNG export: lines 603-938 (complex, platform-specific)
- Animation export: lines 940-1012 (complex, platform-specific)

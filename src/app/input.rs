use winit::event::*;
use crate::app::App;

impl App {
    pub(super) fn handle_keyboard(&mut self, event: &KeyEvent) {
        use winit::keyboard::{KeyCode, PhysicalKey};

        // Only handle key press (not release)
        if !event.state.is_pressed() {
            return;
        }

        // Check for Ctrl/Cmd modifier
        let ctrl_or_cmd = {
            #[cfg(target_os = "macos")]
            { self.modifiers.super_key() }
            #[cfg(not(target_os = "macos"))]
            { self.modifiers.control_key() }
        };

        // Handle undo/redo with logical key
        use winit::keyboard::Key;
        if ctrl_or_cmd {
            if let Key::Character(ref c) = event.logical_key {
                let c_lower = c.to_lowercase().to_string();
                if c_lower == "z" {
                    self.undo();
                    return;
                } else if c_lower == "y" {
                    self.redo();
                    return;
                }
            }
        }

        let pan_step = 0.1 / self.zoom;

        // Pre-calculate rotation for arrow controls
        // Negate rotation to convert screen space to fractal space
        let cos_r = (-self.rotation).cos();
        let sin_r = (-self.rotation).sin();

        match event.physical_key {
            PhysicalKey::Code(KeyCode::ArrowUp) => {
                // Up in screen space: (0, -1), rotate to fractal space
                let screen_dx = 0.0;
                let screen_dy = -pan_step;
                let new_pan_x = self.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                let new_pan_y = self.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                let _ = self.config_manager.update_batch(
                    vec![
                        (crate::config::ConfigPath::PanX, new_pan_x.into()),
                        (crate::config::ConfigPath::PanY, new_pan_y.into()),
                    ],
                    "Pan Up (Keyboard)".to_string(),
                    false  // Immediate capture for keyboard input
                );
                self.pan_x = self.config_manager.active_config().pan_x;
                self.pan_y = self.config_manager.active_config().pan_y;
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::ArrowDown) => {
                // Down in screen space: (0, 1), rotate to fractal space
                let screen_dx = 0.0;
                let screen_dy = pan_step;
                let new_pan_x = self.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                let new_pan_y = self.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                let _ = self.config_manager.update_batch(
                    vec![
                        (crate::config::ConfigPath::PanX, new_pan_x.into()),
                        (crate::config::ConfigPath::PanY, new_pan_y.into()),
                    ],
                    "Pan Down (Keyboard)".to_string(),
                    false  // Immediate capture for keyboard input
                );
                self.pan_x = self.config_manager.active_config().pan_x;
                self.pan_y = self.config_manager.active_config().pan_y;
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::ArrowLeft) => {
                // Left in screen space: (-1, 0), rotate to fractal space
                let screen_dx = -pan_step;
                let screen_dy = 0.0;
                let new_pan_x = self.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                let new_pan_y = self.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                let _ = self.config_manager.update_batch(
                    vec![
                        (crate::config::ConfigPath::PanX, new_pan_x.into()),
                        (crate::config::ConfigPath::PanY, new_pan_y.into()),
                    ],
                    "Pan Left (Keyboard)".to_string(),
                    false  // Immediate capture for keyboard input
                );
                self.pan_x = self.config_manager.active_config().pan_x;
                self.pan_y = self.config_manager.active_config().pan_y;
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::ArrowRight) => {
                // Right in screen space: (1, 0), rotate to fractal space
                let screen_dx = pan_step;
                let screen_dy = 0.0;
                let new_pan_x = self.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                let new_pan_y = self.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                let _ = self.config_manager.update_batch(
                    vec![
                        (crate::config::ConfigPath::PanX, new_pan_x.into()),
                        (crate::config::ConfigPath::PanY, new_pan_y.into()),
                    ],
                    "Pan Right (Keyboard)".to_string(),
                    false  // Immediate capture for keyboard input
                );
                self.pan_x = self.config_manager.active_config().pan_x;
                self.pan_y = self.config_manager.active_config().pan_y;
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::Equal) | PhysicalKey::Code(KeyCode::NumpadAdd) => {
                let new_zoom = self.zoom * 1.5;
                let _ = self.config_manager.update_param(
                    crate::config::ConfigPath::Zoom,
                    new_zoom.into(),
                    false  // Immediate capture for keyboard input
                );
                self.zoom = self.config_manager.active_config().zoom;
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::Minus) | PhysicalKey::Code(KeyCode::NumpadSubtract) => {
                let new_zoom = self.zoom / 1.5;
                let _ = self.config_manager.update_param(
                    crate::config::ConfigPath::Zoom,
                    new_zoom.into(),
                    false  // Immediate capture for keyboard input
                );
                self.zoom = self.config_manager.active_config().zoom;
                self.view_changed_by_keyboard = true;
            }
            _ => {}
        }
    }

    pub(super) fn handle_mouse_button(&mut self, state: ElementState, button: MouseButton, consumed: bool) {
        if button == MouseButton::Left {
            match state {
                ElementState::Pressed => {
                    // Only start dragging if egui didn't consume the event
                    if !consumed {
                        self.mouse_dragging = true;
                    }
                }
                ElementState::Released => {
                    // If we were dragging, commit the preview to finalize the pan operation
                    let was_dragging = self.mouse_dragging;

                    // Always clear dragging state on release, even if egui consumed it
                    self.mouse_dragging = false;
                    self.last_mouse_pos = None;

                    // Commit mouse pan preview if we were dragging
                    // Note: UI controls (sliders, triangle editor) handle their own force_commit
                    // so this only affects mouse viewport panning
                    if was_dragging {
                        // Force commit any pending pan preview from mouse drag
                        // Use PanX as representative path (batch updates share same preview state)
                        let _ = self.config_manager.force_commit_preview(&crate::config::ConfigPath::PanX);
                    }
                }
            }
        }
    }

    pub(super) fn handle_mouse_move(&mut self, x: f32, y: f32) {
        if self.mouse_dragging {
            if let Some((last_x, last_y)) = self.last_mouse_pos {
                // Calculate delta in screen pixels
                let dx = x - last_x;
                let dy = y - last_y;

                // Convert to fractal space - scale inversely with zoom and window size
                let scale = f32::min(self.gpu.size.width as f32, self.gpu.size.height as f32) * 0.25;
                let pan_dx = -dx / (scale * self.zoom);
                let pan_dy = -dy / (scale * self.zoom); // Negative to match drag direction

                // Rotate pan delta to account for view rotation
                // When view is rotated, we want pan to move relative to the rotated view
                // Negate rotation angle to rotate in screen space instead of fractal space
                let cos_r = (-self.rotation).cos();
                let sin_r = (-self.rotation).sin();
                let rotated_dx = pan_dx * cos_r - pan_dy * sin_r;
                let rotated_dy = pan_dx * sin_r + pan_dy * cos_r;

                // Route pan changes through ConfigManager for undo/redo support and live preview
                // Use update_batch to capture both X and Y as a single atomic operation
                let new_pan_x = self.pan_x + rotated_dx;
                let new_pan_y = self.pan_y + rotated_dy;
                let _ = self.config_manager.update_batch(
                    vec![
                        (crate::config::ConfigPath::PanX, new_pan_x.into()),
                        (crate::config::ConfigPath::PanY, new_pan_y.into()),
                    ],
                    "Pan (Mouse)".to_string(),
                    true  // Lazy mode for drag
                );

                // Read back from ConfigManager (gets preview value during drag)
                self.pan_x = self.config_manager.active_config().pan_x;
                self.pan_y = self.config_manager.active_config().pan_y;
                // Note: Don't set view_changed_by_keyboard here - preview mode handles updates via overwrite mode
            }
        }
        // Always update mouse position for zoom-to-cursor
        self.last_mouse_pos = Some((x, y));
    }

    pub(super) fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        let zoom_factor = match delta {
            MouseScrollDelta::LineDelta(_x, y) => {
                // Each line is typically one "click" of the wheel
                // Positive y = scroll up = zoom in, negative y = scroll down = zoom out
                if y > 0.0 {
                    1.1f32.powf(y)
                } else if y < 0.0 {
                    1.1f32.powf(y)
                } else {
                    1.0
                }
            }
            MouseScrollDelta::PixelDelta(pos) => {
                // Pixel delta for touchpad scrolling
                // Positive y = scroll up = zoom in
                let y = pos.y as f32;
                if y.abs() > 0.1 {
                    1.1f32.powf(y * 0.01)
                } else {
                    1.0
                }
            }
        };

        if zoom_factor != 1.0 {
            // Zoom in toward cursor, zoom out from center
            if zoom_factor > 1.0 {
                // Zooming in - zoom toward mouse cursor position
                if let Some((mouse_x, mouse_y)) = self.last_mouse_pos {
                    // Convert mouse position from screen space to fractal space
                    // Screen center
                    let center_x = self.gpu.size.width as f32 / 2.0;
                    let center_y = self.gpu.size.height as f32 / 2.0;

                    // Mouse offset from center in screen pixels
                    let mouse_offset_x = mouse_x - center_x;
                    let mouse_offset_y = mouse_y - center_y;

                    // Convert to fractal space (account for current zoom and scale)
                    let scale = f32::min(self.gpu.size.width as f32, self.gpu.size.height as f32) * 0.25;
                    let fractal_offset_x = mouse_offset_x / (scale * self.zoom);
                    let fractal_offset_y = mouse_offset_y / (scale * self.zoom);

                    // Calculate the point in fractal space that the mouse is pointing at
                    let point_x = self.pan_x + fractal_offset_x;
                    let point_y = self.pan_y + fractal_offset_y;

                    // Apply zoom and adjust pan via ConfigManager
                    let new_zoom = self.zoom * zoom_factor;
                    let new_fractal_offset_x = mouse_offset_x / (scale * new_zoom);
                    let new_fractal_offset_y = mouse_offset_y / (scale * new_zoom);
                    let new_pan_x = point_x - new_fractal_offset_x;
                    let new_pan_y = point_y - new_fractal_offset_y;

                    // Update all three parameters atomically
                    let _ = self.config_manager.update_batch(
                        vec![
                            (crate::config::ConfigPath::Zoom, new_zoom.into()),
                            (crate::config::ConfigPath::PanX, new_pan_x.into()),
                            (crate::config::ConfigPath::PanY, new_pan_y.into()),
                        ],
                        "Zoom In (Wheel)".to_string(),
                        false  // Immediate capture for mouse wheel
                    );
                    self.zoom = self.config_manager.active_config().zoom;
                    self.pan_x = self.config_manager.active_config().pan_x;
                    self.pan_y = self.config_manager.active_config().pan_y;
                } else {
                    // No mouse position, zoom to center
                    let new_zoom = self.zoom * zoom_factor;
                    let _ = self.config_manager.update_param(
                        crate::config::ConfigPath::Zoom,
                        new_zoom.into(),
                        false  // Immediate capture for mouse wheel
                    );
                    self.zoom = self.config_manager.active_config().zoom;
                }
            } else {
                // Zooming out - always zoom from center
                let new_zoom = self.zoom * zoom_factor;
                let _ = self.config_manager.update_param(
                    crate::config::ConfigPath::Zoom,
                    new_zoom.into(),
                    false  // Immediate capture for mouse wheel
                );
                self.zoom = self.config_manager.active_config().zoom;
            }

            self.view_changed_by_keyboard = true; // Reuse same flag
        }
    }
}

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

        // Read current view state from config
        let config = self.config_manager.active_config();
        let pan_step = 0.1 / config.zoom;

        // Pre-calculate rotation for arrow controls
        // Negate rotation to convert screen space to fractal space
        let cos_r = (-config.rotation).cos();
        let sin_r = (-config.rotation).sin();

        match event.physical_key {
            PhysicalKey::Code(KeyCode::ArrowUp) => {
                // Up in screen space: (0, -1), rotate to fractal space
                let screen_dx = 0.0;
                let screen_dy = -pan_step;
                let new_pan_x = config.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                let new_pan_y = config.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                let _ = self.config_manager.update_param(
                    crate::config::ConfigPath::Pan,
                    (new_pan_x, new_pan_y).into(),
                );
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::ArrowDown) => {
                // Down in screen space: (0, 1), rotate to fractal space
                let screen_dx = 0.0;
                let screen_dy = pan_step;
                let new_pan_x = config.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                let new_pan_y = config.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                let _ = self.config_manager.update_param(
                    crate::config::ConfigPath::Pan,
                    (new_pan_x, new_pan_y).into(),
                );
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::ArrowLeft) => {
                // Left in screen space: (-1, 0), rotate to fractal space
                let screen_dx = -pan_step;
                let screen_dy = 0.0;
                let new_pan_x = config.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                let new_pan_y = config.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                let _ = self.config_manager.update_param(
                    crate::config::ConfigPath::Pan,
                    (new_pan_x, new_pan_y).into(),
                );
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::ArrowRight) => {
                // Right in screen space: (1, 0), rotate to fractal space
                let screen_dx = pan_step;
                let screen_dy = 0.0;
                let new_pan_x = config.pan_x + (screen_dx * cos_r - screen_dy * sin_r);
                let new_pan_y = config.pan_y + (screen_dx * sin_r + screen_dy * cos_r);
                let _ = self.config_manager.update_param(
                    crate::config::ConfigPath::Pan,
                    (new_pan_x, new_pan_y).into(),
                );
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::Equal) | PhysicalKey::Code(KeyCode::NumpadAdd) => {
                let new_zoom = config.zoom * 1.5;
                let _ = self.config_manager.update_param(
                    crate::config::ConfigPath::Zoom,
                    new_zoom.into(),
                );
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::Minus) | PhysicalKey::Code(KeyCode::NumpadSubtract) => {
                let new_zoom = config.zoom / 1.5;
                let _ = self.config_manager.update_param(
                    crate::config::ConfigPath::Zoom,
                    new_zoom.into(),
                );
                self.view_changed_by_keyboard = true;
            }
            _ => {}
        }
    }

}

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
            PhysicalKey::Code(KeyCode::KeyF) => {
                self.toggle_fullscreen();
            }
            PhysicalKey::Code(KeyCode::Escape) => {
                if self.fullscreen_mode {
                    self.exit_fullscreen();
                }
            }
            _ => {}
        }
    }

    /// Toggle fullscreen mode (F key)
    pub fn toggle_fullscreen(&mut self) {
        if self.fullscreen_mode {
            self.exit_fullscreen();
        } else {
            self.enter_fullscreen();
        }
    }

    /// Enter fullscreen mode
    pub fn enter_fullscreen(&mut self) {
        self.fullscreen_mode = true;

        // Desktop: Use winit's fullscreen API
        #[cfg(not(target_arch = "wasm32"))]
        {
            use winit::window::Fullscreen;
            self.window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }

        // WASM: Use browser's Fullscreen API on the canvas
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            if let Some(canvas) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id("canvas"))
            {
                let _ = canvas.request_fullscreen();
            }
        }
    }

    /// Exit fullscreen mode
    pub fn exit_fullscreen(&mut self) {
        self.fullscreen_mode = false;

        // Desktop: Exit fullscreen via winit
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.window.set_fullscreen(None);
        }

        // WASM: Exit fullscreen via document API
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                document.exit_fullscreen();
            }
        }
    }

    /// Sync fullscreen state with browser's actual state (WASM only)
    /// Called each frame to detect when browser exits fullscreen via Esc
    pub fn sync_fullscreen_state(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            let browser_is_fullscreen = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.fullscreen_element())
                .is_some();

            // If browser exited fullscreen but we still think we're in it, sync state
            if self.fullscreen_mode && !browser_is_fullscreen {
                self.fullscreen_mode = false;
            }
        }
    }
}

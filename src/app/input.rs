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
                self.cycle_fullscreen();
            }
            PhysicalKey::Code(KeyCode::Escape) => {
                if self.window_fullscreen || self.ui_hidden {
                    self.exit_fullscreen();
                }
            }
            _ => {}
        }
    }

    /// Cycle fullscreen mode (F key)
    /// Normal → Fullscreen+UI → Fullscreen-UI → Normal
    pub fn cycle_fullscreen(&mut self) {
        // For WASM, check actual browser state (our tracked state can be stale due to async)
        #[cfg(target_arch = "wasm32")]
        let is_fullscreen = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.fullscreen_element())
            .is_some();

        #[cfg(not(target_arch = "wasm32"))]
        let is_fullscreen = self.window_fullscreen;

        if !is_fullscreen {
            // Stage 1: Enter fullscreen, keep UI visible
            self.enter_window_fullscreen();
        } else if !self.ui_hidden {
            // Stage 2: Hide UI (already in fullscreen)
            self.ui_hidden = true;
        } else {
            // Stage 3: Exit fullscreen entirely
            self.exit_fullscreen();
        }
    }

    /// Enter window fullscreen mode (stage 1)
    fn enter_window_fullscreen(&mut self) {
        self.window_fullscreen = true;

        // Desktop: Use winit's fullscreen API
        #[cfg(not(target_arch = "wasm32"))]
        {
            use winit::window::Fullscreen;
            self.window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }

        // WASM: Use browser's Fullscreen API on the canvas
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(canvas) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id("canvas"))
            {
                let _ = canvas.request_fullscreen();
            }
        }
    }

    /// Exit fullscreen mode completely
    pub fn exit_fullscreen(&mut self) {
        self.window_fullscreen = false;
        self.ui_hidden = false;

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
            let browser_is_fullscreen = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.fullscreen_element())
                .is_some();

            // If browser exited fullscreen but we still think we're in it, sync state
            if self.window_fullscreen && !browser_is_fullscreen {
                self.window_fullscreen = false;
                self.ui_hidden = false;
            }
        }
    }
}

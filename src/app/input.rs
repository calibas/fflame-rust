use winit::event::*;
use crate::app::App;

impl App {
    pub(super) fn handle_keyboard(&mut self, event: &KeyEvent) {
        use winit::keyboard::{KeyCode, PhysicalKey};

        // Fly-mode key tracking — when fly_mode is on, WASD / QE /
        // Shift drive `update_fly_camera` per-frame instead of
        // falling through to the existing press-only shortcuts. We
        // track press AND release here (the other branches below
        // only handle press) so the held-set stays accurate even
        // if a key is held across multiple frames.
        if self.fly_mode {
            if let PhysicalKey::Code(code) = event.physical_key {
                if matches!(
                    code,
                    KeyCode::KeyW | KeyCode::KeyA | KeyCode::KeyS | KeyCode::KeyD
                    | KeyCode::KeyQ | KeyCode::KeyE
                    | KeyCode::ShiftLeft | KeyCode::ShiftRight
                ) {
                    if event.state.is_pressed() {
                        self.fly_keys_held.insert(code);
                    } else {
                        self.fly_keys_held.remove(&code);
                    }
                    return;  // Consume — don't let WASD bleed into other shortcuts
                }
            }
        }

        // F2 toggles fly mode regardless of current state (press only,
        // not release — so the toggle doesn't double-fire).
        if event.state.is_pressed() {
            if let PhysicalKey::Code(KeyCode::F2) = event.physical_key {
                self.toggle_fly_mode();
                return;
            }
        }

        // Only handle key press (not release)
        if !event.state.is_pressed() {
            return;
        }

        // Check for Ctrl/Cmd modifier.
        //
        // On the web, ACCEPT EITHER — and that is not laziness. The cfg
        // below is resolved at compile time, but a wasm build has no
        // compile-time platform: `target_os` is "unknown" whatever the
        // user is actually running. winit's web backend maps Cmd to
        // SUPER and Ctrl to CONTROL faithfully (see
        // platform_impl/web/web_sys/event.rs), so a macOS visitor
        // pressing Cmd+Z was sending SUPER to a build that only looked
        // at CONTROL: undo silently did nothing for every Mac user of
        // the web app.
        //
        // Accepting both avoids sniffing the user agent to decide, and
        // costs nothing: on Windows and Linux the Super key is the OS's,
        // and Super+Z does not reach the page.
        let ctrl_or_cmd = {
            #[cfg(target_arch = "wasm32")]
            { self.modifiers.control_key() || self.modifiers.super_key() }
            #[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
            { self.modifiers.super_key() }
            #[cfg(all(not(target_arch = "wasm32"), not(target_os = "macos")))]
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

        // Escape mode speaks its own view vocabulary (center strings +
        // zoom_log2): route arrows through the shared pan entry (which
        // branches to escape panning internally, using drag semantics --
        // one press scrolls 5% of the viewport) and +/- through the
        // escape zoom helper. Escape re-renders come from the
        // EscapeRerender update path, so `view_changed_by_keyboard`
        // (a flame accumulation-reset signal) stays untouched.
        if config.render_mode == crate::scene::transforms::RenderMode::Escape {
            let (vw, vh) = self.fractal_viewport_size;
            let panel_size = egui::Vec2::new(vw.max(1) as f32, vh.max(1) as f32);
            let step = 0.05 * panel_size.x.min(panel_size.y);
            // Drag semantics: content follows the drag, so the view
            // moves OPPOSITE the delta -- matching the flame arrows'
            // direction convention (Up scrolls the view up).
            let drag = match event.physical_key {
                PhysicalKey::Code(KeyCode::ArrowUp) => Some(egui::Vec2::new(0.0, step)),
                PhysicalKey::Code(KeyCode::ArrowDown) => Some(egui::Vec2::new(0.0, -step)),
                PhysicalKey::Code(KeyCode::ArrowLeft) => Some(egui::Vec2::new(step, 0.0)),
                PhysicalKey::Code(KeyCode::ArrowRight) => Some(egui::Vec2::new(-step, 0.0)),
                _ => None,
            };
            if let Some(drag) = drag {
                crate::ui::pan_fractal_view(&mut self.config_manager, drag, panel_size);
                return;
            }
            match event.physical_key {
                PhysicalKey::Code(KeyCode::Equal) | PhysicalKey::Code(KeyCode::NumpadAdd) => {
                    crate::ui::escape_zoom_by_factor(&mut self.config_manager, 1.5);
                    return;
                }
                PhysicalKey::Code(KeyCode::Minus) | PhysicalKey::Code(KeyCode::NumpadSubtract) => {
                    crate::ui::escape_zoom_by_factor(&mut self.config_manager, 1.0 / 1.5);
                    return;
                }
                _ => {}
            }
            // Space/F/Escape etc. fall through to the shared handling.
        }

        match event.physical_key {
            PhysicalKey::Code(KeyCode::ArrowUp) => {
                // Up in screen space: (0, -1), convert to pan frame
                let (dx, dy) = config.screen_delta_to_pan_frame(0.0, -pan_step);
                let new_pan_x = config.pan_x + dx;
                let new_pan_y = config.pan_y + dy;
                let _ = self.config_manager.update_param(
                    crate::config::ConfigPath::Pan,
                    (new_pan_x, new_pan_y).into(),
                );
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::ArrowDown) => {
                // Down in screen space: (0, 1), convert to pan frame
                let (dx, dy) = config.screen_delta_to_pan_frame(0.0, pan_step);
                let new_pan_x = config.pan_x + dx;
                let new_pan_y = config.pan_y + dy;
                let _ = self.config_manager.update_param(
                    crate::config::ConfigPath::Pan,
                    (new_pan_x, new_pan_y).into(),
                );
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::ArrowLeft) => {
                // Left in screen space: (-1, 0), convert to pan frame
                let (dx, dy) = config.screen_delta_to_pan_frame(-pan_step, 0.0);
                let new_pan_x = config.pan_x + dx;
                let new_pan_y = config.pan_y + dy;
                let _ = self.config_manager.update_param(
                    crate::config::ConfigPath::Pan,
                    (new_pan_x, new_pan_y).into(),
                );
                self.view_changed_by_keyboard = true;
            }
            PhysicalKey::Code(KeyCode::ArrowRight) => {
                // Right in screen space: (1, 0), convert to pan frame
                let (dx, dy) = config.screen_delta_to_pan_frame(pan_step, 0.0);
                let new_pan_x = config.pan_x + dx;
                let new_pan_y = config.pan_y + dy;
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
            PhysicalKey::Code(KeyCode::Space) => {
                // Toggle animation playback (video-editor convention).
                // Only reachable when egui didn't consume the key, so a
                // space typed into a text field never lands here.
                //
                // Gated on the animation having tracks: rendering the
                // Animation panel once auto-creates an empty "New
                // Animation", and playing THAT flips the renderer into
                // animation mode (single-batch overwrite, grainy) with
                // nothing moving — a mystery degradation from a global
                // shortcut. The panel's own Play button stays permissive;
                // there the user can see what they pressed play on.
                let has_tracks = self
                    .animation_controller
                    .animation
                    .as_ref()
                    .is_some_and(|a| !a.tracks.is_empty());
                if self.animation_controller.is_playing() {
                    self.animation_controller.pause();
                } else if has_tracks {
                    self.animation_controller.play();
                }
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

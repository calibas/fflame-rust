//! Fly mode's input: the held keys and the look drags, handed to
//! `camera::fly`, which turns and moves every 3D camera alike
//! (camera-unification C4) -- a 3D flame, mode D's solid, and both
//! terrains.
//!
//! Active only when `App::fly_mode` is true (the View panel's button, the
//! menu bar's, or F2), and only where `visibility::fly_mode` offers it.
//!
//!   * `apply_fly_mouse_look(drag)`: a look about the eye. FreeLook
//!     turns about the screen's axes (and can roll, as a space sim);
//!     FPS turns about world up and the level axis, so the horizon keeps
//!     its tilt. Composed on the camera's quaternion and written back as
//!     its pitch, yaw and bank, the roll held.
//!   * `update_fly_camera()`: per frame, the held W A S D Q E (and
//!     Shift to sprint) push the camera along its axes.
//!
//! Losing the window's focus lets go of every held key: the key-up goes
//! to another window, and the camera used to fly on.

use crate::app::App;
use crate::storage::FlyCameraMode;
use winit::keyboard::KeyCode;

impl App {
    /// Toggle fly mode on/off. Resets the held-keys set and the
    /// delta-time anchor so re-entering fly mode after a pause
    /// doesn't apply stale state.
    ///
    /// Offered for every 3D camera (`visibility::fly_mode`): a request to
    /// ENABLE it anywhere else is ignored (the menu and View-panel buttons
    /// are not there, and F2 is a no-op). Disabling always works.
    pub fn toggle_fly_mode(&mut self) {
        if !self.fly_mode && !self.fly_mode_available() {
            return;
        }
        self.fly_mode = !self.fly_mode;
        self.release_fly_keys();
    }

    /// Whether the camera the viewport shows flies.
    pub(crate) fn fly_mode_available(&self) -> bool {
        let kind = crate::camera::gesture::view_kind(self.config_manager.active_config());
        crate::ui::visibility::fly_mode(kind).is_show()
    }

    /// Let go of every held fly key: on a toggle, and when the window
    /// loses focus -- its key-up never arrives, and the camera flew on.
    pub(crate) fn release_fly_keys(&mut self) {
        self.fly_keys_held.clear();
        self.fly_last_update = None;
    }

    /// The viewport's size for the fly mode: a simulation terrain's grid
    /// can be the window's.
    fn fly_panel(&self) -> [f32; 2] {
        let (w, h) = self.fractal_viewport_size;
        [w.max(1) as f32, h.max(1) as f32]
    }

    /// Turn the camera by a fly-mode drag, about its eye
    /// (`camera::fly::look`).
    pub fn apply_fly_mouse_look(&mut self, drag_dx: f32, drag_dy: f32) {
        if drag_dx == 0.0 && drag_dy == 0.0 {
            return;
        }
        let settings = crate::camera::fly::LookSettings::from_system(self.config_manager.system_settings());
        let panel = self.fly_panel();
        if let Some(edit) = crate::camera::fly::look(self.config_manager.active_config(), [drag_dx, drag_dy], settings, panel) {
            edit.apply(&mut self.config_manager);
        }
    }

    /// Per-frame flight: the held keys push the camera along its axes
    /// (`camera::fly::fly`) -- W/S forward and back, A/D across, Q/E
    /// down and up (the screen's in FreeLook, the world's in FPS) -- at
    /// the fly speed, times the sprint multiplier while Shift is held.
    ///
    /// No-op when fly mode is off or no movement key is held; the
    /// delta-time window restarts whenever nothing is held, so the next
    /// press does not integrate the gap.
    pub fn update_fly_camera(&mut self) {
        // Drop fly mode when the camera shown stops flying, by ANY path
        // (a mode switch, a config load, undo, ...). Runs every frame.
        if self.fly_mode && !self.fly_mode_available() {
            self.fly_mode = false;
            self.release_fly_keys();
        }
        if !self.fly_mode {
            return;
        }
        if self.fly_keys_held.is_empty() {
            self.fly_last_update = None;
            return;
        }

        let now = web_time::Instant::now();
        let dt = match self.fly_last_update {
            Some(prev) => now.duration_since(prev).as_secs_f32().min(0.1),
            None => 0.0, // First frame after key-down — skip integration
        };
        self.fly_last_update = Some(now);
        if dt <= 0.0 {
            return;
        }

        let settings = self.config_manager.system_settings();
        let mut speed = settings.fly_move_speed;
        let held = |k: KeyCode| self.fly_keys_held.contains(&k);
        if held(KeyCode::ShiftLeft) || held(KeyCode::ShiftRight) {
            speed *= settings.fly_sprint_multiplier;
        }
        let axis = |plus: KeyCode, minus: KeyCode| (held(plus) as i32 - held(minus) as i32) as f64;
        let thrust = crate::camera::fly::Thrust {
            right: axis(KeyCode::KeyD, KeyCode::KeyA),
            up: axis(KeyCode::KeyE, KeyCode::KeyQ),
            forward: axis(KeyCode::KeyW, KeyCode::KeyS),
            world_up: settings.fly_camera_mode == FlyCameraMode::Fps,
        };
        let panel = self.fly_panel();
        if let Some(edit) = crate::camera::fly::fly(self.config_manager.active_config(), thrust, (speed * dt) as f64, panel) {
            edit.apply(&mut self.config_manager);
        }
    }
}

//! Free-fly camera input integration.
//!
//! Active only when `App::fly_mode` is true (toggled by the View
//! panel button or the F2 hotkey). Two responsibilities:
//!
//!   * `apply_mouse_look(drag_delta)`: convert pixel-space mouse
//!     drag into pitch/yaw deltas and push them through the config
//!     manager. Called from `panel_viewer`'s drag handler when
//!     fly_mode is active.
//!
//!   * `update_fly_camera()`: per-frame integration. Reads the set
//!     of currently-held movement keys (WASD / QE / Shift), computes
//!     a camera-local basis from the current pitch+yaw, and pushes
//!     a position delta into `camera_x` / `camera_y` / `camera_z`.
//!     Called from the main render loop.
//!
//! The camera-forward and camera-right vectors are computed from
//! pitch + yaw only — bank and roll are intentionally ignored when
//! deciding "which way is forward." They still twist the view via
//! the camera matrix, but they don't change which direction WASD
//! sends the camera. This matches FPS-game convention and is what
//! users expect when banking the camera mid-flight.

use crate::app::App;
use crate::config::ConfigPath;
use winit::keyboard::KeyCode;

impl App {
    /// Toggle fly mode on/off. Resets the held-keys set and the
    /// delta-time anchor so re-entering fly mode after a pause
    /// doesn't apply stale state.
    pub fn toggle_fly_mode(&mut self) {
        self.fly_mode = !self.fly_mode;
        self.fly_keys_held.clear();
        self.fly_last_update = None;
    }

    /// Apply a mouse-drag delta as a camera rotation. Horizontal
    /// drag rotates yaw, vertical drag rotates pitch. Sensitivity
    /// (radians per pixel) and Y-axis inversion come from
    /// SystemSettings. No clamping on pitch — over 90° gimbal-lock
    /// is accepted, see the project doc for rationale.
    pub fn apply_fly_mouse_look(&mut self, drag_dx: f32, drag_dy: f32) {
        if !self.fly_mode {
            return;
        }
        let settings = self.config_manager.system_settings();
        let sensitivity = settings.fly_mouse_sensitivity;
        let invert_y = settings.fly_invert_y;

        let cfg = self.config_manager.active_config();
        let new_yaw = cfg.camera_rotation_y + drag_dx * sensitivity;
        let dy_sign = if invert_y { 1.0 } else { -1.0 };
        let new_pitch = cfg.camera_rotation_x + drag_dy * sensitivity * dy_sign;

        let _ = self.config_manager.update_param(
            ConfigPath::CameraRotationX,
            new_pitch.into(),
        );
        let _ = self.config_manager.update_param(
            ConfigPath::CameraRotationY,
            new_yaw.into(),
        );
    }

    /// Per-frame fly-camera integration. Reads `fly_keys_held` and
    /// translates `camera_x/y/z` along the camera-local basis (for
    /// WASD) or world-up (for QE). Speed comes from SystemSettings,
    /// scaled by the sprint multiplier while Shift is held.
    ///
    /// No-op when fly_mode is off OR no movement keys are currently
    /// held. Resets `fly_last_update` whenever the held set is empty
    /// so the next press starts a fresh delta-time window instead
    /// of integrating a huge gap.
    pub fn update_fly_camera(&mut self) {
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
            None => 0.0,  // First frame after key-down — skip integration
        };
        self.fly_last_update = Some(now);
        if dt <= 0.0 {
            return;
        }

        let settings = self.config_manager.system_settings();
        let mut speed = settings.fly_move_speed;
        if self.fly_keys_held.contains(&KeyCode::ShiftLeft)
            || self.fly_keys_held.contains(&KeyCode::ShiftRight)
        {
            speed *= settings.fly_sprint_multiplier;
        }

        // Camera-local basis from pitch + yaw. Uses a simple
        // spherical-coords convention rather than mirroring the
        // shader's empirical sign tuning — the signs are tuned
        // here to match observed UI behavior (e.g., W moves the
        // view "into" the scene). If a direction feels backwards
        // in practice we flip its sign in just this block; the
        // shader math stays untouched.
        let cfg = self.config_manager.active_config();
        let pitch = cfg.camera_rotation_x;
        let yaw = cfg.camera_rotation_y;
        let cp = pitch.cos();
        let sp = pitch.sin();
        let cy = yaw.cos();
        let sy = yaw.sin();

        // forward: where the camera is "looking" in world space
        // (negate Z so positive pitch tilts the view up the +Z axis).
        let forward = [-sy * cp, cy * cp, -sp];
        // right: perpendicular to forward, horizontal
        let right = [cy, sy, 0.0];
        // world-up for Q/E
        let world_up = [0.0_f32, 0.0, 1.0];

        let mut delta = [0.0_f32; 3];
        let step = speed * dt;
        if self.fly_keys_held.contains(&KeyCode::KeyW) {
            for i in 0..3 { delta[i] += forward[i] * step; }
        }
        if self.fly_keys_held.contains(&KeyCode::KeyS) {
            for i in 0..3 { delta[i] -= forward[i] * step; }
        }
        if self.fly_keys_held.contains(&KeyCode::KeyD) {
            for i in 0..3 { delta[i] += right[i] * step; }
        }
        if self.fly_keys_held.contains(&KeyCode::KeyA) {
            for i in 0..3 { delta[i] -= right[i] * step; }
        }
        if self.fly_keys_held.contains(&KeyCode::KeyE) {
            for i in 0..3 { delta[i] += world_up[i] * step; }
        }
        if self.fly_keys_held.contains(&KeyCode::KeyQ) {
            for i in 0..3 { delta[i] -= world_up[i] * step; }
        }

        if delta == [0.0; 3] {
            return;
        }

        let new_x = cfg.camera_x + delta[0];
        let new_y = cfg.camera_y + delta[1];
        let new_z = cfg.camera_z + delta[2];
        let _ = self.config_manager.update_param(ConfigPath::CameraX, new_x.into());
        let _ = self.config_manager.update_param(ConfigPath::CameraY, new_y.into());
        let _ = self.config_manager.update_param(ConfigPath::CameraZ, new_z.into());
    }
}

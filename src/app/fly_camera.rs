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
//!     a camera-local basis from the current pitch+yaw+rotation, and
//!     pushes a position delta into `camera_x` / `camera_y` /
//!     `camera_z`. Called from the main render loop.
//!
//! Compensation for `pan_x` / `pan_y` / `rotation` is applied
//! dynamically — the underlying config values are never modified by
//! fly mode. The user's saved pan and rotation are preserved exactly
//! across enter/exit:
//!
//!   * `rotation` is baked into the camera basis so WASD's `right`
//!     vector turns with the on-screen orientation. With rotation =
//!     90° the right axis is no longer world +X; it's whatever
//!     "screen-right" maps to in world coords.
//!
//!   * `pan_x` / `pan_y` shift the visual center of the screen away
//!     from where `camera_pos` projects. When the user mouse-looks,
//!     the rotation pivot would otherwise be off-center by exactly
//!     the pan magnitude. We compensate by translating `camera_pos`
//!     each mouse-look event so the world point that was at screen
//!     center stays at screen center after the rotation.

use crate::app::App;
use crate::config::ConfigPath;
use winit::keyboard::KeyCode;

/// Camera orientation matrix. Mirrors the matrix the GPU builds in
/// [`shaders/core/utilities.wgsl::build_camera_matrix`] with our
/// slider→slot mapping:
///
///   - `rotation` → JWF yaw slot, negated
///   - `pitch`    → JWF pitch slot, negated
///   - `bank`     → JWF bank slot, negated (we assume 0 here)
///   - `yaw`      → JWF roll slot
///
/// Stored row-major as `m[row][col]`. The matrix maps a world point
/// to camera space via `p_cam = M · (p_world − camera_pos)`. The
/// camera's look direction is camera-space `−Z`, so its rows have
/// useful geometric meaning:
///
///   - row 0 = camera-local `+X` in world coords ("right")
///   - row 1 = camera-local `+Y` in world coords ("up" within view)
///   - row 2 = camera-local `+Z` in world coords (look direction is
///             the negation of this row)
struct CameraMatrix {
    m: [[f32; 3]; 3],
}

impl CameraMatrix {
    fn build(pitch: f32, yaw: f32, rotation: f32) -> Self {
        // Pre-compute trig once. Names match the user-facing params.
        let sp = pitch.sin();
        let cp = pitch.cos();
        let sy = yaw.sin();
        let cy = yaw.cos();
        let sr = rotation.sin();
        let cr = rotation.cos();

        // Worked out by substituting our slot-mapping into JWF's
        // `createProjectionMatrix(yaw, pitch, bank, roll)`. With
        // bank = 0 the bank terms drop out, simplifying to:
        Self {
            m: [
                // row 0 — camera-local +X in world
                [
                     cp * sy * sr + cy * cr,
                    -cp * cy * sr + sy * cr,
                     sp * sr,
                ],
                // row 1 — camera-local +Y in world
                [
                    -cp * cr * sy + cy * sr,
                     cp * cy * cr + sy * sr,
                    -sp * cr,
                ],
                // row 2 — camera-local +Z in world (look is −row2)
                [
                    -sp * sy,
                     sp * cy,
                     cp,
                ],
            ],
        }
    }

    /// World-space direction the camera treats as "right" — i.e.
    /// camera-local `+X`. Independent of bank (we assume 0). At
    /// `rotation = 0` this collapses to `(cos Y, sin Y, 0)`.
    fn right(&self) -> [f32; 3] {
        self.m[0]
    }

    /// World-space direction the camera looks toward — i.e.
    /// camera-local `−Z`, which is `−row2(M)`. Does not depend on
    /// rotation or bank (twisting around the look axis can't change
    /// which way it points), so this is just pitch + yaw.
    fn forward(&self) -> [f32; 3] {
        [-self.m[2][0], -self.m[2][1], -self.m[2][2]]
    }

    /// Returns `M^T · (a, b, 0)`. Used by the pan compensator: a
    /// camera-space offset of `(pan_x, pan_y, 0)` (which is what the
    /// pan slider applies after projection at the camera's own depth)
    /// corresponds to this world-space offset of `camera_pos`. Exact
    /// only at camera-space depth 0; under perspective other depths
    /// see a slight residual co-motion.
    fn world_offset_for_camera_xy(&self, a: f32, b: f32) -> [f32; 3] {
        // (M^T)_ij = M_ji, so (M^T · v).i = Σ_j M_ji · v_j.
        [
            self.m[0][0] * a + self.m[1][0] * b,
            self.m[0][1] * a + self.m[1][1] * b,
            self.m[0][2] * a + self.m[1][2] * b,
        ]
    }
}

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
    /// SystemSettings.
    ///
    /// When `pan_x` / `pan_y` are non-zero the rotation pivot would
    /// otherwise be off-center: the camera rotates around
    /// `camera_pos`, but `camera_pos` no longer projects to screen
    /// center because pan has shifted the visible frame. We
    /// compensate by translating `camera_pos` so the world point
    /// currently at screen center stays at screen center after the
    /// rotation. Equivalent to:
    ///
    ///   `camera_pos += (M_old^T − M_new^T) · (pan_x, pan_y, 0)`
    ///
    /// This is geometrically exact at the focal-plane depth (where
    /// camera-space `z = 0`, so the perspective denominator is 1).
    /// At other depths the perspective division leaves a residual
    /// dependence on `persp_strength · z`, but for typical fly-mode
    /// pan ≤ a couple of units it's not noticeable.
    pub fn apply_fly_mouse_look(&mut self, drag_dx: f32, drag_dy: f32) {
        if !self.fly_mode {
            return;
        }
        let settings = self.config_manager.system_settings();
        let sensitivity = settings.fly_mouse_sensitivity;
        let invert_y = settings.fly_invert_y;

        // Snapshot every scalar we need up-front and drop the
        // immutable borrow so the `update_param` calls below can take
        // the mutable borrow.
        let (pitch_old, yaw_old, rotation, pan_x, pan_y, cam_x, cam_y, cam_z) = {
            let cfg = self.config_manager.active_config();
            (
                cfg.camera_rotation_x,
                cfg.camera_rotation_y,
                cfg.rotation,
                cfg.pan_x,
                cfg.pan_y,
                cfg.camera_x,
                cfg.camera_y,
                cfg.camera_z,
            )
        };

        // Convert the screen-pixel drag delta into the camera's
        // intrinsic frame before applying it to yaw / pitch. With
        // `rotation = 0` the two frames coincide and this is a no-op.
        // With `rotation ≠ 0` the screen is rotated relative to the
        // camera's intrinsic XY axes (because `params.rotation` enters
        // the camera matrix as the JWF yaw slot, spinning the world on
        // screen). A drag of `(dx, dy)` in screen-aligned pixels then
        // maps onto camera-frame axes as `(dx·cosR + dy·sinR,
        // −dx·sinR + dy·cosR)`. Tested at `R = π/2`: a horizontal
        // screen drag turns into a pure pitch change, which tilts the
        // camera toward whatever is at screen-right under the rotated
        // view (world `−Y` after the 90° CCW spin).
        let cr = rotation.cos();
        let sr = rotation.sin();
        let drag_dx_local = drag_dx * cr + drag_dy * sr;
        let drag_dy_local = -drag_dx * sr + drag_dy * cr;

        let yaw_new = yaw_old + drag_dx_local * sensitivity;
        let dy_sign = if invert_y { 1.0 } else { -1.0 };
        let pitch_new = pitch_old + drag_dy_local * sensitivity * dy_sign;

        let _ = self
            .config_manager
            .update_param(ConfigPath::CameraRotationX, pitch_new.into());
        let _ = self
            .config_manager
            .update_param(ConfigPath::CameraRotationY, yaw_new.into());

        // Pan-pivot compensation. Skip entirely when pan is zero
        // because then `camera_pos` already projects to screen center
        // and the rotation pivot is correct as-is.
        if pan_x != 0.0 || pan_y != 0.0 {
            let m_old = CameraMatrix::build(pitch_old, yaw_old, rotation);
            let m_new = CameraMatrix::build(pitch_new, yaw_new, rotation);
            let off_old = m_old.world_offset_for_camera_xy(pan_x, pan_y);
            let off_new = m_new.world_offset_for_camera_xy(pan_x, pan_y);
            let new_x = cam_x + off_old[0] - off_new[0];
            let new_y = cam_y + off_old[1] - off_new[1];
            let new_z = cam_z + off_old[2] - off_new[2];
            let _ = self
                .config_manager
                .update_param(ConfigPath::CameraX, new_x.into());
            let _ = self
                .config_manager
                .update_param(ConfigPath::CameraY, new_y.into());
            let _ = self
                .config_manager
                .update_param(ConfigPath::CameraZ, new_z.into());
        }
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
            None => 0.0, // First frame after key-down — skip integration
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

        // Camera-local basis derived from the same matrix the shader
        // applies, including the screen-rotation `params.rotation`.
        // This makes `right` follow the on-screen orientation — at
        // rotation = 90° the world's `+X` is no longer screen-right;
        // whatever world direction maps to screen-right is.
        //
        // Forward intentionally only uses pitch + yaw because the
        // matrix's bottom row doesn't depend on rotation (twisting
        // around the look axis doesn't change where the look points).
        let cfg = self.config_manager.active_config();
        let m = CameraMatrix::build(
            cfg.camera_rotation_x,
            cfg.camera_rotation_y,
            cfg.rotation,
        );
        let forward = m.forward();
        let right = m.right();
        // World up for Q/E. Always world-Z regardless of camera
        // orientation (FPS convention — "up" doesn't follow pitch).
        let world_up = [0.0_f32, 0.0, 1.0];

        let mut delta = [0.0_f32; 3];
        let step = speed * dt;
        if self.fly_keys_held.contains(&KeyCode::KeyW) {
            for i in 0..3 {
                delta[i] += forward[i] * step;
            }
        }
        if self.fly_keys_held.contains(&KeyCode::KeyS) {
            for i in 0..3 {
                delta[i] -= forward[i] * step;
            }
        }
        if self.fly_keys_held.contains(&KeyCode::KeyD) {
            for i in 0..3 {
                delta[i] += right[i] * step;
            }
        }
        if self.fly_keys_held.contains(&KeyCode::KeyA) {
            for i in 0..3 {
                delta[i] -= right[i] * step;
            }
        }
        if self.fly_keys_held.contains(&KeyCode::KeyE) {
            for i in 0..3 {
                delta[i] += world_up[i] * step;
            }
        }
        if self.fly_keys_held.contains(&KeyCode::KeyQ) {
            for i in 0..3 {
                delta[i] -= world_up[i] * step;
            }
        }

        if delta == [0.0; 3] {
            return;
        }

        let new_x = cfg.camera_x + delta[0];
        let new_y = cfg.camera_y + delta[1];
        let new_z = cfg.camera_z + delta[2];
        let _ = self
            .config_manager
            .update_param(ConfigPath::CameraX, new_x.into());
        let _ = self
            .config_manager
            .update_param(ConfigPath::CameraY, new_y.into());
        let _ = self
            .config_manager
            .update_param(ConfigPath::CameraZ, new_z.into());
    }
}

//! Free-fly camera input integration.
//!
//! Active only when `App::fly_mode` is true (toggled by the View
//! panel button or the F2 hotkey). Two responsibilities:
//!
//!   * `apply_fly_mouse_look(drag_delta)`: free-look rotation. The
//!     drag rotates the camera about its own screen axes (rotation
//!     composed in SO(3), then decomposed back to our Euler triple
//!     for storage). Called from `panel_viewer`'s drag handler when
//!     fly_mode is active.
//!
//!   * `update_fly_camera()`: per-frame integration. Reads the set
//!     of currently-held movement keys (WASD / QE / Shift), computes
//!     a camera-local basis from the current pitch+yaw+rotation, and
//!     pushes a position delta into `camera_x` / `camera_y` /
//!     `camera_z`. Called from the main render loop.
//!
//! # Mouse-look models (SystemSettings::fly_camera_mode)
//!
//! **FreeLook** (default): drag right rotates about the
//! screen-vertical axis; drag down rotates about the
//! screen-horizontal axis — at *every* camera orientation,
//! including the default straight-down pose. This is the space-sim
//! convention: there is no world-anchored "up" in the control
//! scheme, so the controls never invert and never lock. The cost is
//! that circular mouse motions accumulate roll (the horizon can
//! tilt); the `rotation` value drifts accordingly and can be
//! re-leveled via its slider.
//!
//! In FreeLook the rotation is composed on the camera matrix itself
//! and only converted back to our `(pitch, yaw, rotation)` Euler
//! triple at the end of each event, so the *view* is smooth and
//! stable everywhere. The Euler values, however, live in a chart
//! with singularities at pitch = 0 and pitch = π (where yaw and
//! rotation alias); passing near those poses can reshuffle the
//! stored values discontinuously even though the view moves
//! continuously — the same way longitude jumps when you walk over
//! the north pole. `to_euler_near` guarantees the reshuffled values
//! still rebuild the exact same matrix.
//!
//! **Fps**: drag right yaws about the world-up axis (a plain Euler
//! yaw increment — algebraically identical, see the
//! `axis_angle_matches_euler_increments` test); drag down pitches
//! about the screen-plane axis (Euler pitch increment, with the
//! drag pre-rotated into the camera frame when `rotation ≠ 0`).
//! The horizon — the fractal's XY plane — always stays level, and
//! mouse-look never touches `rotation`. In exchange, looking past
//! straight-down/up reverses horizontal drag, and at the
//! straight-down home pose horizontal drag spins the view in place
//! (geometric necessity for any world-anchored scheme).
//!
//! # Position keys
//!
//! The camera-local basis for WASD/QE comes from the full camera
//! matrix (pitch + yaw + rotation): W/S along the look axis, A/D
//! along screen-right, Q/E along screen-down/up. All six directions
//! are camera-relative (space-sim style — no world-anchored axis).
//! Bank is intentionally ignored throughout (assumed 0 in fly mode).

use crate::app::App;
use crate::config::ConfigPath;
use crate::storage::FlyCameraMode;
use winit::keyboard::KeyCode;

/// Threshold on |sin pitch| below which the Euler chart is treated
/// as gimbal-locked (yaw and rotation alias). f32 `acos` near ±1
/// can't resolve sin-pitch much below ~5e-4 anyway.
const POLE_EPS: f32 = 1e-3;

/// Wrap an angle into `[−π, π]` — the range the View-panel sliders
/// display as −180°..180°. The free-look decomposition tracks angles
/// continuously, so a sustained drag would otherwise walk the stored
/// value past the slider range; wrapping at the write boundary keeps
/// the config canonical. 2π-periodic, so the camera matrix rebuilt
/// from the wrapped value is identical.
fn wrap_pi(a: f32) -> f32 {
    use std::f32::consts::TAU;
    a - TAU * (a / TAU).round()
}

/// Camera orientation matrix. Mirrors the matrix the GPU builds in
/// [`shaders/core/utilities.wgsl::build_camera_matrix`] with our
/// slider→slot mapping:
///
///   - `rotation` → JWF yaw slot, negated
///   - `pitch`    → JWF pitch slot, negated
///   - `bank`     → JWF bank slot, negated (we assume 0 here)
///   - `yaw`      → JWF roll slot
///
/// Algebraically this factors as the ZXZ Euler product
/// `M = Rz(rotation) · Rx(pitch) · Rz(−yaw)` (verified numerically
/// against the shipped WGSL element-by-element). With non-zero bank
/// the full chain is `Rz(rotation) · Rx(pitch) · Ry(bank) · Rz(−yaw)`
/// — bank sits between pitch and yaw, matching JWildfire's
/// transposed-application order — but fly mode assumes bank = 0, so
/// the `Ry` factor is omitted here.
///
/// Stored row-major as `m[row][col]`. The matrix maps a world point
/// to camera space via `p_cam = M · (p_world − camera_pos)`. The
/// camera's look direction is camera-space `−Z`, so its rows have
/// useful geometric meaning:
///
///   - row 0 = camera-local `+X` in world coords ("right")
///   - row 1 = camera-local `+Y` in world coords (screen-down,
///             because pixel y grows downward)
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
    /// camera-local `+X`. Independent of bank (we assume 0).
    fn right(&self) -> [f32; 3] {
        self.m[0]
    }

    /// World-space direction the camera looks toward — i.e.
    /// camera-local `−Z`, which is `−row2(M)`.
    fn forward(&self) -> [f32; 3] {
        [-self.m[2][0], -self.m[2][1], -self.m[2][2]]
    }

    /// World-space direction that appears as "up" on screen — the
    /// negation of row 1, because camera-local `+Y` is screen-down
    /// (pixel y grows downward). Forms a right-handed frame:
    /// `right × up = forward`.
    fn up(&self) -> [f32; 3] {
        [-self.m[1][0], -self.m[1][1], -self.m[1][2]]
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

    /// Rotation by `angle` radians about a unit `axis`, via
    /// Rodrigues' formula: `R = I + sinθ·K + (1−cosθ)·K²` where K is
    /// the cross-product matrix of the axis. Caller must normalize
    /// the axis.
    fn axis_angle(axis: [f32; 3], angle: f32) -> Self {
        let (x, y, z) = (axis[0], axis[1], axis[2]);
        let s = angle.sin();
        let c = angle.cos();
        let t = 1.0 - c;
        Self {
            m: [
                [t * x * x + c,      t * x * y - s * z,  t * x * z + s * y],
                [t * x * y + s * z,  t * y * y + c,      t * y * z - s * x],
                [t * x * z - s * y,  t * y * z + s * x,  t * z * z + c],
            ],
        }
    }

    /// Matrix product `self · other`. Left-multiplying the camera
    /// matrix by a rotation applies that rotation in *camera space*
    /// (i.e., about an axis given in camera/screen coordinates).
    fn mul(&self, other: &Self) -> Self {
        let mut m = [[0.0f32; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = self.m[i][0] * other.m[0][j]
                    + self.m[i][1] * other.m[1][j]
                    + self.m[i][2] * other.m[2][j];
            }
        }
        Self { m }
    }

    /// Decompose into our `(pitch, yaw, rotation)` Euler triple,
    /// preferring the representation closest to `near`.
    ///
    /// Away from the poles the chart is unique up to two discrete
    /// choices: the sign branch of `pitch = ±acos(m22)` (flipping it
    /// shifts yaw and rotation by π each — same matrix) and 2π wraps
    /// on each angle. Both are resolved toward `near`.
    ///
    /// Within `POLE_EPS` of a pole (|sin pitch| ≈ 0), yaw and
    /// rotation alias into a single angle. We hold `rotation` at its
    /// prior value and put the whole aliased angle into yaw, which
    /// keeps the user's rotation slider stable while hovering near
    /// straight-down/straight-up. (Leaving the pole region, the
    /// exact formulas take over and yaw/rotation land wherever the
    /// geometry dictates — stored values can jump even though the
    /// matrix path is continuous. That is the unavoidable chart
    /// singularity; the guarantee that matters is that the returned
    /// triple always rebuilds this exact matrix.)
    fn to_euler_near(&self, near: (f32, f32, f32)) -> (f32, f32, f32) {
        use std::f32::consts::{PI, TAU};
        let (p_old, y_old, r_old) = near;
        let m = &self.m;

        // Shift `a` by whole turns to land within π of `near`.
        let unwrap_near = |a: f32, near: f32| a + ((near - a) / TAU).round() * TAU;

        let cp = m[2][2].clamp(-1.0, 1.0);
        let p_abs = cp.acos(); // in [0, π]
        let sp_abs = p_abs.sin(); // ≥ 0

        if sp_abs < POLE_EPS {
            // Gimbal-pole regime. The in-plane 2×2 block holds the
            // single well-defined angle:
            //   at pitch ≈ 0: m00 = cos(Y−R), m01 = sin(Y−R)
            //   at pitch ≈ π: m00 = cos(Y+R), m01 = sin(Y+R)
            // (Both verified by substituting sin P = 0 into `build`.)
            let phi = m[0][1].atan2(m[0][0]);
            let (p_pole, y_new) = if cp > 0.0 {
                (0.0, phi + r_old)
            } else {
                (PI, phi - r_old)
            };
            return (
                unwrap_near(p_pole, p_old),
                unwrap_near(y_new, y_old),
                r_old,
            );
        }

        // Away from the pole. Row 2 = (−sinP·sinY, sinP·cosY, cosP)
        // gives yaw; column 2 = (sinP·sinR, −sinP·cosR, cosP) gives
        // rotation. These are exact for the +p_abs branch.
        let y_a = (-m[2][0]).atan2(m[2][1]);
        let r_a = m[0][2].atan2(-m[1][2]);

        // The two equivalent representations: (P, Y, R) and
        // (−P, Y+π, R+π). Score each by summed wrapped distance to
        // `near` and keep the closer one — this is what makes paths
        // through arbitrary orientations come back as continuous
        // angle sequences instead of jumping branches every event.
        let candidate = |p: f32, y: f32, r: f32| {
            let p = unwrap_near(p, p_old);
            let y = unwrap_near(y, y_old);
            let r = unwrap_near(r, r_old);
            let dist = (p - p_old).abs() + (y - y_old).abs() + (r - r_old).abs();
            (dist, (p, y, r))
        };
        let a = candidate(p_abs, y_a, r_a);
        let b = candidate(-p_abs, y_a + PI, r_a + PI);
        if a.0 <= b.0 {
            a.1
        } else {
            b.1
        }
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

    /// Apply a mouse-drag delta as a camera rotation, per the
    /// configured fly-camera mode (see module docs):
    ///
    /// **FreeLook** — the drag vector maps to a single rotation
    /// about the camera-space axis perpendicular to it (exponential
    /// map): horizontal drag rotates about the screen-vertical
    /// axis, vertical drag about the screen-horizontal axis,
    /// diagonals in between — order-free by construction. Because
    /// camera-space axes ARE screen axes, a twisted view
    /// (`rotation ≠ 0`) gets screen-correct drag directions with no
    /// compensation.
    ///
    /// **Fps** — Euler increments: yaw about world-up, pitch about
    /// the screen-plane axis, with the drag pre-rotated by
    /// `rotation` so directions track a twisted screen. `rotation`
    /// itself is never written.
    ///
    /// Sensitivity is radians per pixel; invert-Y flips the
    /// vertical sign. Both modes wrap written angles into
    /// `[−π, π]` (the slider-visible range).
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
        if drag_dx == 0.0 && drag_dy == 0.0 {
            return;
        }
        let settings = self.config_manager.system_settings();
        let sensitivity = settings.fly_mouse_sensitivity;
        let invert_y = settings.fly_invert_y;
        let mode = settings.fly_camera_mode;

        // Snapshot every scalar we need up-front and drop the
        // immutable borrow so the `update_param` calls below can take
        // the mutable borrow.
        let (pitch_old, yaw_old, rotation_old, pan_x, pan_y, cam_x, cam_y, cam_z) = {
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

        let dy_sign = if invert_y { 1.0 } else { -1.0 };

        let (pitch_new, yaw_new, rotation_new) = match mode {
            FlyCameraMode::FreeLook => {
                // Per-axis rotation amounts. Signs chosen so behavior
                // matches the Fps scheme at the horizon pose (pitch =
                // π/2, rotation = 0), where the two modes coincide:
                // drag right looks toward screen-right, drag down
                // looks toward screen-bottom (non-inverted).
                let a_pitch = drag_dy * sensitivity * dy_sign; // about screen-right axis (camera x̂)
                let a_yaw = drag_dx * sensitivity; // about screen-vertical axis (camera ŷ)
                let angle = (a_pitch * a_pitch + a_yaw * a_yaw).sqrt();
                if angle < 1e-8 {
                    return;
                }

                let m_old = CameraMatrix::build(pitch_old, yaw_old, rotation_old);
                // Single combined rotation about the camera-space
                // axis perpendicular to the drag direction.
                // Left-multiply = applied in camera space.
                let delta =
                    CameraMatrix::axis_angle([a_pitch / angle, a_yaw / angle, 0.0], angle);
                let m_new = delta.mul(&m_old);
                m_new.to_euler_near((pitch_old, yaw_old, rotation_old))
            }
            FlyCameraMode::Fps => {
                // Euler increments ARE the world-anchored rotations:
                // yaw += δ is exactly "rotate camera about world-up",
                // pitch += δ is exactly "rotate about the screen-plane
                // axis at angle `rotation` from screen-right" (both
                // pinned by `axis_angle_matches_euler_increments`).
                // Convert the drag from screen frame to camera frame
                // first so directions track a twisted screen; at
                // rotation = 0 this is the identity.
                let cr = rotation_old.cos();
                let sr = rotation_old.sin();
                let dx_local = drag_dx * cr + drag_dy * sr;
                let dy_local = -drag_dx * sr + drag_dy * cr;
                (
                    pitch_old + dy_local * sensitivity * dy_sign,
                    yaw_old + dx_local * sensitivity,
                    rotation_old, // FPS look never rolls
                )
            }
        };

        // Canonicalize into the slider-visible range. Both modes
        // track angles continuously, so values can walk past ±π
        // during sustained drags; wrapping here keeps the config
        // matching the −180°..180° the View sliders show, with an
        // identical rebuilt matrix. Rotation is only wrapped when
        // mouse-look actually changed it — wrap_pi(π) = −π, and
        // silently rewriting an untouched slider value (FPS mode
        // never rolls) would be surprising.
        let pitch_new = wrap_pi(pitch_new);
        let yaw_new = wrap_pi(yaw_new);
        let rotation_new = if rotation_new != rotation_old {
            wrap_pi(rotation_new)
        } else {
            rotation_old
        };

        // All writes for this event go out as ONE batch carrying the
        // fly-camera history marker: consecutive fly batches coalesce
        // into a single undo entry (path-keyed merge in push_undo)
        // even though the path set varies event to event — without
        // this, flying floods the undo history within seconds.
        let mut changes: Vec<(ConfigPath, crate::config::ConfigValue)> = vec![
            (ConfigPath::CameraRotationX, pitch_new.into()),
            (ConfigPath::CameraRotationY, yaw_new.into()),
        ];
        // Free-look rolls: rotation legitimately drifts as the
        // camera's screen axes precess (e.g. circular mouse motion).
        // Skip the write when unchanged so undo history and the
        // slider stay quiet on pure pitch/yaw paths.
        if rotation_new != rotation_old {
            changes.push((ConfigPath::Rotation, rotation_new.into()));
        }

        // Pan-pivot compensation. Skip entirely when pan is zero
        // because then `camera_pos` already projects to screen center
        // and the rotation pivot is correct as-is.
        if pan_x != 0.0 || pan_y != 0.0 {
            // Pan lives in the no-roll projected frame —
            // `world_to_pixel_3d` subtracts pan BEFORE applying the
            // screen rotation (2D-parity composition). The world
            // point at screen center is therefore
            // `camera_pos + M_noroll^T · (pan_x, pan_y, 0)`, so the
            // compensation uses matrices built without the rotation
            // factor.
            let m_old_nr = CameraMatrix::build(pitch_old, yaw_old, 0.0);
            let m_new_nr = CameraMatrix::build(pitch_new, yaw_new, 0.0);
            let off_old = m_old_nr.world_offset_for_camera_xy(pan_x, pan_y);
            let off_new = m_new_nr.world_offset_for_camera_xy(pan_x, pan_y);
            changes.push((ConfigPath::CameraX, (cam_x + off_old[0] - off_new[0]).into()));
            changes.push((ConfigPath::CameraY, (cam_y + off_old[1] - off_new[1]).into()));
            changes.push((ConfigPath::CameraZ, (cam_z + off_old[2] - off_new[2]).into()));
        }
        let _ = self.config_manager.update_batch(
            changes,
            crate::config::manager::FLY_CAMERA_HISTORY_DESC.to_string(),
        );
    }

    /// Per-frame fly-camera integration. Reads `fly_keys_held` and
    /// translates `camera_x/y/z` along the camera-local basis:
    /// W/S = look axis, A/D = screen-right, Q/E = screen-down/up in
    /// FreeLook or world-down/up in Fps mode. Speed comes from
    /// SystemSettings, scaled by the sprint multiplier while Shift
    /// is held.
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
        // Q/E axis follows the camera mode: FreeLook rises along
        // screen-up (space-sim — everything camera-relative), Fps
        // rises along world-Z (everything anchored to the XY plane).
        let up = match self.config_manager.system_settings().fly_camera_mode {
            FlyCameraMode::FreeLook => m.up(),
            FlyCameraMode::Fps => [0.0, 0.0, 1.0],
        };

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
                delta[i] += up[i] * step;
            }
        }
        if self.fly_keys_held.contains(&KeyCode::KeyQ) {
            for i in 0..3 {
                delta[i] -= up[i] * step;
            }
        }

        if delta == [0.0; 3] {
            return;
        }

        let new_x = cfg.camera_x + delta[0];
        let new_y = cfg.camera_y + delta[1];
        let new_z = cfg.camera_z + delta[2];
        // Single batch with the fly-camera history marker — coalesces
        // with the rest of the fly gesture (see apply_fly_mouse_look).
        let _ = self.config_manager.update_batch(
            vec![
                (ConfigPath::CameraX, new_x.into()),
                (ConfigPath::CameraY, new_y.into()),
                (ConfigPath::CameraZ, new_z.into()),
            ],
            crate::config::manager::FLY_CAMERA_HISTORY_DESC.to_string(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn mat_err(a: &CameraMatrix, b: &CameraMatrix) -> f32 {
        let mut worst = 0.0f32;
        for i in 0..3 {
            for j in 0..3 {
                worst = worst.max((a.m[i][j] - b.m[i][j]).abs());
            }
        }
        worst
    }

    /// `build` produces orthonormal right-handed matrices, and `mul`
    /// against the transpose recovers identity.
    #[test]
    fn build_is_rotation_matrix() {
        let angles = [-2.8f32, -1.2, -0.3, 0.0, 0.4, 1.5, 2.9];
        for &p in &angles {
            for &y in &angles {
                for &r in &angles {
                    let m = CameraMatrix::build(p, y, r);
                    let mt = CameraMatrix {
                        m: [
                            [m.m[0][0], m.m[1][0], m.m[2][0]],
                            [m.m[0][1], m.m[1][1], m.m[2][1]],
                            [m.m[0][2], m.m[1][2], m.m[2][2]],
                        ],
                    };
                    let ident = m.mul(&mt);
                    for i in 0..3 {
                        for j in 0..3 {
                            let want = if i == j { 1.0 } else { 0.0 };
                            assert!(
                                (ident.m[i][j] - want).abs() < 1e-5,
                                "M·Mᵀ ≠ I at ({p}, {y}, {r})"
                            );
                        }
                    }
                }
            }
        }
    }

    /// The two sign-convention identities everything else rests on,
    /// ported from the numeric derivation:
    ///   1. Euler yaw increment == right-multiply by Rz(−δ)
    ///      (rotating the camera about world-up)
    ///   2. Euler pitch increment == left-multiply by a rotation
    ///      about the camera-space axis (cos R, sin R, 0)
    #[test]
    fn axis_angle_matches_euler_increments() {
        let cases = [
            (0.3f32, -1.1f32, 0.7f32, 0.25f32),
            (-2.0, 2.4, -0.4, -0.6),
            (1.5, 0.0, 0.0, 0.1),
            (0.05, 3.0, -2.0, 0.5),
        ];
        for &(p, y, r, d) in &cases {
            let m = CameraMatrix::build(p, y, r);

            let yaw_inc = CameraMatrix::build(p, y + d, r);
            let rz_neg = CameraMatrix::axis_angle([0.0, 0.0, 1.0], -d);
            assert!(
                mat_err(&yaw_inc, &m.mul(&rz_neg)) < 1e-5,
                "yaw identity failed at ({p}, {y}, {r}, {d})"
            );

            let pitch_inc = CameraMatrix::build(p + d, y, r);
            let axis = [r.cos(), r.sin(), 0.0];
            let rot = CameraMatrix::axis_angle(axis, d);
            assert!(
                mat_err(&pitch_inc, &rot.mul(&m)) < 1e-5,
                "pitch identity failed at ({p}, {y}, {r}, {d})"
            );
        }
    }

    /// Round trip: decompose(build(P, Y, R)) returns (P, Y, R) when
    /// told to stay near the original — across a grid that avoids
    /// the poles.
    #[test]
    fn to_euler_round_trip_grid() {
        let pitches = [-2.9f32, -1.6, -0.8, 0.2, 1.1, 2.4];
        let others = [-3.0f32, -1.2, 0.0, 0.9, 2.7];
        for &p in &pitches {
            for &y in &others {
                for &r in &others {
                    let m = CameraMatrix::build(p, y, r);
                    let (p2, y2, r2) = m.to_euler_near((p, y, r));
                    assert!(
                        (p2 - p).abs() < 1e-3
                            && (y2 - y).abs() < 1e-3
                            && (r2 - r).abs() < 1e-3,
                        "round trip ({p}, {y}, {r}) → ({p2}, {y2}, {r2})"
                    );
                }
            }
        }
    }

    /// At the gimbal pole the decomposition holds rotation at its
    /// prior value exactly, and the returned triple still rebuilds
    /// the same matrix.
    #[test]
    fn pole_holds_rotation_and_reconstructs() {
        // Pure-pole orientation: pitch = 0 → M = Rz(R − Y).
        for &(y0, r0) in &[(0.4f32, 1.3f32), (-2.0, 0.0), (3.0, -2.5)] {
            let m = CameraMatrix::build(0.0, y0, r0);
            let (p2, y2, r2) = m.to_euler_near((0.0, y0, r0));
            assert_eq!(r2, r0, "rotation must be held at the pole");
            let rebuilt = CameraMatrix::build(p2, y2, r2);
            assert!(
                mat_err(&m, &rebuilt) < 1e-5,
                "pole reconstruction failed for ({y0}, {r0})"
            );
        }
        // Same at the pitch = π pole.
        let m = CameraMatrix::build(PI, 0.7, -0.9);
        let (p2, y2, r2) = m.to_euler_near((PI, 0.7, -0.9));
        assert_eq!(r2, -0.9);
        let rebuilt = CameraMatrix::build(p2, y2, r2);
        assert!(mat_err(&m, &rebuilt) < 1e-5);
    }

    /// The gimbal-free guarantee: a free-look path that pitches
    /// straight through the pole (and then wanders diagonally) stays
    /// smooth at the matrix level AND every decomposed triple
    /// faithfully rebuilds its matrix. This is exactly the per-event
    /// flow of `apply_fly_mouse_look`, simulated.
    #[test]
    fn free_look_through_pole_is_smooth_and_faithful() {
        let mut euler = (0.2f32, 0.7f32, 0.4f32);
        let mut m = CameraMatrix::build(euler.0, euler.1, euler.2);
        let mut prev = CameraMatrix { m: m.m };

        let mut step = |m: &mut CameraMatrix,
                        prev: &mut CameraMatrix,
                        euler: &mut (f32, f32, f32),
                        axis: [f32; 3],
                        angle: f32| {
            let delta = CameraMatrix::axis_angle(axis, angle);
            *m = delta.mul(m);
            // Matrix path must be continuous: one step of size
            // `angle` moves matrix entries by at most ~angle.
            assert!(
                mat_err(m, prev) < 2.0 * angle.abs() + 1e-4,
                "matrix path jumped"
            );
            prev.m = m.m;
            // Decompose near the previous stored triple, as the real
            // code does, then verify faithful reconstruction. The
            // pole-hold approximation may force rotation, costing up
            // to ~POLE_EPS of matrix error in the cone — allow 2×.
            let new_euler = m.to_euler_near(*euler);
            let rebuilt = CameraMatrix::build(new_euler.0, new_euler.1, new_euler.2);
            assert!(
                mat_err(m, &rebuilt) < 2.0 * POLE_EPS,
                "reconstruction unfaithful at euler {new_euler:?}"
            );
            // The real code feeds the *rebuilt* orientation forward
            // (config is the source of truth), so mirror that here to
            // catch error accumulation across the pole cone.
            m.m = rebuilt.m;
            *euler = new_euler;
        };

        // Phase 1: pitch straight down through the pole (pitch
        // crosses 0 around step 10).
        for _ in 0..40 {
            step(&mut m, &mut prev, &mut euler, [1.0, 0.0, 0.0], -0.02);
        }
        // Phase 2: diagonal drag (pitch + yaw mix) — exercises the
        // roll-drift path where rotation legitimately changes.
        for _ in 0..40 {
            step(&mut m, &mut prev, &mut euler, [0.6, 0.8, 0.0], 0.03);
        }
        // Phase 3: pure yaw from wherever we ended up.
        for _ in 0..40 {
            step(&mut m, &mut prev, &mut euler, [0.0, 1.0, 0.0], 0.025);
        }
    }

    /// `up()` is screen-up and completes a right-handed camera frame:
    /// right × up = forward.
    #[test]
    fn up_is_screen_up() {
        for &(p, y, r) in &[(0.7f32, -1.9f32, 2.2f32), (0.0, 0.0, 0.0), (1.5, 0.4, -0.9)] {
            let m = CameraMatrix::build(p, y, r);
            let (rt, up, fw) = (m.right(), m.up(), m.forward());
            let cross = [
                rt[1] * up[2] - rt[2] * up[1],
                rt[2] * up[0] - rt[0] * up[2],
                rt[0] * up[1] - rt[1] * up[0],
            ];
            for i in 0..3 {
                assert!(
                    (cross[i] - fw[i]).abs() < 1e-5,
                    "right × up ≠ forward at ({p}, {y}, {r})"
                );
            }
        }
    }

    /// `wrap_pi` lands in [−π, π] and never changes the angle mod 2π.
    #[test]
    fn wrap_pi_lands_in_range() {
        use std::f32::consts::TAU;
        for &a in &[0.0f32, 3.0, -3.0, 4.0, -4.0, 7.5, -7.5, 100.0, -100.0] {
            let w = wrap_pi(a);
            assert!((-PI..=PI).contains(&w), "wrap_pi({a}) = {w} out of range");
            let turns = (a - w) / TAU;
            assert!(
                (turns - turns.round()).abs() < 1e-3,
                "wrap_pi({a}) changed the angle"
            );
        }
    }

    /// Free-look coincides with the old FPS scheme at the horizon
    /// pose (pitch = π/2, rotation = 0): rotating about the
    /// screen-vertical axis is exactly an Euler yaw increment there.
    #[test]
    fn free_look_yaw_matches_euler_yaw_at_horizon() {
        let m = CameraMatrix::build(PI / 2.0, 0.3, 0.0);
        let d = 0.2f32;
        let rotated = CameraMatrix::axis_angle([0.0, 1.0, 0.0], d).mul(&m);
        let euler_inc = CameraMatrix::build(PI / 2.0, 0.3 + d, 0.0);
        assert!(
            mat_err(&rotated, &euler_inc) < 1e-5,
            "free-look yaw ≠ Euler yaw at horizon"
        );
    }
}

//! The one Euler chain every 3D camera stores its orientation in, and
//! the way back from a rotation to its angles.
//!
//! Every 3D camera in the app is the flame's chain (`build_camera_matrix`
//! in `shaders/core/utilities.wgsl`): world → camera rows
//!
//! ```text
//! M = Rz(roll) · Rx(pitch) · Ry(bank) · Rz(−yaw)
//! ```
//!
//! -- the flame's directly; mode D's and the terrains' re-zeroed and
//! mirrored (`escape::ifs::solid_frame`). Rows: 0 screen-right, 1
//! screen-DOWN, 2 the camera's +z (it looks along −row 2).
//!
//! **Back to angles.** A rotation has three degrees of freedom and the
//! chain four angles, so the way back holds one: the ROLL, which is a
//! turn of the screen (a flame's and a solid's `rotation`; zero on the
//! terrains, which have none). What remains, `Rx(pitch)·Ry(bank)·
//! Rz(−yaw)`, is a Tait–Bryan sequence: unique up to one branch and 2π,
//! and singular only where the bank is ±90° -- a camera rolled on its
//! side. Today's flame free-look solved for pitch, yaw and ROLL instead
//! (a ZXZ sequence), whose singularity is pitch 0: the flame's home pose,
//! looking straight down, which is why its sliders jumped there
//! (docs/projects/camera-unification.md, C2).
//!
//! Of the two solutions, and every 2π, the decomposition returns the one
//! nearest the angles it was given, so a turn moves the sliders by about
//! the turn, and a slider jumps only where the angles truly alias.

use super::quat::Mat3;

/// Where |cos bank| is below this, pitch and yaw alias into one angle.
const POLE_EPS: f64 = 1e-6;

pub fn rot_x(a: f64) -> Mat3 {
    let (s, c) = a.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
}

pub fn rot_y(a: f64) -> Mat3 {
    let (s, c) = a.sin_cos();
    [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]]
}

pub fn rot_z(a: f64) -> Mat3 {
    let (s, c) = a.sin_cos();
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}

/// The chain's world → camera rows for four angles, in radians.
pub fn rows(pitch: f64, yaw: f64, bank: f64, roll: f64) -> Mat3 {
    use super::quat::mat_mul;
    mat_mul(&mat_mul(&rot_z(roll), &rot_x(pitch)), &mat_mul(&rot_y(bank), &rot_z(-yaw)))
}

/// Shift `a` by whole turns to within π of `near`.
pub fn unwrap_near(a: f64, near: f64) -> f64 {
    use std::f64::consts::TAU;
    a + ((near - a) / TAU).round() * TAU
}

/// `(pitch, yaw, bank)` of the chain for `m`, its roll held at `roll`:
/// the solution nearest `near`, each angle unwrapped toward it.
///
/// The solutions, away from the pole, are `(p, y, b)` and
/// `(p + π, y + π, π − b)` -- the same rotation. At the pole (bank
/// ±90°) pitch and yaw alias; the yaw is held at `near`'s and the pitch
/// takes the rest, so a camera on its side keeps its heading.
pub fn decompose_near(m: &Mat3, roll: f64, near: (f64, f64, f64)) -> (f64, f64, f64) {
    use super::quat::mat_mul;
    use std::f64::consts::PI;
    let (p0, y0, b0) = near;
    // X = Rz(−roll) · M = Rx(p) · Ry(b) · Rz(Y), with Y = −yaw:
    //   row 0: [ cb·cY, −cb·sY, sb ]
    //   row 1: [ cp·sY + sp·sb·cY, cp·cY − sp·sb·sY, −sp·cb ]
    //   row 2: [ sp·sY − cp·sb·cY, sp·cY + cp·sb·sY,  cp·cb ]
    let x = mat_mul(&rot_z(-roll), m);
    let sb = x[0][2].clamp(-1.0, 1.0);
    let cb = (1.0 - sb * sb).max(0.0).sqrt();
    if cb < POLE_EPS {
        // Bank ±90°: row 1 is (sin φ, cos φ, 0) with φ = Y + p at +90°,
        // Y − p at −90°. Hold the yaw.
        let phi = x[1][0].atan2(x[1][1]);
        let yy = -y0;
        let (b, p) = if sb > 0.0 { (PI / 2.0, phi - yy) } else { (-PI / 2.0, yy - phi) };
        return (unwrap_near(p, p0), y0, unwrap_near(b, b0));
    }
    let b = sb.asin();
    let yaw = -((-x[0][1]).atan2(x[0][0]));
    let p = (-x[1][2]).atan2(x[2][2]);
    let candidate = |p: f64, y: f64, b: f64| {
        let (p, y, b) = (unwrap_near(p, p0), unwrap_near(y, y0), unwrap_near(b, b0));
        ((p - p0).abs() + (y - y0).abs() + (b - b0).abs(), (p, y, b))
    };
    let a = candidate(p, yaw, b);
    let other = candidate(p + PI, yaw + PI, PI - b);
    if a.0 <= other.0 {
        a.1
    } else {
        other.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::quat::{mat_mul, max_diff, Quat};

    fn grid() -> Vec<f64> {
        (-6..=6).map(|k| k as f64 * 0.53).collect()
    }

    /// The chain is the flame's: with no roll, the rows the shade pass
    /// replicates from the main pass (`effective_camera_rows`, which
    /// copies `build_camera_matrix`'s slots), bank included.
    #[test]
    fn the_chain_is_the_flames() {
        for &p in &grid() {
            for &y in &grid() {
                for &b in &[-1.3, -0.4, 0.0, 0.6, 2.2] {
                    let shade = crate::renderer::shade_pass::effective_camera_rows(p as f32, y as f32, b as f32);
                    let shade: Mat3 = std::array::from_fn(|i| std::array::from_fn(|j| shade[i][j] as f64));
                    let d = max_diff(&rows(p, y, b, 0.0), &shade);
                    assert!(d < 1e-5, "({p}, {y}, {b}): {d}");
                }
            }
        }
    }

    /// The roll is the outermost turn: a turn of the screen.
    #[test]
    fn the_roll_turns_the_screen() {
        let (p, y, b, r) = (0.7, -1.1, 0.3, 0.9);
        let d = max_diff(&rows(p, y, b, r), &mat_mul(&rot_z(r), &rows(p, y, b, 0.0)));
        assert!(d < 1e-12, "{d}");
    }

    /// Angles → rows → angles rebuilds the same rows everywhere -- the
    /// pole included -- and, given the angles themselves as the hint,
    /// returns them (mod 2π) away from the pole.
    #[test]
    fn decompose_round_trips() {
        for &p in &grid() {
            for &y in &grid() {
                for &b in &grid() {
                    for &r in &[0.0, 0.8, -2.5] {
                        let m = rows(p, y, b, r);
                        let (p2, y2, b2) = decompose_near(&m, r, (p, y, b));
                        let d = max_diff(&rows(p2, y2, b2, r), &m);
                        assert!(d < 1e-9, "({p}, {y}, {b}, {r}) -> ({p2}, {y2}, {b2}): {d}");
                        if b.cos().abs() > 1e-3 {
                            let same = (p2 - p).abs() + (y2 - y).abs() + (b2 - b).abs();
                            assert!(same < 1e-9, "({p}, {y}, {b}) came back as ({p2}, {y2}, {b2})");
                        }
                    }
                }
            }
        }
    }

    /// The point of holding the roll: free-look from the flame's home
    /// pose (all angles zero, looking straight down) moves the angles by
    /// about the turn. The ZXZ decomposition free-look used jumped there:
    /// a tiny turn could come back as a yaw and a roll of 90° each.
    #[test]
    fn free_look_from_home_moves_the_angles_by_the_turn() {
        for axis in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [0.3, -1.0, 0.0]] {
            let mut q = Quat::from_rows(&rows(0.0, 0.0, 0.0, 0.0));
            let mut angles = (0.0, 0.0, 0.0);
            for _ in 0..50 {
                q = Quat::from_axis_angle(axis, 0.002) * q;
                let next = decompose_near(&q.to_rows(), 0.0, angles);
                let step = (next.0 - angles.0).abs() + (next.1 - angles.1).abs() + (next.2 - angles.2).abs();
                assert!(step < 0.004, "{axis:?}: a 0.002 turn moved the angles {step}");
                angles = next;
            }
        }
    }

    /// A long free-look path -- through straight down, straight up and a
    /// camera on its side -- rebuilds the turned orientation at every
    /// step, and the angles move by about each step except where the
    /// bank crosses ±90°, the one place they alias.
    #[test]
    fn a_free_look_path_is_faithful() {
        let mut q = Quat::from_rows(&rows(0.2, 0.4, 0.0, 0.3));
        let mut angles = (0.2, 0.4, 0.0);
        for i in 0..3000 {
            let axis = [((i as f64) * 0.013).sin(), ((i as f64) * 0.007).cos(), 0.0];
            q = Quat::from_axis_angle(axis, 0.01) * q;
            let next = decompose_near(&q.to_rows(), 0.3, angles);
            let d = max_diff(&rows(next.0, next.1, next.2, 0.3), &q.to_rows());
            assert!(d < 1e-9, "step {i}: {d}");
            let near_pole = angles.2.cos().abs() < 0.1 || next.2.cos().abs() < 0.1;
            let step = (next.0 - angles.0).abs() + (next.1 - angles.1).abs() + (next.2 - angles.2).abs();
            assert!(near_pole || step < 0.3, "step {i}: the angles jumped {step}");
            angles = next;
        }
    }
}

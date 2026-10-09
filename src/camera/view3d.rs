//! The 3D cameras' angles, read the same way (docs/projects/
//! camera-unification.md, C1-C2).
//!
//! Each 3D camera stores four angles in its own terms and builds its
//! frame from the one chain (`super::chain`). A [`Convention`] says how
//! its angles enter the chain, so every camera's orientation is a
//! quaternion of the same kind, and every turn -- an orbit, a look, a
//! fly -- composes alike and comes back as that camera's angles.
//!
//! - **The flame** stores the chain's slots as they are: pitch 0 looks
//!   straight down (JWildfire's), the screen-roll is `rotation`.
//! - **Mode D and the terrains** (`escape::ifs::solid_frame`) measure
//!   pitch from the horizon, shift the yaw, turn the roll the other way,
//!   and mirror the screen's x (the plane draws Im up, the flame draws y
//!   down). The escape terrain adds its `rotation` to the heading; the
//!   terrains have no roll.

use super::chain;
use super::quat::{Mat3, Quat};
use std::f64::consts::FRAC_PI_2;

/// A camera's four stored angles, radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Angles {
    pub pitch: f64,
    pub yaw: f64,
    pub bank: f64,
    /// The turn of the screen. Held through any turn of the camera: the
    /// angles that come back from one are pitch, yaw and bank.
    pub roll: f64,
}

/// A camera's axes in its world: unit vectors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub right: [f64; 3],
    pub up: [f64; 3],
    pub forward: [f64; 3],
}

/// How a camera's stored angles enter the chain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Convention {
    /// The chain's own slots.
    Flame,
    /// `solid_frame`'s: pitch from the horizon, the stored yaw plus
    /// `yaw_offset` then less a quarter turn, the roll reversed, the
    /// screen's x mirrored.
    Solid { yaw_offset: f64 },
}

impl Convention {
    /// The chain's slots for `a`.
    fn to_chain(&self, a: Angles) -> Angles {
        match *self {
            Convention::Flame => a,
            Convention::Solid { yaw_offset } => Angles {
                pitch: FRAC_PI_2 - a.pitch,
                yaw: a.yaw + yaw_offset - FRAC_PI_2,
                bank: a.bank,
                roll: -a.roll,
            },
        }
    }

    /// The stored angles for the chain's slots `c` -- [`Self::to_chain`]
    /// undone.
    fn from_chain(&self, c: Angles) -> Angles {
        match *self {
            Convention::Flame => c,
            Convention::Solid { yaw_offset } => Angles {
                pitch: FRAC_PI_2 - c.pitch,
                yaw: c.yaw - yaw_offset + FRAC_PI_2,
                bank: c.bank,
                roll: -c.roll,
            },
        }
    }

    /// The orientation: the chain's world → camera rotation. (A solid's
    /// mirrored x is a reflection no rotation holds; it is applied to
    /// the frame, not stored in the orientation.)
    pub fn orientation(&self, a: Angles) -> Quat {
        let c = self.to_chain(a);
        Quat::from_rows(&chain::rows(c.pitch, c.yaw, c.bank, c.roll))
    }

    /// The frame an orientation gives this camera.
    pub fn frame_of(&self, q: &Quat) -> Frame {
        let m: Mat3 = q.to_rows();
        let neg = |v: [f64; 3]| [-v[0], -v[1], -v[2]];
        let right = match self {
            Convention::Flame => m[0],
            Convention::Solid { .. } => neg(m[0]),
        };
        Frame { right, up: neg(m[1]), forward: neg(m[2]) }
    }

    /// The frame for stored angles.
    pub fn frame(&self, a: Angles) -> Frame {
        self.frame_of(&self.orientation(a))
    }

    /// The stored angles of an orientation: the roll held at `near`'s,
    /// pitch, yaw and bank the solution nearest `near`
    /// (`chain::decompose_near`).
    pub fn angles_near(&self, q: &Quat, near: Angles) -> Angles {
        let c = self.to_chain(near);
        let (pitch, yaw, bank) = chain::decompose_near(&q.to_rows(), c.roll, (c.pitch, c.yaw, c.bank));
        self.from_chain(Angles { pitch, yaw, bank, roll: c.roll })
    }
}

/// A 3D flame's angles: pitch, yaw, bank, and `rotation` as the roll.
pub fn flame_angles(config: &crate::config::FractalConfig) -> (Convention, Angles) {
    (
        Convention::Flame,
        Angles {
            pitch: config.camera_rotation_x as f64,
            yaw: config.camera_rotation_y as f64,
            bank: config.camera_bank as f64,
            roll: config.rotation as f64,
        },
    )
}

/// Mode D's solid camera's angles (`escape::ifs::solid_camera`).
#[cfg(feature = "engine-escape")]
pub fn solid_angles(escape: &crate::config::escape::EscapeConfig) -> (Convention, Angles) {
    (
        Convention::Solid { yaw_offset: 0.0 },
        Angles {
            pitch: escape.cam_pitch as f64,
            yaw: escape.cam_yaw as f64,
            bank: escape.cam_bank as f64,
            roll: escape.rotation as f64,
        },
    )
}

/// The escape terrain's camera's angles (`escape::footprint::terrain_camera`):
/// mode D's, the plane's rotation added to the heading, no roll.
#[cfg(feature = "terrain")]
pub fn escape_terrain_angles(escape: &crate::config::escape::EscapeConfig) -> (Convention, Angles) {
    (
        Convention::Solid { yaw_offset: escape.rotation as f64 - FRAC_PI_2 },
        Angles { pitch: escape.cam_pitch as f64, yaw: escape.cam_yaw as f64, bank: escape.cam_bank as f64, roll: 0.0 },
    )
}

/// A simulation terrain's camera's angles (`sim::terrain::sim_terrain_camera`):
/// yaw 0 looking north, no roll.
#[cfg(all(feature = "terrain", feature = "engine-sim"))]
pub fn sim_terrain_angles(terrain: &crate::config::sim::SimTerrainConfig) -> (Convention, Angles) {
    (
        Convention::Solid { yaw_offset: -FRAC_PI_2 },
        Angles { pitch: terrain.cam_pitch as f64, yaw: terrain.cam_yaw as f64, bank: terrain.cam_bank as f64, roll: 0.0 },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < tol)
    }

    fn grid() -> Vec<f64> {
        (-5..=5).map(|k| k as f64 * 0.61).collect()
    }

    /// The flame's frame is the main pass's camera: its rows
    /// (`effective_camera_rows`) turned by the screen's roll.
    #[test]
    fn the_flames_frame_is_the_main_pass_camera() {
        let mut c = crate::config::FractalConfig::default();
        for &p in &grid() {
            for &y in &grid() {
                for &b in &[-0.9, 0.0, 0.4] {
                    for &r in &[0.0, 0.7] {
                        c.camera_rotation_x = p as f32;
                        c.camera_rotation_y = y as f32;
                        c.camera_bank = b as f32;
                        c.rotation = r as f32;
                        let (conv, a) = flame_angles(&c);
                        let f = conv.frame(a);
                        let rows = crate::renderer::shade_pass::effective_camera_rows(p as f32, y as f32, b as f32);
                        let rows: Mat3 = std::array::from_fn(|i| std::array::from_fn(|j| rows[i][j] as f64));
                        let rolled = super::super::quat::mat_mul(&chain::rot_z(r), &rows);
                        let expect = Frame {
                            right: rolled[0],
                            up: [-rolled[1][0], -rolled[1][1], -rolled[1][2]],
                            forward: [-rolled[2][0], -rolled[2][1], -rolled[2][2]],
                        };
                        assert!(close(f.right, expect.right, 1e-5), "right ({p}, {y}, {b}, {r})");
                        assert!(close(f.up, expect.up, 1e-5), "up ({p}, {y}, {b}, {r})");
                        assert!(close(f.forward, expect.forward, 1e-5), "forward ({p}, {y}, {b}, {r})");
                    }
                }
            }
        }
    }

    /// Mode D's frame is `solid_frame`'s, for every angle and roll.
    #[cfg(feature = "engine-escape")]
    #[test]
    fn the_solids_frame_is_solid_frame() {
        let mut esc = crate::config::escape::EscapeConfig::default();
        for &p in &grid() {
            for &y in &grid() {
                for &b in &[-0.9, 0.0, 0.4] {
                    for &r in &[0.0, -1.2] {
                        esc.cam_pitch = p as f32;
                        esc.cam_yaw = y as f32;
                        esc.cam_bank = b as f32;
                        esc.rotation = r as f32;
                        let (conv, a) = solid_angles(&esc);
                        let f = conv.frame(a);
                        let (right, up, forward) =
                            crate::escape::ifs::solid_frame(p as f32 as f64, y as f32 as f64, b as f32 as f64, r as f32 as f64);
                        assert!(close(f.right, right, 1e-9), "right ({p}, {y}, {b}, {r})");
                        assert!(close(f.up, up, 1e-9), "up ({p}, {y}, {b}, {r})");
                        assert!(close(f.forward, forward, 1e-9), "forward ({p}, {y}, {b}, {r})");
                    }
                }
            }
        }
    }

    /// The escape terrain's frame is `terrain_camera`'s, the plane's
    /// rotation in the heading.
    #[cfg(feature = "terrain")]
    #[test]
    fn the_escape_terrains_frame_is_terrain_camera() {
        let mut esc = crate::config::escape::EscapeConfig::default();
        for &p in &grid() {
            for &y in &grid() {
                for &r in &[0.0, 0.8] {
                    esc.cam_pitch = p as f32;
                    esc.cam_yaw = y as f32;
                    esc.cam_bank = 0.3;
                    esc.rotation = r as f32;
                    let (conv, a) = escape_terrain_angles(&esc);
                    let f = conv.frame(a);
                    let cam = crate::escape::footprint::terrain_camera(&esc);
                    assert!(close(f.right, cam.right, 1e-9) && close(f.up, cam.up, 1e-9) && close(f.forward, cam.forward, 1e-9), "({p}, {y}, {r})");
                }
            }
        }
    }

    /// A simulation terrain's frame is `sim_terrain_camera`'s.
    #[cfg(all(feature = "terrain", feature = "engine-sim"))]
    #[test]
    fn the_sim_terrains_frame_is_sim_terrain_camera() {
        let mut c = crate::config::FractalConfig::default();
        for &p in &grid() {
            for &y in &grid() {
                c.sim.terrain.cam_pitch = p as f32;
                c.sim.terrain.cam_yaw = y as f32;
                c.sim.terrain.cam_bank = -0.2;
                let (conv, a) = sim_terrain_angles(&c.sim.terrain);
                let f = conv.frame(a);
                let cam = crate::sim::terrain::sim_terrain_camera(&c, 128, 96);
                assert!(close(f.right, cam.right, 1e-9) && close(f.up, cam.up, 1e-9) && close(f.forward, cam.forward, 1e-9), "({p}, {y})");
            }
        }
    }

    /// Every convention's angles come back from their orientation --
    /// the roll held -- as the same angles, away from the chain's pole.
    #[test]
    fn angles_round_trip_in_every_convention() {
        for conv in [Convention::Flame, Convention::Solid { yaw_offset: 0.0 }, Convention::Solid { yaw_offset: 0.8 - FRAC_PI_2 }] {
            for &p in &grid() {
                for &y in &grid() {
                    for &b in &[-1.0, 0.0, 0.5] {
                        let a = Angles { pitch: p, yaw: y, bank: b, roll: 0.6 };
                        let back = conv.angles_near(&conv.orientation(a), a);
                        let d = (back.pitch - a.pitch).abs() + (back.yaw - a.yaw).abs() + (back.bank - a.bank).abs() + (back.roll - a.roll).abs();
                        assert!(d < 1e-9, "{conv:?} {a:?} -> {back:?}");
                    }
                }
            }
        }
    }
}

//! One fly mode for every 3D camera (camera-unification C4).
//!
//! **Look** turns the camera about its eye: the drag becomes a world
//! rotation (FreeLook: about the screen's axes, so it can roll; FPS: a
//! turntable about world up and the level axis, so the horizon keeps its
//! tilt), composed on the camera's quaternion and written back as its
//! pitch, yaw and bank -- the roll (a flame's `rotation`, mode D's) held,
//! so no gimbal lock anywhere the decomposition is not singular, which is
//! a camera on its side.
//!
//! **Fly** pushes the camera along its own axes -- W/S forward, A/D right,
//! Q/E up (FreeLook) or world up (FPS).
//!
//! What moves is each camera's **pivot**, the point its fields store:
//! - a 3D flame's `camera_x/y/z`. Its projection divides by
//!   `1 − persp·z`, a pinhole whose viewpoint sits `1/persp` behind the
//!   stored point; a look keeps that viewpoint still and moves the
//!   point. An orthographic flame has no viewpoint, and turns about the
//!   point at the screen's centre, as it always has. A flight moves it in
//!   world units a second;
//! - mode D's target, in decimal digits (a fixed-point add of the step,
//!   so it flies at any depth);
//! - a terrain's target: the escape view's centre (decimal) and its lift,
//!   a simulation's grid fractions and lift.
//!
//! The targets sit a distance `D` in front of the eye, and a look moves
//! one by `(forward' − forward)·D`; a flight moves it `D` a second at
//! speed 1, so a flight feels the same at every zoom. A terrain's flight
//! keeps the eye above the ground -- where an orbit's floor puts it.
//!
//! **A solid knows how far its surface is** (3d-look-and-tools P6): the
//! solid's own distance estimate at the eye, measured on the GPU and read
//! back without waiting (`EscapeRenderer::clearance`), a [`Clearance`].
//! With **collisions** a step never takes the eye further than half that
//! distance from where it was measured -- a sphere-traced step, safe in
//! every direction, so the eye closes on a wall and stops short of it.
//! With **proximity speed** a step is a share of that distance rather
//! than of `D`, and the zoom follows it: `D` eases toward the distance,
//! the eye held where it is (the target slides along the view), so
//! flying into the surface is a deep zoom -- as deep as the solid's
//! precision goes.

use super::gesture::{view_kind, ViewKind};
use super::quat::Quat;
use super::view3d::{self, Angles, Convention, Frame};
use super::CameraEdit;
use crate::config::{ConfigPath, ConfigValue, FractalConfig};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// Every fly edit's history entry: a flight coalesces into one undo.
pub const FLY: &str = crate::config::manager::FLY_CAMERA_HISTORY_DESC;

/// How a drag turns the camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookSettings {
    pub radians_per_pixel: f64,
    pub invert_y: bool,
    /// FreeLook (about the screen's axes) rather than FPS (world up).
    pub free_look: bool,
}

impl LookSettings {
    pub fn from_system(s: &crate::storage::SystemSettings) -> Self {
        LookSettings {
            radians_per_pixel: s.fly_mouse_sensitivity as f64,
            invert_y: s.fly_invert_y,
            free_look: s.fly_camera_mode == crate::storage::FlyCameraMode::FreeLook,
        }
    }
}

/// Which way the keys push, in the camera's terms, each in −1..1; `world_up`
/// makes up the world's +z (FPS) rather than the screen's.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Thrust {
    pub right: f64,
    pub up: f64,
    pub forward: f64,
    pub world_up: bool,
}

/// A camera fly mode can turn: its convention, its angles, and their
/// paths (pitch, yaw, bank).
fn flyer(config: &FractalConfig, kind: ViewKind) -> Option<(Convention, Angles, [ConfigPath; 3])> {
    match kind {
        ViewKind::Flame3d => {
            let (c, a) = view3d::flame_angles(config);
            Some((c, a, [ConfigPath::CameraRotationX, ConfigPath::CameraRotationY, ConfigPath::CameraBank]))
        }
        #[cfg(feature = "engine-escape")]
        ViewKind::Solid => {
            let (c, a) = view3d::solid_angles(&config.escape);
            Some((c, a, [ConfigPath::EscapeCamPitch, ConfigPath::EscapeCamYaw, ConfigPath::EscapeCamBank]))
        }
        #[cfg(feature = "terrain")]
        ViewKind::EscapeTerrain => {
            let (c, a) = view3d::escape_terrain_angles(&config.escape);
            Some((c, a, [ConfigPath::EscapeCamPitch, ConfigPath::EscapeCamYaw, ConfigPath::EscapeCamBank]))
        }
        #[cfg(all(feature = "terrain", feature = "engine-sim"))]
        ViewKind::SimTerrain => {
            let (c, a) = view3d::sim_terrain_angles(&config.sim.terrain);
            Some((c, a, [ConfigPath::SimTerrainCamPitch, ConfigPath::SimTerrainCamYaw, ConfigPath::SimTerrainCamBank]))
        }
        _ => None,
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalized(v: [f64; 3]) -> Option<[f64; 3]> {
    let l = dot(v, v).sqrt();
    (l > 1e-12).then(|| [v[0] / l, v[1] / l, v[2] / l])
}

fn wrap_pi(a: f64) -> f64 {
    a - TAU * (a / TAU).round()
}

/// FreeLook: the view turns toward the drag -- right for a drag right,
/// toward the screen's bottom for a drag down (the top with Invert Y) --
/// about the screen axis across it.
fn free_turn(f: &Frame, drag: [f64; 2], s: LookSettings) -> Option<Quat> {
    let (dx, dy) = (drag[0], if s.invert_y { -drag[1] } else { drag[1] });
    let len = (dx * dx + dy * dy).sqrt();
    if !(len > 0.0) {
        return None;
    }
    let d = normalized(std::array::from_fn(|k| dx * f.right[k] - dy * f.up[k]))?;
    // About n = forward × d, forward turns toward d.
    Some(Quat::from_axis_angle(cross(f.forward, d), len * s.radians_per_pixel))
}

/// FPS: a heading turn about world up toward the drag's side, then a turn
/// about the level axis toward its end of the screen, the view stopping a
/// hair short of straight up or down.
fn level_turn(f: &Frame, drag: [f64; 2], s: LookSettings) -> Option<Quat> {
    let z = [0.0, 0.0, 1.0];
    let (dx, dy) = (drag[0] * s.radians_per_pixel, (if s.invert_y { -drag[1] } else { drag[1] }) * s.radians_per_pixel);
    if dx == 0.0 && dy == 0.0 {
        return None;
    }
    // About +z, forward turns toward z × forward: right when that is.
    let yaw_sign = if dot(cross(z, f.forward), f.right) >= 0.0 { 1.0 } else { -1.0 };
    let ry = Quat::from_axis_angle(z, yaw_sign * dx);
    let (f1, right1, up1) = (ry.rotate(f.forward), ry.rotate(f.right), ry.rotate(f.up));
    let level = normalized(cross(f1, z)).or_else(|| normalized([right1[0], right1[1], 0.0])).unwrap_or([1.0, 0.0, 0.0]);
    // About `level`, forward moves along level × forward: toward the
    // screen's bottom when that points down the screen.
    let lf = cross(level, f1);
    let toward_bottom = if dot(lf, up1) <= 0.0 { 1.0 } else { -1.0 };
    let beta = toward_bottom * dy;
    // The elevation moves by ±beta; it stops short of the poles, but a
    // view already past the limit is not pulled back.
    let el_sign = if lf[2] >= 0.0 { 1.0 } else { -1.0 };
    let el = f1[2].clamp(-1.0, 1.0).asin();
    let lim = FRAC_PI_2 - 1e-3;
    let want = el + beta * el_sign;
    let el_new = if want > el { want.min(lim.max(el)) } else { want.max((-lim).min(el)) };
    let rp = Quat::from_axis_angle(level, (el_new - el) * el_sign);
    Some(rp * ry)
}

/// The stored angles in their canonical range: a solid-convention pitch
/// within ±90° (the config's range for those cameras), past which the
/// same orientation is the pitch the other way round, the heading
/// reversed and the camera upside down.
fn canonical(conv: Convention, a: Angles) -> Angles {
    let mut a = a;
    if let Convention::Solid { .. } = conv {
        a.pitch = wrap_pi(a.pitch);
        if a.pitch.abs() > FRAC_PI_2 {
            a.pitch -= PI * a.pitch.signum();
            a.yaw += PI;
            a.bank = PI - a.bank;
        }
    }
    Angles { pitch: wrap_pi(a.pitch), yaw: wrap_pi(a.yaw), bank: wrap_pi(a.bank), roll: a.roll }
}

/// Turn the 3D camera the viewport shows by a drag of `drag` pixels,
/// about its eye. `panel` is the viewport (a simulation terrain's grid
/// may be the window's).
pub fn look(config: &FractalConfig, drag: [f32; 2], settings: LookSettings, panel: [f32; 2]) -> Option<CameraEdit> {
    let kind = view_kind(config);
    let (conv, from, paths) = flyer(config, kind)?;
    let q = conv.orientation(from);
    let f = conv.frame_of(&q);
    let drag = [drag[0] as f64, drag[1] as f64];
    let r = if settings.free_look { free_turn(&f, drag, settings) } else { level_turn(&f, drag, settings) }?;
    let to = canonical(conv, conv.angles_near(&(q * r.conjugate()), from));
    let mut changes: Vec<(ConfigPath, ConfigValue)> = vec![
        (paths[0].clone(), (to.pitch as f32).into()),
        (paths[1].clone(), (to.yaw as f32).into()),
        (paths[2].clone(), (to.bank as f32).into()),
    ];
    // The frame the written angles render: what the pivot follows.
    let stored = Angles { pitch: to.pitch as f32 as f64, yaw: to.yaw as f32 as f64, bank: to.bank as f32 as f64, roll: to.roll };
    let f2 = conv.frame(stored);
    let df: [f64; 3] = std::array::from_fn(|k| f2.forward[k] - f.forward[k]);
    match kind {
        ViewKind::Flame3d => changes.extend(flame_look_pivot(config, conv, from, stored, df)),
        _ => changes.extend(pivot_moved(config, kind, df, panel, None)),
    }
    Some(CameraEdit::batch(changes, FLY))
}

/// A flame's stored point after a look: moved by the forward's change
/// times `1/persp`, keeping the viewpoint still -- or, orthographic,
/// turned about the point at the screen's centre (the pan's), the
/// orbit the flame's look always was.
fn flame_look_pivot(config: &FractalConfig, conv: Convention, from: Angles, to: Angles, df: [f64; 3]) -> Vec<(ConfigPath, ConfigValue)> {
    let pos = [config.camera_x as f64, config.camera_y as f64, config.camera_z as f64];
    let persp = config.perspective_strength as f64;
    let moved: [f64; 3] = if persp > 1e-6 {
        std::array::from_fn(|k| pos[k] + df[k] / persp)
    } else if config.pan_x != 0.0 || config.pan_y != 0.0 {
        // The pan lives in the unrolled projected frame: the point at the
        // centre is `pos + M_noroll^T · (pan_x, pan_y, 0)`.
        let offset = |a: Angles| {
            let m = conv.orientation(Angles { roll: 0.0, ..a }).to_rows();
            let (px, py) = (config.pan_x, config.pan_y);
            [m[0][0] * px + m[1][0] * py, m[0][1] * px + m[1][1] * py, m[0][2] * px + m[1][2] * py]
        };
        let (a, b) = (offset(from), offset(to));
        std::array::from_fn(|k| pos[k] + a[k] - b[k])
    } else {
        return Vec::new();
    };
    vec![
        (ConfigPath::CameraX, (moved[0] as f32).into()),
        (ConfigPath::CameraY, (moved[1] as f32).into()),
        (ConfigPath::CameraZ, (moved[2] as f32).into()),
    ]
}

/// The pitch an orbit's floor keeps the eye above a terrain's target at
/// rest: the height a terrain's flight keeps it above its floor, in
/// distances to the target (its sine).
#[cfg_attr(not(feature = "terrain"), allow(dead_code))]
const FLOOR_PITCH: f64 = 0.02;

/// What a terrain's flight keeps its eye above, and how it paces it: the
/// view the eye looks along; the ground's height under the eye a probe
/// measured (P6), in the terrain's world units; whether collisions keep
/// the eye above that ground (else above where the target rests, the
/// orbit's floor); and whether the step is sized by the eye's height
/// above it (proximity speed).
#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(feature = "terrain"), allow(dead_code))]
struct Floor {
    forward: [f64; 3],
    ground: Option<f64>,
    collide: bool,
    proximity: bool,
}

impl Floor {
    /// The lowest lift the eye may take, in the lift's units: `d` the
    /// distance to the target in them, `rel` the ground under the eye less
    /// the target's rest height in them.
    #[cfg(feature = "terrain")]
    fn lowest_lift(&self, d: f64, rel: Option<f64>) -> f64 {
        let rel = if self.collide { rel.unwrap_or(0.0) } else { 0.0 };
        rel + d * (FLOOR_PITCH.sin() + self.forward[2])
    }

    /// The step `v` (in distances to the target) under proximity speed:
    /// in heights of the eye above the ground instead, the eye at `lift`;
    /// the floor's own height at least, so it never stalls.
    #[cfg(feature = "terrain")]
    fn paced(&self, v: [f64; 3], lift: f64, d: f64, rel: Option<f64>) -> [f64; 3] {
        match rel {
            Some(rel) if self.proximity && d > 0.0 => {
                let k = ((lift - self.forward[2] * d - rel) / d).max(FLOOR_PITCH.sin());
                v.map(|x| x * k)
            }
            _ => v,
        }
    }
}

/// A target camera's pivot moved by `v`, in multiples of its distance
/// from the eye. With `floor`, a terrain's eye is kept above its floor,
/// and paced by it.
#[allow(unused_variables)]
fn pivot_moved(config: &FractalConfig, kind: ViewKind, v: [f64; 3], panel: [f32; 2], floor: Option<Floor>) -> Vec<(ConfigPath, ConfigValue)> {
    // The lift that keeps a terrain's eye where an orbit's floor would --
    // at a pitch of 0.02 above a target at rest -- or above the ground
    // under it (`Floor::lowest_lift`).
    #[cfg(feature = "terrain")]
    let floored = |lift: f64, d: f64, rel: Option<f64>| match floor {
        Some(f) => lift.max(f.lowest_lift(d, rel)),
        None => lift,
    };
    #[cfg(feature = "terrain")]
    let paced = |v: [f64; 3], lift: f64, d: f64, rel: Option<f64>| match floor {
        Some(f) => f.paced(v, lift, d, rel),
        None => v,
    };
    match kind {
        #[cfg(feature = "engine-escape")]
        ViewKind::Solid => {
            use crate::escape::fixedpoint::FixedPoint;
            let esc = &config.escape;
            let registry = crate::variations::global_registry();
            let Some(ifs3) = crate::escape::ifs::solid_analysis(config, &registry) else {
                return Vec::new();
            };
            drop(registry);
            let (m, e) = crate::escape::ifs::solid_distance(esc, &ifs3);
            let mut out = Vec::new();
            for (k, (path, s)) in [
                (ConfigPath::EscapeCamTargetX, &esc.cam_target_x),
                (ConfigPath::EscapeCamTargetY, &esc.cam_target_y),
                (ConfigPath::EscapeCamTargetZ, &esc.cam_target_z),
            ]
            .into_iter()
            .enumerate()
            {
                // An empty target is the attractor's centre; it becomes
                // explicit the moment the camera moves.
                let base = if s.trim().is_empty() { format!("{}", ifs3.ball.centre[k]) } else { s.trim().to_string() };
                let next = if v[k] == 0.0 {
                    Some(base)
                } else {
                    FixedPoint::decimal_add_floatexp(&base, v[k] * m, e, esc.zoom_log2)
                };
                if let Some(next) = next {
                    if next != *s {
                        out.push((path, ConfigValue::String(next)));
                    }
                }
            }
            out
        }
        #[cfg(feature = "terrain")]
        ViewKind::EscapeTerrain => {
            use crate::escape::fixedpoint::FixedPoint;
            let esc = &config.escape;
            let d = crate::escape::footprint::FRAME_DISTANCE;
            // The ground under the eye less the relief's top, where the
            // target rests, in current widths.
            let top = esc.terrain.height as f64 * esc.terrain_scale();
            let rel = floor.and_then(|f| f.ground).map(|g| g - top);
            let v = paced(v, esc.terrain.target_lift as f64, d, rel);
            // A view width is 4 · 2^-zoom: a power of two and a mantissa.
            let x = 2.0 - esc.zoom_log2;
            let e = x.floor();
            let m = (x - e).exp2();
            let mut out = Vec::new();
            for (k, (path, s)) in [(ConfigPath::EscapeCenterRe, &esc.center_re), (ConfigPath::EscapeCenterIm, &esc.center_im)]
                .into_iter()
                .enumerate()
            {
                if v[k] != 0.0 {
                    if let Some(next) = FixedPoint::decimal_add_floatexp(s, v[k] * d * m, e as i64, esc.zoom_log2) {
                        out.push((path, ConfigValue::String(next)));
                    }
                }
            }
            let lift = floored(esc.terrain.target_lift as f64 + v[2] * d, d, rel);
            if lift != esc.terrain.target_lift as f64 {
                out.push((ConfigPath::EscapeTerrainTargetLift, (lift as f32).into()));
            }
            out
        }
        #[cfg(all(feature = "terrain", feature = "engine-sim"))]
        ViewKind::SimTerrain => {
            let t = &config.sim.terrain;
            let (gw, gh) = crate::sim::SimRenderer::grid_for(&config.sim, panel[0].max(1.0) as u32, panel[1].max(1.0) as u32);
            // In grid widths, the target's distance; the target in cells
            // over (n − 1).
            let d = t.cam_distance.max(1.0e-3) as f64;
            let w = gw.max(2) as f64;
            // The ground under the eye (cells) less half a field unit,
            // where the target rests, in grid widths.
            let rel = floor.and_then(|f| f.ground).map(|g| g / w - 0.5 * t.height as f64);
            let v = paced(v, t.target_lift as f64, d, rel);
            let x = t.target_x as f64 + v[0] * d * w / (gw.max(2) - 1) as f64;
            let y = t.target_y as f64 + v[1] * d * w / (gh.max(2) - 1) as f64;
            let lift = floored(t.target_lift as f64 + v[2] * d, d, rel);
            let mut out = Vec::new();
            if v[0] != 0.0 {
                out.push((ConfigPath::SimTerrainTargetX, (x as f32).into()));
            }
            if v[1] != 0.0 {
                out.push((ConfigPath::SimTerrainTargetY, (y as f32).into()));
            }
            if lift != t.target_lift as f64 {
                out.push((ConfigPath::SimTerrainTargetLift, (lift as f32).into()));
            }
            out
        }
        _ => Vec::new(),
    }
}

/// How far the eye is from a solid's surface: the solid's distance
/// estimate at `eye` (the world's), measured on the GPU.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clearance {
    pub eye: [f64; 3],
    pub distance: f64,
}

/// What a flight knows of the surface, and what it does with it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlyLimits {
    /// The last measured clearance; None before one lands, or where
    /// nothing measures one (the solid alone does).
    pub clearance: Option<Clearance>,
    /// Keep each step inside the clearance.
    pub collisions: bool,
    /// Size the step by the clearance, and the zoom with it.
    pub proximity: bool,
    /// The ground's height under the eye a terrain's probe measured, in
    /// its world's units: the floor collisions keep the eye above, and
    /// the height proximity speed paces by. None before one lands, or
    /// where no ground lies under the eye.
    pub ground: Option<f64>,
}

/// The share of the clearance the eye may move from where it was measured
/// before the next measure: half, so a step that lands keeps a margin.
pub const CLEARANCE_FRACTION: f64 = 0.5;

/// Under proximity speed, how far the zoom moves toward the clearance a
/// step, in log2: a quarter of the way, so it eases rather than jumps.
pub const ZOOM_FOLLOW: f64 = 0.25;

/// Fly the 3D camera the viewport shows: `thrust` along its axes, `step`
/// far -- `fly_move_speed · dt`, in world units for a flame and in
/// distances to the target for the others (to the surface, under
/// proximity speed) -- within `limits`.
pub fn fly(config: &FractalConfig, thrust: Thrust, step: f64, panel: [f32; 2], limits: FlyLimits) -> Option<CameraEdit> {
    let kind = view_kind(config);
    let (conv, a, _) = flyer(config, kind)?;
    let f = conv.frame(a);
    let up = if thrust.world_up { [0.0, 0.0, 1.0] } else { f.up };
    let v: [f64; 3] = std::array::from_fn(|k| (thrust.right * f.right[k] + thrust.up * up[k] + thrust.forward * f.forward[k]) * step);
    if v == [0.0; 3] || !v.iter().all(|c| c.is_finite()) {
        return None;
    }
    #[cfg(not(feature = "engine-escape"))]
    let _ = limits;
    #[cfg(feature = "engine-escape")]
    if kind == ViewKind::Solid && (limits.collisions || limits.proximity) {
        if let Some(c) = limits.clearance {
            let changes = solid_flight(config, v, f.forward, c, limits, panel);
            return (!changes.is_empty()).then(|| CameraEdit::batch(changes, FLY));
        }
    }
    let changes = match kind {
        ViewKind::Flame3d => vec![
            (ConfigPath::CameraX, ((config.camera_x as f64 + v[0]) as f32).into()),
            (ConfigPath::CameraY, ((config.camera_y as f64 + v[1]) as f32).into()),
            (ConfigPath::CameraZ, ((config.camera_z as f64 + v[2]) as f32).into()),
        ],
        _ => pivot_moved(
            config,
            kind,
            v,
            panel,
            Some(Floor { forward: f.forward, ground: limits.ground, collide: limits.collisions, proximity: limits.proximity }),
        ),
    };
    (!changes.is_empty()).then(|| CameraEdit::batch(changes, FLY))
}

/// A solid's flight within a measured clearance (P6): `v` in distances to
/// the target, as every flight is, made a share of the clearance under
/// proximity speed, then kept inside it under collisions; and under
/// proximity the zoom eased toward the clearance, the eye held.
#[cfg(feature = "engine-escape")]
fn solid_flight(config: &FractalConfig, v: [f64; 3], forward: [f64; 3], c: Clearance, limits: FlyLimits, panel: [f32; 2]) -> Vec<(ConfigPath, ConfigValue)> {
    let esc = &config.escape;
    let registry = crate::variations::global_registry();
    let Some(ifs3) = crate::escape::ifs::solid_analysis(config, &registry) else {
        return Vec::new();
    };
    drop(registry);
    let (m, e) = crate::escape::ifs::solid_distance(esc, &ifs3);
    let d_target = m * (e as f64).exp2();
    let eye = crate::escape::ifs::solid_camera(esc, &ifs3).eye;
    let distance = c.distance.max(0.0);
    // The step in the world's units: of the target's distance, or of the
    // surface's.
    let scale = if limits.proximity { distance } else { d_target };
    let mut w: [f64; 3] = std::array::from_fn(|k| v[k] * scale);
    if limits.collisions {
        let gone = (0..3).map(|k| (eye[k] - c.eye[k]).powi(2)).sum::<f64>().sqrt();
        // What is left, rounding's residue (a millionth of it) none.
        let room = CLEARANCE_FRACTION * distance - gone;
        let room = if room > 1.0e-6 * distance { room } else { 0.0 };
        let len = w.iter().map(|x| x * x).sum::<f64>().sqrt();
        if len > room {
            w = w.map(|x| x * room / len);
        }
    }
    // The zoom toward the clearance, the eye held: the target slides along
    // the view by what the distance to it changes.
    let mut shift = 0.0;
    let mut zoom = None;
    if limits.proximity && distance > 0.0 && d_target > 0.0 {
        let d_new = d_target * ((distance / d_target).ln() * ZOOM_FOLLOW).exp();
        shift = d_new - d_target;
        zoom = Some(esc.zoom_log2 - (d_new / d_target).log2());
    }
    let moved: [f64; 3] = std::array::from_fn(|k| (w[k] + forward[k] * shift) / d_target);
    let mut out = if moved.iter().any(|x| *x != 0.0) { pivot_moved(config, ViewKind::Solid, moved, panel, None) } else { Vec::new() };
    if let Some(z) = zoom.filter(|z| z.is_finite() && *z != esc.zoom_log2) {
        out.push((ConfigPath::EscapeZoomLog2, (z as f32).into()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::manager::{ConfigManager, EditingTarget};

    fn applied(c: &FractalConfig, e: &CameraEdit) -> FractalConfig {
        let mut out = c.clone();
        for (p, v) in &e.changes {
            ConfigManager::apply_value_detached(&mut out, EditingTarget::Main, p, v.clone()).expect("applies");
        }
        out
    }

    const PANEL: [f32; 2] = [800.0, 600.0];

    fn settings(free_look: bool) -> LookSettings {
        LookSettings { radians_per_pixel: 0.004, invert_y: false, free_look }
    }

    /// A solid's flight within a measured clearance (P6): with collisions
    /// the eye never leaves half the clearance from where it was measured
    /// -- a long step is cut to it, a step from an eye already that far is
    /// nothing -- and without them it goes as before; with proximity the
    /// step is a share of the clearance and the zoom eases toward it, the
    /// eye held. Other cameras ignore a clearance.
    #[cfg(feature = "engine-escape")]
    #[test]
    fn a_solid_flies_within_its_clearance() {
        use crate::scene::transforms::RenderMode;
        let mut c = FractalConfig::default();
        c.render_mode = RenderMode::Escape;
        c.escape.formula = "quaternion_julia_solid".to_string();
        c.escape.cam_pitch = 0.3;
        c.escape.cam_yaw = 0.5;
        let eye0 = eye_of(&c);
        let ahead = Thrust { forward: 1.0, ..Default::default() };
        let moved = |c: &FractalConfig, e: &CameraEdit| dist(eye_of(&applied(c, e)), eye0);
        let free = fly(&c, ahead, 0.5, PANEL, FlyLimits::default()).expect("a flight");
        let d_target = moved(&c, &free) / 0.5;
        // A wall 0.1 target-distances ahead: the eye stops at half of it.
        let near = Clearance { eye: eye0, distance: 0.1 * d_target };
        let limits = FlyLimits { clearance: Some(near), collisions: true, proximity: false, ground: None };
        let cut = fly(&c, ahead, 0.5, PANEL, limits).expect("a flight");
        assert!((moved(&c, &cut) - 0.05 * d_target).abs() < 1.0e-6 * d_target, "cut to half the clearance: {}", moved(&c, &cut) / d_target);
        // From an eye already half the clearance on: nowhere.
        let far = Clearance { eye: std::array::from_fn(|k| eye0[k] - 0.05 * d_target * frame_of(&c).forward[k]), distance: 0.1 * d_target };
        assert!(fly(&c, ahead, 0.5, PANEL, FlyLimits { clearance: Some(far), ..limits }).is_none(), "no room left");
        // A short step inside the room is the step.
        let short = fly(&c, ahead, 0.01, PANEL, limits).expect("a flight");
        assert!((moved(&c, &short) - 0.01 * d_target).abs() < 1.0e-6 * d_target);
        // Proximity: the step a share of the clearance, the zoom eased
        // toward it, the eye where the step put it.
        let prox = FlyLimits { clearance: Some(near), collisions: false, proximity: true, ground: None };
        let e = fly(&c, ahead, 0.2, PANEL, prox).expect("a flight");
        let after = applied(&c, &e);
        assert!((moved(&c, &e) - 0.2 * near.distance).abs() < 1.0e-6 * d_target, "{}", moved(&c, &e) / near.distance);
        let want_zoom = c.escape.zoom_log2 - ZOOM_FOLLOW * (near.distance / d_target).log2();
        assert!((after.escape.zoom_log2 - want_zoom).abs() < 1.0e-4, "{} against {want_zoom}", after.escape.zoom_log2);
        assert!(after.escape.zoom_log2 > c.escape.zoom_log2, "nearer the surface, zoomed in");
        // A flame ignores a clearance.
        let mut flame = FractalConfig::default();
        flame.render_mode = RenderMode::ThreeD;
        flame.perspective_strength = 0.25;
        let a = fly(&flame, ahead, 0.1, PANEL, FlyLimits::default()).expect("a flight");
        let b = fly(&flame, ahead, 0.1, PANEL, limits).expect("a flight");
        assert_eq!(a.changes, b.changes);
    }

    /// A terrain's flight over a measured ground (P6): with collisions a
    /// dive stops the floor's height above the ground under the eye --
    /// into a valley below the relief's top, where the orbit's floor held
    /// it -- and with no ground measured, or without collisions, at the
    /// orbit's floor; a simulation terrain's ground above where its target
    /// rests raises the floor; under proximity speed the step is in heights
    /// of the eye above the ground.
    #[cfg(feature = "terrain")]
    #[test]
    fn a_terrain_flies_above_its_ground() {
        use crate::scene::transforms::RenderMode;
        let mut c = FractalConfig::default();
        c.render_mode = RenderMode::Escape;
        c.escape.terrain.enabled = true;
        c.escape.cam_pitch = 0.5;
        let eye_z = |c: &FractalConfig| crate::escape::footprint::terrain_camera(&c.escape).eye[2];
        let top = c.escape.terrain.height as f64 * c.escape.terrain_scale();
        let d = crate::escape::footprint::FRAME_DISTANCE;
        let margin = d * FLOOR_PITCH.sin();
        let down = Thrust { up: -1.0, world_up: true, ..Default::default() };
        let dive = |c: &FractalConfig, limits: FlyLimits| eye_z(&applied(c, &fly(c, down, 50.0, PANEL, limits).expect("a flight")));
        let valley = top - 0.3 * d;
        let unmeasured = dive(&c, FlyLimits { collisions: true, ..Default::default() });
        let low = dive(&c, FlyLimits { collisions: true, ground: Some(valley), ..Default::default() });
        let free = dive(&c, FlyLimits { ground: Some(valley), ..Default::default() });
        println!("  escape terrain: the top {top}, the dive stops at {unmeasured} unmeasured, {low} over a valley at {valley}, {free} without collisions");
        assert!((unmeasured - (top + margin)).abs() < 1.0e-6 * d, "unmeasured: the orbit's floor");
        assert!((low - (valley + margin)).abs() < 1.0e-6 * d, "over the valley: down to it");
        assert!((free - unmeasured).abs() < 1.0e-9 * d, "without collisions: the orbit's floor");
        // Proximity: the eye's move a share of its height above the ground.
        let moved = |e: &CameraEdit| {
            let a = applied(&c, e);
            let width = 4.0 * (-c.escape.zoom_log2 as f64).exp2();
            let num = |s: &str| s.trim().parse::<f64>().unwrap_or(0.0);
            let dx = (num(&a.escape.center_re) - num(&c.escape.center_re)) / width;
            let dy = (num(&a.escape.center_im) - num(&c.escape.center_im)) / width;
            (dx * dx + dy * dy + (eye_z(&a) - eye_z(&c)).powi(2)).sqrt()
        };
        let ahead = Thrust { forward: 1.0, ..Default::default() };
        let plain = moved(&fly(&c, ahead, 0.01, PANEL, FlyLimits { ground: Some(valley), ..Default::default() }).expect("a flight"));
        let paced = moved(&fly(&c, ahead, 0.01, PANEL, FlyLimits { proximity: true, ground: Some(valley), ..Default::default() }).expect("a flight"));
        let height = (eye_z(&c) - valley) / d;
        println!("  proximity: {paced} against {plain}, the eye {height} distances above the ground");
        assert!((paced / plain - height).abs() < 1.0e-4 * height, "paced by the height above the ground");
        // A simulation terrain: ground above where its target rests.
        let mut s = FractalConfig::default();
        s.render_mode = RenderMode::Simulation;
        s.sim.terrain.enabled = true;
        s.sim.terrain.cam_pitch = 0.5;
        let (gw, gh) = crate::sim::SimRenderer::grid_for(&s.sim, PANEL[0] as u32, PANEL[1] as u32);
        let w = gw.max(2) as f64;
        let sim_eye_z = |s: &FractalConfig| crate::sim::terrain::sim_terrain_camera(s, gw, gh).eye[2];
        let rest = 0.5 * s.sim.terrain.height as f64 * w;
        let hill = rest + 0.6 * s.sim.terrain.cam_distance as f64 * w;
        let sd = s.sim.terrain.cam_distance as f64 * w;
        let stood = sim_eye_z(&applied(&s, &fly(&s, down, 50.0, PANEL, FlyLimits { collisions: true, ground: Some(hill), ..Default::default() }).expect("a flight")));
        println!("  simulation terrain: rests at {rest}, a hill at {hill}, the dive stops at {stood}");
        assert!((stood - (hill + sd * FLOOR_PITCH.sin())).abs() < 1.0e-6 * sd, "the hill raised the floor");
    }

    /// The four 3D cameras, each turned off its home pose.
    fn cameras() -> Vec<(&'static str, FractalConfig)> {
        use crate::scene::transforms::RenderMode;
        let mut v = Vec::new();
        let mut flame = FractalConfig::default();
        flame.render_mode = RenderMode::ThreeD;
        flame.camera_rotation_x = 0.9;
        flame.camera_rotation_y = 0.4;
        flame.camera_bank = 0.1;
        flame.rotation = 0.3;
        flame.perspective_strength = 0.25;
        flame.camera_x = 0.2;
        v.push(("flame", flame));
        #[cfg(feature = "engine-escape")]
        {
            let mut solid = FractalConfig::default();
            solid.render_mode = RenderMode::Escape;
            solid.escape.formula = "quaternion_julia_solid".to_string();
            solid.escape.cam_pitch = 0.4;
            solid.escape.cam_yaw = 0.7;
            solid.escape.rotation = 0.2;
            v.push(("solid", solid));
        }
        #[cfg(feature = "terrain")]
        {
            let mut t = FractalConfig::default();
            t.render_mode = RenderMode::Escape;
            t.escape.terrain.enabled = true;
            t.escape.cam_pitch = 0.5;
            t.escape.cam_yaw = 0.3;
            t.escape.terrain.target_lift = 0.5;
            v.push(("escape terrain", t));
        }
        #[cfg(all(feature = "terrain", feature = "engine-sim"))]
        {
            let mut s = FractalConfig::default();
            s.render_mode = RenderMode::Simulation;
            s.sim.terrain.enabled = true;
            s.sim.terrain.cam_pitch = 0.5;
            s.sim.terrain.cam_yaw = -0.4;
            s.sim.terrain.target_lift = 0.5;
            v.push(("sim terrain", s));
        }
        v
    }

    fn frame_of(c: &FractalConfig) -> Frame {
        let (conv, a, _) = flyer(c, view_kind(c)).expect("a 3D camera");
        conv.frame(a)
    }

    /// The eye in the camera's world, in its own units: a flame's
    /// viewpoint, a target camera's eye (f64 is enough off deep zoom).
    fn eye_of(c: &FractalConfig) -> [f64; 3] {
        match view_kind(c) {
            ViewKind::Flame3d => {
                let f = frame_of(c);
                let p = [c.camera_x as f64, c.camera_y as f64, c.camera_z as f64];
                std::array::from_fn(|k| p[k] - f.forward[k] / c.perspective_strength as f64)
            }
            #[cfg(feature = "engine-escape")]
            ViewKind::Solid => {
                let registry = crate::variations::global_registry();
                let ifs3 = crate::escape::ifs::solid_analysis(c, &registry).expect("a solid");
                crate::escape::ifs::solid_camera(&c.escape, &ifs3).eye
            }
            #[cfg(feature = "terrain")]
            ViewKind::EscapeTerrain => {
                let cam = crate::escape::footprint::terrain_camera(&c.escape);
                let unit = 4.0 * (-c.escape.zoom_log2).exp2();
                let (cx, cy) = c.escape.center_f64();
                [cx + cam.eye_rel[0] * unit, cy + cam.eye_rel[1] * unit, cam.eye[2] * unit]
            }
            #[cfg(all(feature = "terrain", feature = "engine-sim"))]
            ViewKind::SimTerrain => {
                let (gw, gh) = crate::sim::SimRenderer::grid_for(&c.sim, PANEL[0] as u32, PANEL[1] as u32);
                crate::sim::terrain::sim_terrain_camera(c, gw, gh).eye
            }
            _ => unreachable!(),
        }
    }

    fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
        (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt()
    }

    /// A look turns the view toward the drag -- right for a drag right,
    /// down for a drag down -- and leaves the eye where it was, in both
    /// look modes, for every 3D camera.
    #[test]
    fn a_look_turns_about_the_eye() {
        for (name, c) in cameras() {
            for free_look in [true, false] {
                let f0 = frame_of(&c);
                let eye0 = eye_of(&c);
                let scale = dist(eye0, [0.0; 3]).max(1.0);
                let right = applied(&c, &look(&c, [30.0, 0.0], settings(free_look), PANEL).expect("a look"));
                let f1 = frame_of(&right);
                assert!(dot(f1.forward, f0.right) > 0.05, "{name} {free_look}: a drag right looks right");
                assert!(dist(eye_of(&right), eye0) < 1e-4 * scale, "{name} {free_look}: the eye stays {:?} {:?}", eye_of(&right), eye0);
                let down = applied(&c, &look(&c, [0.0, 30.0], settings(free_look), PANEL).expect("a look"));
                let f2 = frame_of(&down);
                assert!(dot(f2.forward, f0.up) < -0.05, "{name} {free_look}: a drag down looks down");
                assert!(dist(eye_of(&down), eye0) < 1e-4 * scale, "{name} {free_look}: the eye stays");
                // The roll is held: the angles that move are pitch, yaw, bank.
                assert_eq!(down.rotation, c.rotation, "{name}");
                assert_eq!(down.escape.rotation, c.escape.rotation, "{name}");
            }
        }
    }

    /// FPS keeps the horizon level: from a level camera -- no bank, no
    /// screen roll -- any look leaves the screen's right horizontal.
    /// FreeLook's circles can roll it.
    #[test]
    fn fps_looks_keep_the_horizon_level() {
        for (name, c) in cameras() {
            let mut a = c.clone();
            a.camera_bank = 0.0;
            a.rotation = 0.0;
            a.escape.rotation = 0.0;
            for drag in [[40.0, 25.0], [-60.0, 10.0], [15.0, -45.0], [80.0, 80.0]] {
                a = applied(&a, &look(&a, drag, settings(false), PANEL).expect("a look"));
                let f = frame_of(&a);
                assert!(f.right[2].abs() < 1e-5, "{name}: the horizon tilted {:?}", f.right);
            }
        }
    }

    /// A long free-look path is faithful: the angles written each step
    /// rebuild the orientation composed so far, through looking straight
    /// up and over -- no gimbal lock, no flip.
    #[test]
    fn a_free_look_path_is_continuous_and_faithful() {
        for (name, c) in cameras() {
            let (conv, a0, _) = flyer(&c, view_kind(&c)).unwrap();
            let mut q = conv.orientation(a0);
            let mut cur = c.clone();
            for i in 0..400 {
                let t = i as f64 * 0.05;
                let drag = [(12.0 * t.cos()) as f32, (9.0 + 4.0 * (1.3 * t).sin()) as f32];
                let f = conv.frame_of(&q);
                let r = free_turn(&f, [drag[0] as f64, drag[1] as f64], settings(true)).unwrap();
                q = q * r.conjugate();
                cur = applied(&cur, &look(&cur, drag, settings(true), PANEL).expect("a look"));
                let (conv2, a, _) = flyer(&cur, view_kind(&cur)).unwrap();
                let got = conv2.frame(a);
                let want = conv.frame_of(&q);
                let err = dist(got.forward, want.forward).max(dist(got.up, want.up));
                assert!(err < 2e-3, "{name} step {i}: {err}");
                // The stored values cannot pass the conventions' range.
                if name != "flame" {
                    assert!(a.pitch.abs() <= FRAC_PI_2 + 1e-6, "{name} step {i}: pitch {}", a.pitch);
                }
                // Re-sync, as the written angles are f32.
                q = conv2.orientation(a);
            }
        }
    }

    /// W flies along the view, D to the right, E up; a flame's in world
    /// units, a target camera's in distances to the target. A terrain's
    /// eye stays above the ground.
    #[test]
    fn a_flight_goes_where_the_camera_points() {
        for (name, c) in cameras() {
            let f = frame_of(&c);
            let eye0 = eye_of(&c);
            for (thrust, axis) in [
                (Thrust { forward: 1.0, ..Default::default() }, f.forward),
                (Thrust { right: 1.0, ..Default::default() }, f.right),
                (Thrust { up: 1.0, ..Default::default() }, f.up),
                (Thrust { up: 1.0, world_up: true, ..Default::default() }, [0.0, 0.0, 1.0]),
            ] {
                let after = applied(&c, &fly(&c, thrust, 0.1, PANEL, FlyLimits::default()).expect("a flight"));
                let moved: [f64; 3] = std::array::from_fn(|k| eye_of(&after)[k] - eye0[k]);
                let along = dot(moved, axis);
                let len = dist(moved, [0.0; 3]);
                assert!(len > 0.0 && along > 0.999 * len, "{name}: {moved:?} along {axis:?}");
            }
        }
    }

    /// A terrain's flight down stops where the orbit's floor is.
    #[cfg(feature = "terrain")]
    #[test]
    fn a_terrain_flight_keeps_the_eye_above_the_ground() {
        let (_, c) = cameras().into_iter().find(|(n, _)| *n == "escape terrain").unwrap();
        let mut rest = c.clone();
        rest.escape.terrain.target_lift = 0.0;
        rest.escape.cam_pitch = 0.02;
        let floor = crate::escape::footprint::terrain_camera(&rest.escape).eye[2];
        let down = Thrust { up: -1.0, world_up: true, ..Default::default() };
        let after = applied(&c, &fly(&c, down, 50.0, PANEL, FlyLimits::default()).expect("a flight"));
        let eye = crate::escape::footprint::terrain_camera(&after.escape).eye[2];
        assert!((eye - floor).abs() < 1e-5, "{eye} vs the floor {floor}");
    }

    /// Mode D flies at a depth f64 cannot step: the target is a decimal,
    /// and a flight adds to it in fixed point.
    #[cfg(feature = "engine-escape")]
    #[test]
    fn a_deep_solid_flies() {
        let (_, mut c) = cameras().into_iter().find(|(n, _)| *n == "solid").unwrap();
        c.escape.cam_target_x = "0.3".to_string();
        c.escape.cam_target_y = "0.1".to_string();
        c.escape.cam_target_z = "-0.2".to_string();
        c.escape.zoom_log2 = 200.0;
        let after = applied(&c, &fly(&c, Thrust { forward: 1.0, ..Default::default() }, 1.0, PANEL, FlyLimits::default()).expect("a flight"));
        let moved = [&after.escape.cam_target_x, &after.escape.cam_target_y, &after.escape.cam_target_z];
        let before = [&c.escape.cam_target_x, &c.escape.cam_target_y, &c.escape.cam_target_z];
        assert!(moved.iter().zip(before).any(|(a, b)| *a != b), "the target moved: {moved:?}");
        for (a, b) in moved.iter().zip(before) {
            let (a, b): (f64, f64) = (a.parse().unwrap(), b.parse().unwrap());
            assert!((a - b).abs() < 1e-50, "a step of one distance at 2^-200: {a} vs {b}");
        }
    }

    /// The canonical angles rebuild the same orientation.
    #[test]
    fn canonical_angles_are_the_same_camera() {
        let conv = Convention::Solid { yaw_offset: 0.3 };
        for pitch in [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0] {
            let a = Angles { pitch, yaw: 0.7, bank: 0.2, roll: 0.1 };
            let b = canonical(conv, a);
            assert!(b.pitch.abs() <= FRAC_PI_2 + 1e-12, "{b:?}");
            let (fa, fb) = (conv.frame(a), conv.frame(b));
            assert!(dist(fa.forward, fb.forward) < 1e-9 && dist(fa.up, fb.up) < 1e-9, "{pitch}");
        }
    }
}

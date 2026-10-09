//! The viewport's camera gestures, as config edits.
//!
//! Each takes the config and the gesture's measure in pixels and returns
//! the [`CameraEdit`] it makes, or `None` for nothing to do. Pure: the
//! viewport applies the edit (`CameraEdit::apply`), so a gesture can be
//! tested without a window, and every input that moves a camera -- the
//! viewport, the tab strip, the keys, the menus -- moves it the same way.
//!
//! The gestures are the plan's (camera-unification C3), the same in every
//! mode -- what each moves is the camera's own:
//!
//! | | 2D | 3D |
//! |---|---|---|
//! | drag | [`pan`] | [`pan`]: in the screen plane; a terrain's along the ground |
//! | Alt+drag, right drag | [`turn`]: rotate the view about the centre | [`turn`]: orbit the target, a turntable |
//! | wheel | [`zoom`] toward the cursor, in and out | [`zoom`]: dolly toward the cursor (a flame: its 2D zoom) |
//! | pinch | [`pinch`]: zoom, pan, twist to rotate | [`pinch`]: dolly, pan, twist to orbit |
//!
//! Every gesture of a kind goes under one history name ([`PAN`], [`ZOOM`],
//! [`ROTATE`], [`ORBIT`]), which the ConfigManager coalesces as one
//! gesture (`GESTURE_HISTORY_DESCS`).

use super::view3d::{self, Angles, Convention};
use super::{quat::Quat, CameraEdit};
use crate::config::{ConfigPath, ConfigValue, FractalConfig};

/// A pan's history entry.
pub const PAN: &str = "history.action.pan_view";
/// A zoom's, a dolly's or a pinch's.
pub const ZOOM: &str = "history.action.zoom_view";
/// A 2D view's rotation.
pub const ROTATE: &str = "history.action.rotate_view";
/// A 3D camera's orbit.
pub const ORBIT: &str = "history.action.orbit_camera";
/// A Reset View.
pub const RESET: &str = "history.action.reset_view";

/// Which camera the viewport shows, and so what a gesture moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    /// The flame's 2D view: pan, zoom, rotation.
    Flame2d,
    /// The flame's 3D camera: its angles and position, and the 2D pan and
    /// zoom after its projection.
    Flame3d,
    /// The escape plane: its centre (exact decimals), zoom and rotation.
    EscapePlane,
    /// Mode D's solid camera: a target, a distance (the zoom), angles.
    Solid,
    /// The escape terrain: the plane's centre as its target on the
    /// ground, mode D's angles, the zoom as its scale.
    EscapeTerrain,
    /// A simulation's 2D picture. No view yet (camera-unification P2).
    Sim2d,
    /// A simulation terrain's camera.
    SimTerrain,
}

impl ViewKind {
    /// Whether it is a 3D camera.
    pub fn is_3d(self) -> bool {
        matches!(self, ViewKind::Flame3d | ViewKind::Solid | ViewKind::EscapeTerrain | ViewKind::SimTerrain)
    }
}

/// The camera a config's picture is seen through.
pub fn view_kind(config: &FractalConfig) -> ViewKind {
    use crate::scene::transforms::RenderMode;
    match config.render_mode {
        RenderMode::TwoD => ViewKind::Flame2d,
        RenderMode::ThreeD => ViewKind::Flame3d,
        RenderMode::Escape => {
            if config.escape.terrain_active() {
                return ViewKind::EscapeTerrain;
            }
            #[cfg(feature = "engine-escape")]
            if crate::escape::ifs::formula_is_solid(&config.escape.formula) {
                return ViewKind::Solid;
            }
            ViewKind::EscapePlane
        }
        RenderMode::Simulation => {
            #[cfg(feature = "terrain")]
            if config.sim.terrain_active() {
                return ViewKind::SimTerrain;
            }
            ViewKind::Sim2d
        }
    }
}

/// The zoom a wheel's scroll asks for: a notch of about 15%.
pub fn wheel_factor(scroll: f32) -> f64 {
    if scroll.abs() > 0.1 {
        f64::from(1.1f32).powf(f64::from(scroll) * 0.03)
    } else {
        1.0
    }
}

/// What a turn reads from the settings: radians a pixel, and whether a
/// vertical drag is reversed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurnSettings {
    pub radians_per_pixel: f64,
    pub invert_y: bool,
}

impl TurnSettings {
    /// The fly mode's settings, which every turn shares.
    pub fn from_system(settings: &crate::storage::SystemSettings) -> Self {
        TurnSettings { radians_per_pixel: settings.fly_mouse_sensitivity as f64, invert_y: settings.fly_invert_y }
    }
}

/// Pan the view by a drag of `drag` pixels on a panel `panel` in size:
/// the picture follows the drag.
pub fn pan(config: &FractalConfig, drag: [f32; 2], panel: [f32; 2]) -> Option<CameraEdit> {
    match view_kind(config) {
        ViewKind::Flame2d | ViewKind::Flame3d => flame_pan(config, drag, panel),
        #[cfg(feature = "engine-escape")]
        ViewKind::EscapePlane | ViewKind::Solid | ViewKind::EscapeTerrain => escape_pan(config, drag, panel),
        #[cfg(all(feature = "terrain", feature = "engine-sim"))]
        ViewKind::SimTerrain => sim_terrain_pan(config, drag, panel),
        _ => None,
    }
}

/// Zoom by `factor` (above one: closer) toward `cursor`, its offset from
/// the panel's centre in pixels -- the point under it stays under it,
/// in and out. A 2D view zooms; a solid dollies toward the point; a
/// terrain dollies toward its target, the screen's centre (the point
/// under the cursor is somewhere on the ground, at a depth an anchor on
/// the plane knows nothing of); a flame's 3D camera zooms its 2D picture.
pub fn zoom(config: &FractalConfig, factor: f64, cursor: Option<[f32; 2]>, panel: [f32; 2]) -> Option<CameraEdit> {
    if !(factor > 0.0) || factor == 1.0 {
        return None;
    }
    match view_kind(config) {
        ViewKind::Flame2d | ViewKind::Flame3d => flame_zoom(config, factor, cursor, panel),
        #[cfg(feature = "engine-escape")]
        ViewKind::EscapePlane | ViewKind::Solid => escape_zoom(config, factor, cursor, panel),
        #[cfg(feature = "engine-escape")]
        ViewKind::EscapeTerrain => escape_zoom(config, factor, None, panel),
        #[cfg(all(feature = "terrain", feature = "engine-sim"))]
        ViewKind::SimTerrain => sim_terrain_dolly(config, factor),
        _ => None,
    }
}

/// Turn the view by a drag of `drag` pixels: a 2D view rotates about the
/// screen's centre by the angle the pointer swept round it (`pointer` is
/// where it ended, as an offset from the centre); a 3D camera orbits its
/// target.
pub fn turn(config: &FractalConfig, drag: [f32; 2], pointer: Option<[f32; 2]>, settings: TurnSettings) -> Option<CameraEdit> {
    let kind = view_kind(config);
    if kind.is_3d() {
        let s = settings.radians_per_pixel;
        let dy = if settings.invert_y { -drag[1] } else { drag[1] };
        return orbit(config, kind, f64::from(drag[0]) * s, f64::from(dy) * s);
    }
    let [ax, ay] = pointer?;
    let (bx, by) = (ax - drag[0], ay - drag[1]);
    // Too near the centre, the angle is noise.
    if (ax * ax + ay * ay).sqrt() < 4.0 || (bx * bx + by * by).sqrt() < 4.0 {
        return None;
    }
    let swept = f64::from(bx * ay - by * ax).atan2(f64::from(bx * ax + by * ay));
    rotate_2d(config, kind, swept)
}

/// Rotate a 2D view by `swept`, the angle a point under the pointer turned
/// through on screen (positive clockwise, the screen's y running down):
/// the picture turns with it.
fn rotate_2d(config: &FractalConfig, kind: ViewKind, swept: f64) -> Option<CameraEdit> {
    match kind {
        ViewKind::Flame2d => Some(CameraEdit::batch(
            vec![(ConfigPath::Rotation, (wrap_pi(config.rotation as f64 + swept) as f32).into())],
            ROTATE,
        )),
        ViewKind::EscapePlane => Some(CameraEdit::batch(
            vec![(ConfigPath::EscapeRotation, (wrap_pi(config.escape.rotation as f64 + swept) as f32).into())],
            ROTATE,
        )),
        _ => None,
    }
}

/// An angle in [−π, π].
fn wrap_pi(a: f64) -> f64 {
    use std::f64::consts::TAU;
    a - TAU * (a / TAU).round()
}

/// What a 3D camera's orbit writes: its convention and stored angles,
/// the paths of its pitch, yaw and bank, and the range its pitch keeps
/// (a terrain's eye stays above the ground).
struct Turnable {
    conv: Convention,
    angles: Angles,
    paths: [ConfigPath; 3],
    pitch_range: Option<(f64, f64)>,
}

fn turnable(config: &FractalConfig, kind: ViewKind) -> Option<Turnable> {
    #[cfg(feature = "terrain")]
    const ABOVE_GROUND: Option<(f64, f64)> = Some((0.02, std::f64::consts::FRAC_PI_2 - 0.001));
    match kind {
        ViewKind::Flame3d => {
            let (conv, angles) = view3d::flame_angles(config);
            Some(Turnable {
                conv,
                angles,
                paths: [ConfigPath::CameraRotationX, ConfigPath::CameraRotationY, ConfigPath::CameraBank],
                pitch_range: None,
            })
        }
        #[cfg(feature = "engine-escape")]
        ViewKind::Solid => {
            let (conv, angles) = view3d::solid_angles(&config.escape);
            Some(Turnable {
                conv,
                angles,
                paths: [ConfigPath::EscapeCamPitch, ConfigPath::EscapeCamYaw, ConfigPath::EscapeCamBank],
                pitch_range: None,
            })
        }
        #[cfg(feature = "terrain")]
        ViewKind::EscapeTerrain => {
            let (conv, angles) = view3d::escape_terrain_angles(&config.escape);
            Some(Turnable {
                conv,
                angles,
                paths: [ConfigPath::EscapeCamPitch, ConfigPath::EscapeCamYaw, ConfigPath::EscapeCamBank],
                pitch_range: ABOVE_GROUND,
            })
        }
        #[cfg(all(feature = "terrain", feature = "engine-sim"))]
        ViewKind::SimTerrain => {
            let (conv, angles) = view3d::sim_terrain_angles(&config.sim.terrain);
            Some(Turnable {
                conv,
                angles,
                paths: [ConfigPath::SimTerrainCamPitch, ConfigPath::SimTerrainCamYaw, ConfigPath::SimTerrainCamBank],
                pitch_range: ABOVE_GROUND,
            })
        }
        _ => None,
    }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalized(v: [f64; 3]) -> Option<[f64; 3]> {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (l > 1e-9).then(|| [v[0] / l, v[1] / l, v[2] / l])
}

/// Orbit a 3D camera about its target, a turntable: `across` radians
/// about world up (the drag across the screen) and `along` about the
/// level axis across the view (the drag down it), so the horizon stays
/// level and the bank stays what it was. The near side of the scene
/// follows the drag. The screen's roll turns the drag first, so a drag
/// is in the screen as it is drawn.
pub fn orbit(config: &FractalConfig, kind: ViewKind, across: f64, along: f64) -> Option<CameraEdit> {
    let t = turnable(config, kind)?;
    // In steps of a quarter radian at most: the way back to angles takes
    // the solution nearest the last, so one long turn could come back on
    // the far side of a pole (a fast flick is hundreds of pixels a frame).
    let n = ((across.abs().max(along.abs()) / 0.25).ceil() as usize).clamp(1, 64);
    let mut a = t.angles;
    for _ in 0..n {
        a = orbit_step(&t, a, across / n as f64, along / n as f64);
    }
    a.pitch = match t.pitch_range {
        Some(_) => a.pitch,
        None => wrap_pi(a.pitch),
    };
    a.yaw = wrap_pi(a.yaw);
    a.bank = wrap_pi(a.bank);
    Some(CameraEdit::batch(
        vec![
            (t.paths[0].clone(), (a.pitch as f32).into()),
            (t.paths[1].clone(), (a.yaw as f32).into()),
            (t.paths[2].clone(), (a.bank as f32).into()),
        ],
        ORBIT,
    ))
}

/// One step of [`orbit`] from the angles `from`, the pitch kept in range.
fn orbit_step(t: &Turnable, from: Angles, across: f64, along: f64) -> Angles {
    let q = t.conv.orientation(from);
    let f = t.conv.frame_of(&q);
    // The drag in the unrolled screen: the screen's right and down are
    // the frame's right and −up, and the level axis is where right
    // would be with no roll.
    let up_world = [0.0, 0.0, 1.0];
    let level = normalized(cross(f.forward, up_world))
        .or_else(|| normalized([f.right[0], f.right[1], 0.0]))
        .unwrap_or([1.0, 0.0, 0.0]);
    let level = if level[0] * f.right[0] + level[1] * f.right[1] + level[2] * f.right[2] < 0.0 {
        [-level[0], -level[1], -level[2]]
    } else {
        level
    };
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let down_screen = [-f.up[0], -f.up[1], -f.up[2]];
    // Down the unrolled screen: across the view from the level axis, in
    // the screen's own handedness (a solid's screen is mirrored).
    let dl = cross(level, f.forward);
    let down_level = if dot(cross(f.right, down_screen), f.forward) > 0.0 { [-dl[0], -dl[1], -dl[2]] } else { dl };
    // The drag, from the screen as drawn into the unrolled one.
    let (dx, dy) = (
        across * dot(f.right, level) + along * dot(down_screen, level),
        across * dot(f.right, down_level) + along * dot(down_screen, down_level),
    );
    // The camera turns by R (a world rotation about the target), so its
    // world → camera rows become M · Rᵀ. A mirrored screen (the flame's:
    // right × up is forward) turns the other way for the same look.
    let hand = if dot(cross(f.right, f.up), f.forward) > 0.0 { 1.0 } else { -1.0 };
    let r = Quat::from_axis_angle(up_world, hand * dx) * Quat::from_axis_angle(level, hand * dy);
    let mut a = t.conv.angles_near(&(q * r.conjugate()), from);
    if let Some((lo, hi)) = t.pitch_range {
        a.pitch = a.pitch.clamp(lo, hi);
    }
    a
}

/// A two-finger gesture: zoom by `factor` about `midpoint` (its offset
/// from the centre), the midpoint's `translation` as a pan, and `twist`
/// radians (screen, clockwise) as a rotation in 2D or an orbit about
/// world up in 3D -- one history entry.
pub fn pinch(
    config: &FractalConfig,
    factor: f64,
    translation: [f32; 2],
    twist: f64,
    midpoint: [f32; 2],
    panel: [f32; 2],
) -> Option<CameraEdit> {
    use crate::config::manager::{ConfigManager, EditingTarget};
    let kind = view_kind(config);
    let mut work = config.clone();
    let mut changes: Vec<(ConfigPath, ConfigValue)> = Vec::new();
    let mut take = |edit: Option<CameraEdit>, work: &mut FractalConfig| {
        for (path, value) in edit.map(|e| e.changes).unwrap_or_default() {
            let _ = ConfigManager::apply_value_detached(work, EditingTarget::Main, &path, value.clone());
            match changes.iter_mut().find(|(p, _)| *p == path) {
                Some(slot) => slot.1 = value,
                None => changes.push((path, value)),
            }
        }
    };
    if factor > 0.0 && factor != 1.0 {
        let e = zoom(&work, factor, Some(midpoint), panel);
        take(e, &mut work);
    }
    if translation != [0.0, 0.0] {
        let e = pan(&work, translation, panel);
        take(e, &mut work);
    }
    if twist != 0.0 && twist.is_finite() {
        let e = if kind.is_3d() { orbit(&work, kind, twist, 0.0) } else { rotate_2d(&work, kind, twist) };
        take(e, &mut work);
    }
    (!changes.is_empty()).then(|| CameraEdit::batch(changes, ZOOM))
}

/// The view as it starts: a flame's centred and unrotated, and its 3D
/// camera home; the escape plane's centre and zoom at the defaults; a
/// solid framed again; a terrain's camera at its defaults over the same
/// ground.
pub fn reset(config: &FractalConfig) -> Option<CameraEdit> {
    let mut changes: Vec<(ConfigPath, ConfigValue)> = Vec::new();
    match view_kind(config) {
        ViewKind::Flame2d | ViewKind::Flame3d => {
            changes.push((ConfigPath::Zoom, 1.0f32.into()));
            changes.push((ConfigPath::Pan, (0.0f64, 0.0f64).into()));
            changes.push((ConfigPath::Rotation, 0.0f32.into()));
            if config.render_mode == crate::scene::transforms::RenderMode::ThreeD {
                for p in [
                    ConfigPath::CameraRotationX,
                    ConfigPath::CameraRotationY,
                    ConfigPath::CameraBank,
                    ConfigPath::CameraX,
                    ConfigPath::CameraY,
                    ConfigPath::CameraZ,
                ] {
                    changes.push((p, 0.0f32.into()));
                }
            }
        }
        ViewKind::EscapePlane | ViewKind::Solid | ViewKind::EscapeTerrain => {
            let home = crate::config::escape::EscapeConfig::default();
            changes.push((ConfigPath::EscapeZoomLog2, ConfigValue::Float(0.0)));
            changes.push((ConfigPath::EscapeRotation, 0.0f32.into()));
            match view_kind(config) {
                ViewKind::EscapePlane => {
                    changes.push((ConfigPath::EscapeCenterRe, ConfigValue::String(home.center_re.clone())));
                    changes.push((ConfigPath::EscapeCenterIm, ConfigValue::String(home.center_im.clone())));
                }
                kind => {
                    if kind == ViewKind::Solid {
                        for p in [ConfigPath::EscapeCamTargetX, ConfigPath::EscapeCamTargetY, ConfigPath::EscapeCamTargetZ] {
                            changes.push((p, ConfigValue::String(String::new())));
                        }
                    }
                    changes.push((ConfigPath::EscapeCamPitch, home.cam_pitch.into()));
                    changes.push((ConfigPath::EscapeCamYaw, home.cam_yaw.into()));
                    changes.push((ConfigPath::EscapeCamBank, home.cam_bank.into()));
                }
            }
        }
        #[cfg(feature = "engine-sim")]
        ViewKind::SimTerrain => {
            let home = crate::config::sim::SimTerrainConfig::default();
            changes.push((ConfigPath::SimTerrainCamPitch, home.cam_pitch.into()));
            changes.push((ConfigPath::SimTerrainCamYaw, home.cam_yaw.into()));
            changes.push((ConfigPath::SimTerrainCamBank, home.cam_bank.into()));
            changes.push((ConfigPath::SimTerrainCamDistance, home.cam_distance.into()));
            changes.push((ConfigPath::SimTerrainTargetX, home.target_x.into()));
            changes.push((ConfigPath::SimTerrainTargetY, home.target_y.into()));
        }
        _ => return None,
    }
    Some(CameraEdit::batch(changes, RESET))
}

/// Pan the flame view by a drag of `drag` pixels on a panel `panel`
/// pixels in size. The smaller side scales both axes, so a drag moves as
/// fast in portrait as in landscape.
pub fn flame_pan(config: &FractalConfig, drag: [f32; 2], panel: [f32; 2]) -> Option<CameraEdit> {
    let ref_size = panel[0].min(panel[1]);
    let scale = 4.0 / (config.zoom * ref_size);
    let dx = -drag[0] * scale;
    let dy = -drag[1] * scale;
    let (fractal_dx, fractal_dy) = config.screen_delta_to_pan_frame(dx as f64, dy as f64);
    Some(CameraEdit::batch(vec![(ConfigPath::Pan, (config.pan_x + fractal_dx, config.pan_y + fractal_dy).into())], PAN))
}

/// Zoom the flame view by `factor` toward `cursor` (its offset from the
/// panel's centre, in pixels), in or out -- the point under the cursor
/// stays under it -- or about the centre without one.
pub fn flame_zoom(config: &FractalConfig, factor: f64, cursor: Option<[f32; 2]>, panel: [f32; 2]) -> Option<CameraEdit> {
    if !(factor > 0.0) || factor == 1.0 {
        return None;
    }
    let new_zoom = (config.zoom * factor as f32).clamp(0.01, config.max_view_zoom());
    let mut changes = vec![(ConfigPath::Zoom, new_zoom.into())];
    if let Some([ox, oy]) = cursor {
        let scale = panel[0].min(panel[1]) * 0.25;
        let (rx, ry) = config.screen_delta_to_pan_frame(ox as f64, oy as f64);
        let point_x = config.pan_x + rx / (scale * config.zoom) as f64;
        let point_y = config.pan_y + ry / (scale * config.zoom) as f64;
        let new_pan_x = point_x - rx / (scale * new_zoom) as f64;
        let new_pan_y = point_y - ry / (scale * new_zoom) as f64;
        changes.push((ConfigPath::Pan, (new_pan_x, new_pan_y).into()));
    }
    Some(CameraEdit::batch(changes, ZOOM))
}

#[cfg(feature = "engine-escape")]
mod escape {
    use super::*;
    use crate::config::escape::EscapeConfig;
    use crate::escape::fixedpoint::FixedPoint;

    /// Escape-mode complex-plane geometry shared by pan and zoom below.
    ///
    /// The escape shader maps the viewport as: vertical span `4 / 2^zoom`
    /// across `height` pixels (horizontal follows aspect with the SAME
    /// per-pixel scale), screen y flipped (Im grows up), then the view
    /// rotation. So one pixel is `span_y / height` complex units in every
    /// direction, and a screen offset becomes a world offset via y-flip +
    /// rotation. Done in f64 from the exact-decimal center strings -- the
    /// phase-1 precision ceiling (f64 formatting round-trips shortest, so
    /// writing back never loses what f64 held).
    pub fn screen_to_world(esc: &EscapeConfig, dx_px: f64, dy_px: f64, panel: [f32; 2]) -> (f64, f64) {
        let height = f64::from(panel[1].max(1.0));
        let per_pixel = (4.0 / esc.zoom_factor()) / height;
        let (dx, dy) = (dx_px * per_pixel, -dy_px * per_pixel);
        let (cos_r, sin_r) = (f64::from(esc.rotation).cos(), f64::from(esc.rotation).sin());
        (dx * cos_r - dy * sin_r, dx * sin_r + dy * cos_r)
    }

    /// Screen delta → world delta in SYMBOLIC form: rotated pixel
    /// offsets as f64 mantissas plus the pixel spacing's shared power-of-
    /// two exponent (S = 2^(2−zoom)/height = s_m·2^s_e). The f64 form
    /// underflows past ~zoom 1060; this form reaches any depth.
    fn pan_delta_symbolic(esc: &EscapeConfig, dx_px: f64, dy_px: f64, panel: [f32; 2]) -> (f64, f64, i64) {
        let height = f64::from(panel[1].max(1.0));
        let x = 2.0 - esc.zoom_log2 - height.log2();
        let s_e = x.floor();
        let s_m = 2f64.powf(x - s_e);
        let (dx, dy) = (dx_px, -dy_px);
        let (cos_r, sin_r) = (f64::from(esc.rotation).cos(), f64::from(esc.rotation).sin());
        ((dx * cos_r - dy * sin_r) * s_m, (dx * sin_r + dy * cos_r) * s_m, s_e as i64)
    }

    /// The solid a config renders, if it renders one: the analysis and
    /// the camera it is looked at through. `None` for the plane, and for
    /// a solid formula over a flame that does not qualify, which renders
    /// nothing there is to steer.
    pub(super) fn solid_view(
        config: &FractalConfig,
    ) -> Option<(crate::scene::ifs_analysis::Ifs3, crate::escape::ifs::SolidCamera)> {
        let registry = crate::variations::global_registry();
        let ifs3 = crate::escape::ifs::solid_analysis(config, &registry)?;
        let cam = crate::escape::ifs::solid_camera(&config.escape, &ifs3);
        Some((ifs3, cam))
    }

    /// The solid's target after a screen offset `(dx, dy)` in pixels is
    /// applied in the target's plane -- once per `(view, sign)` in
    /// `terms`, each at that view's zoom, so a pan is one term and a
    /// zoom-to-cursor is the difference of two.
    ///
    /// The offset becomes `dx · right − dy · up` (screen y grows
    /// downward) times the pixel's step at the target, which is the
    /// plane's symbolic pan delta again: a mantissa and a power of two,
    /// added to the decimal target in fixed point so the step survives
    /// any depth. An empty target is the attractor's own centre, and
    /// becomes explicit here -- the moment the camera moves.
    fn solid_target_shifted(
        digits_at: &EscapeConfig,
        ifs3: &crate::scene::ifs_analysis::Ifs3,
        cam: &crate::escape::ifs::SolidCamera,
        terms: &[(&EscapeConfig, f64)],
        dx_px: f64,
        dy_px: f64,
        height_px: f64,
    ) -> [String; 3] {
        let z = digits_at.zoom_log2;
        let mut out: [String; 3] = std::array::from_fn(|k| {
            let s = [&digits_at.cam_target_x, &digits_at.cam_target_y, &digits_at.cam_target_z][k];
            if s.trim().is_empty() {
                format!("{}", ifs3.ball.centre[k])
            } else {
                s.trim().to_string()
            }
        });
        for &(view, sign) in terms {
            let (m, e) = crate::escape::ifs::solid_pixel_step(view, ifs3, height_px);
            for k in 0..3 {
                let v = dx_px * cam.right[k] - dy_px * cam.up[k];
                if let Some(next) = FixedPoint::decimal_add_floatexp(&out[k], sign * v * m, e, z) {
                    out[k] = next;
                }
            }
        }
        out
    }

    fn solid_target_updates(target: [String; 3]) -> Vec<(ConfigPath, ConfigValue)> {
        let [x, y, z] = target;
        vec![
            (ConfigPath::EscapeCamTargetX, ConfigValue::String(x)),
            (ConfigPath::EscapeCamTargetY, ConfigValue::String(y)),
            (ConfigPath::EscapeCamTargetZ, ConfigValue::String(z)),
        ]
    }

    /// Pan the escape view by a drag of `drag` pixels: a terrain's
    /// target slides across the ground, a solid's across the screen
    /// plane at its depth, and the plane's centre moves opposite the
    /// drag, so the picture follows the cursor.
    pub fn escape_pan(config: &FractalConfig, drag: [f32; 2], panel: [f32; 2]) -> Option<CameraEdit> {
        #[cfg(feature = "terrain")]
        if config.escape.terrain_active() {
            return terrain_pan(config, drag, panel);
        }
        if let Some((ifs3, cam)) = solid_view(config) {
            let esc = &config.escape;
            let shifted = solid_target_shifted(
                esc,
                &ifs3,
                &cam,
                &[(esc, -1.0)],
                f64::from(drag[0]),
                f64::from(drag[1]),
                f64::from(panel[1]),
            );
            return Some(CameraEdit::batch(solid_target_updates(shifted), PAN));
        }
        Some(escape_pan_plane(&config.escape, drag, panel))
    }

    /// The plane's pan: the centre moves opposite a drag of `drag`
    /// pixels on a picture `panel` high.
    ///
    /// The center accumulates in FIXED-POINT with a SYMBOLIC delta
    /// (mantissa · 2^exponent): an f64 round-trip caps the step at the
    /// center's own ulp (the zoom-45 "horizontal pan skips" bug), and a
    /// plain f64 delta underflows outright past ~zoom 1060. The rotated
    /// pixel offset carries the shape, the pixel spacing's exponent
    /// carries the scale -- pan works at any depth the renderer reaches.
    /// Parse failure (mid-edit center text) falls back to the f64 path so
    /// panning never dead-stops.
    pub fn escape_pan_plane(esc: &EscapeConfig, drag: [f32; 2], panel: [f32; 2]) -> CameraEdit {
        let z = esc.zoom_log2;
        let (mx, my, se) = pan_delta_symbolic(esc, f64::from(drag[0]), f64::from(drag[1]), panel);
        let fx = FixedPoint::decimal_add_floatexp(&esc.center_re, -mx, se, z);
        let fy = FixedPoint::decimal_add_floatexp(&esc.center_im, -my, se, z);
        let (new_re, new_im) = match (fx, fy) {
            (Some(re), Some(im)) => (re, im),
            _ => {
                let (cx, cy) = esc.center_f64();
                let (wx, wy) = screen_to_world(esc, f64::from(drag[0]), f64::from(drag[1]), panel);
                (format!("{}", cx - wx), format!("{}", cy - wy))
            }
        };
        CameraEdit::batch(
            vec![
                (ConfigPath::EscapeCenterRe, ConfigValue::String(new_re)),
                (ConfigPath::EscapeCenterIm, ConfigValue::String(new_im)),
            ],
            PAN,
        )
    }

    /// Pan a terrain: the ground under the cursor follows it. A drag is a
    /// displacement of the target across the ground -- along the camera's
    /// right, and along its heading foreshortened by the pitch -- in view
    /// widths, in the plane's own axes (the terrain's world), added to the
    /// centre in fixed point: exact decimals at any depth.
    #[cfg(feature = "terrain")]
    pub fn terrain_pan(config: &FractalConfig, drag: [f32; 2], panel: [f32; 2]) -> Option<CameraEdit> {
        let esc = &config.escape;
        let cam = crate::escape::footprint::terrain_camera(esc);
        // View widths per screen pixel at the target's depth, as `ifs_ray`
        // spreads the rays.
        let s = 2.0 * (f64::from(cam.fov) * 0.5).tan() * cam.distance / f64::from(panel[1].max(1.0));
        let flat = |v: [f64; 3]| {
            let l = (v[0] * v[0] + v[1] * v[1]).sqrt();
            (l > 1e-6).then(|| [v[0] / l, v[1] / l])
        };
        let right = flat(cam.right).unwrap_or([1.0, 0.0]);
        // Looking straight down, the heading is the screen's up.
        let ahead = flat(cam.forward).or_else(|| flat(cam.up)).unwrap_or([0.0, 1.0]);
        // A screen pixel up the picture covers 1/sin(pitch) as much ground;
        // capped near the horizon, where it runs away.
        let along = s / (-cam.forward[2]).clamp(0.2, 1.0);
        let (dx, dy) = (f64::from(drag[0]), f64::from(drag[1]));
        let widths = [-dx * s * right[0] + dy * along * ahead[0], -dx * s * right[1] + dy * along * ahead[1]];
        // A view width is 4 * 2^-zoom: kept as a power of two and a mantissa.
        let x = 2.0 - esc.zoom_log2;
        let e = x.floor();
        let m = (x - e).exp2();
        let re = FixedPoint::decimal_add_floatexp(&esc.center_re, widths[0] * m, e as i64, esc.zoom_log2)?;
        let im = FixedPoint::decimal_add_floatexp(&esc.center_im, widths[1] * m, e as i64, esc.zoom_log2)?;
        Some(CameraEdit::batch(
            vec![(ConfigPath::EscapeCenterRe, ConfigValue::String(re)), (ConfigPath::EscapeCenterIm, ConfigValue::String(im))],
            PAN,
        ))
    }

    /// Zoom the escape view by `zoom_factor` toward `cursor` (its offset
    /// from the panel's centre, in pixels), in or out: the point under
    /// it stays put. A solid's anchor is in its target's plane. Without a
    /// cursor, about the centre.
    pub fn escape_zoom(config: &FractalConfig, zoom_factor: f64, cursor: Option<[f32; 2]>, panel: [f32; 2]) -> Option<CameraEdit> {
        let esc = &config.escape;
        if !(zoom_factor > 0.0) || zoom_factor == 1.0 {
            return None;
        }
        // Ceiling far past practical use but far below the floatexp
        // rung's i32-exponent arithmetic (~2^31): the old 300 was the
        // phase-1 travel clamp and would COLLAPSE a deep session's zoom
        // on the first wheel notch.
        let new_zoom_log2 = (esc.zoom_log2 + zoom_factor.log2()).clamp(-8.0, 100_000_000.0);

        let mut updates = vec![(ConfigPath::EscapeZoomLog2, ConfigValue::Float(new_zoom_log2 as f32))];

        // A solid anchors the zoom the same way, in the target's plane:
        // the point of that plane under the cursor stays under it. What
        // the eye approaches is the target, so a zoom towards the cursor
        // is a zoom that walks the target under the cursor.
        let solid = solid_view(config);

        {
            if let Some((ifs3, cam)) = &solid {
                if let Some([off_x, off_y]) = cursor {
                    let (off_x, off_y) = (f64::from(off_x), f64::from(off_y));
                    let mut esc_new = esc.clone();
                    esc_new.zoom_log2 = new_zoom_log2;
                    let shifted = solid_target_shifted(
                        &esc_new,
                        ifs3,
                        cam,
                        &[(esc, 1.0), (&esc_new, -1.0)],
                        off_x,
                        off_y,
                        f64::from(panel[1]),
                    );
                    updates.extend(solid_target_updates(shifted));
                }
            } else if let Some([off_x, off_y]) = cursor {
                // Keep the point under the cursor fixed: with the offset o
                // (screen → world) and scale ratio k = old/new span,
                // center' = center + o·(1 − 1/k) -- computed here as the
                // difference of the offset at the two spans.
                let (off_x, off_y) = (f64::from(off_x), f64::from(off_y));
                // Symbolic anchor: center += off·S_old − off·S_new, each
                // term a mantissa·2^exponent added in fixed-point. The old
                // f64 form (zoom_factor ratios) turns to inf/NaN past
                // ~zoom 1023 and underflows past ~1060; this reaches any
                // depth. Same exact-accumulation rule as panning.
                let z = esc.zoom_log2.max(new_zoom_log2);
                let (mx_o, my_o, se_o) = pan_delta_symbolic(esc, off_x, off_y, panel);
                let mut esc_new = esc.clone();
                esc_new.zoom_log2 = new_zoom_log2;
                let (mx_n, my_n, se_n) = pan_delta_symbolic(&esc_new, off_x, off_y, panel);
                let fx = FixedPoint::decimal_add_floatexp(&esc.center_re, mx_o, se_o, z)
                    .and_then(|c| FixedPoint::decimal_add_floatexp(&c, -mx_n, se_n, z));
                let fy = FixedPoint::decimal_add_floatexp(&esc.center_im, my_o, se_o, z)
                    .and_then(|c| FixedPoint::decimal_add_floatexp(&c, -my_n, se_n, z));
                let (new_re, new_im) = match (fx, fy) {
                    (Some(re), Some(im)) => (re, im),
                    _ => {
                        let (cx, cy) = esc.center_f64();
                        let (wx_old, wy_old) = screen_to_world(esc, off_x, off_y, panel);
                        let shrink = f64::exp2((esc.zoom_log2 - new_zoom_log2).clamp(-60.0, 60.0));
                        let (dx, dy) = (wx_old * (1.0 - shrink), wy_old * (1.0 - shrink));
                        (format!("{}", cx + dx), format!("{}", cy + dy))
                    }
                };
                updates.push((ConfigPath::EscapeCenterRe, ConfigValue::String(new_re)));
                updates.push((ConfigPath::EscapeCenterIm, ConfigValue::String(new_im)));
            }
        }

        Some(CameraEdit::batch(updates, ZOOM))
    }

    #[cfg(test)]
    mod solid_navigation_tests {
        use super::*;

        /// A shipped solid preset -- a real flame, a real camera.
        fn solid_preset() -> FractalConfig {
            crate::resources::presets::load_embedded_presets()
                .expect("presets parse")
                .into_iter()
                .find(|c| crate::escape::ifs::formula_is_solid(&c.escape.formula))
                .expect("a solid preset ships")
        }

        fn target_f64(t: &[String; 3]) -> [f64; 3] {
            std::array::from_fn(|k| t[k].parse::<f64>().expect("decimal"))
        }

        /// A plane is not a solid, and neither is a solid formula over a
        /// flame that does not qualify.
        #[test]
        fn only_a_qualifying_solid_has_a_solid_view() {
            let plane = FractalConfig::default();
            assert!(solid_view(&plane).is_none());
            let mut broken = solid_preset();
            assert!(solid_view(&broken).is_some());
            // A non-affine variation disqualifies the flame.
            broken.flame.transforms[0].variations.insert("spherical".to_string(), 1.0);
            broken.flame.transforms[0].variation_order.push("spherical".to_string());
            assert!(solid_view(&broken).is_none());
        }

        /// A solid that needs no flame -- the quaternion Julia -- is steered
        /// whatever the config's flame is: a drag moves its target, not the
        /// plane's centre (which it does not read). Reported in the app.
        #[test]
        fn a_flameless_solid_pans_its_target() {
            let mut cfg = FractalConfig::default();
            cfg.render_mode = crate::scene::transforms::RenderMode::Escape;
            cfg.escape.formula = "quaternion_julia_solid".to_string();
            // The default flame has no maps: it does not qualify as a solid.
            let registry = crate::variations::global_registry();
            assert!(crate::scene::ifs_analysis::analyse_3d(&cfg.flame, &registry).is_err());
            drop(registry);
            assert!(solid_view(&cfg).is_some());
            let edit = escape_pan(&cfg, [30.0, -12.0], [800.0, 600.0]).expect("a pan");
            let paths: Vec<_> = edit.changes.iter().map(|(p, _)| p.clone()).collect();
            assert_eq!(paths, vec![ConfigPath::EscapeCamTargetX, ConfigPath::EscapeCamTargetY, ConfigPath::EscapeCamTargetZ]);
            let zoom = escape_zoom(&cfg, 1.3, Some([40.0, 20.0]), [800.0, 600.0]).expect("a zoom");
            assert!(zoom.changes.iter().any(|(p, _)| *p == ConfigPath::EscapeCamTargetX), "the zoom walks the target");
        }

        /// A drag slides the target across the screen plane at its own
        /// depth: right by `dx` pixels moves the target `dx` steps along
        /// −right (content follows the cursor), down by `dy` moves it
        /// `dy` steps along +up. An empty target becomes explicit, from
        /// the attractor's centre.
        #[test]
        fn a_solid_pan_moves_the_target_across_the_screen_plane() {
            let cfg = solid_preset();
            let (ifs3, cam) = solid_view(&cfg).unwrap();
            let esc = &cfg.escape;
            assert!(esc.cam_target_x.is_empty(), "the preset frames itself");
            let (m, e) = crate::escape::ifs::solid_pixel_step(esc, &ifs3, 480.0);
            let step = m * 2f64.powi(e as i32);

            let right = solid_target_shifted(esc, &ifs3, &cam, &[(esc, -1.0)], 30.0, 0.0, 480.0);
            let down = solid_target_shifted(esc, &ifs3, &cam, &[(esc, -1.0)], 0.0, 12.0, 480.0);
            let r = target_f64(&right);
            let d = target_f64(&down);
            for k in 0..3 {
                let want_r = ifs3.ball.centre[k] - 30.0 * step * cam.right[k];
                let want_d = ifs3.ball.centre[k] + 12.0 * step * cam.up[k];
                assert!((r[k] - want_r).abs() < 1e-12 * ifs3.ball.radius, "axis {k}: {} vs {want_r}", r[k]);
                assert!((d[k] - want_d).abs() < 1e-12 * ifs3.ball.radius, "axis {k}: {} vs {want_d}", d[k]);
            }
        }

        /// The pan follows the camera: with the screen rolled by the
        /// view's rotation, a horizontal drag moves the target along the
        /// rolled right, which is not the unrolled one.
        #[test]
        fn a_solid_pan_follows_the_rolled_screen() {
            let mut cfg = solid_preset();
            let (ifs3, cam0) = solid_view(&cfg).unwrap();
            cfg.escape.rotation = 0.6;
            let (_, cam) = solid_view(&cfg).unwrap();
            let esc = &cfg.escape;
            let shifted = solid_target_shifted(esc, &ifs3, &cam, &[(esc, -1.0)], 20.0, 0.0, 480.0);
            let t = target_f64(&shifted);
            let d: [f64; 3] = std::array::from_fn(|k| t[k] - ifs3.ball.centre[k]);
            let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            let len = dot(d, d).sqrt();
            // Along the rolled right, and visibly off the unrolled one.
            assert!((dot(d, cam.right) / len + 1.0).abs() < 1e-9);
            assert!((dot(d, cam0.right) / len + 1.0).abs() > 0.1);
        }

        /// Zooming towards the cursor keeps the point of the target's
        /// plane under the cursor where it is: the target moves by the
        /// offset's change of scale, and the point itself does not.
        #[test]
        fn a_solid_zoom_to_cursor_keeps_the_point_under_it() {
            let cfg = solid_preset();
            let (ifs3, cam) = solid_view(&cfg).unwrap();
            let esc = &cfg.escape;
            let mut esc_new = esc.clone();
            esc_new.zoom_log2 = esc.zoom_log2 + 0.7;
            let (ox, oy) = (137.0, -52.0);
            let step_at = |e: &crate::config::escape::EscapeConfig| {
                let (m, ex) = crate::escape::ifs::solid_pixel_step(e, &ifs3, 480.0);
                m * 2f64.powi(ex as i32)
            };
            let shifted = solid_target_shifted(
                &esc_new, &ifs3, &cam, &[(esc, 1.0), (&esc_new, -1.0)], ox, oy, 480.0,
            );
            let t = target_f64(&shifted);
            for k in 0..3 {
                let v = ox * cam.right[k] - oy * cam.up[k];
                let before = ifs3.ball.centre[k] + v * step_at(esc);
                let after = t[k] + v * step_at(&esc_new);
                assert!((before - after).abs() < 1e-12 * ifs3.ball.radius, "axis {k}: {before} vs {after}");
            }
        }

        /// And at a depth f64 cannot step: the target still moves, by a
        /// pixel's worth, because the step is a mantissa and an exponent
        /// added in fixed point -- the same arrangement that lets the
        /// plane pan past zoom 1060.
        #[test]
        fn a_solid_pan_still_moves_at_a_depth_f64_cannot_step() {
            let mut cfg = solid_preset();
            cfg.escape.zoom_log2 = 1200.0;
            let (ifs3, cam) = solid_view(&cfg).unwrap();
            let esc = &cfg.escape;
            let once = solid_target_shifted(esc, &ifs3, &cam, &[(esc, -1.0)], 1.0, 0.0, 480.0);
            let (m, e) = crate::escape::ifs::solid_pixel_step(esc, &ifs3, 480.0);
            assert!(e < -1100, "the step's exponent should be far below f64's range, got {e}");
            // Moved: every axis with a non-zero right component changed.
            for k in 0..3 {
                let unmoved = format!("{}", ifs3.ball.centre[k]);
                if cam.right[k].abs() > 1e-6 {
                    assert_ne!(once[k], unmoved, "axis {k} did not move at zoom 2^1200");
                }
            }
            // And by the right amount: two one-pixel pans land where one
            // two-pixel pan does, to every digit but the last, which is
            // the decimal formatting's rounding and may differ by one.
            let mut esc_moved = esc.clone();
            esc_moved.cam_target_x = once[0].clone();
            esc_moved.cam_target_y = once[1].clone();
            esc_moved.cam_target_z = once[2].clone();
            let twice = solid_target_shifted(&esc_moved, &ifs3, &cam, &[(esc, -1.0)], 1.0, 0.0, 480.0);
            let direct = solid_target_shifted(esc, &ifs3, &cam, &[(esc, -1.0)], 2.0, 0.0, 480.0);
            for k in 0..3 {
                assert_eq!(twice[k].len(), direct[k].len());
                assert!(twice[k].len() > 360, "axis {k} carries {} digits", twice[k].len());
                assert_eq!(twice[k][..twice[k].len() - 1], direct[k][..direct[k].len() - 1], "axis {k}");
            }
            assert!((1.0..2.0).contains(&m));
        }
    }
}

#[cfg(feature = "engine-escape")]
pub use escape::{escape_pan, escape_pan_plane, escape_zoom, screen_to_world as escape_screen_to_world};
#[cfg(feature = "terrain")]
pub use escape::terrain_pan;

/// Slide a simulation terrain's target across the grid so the ground under
/// the cursor follows it -- in grid fractions, at the pixel's step at the
/// target's depth.
#[cfg(all(feature = "terrain", feature = "engine-sim"))]
pub fn sim_terrain_pan(config: &FractalConfig, drag: [f32; 2], panel: [f32; 2]) -> Option<CameraEdit> {
    let (gw, gh) = crate::sim::SimRenderer::grid_for(&config.sim, panel[0].max(1.0) as u32, panel[1].max(1.0) as u32);
    let cam = crate::sim::terrain::sim_terrain_camera(config, gw, gh);
    let s = 2.0 * (f64::from(cam.fov) * 0.5).tan() * cam.distance / f64::from(panel[1].max(1.0));
    let flat = |v: [f64; 3]| {
        let l = (v[0] * v[0] + v[1] * v[1]).sqrt();
        (l > 1e-6).then(|| [v[0] / l, v[1] / l])
    };
    let right = flat(cam.right).unwrap_or([1.0, 0.0]);
    let ahead = flat(cam.forward).or_else(|| flat(cam.up)).unwrap_or([0.0, 1.0]);
    let along = s / (-cam.forward[2]).clamp(0.2, 1.0);
    let (dx, dy) = (f64::from(drag[0]), f64::from(drag[1]));
    let cells = [-dx * s * right[0] + dy * along * ahead[0], -dx * s * right[1] + dy * along * ahead[1]];
    let t = &config.sim.terrain;
    let x = (t.target_x as f64 + cells[0] / (gw.max(2) - 1) as f64) as f32;
    let y = (t.target_y as f64 + cells[1] / (gh.max(2) - 1) as f64) as f32;
    Some(CameraEdit::batch(
        vec![(ConfigPath::SimTerrainTargetX, x.into()), (ConfigPath::SimTerrainTargetY, y.into())],
        PAN,
    ))
}

/// Move a simulation terrain's camera toward its target (`factor` above
/// one) or away.
#[cfg(all(feature = "terrain", feature = "engine-sim"))]
pub fn sim_terrain_dolly(config: &FractalConfig, factor: f64) -> Option<CameraEdit> {
    let d = config.sim.terrain.cam_distance;
    let next = (d / factor as f32).clamp(0.01, 100.0);
    Some(CameraEdit::batch(vec![(ConfigPath::SimTerrainCamDistance, next.into())], ZOOM))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::manager::{ConfigManager, EditingTarget};

    /// The config after an edit, as the manager would write it.
    pub(crate) fn applied(config: &FractalConfig, edit: &CameraEdit) -> FractalConfig {
        let mut c = config.clone();
        for (path, value) in &edit.changes {
            ConfigManager::apply_value_detached(&mut c, EditingTarget::Main, path, value.clone()).expect("a camera edit applies");
        }
        c
    }

    /// The flame plane's point under a screen offset (from the centre).
    fn flame_point(c: &FractalConfig, panel: [f32; 2], off: [f32; 2]) -> (f64, f64) {
        let s = (panel[0].min(panel[1]) * 0.25 * c.zoom) as f64;
        let (dx, dy) = c.screen_delta_to_pan_frame(off[0] as f64, off[1] as f64);
        (c.pan_x + dx / s, c.pan_y + dy / s)
    }

    fn rotated_flame() -> FractalConfig {
        let mut c = FractalConfig::default();
        c.rotation = 0.4;
        c.zoom = 1.7;
        c.pan_x = 0.3;
        c.pan_y = -0.2;
        c
    }

    /// A drag carries the picture with it: the point that was under a
    /// pixel is, after the drag, under that pixel moved by the drag.
    #[test]
    fn a_flame_pan_carries_the_picture() {
        let c = rotated_flame();
        let panel = [800.0, 600.0];
        let (at, drag) = ([40.0f32, -25.0], [17.0f32, 9.0]);
        let before = flame_point(&c, panel, at);
        let after = applied(&c, &flame_pan(&c, drag, panel).expect("a pan"));
        let moved = flame_point(&after, panel, [at[0] + drag[0], at[1] + drag[1]]);
        assert!((before.0 - moved.0).abs() < 1e-6 && (before.1 - moved.1).abs() < 1e-6, "{before:?} vs {moved:?}");
    }

    /// Zooming in toward the cursor keeps the point under it.
    #[test]
    fn a_flame_zoom_keeps_the_point_under_the_cursor() {
        let c = rotated_flame();
        let panel = [800.0, 600.0];
        let cur = [137.0f32, -52.0];
        let before = flame_point(&c, panel, cur);
        let after = applied(&c, &flame_zoom(&c, wheel_factor(120.0), Some(cur), panel).expect("a zoom"));
        assert!(after.zoom > c.zoom);
        let still = flame_point(&after, panel, cur);
        assert!((before.0 - still.0).abs() < 1e-6 && (before.1 - still.1).abs() < 1e-6, "{before:?} vs {still:?}");
    }

    /// The escape plane's point under a screen offset (from the centre).
    #[cfg(feature = "engine-escape")]
    fn escape_point(c: &FractalConfig, panel: [f32; 2], off: [f32; 2]) -> (f64, f64) {
        let (cx, cy) = c.escape.center_f64();
        let (wx, wy) = escape_screen_to_world(&c.escape, off[0] as f64, off[1] as f64, panel);
        (cx + wx, cy + wy)
    }

    #[cfg(feature = "engine-escape")]
    fn rotated_plane() -> FractalConfig {
        let mut c = FractalConfig::default();
        c.render_mode = crate::scene::transforms::RenderMode::Escape;
        c.escape.center_re = "-0.7436".to_string();
        c.escape.center_im = "0.1318".to_string();
        c.escape.zoom_log2 = 6.0;
        c.escape.rotation = -0.5;
        c
    }

    /// The escape plane's pan carries the picture, as the flame's does.
    #[cfg(feature = "engine-escape")]
    #[test]
    fn an_escape_pan_carries_the_picture() {
        let c = rotated_plane();
        let panel = [800.0, 600.0];
        let (at, drag) = ([40.0f32, -25.0], [17.0f32, 9.0]);
        let before = escape_point(&c, panel, at);
        let after = applied(&c, &escape_pan(&c, drag, panel).expect("a pan"));
        let moved = escape_point(&after, panel, [at[0] + drag[0], at[1] + drag[1]]);
        let tol = 1e-9 * (4.0 / c.escape.zoom_factor());
        assert!((before.0 - moved.0).abs() < tol && (before.1 - moved.1).abs() < tol, "{before:?} vs {moved:?}");
    }

    /// Zooming the escape plane in toward the cursor keeps the point
    /// under it.
    #[cfg(feature = "engine-escape")]
    #[test]
    fn an_escape_zoom_keeps_the_point_under_the_cursor() {
        let c = rotated_plane();
        let panel = [800.0, 600.0];
        let cur = [137.0f32, -52.0];
        let before = escape_point(&c, panel, cur);
        let after = applied(&c, &escape_zoom(&c, wheel_factor(120.0), Some(cur), panel).expect("a zoom"));
        assert!(after.escape.zoom_log2 > c.escape.zoom_log2);
        let still = escape_point(&after, panel, cur);
        let tol = 1e-6 * (4.0 / c.escape.zoom_factor());
        assert!((before.0 - still.0).abs() < tol && (before.1 - still.1).abs() < tol, "{before:?} vs {still:?}");
    }

    /// The camera kinds' configs for the 3D tests: a flame and a solid with
    /// the screen rolled, and the two terrains.
    fn cameras_3d() -> Vec<(&'static str, FractalConfig)> {
        let mut out = Vec::new();
        let mut flame = FractalConfig::default();
        flame.render_mode = crate::scene::transforms::RenderMode::ThreeD;
        flame.camera_rotation_x = 1.0;
        flame.camera_rotation_y = 0.4;
        flame.rotation = 0.5;
        out.push(("flame", flame));
        #[cfg(feature = "engine-escape")]
        {
            let mut solid = FractalConfig::default();
            solid.render_mode = crate::scene::transforms::RenderMode::Escape;
            solid.escape.formula = "ifs_flame_3d".to_string();
            solid.escape.cam_pitch = 0.4;
            solid.escape.cam_yaw = 0.9;
            solid.escape.rotation = -0.6;
            out.push(("solid", solid));
        }
        #[cfg(feature = "terrain")]
        {
            let mut terrain = FractalConfig::default();
            terrain.render_mode = crate::scene::transforms::RenderMode::Escape;
            terrain.escape.terrain.enabled = true;
            terrain.escape.cam_pitch = 0.6;
            terrain.escape.cam_yaw = -0.3;
            terrain.escape.rotation = 0.7;
            out.push(("escape terrain", terrain));
        }
        #[cfg(all(feature = "terrain", feature = "engine-sim"))]
        {
            let mut sim = FractalConfig::default();
            sim.render_mode = crate::scene::transforms::RenderMode::Simulation;
            sim.sim.terrain.enabled = true;
            sim.sim.terrain.cam_pitch = 0.7;
            sim.sim.terrain.cam_yaw = 1.2;
            out.push(("sim terrain", sim));
        }
        out
    }

    fn frame_of(c: &FractalConfig) -> crate::camera::view3d::Frame {
        let t = turnable(c, view_kind(c)).expect("a 3D camera");
        t.conv.frame(t.angles)
    }

    /// Where a point lands on screen, seen from `eye` (x right, y down,
    /// in units of the distance in front).
    fn on_screen(f: &crate::camera::view3d::Frame, eye: [f64; 3], p: [f64; 3]) -> (f64, f64) {
        let d = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let z = dot(d, f.forward);
        (dot(d, f.right) / z, -dot(d, f.up) / z)
    }

    /// An orbit turns the camera about its target so the near side of the
    /// scene follows the drag -- across and down, every camera, a rolled
    /// screen included.
    #[test]
    fn an_orbit_carries_the_near_side_with_the_drag() {
        let settings = TurnSettings { radians_per_pixel: 0.005, invert_y: false };
        for (name, c) in cameras_3d() {
            for drag in [[12.0f32, 0.0], [0.0, 9.0], [-7.0, 0.0], [0.0, -10.0]] {
                let before = frame_of(&c);
                let after = applied(&c, &turn(&c, drag, None, settings).expect("an orbit"));
                let f1 = frame_of(&after);
                // The target at the origin, the eye a unit behind it; a
                // point half way.
                let eye0 = [-before.forward[0], -before.forward[1], -before.forward[2]];
                let eye1 = [-f1.forward[0], -f1.forward[1], -f1.forward[2]];
                let p = [eye0[0] * 0.5, eye0[1] * 0.5, eye0[2] * 0.5];
                let (x0, y0) = on_screen(&before, eye0, p);
                let (x1, y1) = on_screen(&f1, eye1, p);
                let moved = [x1 - x0, y1 - y0];
                let along = moved[0] * drag[0] as f64 + moved[1] * drag[1] as f64;
                assert!(along > 0.0, "{name} {drag:?}: the near side moved {moved:?}");
                let across = moved[0] * drag[1] as f64 - moved[1] * drag[0] as f64;
                assert!(across.abs() < 0.5 * along.abs() + 1e-12, "{name} {drag:?}: it moved {moved:?}, off the drag");
            }
        }
    }

    /// A terrain orbits as it always did: the yaw against a drag across,
    /// the pitch with a drag down, at the fly mode's 0.005 a pixel.
    #[cfg(feature = "terrain")]
    #[test]
    fn a_terrain_orbits_as_it_did() {
        let settings = TurnSettings { radians_per_pixel: 0.005, invert_y: false };
        let c = cameras_3d().into_iter().find(|(n, _)| *n == "escape terrain").unwrap().1;
        let after = applied(&c, &turn(&c, [20.0, 8.0], None, settings).unwrap());
        assert!((after.escape.cam_yaw - (c.escape.cam_yaw - 0.1)).abs() < 1e-5, "{}", after.escape.cam_yaw);
        assert!((after.escape.cam_pitch - (c.escape.cam_pitch + 0.04)).abs() < 1e-5, "{}", after.escape.cam_pitch);
        assert!(after.escape.cam_bank.abs() < 1e-6);
    }

    /// The turntable keeps the horizon as tilted as it was on screen: the
    /// angle between the screen's right and the level axis across the
    /// view. (The stored bank is the right's slope out of the horizontal,
    /// which a pitch changes while the tilt seen stays; at a level horizon
    /// both are zero and stay so.)
    #[test]
    fn an_orbit_keeps_the_horizon_as_tilted() {
        let settings = TurnSettings { radians_per_pixel: 0.005, invert_y: false };
        let tilt = |c: &FractalConfig| {
            let f = frame_of(c);
            let level = normalized(cross(f.forward, [0.0, 0.0, 1.0])).unwrap();
            let d = (f.right[0] * level[0] + f.right[1] * level[1] + f.right[2] * level[2]).abs();
            d.clamp(-1.0, 1.0).acos()
        };
        for (name, mut c) in cameras_3d() {
            c.camera_bank = 0.3;
            c.escape.cam_bank = 0.3;
            c.sim.terrain.cam_bank = 0.3;
            let t0 = tilt(&c);
            let mut cur = c.clone();
            for drag in [[30.0f32, 0.0], [0.0, 25.0], [-12.0, 14.0]] {
                cur = applied(&cur, &turn(&cur, drag, None, settings).unwrap());
                assert!((tilt(&cur) - t0).abs() < 1e-4, "{name}: {} vs {t0}", tilt(&cur));
            }
            let mut level = c.clone();
            level.camera_bank = 0.0;
            level.escape.cam_bank = 0.0;
            level.sim.terrain.cam_bank = 0.0;
            let after = applied(&level, &turn(&level, [20.0, 15.0], None, settings).unwrap());
            assert!(after.camera_bank.abs() < 1e-6 && after.escape.cam_bank.abs() < 1e-6 && after.sim.terrain.cam_bank.abs() < 1e-6, "{name}");
        }
    }

    /// A terrain's eye stays above the ground: its pitch stops short of
    /// the horizon.
    #[cfg(feature = "terrain")]
    #[test]
    fn a_terrain_orbit_stays_above_the_ground() {
        let settings = TurnSettings { radians_per_pixel: 0.005, invert_y: false };
        let c = cameras_3d().into_iter().find(|(n, _)| *n == "escape terrain").unwrap().1;
        let after = applied(&c, &turn(&c, [0.0, -1000.0], None, settings).unwrap());
        assert!((after.escape.cam_pitch - 0.02).abs() < 1e-6, "{}", after.escape.cam_pitch);
    }

    /// A 2D turn rotates the picture with the pointer: the point that was
    /// under it is under it still, swept round the centre.
    #[test]
    fn a_2d_turn_rotates_the_picture_with_the_pointer() {
        let settings = TurnSettings { radians_per_pixel: 0.005, invert_y: false };
        let panel = [800.0, 600.0];
        let c = rotated_flame();
        let (r, a) = (110.0f32, 0.35f32);
        let before = [r, 0.0];
        let after_ptr = [r * a.cos(), r * a.sin()];
        let drag = [after_ptr[0] - before[0], after_ptr[1] - before[1]];
        let held = flame_point(&c, panel, before);
        let after = applied(&c, &turn(&c, drag, Some(after_ptr), settings).expect("a rotation"));
        let now = flame_point(&after, panel, after_ptr);
        assert!((held.0 - now.0).abs() < 1e-5 && (held.1 - now.1).abs() < 1e-5, "{held:?} vs {now:?}");
    }

    /// And the escape plane's, whose screen draws Im up.
    #[cfg(feature = "engine-escape")]
    #[test]
    fn an_escape_turn_rotates_the_picture_with_the_pointer() {
        let settings = TurnSettings { radians_per_pixel: 0.005, invert_y: false };
        let panel = [800.0, 600.0];
        let c = rotated_plane();
        let (r, a) = (110.0f32, -0.5f32);
        let before = [0.0, r];
        let after_ptr = [-r * a.sin(), r * a.cos()];
        let drag = [after_ptr[0] - before[0], after_ptr[1] - before[1]];
        let held = escape_point(&c, panel, before);
        let after = applied(&c, &turn(&c, drag, Some(after_ptr), settings).expect("a rotation"));
        let now = escape_point(&after, panel, after_ptr);
        let tol = 1e-6 * (4.0 / c.escape.zoom_factor());
        assert!((held.0 - now.0).abs() < tol && (held.1 - now.1).abs() < tol, "{held:?} vs {now:?}");
    }

    /// Zooming out keeps the point under the cursor too.
    #[test]
    fn a_zoom_out_keeps_the_point_under_the_cursor() {
        let panel = [800.0, 600.0];
        let cur = [-90.0f32, 140.0];
        let c = rotated_flame();
        let before = flame_point(&c, panel, cur);
        let after = applied(&c, &zoom(&c, 1.0 / 1.4, Some(cur), panel).expect("a zoom"));
        let still = flame_point(&after, panel, cur);
        assert!(after.zoom < c.zoom);
        assert!((before.0 - still.0).abs() < 1e-6 && (before.1 - still.1).abs() < 1e-6, "{before:?} vs {still:?}");
        #[cfg(feature = "engine-escape")]
        {
            let c = rotated_plane();
            let before = escape_point(&c, panel, cur);
            let after = applied(&c, &zoom(&c, 1.0 / 1.4, Some(cur), panel).expect("a zoom"));
            let still = escape_point(&after, panel, cur);
            let tol = 1e-6 * (4.0 / c.escape.zoom_factor());
            assert!((before.0 - still.0).abs() < tol && (before.1 - still.1).abs() < tol, "{before:?} vs {still:?}");
        }
    }

    /// A pinch is a zoom about the midpoint, the midpoint's move as a pan
    /// and a twist as a rotation -- one entry, each path once.
    #[test]
    fn a_pinch_is_zoom_pan_and_twist_in_one_entry() {
        let panel = [800.0, 600.0];
        let c = rotated_flame();
        let edit = pinch(&c, 1.25, [6.0, -4.0], 0.1, [50.0, 30.0], panel).expect("a pinch");
        assert_eq!(edit.history, Some(ZOOM));
        let paths: Vec<_> = edit.changes.iter().map(|(p, _)| p.clone()).collect();
        assert_eq!(paths.len(), 3, "{paths:?}");
        let after = applied(&c, &edit);
        assert!(after.zoom > c.zoom && (after.rotation - c.rotation).abs() > 0.05);
    }

    /// Reset View returns the shown camera home; a 2D simulation has none
    /// yet.
    #[test]
    fn reset_returns_each_camera_home() {
        let c = rotated_flame();
        let after = applied(&c, &reset(&c).expect("a flame resets"));
        assert_eq!((after.zoom, after.pan_x, after.pan_y, after.rotation), (1.0, 0.0, 0.0, 0.0));
        #[cfg(feature = "engine-escape")]
        {
            let c = rotated_plane();
            let after = applied(&c, &reset(&c).expect("the plane resets"));
            let home = crate::config::escape::EscapeConfig::default();
            assert_eq!(
                (after.escape.center_re.as_str(), after.escape.zoom_log2, after.escape.rotation),
                (home.center_re.as_str(), 0.0, 0.0)
            );
        }
        let mut sim = FractalConfig::default();
        sim.render_mode = crate::scene::transforms::RenderMode::Simulation;
        assert!(reset(&sim).is_none());
    }
}

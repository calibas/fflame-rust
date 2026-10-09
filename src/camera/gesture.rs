//! The viewport's camera gestures, as config edits.
//!
//! Each takes the config and the gesture's measure in pixels and returns
//! the [`CameraEdit`] it makes, or `None` for nothing to do. Pure: the
//! viewport applies the edit (`CameraEdit::apply`), so a gesture can be
//! tested without a window, and every input that moves a camera -- the
//! viewport, the tab strip, the keys, the menus -- moves it the same way.
//!
//! Moved here from `ui::panel_viewer` unchanged (camera-unification P0):
//! the arithmetic, its f32/f64 mix and its history names are what the
//! viewport did. P1 makes the gestures the plan's.

use super::CameraEdit;
use crate::config::{ConfigPath, ConfigValue, FractalConfig};

/// Pan the flame view by a drag of `drag` pixels on a panel `panel`
/// pixels in size. The smaller side scales both axes, so a drag moves as
/// fast in portrait as in landscape.
pub fn flame_pan(config: &FractalConfig, drag: [f32; 2], panel: [f32; 2]) -> Option<CameraEdit> {
    let ref_size = panel[0].min(panel[1]);
    let scale = 4.0 / (config.zoom * ref_size);
    let dx = -drag[0] * scale;
    let dy = -drag[1] * scale;
    let (fractal_dx, fractal_dy) = config.screen_delta_to_pan_frame(dx as f64, dy as f64);
    Some(CameraEdit::param(ConfigPath::Pan, (config.pan_x + fractal_dx, config.pan_y + fractal_dy).into()))
}

/// Zoom the flame view by a wheel's `scroll`: in toward `cursor` (its
/// offset from the panel's centre, in pixels) when there is one and
/// `zoom_to_cursor` allows, out from the centre always.
pub fn flame_zoom(
    config: &FractalConfig,
    scroll: f32,
    cursor: Option<[f32; 2]>,
    panel: [f32; 2],
    zoom_to_cursor: bool,
) -> Option<CameraEdit> {
    let zoom_factor = if scroll.abs() > 0.1 { 1.1f32.powf(scroll * 0.03) } else { 1.0 };
    if zoom_factor == 1.0 {
        return None;
    }
    let new_zoom = (config.zoom * zoom_factor).clamp(0.01, config.max_view_zoom());
    match cursor.filter(|_| zoom_to_cursor && zoom_factor > 1.0) {
        Some([ox, oy]) => {
            // The point under the cursor stays under it.
            let scale = panel[0].min(panel[1]) * 0.25;
            let (rx, ry) = config.screen_delta_to_pan_frame(ox as f64, oy as f64);
            let point_x = config.pan_x + rx / (scale * config.zoom) as f64;
            let point_y = config.pan_y + ry / (scale * config.zoom) as f64;
            let new_pan_x = point_x - rx / (scale * new_zoom) as f64;
            let new_pan_y = point_y - ry / (scale * new_zoom) as f64;
            Some(CameraEdit::batch(
                vec![(ConfigPath::Zoom, new_zoom.into()), (ConfigPath::Pan, (new_pan_x, new_pan_y).into())],
                "history.action.wheel_zoom",
            ))
        }
        None => Some(CameraEdit::param(ConfigPath::Zoom, new_zoom.into())),
    }
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
        if !crate::escape::ifs::formula_is_solid(&config.escape.formula) {
            return None;
        }
        let registry = crate::variations::global_registry();
        let ifs3 = crate::scene::ifs_analysis::analyse_3d(&config.flame, &registry).ok()?;
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
            return Some(CameraEdit::batch(solid_target_updates(shifted), "history.param.escape_cam_target_x"));
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
            "history.action.pan_view",
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
            "history.action.pan_view",
        ))
    }

    /// Orbit a terrain's camera about its target: a horizontal drag turns
    /// the yaw, a vertical one the pitch, so the ground under the cursor
    /// turns with it. The pitch stays above the horizon and short of the
    /// zenith, where the yaw would stop meaning anything.
    #[cfg(feature = "terrain")]
    pub fn terrain_orbit(config: &FractalConfig, drag: [f32; 2]) -> Option<CameraEdit> {
        let esc = &config.escape;
        let (yaw, pitch) = orbit_step(esc.cam_yaw, esc.cam_pitch, drag);
        Some(CameraEdit::batch(
            vec![(ConfigPath::EscapeCamYaw, yaw.into()), (ConfigPath::EscapeCamPitch, pitch.into())],
            "history.action.orbit_camera",
        ))
    }

    /// Wheel zoom for the escape view: zoom-in anchors to the cursor
    /// (the point under it stays put), zoom-out recedes from center --
    /// the same feel as the flame viewport. `cursor` is the cursor's
    /// offset from the panel's centre, in pixels.
    pub fn escape_zoom(
        config: &FractalConfig,
        scroll: f32,
        cursor: Option<[f32; 2]>,
        panel: [f32; 2],
        zoom_to_cursor: bool,
    ) -> Option<CameraEdit> {
        let esc = &config.escape;
        // A terrain dollies toward its target, the screen's centre: the
        // point under the cursor is somewhere on the ground, at a depth a
        // plane's anchor knows nothing of.
        let zoom_to_cursor = zoom_to_cursor && !esc.terrain_active();

        let zoom_factor = if scroll.abs() > 0.1 {
            f64::from(1.1f32).powf(f64::from(scroll) * 0.03)
        } else {
            return None;
        };
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

        if zoom_factor > 1.0 {
            if let Some((ifs3, cam)) = &solid {
                if let Some([off_x, off_y]) = cursor.filter(|_| zoom_to_cursor) {
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
            } else if let Some([off_x, off_y]) = cursor.filter(|_| zoom_to_cursor) {
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

        Some(CameraEdit::batch(updates, "history.action.wheel_zoom"))
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
pub use escape::{terrain_orbit, terrain_pan};

/// A terrain orbit's step: a drag of `drag` pixels turns the yaw against
/// the horizontal and the pitch with the vertical, 0.005 radians a pixel;
/// the pitch kept above the horizon and short of the zenith.
#[cfg(feature = "terrain")]
fn orbit_step(yaw: f32, pitch: f32, drag: [f32; 2]) -> (f32, f32) {
    const RAD_PER_PX: f32 = 0.005;
    let mut yaw = yaw - drag[0] * RAD_PER_PX;
    yaw = (yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    let pitch = (pitch + drag[1] * RAD_PER_PX).clamp(0.02, std::f32::consts::FRAC_PI_2 - 0.001);
    (yaw, pitch)
}

/// Orbit a simulation terrain's camera about its target, as an escape
/// terrain's: a horizontal drag turns the yaw, a vertical one the pitch.
#[cfg(all(feature = "terrain", feature = "engine-sim"))]
pub fn sim_terrain_orbit(config: &FractalConfig, drag: [f32; 2]) -> Option<CameraEdit> {
    let t = &config.sim.terrain;
    let (yaw, pitch) = orbit_step(t.cam_yaw, t.cam_pitch, drag);
    Some(CameraEdit::batch(
        vec![(ConfigPath::SimTerrainCamYaw, yaw.into()), (ConfigPath::SimTerrainCamPitch, pitch.into())],
        "history.action.orbit_camera",
    ))
}

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
        "history.action.pan_view",
    ))
}

/// Move a simulation terrain's camera toward its target or away: a wheel
/// notch is about an eighth of the distance.
#[cfg(all(feature = "terrain", feature = "engine-sim"))]
pub fn sim_terrain_dolly(config: &FractalConfig, scroll: f32) -> Option<CameraEdit> {
    let d = config.sim.terrain.cam_distance;
    let next = (d * (-scroll * 0.0025).exp()).clamp(0.01, 100.0);
    Some(CameraEdit::param(ConfigPath::SimTerrainCamDistance, next.into()))
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
        let after = applied(&c, &flame_zoom(&c, 120.0, Some(cur), panel, true).expect("a zoom"));
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
        let after = applied(&c, &escape_zoom(&c, 120.0, Some(cur), panel, true).expect("a zoom"));
        assert!(after.escape.zoom_log2 > c.escape.zoom_log2);
        let still = escape_point(&after, panel, cur);
        let tol = 1e-6 * (4.0 / c.escape.zoom_factor());
        assert!((before.0 - still.0).abs() < tol && (before.1 - still.1).abs() < tol, "{before:?} vs {still:?}");
    }
}

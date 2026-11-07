/// 3D bubble inversion with stripes and Z-axis hole — first does a bubble
/// (sphere) inversion of `(x, y)` (`/= (x²+y²)/4 + 1`), optionally erases
/// or rotates wedge-shaped stripes around the XY plane (count via
/// `number_of_stripes`, width-ratio via `ratio_of_stripes`), and finally
/// erases or blends a cap on the Z axis based on `angle_of_hole` (with
/// `exponent_z` controlling the Z bubble shape). `modus_blur = 0` does hard
/// cutouts; `modus_blur = 1` does a smooth angular blend (only effective
/// for the Z hole when `exponent_z == 1`). Negative `number_of_stripes` or
/// `angle_of_hole` inverts which side of the boundary is drawn vs blanked.
///
/// # Authors
/// - FractalDesire

// Bubble T3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    number_of_stripes: #{
        value: 0, min: -50, max: 50,
        caption: "Number of Stripes",
        hint: "Number of angular stripe sectors around the XY plane. 0 disables stripes entirely. Negative value inverts the stripe pattern (swap drawn vs blanked sectors).",
    },
    ratio_of_stripes: #{
        value: 1.0, min: 0.01, max: 1.99, free: true,
        caption: "Ratio of Stripes",
        hint: "Width fraction of the drawn stripe within each sector, clamped to [0.01, 1.99]. 1.0 = stripes and gaps are equal width.",
    },
    angle_of_hole: #{
        value: 0.0, min: -180.0, max: 180.0, free: true,
        caption: "Angle of Hole",
        hint: "Half-angle of the Z-axis hole in degrees. Negative value inverts which side of the hole is drawn vs blanked.",
    },
    exponent_z: #{
        value: 1.0, min: 0.1, max: 10.0, free: true,
        caption: "Exponent Z",
        hint: "Z-axis bubble shape exponent. 1.0 = standard inversion; smaller values flatten the bubble, larger values elongate it.",
    },
    symmetry_z: #{
        value: false,
        caption: "Symmetry Z",
        hint: "When on, mirror the Z hole symmetrically across the XY plane (clamps `|angle_of_hole|` to < 180°). When off, apply the hole to only one Z-pole.",
    },
    modus_blur: #{
        value: false,
        caption: "Modus Blur",
        hint: "When on, smooth-blend the stripe/hole interior via angular rotation. When off, hard-cutout (zero out). Only effective on the Z hole when `exponent_z == 1`; otherwise falls back to hard cutout regardless.",
    },
};

const INFO = #{ title: "Bubble T3D", name: "rhai_bubblet3d", category: "full_3d", phase: "any" };

fn prepare(p) {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let stripes_raw = to_float(p.number_of_stripes);
    let abs_stripes = abs(stripes_raw);
    let inv_stripes = (if stripes_raw < 0.0 { 1.0 } else { 0.0 });
    let ratio = clamp(p.ratio_of_stripes, 0.01, 1.99);
    let ang_strip = 0.0;
    let ang_strip1 = 0.0;
    let ang_strip2 = 0.0;
    if (abs_stripes > 0.5) {
        ang_strip = pi / abs_stripes;
        ang_strip2 = 2.0 * ang_strip;
        ang_strip1 = ratio * ang_strip;
    }
    let angle_h = p.angle_of_hole;
    let symmetry_z = to_float(p.symmetry_z) > 0.5;
    if (symmetry_z) {
        angle_h = abs(angle_h);
        angle_h = min(179.9, angle_h);
    }
    let inv_hole = (if angle_h < 0.0 { 1.0 } else { 0.0 });
    let angle_eff = abs(angle_h);
    if (inv_hole > 0.5) {
        angle_eff = (angle_eff / 360.0 * two_pi) * 0.5;
    } else {
        angle_eff = pi - (angle_eff / 360.0 * two_pi) * 0.5;
    }
    #{ ang_hole_comp: pi - angle_eff, ang_strip: ang_strip, ang_strip1: ang_strip1, ang_strip2: ang_strip2, inv_stripes: inv_stripes, inv_hole: inv_hole, angle_eff: angle_eff }
}

fn variation(z, p) {
    let stripes_raw = to_float(p.number_of_stripes);
    let abs_stripes = abs(stripes_raw);
    let ratio = clamp(p.ratio_of_stripes, 0.01, 1.99);
    let exponent_z = p.exponent_z;
    let modus_blur = to_int(to_float(p.modus_blur));
    let ang_strip = p.ang_strip;
    let ang_strip1 = p.ang_strip1;
    let ang_strip2 = p.ang_strip2;
    let inv_stripes = p.inv_stripes > 0.5;
    let two_pi = 6.28318530717959;
    let x = z.re;
    let y = z.im;
    let ang_xy = atan2(x, y);
    if (ang_xy < 0.0) {
        ang_xy = ang_xy + two_pi;
    }
    if (abs_stripes > 0.5 && ang_strip2 > 0.0) {
        ang_xy = ang_xy - to_float(floor(ang_xy / ang_strip2)) * ang_strip2;
        let stripe_hit = (if inv_stripes { ang_xy < ang_strip1 } else { ang_xy > ang_strip1 });
        if (stripe_hit) {
            if (modus_blur == 0) {
                x = 0.0;
                y = 0.0;
            } else {
                let ang_rot = 0.0;
                if (ratio == 1.0) {
                    ang_rot = ang_strip;
                } else if (inv_stripes) {
                    let t = (ang_xy - ang_strip1) / ang_strip1;
                    ang_rot = ang_xy - t * (ang_strip2 - ang_strip1);
                } else {
                    let t = (ang_xy - ang_strip1) / (ang_strip2 - ang_strip1);
                    ang_rot = ang_xy - t * ang_strip1;
                }
                let s = sin(ang_rot);
                let c = cos(ang_rot);
                let x_tmp = c * x - s * y;
                let y_tmp = s * x + c * y;
                x = x_tmp;
                y = y_tmp;
            }
        }
    }
    let rad = (x * x + y * y) * 0.25 + 1.0;
    let safe_rad = (if rad < 1.0e-30 { 1.0e-30 } else { rad });
    complex(x / safe_rad, y / safe_rad)
}

fn variation3(v, p) {
    let stripes_raw = to_float(p.number_of_stripes);
    let abs_stripes = abs(stripes_raw);
    let ratio = clamp(p.ratio_of_stripes, 0.01, 1.99);
    let exponent_z = p.exponent_z;
    let symmetry_z = to_int(to_float(p.symmetry_z));
    let modus_blur = to_int(to_float(p.modus_blur));
    let ang_hole_comp = p.ang_hole_comp;
    let ang_strip = p.ang_strip;
    let ang_strip1 = p.ang_strip1;
    let ang_strip2 = p.ang_strip2;
    let inv_stripes = p.inv_stripes > 0.5;
    let inv_hole = p.inv_hole > 0.5;
    let angle_eff = p.angle_eff;
    let pi = 3.14159265358979;
    let pi_2 = 1.5707963267948966;
    let two_pi = 6.28318530717959;
    let x = v.x;
    let y = v.y;
    let z = v.z;
    let ang_xy = atan2(x, y);
    if (ang_xy < 0.0) {
        ang_xy = ang_xy + two_pi;
    }
    if (abs_stripes > 0.5 && ang_strip2 > 0.0) {
        ang_xy = ang_xy - to_float(floor(ang_xy / ang_strip2)) * ang_strip2;
        let stripe_hit = (if inv_stripes { ang_xy < ang_strip1 } else { ang_xy > ang_strip1 });
        if (stripe_hit) {
            if (modus_blur == 0) {
                x = 0.0;
                y = 0.0;
            } else {
                let ang_rot = 0.0;
                if (ratio == 1.0) {
                    ang_rot = ang_strip;
                } else if (inv_stripes) {
                    let t = (ang_xy - ang_strip1) / ang_strip1;
                    ang_rot = ang_xy - t * (ang_strip2 - ang_strip1);
                } else {
                    let t = (ang_xy - ang_strip1) / (ang_strip2 - ang_strip1);
                    ang_rot = ang_xy - t * ang_strip1;
                }
                let s = sin(ang_rot);
                let c = cos(ang_rot);
                let x_tmp = c * x - s * y;
                let y_tmp = s * x + c * y;
                x = x_tmp;
                y = y_tmp;
            }
        }
    }
    let rad = (x * x + y * y) * 0.25 + 1.0;
    let safe_rad = (if rad < 1.0e-30 { 1.0e-30 } else { rad });
    x = x / safe_rad;
    y = y / safe_rad;
    let ang_z = 0.0;
    if (abs(x) > 1.0e-30 || abs(y) > 1.0e-30) {
        z = 2.0 / pow(safe_rad, exponent_z) - 1.0;
        if (exponent_z <= 2.0) {
            let denom = x * x + y * y + z * z;
            ang_z = pi - acos(clamp(z / max(1.0e-30, denom), -1.0, 1.0));
        } else {
            let r2sq = (x * x + y * y) * (x * x + y * y);
            ang_z = pi - atan2(r2sq, z);
        }
    } else {
        z = 0.0;
        ang_z = 0.0;
    }
    if (symmetry_z == 0) {
        let hit = false;
        if (!inv_hole) {
            hit = ang_z > angle_eff;
        } else {
            hit = ang_z < angle_eff;
        }
        if (hit) {
            if (modus_blur == 0 || exponent_z != 1.0) {
                x = 0.0;
                y = 0.0;
                z = 0.0;
            } else {
                let ang_tmp = 0.0;
                if (!inv_hole) {
                    ang_tmp = (pi - ang_z) / max(1.0e-30, ang_hole_comp) * angle_eff - pi_2;
                } else {
                    ang_tmp = pi - ang_z / max(1.0e-30, ang_hole_comp) * angle_eff - pi_2;
                }
                let ang_z_shift = ang_z - pi_2;
                let safe_cos = (if abs(cos(ang_z_shift)) < 1.0e-30 { 1.0e-30 } else { cos(ang_z_shift) });
                let fac = cos(ang_tmp) / safe_cos;
                x = x * fac;
                y = y * fac;
                let safe_sin = (if abs(sin(ang_z_shift)) < 1.0e-30 { 1.0e-30 } else { sin(ang_z_shift) });
                z = z * (sin(ang_tmp) / safe_sin);
            }
        }
    } else if (ang_z > angle_eff || ang_z < (pi - angle_eff)) {
        if (modus_blur == 0 || exponent_z != 1.0) {
            x = 0.0;
            y = 0.0;
            z = 0.0;
        } else {
            let ang_tmp = 0.0;
            let safe_hc = max(1.0e-30, ang_hole_comp);
            if (ang_z > angle_eff) {
                ang_tmp = (pi - ang_z) / safe_hc * (pi - 2.0 * ang_hole_comp) + ang_hole_comp - pi_2;
            } else {
                ang_tmp = pi_2 - (ang_z / safe_hc * (pi - 2.0 * ang_hole_comp) + ang_hole_comp);
            }
            let ang_z_shift = ang_z - pi_2;
            let safe_cos = (if abs(cos(ang_z_shift)) < 1.0e-30 { 1.0e-30 } else { cos(ang_z_shift) });
            let fac = cos(ang_tmp) / safe_cos;
            x = x * fac;
            y = y * fac;
            let safe_sin = (if abs(sin(ang_z_shift)) < 1.0e-30 { 1.0e-30 } else { sin(ang_z_shift) });
            z = z * (sin(ang_tmp) / safe_sin);
        }
    }
    vec3(x, y, z)
}

//! Lazy* family — Michael Faber / FarDareisMai
//!
//! Two N-gon-based "rotate inside, fold outside" warps:
//!
//!   - `lazyjess`   (FarDareisMai 2012) — rotate-and-flip points within
//!     a regular N-gon's inscribed area; n=2 special case for line
//!     segment.
//!   - `lazytravis` (Faber 2012)        — fold-mirror around the
//!     axis-aligned square of side ±VVAR.
//!
//! Both use VVAR as a comparison threshold (lazyjess: `|x| < VVAR` or
//! `modulus < r` where `r` contains VVAR; lazytravis: `x > VVAR ||
//! y > VVAR`). Body uses `needs_transform: true` to read the weight
//! and then applies VVAR as both the threshold and the output scaling
//! through the outer multiplier — output lines are clean
//! `VVAR * stuff` so they factor naturally.
//!
//! Sources:
//!   - output/jwildfire-vars/output/lazyjess.cpp
//!   - output/jwildfire-vars/output/lazytravis.cpp

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// lazyjess: N-gon rotate-and-flip warp (FarDareisMai)
//   Init:
//     vertex     = π · (n − 2) / (2n)         (interior half-angle)
//     sin_vertex = sin(vertex)
//     pie_slice  = 2π / n
//     corner_rot = (corner − 1) · pie_slice
//   Body branches on n: n=2 uses x-stripe test, n>2 uses inscribed
//   N-gon test based on theta_diff inside a pie slice.
//   When inside the designated area: rotate by `spin`, possibly flip
//   into nearest corner. When outside: radial nudge by `space`.
// =============================================================================
/// N-gon rotate-and-flip warp — splits behavior based on whether the input
/// lies inside or outside an inscribed regular N-gon. Inside, the point is
/// rotated by `spin` and, if the rotated point is still inside, kept;
/// otherwise it's flipped into the `corner`-indexed corner sector. Outside
/// the N-gon, the point gets a radial nudge by `space`. The n = 2 special
/// case uses an axis-aligned line-segment test instead of the polygon test.
///
/// # Authors
/// - FarDareisMai
pub static LAZYJESS: VariationDef = VariationDef {
    name: "lazyjess",
    aliases: &[],
    display_name: "Lazy Jess",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("n", "N", int, 4.0, 2.0, 50.0, "Polygon vertex count. The n = 2 special case uses a line-segment test instead of a true polygon."),
        param!("spin", "Spin", unlimited_float, 3.14159265, -10.0, 10.0, "Rotation applied to points inside the N-gon, in radians."),
        param!("space", "Space", unlimited_float, 0.0, -10.0, 10.0, "Radial nudge applied to points outside the N-gon (added to the radius, scaled inversely by `modulus`)."),
        param!("corner", "Corner", int, 1.0, 1.0, 50.0, "Which of the N corners to flip to when the post-rotation inside-test fails (1-based)."),
    ],
    needs_transform: true,
    writes_color: false,
    // 4 derived values at slots 4..8:
    //   4: vertex      (π · (n − 2) / (2n))
    //   5: sin_vertex  (sin(vertex))
    //   6: pie_slice   (2π / n)
    //   7: corner_rot  ((corner − 1) · pie_slice)
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_lazyjess(user: array<f32, 4>) -> array<f32, 4> {
    let n = max(user[0], 2.0);
    let corner = user[3];
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let vertex = pi * (n - 2.0) / (2.0 * n);
    let pie_slice = two_pi / n;
    var out: array<f32, 4>;
    out[0] = vertex;
    out[1] = sin(vertex);
    out[2] = pie_slice;
    out[3] = (corner - 1.0) * pie_slice;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_lazyjess(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let n = max(get_param(xform_id, variation_id, 0u), 2.0);
    let spin = get_param(xform_id, variation_id, 1u);
    let space = get_param(xform_id, variation_id, 2u);
    let vertex = get_param(xform_id, variation_id, 4u);
    let sin_vertex = get_param(xform_id, variation_id, 5u);
    let pie_slice = get_param(xform_id, variation_id, 6u);
    let corner_rot = get_param(xform_id, variation_id, 7u);
    let half_slice = pie_slice * 0.5;
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let sqrt2 = 1.4142135623730951;
    let w = transforms[xform_id].variations[variation_id];

    var fx: f32;
    var fy: f32;
    let modulus = sqrt(p.x * p.x + p.y * p.y);

    if (n == 2.0) {
        if (abs(p.x) < w) {
            var theta = atan2(p.y, p.x) + spin;
            let x = w * modulus * cos(theta);
            let y = w * modulus * sin(theta);
            if (abs(x) < w) {
                fx = x;
                fy = y;
            } else {
                theta = atan2(p.y, p.x) - spin + corner_rot;
                fx = w * modulus * cos(theta);
                fy = -w * modulus * sin(theta);
            }
        } else {
            let m2 = 1.0 + space / max(modulus, 1e-30);
            fx = w * m2 * p.x;
            fy = w * m2 * p.y;
        }
    } else {
        // distance r from origin to the polygon edge at angle theta
        let theta_in = atan2(p.y, p.x) + two_pi;
        let theta_diff_in = theta_in + half_slice - floor((theta_in + half_slice) / pie_slice) * pie_slice;
        let denom_in = sin(pi - theta_diff_in - vertex);
        let r_in = w * sqrt2 * sin_vertex / select(denom_in, 1e-30, abs(denom_in) < 1e-30);

        if (modulus < r_in) {
            // Rotate-then-recheck inscribed
            let theta_rot = atan2(p.y, p.x) + spin + two_pi;
            let x = w * modulus * cos(theta_rot);
            let y = w * modulus * sin(theta_rot);
            let theta_diff_r = theta_rot + half_slice - floor((theta_rot + half_slice) / pie_slice) * pie_slice;
            let denom_r = sin(pi - theta_diff_r - vertex);
            let r_r = w * sqrt2 * sin_vertex / select(denom_r, 1e-30, abs(denom_r) < 1e-30);
            let m_rot = sqrt(x * x + y * y);
            if (m_rot < r_r) {
                fx = x;
                fy = y;
            } else {
                let theta_flip = atan2(p.y, p.x) - spin + corner_rot + two_pi;
                fx = w * modulus * cos(theta_flip);
                fy = -w * modulus * sin(theta_flip);
            }
        } else {
            let m2 = 1.0 + space / max(modulus, 1e-30);
            fx = w * m2 * p.x;
            fy = w * m2 * p.y;
        }
    }

    // Output lines have VVAR baked in already. Divide by w so outer × w
    // restores the cpp result; if w is 0 the variation is "off" anyway.
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_lazyjess(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let n = max(get_param(xform_id, variation_id, 0u), 2.0);
    let spin = get_param(xform_id, variation_id, 1u);
    let space = get_param(xform_id, variation_id, 2u);
    let vertex = get_param(xform_id, variation_id, 4u);
    let sin_vertex = get_param(xform_id, variation_id, 5u);
    let pie_slice = get_param(xform_id, variation_id, 6u);
    let corner_rot = get_param(xform_id, variation_id, 7u);
    let half_slice = pie_slice * 0.5;
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let sqrt2 = 1.4142135623730951;
    let w = transforms[xform_id].variations[variation_id];

    var fx: f32;
    var fy: f32;
    let modulus = sqrt(p.x * p.x + p.y * p.y);

    if (n == 2.0) {
        if (abs(p.x) < w) {
            var theta = atan2(p.y, p.x) + spin;
            let x = w * modulus * cos(theta);
            let y = w * modulus * sin(theta);
            if (abs(x) < w) {
                fx = x;
                fy = y;
            } else {
                theta = atan2(p.y, p.x) - spin + corner_rot;
                fx = w * modulus * cos(theta);
                fy = -w * modulus * sin(theta);
            }
        } else {
            let m2 = 1.0 + space / max(modulus, 1e-30);
            fx = w * m2 * p.x;
            fy = w * m2 * p.y;
        }
    } else {
        let theta_in = atan2(p.y, p.x) + two_pi;
        let theta_diff_in = theta_in + half_slice - floor((theta_in + half_slice) / pie_slice) * pie_slice;
        let denom_in = sin(pi - theta_diff_in - vertex);
        let r_in = w * sqrt2 * sin_vertex / select(denom_in, 1e-30, abs(denom_in) < 1e-30);

        if (modulus < r_in) {
            let theta_rot = atan2(p.y, p.x) + spin + two_pi;
            let x = w * modulus * cos(theta_rot);
            let y = w * modulus * sin(theta_rot);
            let theta_diff_r = theta_rot + half_slice - floor((theta_rot + half_slice) / pie_slice) * pie_slice;
            let denom_r = sin(pi - theta_diff_r - vertex);
            let r_r = w * sqrt2 * sin_vertex / select(denom_r, 1e-30, abs(denom_r) < 1e-30);
            let m_rot = sqrt(x * x + y * y);
            if (m_rot < r_r) {
                fx = x;
                fy = y;
            } else {
                let theta_flip = atan2(p.y, p.x) - spin + corner_rot + two_pi;
                fx = w * modulus * cos(theta_flip);
                fy = -w * modulus * sin(theta_flip);
            }
        } else {
            let m2 = 1.0 + space / max(modulus, 1e-30);
            fx = w * m2 * p.x;
            fy = w * m2 * p.y;
        }
    }

    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};

// =============================================================================
// lazytravis: square fold-mirror with quadrant routing (Michael Faber)
//   if |x| > VVAR or |y| > VVAR: outside the square — fold around
//     the box edge using a perimeter parameterization `p ∈ [0, 8s]`,
//     then map p to a (x', y') on the larger box with `space`-padding
//   else: inside — same perimeter idea but no padding, output =
//     VVAR · (boundary point on inner box)
// =============================================================================
/// Square fold-mirror with quadrant routing — folds points around an axis-
/// aligned square of side ±VVAR. Outside the square, points get folded back
/// onto the square's outer edge via a perimeter parameterization with
/// `spin_out`-driven angular offset and a `space` padding that stretches
/// the perpendicular axis. Inside the square, the same parameterization
/// runs with `spin_in` offset and no padding.
///
/// # Authors
/// - Michael Faber
pub static LAZYTRAVIS: VariationDef = VariationDef {
    name: "lazyTravis",
    aliases: &[],
    display_name: "Lazy Travis",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("spin_in", "Spin In", unlimited_float, 1.0, -10.0, 10.0, "Inner-square angular offset along the perimeter (multiplied by 4 internally; one unit = one full lap of the square)."),
        param!("spin_out", "Spin Out", unlimited_float, 0.5, -10.0, 10.0, "Outer-square angular offset along the perimeter (same scaling as `spin_in`)."),
        param!("space", "Space", unlimited_float, 1.5707963267948966, -10.0, 10.0, "Outer-square padding — extends the box edge by `space` and proportionally stretches the perpendicular coordinate."),
    ],
    needs_transform: true,
    writes_color: false,
    // 2 derived values at slots 3..5:
    //   3: spin_in_4   (4 · spin_in)
    //   4: spin_out_4  (4 · spin_out)
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_lazyTravis(user: array<f32, 3>) -> array<f32, 2> {
    var out: array<f32, 2>;
    out[0] = 4.0 * user[0];
    out[1] = 4.0 * user[1];
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_lazyTravis(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let space = get_param(xform_id, variation_id, 2u);
    let spin_in_4 = get_param(xform_id, variation_id, 3u);
    let spin_out_4 = get_param(xform_id, variation_id, 4u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let ax = abs(p.x);
    let ay = abs(p.y);
    var s: f32;
    var pp: f32;
    var fx: f32;
    var fy: f32;

    if (ax > w || ay > w) {
        // Outside square — fold via perimeter
        if (ax > ay) {
            s = ax;
            if (p.x > 0.0) {
                pp = s + p.y + s * spin_out_4;
            } else {
                pp = 5.0 * s - p.y + s * spin_out_4;
            }
        } else {
            s = ay;
            if (p.y > 0.0) {
                pp = 3.0 * s - p.x + s * spin_out_4;
            } else {
                pp = 7.0 * s + p.x + s * spin_out_4;
            }
        }
        let safe_8s = max(s * 8.0, 1e-30);
        pp = pp - floor(pp / safe_8s) * safe_8s;
        let inv_s = 1.0 / max(s, 1e-30);

        var x2: f32;
        var y2: f32;
        if (pp <= 2.0 * s) {
            x2 = s + space;
            let yt = -(s - pp);
            y2 = yt + yt * inv_s * space;
        } else if (pp <= 4.0 * s) {
            y2 = s + space;
            let xt = 3.0 * s - pp;
            x2 = xt + xt * inv_s * space;
        } else if (pp <= 6.0 * s) {
            x2 = -(s + space);
            let yt = 5.0 * s - pp;
            y2 = yt + yt * inv_s * space;
        } else {
            y2 = -(s + space);
            let xt = -(7.0 * s - pp);
            x2 = xt + xt * inv_s * space;
        }
        fx = w * x2;
        fy = w * y2;
    } else {
        // Inside square — fold without padding
        if (ax > ay) {
            s = ax;
            if (p.x > 0.0) {
                pp = s + p.y + s * spin_in_4;
            } else {
                pp = 5.0 * s - p.y + s * spin_in_4;
            }
        } else {
            s = ay;
            if (p.y > 0.0) {
                pp = 3.0 * s - p.x + s * spin_in_4;
            } else {
                pp = 7.0 * s + p.x + s * spin_in_4;
            }
        }
        let safe_8s = max(s * 8.0, 1e-30);
        pp = pp - floor(pp / safe_8s) * safe_8s;

        if (pp <= 2.0 * s) {
            fx = w * s;
            fy = -w * (s - pp);
        } else if (pp <= 4.0 * s) {
            fx = w * (3.0 * s - pp);
            fy = w * s;
        } else if (pp <= 6.0 * s) {
            fx = -w * s;
            fy = w * (5.0 * s - pp);
        } else {
            fx = -w * (7.0 * s - pp);
            fy = -w * s;
        }
    }
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_lazyTravis(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let space = get_param(xform_id, variation_id, 2u);
    let spin_in_4 = get_param(xform_id, variation_id, 3u);
    let spin_out_4 = get_param(xform_id, variation_id, 4u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let ax = abs(p.x);
    let ay = abs(p.y);
    var s: f32;
    var pp: f32;
    var fx: f32;
    var fy: f32;

    if (ax > w || ay > w) {
        if (ax > ay) {
            s = ax;
            if (p.x > 0.0) {
                pp = s + p.y + s * spin_out_4;
            } else {
                pp = 5.0 * s - p.y + s * spin_out_4;
            }
        } else {
            s = ay;
            if (p.y > 0.0) {
                pp = 3.0 * s - p.x + s * spin_out_4;
            } else {
                pp = 7.0 * s + p.x + s * spin_out_4;
            }
        }
        let safe_8s = max(s * 8.0, 1e-30);
        pp = pp - floor(pp / safe_8s) * safe_8s;
        let inv_s = 1.0 / max(s, 1e-30);

        var x2: f32;
        var y2: f32;
        if (pp <= 2.0 * s) {
            x2 = s + space;
            let yt = -(s - pp);
            y2 = yt + yt * inv_s * space;
        } else if (pp <= 4.0 * s) {
            y2 = s + space;
            let xt = 3.0 * s - pp;
            x2 = xt + xt * inv_s * space;
        } else if (pp <= 6.0 * s) {
            x2 = -(s + space);
            let yt = 5.0 * s - pp;
            y2 = yt + yt * inv_s * space;
        } else {
            y2 = -(s + space);
            let xt = -(7.0 * s - pp);
            x2 = xt + xt * inv_s * space;
        }
        fx = w * x2;
        fy = w * y2;
    } else {
        if (ax > ay) {
            s = ax;
            if (p.x > 0.0) {
                pp = s + p.y + s * spin_in_4;
            } else {
                pp = 5.0 * s - p.y + s * spin_in_4;
            }
        } else {
            s = ay;
            if (p.y > 0.0) {
                pp = 3.0 * s - p.x + s * spin_in_4;
            } else {
                pp = 7.0 * s + p.x + s * spin_in_4;
            }
        }
        let safe_8s = max(s * 8.0, 1e-30);
        pp = pp - floor(pp / safe_8s) * safe_8s;

        if (pp <= 2.0 * s) {
            fx = w * s;
            fy = -w * (s - pp);
        } else if (pp <= 4.0 * s) {
            fx = w * (3.0 * s - pp);
            fy = w * s;
        } else if (pp <= 6.0 * s) {
            fx = -w * s;
            fy = w * (5.0 * s - pp);
        } else {
            fx = -w * (7.0 * s - pp);
            fy = -w * s;
        }
    }
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};

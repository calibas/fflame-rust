//! w + z — Michael Faber's "Lost Variations"
//! (http://michaelfaber.deviantart.com/art/The-Lost-Variations-258913970)
//!
//! Both blend up to four shape contributions (hypergon, star,
//! lituus, super-shape) into a per-angle radius, then either:
//!
//!   - `z`: emit `(|p| + total) · w · (cos a, sin a)` — pure radial
//!     boost following the input angle
//!   - `w`: rotate input angle by `angle` to get `a2`, compute
//!     `total` (at `a`) and `total2` (at `a2`); if input radius
//!     `|p| ≤ total`, emit `total2 · |p| / total · (cos a2, sin a2)`
//!     scaled by `w`; otherwise pass input through scaled by `w`
//!
//! Slot footprints:
//!   - z: 13 user + 7 init = 20 slots
//!   - w: 14 user + 7 init = 21 slots
//!
//! Both were over the old 16-slot ceiling. Port enabled by the
//! packed-variation-params buffer.
//!
//! Shared 7-slot init derived from common params:
//!   _hypergon_d = √(1 + hypergon_r²)
//!   _hypergon_n = hypergon_n (cast)
//!   _lituus_a   = -lituus_a
//!   _star_n     = star_n (cast)
//!   _star_slope = tan(star_slope)
//!   _super_m    = super_m / 4
//!   _super_n1   = -1 / (super_n1 + ε)
//!
//! Sources: `output/jwildfire-vars/output/{w,z}.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// z — pure radial boost
// ---------------------------------------------------------------------------

/// Pure radial boost using Michael Faber's *Lost Variations* four-shape
/// blender. Computes a per-angle `total` by summing contributions from up
/// to four named shapes — `hypergon` (regular polygon with adjustable inner
/// radius), `star` (star polygon with slope-defined arms), `lituus` (polar
/// spiral `(a/π + 1)^(-lituus_a)`), and `super` (Gielis super-shape) — each
/// gated by its weight parameter (zero = disabled) and shaped by its own
/// family of sub-parameters. Output is then `(r_in + total) · (cos a, sin
/// a)`: the input point shoved outward along its own angle.
///
/// # Authors
/// - Michael Faber
pub static Z_VARIATION: VariationDef = VariationDef {
    name: "z",
    aliases: &[],
    display_name: "Z (Faber)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("hypergon", "Hypergon", unlimited_float, 0.0, -10.0, 10.0, "Weight of the hypergon (regular-polygon) shape's contribution to the per-angle radius blender. 0 disables this shape."),
        param!("hypergon_n", "Hypergon N", int, 4.0, 1.0, 50.0, "Hypergon symmetry count (number of sides). Used as the modular reduction divisor for the per-sector angle lookup."),
        param!("hypergon_r", "Hypergon R", unlimited_float, 1.0, -10.0, 10.0, "Hypergon inner-radius parameter. Combined with the per-sector angle to decide whether the shape edge is convex or hollow at that angle."),
        param!("star", "Star", unlimited_float, 0.0, -10.0, 10.0, "Weight of the star-polygon shape's contribution. 0 disables this shape."),
        param!("star_n", "Star N", int, 5.0, 1.0, 50.0, "Star symmetry count (number of points)."),
        param!("star_slope", "Star Slope", unlimited_float, 2.0, -10.0, 10.0, "Star arm slope angle (radians). Internally pre-computed as `tan(star_slope)` and used as the slope of each star arm."),
        param!("lituus", "Lituus", unlimited_float, 0.0, -10.0, 10.0, "Weight of the lituus (polar spiral) shape's contribution. 0 disables this shape."),
        param!("lituus_a", "Lituus A", unlimited_float, 1.0, -10.0, 10.0, "Lituus exponent. Internally negated; the contribution is `(a/π + 1)^(-lituus_a)` — a classical polar spiral."),
        param!("super", "Super", unlimited_float, 0.0, -10.0, 10.0, "Weight of the Gielis super-shape's contribution. 0 disables this shape."),
        param!("super_m", "Super M", unlimited_float, 1.0, -50.0, 50.0, "Super-shape symmetry count (petal count). Internally divided by 4 to feed the Gielis sin/cos formula."),
        param!("super_n1", "Super N1", unlimited_float, 1.0, -10.0, 10.0, "Super-shape outer exponent. Internally `-1/(super_n1 + ε)` is used as the radial blow-up power — small values produce sharper points."),
        param!("super_n2", "Super N2", unlimited_float, 1.0, -10.0, 10.0, "Super-shape cosine-term exponent (controls one half of the petal asymmetry)."),
        param!("super_n3", "Super N3", unlimited_float, 0.0, -10.0, 10.0, "Super-shape sine-term exponent."),
    ],
    init_param_count: 7,
    wgsl_init: Some(r#"
fn init_z(user: array<f32, 13>) -> array<f32, 7> {
    let hypergon_r = user[2];
    let hypergon_n = user[1];
    let star_n = user[4];
    let star_slope = user[5];
    let lituus_a = user[7];
    let super_m = user[9];
    let super_n1 = user[10];
    var out: array<f32, 7>;
    out[0] = sqrt(1.0 + hypergon_r * hypergon_r);   // _hypergon_d
    out[1] = hypergon_n;                             // _hypergon_n
    out[2] = -lituus_a;                              // _lituus_a
    out[3] = star_n;                                 // _star_n
    out[4] = tan(star_slope);                        // _star_slope
    out[5] = super_m * 0.25;                         // _super_m
    out[6] = -1.0 / (super_n1 + 1e-7);               // _super_n1
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn z_blend_total(a: f32,
    hypergon: f32, star: f32, lituus: f32, super_w: f32,
    super_n2: f32, super_n3: f32,
    hypergon_d: f32, hypergon_n: f32, lituus_a_neg: f32,
    star_n: f32, star_slope_t: f32, super_m4: f32, super_n1_neg: f32) -> f32 {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    var total = 0.0;

    if (hypergon != 0.0 && hypergon_n > 0.0) {
        let safe_n = max(hypergon_n, 1.0);
        let temp1 = (abs(a) - floor(abs(a) / (two_pi / safe_n)) * (two_pi / safe_n)) - pi / safe_n;
        let t = tan(temp1);
        let temp2 = t * t + 1.0;
        let h_d_sq = hypergon_d * hypergon_d;
        if (temp2 >= h_d_sq) {
            total = total + hypergon;
        } else {
            let safe_t2 = max(temp2, 1e-30);
            total = total + hypergon * (hypergon_d - sqrt(max(h_d_sq - temp2, 0.0))) / sqrt(safe_t2);
        }
    }
    if (star != 0.0 && star_n > 0.0) {
        let safe_n = max(star_n, 1.0);
        let inner = (abs(a) - floor(abs(a) / (two_pi / safe_n)) * (two_pi / safe_n)) - pi / safe_n;
        let temp1 = tan(abs(inner));
        let denom = (temp1 + star_slope_t) * (temp1 + star_slope_t);
        let safe_d = max(abs(denom), 1e-30);
        total = total + star * sqrt(star_slope_t * star_slope_t * (1.0 + temp1 * temp1) / safe_d);
    }
    if (lituus != 0.0) {
        let base = a / pi + 1.0;
        total = total + lituus * pow(max(abs(base), 1e-30), lituus_a_neg);
    }
    if (super_w != 0.0) {
        let ang = a * super_m4;
        let s = sin(ang);
        let c = cos(ang);
        let inner = pow(max(abs(c), 1e-30), super_n2) + pow(max(abs(s), 1e-30), super_n3);
        total = total + super_w * pow(max(inner, 1e-30), super_n1_neg);
    }
    return total;
}

fn variation_z(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let hypergon = get_param(xform_id, variation_id, 0u);
    let star = get_param(xform_id, variation_id, 3u);
    let lituus = get_param(xform_id, variation_id, 6u);
    let super_w = get_param(xform_id, variation_id, 8u);
    let super_n2 = get_param(xform_id, variation_id, 11u);
    let super_n3 = get_param(xform_id, variation_id, 12u);
    let hypergon_d = get_param(xform_id, variation_id, 13u);
    let hypergon_n = get_param(xform_id, variation_id, 14u);
    let lituus_a_neg = get_param(xform_id, variation_id, 15u);
    let star_n = get_param(xform_id, variation_id, 16u);
    let star_slope_t = get_param(xform_id, variation_id, 17u);
    let super_m4 = get_param(xform_id, variation_id, 18u);
    let super_n1_neg = get_param(xform_id, variation_id, 19u);

    let a = atan2(p.y, p.x);
    let r_in = sqrt(p.x * p.x + p.y * p.y);
    let total = z_blend_total(a, hypergon, star, lituus, super_w,
        super_n2, super_n3,
        hypergon_d, hypergon_n, lituus_a_neg,
        star_n, star_slope_t, super_m4, super_n1_neg);
    let r = r_in + total;
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: r#"
fn z_blend_total(a: f32,
    hypergon: f32, star: f32, lituus: f32, super_w: f32,
    super_n2: f32, super_n3: f32,
    hypergon_d: f32, hypergon_n: f32, lituus_a_neg: f32,
    star_n: f32, star_slope_t: f32, super_m4: f32, super_n1_neg: f32) -> f32 {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    var total = 0.0;

    if (hypergon != 0.0 && hypergon_n > 0.0) {
        let safe_n = max(hypergon_n, 1.0);
        let temp1 = (abs(a) - floor(abs(a) / (two_pi / safe_n)) * (two_pi / safe_n)) - pi / safe_n;
        let t = tan(temp1);
        let temp2 = t * t + 1.0;
        let h_d_sq = hypergon_d * hypergon_d;
        if (temp2 >= h_d_sq) {
            total = total + hypergon;
        } else {
            let safe_t2 = max(temp2, 1e-30);
            total = total + hypergon * (hypergon_d - sqrt(max(h_d_sq - temp2, 0.0))) / sqrt(safe_t2);
        }
    }
    if (star != 0.0 && star_n > 0.0) {
        let safe_n = max(star_n, 1.0);
        let inner = (abs(a) - floor(abs(a) / (two_pi / safe_n)) * (two_pi / safe_n)) - pi / safe_n;
        let temp1 = tan(abs(inner));
        let denom = (temp1 + star_slope_t) * (temp1 + star_slope_t);
        let safe_d = max(abs(denom), 1e-30);
        total = total + star * sqrt(star_slope_t * star_slope_t * (1.0 + temp1 * temp1) / safe_d);
    }
    if (lituus != 0.0) {
        let base = a / pi + 1.0;
        total = total + lituus * pow(max(abs(base), 1e-30), lituus_a_neg);
    }
    if (super_w != 0.0) {
        let ang = a * super_m4;
        let s = sin(ang);
        let c = cos(ang);
        let inner = pow(max(abs(c), 1e-30), super_n2) + pow(max(abs(s), 1e-30), super_n3);
        total = total + super_w * pow(max(inner, 1e-30), super_n1_neg);
    }
    return total;
}

fn variation_z(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let hypergon = get_param(xform_id, variation_id, 0u);
    let star = get_param(xform_id, variation_id, 3u);
    let lituus = get_param(xform_id, variation_id, 6u);
    let super_w = get_param(xform_id, variation_id, 8u);
    let super_n2 = get_param(xform_id, variation_id, 11u);
    let super_n3 = get_param(xform_id, variation_id, 12u);
    let hypergon_d = get_param(xform_id, variation_id, 13u);
    let hypergon_n = get_param(xform_id, variation_id, 14u);
    let lituus_a_neg = get_param(xform_id, variation_id, 15u);
    let star_n = get_param(xform_id, variation_id, 16u);
    let star_slope_t = get_param(xform_id, variation_id, 17u);
    let super_m4 = get_param(xform_id, variation_id, 18u);
    let super_n1_neg = get_param(xform_id, variation_id, 19u);

    let a = atan2(p.y, p.x);
    let r_in = sqrt(p.x * p.x + p.y * p.y);
    let total = z_blend_total(a, hypergon, star, lituus, super_w,
        super_n2, super_n3,
        hypergon_d, hypergon_n, lituus_a_neg,
        star_n, star_slope_t, super_m4, super_n1_neg);
    let r = r_in + total;
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#,
};

// ---------------------------------------------------------------------------
// w — angle-rotate-and-clip variant
// ---------------------------------------------------------------------------

/// Angle-rotated radial clip using the same four-shape blender as `z`
/// (hypergon, star, lituus, super-shape). Computes `total` at the input
/// angle `a`; if `|p| ≤ total` (the input is inside the shape), rotates the
/// angle by `angle` to get `a2`, computes `total2` at `a2`, and emits a new
/// point at radius `total2 · |p| / total` in the rotated direction `a2`.
/// Points outside the shape pass through unchanged. Effectively shears the
/// inside of the four-shape silhouette by a constant rotation.
///
/// # Authors
/// - Michael Faber
pub static W_VARIATION: VariationDef = VariationDef {
    name: "w",
    aliases: &[],
    display_name: "W (Faber)",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("angle", "Angle", unlimited_float, 0.0, -10.0, 10.0, "Rotation angle (radians) applied to the input angle before the radial rebake. The output direction is `a + angle` (wrapped into `[-π, π]`)."),
        param!("hypergon", "Hypergon", unlimited_float, 0.0, -10.0, 10.0, "Weight of the hypergon (regular-polygon) shape's contribution to the per-angle radius blender. 0 disables this shape."),
        param!("hypergon_n", "Hypergon N", int, 4.0, 1.0, 50.0, "Hypergon symmetry count (number of sides). Used as the modular reduction divisor for the per-sector angle lookup."),
        param!("hypergon_r", "Hypergon R", unlimited_float, 1.0, -10.0, 10.0, "Hypergon inner-radius parameter. Combined with the per-sector angle to decide whether the shape edge is convex or hollow at that angle."),
        param!("star", "Star", unlimited_float, 0.0, -10.0, 10.0, "Weight of the star-polygon shape's contribution. 0 disables this shape."),
        param!("star_n", "Star N", int, 5.0, 1.0, 50.0, "Star symmetry count (number of points)."),
        param!("star_slope", "Star Slope", unlimited_float, 2.0, -10.0, 10.0, "Star arm slope angle (radians). Internally pre-computed as `tan(star_slope)` and used as the slope of each star arm."),
        param!("lituus", "Lituus", unlimited_float, 0.0, -10.0, 10.0, "Weight of the lituus (polar spiral) shape's contribution. 0 disables this shape."),
        param!("lituus_a", "Lituus A", unlimited_float, 1.0, -10.0, 10.0, "Lituus exponent. Internally negated; the contribution is `(a/π + 1)^(-lituus_a)` — a classical polar spiral."),
        param!("super", "Super", unlimited_float, 0.0, -10.0, 10.0, "Weight of the Gielis super-shape's contribution. 0 disables this shape."),
        param!("super_m", "Super M", unlimited_float, 1.0, -50.0, 50.0, "Super-shape symmetry count (petal count). Internally divided by 4 to feed the Gielis sin/cos formula."),
        param!("super_n1", "Super N1", unlimited_float, 1.0, -10.0, 10.0, "Super-shape outer exponent. Internally `-1/(super_n1 + ε)` is used as the radial blow-up power — small values produce sharper points."),
        param!("super_n2", "Super N2", unlimited_float, 1.0, -10.0, 10.0, "Super-shape cosine-term exponent (controls one half of the petal asymmetry)."),
        param!("super_n3", "Super N3", unlimited_float, 0.0, -10.0, 10.0, "Super-shape sine-term exponent."),
    ],
    init_param_count: 7,
    wgsl_init: Some(r#"
fn init_w(user: array<f32, 14>) -> array<f32, 7> {
    let hypergon_r = user[3];
    let hypergon_n = user[2];
    let star_n = user[5];
    let star_slope = user[6];
    let lituus_a = user[8];
    let super_m = user[10];
    let super_n1 = user[11];
    var out: array<f32, 7>;
    out[0] = sqrt(1.0 + hypergon_r * hypergon_r);   // _hypergon_d
    out[1] = hypergon_n;                             // _hypergon_n
    out[2] = -lituus_a;                              // _lituus_a
    out[3] = star_n;                                 // _star_n
    out[4] = tan(star_slope);                        // _star_slope
    out[5] = super_m * 0.25;                         // _super_m
    out[6] = -1.0 / (super_n1 + 1e-7);               // _super_n1
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn w_blend_total(a: f32,
    hypergon: f32, star: f32, lituus: f32, super_w: f32,
    super_n2: f32, super_n3: f32,
    hypergon_d: f32, hypergon_n: f32, lituus_a_neg: f32,
    star_n: f32, star_slope_t: f32, super_m4: f32, super_n1_neg: f32) -> f32 {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    var total = 0.0;

    if (hypergon != 0.0 && hypergon_n > 0.0) {
        let safe_n = max(hypergon_n, 1.0);
        let temp1 = (abs(a) - floor(abs(a) / (two_pi / safe_n)) * (two_pi / safe_n)) - pi / safe_n;
        let t = tan(temp1);
        let temp2 = t * t + 1.0;
        let h_d_sq = hypergon_d * hypergon_d;
        if (temp2 >= h_d_sq) {
            total = total + hypergon;
        } else {
            let safe_t2 = max(temp2, 1e-30);
            total = total + hypergon * (hypergon_d - sqrt(max(h_d_sq - temp2, 0.0))) / sqrt(safe_t2);
        }
    }
    if (star != 0.0 && star_n > 0.0) {
        let safe_n = max(star_n, 1.0);
        let inner = (abs(a) - floor(abs(a) / (two_pi / safe_n)) * (two_pi / safe_n)) - pi / safe_n;
        let temp1 = tan(abs(inner));
        let denom = (temp1 + star_slope_t) * (temp1 + star_slope_t);
        let safe_d = max(abs(denom), 1e-30);
        total = total + star * sqrt(star_slope_t * star_slope_t * (1.0 + temp1 * temp1) / safe_d);
    }
    if (lituus != 0.0) {
        let base = a / pi + 1.0;
        total = total + lituus * pow(max(abs(base), 1e-30), lituus_a_neg);
    }
    if (super_w != 0.0) {
        let ang = a * super_m4;
        let s = sin(ang);
        let c = cos(ang);
        let inner = pow(max(abs(c), 1e-30), super_n2) + pow(max(abs(s), 1e-30), super_n3);
        total = total + super_w * pow(max(inner, 1e-30), super_n1_neg);
    }
    return total;
}

fn variation_w(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let angle = get_param(xform_id, variation_id, 0u);
    let hypergon = get_param(xform_id, variation_id, 1u);
    let star = get_param(xform_id, variation_id, 4u);
    let lituus = get_param(xform_id, variation_id, 7u);
    let super_w = get_param(xform_id, variation_id, 9u);
    let super_n2 = get_param(xform_id, variation_id, 12u);
    let super_n3 = get_param(xform_id, variation_id, 13u);
    let hypergon_d = get_param(xform_id, variation_id, 14u);
    let hypergon_n = get_param(xform_id, variation_id, 15u);
    let lituus_a_neg = get_param(xform_id, variation_id, 16u);
    let star_n = get_param(xform_id, variation_id, 17u);
    let star_slope_t = get_param(xform_id, variation_id, 18u);
    let super_m4 = get_param(xform_id, variation_id, 19u);
    let super_n1_neg = get_param(xform_id, variation_id, 20u);
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;

    let a = atan2(p.y, p.x);
    let r_in = sqrt(p.x * p.x + p.y * p.y);
    var a2 = a + angle;
    if (a2 < -pi) { a2 = a2 + two_pi; }
    if (a2 > pi) { a2 = a2 - two_pi; }

    let total = w_blend_total(a, hypergon, star, lituus, super_w,
        super_n2, super_n3,
        hypergon_d, hypergon_n, lituus_a_neg,
        star_n, star_slope_t, super_m4, super_n1_neg);

    if (r_in <= total) {
        let total2 = w_blend_total(a2, hypergon, star, lituus, super_w,
            super_n2, super_n3,
            hypergon_d, hypergon_n, lituus_a_neg,
            star_n, star_slope_t, super_m4, super_n1_neg);
        let safe_total = select(total, 1e-30, abs(total) < 1e-30);
        let r = total2 * r_in / safe_total;
        return vec2<f32>(r * cos(a2), r * sin(a2));
    }
    return p;
}
"#,
    wgsl_3d: r#"
fn w_blend_total(a: f32,
    hypergon: f32, star: f32, lituus: f32, super_w: f32,
    super_n2: f32, super_n3: f32,
    hypergon_d: f32, hypergon_n: f32, lituus_a_neg: f32,
    star_n: f32, star_slope_t: f32, super_m4: f32, super_n1_neg: f32) -> f32 {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    var total = 0.0;

    if (hypergon != 0.0 && hypergon_n > 0.0) {
        let safe_n = max(hypergon_n, 1.0);
        let temp1 = (abs(a) - floor(abs(a) / (two_pi / safe_n)) * (two_pi / safe_n)) - pi / safe_n;
        let t = tan(temp1);
        let temp2 = t * t + 1.0;
        let h_d_sq = hypergon_d * hypergon_d;
        if (temp2 >= h_d_sq) {
            total = total + hypergon;
        } else {
            let safe_t2 = max(temp2, 1e-30);
            total = total + hypergon * (hypergon_d - sqrt(max(h_d_sq - temp2, 0.0))) / sqrt(safe_t2);
        }
    }
    if (star != 0.0 && star_n > 0.0) {
        let safe_n = max(star_n, 1.0);
        let inner = (abs(a) - floor(abs(a) / (two_pi / safe_n)) * (two_pi / safe_n)) - pi / safe_n;
        let temp1 = tan(abs(inner));
        let denom = (temp1 + star_slope_t) * (temp1 + star_slope_t);
        let safe_d = max(abs(denom), 1e-30);
        total = total + star * sqrt(star_slope_t * star_slope_t * (1.0 + temp1 * temp1) / safe_d);
    }
    if (lituus != 0.0) {
        let base = a / pi + 1.0;
        total = total + lituus * pow(max(abs(base), 1e-30), lituus_a_neg);
    }
    if (super_w != 0.0) {
        let ang = a * super_m4;
        let s = sin(ang);
        let c = cos(ang);
        let inner = pow(max(abs(c), 1e-30), super_n2) + pow(max(abs(s), 1e-30), super_n3);
        total = total + super_w * pow(max(inner, 1e-30), super_n1_neg);
    }
    return total;
}

fn variation_w(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let angle = get_param(xform_id, variation_id, 0u);
    let hypergon = get_param(xform_id, variation_id, 1u);
    let star = get_param(xform_id, variation_id, 4u);
    let lituus = get_param(xform_id, variation_id, 7u);
    let super_w = get_param(xform_id, variation_id, 9u);
    let super_n2 = get_param(xform_id, variation_id, 12u);
    let super_n3 = get_param(xform_id, variation_id, 13u);
    let hypergon_d = get_param(xform_id, variation_id, 14u);
    let hypergon_n = get_param(xform_id, variation_id, 15u);
    let lituus_a_neg = get_param(xform_id, variation_id, 16u);
    let star_n = get_param(xform_id, variation_id, 17u);
    let star_slope_t = get_param(xform_id, variation_id, 18u);
    let super_m4 = get_param(xform_id, variation_id, 19u);
    let super_n1_neg = get_param(xform_id, variation_id, 20u);
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;

    let a = atan2(p.y, p.x);
    let r_in = sqrt(p.x * p.x + p.y * p.y);
    var a2 = a + angle;
    if (a2 < -pi) { a2 = a2 + two_pi; }
    if (a2 > pi) { a2 = a2 - two_pi; }

    let total = w_blend_total(a, hypergon, star, lituus, super_w,
        super_n2, super_n3,
        hypergon_d, hypergon_n, lituus_a_neg,
        star_n, star_slope_t, super_m4, super_n1_neg);

    if (r_in <= total) {
        let total2 = w_blend_total(a2, hypergon, star, lituus, super_w,
            super_n2, super_n3,
            hypergon_d, hypergon_n, lituus_a_neg,
            star_n, star_slope_t, super_m4, super_n1_neg);
        let safe_total = select(total, 1e-30, abs(total) < 1e-30);
        let r = total2 * r_in / safe_total;
        return vec3<f32>(r * cos(a2), r * sin(a2), p.z);
    }
    return p;
}
"#,
};

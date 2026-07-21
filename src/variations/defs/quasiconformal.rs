//! `quasiconformal` — exact quasiconformal warp via a log-affine map
//! (original).
//!
//! A quasiconformal map sends infinitesimal circles to infinitesimal
//! ellipses of bounded eccentricity; it is characterized by its
//! Beltrami coefficient `μ = f_z̄ / f_z` (`|μ| < 1`), with dilatation
//! `K = (1+|μ|)/(1−|μ|)`. Solving for `f` from an arbitrary `μ` (the
//! Measurable Riemann Mapping Theorem) is a *global* integral operator,
//! not a pointwise formula — so this variation uses the exact
//! closed-form family that is real-affine in the complex-log
//! coordinate `ℓ = log z`:
//!
//! ```text
//! f(z) = exp( a·log z + μ·conj(log z) )
//! ```
//!
//! whose Beltrami coefficient is `μ_f(z) = (μ/a)·e^{2iθ}`: constant
//! MODULUS `|μ/a|` everywhere (uniform dilatation — a Teichmüller-type
//! extremal map) with an orientation that rotates once around the
//! plane. `a = power + i·spiral` is the conformal part (`z ↦ z^a`: a
//! radial power and a radius↔angle spiral); `μ = a·k·e^{iψ}` fixes the
//! quasiconformal dilation to `|μ/a| = k = dilation` for any `a`, with
//! `ψ = stretch_angle` the direction of maximal stretch. At
//! `dilation = 0` it is a plain conformal power-spiral; raising it
//! introduces the anisotropic elliptical distortion that IS
//! quasiconformality (K = (1+k)/(1−k)).
//!
//! Working in log-polar, with `L = log|z−c|`, `Θ = atan2`:
//! `Re ω = (aᵣ+μᵣ)L + (μᵢ−aᵢ)Θ`,
//! `Im ω = (aᵢ+μᵢ)L + (aᵣ−μᵣ)Θ`, and `f = c + e^{Re ω}·e^{i·Im ω}`.
//!
//! Fundamentally a 2D complex map; the 3D body applies it in the xy
//! plane and passes z through. No JWildfire/Apophysis counterpart —
//! original to this project.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static QUASICONFORMAL: VariationDef = VariationDef {
    name: "quasiconformal",
    aliases: &["qconformal"],
    display_name: "Quasiconformal",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("power", "Power", unlimited_float, 1.0, -4.0, 4.0, "Real part of the conformal exponent a — the radial power (z ↦ z^power). 1 is the identity radial scale; 2 squares the modulus, 0.5 takes its root; negative values invert."),
        param!("spiral", "Spiral", unlimited_float, 0.0, -4.0, 4.0, "Imaginary part of a — couples radius into angle, so the map spirals (the logarithmic-spiral part of z ↦ z^a). 0 = no spiral."),
        param!("dilation", "Dilation", float, 0.35, 0.0, 0.98, "The quasiconformal strength k = |μ/a|, the constant Beltrami modulus. 0 = conformal (a pure power-spiral, circles stay circles); higher values squash circles into ellipses of dilatation K = (1+k)/(1−k). Clamped below 1 (k→1 is the degenerate non-invertible limit)."),
        param!("stretch_angle", "Stretch Angle", angle, 0.0, "Orientation ψ of the quasiconformal stretch (arg of μ/a), in degrees — the direction along which infinitesimal circles are stretched into ellipses. The ellipse axis itself rotates around the plane (µ_f carries an e^{2iθ} factor); this sets its phase at angle 0."),
        param!("cx", "Center X", unlimited_float, 0.0, -2.0, 2.0, "X of the fixed center: the map's log is taken about (cx, cy), and the result is offset back by it. The center and infinity are the two fixed points."),
        param!("cy", "Center Y", unlimited_float, 0.0, -2.0, 2.0, "Y of the fixed center."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_quasiconformal(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let spiral = get_param(xform_id, variation_id, 1u);
    let k = clamp(get_param(xform_id, variation_id, 2u), 0.0, 0.98);
    let psi = get_param(xform_id, variation_id, 3u) * 0.01745329252;
    let cx = get_param(xform_id, variation_id, 4u);
    let cy = get_param(xform_id, variation_id, 5u);

    let zx = p.x - cx;
    let zy = p.y - cy;
    let big_l = 0.5 * log(max(zx * zx + zy * zy, 1e-20)); // log|z-c|
    let theta = atan2(zy, zx);

    // mu = a * k * e^{i psi}, so |mu/a| = k exactly for any a.
    let cps = cos(psi);
    let sps = sin(psi);
    let mr = k * (power * cps - spiral * sps);
    let mi = k * (power * sps + spiral * cps);

    // omega = a*L + mu*conj(L), L = big_l + i*theta.
    let re_omega = (power + mr) * big_l + (mi - spiral) * theta;
    let im_omega = (spiral + mi) * big_l + (power - mr) * theta;

    let rr = exp(re_omega);
    return vec2<f32>(cx + rr * cos(im_omega), cy + rr * sin(im_omega));
}
"#;

const WGSL_3D: &str = r#"
fn variation_quasiconformal(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let spiral = get_param(xform_id, variation_id, 1u);
    let k = clamp(get_param(xform_id, variation_id, 2u), 0.0, 0.98);
    let psi = get_param(xform_id, variation_id, 3u) * 0.01745329252;
    let cx = get_param(xform_id, variation_id, 4u);
    let cy = get_param(xform_id, variation_id, 5u);

    let zx = p.x - cx;
    let zy = p.y - cy;
    let big_l = 0.5 * log(max(zx * zx + zy * zy, 1e-20));
    let theta = atan2(zy, zx);

    let cps = cos(psi);
    let sps = sin(psi);
    let mr = k * (power * cps - spiral * sps);
    let mi = k * (power * sps + spiral * cps);

    let re_omega = (power + mr) * big_l + (mi - spiral) * theta;
    let im_omega = (spiral + mi) * big_l + (power - mr) * theta;

    let rr = exp(re_omega);
    return vec3<f32>(cx + rr * cos(im_omega), cy + rr * sin(im_omega), p.z);
}
"#;

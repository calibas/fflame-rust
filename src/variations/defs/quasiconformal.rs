//! `quasiconformal` — quasiconformal warp: exact log-affine map or a
//! spatially-varying Beltrami field (original).
//!
//! A quasiconformal map sends infinitesimal circles to infinitesimal
//! ellipses of bounded eccentricity; it is characterized by its
//! Beltrami coefficient `μ = f_z̄ / f_z` (`|μ| < 1`), with dilatation
//! `K = (1+|μ|)/(1−|μ|)`. Solving for `f` from an arbitrary `μ` (the
//! Measurable Riemann Mapping Theorem) is a *global* integral operator,
//! not a pointwise formula — so this variation offers two pointwise
//! constructions selected by `mode`:
//!
//! **Exact** — the closed-form family that is real-affine in the
//! complex-log coordinate `ℓ = log z`:
//!
//! ```text
//! f(z) = exp( a·log z + μ·conj(log z) )
//! ```
//!
//! a *genuine global* quasiconformal map, with Beltrami coefficient
//! `μ_f(z) = (μ/a)·e^{2iθ}`: constant MODULUS `|μ/a|` everywhere
//! (uniform dilatation — a Teichmüller-type extremal map) with an
//! orientation that rotates once around the plane. `a = power +
//! i·spiral` is the conformal part (`z ↦ z^a`: a radial power and a
//! radius↔angle spiral); `μ = a·k·e^{iψ}` fixes `|μ/a| = k = dilation`
//! for any `a`, with `ψ = stretch_angle` the direction of maximal
//! stretch. At `dilation = 0` it is a plain conformal power-spiral;
//! raising it introduces the anisotropic elliptical distortion that IS
//! quasiconformality (K = (1+k)/(1−k)).
//!
//! **Field** — the pointwise Beltrami *deformation*
//! `f(z) = z + μ(z)·conj(z−c)` with a spatially-varying coefficient
//! `μ(z) = k·e^{i(power·θ + spiral·L + ψ)}` (here `power`/`spiral`
//! reinterpreted as the angular/radial winding of the stretch
//! orientation, `L = log|z−c|`). Each point still maps circles to
//! ellipses of dilatation `≈K`, but with an orientation field that
//! swirls — this is *not* an exact global QC map (its μ has varying
//! argument, so it does not solve any single Beltrami equation
//! globally), just the honest local deformation, which for an iterated
//! chaos game reads as a modulated anisotropic swirl.
//!
//! Direct color (`dc_mode`): *Orientation* keys the palette to the
//! local ellipse-axis angle (the QC-specific quantity — `arg(μ_f)`),
//! *Phase* to the accumulated spiral phase of the output.
//!
//! Fundamentally a 2D complex map; the 3D body applies it in the xy
//! plane and passes z through. No JWildfire/Apophysis counterpart —
//! original to this project.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Quasiconformal warp: an exact log-affine map or a spatially-varying
/// Beltrami field.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static QUASICONFORMAL: VariationDef = VariationDef {
    name: "quasiconformal",
    aliases: &["qconformal"],
    display_name: "Quasiconformal",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("power", "Power", unlimited_float, 1.0, -4.0, 4.0, "Exact mode: real part of the conformal exponent a — the radial power (z ↦ z^power). Field mode: angular winding of the stretch orientation (how many times the ellipse axis turns going once around the center)."),
        param!("spiral", "Spiral", unlimited_float, 0.0, -4.0, 4.0, "Exact mode: imaginary part of a — couples radius into angle so the map spirals (the logarithmic-spiral part of z ↦ z^a). Field mode: radial winding of the stretch orientation (turns per e-fold of radius)."),
        param!("dilation", "Dilation", float, 0.35, 0.0, 0.98, "The quasiconformal strength k = |μ/a|, the Beltrami modulus. 0 = conformal (a pure power-spiral, circles stay circles); higher values squash circles into ellipses of dilatation K = (1+k)/(1−k). Clamped below 1 (k→1 is the degenerate non-invertible limit)."),
        param!("stretch_angle", "Stretch Angle", angle, 0.0, "Base orientation ψ of the quasiconformal stretch, in degrees — the direction along which infinitesimal circles are stretched into ellipses (the axis rotates around the plane on top of this base)."),
        param!("cx", "Center X", unlimited_float, 0.0, -2.0, 2.0, "X of the fixed center: the map is taken about (cx, cy). In Exact mode the center and infinity are the two fixed points."),
        param!("cy", "Center Y", unlimited_float, 0.0, -2.0, 2.0, "Y of the fixed center."),
        param!("mode", "Mode", enum, 0, &["Exact", "Field"], "Exact: the closed-form log-affine map — a genuine global quasiconformal map with uniform dilatation (Teichmüller-type). Field: the pointwise Beltrami deformation with a spatially-varying stretch orientation (power/spiral become its angular/radial winding) — a locally-QC swirl, not an exact global map."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Orientation", "Phase"], "Direct-color source (needs the transform's Direct Color > 0). Orientation: palette by the local ellipse-axis angle (arg of the Beltrami coefficient) — the QC-specific quantity. Phase: palette by the accumulated spiral phase of the output."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the color modes (wrapped with fract)."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_quasiconformal(p: vec2<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec2<f32> {
    let qc_in = p;

    let power = get_param(xform_id, variation_id, 0u);
    let spiral = get_param(xform_id, variation_id, 1u);
    let k = clamp(get_param(xform_id, variation_id, 2u), 0.0, 0.98);
    let psi = get_param(xform_id, variation_id, 3u) * 0.01745329252;
    let cx = get_param(xform_id, variation_id, 4u);
    let cy = get_param(xform_id, variation_id, 5u);
    let mode = u32(get_param(xform_id, variation_id, 6u));
    let dc_mode = u32(get_param(xform_id, variation_id, 7u));
    let dc_scale = get_param(xform_id, variation_id, 8u);

    let zx = qc_in.x - cx;
    let zy = qc_in.y - cy;
    let big_l = 0.5 * log(max(zx * zx + zy * zy, 1e-20)); // log|z-c|
    let theta = atan2(zy, zx);

    var outx: f32;
    var outy: f32;
    var beta: f32;      // Beltrami argument (Orientation color)
    var phase_out: f32; // output spiral phase (Phase color)
    if (mode == 0u) {
        // Exact: mu = a * k * e^{i psi}, |mu/a| = k for any a.
        let cps = cos(psi);
        let sps = sin(psi);
        let mr = k * (power * cps - spiral * sps);
        let mi = k * (power * sps + spiral * cps);
        // omega = a*L + mu*conj(L), L = big_l + i*theta.
        let re_omega = (power + mr) * big_l + (mi - spiral) * theta;
        let im_omega = (spiral + mi) * big_l + (power - mr) * theta;
        let rr = exp(re_omega);
        outx = cx + rr * cos(im_omega);
        outy = cy + rr * sin(im_omega);
        beta = psi + 2.0 * theta;   // arg(mu_f) = arg(mu/a) + 2 theta
        phase_out = im_omega;
    } else {
        // Field: mu(z) = k e^{i(power*theta + spiral*L + psi)},
        // f = c + w + mu*conj(w).
        let ph = power * theta + spiral * big_l + psi;
        let mur = k * cos(ph);
        let mui = k * sin(ph);
        outx = cx + zx + (mur * zx + mui * zy);
        outy = cy + zy + (mui * zx - mur * zy);
        beta = ph;
        phase_out = atan2(outy - cy, outx - cx);
    }

    if (dc_mode == 1u) {
        *vc = fract(beta * 0.15915494309 * dc_scale);      // /(2 pi)
    } else if (dc_mode == 2u) {
        *vc = fract(phase_out * 0.15915494309 * dc_scale);
    }
    return vec2<f32>(outx, outy);
}
"#;

const WGSL_3D: &str = r#"
fn variation_quasiconformal(p: vec3<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec3<f32> {
    let qc_in = p.xy;

    let power = get_param(xform_id, variation_id, 0u);
    let spiral = get_param(xform_id, variation_id, 1u);
    let k = clamp(get_param(xform_id, variation_id, 2u), 0.0, 0.98);
    let psi = get_param(xform_id, variation_id, 3u) * 0.01745329252;
    let cx = get_param(xform_id, variation_id, 4u);
    let cy = get_param(xform_id, variation_id, 5u);
    let mode = u32(get_param(xform_id, variation_id, 6u));
    let dc_mode = u32(get_param(xform_id, variation_id, 7u));
    let dc_scale = get_param(xform_id, variation_id, 8u);

    let zx = qc_in.x - cx;
    let zy = qc_in.y - cy;
    let big_l = 0.5 * log(max(zx * zx + zy * zy, 1e-20)); // log|z-c|
    let theta = atan2(zy, zx);

    var outx: f32;
    var outy: f32;
    var beta: f32;      // Beltrami argument (Orientation color)
    var phase_out: f32; // output spiral phase (Phase color)
    if (mode == 0u) {
        // Exact: mu = a * k * e^{i psi}, |mu/a| = k for any a.
        let cps = cos(psi);
        let sps = sin(psi);
        let mr = k * (power * cps - spiral * sps);
        let mi = k * (power * sps + spiral * cps);
        // omega = a*L + mu*conj(L), L = big_l + i*theta.
        let re_omega = (power + mr) * big_l + (mi - spiral) * theta;
        let im_omega = (spiral + mi) * big_l + (power - mr) * theta;
        let rr = exp(re_omega);
        outx = cx + rr * cos(im_omega);
        outy = cy + rr * sin(im_omega);
        beta = psi + 2.0 * theta;   // arg(mu_f) = arg(mu/a) + 2 theta
        phase_out = im_omega;
    } else {
        // Field: mu(z) = k e^{i(power*theta + spiral*L + psi)},
        // f = c + w + mu*conj(w).
        let ph = power * theta + spiral * big_l + psi;
        let mur = k * cos(ph);
        let mui = k * sin(ph);
        outx = cx + zx + (mur * zx + mui * zy);
        outy = cy + zy + (mui * zx - mur * zy);
        beta = ph;
        phase_out = atan2(outy - cy, outx - cx);
    }

    if (dc_mode == 1u) {
        *vc = fract(beta * 0.15915494309 * dc_scale);      // /(2 pi)
    } else if (dc_mode == 2u) {
        *vc = fract(phase_out * 0.15915494309 * dc_scale);
    }
    return vec3<f32>(outx, outy, p.z);
}
"#;

//! synth (slobo777, http://slobo777.deviantart.com/art/Synth-V2-128594088)
//!
//! 26-mode "synthesizer" variation that distorts position / radius /
//! angle by a 6-layer additive wave function (synth_value). Each
//! layer (a + b + c + d + e + f, with `a` being a constant offset
//! and b..f independently configurable) blends a chosen waveform
//! (sin / cos / square / saw / triangle / concave / convex / ngon /
//! ingon) with skew, frequency, and phase controls, then combines
//! into the running synth value via add / multiply / max / min.
//!
//! Mode IDs are deliberately non-contiguous in JWildfire: the
//! "first generation" 2D modes are 0..19, and the "wave-smoothing"
//! variants are 1001..1007. We pass the raw int through so .flame
//! XML round-trip works without translation; the body uses an
//! if/else cascade in mode order.
//!
//! 35 user params, 0 init slots (everything is computed per
//! iteration so live UI edits propagate correctly).
//!
//! Source: `output/variation-jwf-source/SynthFunc.java`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// 26-mode "synthesizer" variation by slobo777 — combines 6 wave
/// layers (constant `a` + amplitudes `b..f`) into a `theta_factor`
/// scalar that drives one of many position / radius / angle
/// distortions. The mode picks WHICH distortion (e.g. swirl,
/// julia, rings, mirrors, blur variants, raw axis remaps); the
/// 6-layer waveform stack picks the SHAPE of the distortion.
///
/// Each layer b..f has six tunable knobs:
///   - amplitude (the param's bare name)
///   - `*_type`: which waveform (sin / cos / square / saw / triangle
///     / concave / convex / ngon / ingon)
///   - `*_skew`: how asymmetric the wave is (0 = symmetric, ±1 =
///     fully skewed)
///   - `*_frq`: angular frequency
///   - `*_phs`: phase offset
///   - `*_layer`: how this layer combines with the running
///     `theta_factor` (add / multiply / max / min)
///
/// Globals: `a` is the base offset, `mode` picks the distortion,
/// `power` is consumed by power-using modes, `mix` participates in
/// the SINCOS_MIXIN path, `smooth` is dual-purpose — interpolation
/// type for "Smooth YES" modes (0 = linear, 1 = Bezier-quad), or
/// sincos interpretation for "Smooth WAVE" modes (0 = multiply,
/// 1 = mixin).
///
/// # Authors
/// - slobo777
pub static SYNTH: VariationDef = VariationDef {
    name: "synth",
    aliases: &[],
    display_name: "Synth",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        // Globals (slots 0..4)
        param!("a", "a", unlimited_float, 1.0, -10.0, 10.0, "Constant offset added at the start of every synth_value evaluation. Acts as the baseline `theta_factor` that layers b..f modulate."),
        param!("mode", "Mode", unlimited_int, 3.0, 0.0, 1007.0, "Which distortion to apply. 0–19 are the original 2D modes (spherical / bubble / blur / raw / shift / mirror variants); 1001–1007 are the 'wave-smoothing' modes (sinusoidal / swirl / hyperbolic / julia / disc / rings / cylinder). The IDs are non-contiguous on purpose — they match JWildfire's file format."),
        param!("power", "Power", unlimited_float, -2.0, -10.0, 10.0, "Power exponent consumed by power-using modes (spherical, blur, shifts, etc.). For radial-power modes the exponent applied is `power / 2`; for the rings mode it gates the radial stripe width."),
        param!("mix", "Mix", unlimited_float, 1.0, -10.0, 10.0, "Mix factor for SINCOS_MIXIN path. Affects how the sin/cos of `theta` is blended with the synth-modified value."),
        param!("smooth", "Smooth", unlimited_int, 0.0, 0.0, 1.0, "Dual-purpose flag depending on the mode family: for 'Smooth YES' modes it picks the interpolation between the input radius and the synth target (0 = linear, 1 = Bezier-quad); for 'Smooth WAVE' modes it picks the synthsincos interpretation (0 = multiply, 1 = mixin)."),
        // Layer b (slots 5..10)
        param!("b", "b", unlimited_float, 0.0, -10.0, 10.0, "Amplitude of the second synth layer. 0 disables the layer entirely (the whole block short-circuits in the body)."),
        param!("b_type", "b_type", unlimited_int, 0.0, 0.0, 8.0, "Waveform for layer b. 0=sin, 1=cos, 2=square, 3=saw, 4=triangle, 5=concave, 6=convex, 7=ngon (1/cos-1 style), 8=ingon (inverse ngon)."),
        param!("b_skew", "b_skew", unlimited_float, 0.0, -1.0, 1.0, "Asymmetry of layer b's waveform. 0 keeps the half-period symmetric; positive squeezes the peak into the second half, negative into the first."),
        param!("b_frq", "b_frq", unlimited_float, 1.0, -100.0, 100.0, "Angular frequency of layer b — the input theta gets multiplied by this before the phase shift. Higher = more wiggles per revolution."),
        param!("b_phs", "b_phs", unlimited_float, 0.0, -10.0, 10.0, "Phase offset of layer b (radians, added to `theta * b_frq`)."),
        param!("b_layer", "b_layer", unlimited_int, 0.0, 0.0, 3.0, "How layer b combines with the running theta_factor. 0=add (a + b*x), 1=multiply (factor *= 1 + b*x), 2=max(factor, a + b*x), 3=min(factor, a + b*x)."),
        // Layer c (slots 11..16)
        param!("c", "c", unlimited_float, 0.0, -10.0, 10.0, "Amplitude of layer c. See `b` for shape semantics."),
        param!("c_type", "c_type", unlimited_int, 0.0, 0.0, 8.0, "Waveform for layer c. Same encoding as `b_type`."),
        param!("c_skew", "c_skew", unlimited_float, 0.0, -1.0, 1.0, "Asymmetry of layer c — see `b_skew`."),
        param!("c_frq", "c_frq", unlimited_float, 1.0, -100.0, 100.0, "Angular frequency of layer c — see `b_frq`."),
        param!("c_phs", "c_phs", unlimited_float, 0.0, -10.0, 10.0, "Phase offset of layer c — see `b_phs`."),
        param!("c_layer", "c_layer", unlimited_int, 0.0, 0.0, 3.0, "Combination operator for layer c — see `b_layer`."),
        // Layer d (slots 17..22)
        param!("d", "d", unlimited_float, 0.0, -10.0, 10.0, "Amplitude of layer d. See `b`."),
        param!("d_type", "d_type", unlimited_int, 0.0, 0.0, 8.0, "Waveform for layer d. Same encoding as `b_type`."),
        param!("d_skew", "d_skew", unlimited_float, 0.0, -1.0, 1.0, "Asymmetry of layer d."),
        param!("d_frq", "d_frq", unlimited_float, 1.0, -100.0, 100.0, "Angular frequency of layer d."),
        param!("d_phs", "d_phs", unlimited_float, 0.0, -10.0, 10.0, "Phase offset of layer d."),
        param!("d_layer", "d_layer", unlimited_int, 0.0, 0.0, 3.0, "Combination operator for layer d."),
        // Layer e (slots 23..28)
        param!("e", "e", unlimited_float, 0.0, -10.0, 10.0, "Amplitude of layer e."),
        param!("e_type", "e_type", unlimited_int, 0.0, 0.0, 8.0, "Waveform for layer e."),
        param!("e_skew", "e_skew", unlimited_float, 0.0, -1.0, 1.0, "Asymmetry of layer e."),
        param!("e_frq", "e_frq", unlimited_float, 1.0, -100.0, 100.0, "Angular frequency of layer e."),
        param!("e_phs", "e_phs", unlimited_float, 0.0, -10.0, 10.0, "Phase offset of layer e."),
        param!("e_layer", "e_layer", unlimited_int, 0.0, 0.0, 3.0, "Combination operator for layer e."),
        // Layer f (slots 29..34)
        param!("f", "f", unlimited_float, 0.0, -10.0, 10.0, "Amplitude of layer f."),
        param!("f_type", "f_type", unlimited_int, 0.0, 0.0, 8.0, "Waveform for layer f."),
        param!("f_skew", "f_skew", unlimited_float, 0.0, -1.0, 1.0, "Asymmetry of layer f."),
        param!("f_frq", "f_frq", unlimited_float, 1.0, -100.0, 100.0, "Angular frequency of layer f."),
        param!("f_phs", "f_phs", unlimited_float, 0.0, -10.0, 10.0, "Phase offset of layer f."),
        param!("f_layer", "f_layer", unlimited_int, 0.0, 0.0, 3.0, "Combination operator for layer f."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    // 2D body. The 3D body just appends `p.z` passthrough — synth is
    // a planar variation in JWildfire and we follow suit.
    wgsl_2d: r#"
// One layer of synth_value (called 5 times — for b, c, d, e, f).
// Skips the work when the layer's amplitude is zero (matches cpp's
// `if (b != 0.0)` short-circuit so disabled layers cost nothing).
fn synth_apply_layer(theta: f32, theta_factor_in: f32, base_a: f32,
                     amp: f32, layer_type: i32, skew: f32, frq: f32,
                     phs: f32, layer_op: i32) -> f32 {
    if (amp == 0.0) { return theta_factor_in; }
    const TWO_PI: f32 = 6.283185307179586;
    const SYNTH_EPS: f32 = 1e-20;

    var z = phs + theta * frq;
    var y = z / TWO_PI;
    y = y - floor(y);  // y now in [0, 1)

    // Skew: reshape the half-period split. skew=0 → y stays
    // symmetric; positive squeezes the peak to the right half,
    // negative to the left.
    if (skew != 0.0) {
        let z_split = 0.5 + 0.5 * skew;
        if (y > z_split) {
            y = 0.5 + 0.5 * (y - z_split) / (1.0 - z_split + SYNTH_EPS);
        } else {
            y = 0.5 - 0.5 * (z_split - y) / (z_split + SYNTH_EPS);
        }
    }

    // Waveform — 9 shapes, x ends up in [-1, 1] (roughly).
    var x: f32 = 0.0;
    switch (layer_type) {
    case 0: { x = sin(y * TWO_PI); }
    case 1: { x = cos(y * TWO_PI); }
    case 2: { x = select(-1.0, 1.0, y > 0.5); }
    case 3: { x = 1.0 - 2.0 * y; }
    case 4: { x = select(2.0 * y - 1.0, 3.0 - 4.0 * y, y > 0.5); }
    case 5: { x = 8.0 * (y - 0.5) * (y - 0.5) - 1.0; }
    case 6: { x = 2.0 * sqrt(max(y, 0.0)) - 1.0; }
    case 7: {
        let yy = (y - 0.5) * (TWO_PI / max(abs(frq), SYNTH_EPS));
        x = 1.0 / (cos(yy) + SYNTH_EPS) - 1.0;
    }
    case 8: {
        let yy = (y - 0.5) * (TWO_PI / max(abs(frq), SYNTH_EPS));
        let zz = cos(yy);
        x = zz / (1.0 + SYNTH_EPS - zz);
    }
    default: {}
    }

    // Layer combination operator.
    switch (layer_op) {
    case 0: { return theta_factor_in + amp * x; }
    case 1: { return theta_factor_in * (1.0 + amp * x); }
    case 2: {
        let v = base_a + amp * x;
        return select(v, theta_factor_in, theta_factor_in > v);
    }
    case 3: {
        let v = base_a + amp * x;
        return select(v, theta_factor_in, theta_factor_in < v);
    }
    default: {}
    }
    return theta_factor_in;
}

// Evaluate the 6-layer synth function at angle `theta`. `a` is the
// global baseline; layers b..f modulate it according to their own
// amp / type / skew / frq / phs / layer-op tuples.
fn synth_value(theta: f32, xform_id: u32, variation_id: u32) -> f32 {
    let a = get_param(xform_id, variation_id, 0u);
    var theta_factor = a;
    theta_factor = synth_apply_layer(theta, theta_factor, a,
        get_param(xform_id, variation_id, 5u),
        i32(get_param(xform_id, variation_id, 6u)),
        get_param(xform_id, variation_id, 7u),
        get_param(xform_id, variation_id, 8u),
        get_param(xform_id, variation_id, 9u),
        i32(get_param(xform_id, variation_id, 10u)));
    theta_factor = synth_apply_layer(theta, theta_factor, a,
        get_param(xform_id, variation_id, 11u),
        i32(get_param(xform_id, variation_id, 12u)),
        get_param(xform_id, variation_id, 13u),
        get_param(xform_id, variation_id, 14u),
        get_param(xform_id, variation_id, 15u),
        i32(get_param(xform_id, variation_id, 16u)));
    theta_factor = synth_apply_layer(theta, theta_factor, a,
        get_param(xform_id, variation_id, 17u),
        i32(get_param(xform_id, variation_id, 18u)),
        get_param(xform_id, variation_id, 19u),
        get_param(xform_id, variation_id, 20u),
        get_param(xform_id, variation_id, 21u),
        i32(get_param(xform_id, variation_id, 22u)));
    theta_factor = synth_apply_layer(theta, theta_factor, a,
        get_param(xform_id, variation_id, 23u),
        i32(get_param(xform_id, variation_id, 24u)),
        get_param(xform_id, variation_id, 25u),
        get_param(xform_id, variation_id, 26u),
        get_param(xform_id, variation_id, 27u),
        i32(get_param(xform_id, variation_id, 28u)));
    theta_factor = synth_apply_layer(theta, theta_factor, a,
        get_param(xform_id, variation_id, 29u),
        i32(get_param(xform_id, variation_id, 30u)),
        get_param(xform_id, variation_id, 31u),
        get_param(xform_id, variation_id, 32u),
        get_param(xform_id, variation_id, 33u),
        i32(get_param(xform_id, variation_id, 34u)));
    return theta_factor;
}

// Bezier-quad interpolation curve from slobo777's reference — used
// when `smooth == 1` ("Smooth YES" modes). The curve passes through
// (0, 0) and (1, m), behaves like a soft S-curve, and is reflected
// through the origin for negative m or x. Past x = L (≥ 1) it just
// returns `a * x` (linear). The four "Curve #N" branches cover the
// four quadrants of (m, x) inside the curved region.
fn synth_bezier(x_in: f32, m_in: f32) -> f32 {
    var x = x_in;
    var m = m_in;
    var sign_factor = 1.0;
    if (m < 0.0) { m = -m; sign_factor = -1.0; }
    if (x < 0.0) { x = -x; sign_factor = -sign_factor; }

    var i_m = 1e10;
    if (m > 1e-10) { i_m = 1.0 / m; }
    let L = select(2.0 * m, 2.0 - m, (2.0 - m) > 2.0 * m);

    if (x > L || m == 1.0) {
        return sign_factor * x;
    }

    if (m < 1.0 && x <= 1.0) {
        var t = x;
        if ((m - 0.5) * (m - 0.5) > 1e-10) {
            t = (-m + sqrt(max(m * m + (1.0 - 2.0 * m) * x, 0.0))) / (1.0 - 2.0 * m);
        }
        return sign_factor * (x + (m - 1.0) * t * t);
    }

    if (m > 1.0 && x <= 1.0) {
        var t = x;
        if ((m - 2.0) * (m - 2.0) > 1e-10) {
            t = (-i_m + sqrt(max(i_m * i_m + (1.0 - 2.0 * i_m) * x, 0.0))) / (1.0 - 2.0 * i_m);
        }
        return sign_factor * (x + (m - 1.0) * t * t);
    }

    if (m < 1.0) {
        let t = sqrt(max((x - 1.0) / (L - 1.0), 0.0));
        return sign_factor * (x + (m - 1.0) * t * t + 2.0 * (1.0 - m) * t + (m - 1.0));
    }

    let t = (1.0 - m) + sqrt(max((m - 1.0) * (m - 1.0) + (x - 1.0), 0.0));
    return sign_factor * (x + (m - 1.0) * t * t - 2.0 * (m - 1.0) * t + (m - 1.0));
}

// Lerp picker — linear scale or the Bezier-quad curve above.
fn synth_interp(x: f32, m: f32, lerp_type: i32) -> f32 {
    if (lerp_type == 1) { return synth_bezier(x, m); }
    return x * m;
}

// synthsincos — modulate (sin θ, cos θ) by the synth function. The
// `smooth` flag here is reinterpreted as the SINCOS_MULTIPLY (0) or
// SINCOS_MIXIN (1) selector (cpp pulls double duty out of the same
// param). For the mixin path the global `mix` weighs sin/cos vs
// synth_value.
fn synth_sincos(theta: f32, sincos_type: i32, xform_id: u32, variation_id: u32) -> vec2<f32> {
    const HALF_PI: f32 = 1.5707963267948966;
    var s = sin(theta);
    var c = cos(theta);
    if (sincos_type == 0) {
        s = s * synth_value(theta, xform_id, variation_id);
        c = c * synth_value(theta + HALF_PI, xform_id, variation_id);
    } else if (sincos_type == 1) {
        let mix = get_param(xform_id, variation_id, 3u);
        s = (1.0 - mix) * s + (synth_value(theta, xform_id, variation_id) - 1.0);
        c = (1.0 - mix) * c + (synth_value(theta + HALF_PI, xform_id, variation_id) - 1.0);
    }
    return vec2<f32>(s, c);
}

fn variation_synth(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    const PI: f32 = 3.14159265359;
    const TWO_PI: f32 = 6.283185307179586;
    const SYNTH_EPS: f32 = 1e-20;

    let mode = i32(get_param(xform_id, variation_id, 1u));
    let power = get_param(xform_id, variation_id, 2u);
    let smooth_i = i32(get_param(xform_id, variation_id, 4u));

    var vx = p.x;
    var vy = p.y;
    var radius: f32;
    var theta: f32;
    var theta_factor: f32;
    var s: f32;
    var c: f32;
    var mu: f32;
    var ox = 0.0;
    var oy = 0.0;

    // Precompute r² once — used by all radial modes. Hoisting it
    // here lets the compiler factor common subexpressions across
    // mode bodies and keeps each `case` small.
    let r2 = vx * vx + vy * vy;

    // Switch over mode (vs an if/else cascade) — naga lowers `switch`
    // to a jump-table, which compiles ~10× faster than 26 chained
    // ifs and produces tighter SPIR-V. The mode IDs are
    // deliberately non-contiguous; 0..19 are the original 2D modes,
    // 1001..1007 are the wave-smoothing variants.
    switch (mode) {
    case 0: {
        // MODE_SPHERICAL
        radius = pow(r2 + SYNTH_EPS, (power + 1.0) * 0.5);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, xform_id, variation_id);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    }
    case 1: {
        // MODE_BUBBLE
        radius = sqrt(r2) / (r2 * 0.25 + 1.0);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, xform_id, variation_id);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    }
    case 2: {
        // MODE_BLUR_LEGACY
        radius = (rng_nextf(rng) + rng_nextf(rng) + 0.002 * rng_nextf(rng)) / 2.002;
        theta = TWO_PI * rng_nextf(rng) - PI;
        let bx = radius * sin(theta);
        let by = radius * cos(theta);
        radius = pow(radius * radius + SYNTH_EPS, power * 0.5);
        theta_factor = synth_value(theta, xform_id, variation_id);
        let rr = synth_interp(radius, theta_factor, smooth_i);
        ox = bx * rr;
        oy = by * rr;
    }
    case 3: {
        // MODE_BLUR_NEW
        radius = 0.5 * (rng_nextf(rng) + rng_nextf(rng));
        theta = TWO_PI * rng_nextf(rng) - PI;
        radius = pow(radius * radius + SYNTH_EPS, -power * 0.5);
        theta_factor = synth_value(theta, xform_id, variation_id);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    }
    case 4: {
        // MODE_BLUR_ZIGZAG
        vy = 1.0 + 0.1 * (rng_nextf(rng) + rng_nextf(rng) - 1.0) * power;
        theta = 2.0 * asin((rng_nextf(rng) - 0.5) * 2.0);
        theta_factor = synth_value(theta, xform_id, variation_id);
        vy = synth_interp(vy, theta_factor, smooth_i);
        ox = theta / PI;
        oy = vy - 1.0;
    }
    case 5: {
        // MODE_RAWCIRCLE
        radius = sqrt(r2);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, xform_id, variation_id);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    }
    case 6: {
        // MODE_RAWX
        theta_factor = synth_value(vy, xform_id, variation_id);
        ox = synth_interp(vx, theta_factor, smooth_i);
        oy = vy;
    }
    case 7: {
        // MODE_RAWY
        theta_factor = synth_value(vx, xform_id, variation_id);
        ox = vx;
        oy = synth_interp(vy, theta_factor, smooth_i);
    }
    case 8: {
        // MODE_RAWXY
        let tf_x = synth_value(vy, xform_id, variation_id);
        ox = synth_interp(vx, tf_x, smooth_i);
        let tf_y = synth_value(vx, xform_id, variation_id);
        oy = synth_interp(vy, tf_y, smooth_i);
    }
    case 9: {
        // MODE_SHIFTX
        ox = vx + synth_value(vy, xform_id, variation_id) - 1.0;
        oy = vy;
    }
    case 10: {
        // MODE_SHIFTY
        ox = vx;
        oy = vy + synth_value(vx, xform_id, variation_id) - 1.0;
    }
    case 11: {
        // MODE_SHIFTXY
        ox = vx + synth_value(vy, xform_id, variation_id) - 1.0;
        oy = vy + synth_value(vx, xform_id, variation_id) - 1.0;
    }
    case 12: {
        // MODE_BLUR_RING
        radius = 1.0 + 0.1 * (rng_nextf(rng) + rng_nextf(rng) - 1.0) * power;
        theta = TWO_PI * rng_nextf(rng) - PI;
        theta_factor = synth_value(theta, xform_id, variation_id);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    }
    case 13: {
        // MODE_BLUR_RING2
        theta = TWO_PI * rng_nextf(rng) - PI;
        let rr = pow(rng_nextf(rng) + SYNTH_EPS, power);
        radius = synth_value(theta, xform_id, variation_id) + 0.1 * rr;
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    }
    case 14: {
        // MODE_SHIFTNSTRETCH
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy) - 1.0 + synth_value(radius, xform_id, variation_id);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    }
    case 15: {
        // MODE_SHIFTTANGENT
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy);
        s = sin(theta);
        c = cos(theta);
        mu = synth_value(radius, xform_id, variation_id) - 1.0;
        vx = vx + mu * c;
        vy = vy - mu * s;
        ox = vx;
        oy = vy;
    }
    case 16: {
        // MODE_SHIFTTHETA
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy) - 1.0 + synth_value(radius, xform_id, variation_id);
        s = sin(theta);
        c = cos(theta);
        radius = sqrt(r2);
        ox = radius * s;
        oy = radius * c;
    }
    case 17: {
        // MODE_XMIRROR
        mu = synth_value(vx, xform_id, variation_id) - 1.0;
        vy = 2.0 * mu - vy;
        ox = vx;
        oy = vy;
    }
    case 18: {
        // MODE_XYMIRROR
        mu = synth_value(vx, xform_id, variation_id) - 1.0;
        let mur = synth_value(vy, xform_id, variation_id) - 1.0;
        vy = 2.0 * mu - vy;
        vx = 2.0 * mur - vx;
        ox = vx;
        oy = vy;
    }
    case 19: {
        // MODE_SPHERICAL2
        radius = sqrt(r2);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, xform_id, variation_id);
        radius = synth_interp(radius, theta_factor, smooth_i);
        radius = pow(max(radius, 0.0), power);
        ox = radius * sin(theta);
        oy = radius * cos(theta);
    }

    // -----------------------------------------------------------------
    // "Wave-smoothing" modes (1001..1007). All use synth_sincos with
    // the `smooth` flag re-interpreted as SINCOS_MULTIPLY / MIXIN.
    // -----------------------------------------------------------------
    case 1001: {
        // MODE_SINUSOIDAL — sx + (1-mix)*sin(x) shifted by synth.
        let mix = get_param(xform_id, variation_id, 3u);
        ox = synth_value(vx, xform_id, variation_id) - 1.0 + (1.0 - mix) * sin(vx);
        oy = synth_value(vy, xform_id, variation_id) - 1.0 + (1.0 - mix) * sin(vy);
    }
    case 1002: {
        // MODE_SWIRL
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        let pair = synth_sincos(radius, smooth_i, xform_id, variation_id);
        s = pair.x;
        c = pair.y;
        ox = s * vx - c * vy;
        oy = c * vx + s * vy;
    }
    case 1003: {
        // MODE_HYPERBOLIC
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy);
        let pair = synth_sincos(theta, smooth_i, xform_id, variation_id);
        s = pair.x;
        c = pair.y;
        let safe_r = select(radius, SYNTH_EPS, abs(radius) < SYNTH_EPS);
        ox = s / safe_r;
        oy = c * radius;
    }
    case 1004: {
        // MODE_JULIA — random branch flip, like the Apo julia.
        radius = pow(r2 + SYNTH_EPS, power * 0.25);
        theta = atan2(vx, vy) * 0.5;
        if (rng_nextf(rng) < 0.5) { theta = theta + PI; }
        let pair = synth_sincos(theta, smooth_i, xform_id, variation_id);
        s = pair.x;
        c = pair.y;
        ox = radius * c;
        oy = radius * s;
    }
    case 1005: {
        // MODE_DISC — note JWildfire's cpp uses `=` here, not `+=`, so
        // this mode REPLACES the variation's contribution rather than
        // accumulating onto whatever prior variations contributed. Our
        // dispatcher is purely additive, so we return the value as-is;
        // when this is the only variation in a transform the result is
        // identical to the cpp.
        theta = atan2(vx, vy) / PI;
        radius = PI * pow(r2 + SYNTH_EPS, power * 0.5);
        let pair = synth_sincos(radius, smooth_i, xform_id, variation_id);
        s = pair.x;
        c = pair.y;
        ox = s * theta;
        oy = c * theta;
    }
    case 1006: {
        // MODE_RINGS — power is used as `mu = power² + EPS` to slice
        // radius into rings.
        let safe_eps = SYNTH_EPS;
        let mu_r = power * power + safe_eps;
        radius = sqrt(r2);
        theta = atan2(vx, vy);
        radius = radius + -2.0 * mu_r * floor((radius + mu_r) / (2.0 * mu_r)) + radius * (1.0 - mu_r);
        let pair = synth_sincos(theta, smooth_i, xform_id, variation_id);
        s = pair.x;
        c = pair.y;
        ox = s * radius;
        oy = c * radius;
    }
    case 1007: {
        // MODE_CYLINDER — synth-modulated sine on X, pass-through Y.
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        let pair = synth_sincos(vx, smooth_i, xform_id, variation_id);
        s = pair.x;
        ox = radius * s;
        oy = radius * vy;
    }
    default: {
        // Unknown modes return (0, 0) — matches the cpp's `default:
        // nothing to do` branch.
    }
    }

    return vec2<f32>(ox, oy);
}
"#,
    // 3D body is the same XY math; Z passes through unchanged
    // (synth is a 2D variation in JWildfire — VARTYPE_2D, no Z math).
    wgsl_3d: r#"
fn synth_apply_layer(theta: f32, theta_factor_in: f32, base_a: f32,
                     amp: f32, layer_type: i32, skew: f32, frq: f32,
                     phs: f32, layer_op: i32) -> f32 {
    if (amp == 0.0) { return theta_factor_in; }
    const TWO_PI: f32 = 6.283185307179586;
    const SYNTH_EPS: f32 = 1e-20;
    var z = phs + theta * frq;
    var y = z / TWO_PI;
    y = y - floor(y);
    if (skew != 0.0) {
        let z_split = 0.5 + 0.5 * skew;
        if (y > z_split) {
            y = 0.5 + 0.5 * (y - z_split) / (1.0 - z_split + SYNTH_EPS);
        } else {
            y = 0.5 - 0.5 * (z_split - y) / (z_split + SYNTH_EPS);
        }
    }
    var x: f32 = 0.0;
    if (layer_type == 0) { x = sin(y * TWO_PI); }
    else if (layer_type == 1) { x = cos(y * TWO_PI); }
    else if (layer_type == 2) { x = select(-1.0, 1.0, y > 0.5); }
    else if (layer_type == 3) { x = 1.0 - 2.0 * y; }
    else if (layer_type == 4) { x = select(2.0 * y - 1.0, 3.0 - 4.0 * y, y > 0.5); }
    else if (layer_type == 5) { x = 8.0 * (y - 0.5) * (y - 0.5) - 1.0; }
    else if (layer_type == 6) { x = 2.0 * sqrt(max(y, 0.0)) - 1.0; }
    else if (layer_type == 7) {
        let yy = (y - 0.5) * (TWO_PI / max(abs(frq), SYNTH_EPS));
        x = 1.0 / (cos(yy) + SYNTH_EPS) - 1.0;
    } else if (layer_type == 8) {
        let yy = (y - 0.5) * (TWO_PI / max(abs(frq), SYNTH_EPS));
        let zz = cos(yy);
        x = zz / (1.0 + SYNTH_EPS - zz);
    }
    if (layer_op == 0) { return theta_factor_in + amp * x; }
    if (layer_op == 1) { return theta_factor_in * (1.0 + amp * x); }
    if (layer_op == 2) { let v = base_a + amp * x; return select(v, theta_factor_in, theta_factor_in > v); }
    if (layer_op == 3) { let v = base_a + amp * x; return select(v, theta_factor_in, theta_factor_in < v); }
    return theta_factor_in;
}

fn synth_value(theta: f32, xform_id: u32, variation_id: u32) -> f32 {
    let a = get_param(xform_id, variation_id, 0u);
    var theta_factor = a;
    theta_factor = synth_apply_layer(theta, theta_factor, a,
        get_param(xform_id, variation_id, 5u),
        i32(get_param(xform_id, variation_id, 6u)),
        get_param(xform_id, variation_id, 7u),
        get_param(xform_id, variation_id, 8u),
        get_param(xform_id, variation_id, 9u),
        i32(get_param(xform_id, variation_id, 10u)));
    theta_factor = synth_apply_layer(theta, theta_factor, a,
        get_param(xform_id, variation_id, 11u),
        i32(get_param(xform_id, variation_id, 12u)),
        get_param(xform_id, variation_id, 13u),
        get_param(xform_id, variation_id, 14u),
        get_param(xform_id, variation_id, 15u),
        i32(get_param(xform_id, variation_id, 16u)));
    theta_factor = synth_apply_layer(theta, theta_factor, a,
        get_param(xform_id, variation_id, 17u),
        i32(get_param(xform_id, variation_id, 18u)),
        get_param(xform_id, variation_id, 19u),
        get_param(xform_id, variation_id, 20u),
        get_param(xform_id, variation_id, 21u),
        i32(get_param(xform_id, variation_id, 22u)));
    theta_factor = synth_apply_layer(theta, theta_factor, a,
        get_param(xform_id, variation_id, 23u),
        i32(get_param(xform_id, variation_id, 24u)),
        get_param(xform_id, variation_id, 25u),
        get_param(xform_id, variation_id, 26u),
        get_param(xform_id, variation_id, 27u),
        i32(get_param(xform_id, variation_id, 28u)));
    theta_factor = synth_apply_layer(theta, theta_factor, a,
        get_param(xform_id, variation_id, 29u),
        i32(get_param(xform_id, variation_id, 30u)),
        get_param(xform_id, variation_id, 31u),
        get_param(xform_id, variation_id, 32u),
        get_param(xform_id, variation_id, 33u),
        i32(get_param(xform_id, variation_id, 34u)));
    return theta_factor;
}

fn synth_bezier(x_in: f32, m_in: f32) -> f32 {
    var x = x_in;
    var m = m_in;
    var sign_factor = 1.0;
    if (m < 0.0) { m = -m; sign_factor = -1.0; }
    if (x < 0.0) { x = -x; sign_factor = -sign_factor; }
    var i_m = 1e10;
    if (m > 1e-10) { i_m = 1.0 / m; }
    let L = select(2.0 * m, 2.0 - m, (2.0 - m) > 2.0 * m);
    if (x > L || m == 1.0) { return sign_factor * x; }
    if (m < 1.0 && x <= 1.0) {
        var t = x;
        if ((m - 0.5) * (m - 0.5) > 1e-10) {
            t = (-m + sqrt(max(m * m + (1.0 - 2.0 * m) * x, 0.0))) / (1.0 - 2.0 * m);
        }
        return sign_factor * (x + (m - 1.0) * t * t);
    }
    if (m > 1.0 && x <= 1.0) {
        var t = x;
        if ((m - 2.0) * (m - 2.0) > 1e-10) {
            t = (-i_m + sqrt(max(i_m * i_m + (1.0 - 2.0 * i_m) * x, 0.0))) / (1.0 - 2.0 * i_m);
        }
        return sign_factor * (x + (m - 1.0) * t * t);
    }
    if (m < 1.0) {
        let t = sqrt(max((x - 1.0) / (L - 1.0), 0.0));
        return sign_factor * (x + (m - 1.0) * t * t + 2.0 * (1.0 - m) * t + (m - 1.0));
    }
    let t = (1.0 - m) + sqrt(max((m - 1.0) * (m - 1.0) + (x - 1.0), 0.0));
    return sign_factor * (x + (m - 1.0) * t * t - 2.0 * (m - 1.0) * t + (m - 1.0));
}

fn synth_interp(x: f32, m: f32, lerp_type: i32) -> f32 {
    if (lerp_type == 1) { return synth_bezier(x, m); }
    return x * m;
}

fn synth_sincos(theta: f32, sincos_type: i32, xform_id: u32, variation_id: u32) -> vec2<f32> {
    const HALF_PI: f32 = 1.5707963267948966;
    var s = sin(theta);
    var c = cos(theta);
    if (sincos_type == 0) {
        s = s * synth_value(theta, xform_id, variation_id);
        c = c * synth_value(theta + HALF_PI, xform_id, variation_id);
    } else if (sincos_type == 1) {
        let mix = get_param(xform_id, variation_id, 3u);
        s = (1.0 - mix) * s + (synth_value(theta, xform_id, variation_id) - 1.0);
        c = (1.0 - mix) * c + (synth_value(theta + HALF_PI, xform_id, variation_id) - 1.0);
    }
    return vec2<f32>(s, c);
}

fn variation_synth(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    const TWO_PI: f32 = 6.283185307179586;
    const SYNTH_EPS: f32 = 1e-20;
    let mode = i32(get_param(xform_id, variation_id, 1u));
    let power = get_param(xform_id, variation_id, 2u);
    let smooth_i = i32(get_param(xform_id, variation_id, 4u));
    var vx = p.x;
    var vy = p.y;
    var radius: f32;
    var theta: f32;
    var theta_factor: f32;
    var s: f32;
    var c: f32;
    var mu: f32;
    var ox = 0.0;
    var oy = 0.0;
    let r2 = vx * vx + vy * vy;
    switch (mode) {
    case 0: {
        radius = pow(r2 + SYNTH_EPS, (power + 1.0) * 0.5);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, xform_id, variation_id);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta); oy = radius * cos(theta);
    }
    case 1: {
        radius = sqrt(r2) / (r2 * 0.25 + 1.0);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, xform_id, variation_id);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta); oy = radius * cos(theta);
    }
    case 2: {
        radius = (rng_nextf(rng) + rng_nextf(rng) + 0.002 * rng_nextf(rng)) / 2.002;
        theta = TWO_PI * rng_nextf(rng) - PI;
        let bx = radius * sin(theta); let by = radius * cos(theta);
        radius = pow(radius * radius + SYNTH_EPS, power * 0.5);
        theta_factor = synth_value(theta, xform_id, variation_id);
        let rr = synth_interp(radius, theta_factor, smooth_i);
        ox = bx * rr; oy = by * rr;
    }
    case 3: {
        radius = 0.5 * (rng_nextf(rng) + rng_nextf(rng));
        theta = TWO_PI * rng_nextf(rng) - PI;
        radius = pow(radius * radius + SYNTH_EPS, -power * 0.5);
        theta_factor = synth_value(theta, xform_id, variation_id);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta); oy = radius * cos(theta);
    }
    case 4: {
        vy = 1.0 + 0.1 * (rng_nextf(rng) + rng_nextf(rng) - 1.0) * power;
        theta = 2.0 * asin((rng_nextf(rng) - 0.5) * 2.0);
        theta_factor = synth_value(theta, xform_id, variation_id);
        vy = synth_interp(vy, theta_factor, smooth_i);
        ox = theta / PI; oy = vy - 1.0;
    }
    case 5: {
        radius = sqrt(r2);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, xform_id, variation_id);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta); oy = radius * cos(theta);
    }
    case 6: {
        theta_factor = synth_value(vy, xform_id, variation_id);
        ox = synth_interp(vx, theta_factor, smooth_i); oy = vy;
    }
    case 7: {
        theta_factor = synth_value(vx, xform_id, variation_id);
        ox = vx; oy = synth_interp(vy, theta_factor, smooth_i);
    }
    case 8: {
        let tf_x = synth_value(vy, xform_id, variation_id);
        ox = synth_interp(vx, tf_x, smooth_i);
        let tf_y = synth_value(vx, xform_id, variation_id);
        oy = synth_interp(vy, tf_y, smooth_i);
    }
    case 9: {
        ox = vx + synth_value(vy, xform_id, variation_id) - 1.0; oy = vy;
    }
    case 10: {
        ox = vx; oy = vy + synth_value(vx, xform_id, variation_id) - 1.0;
    }
    case 11: {
        ox = vx + synth_value(vy, xform_id, variation_id) - 1.0;
        oy = vy + synth_value(vx, xform_id, variation_id) - 1.0;
    }
    case 12: {
        radius = 1.0 + 0.1 * (rng_nextf(rng) + rng_nextf(rng) - 1.0) * power;
        theta = TWO_PI * rng_nextf(rng) - PI;
        theta_factor = synth_value(theta, xform_id, variation_id);
        radius = synth_interp(radius, theta_factor, smooth_i);
        ox = radius * sin(theta); oy = radius * cos(theta);
    }
    case 13: {
        theta = TWO_PI * rng_nextf(rng) - PI;
        let rr = pow(rng_nextf(rng) + SYNTH_EPS, power);
        radius = synth_value(theta, xform_id, variation_id) + 0.1 * rr;
        ox = radius * sin(theta); oy = radius * cos(theta);
    }
    case 14: {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy) - 1.0 + synth_value(radius, xform_id, variation_id);
        ox = radius * sin(theta); oy = radius * cos(theta);
    }
    case 15: {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy);
        s = sin(theta); c = cos(theta);
        mu = synth_value(radius, xform_id, variation_id) - 1.0;
        vx = vx + mu * c; vy = vy - mu * s;
        ox = vx; oy = vy;
    }
    case 16: {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy) - 1.0 + synth_value(radius, xform_id, variation_id);
        s = sin(theta); c = cos(theta);
        radius = sqrt(r2);
        ox = radius * s; oy = radius * c;
    }
    case 17: {
        mu = synth_value(vx, xform_id, variation_id) - 1.0;
        vy = 2.0 * mu - vy;
        ox = vx; oy = vy;
    }
    case 18: {
        mu = synth_value(vx, xform_id, variation_id) - 1.0;
        let mur = synth_value(vy, xform_id, variation_id) - 1.0;
        vy = 2.0 * mu - vy; vx = 2.0 * mur - vx;
        ox = vx; oy = vy;
    }
    case 19: {
        radius = sqrt(r2);
        theta = atan2(vx, vy);
        theta_factor = synth_value(theta, xform_id, variation_id);
        radius = synth_interp(radius, theta_factor, smooth_i);
        radius = pow(max(radius, 0.0), power);
        ox = radius * sin(theta); oy = radius * cos(theta);
    }
    case 1001: {
        let mix = get_param(xform_id, variation_id, 3u);
        ox = synth_value(vx, xform_id, variation_id) - 1.0 + (1.0 - mix) * sin(vx);
        oy = synth_value(vy, xform_id, variation_id) - 1.0 + (1.0 - mix) * sin(vy);
    }
    case 1002: {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        let pair = synth_sincos(radius, smooth_i, xform_id, variation_id);
        s = pair.x; c = pair.y;
        ox = s * vx - c * vy; oy = c * vx + s * vy;
    }
    case 1003: {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        theta = atan2(vx, vy);
        let pair = synth_sincos(theta, smooth_i, xform_id, variation_id);
        s = pair.x; c = pair.y;
        let safe_r = select(radius, SYNTH_EPS, abs(radius) < SYNTH_EPS);
        ox = s / safe_r; oy = c * radius;
    }
    case 1004: {
        radius = pow(r2 + SYNTH_EPS, power * 0.25);
        theta = atan2(vx, vy) * 0.5;
        if (rng_nextf(rng) < 0.5) { theta = theta + PI; }
        let pair = synth_sincos(theta, smooth_i, xform_id, variation_id);
        s = pair.x; c = pair.y;
        ox = radius * c; oy = radius * s;
    }
    case 1005: {
        theta = atan2(vx, vy) / PI;
        radius = PI * pow(r2 + SYNTH_EPS, power * 0.5);
        let pair = synth_sincos(radius, smooth_i, xform_id, variation_id);
        s = pair.x; c = pair.y;
        ox = s * theta; oy = c * theta;
    }
    case 1006: {
        let mu_r = power * power + SYNTH_EPS;
        radius = sqrt(r2);
        theta = atan2(vx, vy);
        radius = radius + -2.0 * mu_r * floor((radius + mu_r) / (2.0 * mu_r)) + radius * (1.0 - mu_r);
        let pair = synth_sincos(theta, smooth_i, xform_id, variation_id);
        s = pair.x; c = pair.y;
        ox = s * radius; oy = c * radius;
    }
    case 1007: {
        radius = pow(r2 + SYNTH_EPS, power * 0.5);
        let pair = synth_sincos(vx, smooth_i, xform_id, variation_id);
        s = pair.x;
        ox = radius * s; oy = radius * vy;
    }
    default: {
        // Unknown modes return (0, 0) — matches the cpp's `default:
        // nothing to do` branch.
    }
    }

    return vec3<f32>(ox, oy, p.z);
}
"#,
};

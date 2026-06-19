//! quaternion (zephyrtronium / Brad Stefanov)
//!
//! Single mega-variation that combines all 13 quaternion-style trig +
//! hyperbolic + log + estiq subfunctions, each gated by its own `*pow`
//! weight. Subfunctions are independently scalable per axis via
//! `*x1, *x2, *y1, *y2, *z1, *z2` mixers.
//!
//! 92 user params + 1 init slot (`_denom`) = 93 total. Far over the old
//! 16-slot ceiling — port enabled by the packed-variation-params buffer.
//!
//! Slot map:
//!   cosq:  0..6   (pow, x1, x2, y1, y2, z1, z2)
//!   coshq: 7..13
//!   cotq:  14..20
//!   cothq: 21..27
//!   cscq:  28..34
//!   cschq: 35..41
//!   estiq: 42..47 (pow, x1, y1, y2, z1, z2)  [no x2]
//!   logq:  48..49 (pow, base)
//!   secq:  50..56
//!   sechq: 57..63
//!   sinq:  64..70
//!   sinhq: 71..77
//!   tanq:  78..84
//!   tanhq: 85..91
//!   _denom: 92      (0.5 / ln(logqbase))
//!
//! cpp drops the `pAmount` factor that pre-scales each subfunction since
//! our outer multiplier already supplies it. logq's denom in cpp uses
//! `0.5 * pAmount / log(base)`; we precompute `0.5 / log(base)` and let
//! the framework apply pAmount once.
//!
//! Source: `output/jwildfire-vars/output/quaternion.cpp` (Java embedded).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Mega-variation combining 14 quaternion-extended scalar functions — 12
/// hyperbolic-extended trig pairs (`cosq`/`coshq`, `cotq`/`cothq`,
/// `cscq`/`cschq`, `secq`/`sechq`, `sinq`/`sinhq`, `tanq`/`tanhq`) plus
/// `estiq` (exp-times-trig) and `logq` (complex log). Each subfunction
/// treats `x` as the scalar quaternion component and `|y, z|` as the
/// imaginary magnitude, evaluates a quaternion-style identity, and
/// accumulates into the output. Subfunctions are independently gated by
/// their own `*pow` weight (zero = disabled) and tunable per axis via `x1,
/// x2, y1, y2, z1, z2` mixers. Only `cosq` is active by default. At 92 user
/// params, this is one of the most parameter-heavy variations in the
/// registry — the port was unblocked by the packed-variation-params buffer
/// change.
///
/// # Authors
/// - zephyrtronium
/// - Brad Stefanov
pub static QUATERNION: VariationDef = VariationDef {
    name: "quaternion",
    aliases: &[],
    display_name: "Quaternion",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[
        // cosq (default pow=1, others=1)
        param!("cosqpow", "Cosq Pow", unlimited_float, 1.0, -10.0, 10.0, "cosq: subfunction weight (quaternion-extended cosine). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("cosqx1", "Cosq X1", unlimited_float, 1.0, -10.0, 10.0, "cosq: scale applied to `x` before the first trig/hyperbolic term."),
        param!("cosqx2", "Cosq X2", unlimited_float, 1.0, -10.0, 10.0, "cosq: scale applied to `x` before the second trig/hyperbolic term."),
        param!("cosqy1", "Cosq Y1", unlimited_float, 1.0, -10.0, 10.0, "cosq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term."),
        param!("cosqy2", "Cosq Y2", unlimited_float, 1.0, -10.0, 10.0, "cosq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term."),
        param!("cosqz1", "Cosq Z1", unlimited_float, 1.0, -10.0, 10.0, "cosq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings)."),
        param!("cosqz2", "Cosq Z2", unlimited_float, 1.0, -10.0, 10.0, "cosq: scale applied to the Y/Z cross-term that builds the off-axis output."),
        // coshq
        param!("coshqpow", "Coshq Pow", unlimited_float, 0.0, -10.0, 10.0, "coshq: subfunction weight (quaternion-extended hyperbolic cosine (cosh)). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("coshqx1", "Coshq X1", unlimited_float, 1.0, -10.0, 10.0, "coshq: scale applied to `x` before the first trig/hyperbolic term."),
        param!("coshqx2", "Coshq X2", unlimited_float, 1.0, -10.0, 10.0, "coshq: scale applied to `x` before the second trig/hyperbolic term."),
        param!("coshqy1", "Coshq Y1", unlimited_float, 1.0, -10.0, 10.0, "coshq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term."),
        param!("coshqy2", "Coshq Y2", unlimited_float, 1.0, -10.0, 10.0, "coshq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term."),
        param!("coshqz1", "Coshq Z1", unlimited_float, 1.0, -10.0, 10.0, "coshq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings)."),
        param!("coshqz2", "Coshq Z2", unlimited_float, 1.0, -10.0, 10.0, "coshq: scale applied to the Y/Z cross-term that builds the off-axis output."),
        // cotq
        param!("cotqpow", "Cotq Pow", unlimited_float, 0.0, -10.0, 10.0, "cotq: subfunction weight (quaternion-extended cotangent). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("cotqx1", "Cotq X1", unlimited_float, 1.0, -10.0, 10.0, "cotq: scale applied to `x` before the first trig/hyperbolic term."),
        param!("cotqx2", "Cotq X2", unlimited_float, 1.0, -10.0, 10.0, "cotq: scale applied to `x` before the second trig/hyperbolic term."),
        param!("cotqy1", "Cotq Y1", unlimited_float, 1.0, -10.0, 10.0, "cotq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term."),
        param!("cotqy2", "Cotq Y2", unlimited_float, 1.0, -10.0, 10.0, "cotq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term."),
        param!("cotqz1", "Cotq Z1", unlimited_float, 1.0, -10.0, 10.0, "cotq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings)."),
        param!("cotqz2", "Cotq Z2", unlimited_float, 1.0, -10.0, 10.0, "cotq: scale applied to the Y/Z cross-term that builds the off-axis output."),
        // cothq
        param!("cothqpow", "Cothq Pow", unlimited_float, 0.0, -10.0, 10.0, "cothq: subfunction weight (quaternion-extended hyperbolic cotangent (coth)). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("cothqx1", "Cothq X1", unlimited_float, 1.0, -10.0, 10.0, "cothq: scale applied to `x` before the first trig/hyperbolic term."),
        param!("cothqx2", "Cothq X2", unlimited_float, 1.0, -10.0, 10.0, "cothq: scale applied to `x` before the second trig/hyperbolic term."),
        param!("cothqy1", "Cothq Y1", unlimited_float, 1.0, -10.0, 10.0, "cothq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term."),
        param!("cothqy2", "Cothq Y2", unlimited_float, 1.0, -10.0, 10.0, "cothq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term."),
        param!("cothqz1", "Cothq Z1", unlimited_float, 1.0, -10.0, 10.0, "cothq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings)."),
        param!("cothqz2", "Cothq Z2", unlimited_float, 1.0, -10.0, 10.0, "cothq: scale applied to the Y/Z cross-term that builds the off-axis output."),
        // cscq
        param!("cscqpow", "Cscq Pow", unlimited_float, 0.0, -10.0, 10.0, "cscq: subfunction weight (quaternion-extended cosecant (csc)). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("cscqx1", "Cscq X1", unlimited_float, 1.0, -10.0, 10.0, "cscq: scale applied to `x` before the first trig/hyperbolic term."),
        param!("cscqx2", "Cscq X2", unlimited_float, 1.0, -10.0, 10.0, "cscq: scale applied to `x` before the second trig/hyperbolic term."),
        param!("cscqy1", "Cscq Y1", unlimited_float, 1.0, -10.0, 10.0, "cscq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term."),
        param!("cscqy2", "Cscq Y2", unlimited_float, 1.0, -10.0, 10.0, "cscq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term."),
        param!("cscqz1", "Cscq Z1", unlimited_float, 1.0, -10.0, 10.0, "cscq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings)."),
        param!("cscqz2", "Cscq Z2", unlimited_float, 1.0, -10.0, 10.0, "cscq: scale applied to the Y/Z cross-term that builds the off-axis output."),
        // cschq
        param!("cschqpow", "Cschq Pow", unlimited_float, 0.0, -10.0, 10.0, "cschq: subfunction weight (quaternion-extended hyperbolic cosecant (csch)). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("cschqx1", "Cschq X1", unlimited_float, 1.0, -10.0, 10.0, "cschq: scale applied to `x` before the first trig/hyperbolic term."),
        param!("cschqx2", "Cschq X2", unlimited_float, 1.0, -10.0, 10.0, "cschq: scale applied to `x` before the second trig/hyperbolic term."),
        param!("cschqy1", "Cschq Y1", unlimited_float, 1.0, -10.0, 10.0, "cschq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term."),
        param!("cschqy2", "Cschq Y2", unlimited_float, 1.0, -10.0, 10.0, "cschq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term."),
        param!("cschqz1", "Cschq Z1", unlimited_float, 1.0, -10.0, 10.0, "cschq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings)."),
        param!("cschqz2", "Cschq Z2", unlimited_float, 1.0, -10.0, 10.0, "cschq: scale applied to the Y/Z cross-term that builds the off-axis output."),
        // estiq (6 params, no x2)
        param!("estiqpow", "Estiq Pow", unlimited_float, 0.0, -10.0, 10.0, "estiq: subfunction weight (exp-times-trig: `e^x · (cos, sin)`). 0 disables; non-zero activates and scales the contribution."),
        param!("estiqx1", "Estiq X1", unlimited_float, 1.0, -10.0, 10.0, "estiq: scale applied to `x` before the exponential `e^(x · x1)`."),
        param!("estiqy1", "Estiq Y1", unlimited_float, 1.0, -10.0, 10.0, "estiq: scale applied to the `|y, z|` magnitude before the sine term."),
        param!("estiqy2", "Estiq Y2", unlimited_float, 1.0, -10.0, 10.0, "estiq: scale applied to the `|y, z|` magnitude before the cosine term."),
        param!("estiqz1", "Estiq Z1", unlimited_float, 1.0, -10.0, 10.0, "estiq: pre-multiplier on the `|y, z|` magnitude."),
        param!("estiqz2", "Estiq Z2", unlimited_float, 1.0, -10.0, 10.0, "estiq: scale applied to the Y/Z cross-term."),
        // logq
        param!("logqpow", "Logq Pow", unlimited_float, 0.0, -10.0, 10.0, "logq: subfunction weight (complex logarithm: `(½·log|z|², atan2(|y,z|, x))`). 0 disables; non-zero activates and scales the contribution. X uses the pre-computed `_denom = 0.5 / ln(base)`."),
        param!("logqbase", "Logq Base", unlimited_float, 2.71828182845905, 0.001, 100.0, "logq: logarithm base. Pre-computed at init into `_denom = 0.5 / ln(base)`. Default `e` (~2.718) gives the natural log; 10 gives common log; 2 gives binary log."),
        // secq
        param!("secqpow", "Secq Pow", unlimited_float, 0.0, -10.0, 10.0, "secq: subfunction weight (quaternion-extended secant). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("secqx1", "Secq X1", unlimited_float, 1.0, -10.0, 10.0, "secq: scale applied to `x` before the first trig/hyperbolic term."),
        param!("secqx2", "Secq X2", unlimited_float, 1.0, -10.0, 10.0, "secq: scale applied to `x` before the second trig/hyperbolic term."),
        param!("secqy1", "Secq Y1", unlimited_float, 1.0, -10.0, 10.0, "secq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term."),
        param!("secqy2", "Secq Y2", unlimited_float, 1.0, -10.0, 10.0, "secq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term."),
        param!("secqz1", "Secq Z1", unlimited_float, 1.0, -10.0, 10.0, "secq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings)."),
        param!("secqz2", "Secq Z2", unlimited_float, 1.0, -10.0, 10.0, "secq: scale applied to the Y/Z cross-term that builds the off-axis output."),
        // sechq
        param!("sechqpow", "Sechq Pow", unlimited_float, 0.0, -10.0, 10.0, "sechq: subfunction weight (quaternion-extended hyperbolic secant (sech)). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("sechqx1", "Sechq X1", unlimited_float, 1.0, -10.0, 10.0, "sechq: scale applied to `x` before the first trig/hyperbolic term."),
        param!("sechqx2", "Sechq X2", unlimited_float, 1.0, -10.0, 10.0, "sechq: scale applied to `x` before the second trig/hyperbolic term."),
        param!("sechqy1", "Sechq Y1", unlimited_float, 1.0, -10.0, 10.0, "sechq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term."),
        param!("sechqy2", "Sechq Y2", unlimited_float, 1.0, -10.0, 10.0, "sechq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term."),
        param!("sechqz1", "Sechq Z1", unlimited_float, 1.0, -10.0, 10.0, "sechq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings)."),
        param!("sechqz2", "Sechq Z2", unlimited_float, 1.0, -10.0, 10.0, "sechq: scale applied to the Y/Z cross-term that builds the off-axis output."),
        // sinq
        param!("sinqpow", "Sinq Pow", unlimited_float, 0.0, -10.0, 10.0, "sinq: subfunction weight (quaternion-extended sine). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("sinqx1", "Sinq X1", unlimited_float, 1.0, -10.0, 10.0, "sinq: scale applied to `x` before the first trig/hyperbolic term."),
        param!("sinqx2", "Sinq X2", unlimited_float, 1.0, -10.0, 10.0, "sinq: scale applied to `x` before the second trig/hyperbolic term."),
        param!("sinqy1", "Sinq Y1", unlimited_float, 1.0, -10.0, 10.0, "sinq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term."),
        param!("sinqy2", "Sinq Y2", unlimited_float, 1.0, -10.0, 10.0, "sinq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term."),
        param!("sinqz1", "Sinq Z1", unlimited_float, 1.0, -10.0, 10.0, "sinq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings)."),
        param!("sinqz2", "Sinq Z2", unlimited_float, 1.0, -10.0, 10.0, "sinq: scale applied to the Y/Z cross-term that builds the off-axis output."),
        // sinhq
        param!("sinhqpow", "Sinhq Pow", unlimited_float, 0.0, -10.0, 10.0, "sinhq: subfunction weight (quaternion-extended hyperbolic sine (sinh)). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("sinhqx1", "Sinhq X1", unlimited_float, 1.0, -10.0, 10.0, "sinhq: scale applied to `x` before the first trig/hyperbolic term."),
        param!("sinhqx2", "Sinhq X2", unlimited_float, 1.0, -10.0, 10.0, "sinhq: scale applied to `x` before the second trig/hyperbolic term."),
        param!("sinhqy1", "Sinhq Y1", unlimited_float, 1.0, -10.0, 10.0, "sinhq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term."),
        param!("sinhqy2", "Sinhq Y2", unlimited_float, 1.0, -10.0, 10.0, "sinhq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term."),
        param!("sinhqz1", "Sinhq Z1", unlimited_float, 1.0, -10.0, 10.0, "sinhq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings)."),
        param!("sinhqz2", "Sinhq Z2", unlimited_float, 1.0, -10.0, 10.0, "sinhq: scale applied to the Y/Z cross-term that builds the off-axis output."),
        // tanq
        param!("tanqpow", "Tanq Pow", unlimited_float, 0.0, -10.0, 10.0, "tanq: subfunction weight (quaternion-extended tangent). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("tanqx1", "Tanq X1", unlimited_float, 1.0, -10.0, 10.0, "tanq: scale applied to `x` before the first trig/hyperbolic term."),
        param!("tanqx2", "Tanq X2", unlimited_float, 1.0, -10.0, 10.0, "tanq: scale applied to `x` before the second trig/hyperbolic term."),
        param!("tanqy1", "Tanq Y1", unlimited_float, 1.0, -10.0, 10.0, "tanq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term."),
        param!("tanqy2", "Tanq Y2", unlimited_float, 1.0, -10.0, 10.0, "tanq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term."),
        param!("tanqz1", "Tanq Z1", unlimited_float, 1.0, -10.0, 10.0, "tanq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings)."),
        param!("tanqz2", "Tanq Z2", unlimited_float, 1.0, -10.0, 10.0, "tanq: scale applied to the Y/Z cross-term that builds the off-axis output."),
        // tanhq
        param!("tanhqpow", "Tanhq Pow", unlimited_float, 0.0, -10.0, 10.0, "tanhq: subfunction weight (quaternion-extended hyperbolic tangent (tanh)). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("tanhqx1", "Tanhq X1", unlimited_float, 1.0, -10.0, 10.0, "tanhq: scale applied to `x` before the first trig/hyperbolic term."),
        param!("tanhqx2", "Tanhq X2", unlimited_float, 1.0, -10.0, 10.0, "tanhq: scale applied to `x` before the second trig/hyperbolic term."),
        param!("tanhqy1", "Tanhq Y1", unlimited_float, 1.0, -10.0, 10.0, "tanhq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term."),
        param!("tanhqy2", "Tanhq Y2", unlimited_float, 1.0, -10.0, 10.0, "tanhq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term."),
        param!("tanhqz1", "Tanhq Z1", unlimited_float, 1.0, -10.0, 10.0, "tanhq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings)."),
        param!("tanhqz2", "Tanhq Z2", unlimited_float, 1.0, -10.0, 10.0, "tanhq: scale applied to the Y/Z cross-term that builds the off-axis output."),
    ],
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_quaternion(user: array<f32, 92>) -> array<f32, 1> {
    let logqbase = user[49];
    var out: array<f32, 1>;
    let safe_base = max(logqbase, 1.0001);
    out[0] = 0.5 / log(safe_base);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_quaternion(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x_in = p.x;
    let y_in = p.y;
    let z_in = 0.0;
    var ox = 0.0;
    var oy = 0.0;

    // cosq
    let cosqpow = get_param(xform_id, variation_id, 0u);
    if (cosqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 5u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(x_in * get_param(xform_id, variation_id, 1u));
        let c = cos(x_in * get_param(xform_id, variation_id, 2u));
        let sh = sinh(abs_v * get_param(xform_id, variation_id, 3u));
        let ch = cosh(abs_v * get_param(xform_id, variation_id, 4u));
        let cap = s * sh / safe_abs * get_param(xform_id, variation_id, 6u);
        ox = ox + cosqpow * c * ch;
        oy = oy + cosqpow * cap * y_in;
    }

    // coshq
    let coshqpow = get_param(xform_id, variation_id, 7u);
    if (coshqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 12u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(abs_v * get_param(xform_id, variation_id, 10u));
        let c = cos(abs_v * get_param(xform_id, variation_id, 11u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 8u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 9u));
        let cap = sh * s / safe_abs * get_param(xform_id, variation_id, 13u);
        ox = ox + coshqpow * ch * c;
        oy = oy + coshqpow * cap * y_in;
    }

    // cotq
    let cotqpow = get_param(xform_id, variation_id, 14u);
    if (cotqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 19u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(x_in * get_param(xform_id, variation_id, 15u));
        let c = cos(x_in * get_param(xform_id, variation_id, 16u));
        let sh = sinh(abs_v * get_param(xform_id, variation_id, 17u));
        let ch = cosh(abs_v * get_param(xform_id, variation_id, 18u));
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = c * sh / safe_abs * get_param(xform_id, variation_id, 20u);
        let cap_b = -s * sh / safe_abs;
        let stcv = s * ch;
        let nstcv = -stcv;
        let ctcv = c * ch;
        ox = ox + cotqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy - cotqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
    }

    // cothq
    let cothqpow = get_param(xform_id, variation_id, 21u);
    if (cothqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 26u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(abs_v * get_param(xform_id, variation_id, 24u));
        let c = cos(abs_v * get_param(xform_id, variation_id, 25u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 22u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 23u));
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = ch * s / safe_abs * get_param(xform_id, variation_id, 27u);
        let cap_b = sh * s / safe_abs;
        let stcv = sh * c;
        let nstcv = -stcv;
        let ctcv = ch * c;
        ox = ox + cothqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy + cothqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
    }

    // cscq
    let cscqpow = get_param(xform_id, variation_id, 28u);
    if (cscqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 33u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(x_in * get_param(xform_id, variation_id, 29u));
        let c = cos(x_in * get_param(xform_id, variation_id, 30u));
        let sh = sinh(abs_v * get_param(xform_id, variation_id, 31u));
        let ch = cosh(abs_v * get_param(xform_id, variation_id, 32u));
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = ni * c * sh / safe_abs * get_param(xform_id, variation_id, 34u);
        ox = ox + cscqpow * s * ch * ni;
        oy = oy - cscqpow * cap * y_in;
    }

    // cschq
    let cschqpow = get_param(xform_id, variation_id, 35u);
    if (cschqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 40u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(abs_v * get_param(xform_id, variation_id, 38u));
        let c = cos(abs_v * get_param(xform_id, variation_id, 39u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 36u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 37u));
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = ni * ch * s / safe_abs * get_param(xform_id, variation_id, 41u);
        ox = ox + cschqpow * sh * c * ni;
        oy = oy - cschqpow * cap * y_in;
    }

    // estiq
    let estiqpow = get_param(xform_id, variation_id, 42u);
    if (estiqpow != 0.0) {
        let e = exp(x_in * get_param(xform_id, variation_id, 43u));
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 46u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(abs_v * get_param(xform_id, variation_id, 44u));
        let c = cos(abs_v * get_param(xform_id, variation_id, 45u));
        let a = e * s / safe_abs * get_param(xform_id, variation_id, 47u);
        ox = ox + estiqpow * e * c;
        oy = oy + estiqpow * a * y_in;
    }

    // logq
    let logqpow = get_param(xform_id, variation_id, 48u);
    if (logqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let cap = atan2(abs_v, x_in) / safe_abs;
        let log_arg = x_in * x_in + abs_v * abs_v;
        let safe_log = max(log_arg, 1e-30);
        let denom_init = get_param(xform_id, variation_id, 92u);
        ox = ox + logqpow * log(safe_log) * denom_init;
        oy = oy + logqpow * cap * y_in;
    }

    // secq
    let secqpow = get_param(xform_id, variation_id, 50u);
    if (secqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 55u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(-x_in * get_param(xform_id, variation_id, 51u));
        let c = cos(-x_in * get_param(xform_id, variation_id, 52u));
        let sh = sinh(abs_v * get_param(xform_id, variation_id, 53u));
        let ch = cosh(abs_v * get_param(xform_id, variation_id, 54u));
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = ni * s * sh / safe_abs * get_param(xform_id, variation_id, 56u);
        ox = ox + secqpow * c * ch * ni;
        oy = oy - secqpow * cap * y_in;
    }

    // sechq
    let sechqpow = get_param(xform_id, variation_id, 57u);
    if (sechqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 62u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(abs_v * get_param(xform_id, variation_id, 60u));
        let c = cos(abs_v * get_param(xform_id, variation_id, 61u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 58u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 59u));
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = ni * sh * s / safe_abs * get_param(xform_id, variation_id, 63u);
        ox = ox + sechqpow * ch * c * ni;
        oy = oy - sechqpow * cap * y_in;
    }

    // sinq
    let sinqpow = get_param(xform_id, variation_id, 64u);
    if (sinqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 69u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(x_in * get_param(xform_id, variation_id, 65u));
        let c = cos(x_in * get_param(xform_id, variation_id, 66u));
        let sh = sinh(abs_v * get_param(xform_id, variation_id, 67u));
        let ch = cosh(abs_v * get_param(xform_id, variation_id, 68u));
        let cap = c * sh / safe_abs * get_param(xform_id, variation_id, 70u);
        ox = ox + sinqpow * s * ch;
        oy = oy + sinqpow * cap * y_in;
    }

    // sinhq
    let sinhqpow = get_param(xform_id, variation_id, 71u);
    if (sinhqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 76u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(abs_v * get_param(xform_id, variation_id, 74u));
        let c = cos(abs_v * get_param(xform_id, variation_id, 75u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 72u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 73u));
        let cap = ch * s / safe_abs * get_param(xform_id, variation_id, 77u);
        ox = ox + sinhqpow * sh * c;
        oy = oy + sinhqpow * cap * y_in;
    }

    // tanq
    let tanqpow = get_param(xform_id, variation_id, 78u);
    if (tanqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 83u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(x_in * get_param(xform_id, variation_id, 79u));
        let c = cos(x_in * get_param(xform_id, variation_id, 80u));
        let sh = sinh(abs_v * get_param(xform_id, variation_id, 81u));
        let ch = cosh(abs_v * get_param(xform_id, variation_id, 82u));
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = c * sh / safe_abs * get_param(xform_id, variation_id, 84u);
        let cap_b = -s * sh / safe_abs;
        let stcv = s * ch;
        let nstcv = -stcv;
        let ctcv = c * ch;
        ox = ox + tanqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy + tanqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
    }

    // tanhq
    let tanhqpow = get_param(xform_id, variation_id, 85u);
    if (tanhqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 90u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(abs_v * get_param(xform_id, variation_id, 88u));
        let c = cos(abs_v * get_param(xform_id, variation_id, 89u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 86u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 87u));
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = ch * s / safe_abs * get_param(xform_id, variation_id, 91u);
        let cap_b = sh * s / safe_abs;
        let stcv = sh * c;
        let nstcv = -stcv;
        let ctcv = ch * c;
        ox = ox + tanhqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy + tanhqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
    }

    return vec2<f32>(ox, oy);
}
"#,
    wgsl_3d: r#"
fn variation_quaternion(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x_in = p.x;
    let y_in = p.y;
    let z_in = p.z;
    var ox = 0.0;
    var oy = 0.0;
    var oz = 0.0;

    // cosq
    let cosqpow = get_param(xform_id, variation_id, 0u);
    if (cosqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 5u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(x_in * get_param(xform_id, variation_id, 1u));
        let c = cos(x_in * get_param(xform_id, variation_id, 2u));
        let sh = sinh(abs_v * get_param(xform_id, variation_id, 3u));
        let ch = cosh(abs_v * get_param(xform_id, variation_id, 4u));
        let cap = s * sh / safe_abs * get_param(xform_id, variation_id, 6u);
        ox = ox + cosqpow * c * ch;
        oy = oy + cosqpow * cap * y_in;
        oz = oz + cosqpow * cap * z_in;
    }

    // coshq
    let coshqpow = get_param(xform_id, variation_id, 7u);
    if (coshqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 12u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(abs_v * get_param(xform_id, variation_id, 10u));
        let c = cos(abs_v * get_param(xform_id, variation_id, 11u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 8u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 9u));
        let cap = sh * s / safe_abs * get_param(xform_id, variation_id, 13u);
        ox = ox + coshqpow * ch * c;
        oy = oy + coshqpow * cap * y_in;
        oz = oz + coshqpow * cap * z_in;
    }

    // cotq
    let cotqpow = get_param(xform_id, variation_id, 14u);
    if (cotqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 19u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(x_in * get_param(xform_id, variation_id, 15u));
        let c = cos(x_in * get_param(xform_id, variation_id, 16u));
        let sh = sinh(abs_v * get_param(xform_id, variation_id, 17u));
        let ch = cosh(abs_v * get_param(xform_id, variation_id, 18u));
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = c * sh / safe_abs * get_param(xform_id, variation_id, 20u);
        let cap_b = -s * sh / safe_abs;
        let stcv = s * ch;
        let nstcv = -stcv;
        let ctcv = c * ch;
        ox = ox + cotqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy - cotqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
        oz = oz - cotqpow * (nstcv * cap_b * z_in + cap * z_in * ctcv) * ni;
    }

    // cothq
    let cothqpow = get_param(xform_id, variation_id, 21u);
    if (cothqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 26u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(abs_v * get_param(xform_id, variation_id, 24u));
        let c = cos(abs_v * get_param(xform_id, variation_id, 25u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 22u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 23u));
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = ch * s / safe_abs * get_param(xform_id, variation_id, 27u);
        let cap_b = sh * s / safe_abs;
        let stcv = sh * c;
        let nstcv = -stcv;
        let ctcv = ch * c;
        ox = ox + cothqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy + cothqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
        oz = oz + cothqpow * (nstcv * cap_b * z_in + cap * z_in * ctcv) * ni;
    }

    // cscq
    let cscqpow = get_param(xform_id, variation_id, 28u);
    if (cscqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 33u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(x_in * get_param(xform_id, variation_id, 29u));
        let c = cos(x_in * get_param(xform_id, variation_id, 30u));
        let sh = sinh(abs_v * get_param(xform_id, variation_id, 31u));
        let ch = cosh(abs_v * get_param(xform_id, variation_id, 32u));
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = ni * c * sh / safe_abs * get_param(xform_id, variation_id, 34u);
        ox = ox + cscqpow * s * ch * ni;
        oy = oy - cscqpow * cap * y_in;
        oz = oz - cscqpow * cap * z_in;
    }

    // cschq
    let cschqpow = get_param(xform_id, variation_id, 35u);
    if (cschqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 40u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(abs_v * get_param(xform_id, variation_id, 38u));
        let c = cos(abs_v * get_param(xform_id, variation_id, 39u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 36u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 37u));
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = ni * ch * s / safe_abs * get_param(xform_id, variation_id, 41u);
        ox = ox + cschqpow * sh * c * ni;
        oy = oy - cschqpow * cap * y_in;
        oz = oz - cschqpow * cap * z_in;
    }

    // estiq
    let estiqpow = get_param(xform_id, variation_id, 42u);
    if (estiqpow != 0.0) {
        let e = exp(x_in * get_param(xform_id, variation_id, 43u));
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 46u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(abs_v * get_param(xform_id, variation_id, 44u));
        let c = cos(abs_v * get_param(xform_id, variation_id, 45u));
        let a = e * s / safe_abs * get_param(xform_id, variation_id, 47u);
        ox = ox + estiqpow * e * c;
        oy = oy + estiqpow * a * y_in;
        oz = oz + estiqpow * a * z_in;
    }

    // logq
    let logqpow = get_param(xform_id, variation_id, 48u);
    if (logqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let cap = atan2(abs_v, x_in) / safe_abs;
        let log_arg = x_in * x_in + abs_v * abs_v;
        let safe_log = max(log_arg, 1e-30);
        let denom_init = get_param(xform_id, variation_id, 92u);
        ox = ox + logqpow * log(safe_log) * denom_init;
        oy = oy + logqpow * cap * y_in;
        oz = oz + logqpow * cap * z_in;
    }

    // secq
    let secqpow = get_param(xform_id, variation_id, 50u);
    if (secqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 55u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(-x_in * get_param(xform_id, variation_id, 51u));
        let c = cos(-x_in * get_param(xform_id, variation_id, 52u));
        let sh = sinh(abs_v * get_param(xform_id, variation_id, 53u));
        let ch = cosh(abs_v * get_param(xform_id, variation_id, 54u));
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = ni * s * sh / safe_abs * get_param(xform_id, variation_id, 56u);
        ox = ox + secqpow * c * ch * ni;
        oy = oy - secqpow * cap * y_in;
        oz = oz - secqpow * cap * z_in;
    }

    // sechq
    let sechqpow = get_param(xform_id, variation_id, 57u);
    if (sechqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 62u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(abs_v * get_param(xform_id, variation_id, 60u));
        let c = cos(abs_v * get_param(xform_id, variation_id, 61u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 58u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 59u));
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = ni * sh * s / safe_abs * get_param(xform_id, variation_id, 63u);
        ox = ox + sechqpow * ch * c * ni;
        oy = oy - sechqpow * cap * y_in;
        oz = oz - sechqpow * cap * z_in;
    }

    // sinq
    let sinqpow = get_param(xform_id, variation_id, 64u);
    if (sinqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 69u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(x_in * get_param(xform_id, variation_id, 65u));
        let c = cos(x_in * get_param(xform_id, variation_id, 66u));
        let sh = sinh(abs_v * get_param(xform_id, variation_id, 67u));
        let ch = cosh(abs_v * get_param(xform_id, variation_id, 68u));
        let cap = c * sh / safe_abs * get_param(xform_id, variation_id, 70u);
        ox = ox + sinqpow * s * ch;
        oy = oy + sinqpow * cap * y_in;
        oz = oz + sinqpow * cap * z_in;
    }

    // sinhq
    let sinhqpow = get_param(xform_id, variation_id, 71u);
    if (sinhqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 76u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(abs_v * get_param(xform_id, variation_id, 74u));
        let c = cos(abs_v * get_param(xform_id, variation_id, 75u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 72u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 73u));
        let cap = ch * s / safe_abs * get_param(xform_id, variation_id, 77u);
        ox = ox + sinhqpow * sh * c;
        oy = oy + sinhqpow * cap * y_in;
        oz = oz + sinhqpow * cap * z_in;
    }

    // tanq
    let tanqpow = get_param(xform_id, variation_id, 78u);
    if (tanqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 83u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(x_in * get_param(xform_id, variation_id, 79u));
        let c = cos(x_in * get_param(xform_id, variation_id, 80u));
        let sh = sinh(abs_v * get_param(xform_id, variation_id, 81u));
        let ch = cosh(abs_v * get_param(xform_id, variation_id, 82u));
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = c * sh / safe_abs * get_param(xform_id, variation_id, 84u);
        let cap_b = -s * sh / safe_abs;
        let stcv = s * ch;
        let nstcv = -stcv;
        let ctcv = c * ch;
        ox = ox + tanqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy + tanqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
        oz = oz + tanqpow * (nstcv * cap_b * z_in + cap * z_in * ctcv) * ni;
    }

    // tanhq
    let tanhqpow = get_param(xform_id, variation_id, 85u);
    if (tanhqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * get_param(xform_id, variation_id, 90u);
        let safe_abs = select(abs_v, 1e-20, abs(abs_v) < 1e-20);
        let s = sin(abs_v * get_param(xform_id, variation_id, 88u));
        let c = cos(abs_v * get_param(xform_id, variation_id, 89u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 86u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 87u));
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = select(denom, 1e-20, abs(denom) < 1e-20);
        let ni = 1.0 / safe_d;
        let cap = ch * s / safe_abs * get_param(xform_id, variation_id, 91u);
        let cap_b = sh * s / safe_abs;
        let stcv = sh * c;
        let nstcv = -stcv;
        let ctcv = ch * c;
        ox = ox + tanhqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy + tanhqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
        oz = oz + tanhqpow * (nstcv * cap_b * z_in + cap * z_in * ctcv) * ni;
    }

    return vec3<f32>(ox, oy, oz);
}
"#,
};

//! complex (cothe / Brad Stefanov)
//!
//! 2D analog of `quaternion` — combines 13 standard complex trig + exp
//! sub-functions (`cos, cosh, cot, coth, csc, csch, exp, sec, sech,
//! sin, sinh, tan, tanh`), each gated by its own `*pow` weight and
//! independently scaled per-axis via `*x1, *x2, *y1, *y2` mixers.
//!
//! 64 user params, no init slots. Over the old 16-slot ceiling — port
//! enabled by the packed-variation-params buffer.
//!
//! Slot map:
//!   cos:  0..4   (pow, x1, x2, y1, y2)
//!   cosh: 5..9
//!   cot:  10..14
//!   coth: 15..19
//!   csc:  20..24
//!   csch: 25..29
//!   exp:  30..33  (pow, x1, y1, y2)  [no x2]
//!   sec:  34..38
//!   sech: 39..43
//!   sin:  44..48
//!   sinh: 49..53
//!   tan:  54..58
//!   tanh: 59..63
//!
//! cpp drops `pAmount` factor (= our weight) since the framework's outer
//! multiplier supplies it. Divide-by-zero guards use safe denominators
//! instead of cpp's early-return pattern (which doesn't fit our model).
//!
//! Source: `output/jwildfire-vars/output/complex.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// 2D analog of `quaternion` — combines 13 complex-plane
/// trig/hyperbolic/exp sub-functions (`cos, cosh, cot, coth, csc, csch,
/// exp, sec, sech, sin, sinh, tan, tanh`), each treating the input `(x, y)`
/// as a complex number `z = x + iy` and evaluating its complex-function
/// image. Subfunctions are independently gated by their own `*pow` weight
/// (zero = disabled) and per-axis-tunable via `x1, x2, y1, y2` mixers. 12
/// subs have 5 params each; `exp` has 4 (no x2 since `e^(x·x1) · (cos,
/// sin)` is single-axis). Default has only `cos` active. 2D-only — no 3D
/// body.
///
/// # Authors
/// - cothe
/// - Brad Stefanov
pub static COMPLEX: VariationDef = VariationDef {
    name: "complex",
    aliases: &[],
    display_name: "Complex",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        param!("cospow", "Cos Pow", unlimited_float, 1.0, -10.0, 10.0, "cos: subfunction weight (complex cosine of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("cosx1", "Cos X1", unlimited_float, 1.0, -10.0, 10.0, "cos: scale applied to `x` before the first trig/hyperbolic term."),
        param!("cosx2", "Cos X2", unlimited_float, 1.0, -10.0, 10.0, "cos: scale applied to `x` before the second trig/hyperbolic term."),
        param!("cosy1", "Cos Y1", unlimited_float, 1.0, -10.0, 10.0, "cos: scale applied to `y` before the first trig/hyperbolic term."),
        param!("cosy2", "Cos Y2", unlimited_float, 1.0, -10.0, 10.0, "cos: scale applied to `y` before the second trig/hyperbolic term."),
        param!("coshpow", "Cosh Pow", unlimited_float, 0.0, -10.0, 10.0, "cosh: subfunction weight (complex hyperbolic cosine (cosh) of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("coshx1", "Cosh X1", unlimited_float, 1.0, -10.0, 10.0, "cosh: scale applied to `x` before the first trig/hyperbolic term."),
        param!("coshx2", "Cosh X2", unlimited_float, 1.0, -10.0, 10.0, "cosh: scale applied to `x` before the second trig/hyperbolic term."),
        param!("coshy1", "Cosh Y1", unlimited_float, 1.0, -10.0, 10.0, "cosh: scale applied to `y` before the first trig/hyperbolic term."),
        param!("coshy2", "Cosh Y2", unlimited_float, 1.0, -10.0, 10.0, "cosh: scale applied to `y` before the second trig/hyperbolic term."),
        param!("cotpow", "Cot Pow", unlimited_float, 0.0, -10.0, 10.0, "cot: subfunction weight (complex cotangent of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("cotx1", "Cot X1", unlimited_float, 2.0, -10.0, 10.0, "cot: scale applied to `x` before the first trig/hyperbolic term."),
        param!("cotx2", "Cot X2", unlimited_float, 2.0, -10.0, 10.0, "cot: scale applied to `x` before the second trig/hyperbolic term."),
        param!("coty1", "Cot Y1", unlimited_float, 2.0, -10.0, 10.0, "cot: scale applied to `y` before the first trig/hyperbolic term."),
        param!("coty2", "Cot Y2", unlimited_float, 2.0, -10.0, 10.0, "cot: scale applied to `y` before the second trig/hyperbolic term."),
        param!("cothpow", "Coth Pow", unlimited_float, 0.0, -10.0, 10.0, "coth: subfunction weight (complex hyperbolic cotangent (coth) of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("cothx1", "Coth X1", unlimited_float, 2.0, -10.0, 10.0, "coth: scale applied to `x` before the first trig/hyperbolic term."),
        param!("cothx2", "Coth X2", unlimited_float, 2.0, -10.0, 10.0, "coth: scale applied to `x` before the second trig/hyperbolic term."),
        param!("cothy1", "Coth Y1", unlimited_float, 2.0, -10.0, 10.0, "coth: scale applied to `y` before the first trig/hyperbolic term."),
        param!("cothy2", "Coth Y2", unlimited_float, 2.0, -10.0, 10.0, "coth: scale applied to `y` before the second trig/hyperbolic term."),
        param!("cscpow", "Csc Pow", unlimited_float, 0.0, -10.0, 10.0, "csc: subfunction weight (complex cosecant (csc) of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("cscx1", "Csc X1", unlimited_float, 1.0, -10.0, 10.0, "csc: scale applied to `x` before the first trig/hyperbolic term."),
        param!("cscx2", "Csc X2", unlimited_float, 1.0, -10.0, 10.0, "csc: scale applied to `x` before the second trig/hyperbolic term."),
        param!("cscy1", "Csc Y1", unlimited_float, 1.0, -10.0, 10.0, "csc: scale applied to `y` before the first trig/hyperbolic term."),
        param!("cscy2", "Csc Y2", unlimited_float, 1.0, -10.0, 10.0, "csc: scale applied to `y` before the second trig/hyperbolic term."),
        param!("cschpow", "Csch Pow", unlimited_float, 0.0, -10.0, 10.0, "csch: subfunction weight (complex hyperbolic cosecant (csch) of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("cschx1", "Csch X1", unlimited_float, 1.0, -10.0, 10.0, "csch: scale applied to `x` before the first trig/hyperbolic term."),
        param!("cschx2", "Csch X2", unlimited_float, 1.0, -10.0, 10.0, "csch: scale applied to `x` before the second trig/hyperbolic term."),
        param!("cschy1", "Csch Y1", unlimited_float, 1.0, -10.0, 10.0, "csch: scale applied to `y` before the first trig/hyperbolic term."),
        param!("cschy2", "Csch Y2", unlimited_float, 1.0, -10.0, 10.0, "csch: scale applied to `y` before the second trig/hyperbolic term."),
        param!("exppow", "Exp Pow", unlimited_float, 0.0, -10.0, 10.0, "exp: subfunction weight (complex exponential `e^z = e^x · (cos y, sin y)`). 0 disables; non-zero activates and scales the contribution."),
        param!("expx1", "Exp X1", unlimited_float, 1.0, -10.0, 10.0, "exp: scale applied to `x` before the exponential `e^(x · x1)`."),
        param!("expy1", "Exp Y1", unlimited_float, 1.0, -10.0, 10.0, "exp: scale applied to `y` before the cosine term."),
        param!("expy2", "Exp Y2", unlimited_float, 1.0, -10.0, 10.0, "exp: scale applied to `y` before the sine term."),
        param!("secpow", "Sec Pow", unlimited_float, 0.0, -10.0, 10.0, "sec: subfunction weight (complex secant of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("secx1", "Sec X1", unlimited_float, 1.0, -10.0, 10.0, "sec: scale applied to `x` before the first trig/hyperbolic term."),
        param!("secx2", "Sec X2", unlimited_float, 1.0, -10.0, 10.0, "sec: scale applied to `x` before the second trig/hyperbolic term."),
        param!("secy1", "Sec Y1", unlimited_float, 1.0, -10.0, 10.0, "sec: scale applied to `y` before the first trig/hyperbolic term."),
        param!("secy2", "Sec Y2", unlimited_float, 1.0, -10.0, 10.0, "sec: scale applied to `y` before the second trig/hyperbolic term."),
        param!("sechpow", "Sech Pow", unlimited_float, 0.0, -10.0, 10.0, "sech: subfunction weight (complex hyperbolic secant (sech) of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("sechx1", "Sech X1", unlimited_float, 1.0, -10.0, 10.0, "sech: scale applied to `x` before the first trig/hyperbolic term."),
        param!("sechx2", "Sech X2", unlimited_float, 1.0, -10.0, 10.0, "sech: scale applied to `x` before the second trig/hyperbolic term."),
        param!("sechy1", "Sech Y1", unlimited_float, 1.0, -10.0, 10.0, "sech: scale applied to `y` before the first trig/hyperbolic term."),
        param!("sechy2", "Sech Y2", unlimited_float, 1.0, -10.0, 10.0, "sech: scale applied to `y` before the second trig/hyperbolic term."),
        param!("sinpow", "Sin Pow", unlimited_float, 0.0, -10.0, 10.0, "sin: subfunction weight (complex sine of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("sinx1", "Sin X1", unlimited_float, 1.0, -10.0, 10.0, "sin: scale applied to `x` before the first trig/hyperbolic term."),
        param!("sinx2", "Sin X2", unlimited_float, 1.0, -10.0, 10.0, "sin: scale applied to `x` before the second trig/hyperbolic term."),
        param!("siny1", "Sin Y1", unlimited_float, 1.0, -10.0, 10.0, "sin: scale applied to `y` before the first trig/hyperbolic term."),
        param!("siny2", "Sin Y2", unlimited_float, 1.0, -10.0, 10.0, "sin: scale applied to `y` before the second trig/hyperbolic term."),
        param!("sinhpow", "Sinh Pow", unlimited_float, 0.0, -10.0, 10.0, "sinh: subfunction weight (complex hyperbolic sine (sinh) of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("sinhx1", "Sinh X1", unlimited_float, 1.0, -10.0, 10.0, "sinh: scale applied to `x` before the first trig/hyperbolic term."),
        param!("sinhx2", "Sinh X2", unlimited_float, 1.0, -10.0, 10.0, "sinh: scale applied to `x` before the second trig/hyperbolic term."),
        param!("sinhy1", "Sinh Y1", unlimited_float, 1.0, -10.0, 10.0, "sinh: scale applied to `y` before the first trig/hyperbolic term."),
        param!("sinhy2", "Sinh Y2", unlimited_float, 1.0, -10.0, 10.0, "sinh: scale applied to `y` before the second trig/hyperbolic term."),
        param!("tanpow", "Tan Pow", unlimited_float, 0.0, -10.0, 10.0, "tan: subfunction weight (complex tangent of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("tanx1", "Tan X1", unlimited_float, 2.0, -10.0, 10.0, "tan: scale applied to `x` before the first trig/hyperbolic term."),
        param!("tanx2", "Tan X2", unlimited_float, 2.0, -10.0, 10.0, "tan: scale applied to `x` before the second trig/hyperbolic term."),
        param!("tany1", "Tan Y1", unlimited_float, 2.0, -10.0, 10.0, "tan: scale applied to `y` before the first trig/hyperbolic term."),
        param!("tany2", "Tan Y2", unlimited_float, 2.0, -10.0, 10.0, "tan: scale applied to `y` before the second trig/hyperbolic term."),
        param!("tanhpow", "Tanh Pow", unlimited_float, 0.0, -10.0, 10.0, "tanh: subfunction weight (complex hyperbolic tangent (tanh) of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."),
        param!("tanhx1", "Tanh X1", unlimited_float, 2.0, -10.0, 10.0, "tanh: scale applied to `x` before the first trig/hyperbolic term."),
        param!("tanhx2", "Tanh X2", unlimited_float, 2.0, -10.0, 10.0, "tanh: scale applied to `x` before the second trig/hyperbolic term."),
        param!("tanhy1", "Tanh Y1", unlimited_float, 2.0, -10.0, 10.0, "tanh: scale applied to `y` before the first trig/hyperbolic term."),
        param!("tanhy2", "Tanh Y2", unlimited_float, 2.0, -10.0, 10.0, "tanh: scale applied to `y` before the second trig/hyperbolic term."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_complex(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x_in = p.x;
    let y_in = p.y;
    var ox = 0.0;
    var oy = 0.0;

    // cos
    let cospow = get_param(xform_id, variation_id, 0u);
    if (cospow != 0.0) {
        let s = sin(x_in * get_param(xform_id, variation_id, 1u));
        let c = cos(x_in * get_param(xform_id, variation_id, 2u));
        let sh = sinh(y_in * get_param(xform_id, variation_id, 3u));
        let ch = cosh(y_in * get_param(xform_id, variation_id, 4u));
        ox = ox + cospow * c * ch;
        oy = oy - cospow * s * sh;
    }

    // cosh
    let coshpow = get_param(xform_id, variation_id, 5u);
    if (coshpow != 0.0) {
        let sh = sinh(x_in * get_param(xform_id, variation_id, 6u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 7u));
        let s = sin(y_in * get_param(xform_id, variation_id, 8u));
        let c = cos(y_in * get_param(xform_id, variation_id, 9u));
        ox = ox + coshpow * ch * c;
        oy = oy + coshpow * sh * s;
    }

    // cot
    let cotpow = get_param(xform_id, variation_id, 10u);
    if (cotpow != 0.0) {
        let s = sin(get_param(xform_id, variation_id, 11u) * x_in);
        let c = cos(get_param(xform_id, variation_id, 12u) * x_in);
        let sh = sinh(get_param(xform_id, variation_id, 13u) * y_in);
        let ch = cosh(get_param(xform_id, variation_id, 14u) * y_in);
        let d = ch - c;
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 1.0 / safe_d;
        ox = ox + cotpow * den * s;
        oy = oy - cotpow * den * sh;
    }

    // coth
    let cothpow = get_param(xform_id, variation_id, 15u);
    if (cothpow != 0.0) {
        let sh = sinh(get_param(xform_id, variation_id, 16u) * x_in);
        let ch = cosh(get_param(xform_id, variation_id, 17u) * x_in);
        let s = sin(get_param(xform_id, variation_id, 18u) * y_in);
        let c = cos(get_param(xform_id, variation_id, 19u) * y_in);
        let d = ch - c;
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 1.0 / safe_d;
        ox = ox + cothpow * den * sh;
        oy = oy + cothpow * den * s;
    }

    // csc
    let cscpow = get_param(xform_id, variation_id, 20u);
    if (cscpow != 0.0) {
        let s = sin(x_in * get_param(xform_id, variation_id, 21u));
        let c = cos(x_in * get_param(xform_id, variation_id, 22u));
        let sh = sinh(y_in * get_param(xform_id, variation_id, 23u));
        let ch = cosh(y_in * get_param(xform_id, variation_id, 24u));
        let d = cosh(2.0 * y_in) - cos(2.0 * x_in);
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 2.0 / safe_d;
        ox = ox + cscpow * den * s * ch;
        oy = oy - cscpow * den * c * sh;
    }

    // csch
    let cschpow = get_param(xform_id, variation_id, 25u);
    if (cschpow != 0.0) {
        let s = sin(y_in * get_param(xform_id, variation_id, 28u));
        let c = cos(y_in * get_param(xform_id, variation_id, 29u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 26u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 27u));
        let d = cosh(2.0 * x_in) - cos(2.0 * y_in);
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 2.0 / safe_d;
        ox = ox + cschpow * den * sh * c;
        oy = oy - cschpow * den * ch * s;
    }

    // exp
    let exppow = get_param(xform_id, variation_id, 30u);
    if (exppow != 0.0) {
        let e = exp(x_in * get_param(xform_id, variation_id, 31u));
        let s = sin(y_in * get_param(xform_id, variation_id, 32u));
        let c = cos(y_in * get_param(xform_id, variation_id, 33u));
        ox = ox + exppow * e * c;
        oy = oy + exppow * e * s;
    }

    // sec
    let secpow = get_param(xform_id, variation_id, 34u);
    if (secpow != 0.0) {
        let s = sin(x_in * get_param(xform_id, variation_id, 35u));
        let c = cos(x_in * get_param(xform_id, variation_id, 36u));
        let sh = sinh(y_in * get_param(xform_id, variation_id, 37u));
        let ch = cosh(y_in * get_param(xform_id, variation_id, 38u));
        let d = cos(2.0 * x_in) + cosh(2.0 * y_in);
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 2.0 / safe_d;
        ox = ox + secpow * den * c * ch;
        oy = oy + secpow * den * s * sh;
    }

    // sech (cpp swaps trig/hyper assignment naming — we follow exact cpp formula)
    let sechpow = get_param(xform_id, variation_id, 39u);
    if (sechpow != 0.0) {
        let sechsinh = sin(y_in * get_param(xform_id, variation_id, 42u));
        let sechcosh = cos(y_in * get_param(xform_id, variation_id, 43u));
        let sechsin = sinh(x_in * get_param(xform_id, variation_id, 40u));
        let sechcos = cosh(x_in * get_param(xform_id, variation_id, 41u));
        let d = cos(2.0 * y_in) + cosh(2.0 * x_in);
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 2.0 / safe_d;
        ox = ox + sechpow * den * sechcos * sechcosh;
        oy = oy - sechpow * den * sechsin * sechsinh;
    }

    // sin
    let sinpow = get_param(xform_id, variation_id, 44u);
    if (sinpow != 0.0) {
        let s = sin(x_in * get_param(xform_id, variation_id, 45u));
        let c = cos(x_in * get_param(xform_id, variation_id, 46u));
        let sh = sinh(y_in * get_param(xform_id, variation_id, 47u));
        let ch = cosh(y_in * get_param(xform_id, variation_id, 48u));
        ox = ox + sinpow * s * ch;
        oy = oy + sinpow * c * sh;
    }

    // sinh
    let sinhpow = get_param(xform_id, variation_id, 49u);
    if (sinhpow != 0.0) {
        let s = sin(y_in * get_param(xform_id, variation_id, 52u));
        let c = cos(y_in * get_param(xform_id, variation_id, 53u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 50u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 51u));
        ox = ox + sinhpow * sh * c;
        oy = oy + sinhpow * ch * s;
    }

    // tan
    let tanpow = get_param(xform_id, variation_id, 54u);
    if (tanpow != 0.0) {
        let s = sin(get_param(xform_id, variation_id, 55u) * x_in);
        let c = cos(get_param(xform_id, variation_id, 56u) * x_in);
        let sh = sinh(get_param(xform_id, variation_id, 57u) * y_in);
        let ch = cosh(get_param(xform_id, variation_id, 58u) * y_in);
        let d = c + ch;
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 1.0 / safe_d;
        ox = ox + tanpow * den * s;
        oy = oy + tanpow * den * sh;
    }

    // tanh
    let tanhpow = get_param(xform_id, variation_id, 59u);
    if (tanhpow != 0.0) {
        let s = sin(y_in * get_param(xform_id, variation_id, 62u));
        let c = cos(y_in * get_param(xform_id, variation_id, 63u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 60u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 61u));
        let d = c + ch;
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 1.0 / safe_d;
        ox = ox + tanhpow * den * sh;
        oy = oy + tanhpow * den * s;
    }

    return vec2<f32>(ox, oy);
}
"#,
    wgsl_3d: r#"
fn variation_complex(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Mirrors the 2D body verbatim — `complex` is a 2D-shaped variation
    // (14 sub-flavors, all on x/y). The 3D form passes z through unchanged
    // so the external dispatch `result += weight * var(p)` contributes
    // `weight × p.z` to the z accumulator (z-identity, same shape as
    // sinusoidal/spherical/etc.).
    let x_in = p.x;
    let y_in = p.y;
    var ox = 0.0;
    var oy = 0.0;

    // cos
    let cospow = get_param(xform_id, variation_id, 0u);
    if (cospow != 0.0) {
        let s = sin(x_in * get_param(xform_id, variation_id, 1u));
        let c = cos(x_in * get_param(xform_id, variation_id, 2u));
        let sh = sinh(y_in * get_param(xform_id, variation_id, 3u));
        let ch = cosh(y_in * get_param(xform_id, variation_id, 4u));
        ox = ox + cospow * c * ch;
        oy = oy - cospow * s * sh;
    }

    // cosh
    let coshpow = get_param(xform_id, variation_id, 5u);
    if (coshpow != 0.0) {
        let sh = sinh(x_in * get_param(xform_id, variation_id, 6u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 7u));
        let s = sin(y_in * get_param(xform_id, variation_id, 8u));
        let c = cos(y_in * get_param(xform_id, variation_id, 9u));
        ox = ox + coshpow * ch * c;
        oy = oy + coshpow * sh * s;
    }

    // cot
    let cotpow = get_param(xform_id, variation_id, 10u);
    if (cotpow != 0.0) {
        let s = sin(get_param(xform_id, variation_id, 11u) * x_in);
        let c = cos(get_param(xform_id, variation_id, 12u) * x_in);
        let sh = sinh(get_param(xform_id, variation_id, 13u) * y_in);
        let ch = cosh(get_param(xform_id, variation_id, 14u) * y_in);
        let d = ch - c;
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 1.0 / safe_d;
        ox = ox + cotpow * den * s;
        oy = oy - cotpow * den * sh;
    }

    // coth
    let cothpow = get_param(xform_id, variation_id, 15u);
    if (cothpow != 0.0) {
        let sh = sinh(get_param(xform_id, variation_id, 16u) * x_in);
        let ch = cosh(get_param(xform_id, variation_id, 17u) * x_in);
        let s = sin(get_param(xform_id, variation_id, 18u) * y_in);
        let c = cos(get_param(xform_id, variation_id, 19u) * y_in);
        let d = ch - c;
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 1.0 / safe_d;
        ox = ox + cothpow * den * sh;
        oy = oy + cothpow * den * s;
    }

    // csc
    let cscpow = get_param(xform_id, variation_id, 20u);
    if (cscpow != 0.0) {
        let s = sin(x_in * get_param(xform_id, variation_id, 21u));
        let c = cos(x_in * get_param(xform_id, variation_id, 22u));
        let sh = sinh(y_in * get_param(xform_id, variation_id, 23u));
        let ch = cosh(y_in * get_param(xform_id, variation_id, 24u));
        let d = cosh(2.0 * y_in) - cos(2.0 * x_in);
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 2.0 / safe_d;
        ox = ox + cscpow * den * s * ch;
        oy = oy - cscpow * den * c * sh;
    }

    // csch
    let cschpow = get_param(xform_id, variation_id, 25u);
    if (cschpow != 0.0) {
        let s = sin(y_in * get_param(xform_id, variation_id, 28u));
        let c = cos(y_in * get_param(xform_id, variation_id, 29u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 26u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 27u));
        let d = cosh(2.0 * x_in) - cos(2.0 * y_in);
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 2.0 / safe_d;
        ox = ox + cschpow * den * sh * c;
        oy = oy - cschpow * den * ch * s;
    }

    // exp
    let exppow = get_param(xform_id, variation_id, 30u);
    if (exppow != 0.0) {
        let e = exp(x_in * get_param(xform_id, variation_id, 31u));
        let s = sin(y_in * get_param(xform_id, variation_id, 32u));
        let c = cos(y_in * get_param(xform_id, variation_id, 33u));
        ox = ox + exppow * e * c;
        oy = oy + exppow * e * s;
    }

    // sec
    let secpow = get_param(xform_id, variation_id, 34u);
    if (secpow != 0.0) {
        let s = sin(x_in * get_param(xform_id, variation_id, 35u));
        let c = cos(x_in * get_param(xform_id, variation_id, 36u));
        let sh = sinh(y_in * get_param(xform_id, variation_id, 37u));
        let ch = cosh(y_in * get_param(xform_id, variation_id, 38u));
        let d = cos(2.0 * x_in) + cosh(2.0 * y_in);
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 2.0 / safe_d;
        ox = ox + secpow * den * c * ch;
        oy = oy + secpow * den * s * sh;
    }

    // sech (cpp swaps trig/hyper assignment naming — we follow exact cpp formula)
    let sechpow = get_param(xform_id, variation_id, 39u);
    if (sechpow != 0.0) {
        let sechsinh = sin(y_in * get_param(xform_id, variation_id, 42u));
        let sechcosh = cos(y_in * get_param(xform_id, variation_id, 43u));
        let sechsin = sinh(x_in * get_param(xform_id, variation_id, 40u));
        let sechcos = cosh(x_in * get_param(xform_id, variation_id, 41u));
        let d = cos(2.0 * y_in) + cosh(2.0 * x_in);
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 2.0 / safe_d;
        ox = ox + sechpow * den * sechcos * sechcosh;
        oy = oy - sechpow * den * sechsin * sechsinh;
    }

    // sin
    let sinpow = get_param(xform_id, variation_id, 44u);
    if (sinpow != 0.0) {
        let s = sin(x_in * get_param(xform_id, variation_id, 45u));
        let c = cos(x_in * get_param(xform_id, variation_id, 46u));
        let sh = sinh(y_in * get_param(xform_id, variation_id, 47u));
        let ch = cosh(y_in * get_param(xform_id, variation_id, 48u));
        ox = ox + sinpow * s * ch;
        oy = oy + sinpow * c * sh;
    }

    // sinh
    let sinhpow = get_param(xform_id, variation_id, 49u);
    if (sinhpow != 0.0) {
        let s = sin(y_in * get_param(xform_id, variation_id, 52u));
        let c = cos(y_in * get_param(xform_id, variation_id, 53u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 50u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 51u));
        ox = ox + sinhpow * sh * c;
        oy = oy + sinhpow * ch * s;
    }

    // tan
    let tanpow = get_param(xform_id, variation_id, 54u);
    if (tanpow != 0.0) {
        let s = sin(get_param(xform_id, variation_id, 55u) * x_in);
        let c = cos(get_param(xform_id, variation_id, 56u) * x_in);
        let sh = sinh(get_param(xform_id, variation_id, 57u) * y_in);
        let ch = cosh(get_param(xform_id, variation_id, 58u) * y_in);
        let d = c + ch;
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 1.0 / safe_d;
        ox = ox + tanpow * den * s;
        oy = oy + tanpow * den * sh;
    }

    // tanh
    let tanhpow = get_param(xform_id, variation_id, 59u);
    if (tanhpow != 0.0) {
        let s = sin(y_in * get_param(xform_id, variation_id, 62u));
        let c = cos(y_in * get_param(xform_id, variation_id, 63u));
        let sh = sinh(x_in * get_param(xform_id, variation_id, 60u));
        let ch = cosh(x_in * get_param(xform_id, variation_id, 61u));
        let d = c + ch;
        let safe_d = select(d, 1e-30, abs(d) < 1e-30);
        let den = 1.0 / safe_d;
        ox = ox + tanhpow * den * sh;
        oy = oy + tanhpow * den * s;
    }

    return vec3<f32>(ox, oy, p.z);
}
"#,
};

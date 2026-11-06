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

// Complex, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    cospow: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cos Pow",
        hint: "cos: subfunction weight (complex cosine of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    cosx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cos X1",
        hint: "cos: scale applied to `x` before the first trig/hyperbolic term.",
    },
    cosx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cos X2",
        hint: "cos: scale applied to `x` before the second trig/hyperbolic term.",
    },
    cosy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cos Y1",
        hint: "cos: scale applied to `y` before the first trig/hyperbolic term.",
    },
    cosy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cos Y2",
        hint: "cos: scale applied to `y` before the second trig/hyperbolic term.",
    },
    coshpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Cosh Pow",
        hint: "cosh: subfunction weight (complex hyperbolic cosine (cosh) of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    coshx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cosh X1",
        hint: "cosh: scale applied to `x` before the first trig/hyperbolic term.",
    },
    coshx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cosh X2",
        hint: "cosh: scale applied to `x` before the second trig/hyperbolic term.",
    },
    coshy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cosh Y1",
        hint: "cosh: scale applied to `y` before the first trig/hyperbolic term.",
    },
    coshy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cosh Y2",
        hint: "cosh: scale applied to `y` before the second trig/hyperbolic term.",
    },
    cotpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Cot Pow",
        hint: "cot: subfunction weight (complex cotangent of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    cotx1: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Cot X1",
        hint: "cot: scale applied to `x` before the first trig/hyperbolic term.",
    },
    cotx2: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Cot X2",
        hint: "cot: scale applied to `x` before the second trig/hyperbolic term.",
    },
    coty1: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Cot Y1",
        hint: "cot: scale applied to `y` before the first trig/hyperbolic term.",
    },
    coty2: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Cot Y2",
        hint: "cot: scale applied to `y` before the second trig/hyperbolic term.",
    },
    cothpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Coth Pow",
        hint: "coth: subfunction weight (complex hyperbolic cotangent (coth) of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    cothx1: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Coth X1",
        hint: "coth: scale applied to `x` before the first trig/hyperbolic term.",
    },
    cothx2: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Coth X2",
        hint: "coth: scale applied to `x` before the second trig/hyperbolic term.",
    },
    cothy1: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Coth Y1",
        hint: "coth: scale applied to `y` before the first trig/hyperbolic term.",
    },
    cothy2: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Coth Y2",
        hint: "coth: scale applied to `y` before the second trig/hyperbolic term.",
    },
    cscpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Csc Pow",
        hint: "csc: subfunction weight (complex cosecant (csc) of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    cscx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Csc X1",
        hint: "csc: scale applied to `x` before the first trig/hyperbolic term.",
    },
    cscx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Csc X2",
        hint: "csc: scale applied to `x` before the second trig/hyperbolic term.",
    },
    cscy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Csc Y1",
        hint: "csc: scale applied to `y` before the first trig/hyperbolic term.",
    },
    cscy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Csc Y2",
        hint: "csc: scale applied to `y` before the second trig/hyperbolic term.",
    },
    cschpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Csch Pow",
        hint: "csch: subfunction weight (complex hyperbolic cosecant (csch) of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    cschx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Csch X1",
        hint: "csch: scale applied to `x` before the first trig/hyperbolic term.",
    },
    cschx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Csch X2",
        hint: "csch: scale applied to `x` before the second trig/hyperbolic term.",
    },
    cschy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Csch Y1",
        hint: "csch: scale applied to `y` before the first trig/hyperbolic term.",
    },
    cschy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Csch Y2",
        hint: "csch: scale applied to `y` before the second trig/hyperbolic term.",
    },
    exppow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Exp Pow",
        hint: "exp: subfunction weight (complex exponential `e^z = e^x · (cos y, sin y)`). 0 disables; non-zero activates and scales the contribution.",
    },
    expx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Exp X1",
        hint: "exp: scale applied to `x` before the exponential `e^(x · x1)`.",
    },
    expy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Exp Y1",
        hint: "exp: scale applied to `y` before the cosine term.",
    },
    expy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Exp Y2",
        hint: "exp: scale applied to `y` before the sine term.",
    },
    secpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Sec Pow",
        hint: "sec: subfunction weight (complex secant of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    secx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sec X1",
        hint: "sec: scale applied to `x` before the first trig/hyperbolic term.",
    },
    secx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sec X2",
        hint: "sec: scale applied to `x` before the second trig/hyperbolic term.",
    },
    secy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sec Y1",
        hint: "sec: scale applied to `y` before the first trig/hyperbolic term.",
    },
    secy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sec Y2",
        hint: "sec: scale applied to `y` before the second trig/hyperbolic term.",
    },
    sechpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Sech Pow",
        hint: "sech: subfunction weight (complex hyperbolic secant (sech) of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    sechx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sech X1",
        hint: "sech: scale applied to `x` before the first trig/hyperbolic term.",
    },
    sechx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sech X2",
        hint: "sech: scale applied to `x` before the second trig/hyperbolic term.",
    },
    sechy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sech Y1",
        hint: "sech: scale applied to `y` before the first trig/hyperbolic term.",
    },
    sechy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sech Y2",
        hint: "sech: scale applied to `y` before the second trig/hyperbolic term.",
    },
    sinpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Sin Pow",
        hint: "sin: subfunction weight (complex sine of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    sinx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sin X1",
        hint: "sin: scale applied to `x` before the first trig/hyperbolic term.",
    },
    sinx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sin X2",
        hint: "sin: scale applied to `x` before the second trig/hyperbolic term.",
    },
    siny1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sin Y1",
        hint: "sin: scale applied to `y` before the first trig/hyperbolic term.",
    },
    siny2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sin Y2",
        hint: "sin: scale applied to `y` before the second trig/hyperbolic term.",
    },
    sinhpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinh Pow",
        hint: "sinh: subfunction weight (complex hyperbolic sine (sinh) of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    sinhx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinh X1",
        hint: "sinh: scale applied to `x` before the first trig/hyperbolic term.",
    },
    sinhx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinh X2",
        hint: "sinh: scale applied to `x` before the second trig/hyperbolic term.",
    },
    sinhy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinh Y1",
        hint: "sinh: scale applied to `y` before the first trig/hyperbolic term.",
    },
    sinhy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinh Y2",
        hint: "sinh: scale applied to `y` before the second trig/hyperbolic term.",
    },
    tanpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Tan Pow",
        hint: "tan: subfunction weight (complex tangent of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    tanx1: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Tan X1",
        hint: "tan: scale applied to `x` before the first trig/hyperbolic term.",
    },
    tanx2: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Tan X2",
        hint: "tan: scale applied to `x` before the second trig/hyperbolic term.",
    },
    tany1: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Tan Y1",
        hint: "tan: scale applied to `y` before the first trig/hyperbolic term.",
    },
    tany2: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Tan Y2",
        hint: "tan: scale applied to `y` before the second trig/hyperbolic term.",
    },
    tanhpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanh Pow",
        hint: "tanh: subfunction weight (complex hyperbolic tangent (tanh) of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    tanhx1: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanh X1",
        hint: "tanh: scale applied to `x` before the first trig/hyperbolic term.",
    },
    tanhx2: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanh X2",
        hint: "tanh: scale applied to `x` before the second trig/hyperbolic term.",
    },
    tanhy1: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanh Y1",
        hint: "tanh: scale applied to `y` before the first trig/hyperbolic term.",
    },
    tanhy2: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanh Y2",
        hint: "tanh: scale applied to `y` before the second trig/hyperbolic term.",
    },
};

const INFO = #{ title: "Complex", name: "rhai_complex", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let x_in = z.re;
    let y_in = z.im;
    let ox = 0.0;
    let oy = 0.0;
    let cospow = p.cospow;
    if (cospow != 0.0) {
        let s = sin(x_in * p.cosx1);
        let c = cos(x_in * p.cosx2);
        let sh = sinh(y_in * p.cosy1);
        let ch = cosh(y_in * p.cosy2);
        ox = ox + cospow * c * ch;
        oy = oy - cospow * s * sh;
    }
    let coshpow = p.coshpow;
    if (coshpow != 0.0) {
        let sh = sinh(x_in * p.coshx1);
        let ch = cosh(x_in * p.coshx2);
        let s = sin(y_in * p.coshy1);
        let c = cos(y_in * p.coshy2);
        ox = ox + coshpow * ch * c;
        oy = oy + coshpow * sh * s;
    }
    let cotpow = p.cotpow;
    if (cotpow != 0.0) {
        let s = sin(p.cotx1 * x_in);
        let c = cos(p.cotx2 * x_in);
        let sh = sinh(p.coty1 * y_in);
        let ch = cosh(p.coty2 * y_in);
        let d = ch - c;
        let safe_d = (if abs(d) < 1.0e-30 { 1.0e-30 } else { d });
        let den = 1.0 / safe_d;
        ox = ox + cotpow * den * s;
        oy = oy - cotpow * den * sh;
    }
    let cothpow = p.cothpow;
    if (cothpow != 0.0) {
        let sh = sinh(p.cothx1 * x_in);
        let ch = cosh(p.cothx2 * x_in);
        let s = sin(p.cothy1 * y_in);
        let c = cos(p.cothy2 * y_in);
        let d = ch - c;
        let safe_d = (if abs(d) < 1.0e-30 { 1.0e-30 } else { d });
        let den = 1.0 / safe_d;
        ox = ox + cothpow * den * sh;
        oy = oy + cothpow * den * s;
    }
    let cscpow = p.cscpow;
    if (cscpow != 0.0) {
        let s = sin(x_in * p.cscx1);
        let c = cos(x_in * p.cscx2);
        let sh = sinh(y_in * p.cscy1);
        let ch = cosh(y_in * p.cscy2);
        let d = cosh(2.0 * y_in) - cos(2.0 * x_in);
        let safe_d = (if abs(d) < 1.0e-30 { 1.0e-30 } else { d });
        let den = 2.0 / safe_d;
        ox = ox + cscpow * den * s * ch;
        oy = oy - cscpow * den * c * sh;
    }
    let cschpow = p.cschpow;
    if (cschpow != 0.0) {
        let s = sin(y_in * p.cschy1);
        let c = cos(y_in * p.cschy2);
        let sh = sinh(x_in * p.cschx1);
        let ch = cosh(x_in * p.cschx2);
        let d = cosh(2.0 * x_in) - cos(2.0 * y_in);
        let safe_d = (if abs(d) < 1.0e-30 { 1.0e-30 } else { d });
        let den = 2.0 / safe_d;
        ox = ox + cschpow * den * sh * c;
        oy = oy - cschpow * den * ch * s;
    }
    let exppow = p.exppow;
    if (exppow != 0.0) {
        let e = exp(x_in * p.expx1);
        let s = sin(y_in * p.expy1);
        let c = cos(y_in * p.expy2);
        ox = ox + exppow * e * c;
        oy = oy + exppow * e * s;
    }
    let secpow = p.secpow;
    if (secpow != 0.0) {
        let s = sin(x_in * p.secx1);
        let c = cos(x_in * p.secx2);
        let sh = sinh(y_in * p.secy1);
        let ch = cosh(y_in * p.secy2);
        let d = cos(2.0 * x_in) + cosh(2.0 * y_in);
        let safe_d = (if abs(d) < 1.0e-30 { 1.0e-30 } else { d });
        let den = 2.0 / safe_d;
        ox = ox + secpow * den * c * ch;
        oy = oy + secpow * den * s * sh;
    }
    let sechpow = p.sechpow;
    if (sechpow != 0.0) {
        let sechsinh = sin(y_in * p.sechy1);
        let sechcosh = cos(y_in * p.sechy2);
        let sechsin = sinh(x_in * p.sechx1);
        let sechcos = cosh(x_in * p.sechx2);
        let d = cos(2.0 * y_in) + cosh(2.0 * x_in);
        let safe_d = (if abs(d) < 1.0e-30 { 1.0e-30 } else { d });
        let den = 2.0 / safe_d;
        ox = ox + sechpow * den * sechcos * sechcosh;
        oy = oy - sechpow * den * sechsin * sechsinh;
    }
    let sinpow = p.sinpow;
    if (sinpow != 0.0) {
        let s = sin(x_in * p.sinx1);
        let c = cos(x_in * p.sinx2);
        let sh = sinh(y_in * p.siny1);
        let ch = cosh(y_in * p.siny2);
        ox = ox + sinpow * s * ch;
        oy = oy + sinpow * c * sh;
    }
    let sinhpow = p.sinhpow;
    if (sinhpow != 0.0) {
        let s = sin(y_in * p.sinhy1);
        let c = cos(y_in * p.sinhy2);
        let sh = sinh(x_in * p.sinhx1);
        let ch = cosh(x_in * p.sinhx2);
        ox = ox + sinhpow * sh * c;
        oy = oy + sinhpow * ch * s;
    }
    let tanpow = p.tanpow;
    if (tanpow != 0.0) {
        let s = sin(p.tanx1 * x_in);
        let c = cos(p.tanx2 * x_in);
        let sh = sinh(p.tany1 * y_in);
        let ch = cosh(p.tany2 * y_in);
        let d = c + ch;
        let safe_d = (if abs(d) < 1.0e-30 { 1.0e-30 } else { d });
        let den = 1.0 / safe_d;
        ox = ox + tanpow * den * s;
        oy = oy + tanpow * den * sh;
    }
    let tanhpow = p.tanhpow;
    if (tanhpow != 0.0) {
        let s = sin(y_in * p.tanhy1);
        let c = cos(y_in * p.tanhy2);
        let sh = sinh(x_in * p.tanhx1);
        let ch = cosh(x_in * p.tanhx2);
        let d = c + ch;
        let safe_d = (if abs(d) < 1.0e-30 { 1.0e-30 } else { d });
        let den = 1.0 / safe_d;
        ox = ox + tanhpow * den * sh;
        oy = oy + tanhpow * den * s;
    }
    complex(ox, oy)
}

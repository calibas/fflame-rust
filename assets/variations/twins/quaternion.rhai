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

// Quaternion, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    cosqpow: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cosq Pow",
        hint: "cosq: subfunction weight (quaternion-extended cosine). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    cosqx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cosq X1",
        hint: "cosq: scale applied to `x` before the first trig/hyperbolic term.",
    },
    cosqx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cosq X2",
        hint: "cosq: scale applied to `x` before the second trig/hyperbolic term.",
    },
    cosqy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cosq Y1",
        hint: "cosq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term.",
    },
    cosqy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cosq Y2",
        hint: "cosq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term.",
    },
    cosqz1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cosq Z1",
        hint: "cosq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings).",
    },
    cosqz2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cosq Z2",
        hint: "cosq: scale applied to the Y/Z cross-term that builds the off-axis output.",
    },
    coshqpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Coshq Pow",
        hint: "coshq: subfunction weight (quaternion-extended hyperbolic cosine (cosh)). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    coshqx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Coshq X1",
        hint: "coshq: scale applied to `x` before the first trig/hyperbolic term.",
    },
    coshqx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Coshq X2",
        hint: "coshq: scale applied to `x` before the second trig/hyperbolic term.",
    },
    coshqy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Coshq Y1",
        hint: "coshq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term.",
    },
    coshqy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Coshq Y2",
        hint: "coshq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term.",
    },
    coshqz1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Coshq Z1",
        hint: "coshq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings).",
    },
    coshqz2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Coshq Z2",
        hint: "coshq: scale applied to the Y/Z cross-term that builds the off-axis output.",
    },
    cotqpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Cotq Pow",
        hint: "cotq: subfunction weight (quaternion-extended cotangent). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    cotqx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cotq X1",
        hint: "cotq: scale applied to `x` before the first trig/hyperbolic term.",
    },
    cotqx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cotq X2",
        hint: "cotq: scale applied to `x` before the second trig/hyperbolic term.",
    },
    cotqy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cotq Y1",
        hint: "cotq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term.",
    },
    cotqy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cotq Y2",
        hint: "cotq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term.",
    },
    cotqz1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cotq Z1",
        hint: "cotq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings).",
    },
    cotqz2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cotq Z2",
        hint: "cotq: scale applied to the Y/Z cross-term that builds the off-axis output.",
    },
    cothqpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Cothq Pow",
        hint: "cothq: subfunction weight (quaternion-extended hyperbolic cotangent (coth)). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    cothqx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cothq X1",
        hint: "cothq: scale applied to `x` before the first trig/hyperbolic term.",
    },
    cothqx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cothq X2",
        hint: "cothq: scale applied to `x` before the second trig/hyperbolic term.",
    },
    cothqy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cothq Y1",
        hint: "cothq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term.",
    },
    cothqy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cothq Y2",
        hint: "cothq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term.",
    },
    cothqz1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cothq Z1",
        hint: "cothq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings).",
    },
    cothqz2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cothq Z2",
        hint: "cothq: scale applied to the Y/Z cross-term that builds the off-axis output.",
    },
    cscqpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Cscq Pow",
        hint: "cscq: subfunction weight (quaternion-extended cosecant (csc)). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    cscqx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cscq X1",
        hint: "cscq: scale applied to `x` before the first trig/hyperbolic term.",
    },
    cscqx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cscq X2",
        hint: "cscq: scale applied to `x` before the second trig/hyperbolic term.",
    },
    cscqy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cscq Y1",
        hint: "cscq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term.",
    },
    cscqy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cscq Y2",
        hint: "cscq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term.",
    },
    cscqz1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cscq Z1",
        hint: "cscq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings).",
    },
    cscqz2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cscq Z2",
        hint: "cscq: scale applied to the Y/Z cross-term that builds the off-axis output.",
    },
    cschqpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Cschq Pow",
        hint: "cschq: subfunction weight (quaternion-extended hyperbolic cosecant (csch)). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    cschqx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cschq X1",
        hint: "cschq: scale applied to `x` before the first trig/hyperbolic term.",
    },
    cschqx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cschq X2",
        hint: "cschq: scale applied to `x` before the second trig/hyperbolic term.",
    },
    cschqy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cschq Y1",
        hint: "cschq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term.",
    },
    cschqy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cschq Y2",
        hint: "cschq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term.",
    },
    cschqz1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cschq Z1",
        hint: "cschq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings).",
    },
    cschqz2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cschq Z2",
        hint: "cschq: scale applied to the Y/Z cross-term that builds the off-axis output.",
    },
    estiqpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Estiq Pow",
        hint: "estiq: subfunction weight (exp-times-trig: `e^x · (cos, sin)`). 0 disables; non-zero activates and scales the contribution.",
    },
    estiqx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Estiq X1",
        hint: "estiq: scale applied to `x` before the exponential `e^(x · x1)`.",
    },
    estiqy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Estiq Y1",
        hint: "estiq: scale applied to the `|y, z|` magnitude before the sine term.",
    },
    estiqy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Estiq Y2",
        hint: "estiq: scale applied to the `|y, z|` magnitude before the cosine term.",
    },
    estiqz1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Estiq Z1",
        hint: "estiq: pre-multiplier on the `|y, z|` magnitude.",
    },
    estiqz2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Estiq Z2",
        hint: "estiq: scale applied to the Y/Z cross-term.",
    },
    logqpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Logq Pow",
        hint: "logq: subfunction weight (complex logarithm: `(½·log|z|², atan2(|y,z|, x))`). 0 disables; non-zero activates and scales the contribution. X uses the pre-computed `_denom = 0.5 / ln(base)`.",
    },
    logqbase: #{
        value: 2.7182817, min: 0.001, max: 100.0, free: true,
        caption: "Logq Base",
        hint: "logq: logarithm base. Pre-computed at init into `_denom = 0.5 / ln(base)`. Default `e` (~2.718) gives the natural log; 10 gives common log; 2 gives binary log.",
    },
    secqpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Secq Pow",
        hint: "secq: subfunction weight (quaternion-extended secant). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    secqx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Secq X1",
        hint: "secq: scale applied to `x` before the first trig/hyperbolic term.",
    },
    secqx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Secq X2",
        hint: "secq: scale applied to `x` before the second trig/hyperbolic term.",
    },
    secqy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Secq Y1",
        hint: "secq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term.",
    },
    secqy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Secq Y2",
        hint: "secq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term.",
    },
    secqz1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Secq Z1",
        hint: "secq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings).",
    },
    secqz2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Secq Z2",
        hint: "secq: scale applied to the Y/Z cross-term that builds the off-axis output.",
    },
    sechqpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Sechq Pow",
        hint: "sechq: subfunction weight (quaternion-extended hyperbolic secant (sech)). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    sechqx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sechq X1",
        hint: "sechq: scale applied to `x` before the first trig/hyperbolic term.",
    },
    sechqx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sechq X2",
        hint: "sechq: scale applied to `x` before the second trig/hyperbolic term.",
    },
    sechqy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sechq Y1",
        hint: "sechq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term.",
    },
    sechqy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sechq Y2",
        hint: "sechq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term.",
    },
    sechqz1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sechq Z1",
        hint: "sechq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings).",
    },
    sechqz2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sechq Z2",
        hint: "sechq: scale applied to the Y/Z cross-term that builds the off-axis output.",
    },
    sinqpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinq Pow",
        hint: "sinq: subfunction weight (quaternion-extended sine). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    sinqx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinq X1",
        hint: "sinq: scale applied to `x` before the first trig/hyperbolic term.",
    },
    sinqx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinq X2",
        hint: "sinq: scale applied to `x` before the second trig/hyperbolic term.",
    },
    sinqy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinq Y1",
        hint: "sinq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term.",
    },
    sinqy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinq Y2",
        hint: "sinq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term.",
    },
    sinqz1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinq Z1",
        hint: "sinq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings).",
    },
    sinqz2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinq Z2",
        hint: "sinq: scale applied to the Y/Z cross-term that builds the off-axis output.",
    },
    sinhqpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinhq Pow",
        hint: "sinhq: subfunction weight (quaternion-extended hyperbolic sine (sinh)). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    sinhqx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinhq X1",
        hint: "sinhq: scale applied to `x` before the first trig/hyperbolic term.",
    },
    sinhqx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinhq X2",
        hint: "sinhq: scale applied to `x` before the second trig/hyperbolic term.",
    },
    sinhqy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinhq Y1",
        hint: "sinhq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term.",
    },
    sinhqy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinhq Y2",
        hint: "sinhq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term.",
    },
    sinhqz1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinhq Z1",
        hint: "sinhq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings).",
    },
    sinhqz2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sinhq Z2",
        hint: "sinhq: scale applied to the Y/Z cross-term that builds the off-axis output.",
    },
    tanqpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanq Pow",
        hint: "tanq: subfunction weight (quaternion-extended tangent). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    tanqx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanq X1",
        hint: "tanq: scale applied to `x` before the first trig/hyperbolic term.",
    },
    tanqx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanq X2",
        hint: "tanq: scale applied to `x` before the second trig/hyperbolic term.",
    },
    tanqy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanq Y1",
        hint: "tanq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term.",
    },
    tanqy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanq Y2",
        hint: "tanq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term.",
    },
    tanqz1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanq Z1",
        hint: "tanq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings).",
    },
    tanqz2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanq Z2",
        hint: "tanq: scale applied to the Y/Z cross-term that builds the off-axis output.",
    },
    tanhqpow: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanhq Pow",
        hint: "tanhq: subfunction weight (quaternion-extended hyperbolic tangent (tanh)). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output.",
    },
    tanhqx1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanhq X1",
        hint: "tanhq: scale applied to `x` before the first trig/hyperbolic term.",
    },
    tanhqx2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanhq X2",
        hint: "tanhq: scale applied to `x` before the second trig/hyperbolic term.",
    },
    tanhqy1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanhq Y1",
        hint: "tanhq: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term.",
    },
    tanhqy2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanhq Y2",
        hint: "tanhq: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term.",
    },
    tanhqz1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanhq Z1",
        hint: "tanhq: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings).",
    },
    tanhqz2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Tanhq Z2",
        hint: "tanhq: scale applied to the Y/Z cross-term that builds the off-axis output.",
    },
};

const INFO = #{ title: "Quaternion", name: "rhai_quaternion", category: "full_3d", phase: "any" };

fn prepare(p) {
    let logqbase = p.logqbase;
    let safe_base = max(1.0001, logqbase);
    #{ denom_init: 0.5 / ln(safe_base) }
}

fn variation(z, p) {
    let x_in = z.re;
    let y_in = z.im;
    let z_in = 0.0;
    let ox = 0.0;
    let oy = 0.0;
    let cosqpow = p.cosqpow;
    if (cosqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.cosqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(x_in * p.cosqx1);
        let c = cos(x_in * p.cosqx2);
        let sh = sinh(abs_v * p.cosqy1);
        let ch = cosh(abs_v * p.cosqy2);
        let cap = s * sh / safe_abs * p.cosqz2;
        ox = ox + cosqpow * c * ch;
        oy = oy + cosqpow * cap * y_in;
    }
    let coshqpow = p.coshqpow;
    if (coshqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.coshqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(abs_v * p.coshqy1);
        let c = cos(abs_v * p.coshqy2);
        let sh = sinh(x_in * p.coshqx1);
        let ch = cosh(x_in * p.coshqx2);
        let cap = sh * s / safe_abs * p.coshqz2;
        ox = ox + coshqpow * ch * c;
        oy = oy + coshqpow * cap * y_in;
    }
    let cotqpow = p.cotqpow;
    if (cotqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.cotqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(x_in * p.cotqx1);
        let c = cos(x_in * p.cotqx2);
        let sh = sinh(abs_v * p.cotqy1);
        let ch = cosh(abs_v * p.cotqy2);
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = c * sh / safe_abs * p.cotqz2;
        let cap_b = -s * sh / safe_abs;
        let stcv = s * ch;
        let nstcv = -stcv;
        let ctcv = c * ch;
        ox = ox + cotqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy - cotqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
    }
    let cothqpow = p.cothqpow;
    if (cothqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.cothqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(abs_v * p.cothqy1);
        let c = cos(abs_v * p.cothqy2);
        let sh = sinh(x_in * p.cothqx1);
        let ch = cosh(x_in * p.cothqx2);
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = ch * s / safe_abs * p.cothqz2;
        let cap_b = sh * s / safe_abs;
        let stcv = sh * c;
        let nstcv = -stcv;
        let ctcv = ch * c;
        ox = ox + cothqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy + cothqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
    }
    let cscqpow = p.cscqpow;
    if (cscqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.cscqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(x_in * p.cscqx1);
        let c = cos(x_in * p.cscqx2);
        let sh = sinh(abs_v * p.cscqy1);
        let ch = cosh(abs_v * p.cscqy2);
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = ni * c * sh / safe_abs * p.cscqz2;
        ox = ox + cscqpow * s * ch * ni;
        oy = oy - cscqpow * cap * y_in;
    }
    let cschqpow = p.cschqpow;
    if (cschqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.cschqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(abs_v * p.cschqy1);
        let c = cos(abs_v * p.cschqy2);
        let sh = sinh(x_in * p.cschqx1);
        let ch = cosh(x_in * p.cschqx2);
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = ni * ch * s / safe_abs * p.cschqz2;
        ox = ox + cschqpow * sh * c * ni;
        oy = oy - cschqpow * cap * y_in;
    }
    let estiqpow = p.estiqpow;
    if (estiqpow != 0.0) {
        let e = exp(x_in * p.estiqx1);
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.estiqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(abs_v * p.estiqy1);
        let c = cos(abs_v * p.estiqy2);
        let a = e * s / safe_abs * p.estiqz2;
        ox = ox + estiqpow * e * c;
        oy = oy + estiqpow * a * y_in;
    }
    let logqpow = p.logqpow;
    if (logqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in);
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let cap = atan2(abs_v, x_in) / safe_abs;
        let log_arg = x_in * x_in + abs_v * abs_v;
        let safe_log = max(1.0e-30, log_arg);
        let denom_init = p.denom_init;
        ox = ox + logqpow * ln(safe_log) * denom_init;
        oy = oy + logqpow * cap * y_in;
    }
    let secqpow = p.secqpow;
    if (secqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.secqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(-x_in * p.secqx1);
        let c = cos(-x_in * p.secqx2);
        let sh = sinh(abs_v * p.secqy1);
        let ch = cosh(abs_v * p.secqy2);
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = ni * s * sh / safe_abs * p.secqz2;
        ox = ox + secqpow * c * ch * ni;
        oy = oy - secqpow * cap * y_in;
    }
    let sechqpow = p.sechqpow;
    if (sechqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.sechqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(abs_v * p.sechqy1);
        let c = cos(abs_v * p.sechqy2);
        let sh = sinh(x_in * p.sechqx1);
        let ch = cosh(x_in * p.sechqx2);
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = ni * sh * s / safe_abs * p.sechqz2;
        ox = ox + sechqpow * ch * c * ni;
        oy = oy - sechqpow * cap * y_in;
    }
    let sinqpow = p.sinqpow;
    if (sinqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.sinqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(x_in * p.sinqx1);
        let c = cos(x_in * p.sinqx2);
        let sh = sinh(abs_v * p.sinqy1);
        let ch = cosh(abs_v * p.sinqy2);
        let cap = c * sh / safe_abs * p.sinqz2;
        ox = ox + sinqpow * s * ch;
        oy = oy + sinqpow * cap * y_in;
    }
    let sinhqpow = p.sinhqpow;
    if (sinhqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.sinhqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(abs_v * p.sinhqy1);
        let c = cos(abs_v * p.sinhqy2);
        let sh = sinh(x_in * p.sinhqx1);
        let ch = cosh(x_in * p.sinhqx2);
        let cap = ch * s / safe_abs * p.sinhqz2;
        ox = ox + sinhqpow * sh * c;
        oy = oy + sinhqpow * cap * y_in;
    }
    let tanqpow = p.tanqpow;
    if (tanqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.tanqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(x_in * p.tanqx1);
        let c = cos(x_in * p.tanqx2);
        let sh = sinh(abs_v * p.tanqy1);
        let ch = cosh(abs_v * p.tanqy2);
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = c * sh / safe_abs * p.tanqz2;
        let cap_b = -s * sh / safe_abs;
        let stcv = s * ch;
        let nstcv = -stcv;
        let ctcv = c * ch;
        ox = ox + tanqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy + tanqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
    }
    let tanhqpow = p.tanhqpow;
    if (tanhqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.tanhqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(abs_v * p.tanhqy1);
        let c = cos(abs_v * p.tanhqy2);
        let sh = sinh(x_in * p.tanhqx1);
        let ch = cosh(x_in * p.tanhqx2);
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = ch * s / safe_abs * p.tanhqz2;
        let cap_b = sh * s / safe_abs;
        let stcv = sh * c;
        let nstcv = -stcv;
        let ctcv = ch * c;
        ox = ox + tanhqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy + tanhqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
    }
    complex(ox, oy)
}

fn variation3(v, p) {
    let x_in = v.x;
    let y_in = v.y;
    let z_in = v.z;
    let ox = 0.0;
    let oy = 0.0;
    let oz = 0.0;
    let cosqpow = p.cosqpow;
    if (cosqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.cosqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(x_in * p.cosqx1);
        let c = cos(x_in * p.cosqx2);
        let sh = sinh(abs_v * p.cosqy1);
        let ch = cosh(abs_v * p.cosqy2);
        let cap = s * sh / safe_abs * p.cosqz2;
        ox = ox + cosqpow * c * ch;
        oy = oy + cosqpow * cap * y_in;
        oz = oz + cosqpow * cap * z_in;
    }
    let coshqpow = p.coshqpow;
    if (coshqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.coshqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(abs_v * p.coshqy1);
        let c = cos(abs_v * p.coshqy2);
        let sh = sinh(x_in * p.coshqx1);
        let ch = cosh(x_in * p.coshqx2);
        let cap = sh * s / safe_abs * p.coshqz2;
        ox = ox + coshqpow * ch * c;
        oy = oy + coshqpow * cap * y_in;
        oz = oz + coshqpow * cap * z_in;
    }
    let cotqpow = p.cotqpow;
    if (cotqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.cotqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(x_in * p.cotqx1);
        let c = cos(x_in * p.cotqx2);
        let sh = sinh(abs_v * p.cotqy1);
        let ch = cosh(abs_v * p.cotqy2);
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = c * sh / safe_abs * p.cotqz2;
        let cap_b = -s * sh / safe_abs;
        let stcv = s * ch;
        let nstcv = -stcv;
        let ctcv = c * ch;
        ox = ox + cotqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy - cotqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
        oz = oz - cotqpow * (nstcv * cap_b * z_in + cap * z_in * ctcv) * ni;
    }
    let cothqpow = p.cothqpow;
    if (cothqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.cothqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(abs_v * p.cothqy1);
        let c = cos(abs_v * p.cothqy2);
        let sh = sinh(x_in * p.cothqx1);
        let ch = cosh(x_in * p.cothqx2);
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = ch * s / safe_abs * p.cothqz2;
        let cap_b = sh * s / safe_abs;
        let stcv = sh * c;
        let nstcv = -stcv;
        let ctcv = ch * c;
        ox = ox + cothqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy + cothqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
        oz = oz + cothqpow * (nstcv * cap_b * z_in + cap * z_in * ctcv) * ni;
    }
    let cscqpow = p.cscqpow;
    if (cscqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.cscqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(x_in * p.cscqx1);
        let c = cos(x_in * p.cscqx2);
        let sh = sinh(abs_v * p.cscqy1);
        let ch = cosh(abs_v * p.cscqy2);
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = ni * c * sh / safe_abs * p.cscqz2;
        ox = ox + cscqpow * s * ch * ni;
        oy = oy - cscqpow * cap * y_in;
        oz = oz - cscqpow * cap * z_in;
    }
    let cschqpow = p.cschqpow;
    if (cschqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.cschqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(abs_v * p.cschqy1);
        let c = cos(abs_v * p.cschqy2);
        let sh = sinh(x_in * p.cschqx1);
        let ch = cosh(x_in * p.cschqx2);
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = ni * ch * s / safe_abs * p.cschqz2;
        ox = ox + cschqpow * sh * c * ni;
        oy = oy - cschqpow * cap * y_in;
        oz = oz - cschqpow * cap * z_in;
    }
    let estiqpow = p.estiqpow;
    if (estiqpow != 0.0) {
        let e = exp(x_in * p.estiqx1);
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.estiqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(abs_v * p.estiqy1);
        let c = cos(abs_v * p.estiqy2);
        let a = e * s / safe_abs * p.estiqz2;
        ox = ox + estiqpow * e * c;
        oy = oy + estiqpow * a * y_in;
        oz = oz + estiqpow * a * z_in;
    }
    let logqpow = p.logqpow;
    if (logqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in);
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let cap = atan2(abs_v, x_in) / safe_abs;
        let log_arg = x_in * x_in + abs_v * abs_v;
        let safe_log = max(1.0e-30, log_arg);
        let denom_init = p.denom_init;
        ox = ox + logqpow * ln(safe_log) * denom_init;
        oy = oy + logqpow * cap * y_in;
        oz = oz + logqpow * cap * z_in;
    }
    let secqpow = p.secqpow;
    if (secqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.secqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(-x_in * p.secqx1);
        let c = cos(-x_in * p.secqx2);
        let sh = sinh(abs_v * p.secqy1);
        let ch = cosh(abs_v * p.secqy2);
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = ni * s * sh / safe_abs * p.secqz2;
        ox = ox + secqpow * c * ch * ni;
        oy = oy - secqpow * cap * y_in;
        oz = oz - secqpow * cap * z_in;
    }
    let sechqpow = p.sechqpow;
    if (sechqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.sechqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(abs_v * p.sechqy1);
        let c = cos(abs_v * p.sechqy2);
        let sh = sinh(x_in * p.sechqx1);
        let ch = cosh(x_in * p.sechqx2);
        let denom = x_in * x_in + y_in * y_in + z_in * z_in;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = ni * sh * s / safe_abs * p.sechqz2;
        ox = ox + sechqpow * ch * c * ni;
        oy = oy - sechqpow * cap * y_in;
        oz = oz - sechqpow * cap * z_in;
    }
    let sinqpow = p.sinqpow;
    if (sinqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.sinqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(x_in * p.sinqx1);
        let c = cos(x_in * p.sinqx2);
        let sh = sinh(abs_v * p.sinqy1);
        let ch = cosh(abs_v * p.sinqy2);
        let cap = c * sh / safe_abs * p.sinqz2;
        ox = ox + sinqpow * s * ch;
        oy = oy + sinqpow * cap * y_in;
        oz = oz + sinqpow * cap * z_in;
    }
    let sinhqpow = p.sinhqpow;
    if (sinhqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.sinhqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(abs_v * p.sinhqy1);
        let c = cos(abs_v * p.sinhqy2);
        let sh = sinh(x_in * p.sinhqx1);
        let ch = cosh(x_in * p.sinhqx2);
        let cap = ch * s / safe_abs * p.sinhqz2;
        ox = ox + sinhqpow * sh * c;
        oy = oy + sinhqpow * cap * y_in;
        oz = oz + sinhqpow * cap * z_in;
    }
    let tanqpow = p.tanqpow;
    if (tanqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.tanqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(x_in * p.tanqx1);
        let c = cos(x_in * p.tanqx2);
        let sh = sinh(abs_v * p.tanqy1);
        let ch = cosh(abs_v * p.tanqy2);
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = c * sh / safe_abs * p.tanqz2;
        let cap_b = -s * sh / safe_abs;
        let stcv = s * ch;
        let nstcv = -stcv;
        let ctcv = c * ch;
        ox = ox + tanqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy + tanqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
        oz = oz + tanqpow * (nstcv * cap_b * z_in + cap * z_in * ctcv) * ni;
    }
    let tanhqpow = p.tanhqpow;
    if (tanhqpow != 0.0) {
        let abs_v = sqrt(y_in * y_in + z_in * z_in) * p.tanhqz1;
        let safe_abs = (if abs(abs_v) < 1.0e-20 { 1.0e-20 } else { abs_v });
        let s = sin(abs_v * p.tanhqy1);
        let c = cos(abs_v * p.tanhqy2);
        let sh = sinh(x_in * p.tanhqx1);
        let ch = cosh(x_in * p.tanhqx2);
        let sysz = y_in * y_in + z_in * z_in;
        let denom = x_in * x_in + sysz;
        let safe_d = (if abs(denom) < 1.0e-20 { 1.0e-20 } else { denom });
        let ni = 1.0 / safe_d;
        let cap = ch * s / safe_abs * p.tanhqz2;
        let cap_b = sh * s / safe_abs;
        let stcv = sh * c;
        let nstcv = -stcv;
        let ctcv = ch * c;
        ox = ox + tanhqpow * (stcv * ctcv + cap * cap_b * sysz) * ni;
        oy = oy + tanhqpow * (nstcv * cap_b * y_in + cap * y_in * ctcv) * ni;
        oz = oz + tanhqpow * (nstcv * cap_b * z_in + cap * z_in * ctcv) * ni;
    }
    vec3(ox, oy, oz)
}

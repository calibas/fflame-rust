/// Cosecant-squared power scale — computes a csc-style intermediate `csc =
/// csc_div / (cos(x/cos_div) · tan(x/tan_div))`, then a per-axis scale
/// factor `f = (csc² + π·pi_mult)^csc_pow + csc_add`, then emits `(x·f,
/// y·f·scaley)`. The csc/cos/tan composition produces sharp pole
/// structures.
///
/// # Authors
/// - Whittaker Courtney

// Csc Squared, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    csc_div: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Csc Div",
        hint: "Numerator of the csc fraction.",
    },
    cos_div: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Cos Div",
        hint: "Divisor on x in the cos term `cos(x/cos_div)`.",
    },
    tan_div: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Tan Div",
        hint: "Divisor on x in the tan term `tan(x/tan_div)`.",
    },
    csc_pow: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Csc Pow",
        hint: "Exponent on `csc² + π·pi_mult`.",
    },
    pi_mult: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Pi Mult",
        hint: "Coefficient on the π offset added to `csc²` before the pow.",
    },
    csc_add: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "Csc Add",
        hint: "Additive offset on the per-axis scale.",
    },
    scaley: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale Y",
        hint: "Y-axis scale multiplier (applied after the per-axis scale).",
    },
};

const INFO = #{ title: "Csc Squared", name: "rhai_csc_squared", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let csc_div = p.csc_div;
    let cos_div = p.cos_div;
    let tan_div = p.tan_div;
    let csc_pow = p.csc_pow;
    let pi_mult = p.pi_mult;
    let csc_add = p.csc_add;
    let scaley = p.scaley;
    let pi = 3.14159265358979;
    let safe_cos_div = (if abs(cos_div) < 1.0e-30 { 1.0e-30 } else { cos_div });
    let safe_tan_div = (if abs(tan_div) < 1.0e-30 { 1.0e-30 } else { tan_div });
    let cx = cos(z.re / safe_cos_div);
    let tx = tan(z.re / safe_tan_div);
    let safe_cx = (if abs(cx) < 1.0e-30 { 1.0e-30 } else { cx });
    let safe_tx = (if abs(tx) < 1.0e-30 { 1.0e-30 } else { tx });
    let csc = csc_div / safe_cx / safe_tx;
    let base = max(1.0e-30, csc * csc + pi * pi_mult);
    let fx_factor = pow(base, csc_pow) + csc_add;
    complex(z.re * fx_factor, z.im * fx_factor * scaley)
}

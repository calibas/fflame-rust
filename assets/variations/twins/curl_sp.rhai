/// Curl-power 3D variation with sign-preserving power — raises each axis to
/// `pow` (preserving sign), applies a `c1`/`c2`-tuned Möbius-style curl
/// warp with a configurable spread function (`sx`, `sy` exponents), then
/// scales the output by `w/c` where `c = (re² + im²)^(1/pow)` is the
/// squared output magnitude raised to the inverse power.
///
/// # Authors
/// - Xyrus02

// Curl SP, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    pow: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Power",
        hint: "Sign-preserving power exponent applied to each input axis.",
    },
    c1: #{
        value: -0.01, min: -10.0, max: 10.0, free: true,
        caption: "C1",
        hint: "Linear-term coefficient in the curl warp.",
    },
    c2: #{
        value: 0.03, min: -10.0, max: 10.0, free: true,
        caption: "C2",
        hint: "Quadratic-term coefficient in the curl warp.",
    },
    sx: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "SX",
        hint: "Spread function exponent for the X component.",
    },
    sy: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "SY",
        hint: "Spread function exponent for the Y component.",
    },
    dc: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "DC",
        hint: "Color drift amount. Preserved as a parameter for preset compatibility, but the actual color write was dropped during the port (see module header).",
    },
};

const INFO = #{ title: "Curl SP", name: "rhai_curl_sp", category: "full_3d", phase: "any" };

fn prepare(p) {
    let p_raw = p.pow;
    let p_safe = (if abs(p_raw) < 1.0e-30 { 1.0e-6 } else { p_raw });
    #{ power: p_safe, c2_x2: 2.0 * p.c2, power_inv: 1.0 / p_safe, d3: 0.1 * p.dc }
}

fn curl_sp_powq4c(x, e) {
    if (abs(e - 1.0) < 1.0e-30) {
        return x;
    }
    pow(max(1.0e-30, abs(x)), e) * (if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 })
}

fn curl_sp_spread(a, b) {
    sqrt(a * a + b * b) * (if a > 0.0 { 1.0 } else { -1.0 })
}

fn variation(z, p, ctx) {
    let c1 = p.c1;
    let c2 = p.c2;
    let sx = p.sx;
    let sy = p.sy;
    let power = p.power;
    let c2_x2 = p.c2_x2;
    let power_inv = p.power_inv;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let x = curl_sp_powq4c(z.re, power);
    let y = curl_sp_powq4c(z.im, power);
    let d = x * x - y * y;
    let re = curl_sp_spread(c1 * x + c2 * d, sx) + 1.0;
    let im = curl_sp_spread(c1 * y + c2_x2 * x * y, sy);
    let c = curl_sp_powq4c(re * re + im * im, power_inv);
    let safe_c = (if abs(c) < 1.0e-30 { 1.0e-30 } else { c });
    let r = w / safe_c;
    let fx = (x * re + y * im) * r;
    let fy = (y * re - x * im) * r;
    complex(fx * inv_w, fy * inv_w)
}

fn variation3(v, p, ctx) {
    let c1 = p.c1;
    let c2 = p.c2;
    let sx = p.sx;
    let sy = p.sy;
    let power = p.power;
    let c2_x2 = p.c2_x2;
    let power_inv = p.power_inv;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let x = curl_sp_powq4c(v.x, power);
    let y = curl_sp_powq4c(v.y, power);
    let z = curl_sp_powq4c(v.z, power);
    let d = x * x - y * y;
    let re = curl_sp_spread(c1 * x + c2 * d, sx) + 1.0;
    let im = curl_sp_spread(c1 * y + c2_x2 * x * y, sy);
    let c = curl_sp_powq4c(re * re + im * im, power_inv);
    let safe_c = (if abs(c) < 1.0e-30 { 1.0e-30 } else { c });
    let r = w / safe_c;
    let fx = (x * re + y * im) * r;
    let fy = (y * re - x * im) * r;
    let fz = z * w / safe_c;
    vec3(fx * inv_w, fy * inv_w, fz * inv_w)
}

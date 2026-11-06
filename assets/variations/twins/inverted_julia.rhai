/// Inverted Julia warp — 9-parameter Julia variant with adjustable inward
/// center. Computes `z = (x² + y²·y2_mult)^power + x2y2_add`, picks a
/// random hemisphere via `q = atan2(...)/2 + π·floor(2·rand)`, then emits
/// `cos(z·cos_mult) · (sin q, cos q · y_mult) / z / center`.
///
/// # Authors
/// - Whittaker Courtney

// Inverted Julia, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "Power",
        hint: "Exponent on the squared-radius base term `(x² + y²·y2_mult)`.",
    },
    y2_mult: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y² Mult",
        hint: "Multiplier on y² in the base term.",
    },
    a2x_mult: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A2x Mult",
        hint: "Multiplier on x in the angle term.",
    },
    a2y_mult: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A2y Mult",
        hint: "Multiplier on y in the angle term.",
    },
    a2y_add: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "A2y Add",
        hint: "Additive offset on y in the angle term.",
    },
    cos_mult: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Cos Mult",
        hint: "Frequency multiplier on z in the cosine modulator.",
    },
    y_mult: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Mult",
        hint: "Y output scaling.",
    },
    center: #{
        value: 3.14, min: -10.0, max: 10.0, free: true,
        caption: "Center",
        hint: "Divisor on the output radius. Higher = tighter pattern.",
    },
    x2y2_add: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "X²Y² Add",
        hint: "Additive offset on the base term (added after the pow).",
    },
};

const INFO = #{ title: "Inverted Julia", name: "rhai_inverted_julia", category: "advanced_2d", phase: "any" };

fn variation(w, p) {
    let power = p.power;
    let y2_mult = p.y2_mult;
    let a2x_mult = p.a2x_mult;
    let a2y_mult = p.a2y_mult;
    let a2y_add = p.a2y_add;
    let cos_mult = p.cos_mult;
    let y_mult = p.y_mult;
    let center = p.center;
    let x2y2_add = p.x2y2_add;
    let pi = 3.14159265358979;
    let xs = w.re * w.re;
    let ys = w.im * w.im;
    let base = xs + ys * y2_mult;
    let z = pow(max(base, 1.0e-30), power) + x2y2_add;
    let safe_z = (if abs(z) < 1.0e-30 { 1.0e-30 } else { z });
    let safe_center = (if abs(center) < 1.0e-30 { 1.0e-30 } else { center });
    let q = atan2(w.re * a2x_mult, w.im * a2y_mult + a2y_add) * 0.5 + pi * to_float(floor(2.0 * rand()));
    let cz = cos(z * cos_mult);
    let fx = cz * (sin(q) / safe_z / safe_center);
    let fy = cz * (cos(q) / safe_z / safe_center) * y_mult;
    complex(fx, fy)
}

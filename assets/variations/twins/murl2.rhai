/// Variant of `murl` — applies a more involved complex Möbius mapping:
/// power-transforms the input, adds 1, then power-transforms the result
/// back, then divides by squared magnitude. The `2/power` exponent in the
/// internal `vp = w · (c+1)^(2/power)` factor adjusts the output magnitude
/// per power value.
///
/// # Authors
/// - Zueuk

// Murl 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    c: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Möbius coefficient — controls the strength of the Möbius distortion.",
    },
    power: #{
        value: 3, min: -100, max: 100,
        caption: "Power",
        hint: "Power exponent. 0 falls back to a degenerate case with a high invp.",
    },
};

const INFO = #{ title: "Murl 2", name: "rhai_murl2", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p, ctx) {
    let c = p.c;
    let power = to_float(p.power);
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let p2 = power / 2.0;
    let invp = 0.0;
    let vp = 0.0;
    if (abs(power) > 1.0e-30) {
        invp = 1.0 / power;
        if (abs(c + 1.0) < 1.0e-30) {
            vp = 0.0;
        } else {
            let t1 = c + 1.0;
            vp = w * pow(abs(c + 1.0), 2.0 / power) * (if abs(2.0 / power - to_float(floor(2.0 / power + 0.5))) < 1.0e-6 && fract(1.0 / power + 0.5) > 0.99 { (if t1 > 0.0 { 1.0 } else if t1 < 0.0 { -1.0 } else { 0.0 }) } else { 1.0 });
        }
    } else {
        invp = 1.0e11;
        vp = w * pow(max(1.0e-30, c + 1.0), 4.0);
    }
    let a = atan2(z.im, z.re) * power;
    let r = c * pow(max(1.0e-30, z.re * z.re + z.im * z.im), p2);
    let re = r * cos(a) + 1.0;
    let im = r * sin(a);
    r = pow(max(1.0e-30, re * re + im * im), invp);
    a = atan2(im, re) * 2.0 * invp;
    re = r * cos(a);
    im = r * sin(a);
    let safe_r2 = max(1.0e-30, r * r);
    let rl = vp / safe_r2;
    let fx = rl * (z.re * re + z.im * im);
    let fy = rl * (z.im * re - z.re * im);
    complex(fx * inv_w, fy * inv_w)
}

fn variation3(v, p, ctx) {
    let c = p.c;
    let power = to_float(p.power);
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let p2 = power / 2.0;
    let invp = 0.0;
    let vp = 0.0;
    if (abs(power) > 1.0e-30) {
        invp = 1.0 / power;
        if (abs(c + 1.0) < 1.0e-30) {
            vp = 0.0;
        } else {
            vp = w * pow(abs(c + 1.0), 2.0 / power);
        }
    } else {
        invp = 1.0e11;
        vp = w * pow(max(1.0e-30, c + 1.0), 4.0);
    }
    let a = atan2(v.y, v.x) * power;
    let r = c * pow(max(1.0e-30, v.x * v.x + v.y * v.y), p2);
    let re = r * cos(a) + 1.0;
    let im = r * sin(a);
    r = pow(max(1.0e-30, re * re + im * im), invp);
    a = atan2(im, re) * 2.0 * invp;
    re = r * cos(a);
    im = r * sin(a);
    let safe_r2 = max(1.0e-30, r * r);
    let rl = vp / safe_r2;
    let fx = rl * (v.x * re + v.y * im);
    let fy = rl * (v.y * re - v.x * im);
    vec3(fx * inv_w, fy * inv_w, v.z)
}

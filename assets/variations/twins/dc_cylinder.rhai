/// Direct-color cylinder warp — wraps the input around a cylinder via
/// `sin(x + rr·sin(a))` on X with a Gaussian-approximated Y blur (`rr =
/// blur · (sum of 4 uniforms − 2)`). 3D mode adds a `cos(x + rr·cos(a))` Z
/// component. Color is written from a rotated projection of the weighted
/// output position (`TC = fmod(|0.5 · (ldcs · proj + 1)|, 1)`), matching
/// the cpp/Java original. Visible color requires the transform's Direct
/// Color slider > 0.
///
/// # Authors
/// - FracFx

// DC Cylinder, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    offset: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Offset",
        hint: "Color-projection offset. Init precomputes `offset · π` for parity (unused) and the raw `offset` is added to the projection.",
    },
    angle: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Angle",
        hint: "Color-projection rotation angle (radians). Init precomputes `cos(angle)` and `sin(angle)`.",
    },
    scale: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Color-projection scale. Init precomputes `1/scale` (with zero-guard).",
    },
    x: #{
        value: 0.125, min: -10.0, max: 10.0, free: true,
        caption: "X",
        hint: "X-axis output scale (multiplies the `sin(x + rr·sin(a))` term).",
    },
    y: #{
        value: 0.125, min: -10.0, max: 10.0, free: true,
        caption: "Y",
        hint: "Y-axis output scale (multiplies the input Y in the output).",
    },
    blur: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Blur",
        hint: "Gaussian blur amplitude — multiplies the sum-of-4-uniforms approximation.",
    },
};

const INFO = #{ title: "DC Cylinder", name: "rhai_dc_cylinder", category: "full_3d", phase: "any" };

fn prepare(p) {
    let pi = 3.14159265358979;
    let scale = p.scale;
    let safe_scale = (if abs(scale) < 1.0e-30 { 1.0e-5 } else { scale });
    #{ ldcs: 1.0 / safe_scale, d1: p.offset * pi, cos_a: cos(p.angle), sin_a: sin(p.angle) }
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let offset = p.offset;
    let xp = p.x;
    let yp = p.y;
    let blur = p.blur;
    let ldcs = p.ldcs;
    let cos_a = p.cos_a;
    let sin_a = p.sin_a;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let two_pi = 6.28318530717959;
    let a = rand() * two_pi;
    let sr = sin(a);
    let r0 = rand();
    let r1 = rand();
    let r2 = rand();
    let r3 = rand();
    let rr = blur * (r0 + r1 + r2 + r3 - 2.0);
    let nx = sin(z.re + rr * sr) * xp;
    let ny = (rr + z.im * yp) * inv_w;
    let fpx = w * nx;
    let fpy = w * ny;
    let proj = cos_a * fpx + sin_a * fpy + offset;
    let raw = abs(0.5 * (ldcs * proj + 1.0));
    color = raw - to_float(floor(raw));
    #{ z: complex(nx, ny), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let offset = p.offset;
    let xp = p.x;
    let yp = p.y;
    let blur = p.blur;
    let ldcs = p.ldcs;
    let cos_a = p.cos_a;
    let sin_a = p.sin_a;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let two_pi = 6.28318530717959;
    let a = rand() * two_pi;
    let sr = sin(a);
    let cr = cos(a);
    let r0 = rand();
    let r1 = rand();
    let r2 = rand();
    let r3 = rand();
    let rr = blur * (r0 + r1 + r2 + r3 - 2.0);
    let nx = sin(v.x + rr * sr) * xp;
    let ny = (rr + v.y * yp) * inv_w;
    let nz = cos(v.x + rr * cr);
    let fpx = w * nx;
    let fpy = w * ny;
    let proj = cos_a * fpx + sin_a * fpy + offset;
    let raw = abs(0.5 * (ldcs * proj + 1.0));
    color = raw - to_float(floor(raw));
    #{ z: vec3(nx, ny, nz), color: color }
}

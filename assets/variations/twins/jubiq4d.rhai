/// Honest 4D jubiq — the julian2/Möbius chimera with the k-component
/// classic jubiQ drops carried through.
///
/// # Authors
/// - Brad Stefanov
/// - Xyrus02
/// - zephyrtronium
/// - Fractals for All
/// - Claude Fable 5

// Jubiq 4D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 1, min: -50, max: 50,
        caption: "Power",
        hint: "Julia angular branch count (integer). 0 collapses the variation to the origin. Otherwise: picks one of |power| equally-spaced angles per iteration and divides the polar angle by power.",
    },
    dist: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Distance",
        hint: "Radial exponent multiplier. Combines with power to set the radial power as 0.5·dist/power.",
    },
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Output XY affine wrapper, component A (X-to-X scale).",
    },
    b: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Output XY affine wrapper, component B (Y-to-X scale).",
    },
    c: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Output XY affine wrapper, component C (X-to-Y scale).",
    },
    d: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "Output XY affine wrapper, component D (Y-to-Y scale).",
    },
    e: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "E",
        hint: "Output XY affine wrapper, component E (X translation).",
    },
    f: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "F",
        hint: "Output XY affine wrapper, component F (Y translation).",
    },
    qat: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "QA t",
        hint: "Möbius numerator quaternion A, scalar (t) component. Unlike classic jubiQ the input k rides the per-thread w register and the output k feeds back.",
    },
    qax: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "QA x",
        hint: "Möbius numerator quaternion A, i component.",
    },
    qay: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "QA y",
        hint: "Möbius numerator quaternion A, j component.",
    },
    qaz: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "QA z",
        hint: "Möbius numerator quaternion A, k component.",
    },
    qbt: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "QB t",
        hint: "Möbius numerator quaternion B, scalar (t) component.",
    },
    qbx: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "QB x",
        hint: "Möbius numerator quaternion B, i component.",
    },
    qby: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "QB y",
        hint: "Möbius numerator quaternion B, j component.",
    },
    qbz: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "QB z",
        hint: "Möbius numerator quaternion B, k component.",
    },
    qct: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "QC t",
        hint: "Möbius denominator quaternion C, scalar (t) component.",
    },
    qcx: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "QC x",
        hint: "Möbius denominator quaternion C, i component.",
    },
    qcy: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "QC y",
        hint: "Möbius denominator quaternion C, j component.",
    },
    qcz: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "QC z",
        hint: "Möbius denominator quaternion C, k component.",
    },
    qdt: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "QD t",
        hint: "Möbius denominator quaternion D, scalar (t) component.",
    },
    qdx: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "QD x",
        hint: "Möbius denominator quaternion D, i component.",
    },
    qdy: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "QD y",
        hint: "Möbius denominator quaternion D, j component.",
    },
    qdz: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "QD z",
        hint: "Möbius denominator quaternion D, k component.",
    },
    k_input: #{
        value: 1.0, min: 0.0, max: 1.0,
        caption: "K Input",
        hint: "How much of the per-thread w register feeds the quaternion's k input. 1 = the honest 4D Möbius stage; 0 = xyz output reproduces classic jubiQ exactly (k output still writes to w). Animate for a projection→group-action morph.",
    },
};

const INFO = #{ title: "Jubiq 4D", name: "rhai_jubiq4d", category: "full_3d", phase: "any" };

fn prepare(p) {
    let power = to_float(p.power);
    let dist = p.dist;
    let safe_power = (if abs(power) < 1.0e-30 { 1.0e-30 } else { power });
    #{ abs_n: abs(power), cn: dist / safe_power * 0.5 }
}

fn variation(z, p) {
    let power = to_int(to_float(p.power));
    if (power == 0) {
        return complex(0.0, 0.0);
    }
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d_p = p.d;
    let ep = p.e;
    let fp = p.f;
    let qat = p.qat;
    let qax = p.qax;
    let qay = p.qay;
    let qaz = p.qaz;
    let qbt = p.qbt;
    let qbx = p.qbx;
    let qby = p.qby;
    let qbz = p.qbz;
    let qct = p.qct;
    let qcx = p.qcx;
    let qcy = p.qcy;
    let qcz = p.qcz;
    let qdt = p.qdt;
    let qdx = p.qdx;
    let qdy = p.qdy;
    let qdz = p.qdz;
    let abs_n = p.abs_n;
    let cn = p.cn;
    let two_pi = 6.28318530717959;
    let t2 = z.re;
    let x2 = z.im;
    let nt = qat * t2 - qax * x2 + qbt;
    let nx = qat * x2 + qax * t2 + qbx;
    let ny = qay * t2 + qaz * x2 + qby;
    let nz = qaz * t2 - qay * x2 + qbz;
    let dt = qct * t2 - qcx * x2 + qdt;
    let dx = qct * x2 + qcx * t2 + qdx;
    let dy = qcy * t2 + qcz * x2 + qdy;
    let dz = qcz * t2 - qcy * x2 + qdz;
    let denom = dt * dt + dx * dx + dy * dy + dz * dz;
    let safe_denom = (if denom < 1.0e-30 { 1.0e-30 } else { denom });
    let ni = 1.0 / safe_denom;
    let x = a * z.re + b * z.im + (nt * dt + nx * dx + ny * dy + nz * dz) * ni + ep;
    let y = c * z.re + d_p * z.im + (nx * dt - nt * dx - ny * dz + nz * dy) * ni + fp;
    let abs_ni = max(to_int(abs_n), 1);
    let pick = to_int(rand() * to_float(abs_ni));
    let safe_pow = (if abs(to_float(power)) < 1.0e-30 { 1.0e-30 } else { to_float(power) });
    let angle = (atan2(y, x) + two_pi * to_float(pick)) / safe_pow;
    let r = pow(max(1.0e-30, x * x + y * y), cn);
    complex(r * cos(angle), r * sin(angle))
}

fn variation3(v, p, ctx) {
    let w_out = 0.0;
    let power = to_int(to_float(p.power));
    if (power == 0) {
        w_out = ctx.w;
        return #{ z: vec3(0.0, 0.0, v.z), w: w_out };
    }
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d_p = p.d;
    let ep = p.e;
    let fp = p.f;
    let qat = p.qat;
    let qax = p.qax;
    let qay = p.qay;
    let qaz = p.qaz;
    let qbt = p.qbt;
    let qbx = p.qbx;
    let qby = p.qby;
    let qbz = p.qbz;
    let qct = p.qct;
    let qcx = p.qcx;
    let qcy = p.qcy;
    let qcz = p.qcz;
    let qdt = p.qdt;
    let qdx = p.qdx;
    let qdy = p.qdy;
    let qdz = p.qdz;
    let abs_n = p.abs_n;
    let cn = p.cn;
    let two_pi = 6.28318530717959;
    let k_in = p.k_input;
    let t2 = v.x;
    let x2 = v.y;
    let y2 = v.z;
    let z2 = ctx.w * k_in;
    let nt = qat * t2 - qax * x2 - qay * y2 - qaz * z2 + qbt;
    let nx = qat * x2 + qax * t2 + qay * z2 - qaz * y2 + qbx;
    let ny = qat * y2 - qax * z2 + qay * t2 + qaz * x2 + qby;
    let nz = qat * z2 + qax * y2 - qay * x2 + qaz * t2 + qbz;
    let dt = qct * t2 - qcx * x2 - qcy * y2 - qcz * z2 + qdt;
    let dx = qct * x2 + qcx * t2 + qcy * z2 - qcz * y2 + qdx;
    let dy = qct * y2 - qcx * z2 + qcy * t2 + qcz * x2 + qdy;
    let dz = qct * z2 + qcx * y2 - qcy * x2 + qcz * t2 + qdz;
    let denom = dt * dt + dx * dx + dy * dy + dz * dz;
    let safe_denom = (if denom < 1.0e-30 { 1.0e-30 } else { denom });
    let ni = 1.0 / safe_denom;
    let x = a * v.x + b * v.y + (nt * dt + nx * dx + ny * dy + nz * dz) * ni + ep;
    let y = c * v.x + d_p * v.y + (nx * dt - nt * dx - ny * dz + nz * dy) * ni + fp;
    let abs_ni = max(to_int(abs_n), 1);
    let pick = to_int(rand() * to_float(abs_ni));
    let safe_pow = (if abs(to_float(power)) < 1.0e-30 { 1.0e-30 } else { to_float(power) });
    let angle = (atan2(y, x) + two_pi * to_float(pick)) / safe_pow;
    let r = pow(max(1.0e-30, x * x + y * y), cn);
    let z_half = v.z * 0.5;
    let r2d = v.x * v.x + v.y * v.y;
    let r3d = sqrt(r2d + z_half * z_half);
    let safe_r3d = max(1.0e-30, r3d);
    let r2_inv = 1.0 / (sqrt(safe_r3d) * safe_r3d);
    let nz_term = (ny * dt - nt * dy - nz * dx + nx * dz) * ni;
    w_out = (nz * dt - nt * dz + ny * dx - nx * dy) * ni;
    #{ z: vec3(r * cos(angle), r * sin(angle), nz_term + r2_inv * z_half), w: w_out }
}

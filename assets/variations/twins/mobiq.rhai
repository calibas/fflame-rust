/// Quaternion Möbius — same `(Az + B)/(Cz + D)` form as Möbius but with
/// quaternion-valued A, B, C, D and input. Treats `(x, y, z)` as a
/// quaternion with no k-component, producing true 3D output.
///
/// # Authors
/// - zephyrtronium

// Mobiq, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    qat: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "qa.t",
        hint: "T (scalar) component of quaternion A (numerator multiplier).",
    },
    qax: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qa.x",
        hint: "X (i) component of quaternion A (numerator multiplier).",
    },
    qay: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qa.y",
        hint: "Y (j) component of quaternion A (numerator multiplier).",
    },
    qaz: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qa.z",
        hint: "Z (k) component of quaternion A (numerator multiplier).",
    },
    qbt: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qb.t",
        hint: "T (scalar) component of quaternion B (numerator offset).",
    },
    qbx: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qb.x",
        hint: "X (i) component of quaternion B (numerator offset).",
    },
    qby: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qb.y",
        hint: "Y (j) component of quaternion B (numerator offset).",
    },
    qbz: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qb.z",
        hint: "Z (k) component of quaternion B (numerator offset).",
    },
    qct: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qc.t",
        hint: "T (scalar) component of quaternion C (denominator multiplier).",
    },
    qcx: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qc.x",
        hint: "X (i) component of quaternion C (denominator multiplier).",
    },
    qcy: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qc.y",
        hint: "Y (j) component of quaternion C (denominator multiplier).",
    },
    qcz: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qc.z",
        hint: "Z (k) component of quaternion C (denominator multiplier).",
    },
    qdt: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "qd.t",
        hint: "T (scalar) component of quaternion D (denominator offset).",
    },
    qdx: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qd.x",
        hint: "X (i) component of quaternion D (denominator offset).",
    },
    qdy: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qd.y",
        hint: "Y (j) component of quaternion D (denominator offset).",
    },
    qdz: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qd.z",
        hint: "Z (k) component of quaternion D (denominator offset).",
    },
};

const INFO = #{ title: "Mobiq", name: "rhai_mobiq", category: "full_3d", phase: "any" };

fn variation(z, p) {
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
    let tx = z.re;
    let xx = z.im;
    let yy = 0.0;
    let nt = qat * tx - qax * xx - qay * yy + qbt;
    let nx = qat * xx + qax * tx - qaz * yy + qbx;
    let ny = qat * yy + qay * tx + qaz * xx + qby;
    let nz = qaz * tx + qax * yy - qay * xx + qbz;
    let dt = qct * tx - qcx * xx - qcy * yy + qdt;
    let dx = qct * xx + qcx * tx - qcz * yy + qdx;
    let dy = qct * yy + qcy * tx + qcz * xx + qdy;
    let dz = qcz * tx + qcx * yy - qcy * xx + qdz;
    let denom = max(1.0e-30, dt * dt + dx * dx + dy * dy + dz * dz);
    let ni = 1.0 / denom;
    complex((nt * dt + nx * dx + ny * dy + nz * dz) * ni, (nx * dt - nt * dx - ny * dz + nz * dy) * ni)
}

fn variation3(v, p) {
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
    let tx = v.x;
    let xx = v.y;
    let yy = v.z;
    let nt = qat * tx - qax * xx - qay * yy + qbt;
    let nx = qat * xx + qax * tx - qaz * yy + qbx;
    let ny = qat * yy + qay * tx + qaz * xx + qby;
    let nz = qaz * tx + qax * yy - qay * xx + qbz;
    let dt = qct * tx - qcx * xx - qcy * yy + qdt;
    let dx = qct * xx + qcx * tx - qcz * yy + qdx;
    let dy = qct * yy + qcy * tx + qcz * xx + qdy;
    let dz = qcz * tx + qcx * yy - qcy * xx + qdz;
    let denom = max(1.0e-30, dt * dt + dx * dx + dy * dy + dz * dz);
    let ni = 1.0 / denom;
    vec3((nt * dt + nx * dx + ny * dy + nz * dz) * ni, (nx * dt - nt * dx - ny * dz + nz * dy) * ni, (ny * dt - nt * dy - nz * dx + nx * dz) * ni)
}

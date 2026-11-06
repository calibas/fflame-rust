/// Honest 4D quaternionic Möbius (GL(2,ℍ) on S⁴) — carries the
/// k-component classic mobiq drops.
///
/// # Authors
/// - zephyrtronium
/// - Brad Stefanov
/// - Fractals for All
/// - Claude Fable 5

// Mobiq 4D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    qat: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "qa.t",
        hint: "Scalar (t) component of quaternion A (numerator multiplier). Conventions match mobiq: flame x = quaternion t, y = i, z = j, and the per-thread w register = k (the component mobiq drops).",
    },
    qax: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qa.x",
        hint: "i component of quaternion A.",
    },
    qay: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qa.y",
        hint: "j component of quaternion A.",
    },
    qaz: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qa.z",
        hint: "k component of quaternion A.",
    },
    qbt: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qb.t",
        hint: "Scalar (t) component of quaternion B (numerator offset).",
    },
    qbx: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qb.x",
        hint: "i component of quaternion B.",
    },
    qby: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qb.y",
        hint: "j component of quaternion B.",
    },
    qbz: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qb.z",
        hint: "k component of quaternion B.",
    },
    qct: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qc.t",
        hint: "Scalar (t) component of quaternion C (denominator multiplier).",
    },
    qcx: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qc.x",
        hint: "i component of quaternion C.",
    },
    qcy: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qc.y",
        hint: "j component of quaternion C.",
    },
    qcz: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qc.z",
        hint: "k component of quaternion C.",
    },
    qdt: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "qd.t",
        hint: "Scalar (t) component of quaternion D (denominator offset).",
    },
    qdx: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qd.x",
        hint: "i component of quaternion D.",
    },
    qdy: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qd.y",
        hint: "j component of quaternion D.",
    },
    qdz: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "qd.z",
        hint: "k component of quaternion D.",
    },
    k_input: #{
        value: 1.0, min: 0.0, max: 1.0,
        caption: "K Input",
        hint: "How much of the per-thread w register feeds the quaternion's k input. 1 = the honest 4D group action; 0 = the xyz output reproduces classic mobiq EXACTLY (the k output still writes to w). Animating 0→1 morphs continuously from the classic projection's fuzz to the group action's crisp limit sets.",
    },
};

const INFO = #{ title: "Mobiq 4D", name: "rhai_mobiq4d", category: "full_3d", phase: "any" };

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
    let nt = qat * tx - qax * xx + qbt;
    let nx = qat * xx + qax * tx + qbx;
    let ny = qat * 0.0 + qay * tx + qaz * xx + qby;
    let nz = qaz * tx - qay * xx + qbz;
    let dt = qct * tx - qcx * xx + qdt;
    let dx = qct * xx + qcx * tx + qdx;
    let dy = qcy * tx + qcz * xx + qdy;
    let dz = qcz * tx - qcy * xx + qdz;
    let denom = max(1.0e-30, dt * dt + dx * dx + dy * dy + dz * dz);
    let ni = 1.0 / denom;
    complex((nt * dt + nx * dx + ny * dy + nz * dz) * ni, (nx * dt - nt * dx - ny * dz + nz * dy) * ni)
}

fn variation3(v, p, ctx) {
    let w_out = 0.0;
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
    let k_in = p.k_input;
    let zz = ctx.w * k_in;
    let nt = qat * tx - qax * xx - qay * yy - qaz * zz + qbt;
    let nx = qat * xx + qax * tx + qay * zz - qaz * yy + qbx;
    let ny = qat * yy - qax * zz + qay * tx + qaz * xx + qby;
    let nz = qat * zz + qax * yy - qay * xx + qaz * tx + qbz;
    let dt = qct * tx - qcx * xx - qcy * yy - qcz * zz + qdt;
    let dx = qct * xx + qcx * tx + qcy * zz - qcz * yy + qdx;
    let dy = qct * yy - qcx * zz + qcy * tx + qcz * xx + qdy;
    let dz = qct * zz + qcx * yy - qcy * xx + qcz * tx + qdz;
    let denom = max(1.0e-30, dt * dt + dx * dx + dy * dy + dz * dz);
    let ni = 1.0 / denom;
    w_out = (nz * dt - nt * dz + ny * dx - nx * dy) * ni;
    #{ z: vec3((nt * dt + nx * dx + ny * dy + nz * dz) * ni, (nx * dt - nt * dx - ny * dz + nz * dy) * ni, (ny * dt - nt * dy - nz * dx + nx * dz) * ni), w: w_out }
}

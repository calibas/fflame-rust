/// Quaternion cotangent — 3D extension of Cot (cos/sin).
///
/// # Authors
/// - zephyrtronium

// Cotq, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Cotq", name: "rhai_cotq", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let v = max(1.0e-20, abs(z.im));
    let sysz = z.im * z.im;
    let ni = 1.0 / max(1.0e-20, z.re * z.re + sysz);
    let s = sin(z.re);
    let c = cos(z.re);
    let sh = sinh(v);
    let ch = cosh(v);
    let cc = c * sh / v;
    let bb = -s * sh / v;
    let stcv = s * ch;
    let ctcv = c * ch;
    let rx = (stcv * ctcv + cc * bb * sysz) * ni;
    let ry = -(-stcv * bb * z.im + cc * z.im * ctcv) * ni;
    complex(rx, ry)
}

fn variation3(q, p) {
    let v = max(1.0e-20, sqrt(q.y * q.y + q.z * q.z));
    let sysz = q.y * q.y + q.z * q.z;
    let ni = 1.0 / max(1.0e-20, q.x * q.x + sysz);
    let s = sin(q.x);
    let c = cos(q.x);
    let sh = sinh(v);
    let ch = cosh(v);
    let cc = c * sh / v;
    let bb = -s * sh / v;
    let stcv = s * ch;
    let ctcv = c * ch;
    let rx = (stcv * ctcv + cc * bb * sysz) * ni;
    let ry = -(-stcv * bb * q.y + cc * q.y * ctcv) * ni;
    let rz = -(-stcv * bb * q.z + cc * q.z * ctcv) * ni;
    vec3(rx, ry, rz)
}

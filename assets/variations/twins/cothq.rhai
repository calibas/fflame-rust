/// Quaternion hyperbolic cotangent — 3D extension of Coth.
///
/// # Authors
/// - zephyrtronium

// Cothq, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Cothq", name: "rhai_cothq", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let v = max(1.0e-20, abs(z.im));
    let sysz = z.im * z.im;
    let ni = 1.0 / max(1.0e-20, z.re * z.re + sysz);
    let s = sin(v);
    let c = cos(v);
    let sh = sinh(z.re);
    let ch = cosh(z.re);
    let cc = ch * s / v;
    let bb = sh * s / v;
    let stcv = sh * c;
    let ctcv = ch * c;
    let rx = (stcv * ctcv + cc * bb * sysz) * ni;
    let ry = (-stcv * bb * z.im + cc * z.im * ctcv) * ni;
    complex(rx, ry)
}

fn variation3(q, p) {
    let v = max(1.0e-20, sqrt(q.y * q.y + q.z * q.z));
    let sysz = q.y * q.y + q.z * q.z;
    let ni = 1.0 / max(1.0e-20, q.x * q.x + sysz);
    let s = sin(v);
    let c = cos(v);
    let sh = sinh(q.x);
    let ch = cosh(q.x);
    let cc = ch * s / v;
    let bb = sh * s / v;
    let stcv = sh * c;
    let ctcv = ch * c;
    let rx = (stcv * ctcv + cc * bb * sysz) * ni;
    let ry = (-stcv * bb * q.y + cc * q.y * ctcv) * ni;
    let rz = (-stcv * bb * q.z + cc * q.z * ctcv) * ni;
    vec3(rx, ry, rz)
}

// Tanq, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Tanq", name: "rhai_tanq" };

fn variation(z, p) {
    let v = max(abs(z.im), 1.0e-20);
    let sysz = z.im * z.im;
    let ni = 1.0 / max(z.re * z.re + sysz, 1.0e-20);
    let s = sin(z.re);
    let c = cos(z.re);
    let sh = sinh(v);
    let ch = cosh(v);
    let cc = c * sh / v;
    let bb = -s * sh / v;
    let stcv = s * ch;
    let ctcv = c * ch;
    let rx = (stcv * ctcv + cc * bb * sysz) * ni;
    let ry = (-stcv * bb * z.im + cc * z.im * ctcv) * ni;
    complex(rx, ry)
}

fn variation3(q, p) {
    let v = max(sqrt(q.y * q.y + q.z * q.z), 1.0e-20);
    let sysz = q.y * q.y + q.z * q.z;
    let ni = 1.0 / max(q.x * q.x + sysz, 1.0e-20);
    let s = sin(q.x);
    let c = cos(q.x);
    let sh = sinh(v);
    let ch = cosh(v);
    let cc = c * sh / v;
    let bb = -s * sh / v;
    let stcv = s * ch;
    let ctcv = c * ch;
    let rx = (stcv * ctcv + cc * bb * sysz) * ni;
    let ry = (-stcv * bb * q.y + cc * q.y * ctcv) * ni;
    let rz = (-stcv * bb * q.z + cc * q.z * ctcv) * ni;
    vec3(rx, ry, rz)
}

// Coshq, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Coshq", name: "rhai_coshq" };

fn variation(z, p) {
    let v = max(abs(z.im), 1.0e-20);
    let s = sin(v);
    let c = cos(v);
    let sh = sinh(z.re);
    let ch = cosh(z.re);
    let coef = sh * s / v;
    complex(ch * c, coef * z.im)
}

fn variation3(q, p) {
    let v = max(sqrt(q.y * q.y + q.z * q.z), 1.0e-20);
    let s = sin(v);
    let c = cos(v);
    let sh = sinh(q.x);
    let ch = cosh(q.x);
    let coef = sh * s / v;
    vec3(ch * c, coef * q.y, coef * q.z)
}

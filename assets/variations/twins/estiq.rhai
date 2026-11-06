// Estiq, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Estiq", name: "rhai_estiq" };

fn variation(z, p) {
    let e = exp(z.re);
    let abs_v = max(abs(z.im), 1.0e-30);
    let s = sin(abs_v);
    let c = cos(abs_v);
    let a = e * s / abs_v;
    complex(e * c, a * z.im)
}

fn variation3(v, p) {
    let e = exp(v.x);
    let abs_v = max(sqrt(v.y * v.y + v.z * v.z), 1.0e-30);
    let s = sin(abs_v);
    let c = cos(abs_v);
    let a = e * s / abs_v;
    vec3(e * c, a * v.y, a * v.z)
}

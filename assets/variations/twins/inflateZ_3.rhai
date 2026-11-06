// Inflate Z 3, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Inflate Z 3", name: "rhai_inflatez_3" };

fn variation(z, p) {
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    let pi = 3.14159265358979;
    let ang = atan2(v.y, v.x);
    let val1 = 0.2 * (pi - ang) * cos(3.0 * ang + (v.y - v.x));
    vec3(0.0, 0.0, val1)
}

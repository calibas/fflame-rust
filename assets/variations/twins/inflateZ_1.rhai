// Inflate Z 1, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Inflate Z 1", name: "rhai_inflatez_1" };

fn variation(z, p) {
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    let ang = atan2(v.y, v.x);
    let val1 = v.y * 2.0;
    vec3(0.0, 0.0, sin(ang) - val1)
}

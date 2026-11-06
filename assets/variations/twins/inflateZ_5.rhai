// Inflate Z 5, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Inflate Z 5", name: "rhai_inflatez_5" };

fn variation(z, p) {
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    let pi_2 = 1.5707963267948966;
    let ang1 = atan2(v.y, v.x);
    let val1 = cos(pi_2 - ang1) * 0.5;
    vec3(0.0, 0.0, val1)
}

// Bubble, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Bubble", name: "rhai_bubble" };

fn variation(z, p) {
    let r2 = z.norm_sqr();
    let r = r2 / 4.0 + 1.0;
    let scale = 1.0 / r;
    complex(scale * z.re, scale * z.im)
}

fn variation3(v, p) {
    let r2_xy = complex(v.x, v.y).norm_sqr();
    let r = r2_xy / 4.0 + 1.0;
    let scale = 1.0 / r;
    let new_z = 2.0 / r - 1.0;
    vec3(scale * v.x, scale * v.y, new_z)
}

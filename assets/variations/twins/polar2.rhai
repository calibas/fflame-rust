// Polar2, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Polar2", name: "rhai_polar2" };

fn variation(z, p) {
    let PI = 3.14159265359;
    let r2 = z.norm_sqr();
    let new_x = atan2(z.re, z.im) / PI();
    let new_y = 0.5 * ln(r2) / PI();
    complex(new_x, new_y)
}

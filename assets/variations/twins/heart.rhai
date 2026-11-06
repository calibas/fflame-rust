// Heart, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Heart", name: "rhai_heart" };

fn variation(z, p) {
    let r = z.abs();
    let theta = atan2(z.re, z.im);
    let r_theta = r * theta;
    complex(r * sin(r_theta), -r * cos(r_theta))
}

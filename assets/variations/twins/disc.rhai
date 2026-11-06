// Disc, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Disc", name: "rhai_disc" };

fn variation(z, p) {
    let theta = atan2(z.re, z.im);
    let r = z.abs();
    let theta_pi = theta / 3.14159265359;
    let pi_r = 3.14159265359 * r;
    complex(theta_pi * sin(pi_r), theta_pi * cos(pi_r))
}

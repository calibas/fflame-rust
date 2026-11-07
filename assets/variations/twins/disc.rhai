/// Wraps the plane onto a disc, with the angle controlling the radial
/// position and the radius controlling the ripples. Creates a hypnotic
/// sunburst pattern.
///
/// # Authors
/// - Scott Draves

// Disc, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Disc", name: "rhai_disc", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let theta = atan2(z.re, z.im);
    let r = z.abs();
    let theta_pi = theta / 3.14159265359;
    let pi_r = 3.14159265359 * r;
    complex(theta_pi * sin(pi_r), theta_pi * cos(pi_r))
}

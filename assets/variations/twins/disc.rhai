/// Wraps the plane onto a disc, with the angle controlling the radial
/// position and the radius controlling the ripples. Creates a hypnotic
/// sunburst pattern.
///
/// # Authors
/// - Scott Draves

// Disc, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Disc", name: "rhai_disc", category: "advanced_2d", phase: "any", inverse_branches: 12 };

fn variation(z, p) {
    let theta = atan2(z.re, z.im);
    let r = z.abs();
    let theta_pi = theta / 3.14159265359;
    let pi_r = 3.14159265359 * r;
    complex(theta_pi * sin(pi_r), theta_pi * cos(pi_r))
}

fn inverse(z, p, branch) {
    let rho = z.abs();
    if rho > 1.0 {
        return no_preimage();
    }
    let r = atan2(z.re, z.im) / 3.14159265358979 + to_float(branch);
    if r < 0.0 {
        return no_preimage();
    }
    let theta = if (branch & 1) == 0 { 3.14159265358979 * rho } else { -3.14159265358979 * rho };
    complex(r * sin(theta), r * cos(theta))
}

fn image_gap(z, p) {
    z.abs() - 1.0
}

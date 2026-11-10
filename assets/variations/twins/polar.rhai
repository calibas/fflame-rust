/// Switches to polar coordinates: the X output becomes the angle (scaled to
/// [-1, 1]), the Y output becomes the radius minus 1. Unwraps circular
/// patterns into horizontal stripes.
///
/// # Authors
/// - Scott Draves

// Polar, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Polar", name: "rhai_polar", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let theta = atan2(z.re, z.im);
    let r = z.abs();
    complex(theta / 3.14159265359, r - 1.0)
}

fn inverse(z, p, branch) {
    if abs(z.re) > 1.0 || z.im < -1.0 {
        return no_preimage();
    }
    let th = 3.14159265359 * z.re;
    let r = z.im + 1.0;
    complex(r * sin(th), r * cos(th))
}

fn image_gap(z, p) {
    max(abs(z.re) - 1.0, -1.0 - z.im)
}

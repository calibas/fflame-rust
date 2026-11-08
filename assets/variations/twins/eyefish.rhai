/// An anti-fisheye that pulls everything toward the unit circle. Inverse of
/// the classic fisheye warp.
///
/// # Authors
/// - Scott Draves

// Eyefish, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Eyefish", name: "rhai_eyefish", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r_xy = z.abs() + 1.0;
    let scale = 2.0 / r_xy;
    complex(scale * z.re, scale * z.im)
}

fn inverse(z, p, branch) {
    let s = z.abs();
    if s >= 2.0 {
        return no_preimage();
    }
    z / (2.0 - s)
}

fn image_gap(z, p) {
    z.abs() - 2.0
}

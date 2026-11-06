/// Twists radial waves so the pattern looks like a knotted handkerchief -
/// concentric folds that ripple in toward the center.
///
/// # Authors
/// - Scott Draves

// Handkerchief, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Handkerchief", name: "rhai_handkerchief", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r = z.abs();
    let theta = atan2(z.im, z.re);
    complex(r * sin(theta + r), r * cos(theta - r))
}

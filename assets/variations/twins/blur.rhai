/// Replaces the input with a uniformly random point inside the unit disc -
/// the position is ignored. Useful for adding a soft glow or particle haze.
///
/// # Authors
/// - Scott Draves

// Blur, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Blur", name: "rhai_blur", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let theta = rand() * 6.28318530718;
    let r = rand();
    complex(r * cos(theta), r * sin(theta))
}

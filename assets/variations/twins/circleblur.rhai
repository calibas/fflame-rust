/// Pure random sample inside the unit disc — replaces the input with a
/// uniformly random point in the unit circle (area-uniform via `sqrt(rand)`
/// radius).
///
/// # Authors
/// - Zy0rg

// Circle Blur, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Circle Blur", name: "rhai_circleblur", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let rad = sqrt(rand());
    let ang = rand() * 6.28318530717959;
    complex(cos(ang) * rad, sin(ang) * rad)
}

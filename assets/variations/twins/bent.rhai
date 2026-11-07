/// Doubles the X coordinate when X is negative, halves the Y coordinate
/// when Y is negative. A simple asymmetric pinch.
///
/// # Authors
/// - Scott Draves

// Bent, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Bent", name: "rhai_bent", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let nx = (if z.re >= 0.0 { z.re } else { 2.0 * z.re });
    let ny = (if z.im >= 0.0 { z.im } else { z.im / 2.0 });
    complex(nx, ny)
}

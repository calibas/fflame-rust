/// Maps each coordinate through `sin`, folding the plane into a
/// [-1, 1] tile. Produces wavy, ribbon-like patterns that repeat
/// outward from the origin.
///
/// # Authors
/// - Scott Draves

// Sinusoidal, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Sinusoidal", name: "rhai_sinusoidal", category: "basic_2d", phase: "any" };

fn variation(z, p) {
    complex(sin(z.re), sin(z.im))
}

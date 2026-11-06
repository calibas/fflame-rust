/// Coordinate swap — outputs `(y, x)`. The simplest possible 2D coordinate
/// operation; useful as a building block in combination with other
/// variations.

// Bi-Linear, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Bi-Linear", name: "rhai_bi_linear", category: "basic_2d", phase: "any" };

fn variation(z, p) {
    complex(z.im, z.re)
}

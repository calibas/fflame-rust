/// Lengthwise unit-cylinder warp — `(x / sqrt(x² + 1), y)`. Compresses X
/// toward [-1, 1] without affecting Y.

// Cylinder 2, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Cylinder 2", name: "rhai_cylinder2", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    complex(z.re / sqrt(z.re * z.re + 1.0), z.im)
}

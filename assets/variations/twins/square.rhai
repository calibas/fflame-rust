/// Random 2D unit-square sampler — replaces the input with a uniformly
/// random point in `[-0.5, 0.5]²`.

// Square, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Square", name: "rhai_square", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    complex(rand() - 0.5, rand() - 0.5)
}

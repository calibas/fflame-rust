// Square 3D, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Square 3D", name: "rhai_square3d" };

fn variation(z, p) {
    complex(rand() - 0.5, rand() - 0.5)
}

fn variation3(v, p) {
    vec3(rand() - 0.5, rand() - 0.5, rand() - 0.5)
}

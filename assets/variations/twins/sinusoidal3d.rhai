/// 3D sinusoidal — applies `sin` to X and Y like Sinusoidal, then adds
/// `atan2(x², y²) · cos(z)` on the Z axis.
///
/// # Authors
/// - gossamer light

// Sinusoidal 3D, the built-in variation's twin: the sine of each part; in 3D, z the angle of the squares times cos z.

const INFO = #{ title: "Sinusoidal 3D", name: "rhai_sinusoidal3d", category: "full_3d", phase: "any" };

fn variation(z, p) {
    complex(sin(z.re), sin(z.im))
}

fn variation3(v, p) {
    vec3(sin(v.x), sin(v.y), atan2(v.x * v.x, v.y * v.y) * cos(v.z))
}

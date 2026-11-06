/// Same math as Sinusoidal (sine on each axis) but runs before the rest of
/// the variations. The variation's weight scales the output.
///
/// # Authors
/// - Scott Draves

// Pre-Sinusoidal, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Pre-Sinusoidal", name: "rhai_pre_sinusoidal", category: "advanced_2d", phase: "pre", z: "preserve_z" };

fn variation(z, p, ctx) {
    let weight = ctx.weight;
    complex(weight * sin(z.re), weight * sin(z.im))
}

fn variation3(v, p, ctx) {
    let weight = ctx.weight;
    vec3(weight * sin(v.x), weight * sin(v.y), weight * v.z)
}

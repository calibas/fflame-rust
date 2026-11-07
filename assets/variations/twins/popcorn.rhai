/// Adds sinusoidal displacement to each axis using the transform's affine
/// translation fields (`e` and `f`) as amplitudes. Like Popcorn2 but takes
/// its strength directly from the affine matrix instead of dedicated
/// sliders.
///
/// # Authors
/// - Scott Draves

// Popcorn, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Popcorn", name: "rhai_popcorn", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p, ctx) {
    let dx = tan(3.0 * z.im);
    if (!(abs(dx) <= 1.0e32)) {
        dx = 0.0;
    }
    let dy = tan(3.0 * z.re);
    if (!(abs(dy) <= 1.0e32)) {
        dy = 0.0;
    }
    complex(z.re + ctx.e * sin(dx), z.im + ctx.f * sin(dy))
}

fn variation3(v, p, ctx) {
    let dx = tan(3.0 * v.y);
    if (!(abs(dx) <= 1.0e32)) {
        dx = 0.0;
    }
    let dy = tan(3.0 * v.x);
    if (!(abs(dy) <= 1.0e32)) {
        dy = 0.0;
    }
    vec3(v.x + ctx.e * sin(dx), v.y + ctx.f * sin(dy), v.z)
}

/// Same math as Disc (wraps the plane onto a disc) but runs before the rest
/// of the variations. The variation's weight scales the result.
///
/// # Authors
/// - Scott Draves

// Pre-Disc, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Pre-Disc", name: "rhai_pre_disc", category: "advanced_2d", phase: "pre", z: "preserve_z" };

fn variation(z, p, ctx) {
    let PI = 3.14159265359;
    let weight = ctx.weight;
    let rad = sqrt(z.norm_sqr());
    let r = (weight / PI()) * atan2(z.re, z.im);
    complex(sin(PI() * rad) * r, cos(PI() * rad) * r)
}

fn variation3(v, p, ctx) {
    let PI = 3.14159265359;
    let weight = ctx.weight;
    let rad = sqrt(complex(v.x, v.y).norm_sqr());
    let r = (weight / PI()) * atan2(v.x, v.y);
    vec3(sin(PI() * rad) * r, cos(PI() * rad) * r, weight * v.z)
}

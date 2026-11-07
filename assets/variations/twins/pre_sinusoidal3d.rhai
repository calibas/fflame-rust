/// Pre-phase sinusoidal with 3D output — applies `(sin x, sin y)` to XY
/// (same as the standard sinusoidal), plus a Z output `atan2(sin²x, sin²y)
/// · cos(z)` that uses the just-written X/Y values. Distinct from the
/// existing `pre_sinusoidal`, which leaves Z alone.
///
/// # Authors
/// - gossamer light

// Pre Sinusoidal 3D, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Pre Sinusoidal 3D", name: "rhai_pre_sinusoidal3d", category: "full_3d", phase: "pre", z: "preserve_z" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    complex(w * sin(z.re), w * sin(z.im))
}

fn variation3(v, p, ctx) {
    let w = ctx.weight;
    let nx = w * sin(v.x);
    let ny = w * sin(v.y);
    let nz = w * atan2(nx * nx, ny * ny) * cos(v.z);
    vec3(nx, ny, nz)
}

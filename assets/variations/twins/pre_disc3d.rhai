/// Pre-phase 3D disc warp — applies a disc-style warp with an explicit Z
/// output `vv · r · cos(z)`. The `pi` parameter is user-configurable
/// (default π) and supplies the divisor in the `vv = w · atan2(x, y) / pi`
/// term, which makes it a non-standard variation where the meaning of π is
/// itself tunable.
///
/// # Authors
/// - gossamer light

// Pre Disc 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    pi: #{
        value: 3.1415927, min: -10.0, max: 10.0, free: true,
        caption: "Pi",
        hint: "Divisor in the `vv = w · atan2(x, y) / pi` term. Default is the literal value of π.",
    },
};

const INFO = #{ title: "Pre Disc 3D", name: "rhai_pre_disc3d", category: "full_3d", phase: "pre", z: "preserve_z" };

fn variation(z, p, ctx) {
    let pi_p = p.pi;
    let w = ctx.weight;
    let r = sqrt(z.im * z.im + z.re * z.re + 1.0e-30);
    let a = pi_p * r;
    let sr = sin(a);
    let cr = cos(a);
    let vv = w * atan2(z.re, z.im) / (pi_p + 1.0e-30);
    complex(vv * sr, vv * cr)
}

fn variation3(v, p, ctx) {
    let pi_p = p.pi;
    let w = ctx.weight;
    let r = sqrt(v.y * v.y + v.x * v.x + 1.0e-30);
    let a = pi_p * r;
    let sr = sin(a);
    let cr = cos(a);
    let vv = w * atan2(v.x, v.y) / (pi_p + 1.0e-30);
    vec3(vv * sr, vv * cr, vv * r * cos(v.z))
}

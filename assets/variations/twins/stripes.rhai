/// Vertical-stripe warp with parabolic Y bend — snaps X toward the
/// nearest integer (with the local offset compressed by `1 − space`),
/// and bends Y as a parabola of the local stripe offset. Produces
/// vertical stripes that arch up or down depending on `warp`.
///
/// # Authors
/// - Apophysis Plugin Pack

// Stripes, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    space: #{
        value: 0.2, min: -5.0, max: 5.0, free: true,
        caption: "Space",
        hint: "Compression factor for X within each stripe — 1 collapses to integer values, 0 disables compression.",
    },
    warp: #{
        value: 0.6, min: -5.0, max: 5.0, free: true,
        caption: "Warp",
        hint: "Parabolic Y-bend amplitude as a function of local stripe offset.",
    },
};

const INFO = #{ title: "Stripes", name: "rhai_stripes", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let space = p.space;
    let warp = p.warp;
    let round_x = to_float(floor(z.re + 0.5));
    let off = z.re - round_x;
    complex(off * (1.0 - space) + round_x, z.im + off * off * warp)
}

fn variation3(v, p) {
    let space = p.space;
    let warp = p.warp;
    let round_x = to_float(floor(v.x + 0.5));
    let off = v.x - round_x;
    vec3(off * (1.0 - space) + round_x, v.y + off * off * warp, v.z)
}

/// Rotates each point around the origin by an angle proportional to
/// its distance — the further out, the more it twists. Produces
/// whirlpool and pinwheel patterns.
///
/// # Authors
/// - Scott Draves

// Swirl, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Swirl", name: "rhai_swirl", category: "basic_2d", phase: "any" };

fn variation(z, p) {
    let r2 = z.norm_sqr();
    let s = sin(r2);
    let c = cos(r2);
    complex(z.re * s - z.im * c, z.re * c + z.im * s)
}

fn inverse(z, p, branch) {
    let r2 = z.norm_sqr();
    let s = sin(r2);
    let c = cos(r2);
    complex(z.re * s + z.im * c, z.im * s - z.re * c)
}

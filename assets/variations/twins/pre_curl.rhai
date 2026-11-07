/// Pre-phase version of Curl — applies the same complex-polynomial twist as
/// Curl but before the rest of the variations run.
///
/// # Authors
/// - Xyrus02

// Pre Curl, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    c1: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "C1",
        hint: "Linear twist strength. Stronger = tighter curl around the center.",
    },
    c2: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "C2",
        hint: "Quadratic twist strength. Adds a second-order curl that grows away from the origin.",
    },
};

const INFO = #{ title: "Pre Curl", name: "rhai_pre_curl", category: "advanced_2d", phase: "pre", z: "preserve_z" };

fn variation(z, p, ctx) {
    let c1 = p.c1;
    let c2 = p.c2;
    let w = ctx.weight;
    let re = 1.0 + c1 * z.re + c2 * (z.re * z.re - z.im * z.im);
    let im = c1 * z.im + 2.0 * c2 * z.re * z.im;
    let r = w / max(1.0e-30, re * re + im * im);
    complex((z.re * re + z.im * im) * r, (z.im * re - z.re * im) * r)
}

fn variation3(v, p, ctx) {
    let c1 = p.c1;
    let c2 = p.c2;
    let w = ctx.weight;
    let re = 1.0 + c1 * v.x + c2 * (v.x * v.x - v.y * v.y);
    let im = c1 * v.y + 2.0 * c2 * v.x * v.y;
    let r = w / max(1.0e-30, re * re + im * im);
    vec3((v.x * re + v.y * im) * r, (v.y * re - v.x * im) * r, v.z)
}

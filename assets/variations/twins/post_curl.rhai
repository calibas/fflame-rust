/// Same math as Curl (complex polynomial twist) but runs after all other
/// variations, distorting the final output coordinates.
///
/// # Authors
/// - Scott Draves

// Post Curl, the built-in variation's twin, translated from its WGSL.

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

const INFO = #{ title: "Post Curl", name: "rhai_post_curl", category: "advanced_2d", phase: "post" };

fn variation(z, p) {
    let c1 = p.c1;
    let c2 = p.c2;
    let re = 1.0 + c1 * z.re + c2 * (z.re * z.re - z.im * z.im);
    let im = c1 * z.im + 2.0 * c2 * z.re * z.im;
    let r = 1.0 / (re * re + im * im);
    complex((z.re * re + z.im * im) * r, (z.im * re - z.re * im) * r)
}

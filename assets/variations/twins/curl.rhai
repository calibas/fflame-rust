// Curl, the built-in variation's twin: a point divided by 1 + c1 z + c2 z^2.

const PARAMS = #{
    c1: #{
        value: 1.0, min: -2.0, max: 2.0, free: true,
        caption: "C1",
        hint: "Linear twist strength. Stronger = tighter curl around the center.",
    },
    c2: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "C2",
        hint: "Quadratic twist strength. Adds a second-order curl that grows away from the origin.",
    },
};

const INFO = #{ title: "Curl", name: "rhai_curl" };

fn variation(z, p) {
    let re = 1.0 + p.c1 * z.re + p.c2 * (z.re * z.re - z.im * z.im);
    let im = p.c1 * z.im + 2.0 * p.c2 * z.re * z.im;
    let r = 1.0 / (re * re + im * im);
    complex((z.re * re + z.im * im) * r, (z.im * re - z.re * im) * r)
}

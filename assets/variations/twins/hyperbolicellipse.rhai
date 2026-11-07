/// Hyperbolic-elliptic mapping — emits `(sinh(x) · cos(a·y), cosh(x) ·
/// sin(a·y))`. Echoes the parametric ellipse `(cos t, sin t)` but with
/// hyperbolic radial scaling along X.

// Hyperbolic Ellipse, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Frequency multiplier on Y in the cos/sin arguments.",
    },
};

const INFO = #{ title: "Hyperbolic Ellipse", name: "rhai_hyperbolicellipse", category: "advanced_2d", phase: "any", replace: true };

fn variation(z, p) {
    let a = p.a;
    let xt = sinh(z.re) * cos(a * z.im);
    let yt = cosh(z.re) * sin(a * z.im);
    complex(xt, yt)
}

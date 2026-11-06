/// Cos/tan composite warp — outputs `(cos(s)·tan(x), sin(s)·tan(y))` where
/// `s = sqrt(a·x² + b·y²)`. The tangent terms produce vertical and
/// horizontal stripes near `x = π/2 + kπ` etc.
///
/// # Authors
/// - Raykoid666

// Squirrel, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "X² weight in the radius term.",
    },
    b: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Y² weight in the radius term.",
    },
};

const INFO = #{ title: "Squirrel", name: "rhai_squirrel", category: "advanced_2d", phase: "any", replace: true };

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let eps = 1.0e-6;
    let u = (a + eps) * z.re * z.re + (b + eps) * z.im * z.im;
    let s = sqrt(max(0.0, u));
    complex(cos(s) * tan(z.re), sin(s) * tan(z.im))
}

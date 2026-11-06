/// Sin × (squared − weighted-radius) — per axis emits `sin(coord) · (coord²
/// + w − (x² + y²)·w)`. The trailing weighted-radius subtraction modulates
/// the local sin profile based on distance from the origin.
///
/// # Authors
/// - Ffey

// Sin T Range, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    w: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "W",
        hint: "Weight on the radius term `(x²+y²)·w` and the constant offset `+w`. Distinct from the variation's outer weight (VVAR).",
    },
};

const INFO = #{ title: "Sin T Range", name: "rhai_sintrange", category: "advanced_2d", phase: "any", replace: true };

fn variation(z, p) {
    let w = p.w;
    let v = (z.re * z.re + z.im * z.im) * w;
    let fx = sin(z.re) * (z.re * z.re + w - v);
    let fy = sin(z.im) * (z.im * z.im + w - v);
    complex(fx, fy)
}

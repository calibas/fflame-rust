/// Radial spiral — emits `(x · radius · cos(r² + ε), x · radius · sin(r² +
/// ε))` where `r² = x² + y²`. The angular position rotates with squared
/// radius, producing tightly wound layered spirals.
///
/// # Authors
/// - Will Evans

// Layered Spiral, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Radius",
        hint: "Radial scale on the spiral magnitude.",
    },
};

const INFO = #{ title: "Layered Spiral", name: "rhai_layered_spiral", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let radius = p.radius;
    let a = z.re * radius;
    let t = z.re * z.re + z.im * z.im + 1.0e-9;
    complex(a * cos(t), a * sin(t))
}

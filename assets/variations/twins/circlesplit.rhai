/// Pass-through inside a `radius − split` disc; outside, points get pushed
/// outward by `split`. Creates a visible gap ring between the inner and
/// outer regions.
///
/// # Authors
/// - Tatyana Zabanova
/// - Brad Stefanov

// Circle Split, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    radius: #{
        value: 1.0, min: 0.0, max: 5.0, free: true,
        caption: "Radius",
        hint: "Distance from origin where the splitting starts.",
    },
    split: #{
        value: 0.5, min: 0.0, max: 5.0, free: true,
        caption: "Split",
        hint: "Gap size — how far outside the radius points get pushed.",
    },
};

const INFO = #{ title: "Circle Split", name: "rhai_circlesplit", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let radius = p.radius;
    let split = p.split;
    let r = sqrt(z.re * z.re + z.im * z.im);
    if (r < radius - split) {
        return z;
    }
    let a = atan2(z.im, z.re);
    let len = r + split;
    complex(cos(a) * len, sin(a) * len)
}

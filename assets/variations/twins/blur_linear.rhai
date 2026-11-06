/// Directional linear-segment blur — each iteration scatters the point
/// along a fixed-angle line of random length up to `length`. Produces
/// directional streaks.
///
/// # Authors
/// - Joel Faber
/// - DarkBeam

// Blur Linear, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    length: #{
        value: 1.0, min: 0.0, max: 10.0, free: true,
        caption: "Length",
        hint: "Maximum scatter distance along the blur direction.",
    },
    angle: #{
        value: 0.5, min: -6.28318, max: 6.28318, free: true,
        caption: "Angle (rad)",
        hint: "Direction of the blur, in radians.",
    },
};

const INFO = #{ title: "Blur Linear", name: "rhai_blur_linear", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let angle = p.angle;
    #{ cos_a: cos(angle), sin_a: sin(angle) }
}

fn variation(z, p) {
    let length_p = p.length;
    let cos_a = p.cos_a;
    let sin_a = p.sin_a;
    let r = length_p * rand();
    complex(z.re + r * cos_a, z.im + r * sin_a)
}

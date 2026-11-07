/// Wedge applied in spherical-inversion space — like Wedge but the input
/// is first inverted through the unit circle, producing a wrapped/folded
/// version of the wedge pattern.
///
/// # Authors
/// - Apophysis Plugin Pack

// Wedge Sph, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    angle: #{
        value: 0.2, min: -10.0, max: 10.0, free: true,
        caption: "Angle",
        hint: "Wedge sector rotation, in radians.",
    },
    hole: #{
        value: 0.2, min: -10.0, max: 10.0, free: true,
        caption: "Hole",
        hint: "Radial offset added after the inversion. Positive opens a hole at the center; negative compresses inward.",
    },
    count: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Count",
        hint: "Number of wedge sectors around the center.",
    },
    swirl: #{
        value: 0.3, min: -10.0, max: 10.0, free: true,
        caption: "Swirl",
        hint: "Extra rotation that grows with distance — gives the wedges a spiral.",
    },
};

const INFO = #{ title: "Wedge Sph", name: "rhai_wedge_sph", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let angle = p.angle;
    let count = p.count;
    let inv_two_pi = 0.15915494309189535;
    #{ cf: 1.0 - angle * count * inv_two_pi }
}

fn variation(z, p) {
    let angle = p.angle;
    let hole = p.hole;
    let count = p.count;
    let swirl = p.swirl;
    let cf = p.cf;
    let inv_two_pi = 0.15915494309189535;
    let r0 = 1.0 / (sqrt(z.re * z.re + z.im * z.im) + 1.0e-6);
    let a = atan2(z.re, z.im) + swirl * r0;
    let c = to_float(floor((count * a + 3.14159265358979) * inv_two_pi));
    a = a * cf + c * angle;
    let r = r0 + hole;
    complex(r * cos(a), r * sin(a))
}

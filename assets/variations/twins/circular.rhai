/// Randomized rotation by a deterministic-plus-RNG term — each iteration
/// adds a small random angle to the point's polar angle. The `seed`
/// parameter changes the noise pattern.
///
/// # Authors
/// - Tatyana Zabanova

// Circular, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    angle: #{
        value: 90.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Angle",
        hint: "Maximum rotation per iteration (degrees).",
    },
    seed: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Seed",
        hint: "Random seed for the noise term — change to vary the pattern.",
    },
};

const INFO = #{ title: "Circular", name: "rhai_circular", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let angle_deg = p.angle;
    let seed = p.seed;
    let c_a = angle_deg * 3.14159265358979 / 180.0;
    let h = sin(z.re * 12.9898 + z.im * 78.233 + seed) * 43758.5453;
    let aux = h - to_float(trunc(h));
    let rnd = (2.0 * (rand() + aux) - 2.0) * c_a;
    let rad = sqrt(z.re * z.re + z.im * z.im);
    let ang = atan2(z.im, z.re);
    complex(cos(ang + rnd) * rad, sin(ang + rnd) * rad)
}

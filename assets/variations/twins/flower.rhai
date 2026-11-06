/// Flower — produces petal patterns based on a uniform-random distance
/// scaled by `cos(petals · angle)`. The `holes` parameter controls how
/// hollow the center is.
///
/// # Authors
/// - cyberxaos

// Flower, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    holes: #{
        value: 0.4, min: -10.0, max: 10.0, free: true,
        caption: "Holes",
        hint: "How hollow the center of the flower is. Higher = bigger center hole.",
    },
    petals: #{
        value: 7.0, min: 1.0, max: 64.0, free: true,
        caption: "Petals",
        hint: "Number of petals around the flower.",
    },
};

const INFO = #{ title: "Flower", name: "rhai_flower", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let holes = p.holes;
    let petals = p.petals;
    let theta = atan2(z.re, z.im);
    let d = sqrt(z.re * z.re + z.im * z.im) + 1.0e-30;
    let r = (rand() - holes) * cos(petals * theta) / d;
    complex(r * z.re, r * z.im)
}

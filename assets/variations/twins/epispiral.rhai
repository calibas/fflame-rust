/// Maps the plane onto an epicycloid spiral pattern. `n` controls how many
/// lobes, `thickness` adds randomness, `holes` punches gaps in the pattern.
/// 
/// # Authors
/// - Joel Faber
/// - cyberxaos

// Epispiral, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    n: #{
        value: 6.0, min: -20.0, max: 20.0, free: true,
        caption: "N",
        hint: "Number of lobes in the spiral pattern.",
    },
    thickness: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Thickness",
        hint: "Random thickness of each lobe. 0 = razor-thin curves, higher = wider bands.",
    },
    holes: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Holes",
        hint: "Radial offset that punches gaps in the pattern.",
    },
};

const INFO = #{ title: "Epispiral", name: "rhai_epispiral", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let n = p.n;
    let thickness = p.thickness;
    let holes = p.holes;
    let theta = atan2(z.im, z.re);
    let t = rand() * thickness / cos(n * theta) - holes;
    if (abs(t) < 1.0e-6) {
        return complex(0.0, 0.0);
    }
    complex(t * cos(theta), t * sin(theta))
}

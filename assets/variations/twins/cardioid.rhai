/// Parameterized cardioid (heart-shaped curve). The `a` parameter controls
/// how many lobes/cusps the shape has.
///
/// # Authors
/// - Michael Faber

// Cardioid, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Number of cusps/lobes in the cardioid shape. 1 = standard heart, 2 = figure-eight, higher values add more lobes.",
    },
};

const INFO = #{ title: "Cardioid", name: "rhai_cardioid", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let a_param = p.a;
    let a = atan2(z.im, z.re);
    let r = sqrt(max(z.re * z.re + z.im * z.im + sin(a * a_param) + 1.0, 0.0));
    complex(r * cos(a), r * sin(a))
}

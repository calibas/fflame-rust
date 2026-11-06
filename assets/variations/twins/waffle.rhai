/// Waffle grid scatter — picks one of 5 sampling modes uniformly, then
/// samples from a `slices × slices` grid with configurable per-axis line
/// thickness. The output is rotated by `rotation`. Produces a woven-pattern
/// scatter.
///
/// # Authors
/// - Jed Kelsey

// Waffle, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    slices: #{
        value: 6, min: 1, max: 100,
        caption: "Slices",
        hint: "Number of grid divisions per axis.",
    },
    xthickness: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "X thickness",
        hint: "X-axis line thickness (0 = grid lines only, 1 = solid fill).",
    },
    ythickness: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Y thickness",
        hint: "Y-axis line thickness.",
    },
    rotation: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Rotation",
        hint: "Output rotation in radians.",
    },
};

const INFO = #{ title: "Waffle", name: "rhai_waffle", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ cos_rot: cos(p.rotation), sin_rot: sin(p.rotation) }
}

fn variation(z, p) {
    let slices = max(1.0, to_float(p.slices));
    let xth = p.xthickness;
    let yth = p.ythickness;
    let cos_rot = p.cos_rot;
    let sin_rot = p.sin_rot;
    let inv_s = 1.0 / slices;
    let case_i = to_int(to_float(floor(rand() * 5.0)));
    let a = 0.0;
    let r = 0.0;
    if (case_i == 0) {
        a = (to_float(floor(rand() * slices)) + rand() * xth) * inv_s;
        r = (to_float(floor(rand() * slices)) + rand() * yth) * inv_s;
    } else if (case_i == 1) {
        a = (to_float(floor(rand() * slices)) + rand()) * inv_s;
        r = (to_float(floor(rand() * slices)) + yth) * inv_s;
    } else if (case_i == 2) {
        a = (to_float(floor(rand() * slices)) + xth) * inv_s;
        r = (to_float(floor(rand() * slices)) + rand()) * inv_s;
    } else if (case_i == 3) {
        a = rand();
        r = (to_float(floor(rand() * slices)) + yth + rand() * (1.0 - yth)) * inv_s;
    } else {
        a = (to_float(floor(rand() * slices)) + xth + rand() * (1.0 - xth)) * inv_s;
        r = rand();
    }
    complex(cos_rot * a + sin_rot * r, -sin_rot * a + cos_rot * r)
}

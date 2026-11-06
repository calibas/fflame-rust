/// Slices the plane into N pie wedges, each compressed and offset by the
/// chosen angle. Adds an optional swirl that increases with distance and a
/// `hole` radial offset.
///
/// # Authors
/// - Apophysis Plugin Pack

// Wedge, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    angle: #{
        value: 90.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Angle",
        hint: "Wedge angle in degrees — how wide each pie slice is before compression.",
    },
    hole: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Hole",
        hint: "Radial offset added to the output. Positive pushes the pattern outward, negative pulls it inward.",
    },
    count: #{
        value: 2, min: 1, max: 20,
        caption: "Count",
        hint: "Number of pie wedges arranged around the center.",
    },
    swirl: #{
        value: 0.0, min: -30.0, max: 30.0, free: true,
        caption: "Swirl",
        hint: "Extra rotation that grows with distance. 0 = no swirl, positive = curves arms outward.",
    },
};

const INFO = #{ title: "Wedge", name: "rhai_wedge", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let angle_deg = p.angle;
    let count = to_float(p.count);
    let angle_rad = angle_deg * 3.14159265358979 / 180.0;
    #{ angle_rad: angle_rad, comp_fac: 1.0 - angle_rad * count * 0.15915494309189534 }
}

fn variation(z, p) {
    let PI = 3.14159265359;
    let C1_2PI = 0.15915494309189534;
    let hole = p.hole;
    let count = to_float(p.count);
    let swirl = p.swirl;
    let angle_rad = p.angle_rad;
    let comp_fac = p.comp_fac;
    let r = sqrt(z.norm_sqr());
    let a = atan2(z.im, z.re) + swirl * r;
    let c = to_float(floor((count * a + PI()) * C1_2PI));
    a = a * comp_fac + c * angle_rad;
    let r_out = r + hole;
    complex(r_out * cos(a), r_out * sin(a))
}

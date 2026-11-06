/// Pushes positive-X points and negative-X points apart by `x`, and same
/// for Y. Creates a gap down the middle along each axis.
///
/// # Authors
/// - Apophysis Plugin Pack

// Splits, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 0.4, min: -2.0, max: 2.0, free: true,
        caption: "X",
        hint: "Horizontal gap. Pushes positive-X and negative-X points apart by this amount.",
    },
    y: #{
        value: 0.4, min: -2.0, max: 2.0, free: true,
        caption: "Y",
        hint: "Vertical gap. Pushes positive-Y and negative-Y points apart by this amount.",
    },
    lshear: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Left Shear",
        hint: "Y shift subtracted from points with x < 0.",
    },
    rshear: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Right Shear",
        hint: "Y shift added to points with x >= 0.",
    },
    ushear: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Up Shear",
        hint: "X shift added to points with y >= 0.",
    },
    dshear: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Down Shear",
        hint: "X shift subtracted from points with y < 0.",
    },
};

const INFO = #{ title: "Splits", name: "rhai_splits", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let splits_x = p.x;
    let splits_y = p.y;
    let lshear = p.lshear;
    let rshear = p.rshear;
    let ushear = p.ushear;
    let dshear = p.dshear;
    let px = (if z.re >= 0.0 { z.re + splits_x } else { z.re - splits_x }) + (if z.im >= 0.0 { ushear } else { -dshear });
    let py = (if z.im >= 0.0 { z.im + splits_y } else { z.im - splits_y }) + (if z.re >= 0.0 { rshear } else { -lshear });
    complex(px, py)
}

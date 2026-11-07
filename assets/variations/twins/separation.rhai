/// Pushes points away from the X and Y axes by configurable amounts, with
/// separate inside/outside offsets. Creates a split, mirrored look.
///
/// # Authors
/// - Apophysis Plugin Pack

// Separation, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 1.0, min: -20.0, max: 20.0, free: true,
        caption: "X",
        hint: "How far to push points away from the X axis on either side.",
    },
    y: #{
        value: 1.0, min: -20.0, max: 20.0, free: true,
        caption: "Y",
        hint: "How far to push points away from the Y axis on either side.",
    },
    xinside: #{
        value: 0.0, min: -20.0, max: 20.0, free: true,
        caption: "X Inside",
        hint: "Inside offset along X — adjusts how the separation looks near the axis.",
    },
    yinside: #{
        value: 0.0, min: -20.0, max: 20.0, free: true,
        caption: "Y Inside",
        hint: "Inside offset along Y — adjusts how the separation looks near the axis.",
    },
};

const INFO = #{ title: "Separation", name: "rhai_separation", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let sep_x = p.x;
    let sep_y = p.y;
    let xinside = p.xinside;
    let yinside = p.yinside;
    let x_out = (if z.re > 0.0 { sqrt(z.re * z.re + sep_x * sep_x) - z.re * xinside } else { -(sqrt(z.re * z.re + sep_x * sep_x) + z.re * xinside) });
    let y_out = (if z.im > 0.0 { sqrt(z.im * z.im + sep_y * sep_y) - z.im * yinside } else { -(sqrt(z.im * z.im + sep_y * sep_y) + z.im * yinside) });
    complex(x_out, y_out)
}

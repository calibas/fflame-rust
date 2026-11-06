/// Inside a unit disc the points rotate and twist; outside the disc they're
/// pushed away from the center. Produces a layered, plate-like swirl.
///
/// # Authors
/// - Apophysis Plugin Pack

// LazySusan, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    spin: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Spin",
        hint: "How far points inside the unit disc rotate.",
    },
    space: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Space",
        hint: "Gap added to points outside the disc - pushes the outer region away from center.",
    },
    twist: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Twist",
        hint: "Extra rotation that fades with distance. Adds a twisting motion to the inside.",
    },
    x: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "X Offset",
        hint: "Horizontal offset of the rotation center.",
    },
    y: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Y Offset",
        hint: "Vertical offset of the rotation center.",
    },
};

const INFO = #{ title: "LazySusan", name: "rhai_lazysusan", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let PI = 3.14159265359;
    let spin = p.spin * PI() / 180.0;
    let space = p.space;
    let twist = p.twist;
    let x_offset = p.x;
    let y_offset = p.y;
    let x = z.re - x_offset;
    let y = z.im + y_offset;
    let r = sqrt(x * x + y * y);
    if (r < 1.0) {
        let a = atan2(y, x) + spin + twist * (1.0 - r);
        complex(r * cos(a) + x_offset, r * sin(a) - y_offset)
    } else {
        let r_scale = 1.0 + space / (r + 1.0e-6);
        complex(r_scale * x + x_offset, r_scale * y - y_offset)
    }
}

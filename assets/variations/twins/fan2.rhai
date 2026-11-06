/// Slices the plane into pie wedges and offsets each wedge alternately -
/// even wedges go one way, odd wedges the other. Configurable wedge width
/// and rotation.
///
/// # Authors
/// - Scott Draves

// Fan2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 1.0, min: 0.01, max: 5.0, free: true,
        caption: "X",
        hint: "Wedge width. Controls how many sectors the fan is split into.",
    },
    y: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Y",
        hint: "Rotation offset. Spins the whole fan around the origin.",
    },
};

const INFO = #{ title: "Fan2", name: "rhai_fan2", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let PI = 3.14159265359;
    let x_param = p.x;
    let y_param = p.y;
    let dx = PI() * (x_param * x_param + 1.0e-10);
    let dx2 = dx / 2.0;
    let angle = atan2(z.re, z.im);
    let a = 0.0;
    if (fract((angle + y_param) / dx) > 0.5) {
        a = angle - dx2;
    } else {
        a = angle + dx2;
    }
    let r = sqrt(z.norm_sqr());
    complex(r * cos(a), r * sin(a))
}

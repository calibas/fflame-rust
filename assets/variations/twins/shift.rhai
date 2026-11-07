/// Rotated additive shift — adds a 2D shift vector to the input, where the
/// shift direction is rotated by `angle` degrees. Equivalent to translating
/// in a rotated frame.
///
/// # Authors
/// - Tatyana Zabanova
/// - Brad Stefanov

// Shift, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    shift_x: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Shift X",
        hint: "X-axis shift amount (in the rotated frame).",
    },
    shift_y: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Shift Y",
        hint: "Y-axis shift amount (in the rotated frame).",
    },
    angle: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Angle (deg)",
        hint: "Rotation angle of the shift vector, in degrees.",
    },
};

const INFO = #{ title: "Shift", name: "rhai_shift", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn prepare(p) {
    let ang_rad = p.angle * 3.14159265358979 / 180.0;
    #{ cs: cos(ang_rad), sn: sin(ang_rad) }
}

fn variation(z, p) {
    let sx = p.shift_x;
    let sy = p.shift_y;
    let cs = p.cs;
    let sn = p.sn;
    complex(z.re + cs * sx - sn * sy, z.im - cs * sy - sn * sx)
}

fn variation3(v, p) {
    let sx = p.shift_x;
    let sy = p.shift_y;
    let cs = p.cs;
    let sn = p.sn;
    vec3(v.x + cs * sx - sn * sy, v.y - cs * sy - sn * sx, v.z)
}

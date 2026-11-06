/// 3D extension of `popcorn2` — modulates XY by `0.5 · (coord + scale ·
/// sin(tan(c · other_coord)))` (the standard popcorn2 form, halved) and
/// adds a Z output combining `sin(tan(c)) · atan2(y, x)` with an optional
/// Z-dependent term scaled by `popcorn2_3D_z`. The accumulator-read path in
/// the cpp (`otherZ ≠ 0`) is unreachable in our model since `FPz` starts at
/// 0 each iteration, so we always take the `otherZ == 0` branch.
///
/// # Authors
/// - Larry Berlin

// Popcorn2 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    popcorn2_3D_x: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "X",
        hint: "X-axis sine amplitude (multiplies `sin(tan(c · y))` in the X output).",
    },
    popcorn2_3D_y: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Y",
        hint: "Y-axis sine amplitude (multiplies `sin(tan(c · x))` in the Y output).",
    },
    popcorn2_3D_z: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Z",
        hint: "Z-axis amplitude on the secondary `sin(tan(c)) · temp_tz` term.",
    },
    popcorn2_3D_c: #{
        value: 3.0, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Frequency multiplier inside `tan(c · coord)` for the XY modulators and `tan(c)` for the Z term.",
    },
};

const INFO = #{ title: "Popcorn2 3D", name: "rhai_popcorn2_3d", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let xp = p.popcorn2_3D_x;
    let yp = p.popcorn2_3D_y;
    let cp = p.popcorn2_3D_c;
    let tan_y = tan(cp * z.im);
    tan_y = (if abs(tan_y) <= 1.0e32 { tan_y } else { 0.0 });
    let tan_x = tan(cp * z.re);
    tan_x = (if abs(tan_x) <= 1.0e32 { tan_x } else { 0.0 });
    let nx = 0.5 * (z.re + xp * sin(tan_y));
    let ny = 0.5 * (z.im + yp * sin(tan_x));
    complex(nx, ny)
}

fn variation3(v, p, ctx) {
    let xp = p.popcorn2_3D_x;
    let yp = p.popcorn2_3D_y;
    let zp = p.popcorn2_3D_z;
    let cp = p.popcorn2_3D_c;
    let w = ctx.weight;
    let tan_y = tan(cp * v.y);
    tan_y = (if abs(tan_y) <= 1.0e32 { tan_y } else { 0.0 });
    let tan_x = tan(cp * v.x);
    tan_x = (if abs(tan_x) <= 1.0e32 { tan_x } else { 0.0 });
    let tan_c = tan(cp);
    tan_c = (if abs(tan_c) <= 1.0e32 { tan_c } else { 0.0 });
    let nx = 0.5 * (v.x + xp * sin(tan_y));
    let ny = 0.5 * (v.y + yp * sin(tan_x));
    let ratio = min(abs(w), 1.0);
    let sin_tan_c = sin(tan_c);
    let temp_tz = (if abs(v.z) < 1.0e-30 { sin_tan_c * atan2(v.y, v.x) } else { v.z });
    let temp_pz_over_w = ratio * sin_tan_c * atan2(v.y, v.x);
    let nz = temp_pz_over_w + ratio * zp * sin_tan_c * temp_tz;
    vec3(nx, ny, nz)
}

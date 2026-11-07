/// Random cube-face sampler with direct-color writes — picks one of 6 unit
/// cube faces (3 axes × 2 sides) and emits a random point on that face,
/// scaled per-axis by `(x, y, z)`. Each face has its own color (`c1..c6`,
/// clamped to `[0, 1]`) written to the color register, so a flame using
/// `dc_cube` shows the cube as 6 distinct colored faces.
///
/// Discards the input position entirely (samples a new point per
/// iteration). Visible color requires the transform's Direct Color
/// slider > 0.

// DC Cube, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X Scale",
        hint: "X-axis output scale.",
    },
    y: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Scale",
        hint: "Y-axis output scale.",
    },
    z: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Z Scale",
        hint: "Z-axis output scale (3D only).",
    },
    c1: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Color +X",
        hint: "Color (clamped to [0, 1]) written when the +X face is picked.",
    },
    c2: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Color -X",
        hint: "Color (clamped to [0, 1]) written when the -X face is picked.",
    },
    c3: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Color +Y",
        hint: "Color (clamped to [0, 1]) written when the +Y face is picked.",
    },
    c4: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Color -Y",
        hint: "Color (clamped to [0, 1]) written when the -Y face is picked.",
    },
    c5: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Color +Z",
        hint: "Color (clamped to [0, 1]) written when the +Z face is picked.",
    },
    c6: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Color -Z",
        hint: "Color (clamped to [0, 1]) written when the -Z face is picked.",
    },
};

const INFO = #{ title: "DC Cube", name: "rhai_dc_cube", category: "full_3d", phase: "any" };

fn prepare(p) {
    #{ d0: clamp(p.c1, 0.0, 1.0), d1: clamp(p.c2, 0.0, 1.0), d2: clamp(p.c3, 0.0, 1.0), d3: clamp(p.c4, 0.0, 1.0), d4: clamp(p.c5, 0.0, 1.0), d5: clamp(p.c6, 0.0, 1.0) }
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let xs = p.x;
    let ys = p.y;
    let pp = 2.0 * rand() - 1.0;
    let q = 2.0 * rand() - 1.0;
    let i = to_int(rand() * 3.0);
    let j = to_int(rand() * 2.0);
    let s = (if j == 1 { -1.0 } else { 1.0 });
    let x = 0.0;
    let y = 0.0;
    if (i <= 0) {
        x = s;
        y = pp;
        color = (if j == 1 { p.d1 } else { p.d0 });
    } else if (i == 1) {
        x = pp;
        y = s;
        color = (if j == 1 { p.d3 } else { p.d2 });
    } else {
        x = pp;
        y = q;
        color = (if j == 1 { p.d5 } else { p.d4 });
    }
    #{ z: complex(x * xs, y * ys), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let xs = p.x;
    let ys = p.y;
    let zs = p.z;
    let pp = 2.0 * rand() - 1.0;
    let q = 2.0 * rand() - 1.0;
    let i = to_int(rand() * 3.0);
    let j = to_int(rand() * 2.0);
    let s = (if j == 1 { -1.0 } else { 1.0 });
    let x = 0.0;
    let y = 0.0;
    let z = 0.0;
    if (i <= 0) {
        x = s;
        y = pp;
        z = q;
        color = (if j == 1 { p.d1 } else { p.d0 });
    } else if (i == 1) {
        x = pp;
        y = s;
        z = q;
        color = (if j == 1 { p.d3 } else { p.d2 });
    } else {
        x = pp;
        y = q;
        z = s;
        color = (if j == 1 { p.d5 } else { p.d4 });
    }
    #{ z: vec3(x * xs, y * ys, z * zs), color: color }
}

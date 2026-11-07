/// Bubble warp with twist and tilt — applies a bubble-style radial scaling
/// `r = 1 / (r²/4 + 1)` to each axis with independent scale factors, then
/// adds a quadratic twist term to X and a linear tilt term to Y.
///
/// # Authors
/// - FracFx

// MCarpet, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X scale",
        hint: "X-axis bubble scale.",
    },
    y: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y scale",
        hint: "Y-axis bubble scale.",
    },
    twist: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Twist",
        hint: "Quadratic twist amount on X output (subtracts `twist · x²`).",
    },
    tilt: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Tilt",
        hint: "Linear tilt amount on Y output (adds `tilt · x`).",
    },
};

const INFO = #{ title: "MCarpet", name: "rhai_mcarpet", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let xs = p.x;
    let ys = p.y;
    let twist = p.twist;
    let tilt = p.tilt;
    let T = 0.25 * (z.re * z.re + z.im * z.im) + 1.0;
    let r = 1.0 / max(1.0e-30, T);
    complex(z.re * r * xs + (1.0 - twist * z.re * z.re + z.im), z.im * r * ys + tilt * z.re)
}

fn variation3(v, p) {
    let xs = p.x;
    let ys = p.y;
    let twist = p.twist;
    let tilt = p.tilt;
    let T = 0.25 * (v.x * v.x + v.y * v.y) + 1.0;
    let r = 1.0 / max(1.0e-30, T);
    vec3(v.x * r * xs + (1.0 - twist * v.x * v.x + v.y), v.y * r * ys + tilt * v.x, v.z)
}

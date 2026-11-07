/// Per-axis sign-preserving power (2D) — applies `sign(coord) ·
/// |coord|^pow` to each of X and Y independently, with separate exponents
/// per axis. The 2D analogue of `lineart3d`.
///
/// # Authors
/// - FractalDesire

// Linear T, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    powX: #{
        value: 1.2, min: -10.0, max: 10.0, free: true,
        caption: "Power X",
        hint: "X-axis power exponent.",
    },
    powY: #{
        value: 1.2, min: -10.0, max: 10.0, free: true,
        caption: "Power Y",
        hint: "Y-axis power exponent.",
    },
};

const INFO = #{ title: "Linear T", name: "rhai_lineart", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let powx = p.powX;
    let powy = p.powY;
    let t1 = z.re;
    let sx = (if z.re == 0.0 { 1.0 } else { (if t1 > 0.0 { 1.0 } else if t1 < 0.0 { -1.0 } else { 0.0 }) });
    let t2 = z.im;
    let sy = (if z.im == 0.0 { 1.0 } else { (if t2 > 0.0 { 1.0 } else if t2 < 0.0 { -1.0 } else { 0.0 }) });
    complex(sx * pow(max(1.0e-30, abs(z.re)), powx), sy * pow(max(1.0e-30, abs(z.im)), powy))
}

fn variation3(v, p) {
    let powx = p.powX;
    let powy = p.powY;
    let t1 = v.x;
    let sx = (if v.x == 0.0 { 1.0 } else { (if t1 > 0.0 { 1.0 } else if t1 < 0.0 { -1.0 } else { 0.0 }) });
    let t2 = v.y;
    let sy = (if v.y == 0.0 { 1.0 } else { (if t2 > 0.0 { 1.0 } else if t2 < 0.0 { -1.0 } else { 0.0 }) });
    vec3(sx * pow(max(1.0e-30, abs(v.x)), powx), sy * pow(max(1.0e-30, abs(v.y)), powy), v.z)
}

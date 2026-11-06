/// Per-axis sign-preserving power — applies `sign(coord) · |coord|^pow` to
/// each axis independently, with separate exponents per axis. Preserves the
/// sign while compressing or expanding the magnitude.
///
/// # Authors
/// - FractalDesire

// Linear T 3D, the built-in variation's twin, translated from its WGSL.

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
    powZ: #{
        value: 1.2, min: -10.0, max: 10.0, free: true,
        caption: "Power Z",
        hint: "Z-axis power exponent (3D only).",
    },
};

const INFO = #{ title: "Linear T 3D", name: "rhai_lineart3d", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let powx = p.powX;
    let powy = p.powY;
    let t1 = z.re;
    let sx = (if z.re == 0.0 { 1.0 } else { (if t1 > 0.0 { 1.0 } else if t1 < 0.0 { -1.0 } else { 0.0 }) });
    let t2 = z.im;
    let sy = (if z.im == 0.0 { 1.0 } else { (if t2 > 0.0 { 1.0 } else if t2 < 0.0 { -1.0 } else { 0.0 }) });
    complex(sx * pow(max(abs(z.re), 1.0e-30), powx), sy * pow(max(abs(z.im), 1.0e-30), powy))
}

fn variation3(v, p) {
    let powx = p.powX;
    let powy = p.powY;
    let powz = p.powZ;
    let t1 = v.x;
    let sx = (if v.x == 0.0 { 1.0 } else { (if t1 > 0.0 { 1.0 } else if t1 < 0.0 { -1.0 } else { 0.0 }) });
    let t2 = v.y;
    let sy = (if v.y == 0.0 { 1.0 } else { (if t2 > 0.0 { 1.0 } else if t2 < 0.0 { -1.0 } else { 0.0 }) });
    let t3 = v.z;
    let sz = (if v.z == 0.0 { 1.0 } else { (if t3 > 0.0 { 1.0 } else if t3 < 0.0 { -1.0 } else { 0.0 }) });
    vec3(sx * pow(max(abs(v.x), 1.0e-30), powx), sy * pow(max(abs(v.y), 1.0e-30), powy), sz * pow(max(abs(v.z), 1.0e-30), powz))
}

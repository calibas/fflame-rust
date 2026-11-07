/// 3D spherical inversion with adjustable exponent — emits `(x, y, z) /
/// r^exponent` where `r² = x² + y² + z²`. With `exponent = 2` (the default)
/// this reduces to a standard 3D spherical inversion; other values produce
/// stronger or weaker radial scaling. `invert` flips the sign of the
/// output.

// Spherical 3D WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    invert: #{
        value: 0, min: 0, max: 1,
        caption: "Invert",
        hint: "1 = flip the sign of the output (inverts through the origin); 0 = standard direction.",
    },
    exponent: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Exponent",
        hint: "Radial-inversion exponent. 2 = standard spherical (`r⁻²`); higher = stronger inverse; lower = weaker.",
    },
};

const INFO = #{ title: "Spherical 3D WF", name: "rhai_spherical3d_wf", category: "full_3d", phase: "any" };

fn prepare(p) {
    let out = array(1, 0.0);
    if (abs(p.exponent - 2.0) < 1.0e-6) {
        out[0] = 1.0;
    } else {
        out[0] = 0.0;
    }
    #{ regular: out[0] }
}

fn variation(z, p) {
    let invert = to_int(to_float(p.invert));
    let exponent = p.exponent;
    let regular = p.regular > 0.5;
    let small = 1.0e-30;
    let denom = z.re * z.re + z.im * z.im + small;
    let r = 0.0;
    if (regular) {
        r = 1.0 / denom;
    } else {
        r = 1.0 / pow(max(denom, small), exponent * 0.5);
    }
    if (invert != 0) {
        r = -r;
    }
    complex(z.re * r, z.im * r)
}

fn variation3(v, p) {
    let invert = to_int(to_float(p.invert));
    let exponent = p.exponent;
    let regular = p.regular > 0.5;
    let small = 1.0e-30;
    let denom = v.x * v.x + v.y * v.y + v.z * v.z + small;
    let r = 0.0;
    if (regular) {
        r = 1.0 / denom;
    } else {
        r = 1.0 / pow(max(denom, small), exponent * 0.5);
    }
    if (invert != 0) {
        r = -r;
    }
    vec3(v.x * r, v.y * r, v.z * r)
}

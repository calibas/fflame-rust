/// Complex-power Möbius warp — treats the input as a complex number `z = x
/// + iy`, computes `c·z^power + 1`, and applies the Möbius transform `z' =
/// (c+1)·z / (c·z^power + 1)`. With `power = 1` and `c → 0` it reduces to
/// identity; large `c` produces strong Möbius distortion.
///
/// # Authors
/// - Zueuk

// Murl, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    c: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Möbius coefficient. Rescaled internally by `1/(power−1)` when `power ≠ 1`.",
    },
    power: #{
        value: 1, min: -50, max: 50,
        caption: "Power",
        hint: "Complex power applied to the input. Integer values produce rotational symmetries; `power = 1` reduces to a simpler Möbius warp.",
    },
};

const INFO = #{ title: "Murl", name: "rhai_murl", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn prepare(p) {
    let c_in = p.c;
    let power = to_float(p.power);
    let c = c_in;
    if (power != 1.0) {
        let pm1 = power - 1.0;
        let safe_pm1 = (if abs(pm1) < 1.0e-30 { 1.0e-30 } else { pm1 });
        c = c / safe_pm1;
    }
    #{ d0: c, p2: power * 0.5, vp: c + 1.0 }
}

fn variation(z, p) {
    let power = to_float(p.power);
    let c = p.d0;
    let p2 = p.p2;
    let vp = p.vp;
    let a = atan2(z.im, z.re) * power;
    let r = c * pow(max(1.0e-30, z.re * z.re + z.im * z.im), p2);
    let re = r * cos(a) + 1.0;
    let im = r * sin(a);
    let rl = vp / max(1.0e-30, re * re + im * im);
    complex(rl * (z.re * re + z.im * im), rl * (z.im * re - z.re * im))
}

fn variation3(v, p) {
    let power = to_float(p.power);
    let c = p.d0;
    let p2 = p.p2;
    let vp = p.vp;
    let a = atan2(v.y, v.x) * power;
    let r = c * pow(max(1.0e-30, v.x * v.x + v.y * v.y), p2);
    let re = r * cos(a) + 1.0;
    let im = r * sin(a);
    let rl = vp / max(1.0e-30, re * re + im * im);
    vec3(rl * (v.x * re + v.y * im), rl * (v.y * re - v.x * im), v.z)
}

/// 3D extension of [`JULIASCOPE`] with Brad Stefanov's added Z-warp
/// controls. Inherits the kaleidoscope's mirror-branch dispatch
/// (random branch index ∈ [0, |power|), sign of the angle flipped
/// on odd branches) but layers a configurable Z output on top:
///   - `za`, `zb` rescale X² and Y² in the 2D radius term;
///   - `warp` offsets the input Z before participating;
///   - `zamount`, `zdist` scale the radial Z power (`pow(r2d + z²,
///     cPower × zdist)`);
///   - `mode = 1` multiplies the Z output by `sin(wave1) × cos(wave2)`
///     for a per-iteration depth ripple;
///   - `type = 1` switches the XY radial power to use the full 3D
///     length `sqrt(x²+y²+z²)` instead of just `sqrt(x²+y²)`.
///
/// `power` is intentionally unconstrained; values close to 0 NaN
/// the trig branch, so the body guards with `abs_power = max(|int
/// power|, 1)` for the random pick and an `|power| < EPS` early-out.
///
/// # Authors
/// - Brad Stefanov
/// - Scott Draves

// JuliaScope 3Db, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 3, min: -20, max: 20, free: true,
        caption: "Power",
        hint: "Number of mirror branches. Higher = more reflections; negative values invert the rotation. `0` no-ops the variation to avoid divide-by-zero in the angle dispatch.",
    },
    dist: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Distance",
        hint: "Radial scaling factor. Combined with `power` into `cPower = dist / power × 0.5`, which sets the exponent on the radial term.",
    },
    type: #{
        value: false,
        caption: "Type",
        hint: "When on, the XY radial term uses the full 3D length `sqrt(x²+y²+z²)` instead of just the 2D `sqrt(x²+y²)`. Affects the XY scale only; Z output is unchanged.",
    },
    warp: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Warp",
        hint: "Pre-offset added to the input Z (after dividing by `abs(power)`) before it participates in the Z radial term and output multiplier.",
    },
    za: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "za",
        hint: "X² scale in the 2D radius `r2d = x²·za + y²·zb` used by the Z radial term.",
    },
    zb: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "zb",
        hint: "Y² scale in the 2D radius (see `za`).",
    },
    zamount: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Z Amount",
        hint: "Outer multiplier on the Z output — scales `rz` directly.",
    },
    zdist: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Z Distance",
        hint: "Exponent multiplier on the Z radial term — `rz = zamount × pow(r2d + z², cPower × zdist)`.",
    },
    mode: #{
        value: false,
        caption: "Mode",
        hint: "When on, multiplies the Z output by `sin(wave1) × cos(wave2)`. Lets `wave1` / `wave2` modulate the depth contribution per iteration.",
    },
    wave1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Wave 1",
        hint: "Argument to `sin()` in the mode-on Z multiplier. Ignored when `mode` is off.",
    },
    wave2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Wave 2",
        hint: "Argument to `cos()` in the mode-on Z multiplier. Ignored when `mode` is off.",
    },
};

const INFO = #{ title: "JuliaScope 3Db", name: "rhai_juliascope3db", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let PI = 3.14159265359;
    let power = to_float(p.power);
    let dist = p.dist;
    if (abs(power) < 1.0e-6) {
        return complex(0.0, 0.0);
    }
    let c_power = dist / power * 0.5;
    let abs_power = max(to_int(abs(power)), 1);
    let rnd = to_int(rand() * to_float(abs_power));
    let theta = atan2(z.im, z.re);
    let a = (if (rnd & 1) == 0 { (2.0 * PI() * to_float(rnd) + theta) / power } else { (2.0 * PI() * to_float(rnd) - theta) / power });
    let r2 = z.re * z.re + z.im * z.im;
    let r = pow(r2, c_power);
    complex(r * cos(a), r * sin(a))
}

fn variation3(v, p) {
    let PI = 3.14159265359;
    let power = to_float(p.power);
    let dist = p.dist;
    let type_ = to_int(to_float(p.type));
    let warp = p.warp;
    let za = p.za;
    let zb = p.zb;
    let zamount = p.zamount;
    let zdist = p.zdist;
    let mode = to_int(to_float(p.mode));
    let wave1 = p.wave1;
    let wave2 = p.wave2;
    if (abs(power) < 1.0e-6) {
        return vec3(0.0, 0.0, 0.0);
    }
    let c_power = dist / power * 0.5;
    let abs_power = max(to_int(abs(power)), 1);
    let abs_power_f = to_float(abs_power);
    let z = v.z / abs_power_f + warp;
    let r2d = v.x * v.x * za + v.y * v.y * zb;
    let rz = zamount * pow(max(1.0e-30, r2d + z * z), c_power * zdist);
    let rnd = to_int(rand() * abs_power_f);
    let theta = atan2(v.y, v.x);
    let a = (if (rnd & 1) == 0 { (2.0 * PI() * to_float(rnd) + theta) / power } else { (2.0 * PI() * to_float(rnd) - theta) / power });
    let xy2 = v.x * v.x + v.y * v.y;
    let r2 = (if type_ != 0 { xy2 + v.z * v.z } else { xy2 });
    let r = pow(max(1.0e-30, r2), c_power);
    let z_factor = (if mode != 0 { sin(wave1) * cos(wave2) } else { 1.0 });
    vec3(r * cos(a), r * sin(a), rz * z * z_factor)
}

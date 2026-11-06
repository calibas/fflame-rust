/// Post-phase version of Julia3DQ — applies the 3D rational-power Julia
/// branching after all other variations have run.
///
/// # Authors
/// - Zueuk

// Post Julia3DQ, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 3, min: -50, max: 50,
        caption: "Power",
        hint: "Number of Julia branches.",
    },
    divisor: #{
        value: 2, min: -50, max: 50,
        caption: "Divisor",
        hint: "Rational-power divisor.",
    },
};

const INFO = #{ title: "Post Julia3DQ", name: "rhai_post_julia3dq", category: "full_3d", phase: "post" };

fn prepare(p) {
    let power = to_float(p.power);
    let divisor = to_float(p.divisor);
    let safe_power = (if power == 0.0 { 1.0e-30 } else { power });
    let inv_power = divisor / safe_power;
    let two_pi = 6.28318530717959;
    #{ inv_power: inv_power, abs_inv_power: abs(inv_power), half_inv_power: 0.5 * inv_power - 0.5, inv_power_2pi: two_pi / safe_power }
}

fn variation(z, p, ctx) {
    let power = to_float(p.power);
    let inv_power = p.inv_power;
    let half_inv_power = p.half_inv_power;
    let inv_power_2pi = p.inv_power_2pi;
    let w = ctx.weight;
    let n = to_float(floor(rand() * power));
    let a = atan2(z.im, z.re) * inv_power + n * inv_power_2pi;
    let r2d = z.re * z.re + z.im * z.im;
    let r = w * pow(max(1.0e-30, r2d), half_inv_power);
    r = r * sqrt(r2d);
    complex(r * cos(a), r * sin(a))
}

fn variation3(v, p, ctx) {
    let power = to_float(p.power);
    let inv_power = p.inv_power;
    let abs_inv_power = p.abs_inv_power;
    let half_inv_power = p.half_inv_power;
    let inv_power_2pi = p.inv_power_2pi;
    let w = ctx.weight;
    let n = to_float(floor(rand() * power));
    let a = atan2(v.y, v.x) * inv_power + n * inv_power_2pi;
    let z = v.z * abs_inv_power;
    let r2d = v.x * v.x + v.y * v.y;
    let r = w * pow(max(1.0e-30, r2d + z * z), half_inv_power);
    let out_z = r * z;
    r = r * sqrt(r2d);
    vec3(r * cos(a), r * sin(a), out_z)
}

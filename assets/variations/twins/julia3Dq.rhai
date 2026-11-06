/// 3D version of JuliaQ — extends the rational-power Julia into the Z axis.
///
/// # Authors
/// - Zueuk

// Julia 3D Q, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 3, min: 1, max: 64,
        caption: "Power",
        hint: "Number of Julia branches.",
    },
    divisor: #{
        value: 2, min: 1, max: 64,
        caption: "Divisor",
        hint: "Rational-power divisor.",
    },
};

const INFO = #{ title: "Julia 3D Q", name: "rhai_julia3dq", category: "full_3d", phase: "any" };

fn prepare(p) {
    let power = max(1.0, to_float(p.power));
    let divisor = to_float(p.divisor);
    let inv_power = divisor / power;
    #{ inv_power: inv_power, inv_power_2pi: 6.28318530717959 / power, half_inv_power: 0.5 * inv_power - 0.5, abs_inv_power: abs(inv_power) }
}

fn julia3dq_branch(power_param) {
    let m = max(1.0, power_param);
    let n = rand_u32() & u32(0x7FFFFFFF);
    let n_hi = to_float(n >> u32(16));
    let n_lo = to_float(n & u32(0xFFFF));
    let r16 = 65536.0 - m * to_float(floor(65536.0 / m));
    let t1 = n_hi * r16;
    let t2 = t1 - m * to_float(floor(t1 / m));
    let t3 = n_lo - m * to_float(floor(n_lo / m));
    let s = t2 + t3;
    s - m * to_float(floor(s / m))
}

fn variation(z, p) {
    let power_param = to_float(p.power);
    let inv_power = p.inv_power;
    let inv_power_2pi = p.inv_power_2pi;
    let half_inv_power = p.half_inv_power;
    let a = atan2(z.im, z.re) * inv_power + julia3dq_branch(power_param) * inv_power_2pi;
    let r2d = max(1.0e-30, z.re * z.re + z.im * z.im);
    let r_pow = pow(r2d, half_inv_power);
    let r = r_pow * sqrt(r2d);
    complex(r * cos(a), r * sin(a))
}

fn variation3(v, p) {
    let power_param = to_float(p.power);
    let inv_power = p.inv_power;
    let inv_power_2pi = p.inv_power_2pi;
    let half_inv_power = p.half_inv_power;
    let abs_inv_power = p.abs_inv_power;
    let a = atan2(v.y, v.x) * inv_power + julia3dq_branch(power_param) * inv_power_2pi;
    let z_arg = v.z * abs_inv_power;
    let r2d = max(1.0e-30, v.x * v.x + v.y * v.y);
    let r_pow = pow(max(1.0e-30, r2d + z_arg * z_arg), half_inv_power);
    let r = r_pow * sqrt(r2d);
    vec3(r * cos(a), r * sin(a), r_pow * z_arg)
}

// Julia3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 2, min: -10, max: 10, free: true,
        caption: "Power",
        hint: "Number of branches in the 3D Julia output. Higher = more arms; negative values flip the rotation.",
    },
};

const INFO = #{ title: "Julia3D", name: "rhai_julia3d" };

fn variation(z, p) {
    let power_f = to_float(p.power);
    let N = to_int(power_f);
    if (N == 0) {
        return z;
    }
    let absN_f = to_float(abs(N));
    let r2d = z.norm_sqr();
    let cN = (1.0 / power_f - 1.0) * 0.5;
    let r = pow(r2d, cN);
    let random_idx = to_int(rand() * absN_f);
    let angle = (atan2(z.im, z.re) + 6.28318530718 * to_float(random_idx)) / power_f;
    let tmp = r * sqrt(r2d);
    complex(tmp * cos(angle), tmp * sin(angle))
}

fn variation3(v, p) {
    let power_f = to_float(p.power);
    let N = to_int(power_f);
    if (N == 0) {
        return v;
    }
    let absN_f = to_float(abs(N));
    let z = v.z / absN_f;
    let r2d = complex(v.x, v.y).norm_sqr();
    let cN = (1.0 / power_f - 1.0) * 0.5;
    let r = pow(r2d + z * z, cN);
    let random_idx = to_int(rand() * absN_f);
    let angle = (atan2(v.y, v.x) + 6.28318530718 * to_float(random_idx)) / power_f;
    let tmp = r * sqrt(r2d);
    vec3(tmp * cos(angle), tmp * sin(angle), r * z)
}

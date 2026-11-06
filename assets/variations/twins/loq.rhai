// Loq, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    base: #{
        value: 2.7182817, min: 1.01, max: 100.0, free: true,
        caption: "Base",
        hint: "Log base for the X-output normalization. Defaults to e.",
    },
};

const INFO = #{ title: "Loq", name: "rhai_loq" };

fn variation(z, p) {
    let base = max(p.base, 1.000001);
    let abs_v = max(abs(z.im), 1.0e-30);
    let denom = 0.5 / ln(base);
    let c_val = atan2(abs_v, z.re) / abs_v;
    let log_arg = max(z.re * z.re + abs_v * abs_v, 1.0e-30);
    let fx = ln(log_arg) * denom;
    let fy = c_val * z.im;
    complex(fx, fy)
}

fn variation3(v, p) {
    let base = max(p.base, 1.000001);
    let abs_v = max(sqrt(v.y * v.y + v.z * v.z), 1.0e-30);
    let denom = 0.5 / ln(base);
    let c_val = atan2(abs_v, v.x) / abs_v;
    let log_arg = max(v.x * v.x + abs_v * abs_v, 1.0e-30);
    let fx = ln(log_arg) * denom;
    let fy = c_val * v.y;
    let fz = c_val * v.z;
    vec3(fx, fy, fz)
}

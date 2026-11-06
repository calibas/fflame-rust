// Julia3Dz, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 2, min: -20, max: 20, free: true,
        caption: "Power",
        hint: "Number of Julia branches in the 3D output. Higher = more arms.",
    },
};

const INFO = #{ title: "Julia3Dz", name: "rhai_julia3dz" };

fn variation(z, p) {
    complex(0.0, 0.0)
}

fn variation3(v, p) {
    let PI = 3.14159265359;
    let power = to_int(to_float(p.power));
    let abs_power = abs(power);
    let power_f = to_float(power);
    let r2d = v.x * v.x + v.y * v.y;
    if (power == 1) {
        return v;
    }
    if (power == -1) {
        let r = 1.0 / r2d;
        return vec3(r * v.x, -r * v.y, r * v.z);
    }
    let rnd_int = to_int(rand() * to_float(abs_power));
    let angle = (atan2(v.y, v.x) + 2.0 * PI() * to_float(rnd_int)) / power_f;
    if (power == 2) {
        let r2d_sqrt = sqrt(r2d);
        let r = sqrt(r2d_sqrt);
        let z_out = r * v.z / r2d_sqrt / 2.0;
        return vec3(r * cos(angle), r * sin(angle), z_out);
    }
    if (power == -2) {
        let r2d_sqrt = sqrt(r2d);
        let r = 1.0 / sqrt(r2d_sqrt);
        let z_out = r * v.z / r2d_sqrt / 2.0;
        return vec3(r * cos(angle), -r * sin(angle), z_out);
    }
    let cN = 1.0 / power_f / 2.0;
    let r = pow(r2d, cN);
    let r2d_sqrt = sqrt(r2d);
    let z_out = r * v.z / (r2d_sqrt * to_float(abs_power));
    vec3(r * cos(angle), r * sin(angle), z_out)
}

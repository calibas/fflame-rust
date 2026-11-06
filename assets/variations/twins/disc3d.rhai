// Disc 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    pi: #{
        value: 3.1415927, min: -10.0, max: 10.0, free: true,
        caption: "Pi",
        hint: "Phase constant — defaults to π. Tweaking it warps the disc shape.",
    },
};

const INFO = #{ title: "Disc 3D", name: "rhai_disc3d" };

fn variation(z, p) {
    let pi_p = p.pi;
    let r = sqrt(z.re * z.re + z.im * z.im + 1.0e-30);
    let vv = atan2(z.re, z.im) / (pi_p + 1.0e-30);
    complex(vv * sin(pi_p * r), vv * cos(pi_p * r))
}

fn variation3(v, p) {
    let pi_p = p.pi;
    let r = sqrt(v.x * v.x + v.y * v.y + 1.0e-30);
    let vv = atan2(v.x, v.y) / (pi_p + 1.0e-30);
    vec3(vv * sin(pi_p * r), vv * cos(pi_p * r), vv * (r * cos(v.z)))
}

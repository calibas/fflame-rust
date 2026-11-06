// Ovoid 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X scale",
        hint: "X-axis scale on the inverted output.",
    },
    y: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y scale",
        hint: "Y-axis scale.",
    },
    z: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Z scale",
        hint: "Z-axis scale (3D only).",
    },
};

const INFO = #{ title: "Ovoid 3D", name: "rhai_ovoid3d" };

fn variation(z, p) {
    let x_scale = p.x;
    let y_scale = p.y;
    let t = max(z.re * z.re + z.im * z.im, 1.0e-30);
    let r = 1.0 / t;
    complex(z.re * r * x_scale, z.im * r * y_scale)
}

fn variation3(v, p) {
    let x_scale = p.x;
    let y_scale = p.y;
    let z_scale = p.z;
    let t = max(v.x * v.x + v.y * v.y + v.z * v.z, 1.0e-30);
    let r = 1.0 / t;
    vec3(v.x * r * x_scale, v.y * r * y_scale, v.z * r * z_scale)
}

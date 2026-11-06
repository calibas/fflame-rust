/// Parameterized 3D bubble with separate X / Y / Z scaling. Maps the input
/// onto a sphere and stretches each axis independently.
///
/// # Authors
/// - FracFx

// Bubble2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X",
        hint: "X-axis scaling of the sphere projection.",
    },
    y: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y",
        hint: "Y-axis scaling.",
    },
    z: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Z",
        hint: "Z-axis displacement plus scaling.",
    },
};

const INFO = #{ title: "Bubble2", name: "rhai_bubble2", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let x_p = p.x;
    let y_p = p.y;
    let t = (z.re * z.re + z.im * z.im) / 4.0 + 1.0;
    let r = 1.0 / t;
    complex(z.re * r * x_p, z.im * r * y_p)
}

fn variation3(v, p) {
    let x_p = p.x;
    let y_p = p.y;
    let z_p = p.z;
    let t = (v.x * v.x + v.y * v.y + v.z * v.z) / 4.0 + 1.0;
    let r = 1.0 / t;
    let z_sign = (if v.z >= 0.0 { 1.0 } else { -1.0 });
    let z_out = (v.z + z_sign * z_p) + v.z * r * z_p;
    vec3(v.x * r * x_p, v.y * r * y_p, z_out)
}

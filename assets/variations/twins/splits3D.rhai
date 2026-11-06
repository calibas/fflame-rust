// Splits 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "X",
        hint: "How far positive-X and negative-X points get pushed apart along X.",
    },
    y: #{
        value: 0.3, min: -10.0, max: 10.0, free: true,
        caption: "Y",
        hint: "How far positive-Y and negative-Y points get pushed apart along Y.",
    },
    z: #{
        value: 0.2, min: -10.0, max: 10.0, free: true,
        caption: "Z",
        hint: "How far positive-Z and negative-Z points get pushed apart along Z.",
    },
};

const INFO = #{ title: "Splits 3D", name: "rhai_splits3d" };

fn variation(z, p) {
    let x_p = p.x;
    let y_p = p.y;
    let xs = (if z.re >= 0.0 { x_p } else { -x_p });
    let ys = (if z.im >= 0.0 { y_p } else { -y_p });
    complex(z.re + xs, z.im + ys)
}

fn variation3(v, p) {
    let x_p = p.x;
    let y_p = p.y;
    let z_p = p.z;
    let xs = (if v.x >= 0.0 { x_p } else { -x_p });
    let ys = (if v.y >= 0.0 { y_p } else { -y_p });
    let zs = (if v.z >= 0.0 { z_p } else { -z_p });
    vec3(v.x + xs, v.y + ys, v.z + zs)
}

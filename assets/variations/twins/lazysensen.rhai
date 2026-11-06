// Lazy Sensen, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    scale_x: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale X",
        hint: "X-axis stripe scale. 0 disables the X flip; the stripe width is `1/|scale_x|`.",
    },
    scale_y: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale Y",
        hint: "Y-axis stripe scale.",
    },
    scale_z: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale Z",
        hint: "Z-axis stripe scale (3D only).",
    },
};

const INFO = #{ title: "Lazy Sensen", name: "rhai_lazysensen" };

fn lazysensen_flip(c, scale) {
    if (abs(scale) < 1.0e-30) {
        return c;
    }
    let nr = to_float(floor(c * scale));
    let nri = to_int(nr);
    let parity = nri - (nri / 2) * 2;
    if (nri >= 0) {
        if (parity == 1 || parity == -1) {
            return -c;
        }
    } else if (parity == 0) {
        return -c;
    }
    c
}

fn variation(z, p) {
    let sx = p.scale_x;
    let sy = p.scale_y;
    complex(lazysensen_flip(z.re, sx), lazysensen_flip(z.im, sy))
}

fn variation3(v, p) {
    let sx = p.scale_x;
    let sy = p.scale_y;
    let sz = p.scale_z;
    vec3(lazysensen_flip(v.x, sx), lazysensen_flip(v.y, sy), lazysensen_flip(v.z, sz))
}

// SVF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    n: #{
        value: 2.0, min: -50.0, max: 50.0, free: true,
        caption: "N",
        hint: "Frequency multiplier on Y in the inner `cos(n·y)` term.",
    },
};

const INFO = #{ title: "SVF", name: "rhai_svf" };

fn variation(z, p) {
    let n = p.n;
    let cn = cos(n * z.im);
    let sx = sin(z.re);
    let cx = cos(z.re);
    let cy = cos(z.im);
    complex(cy * cn * cx, cy * cn * sx)
}

fn variation3(v, p) {
    let n = p.n;
    let cn = cos(n * v.y);
    let sx = sin(v.x);
    let cx = cos(v.x);
    let sy = sin(v.y);
    let cy = cos(v.y);
    vec3(cy * cn * cx, cy * cn * sx, sy * cn)
}

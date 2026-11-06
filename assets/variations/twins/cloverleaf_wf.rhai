// Cloverleaf WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    filled: #{
        value: true,
        caption: "Filled",
        hint: "1 = fill the curve interior by randomizing the radius per iteration; 0 = trace only the curve outline.",
    },
};

const INFO = #{ title: "Cloverleaf WF", name: "rhai_cloverleaf_wf" };

fn variation(z, p) {
    let filled = to_int(to_float(p.filled));
    let a = atan2(z.re, z.im);
    let r = sin(2.0 * a) + 0.25 * sin(6.0 * a);
    if (filled == 1) {
        r = r * rand();
    }
    complex(sin(a) * r, cos(a) * r)
}

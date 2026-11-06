// Cannabis Curve WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    filled: #{
        value: true,
        caption: "Filled",
        hint: "When on, fill the curve interior by randomizing the radius per iteration. When off, trace only the outline.",
    },
};

const INFO = #{ title: "Cannabis Curve WF", name: "rhai_cannabiscurve_wf" };

fn variation(z, p) {
    let filled = to_int(to_float(p.filled));
    let a = atan2(z.re, z.im);
    let r = (1.0 + 0.9 * cos(8.0 * a)) * (1.0 + 0.1 * cos(24.0 * a)) * (0.9 + 0.1 * cos(200.0 * a)) * (1.0 + sin(a));
    a = a + 1.5707963267948966;
    if (filled == 1) {
        r = r * rand();
    }
    complex(sin(a) * r, cos(a) * r)
}

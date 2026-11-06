// Bent 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X scale",
        hint: "X-axis scale applied where x < 0. Positive x passes through unchanged.",
    },
    y: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y scale",
        hint: "Y-axis scale applied where y < 0. Positive y passes through unchanged.",
    },
};

const INFO = #{ title: "Bent 2", name: "rhai_bent2" };

fn variation(z, p) {
    let xs = p.x;
    let ys = p.y;
    let nx = (if z.re < 0.0 { z.re * xs } else { z.re });
    let ny = (if z.im < 0.0 { z.im * ys } else { z.im });
    complex(nx, ny)
}

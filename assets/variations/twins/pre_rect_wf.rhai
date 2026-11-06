/// Pre-phase uniform rectangle sample — replaces the input with a random
/// point uniformly sampled from the rectangle `[x0, x1] × [y0, y1]`, scaled
/// by the variation weight. Discards the input position entirely.

// Pre Rect WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x0: #{
        value: -0.5, min: -10.0, max: 10.0, free: true,
        caption: "X0",
        hint: "Left edge of the sampling rectangle.",
    },
    x1: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "X1",
        hint: "Right edge of the sampling rectangle.",
    },
    y0: #{
        value: -0.5, min: -10.0, max: 10.0, free: true,
        caption: "Y0",
        hint: "Bottom edge of the sampling rectangle.",
    },
    y1: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Y1",
        hint: "Top edge of the sampling rectangle.",
    },
};

const INFO = #{ title: "Pre Rect WF", name: "rhai_pre_rect_wf", category: "advanced_2d", phase: "pre" };

fn prepare(p) {
    #{ dx: p.x1 - p.x0, dy: p.y1 - p.y0 }
}

fn variation(z, p, ctx) {
    let x0 = p.x0;
    let y0 = p.y0;
    let dx = p.dx;
    let dy = p.dy;
    let w = ctx.weight;
    complex(w * (x0 + dx * rand()), w * (y0 + dy * rand()))
}

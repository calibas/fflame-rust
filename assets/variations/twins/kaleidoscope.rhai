// Kaleidoscope, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    pull: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Pull",
        hint: "Y-axis pull strength — pulls the upper and lower halves apart.",
    },
    rotate: #{
        value: 1.0, min: -5.0, max: 5.0, free: true,
        caption: "Rotate",
        hint: "Rotation scaling on both axes.",
    },
    line_up: #{
        value: 1.0, min: -5.0, max: 5.0, free: true,
        caption: "Line Up",
        hint: "Linear offset along the mirror line.",
    },
    x: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "X",
        hint: "Additional X offset.",
    },
    y: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Y",
        hint: "Additional Y offset for the upper half.",
    },
};

const INFO = #{ title: "Kaleidoscope", name: "rhai_kaleidoscope" };

fn variation(z, p, ctx) {
    let pull = p.pull;
    let rotate = p.rotate;
    let line_up = p.line_up;
    let x_off = p.x;
    let y_off = p.y;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let c45 = cos(45.0);
    let s45 = sin(45.0);
    let fx = (rotate * z.re) * c45 - z.im * s45 + line_up + x_off;
    let fy = 0.0;
    if (z.im > 0.0) {
        fy = (rotate * z.im) * c45 + z.re * s45 + pull + line_up + y_off;
    } else {
        fy = (rotate * z.im) * c45 + z.re * s45 - pull - line_up;
    }
    complex(fx * inv_w, fy * inv_w)
}

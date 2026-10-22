// The built-in Burning Ship and its family, as a formula of your own
// (the Escape panel's Edit as Formula): the quadratic step with each
// variant's absolute values placed around it, as the perpendicular
// family's chart has them. Deep zoom follows every variant.

const PARAMS = #{
    variant: #{
        value: 0,
        caption: "Variant",
        hint: "0 Burning Ship, 1 Perpendicular Mandelbrot, 2 Perpendicular Ship, 3 Celtic, 4 Buffalo, 5 Perpendicular Celtic.",
        choices: ["Burning Ship", "Perpendicular Mandelbrot", "Perpendicular Ship", "Celtic", "Buffalo", "Perpendicular Celtic"],
    },
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Burning Ship", center: complex(-0.5, 0.5), maxiter: 256 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: complex(0.0, 0.0), c: pixel }
    }
}

fn step(s, p) {
    let x = s.z.re;
    let y = s.z.im;
    let w = if p.variant == 0 {
        // Burning Ship: both parts folded, then squared.
        let ax = x.abs();
        let ay = y.abs();
        complex(ax * ax - ay * ay, 2.0 * ax * ay)
    } else if p.variant == 1 {
        // Perpendicular Mandelbrot
        complex(x * x - y * y, -2.0 * x.abs() * y)
    } else if p.variant == 2 {
        // Perpendicular Burning Ship
        complex(x * x - y * y, -2.0 * x * y.abs())
    } else if p.variant == 3 {
        // Celtic
        complex((x * x - y * y).abs(), 2.0 * x * y)
    } else if p.variant == 4 {
        // Buffalo
        complex((x * x - y * y).abs(), -2.0 * (x * y).abs())
    } else {
        // Perpendicular Celtic
        complex((x * x - y * y).abs(), -2.0 * x.abs() * y)
    };
    s.z = w + s.c;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}

fn degree(p) {
    2.0
}

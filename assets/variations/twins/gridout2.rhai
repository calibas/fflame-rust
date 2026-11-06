/// 8-cell quadrant routing — rounds the input to a grid (with separate cell
/// sizes `c, d` per axis), then chooses one of 8 octant-style cell-boundary
/// directions to add `±a` to X or `±b` to Y. Produces a grid-tile shifted
/// pattern.
///
/// # Authors
/// - Michael Faber
/// - Joel Faber
/// - Brad Stefanov
/// - DarkBeam

// Grid Out 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "X-axis shift magnitude.",
    },
    b: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Y-axis shift magnitude.",
    },
    c: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "X-axis cell size (multiplied with `round(x)` before the octant routing).",
    },
    d: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "Y-axis cell size (multiplied with `round(y)`).",
    },
};

const INFO = #{ title: "Grid Out 2", name: "rhai_gridout2", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let x = round_even(z.re) * c;
    let y = round_even(z.im) * d;
    let fx = 0.0;
    let fy = 0.0;
    if (y <= 0.0) {
        if (x > 0.0) {
            if (-y >= x) {
                fx = z.re + a;
                fy = z.im;
            } else {
                fx = z.re;
                fy = z.im + b;
            }
        } else if (y <= x) {
            fx = z.re + a;
            fy = z.im;
        } else {
            fx = z.re;
            fy = z.im - b;
        }
    } else if (x > 0.0) {
        if (y >= x) {
            fx = z.re - a;
            fy = z.im;
        } else {
            fx = z.re;
            fy = z.im + b;
        }
    } else if (y > -x) {
        fx = z.re - a;
        fy = z.im;
    } else {
        fx = z.re;
        fy = z.im - b;
    }
    complex(fx, fy)
}

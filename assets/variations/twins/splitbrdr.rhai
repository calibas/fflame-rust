// Split Brdr, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 0.25, min: -5.0, max: 5.0, free: true,
        caption: "X",
        hint: "Border push offset in one direction.",
    },
    y: #{
        value: 0.25, min: -5.0, max: 5.0, free: true,
        caption: "Y",
        hint: "Border push offset in the other direction.",
    },
    px: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "PX",
        hint: "Linear pass-through scaling for X — adds a fraction of the input X to the output.",
    },
    py: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "PY",
        hint: "Linear pass-through scaling for Y.",
    },
};

const INFO = #{ title: "Split Brdr", name: "rhai_splitbrdr" };

fn variation(z, p, ctx) {
    let xp = p.x;
    let yp = p.y;
    let px = p.px;
    let py = p.py;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let b = w / max(0.25 * (z.re * z.re + z.im * z.im) + 1.0, 1.0e-30);
    let fpx = z.re * b;
    let fpy = z.im * b;
    let round_x = round_even(z.re);
    let round_y = round_even(z.im);
    let off_x = z.re - round_x;
    let off_y = z.im - round_y;
    let bx = 0.0;
    let by = 0.0;
    if (rand() >= 0.75) {
        bx = off_x * 0.5 + round_x;
        by = off_y * 0.5 + round_y;
    } else if (abs(off_x) >= abs(off_y)) {
        if (off_x >= 0.0) {
            bx = off_x * 0.5 + round_x + xp;
            by = off_y * 0.5 + round_y + yp * off_y / off_x;
        } else {
            bx = off_x * 0.5 + round_x - yp;
            by = off_y * 0.5 + round_y - yp * off_y / off_x;
        }
    } else if (off_y >= 0.0) {
        bx = off_x * 0.5 + round_x + off_x / off_y * yp;
        by = off_y * 0.5 + round_y + yp;
    } else {
        bx = off_x * 0.5 + round_x - off_x / off_y * xp;
        by = off_y * 0.5 + round_y - yp;
    }
    fpx = fpx + w * bx;
    fpy = fpy + w * by;
    fpx = fpx + z.re * px;
    fpy = fpy + z.im * py;
    complex(fpx * inv_w, fpy * inv_w)
}

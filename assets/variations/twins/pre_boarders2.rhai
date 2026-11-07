/// Pre-phase version of Boarders 2 — same border-warp math but applied
/// before the rest of the variations run.
///
/// # Authors
/// - Xyrus02

// Pre Boarders 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    c: #{
        value: 0.4, min: -5.0, max: 5.0, free: true,
        caption: "C",
        hint: "Cell scale factor — how much the in-cell offset shrinks toward the cell center.",
    },
    left: #{
        value: 0.65, min: -5.0, max: 5.0, free: true,
        caption: "Left",
        hint: "Border push-out distance — how far points get pushed toward the nearest cell border.",
    },
    right: #{
        value: 0.35, min: -5.0, max: 5.0, free: true,
        caption: "Right",
        hint: "Threshold for border behavior vs pass-through. Higher = more points get pushed to borders.",
    },
};

const INFO = #{ title: "Pre Boarders 2", name: "rhai_pre_boarders2", category: "advanced_2d", phase: "pre" };

fn prepare(p) {
    let c0 = max(1.0e-6, abs(p.c));
    let cl0 = max(1.0e-6, abs(p.left));
    let cr0 = max(1.0e-6, abs(p.right));
    #{ d0: c0, cl: c0 * cl0, cr: c0 + c0 * cr0 }
}

fn variation(z, p, ctx) {
    let c = p.d0;
    let cl = p.cl;
    let cr = p.cr;
    let w = ctx.weight;
    let round_x = round_even(z.re);
    let round_y = round_even(z.im);
    let off_x = z.re - round_x;
    let off_y = z.im - round_y;
    let ox = 0.0;
    let oy = 0.0;
    if (rand() >= cr) {
        ox = off_x * c + round_x;
        oy = off_y * c + round_y;
    } else if (abs(off_x) >= abs(off_y)) {
        if (off_x >= 0.0) {
            ox = off_x * c + round_x + cl;
            oy = off_y * c + round_y + cl * off_y / off_x;
        } else {
            ox = off_x * c + round_x - cl;
            oy = off_y * c + round_y - cl * off_y / off_x;
        }
    } else if (off_y >= 0.0) {
        ox = off_x * c + round_x + off_x / off_y * cl;
        oy = off_y * c + round_y + cl;
    } else {
        ox = off_x * c + round_x - off_x / off_y * cl;
        oy = off_y * c + round_y - cl;
    }
    complex(w * ox, w * oy)
}

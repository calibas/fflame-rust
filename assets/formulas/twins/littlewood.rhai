// The built-in Littlewood, as a formula of your own (the Escape panel's
// Edit as Formula). The pixel is a lambda; the map w <- lambda w + d adds
// whichever digit d of the set leaves w smallest, and lambda is in the
// root cloud when w stays bounded.

const PARAMS = #{
    digit_set: #{
        value: 0,
        caption: "Digit set",
        hint: "0: {+1,-1} (Littlewood), 1: {0,+1,-1}, 2: {+1,-1,+i,-i}.",
        choices: ["{+1,-1} (Littlewood)", "{0,+1,-1}", "{+1,-1,+i,-i}"],
    },
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Littlewood", center: complex(0.0, 0.0), maxiter: 48 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: complex(1.0, 0.0), c: pixel }
    }
}

fn step(s, p) {
    let w = s.c * s.z;
    // The greedy choice: the digit nearest to cancelling lambda w, the
    // first of equals kept.
    let best = w + 1.0;
    let minus = w - 1.0;
    if minus.norm_sqr() < best.norm_sqr() {
        best = minus;
    }
    if p.digit_set >= 1 && w.norm_sqr() < best.norm_sqr() {
        // {0, ...}: keeping w is a choice.
        best = w;
    }
    if p.digit_set == 2 {
        let up = w + complex(0.0, 1.0);
        if up.norm_sqr() < best.norm_sqr() {
            best = up;
        }
        let down = w - complex(0.0, 1.0);
        if down.norm_sqr() < best.norm_sqr() {
            best = down;
        }
    }
    s.z = best;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}

// The built-in's smooth colourings divide by 2 (it declares no degree).
fn degree(p) {
    2.0
}

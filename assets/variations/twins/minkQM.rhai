/// Minkowski's question-mark function — applies the Stern-Brocot tree
/// iteration of Minkowski's `?(x)` to each axis separately. The function
/// maps rationals with simple continued-fraction expansions to dyadic
/// rationals, producing a self-similar staircase pattern. Parameters `a, b,
/// c, dd, e` seed the SB tree recursion; `f` sets the iteration count.
///
/// # Authors
/// - DarkBeam
/// - Brad Stefanov

// Mink QM, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Initial denominator `q` of the SB-tree recursion.",
    },
    b: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Initial offset on the SB-tree numerator `r`.",
    },
    c: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Initial denominator `s` of the SB-tree recursion.",
    },
    dd: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "Initial step size on the output `y`.",
    },
    e: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "E",
        hint: "Step decay factor — multiplied with `d` each iteration. 0.5 gives the standard Minkowski function.",
    },
    f: #{
        value: 20, min: 1, max: 50,
        caption: "Iters",
        hint: "Iteration count. Default 20 yields about 1e-6 precision.",
    },
};

const INFO = #{ title: "Mink QM", name: "rhai_minkqm", category: "advanced_2d", phase: "any" };

fn minkqm_minkowski(xv, a, b, c, dd, e, iters) {
    let p = 0.0;
    let q = a;
    let r = p + b;
    let s = c;
    let d = dd;
    let y = p;
    for it in 0..iters {
        d = d * e;
        let m = p + r;
        let n = q + s;
        let safe_n = (if abs(n) < 1.0e-30 { 1.0e-30 } else { n });
        if (xv < (m / safe_n)) {
            r = m;
            s = n;
        } else {
            y = y + d;
            p = m;
            q = n;
        }
    }
    y + d
}

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let dd = p.dd;
    let e = p.e;
    let iters = to_int(max(1.0, to_float(p.f)));
    let mnkX = z.re;
    let sx = 1.0;
    if (mnkX < 0.0) {
        mnkX = -mnkX;
        sx = -1.0;
    }
    if (mnkX > 0.0 && mnkX < 1.0) {
        mnkX = minkqm_minkowski(mnkX, a, b, c, dd, e, iters);
    }
    mnkX = sx * mnkX;
    let mnkY = z.im;
    let sy = 1.0;
    if (mnkY < 0.0) {
        mnkY = -mnkY;
        sy = -1.0;
    }
    if (mnkY > 0.0 && mnkY < 1.0) {
        mnkY = minkqm_minkowski(mnkY, a, b, c, dd, e, iters);
    }
    mnkY = sy * mnkY;
    complex(mnkX, mnkY)
}

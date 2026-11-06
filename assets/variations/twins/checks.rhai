/// Checkerboard cell-shift — divides space into a grid of cells of size
/// `size`, classifies each cell as odd/even, and applies a different per-
/// axis shift in each parity class. Optionally jitters one component of the
/// shift by `rnd`.
///
/// # Authors
/// - Keeps
/// - Xyrus02

// Checks, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "X",
        hint: "X-cell-shift magnitude.",
    },
    y: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Y",
        hint: "Y-cell-shift magnitude.",
    },
    size: #{
        value: 0.5, min: 0.001, max: 10.0, free: true,
        caption: "Size",
        hint: "Cell grid size.",
    },
    rnd: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Rnd",
        hint: "Random jitter magnitude on the cell shift.",
    },
};

const INFO = #{ title: "Checks", name: "rhai_checks", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ cs: 1.0 / (p.size + 1.0e-30) }
}

fn variation(z, p) {
    let cx = p.x;
    let cy = p.y;
    let rnd = p.rnd;
    let cs = p.cs;
    let isXY = to_int(round_even(z.re * cs)) + to_int(round_even(z.im * cs));
    let rnx = rnd * rand();
    let rny = rnd * rand();
    let dx = 0.0;
    let dy = 0.0;
    if ((isXY & 1) != 0) {
        dx = -cx + rnx;
        dy = -cy;
    } else {
        dx = cx;
        dy = cy + rny;
    }
    complex(z.re + dx, z.im + dy)
}

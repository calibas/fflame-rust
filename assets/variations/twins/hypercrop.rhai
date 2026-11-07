/// N-gon corner-cropping warp — snaps the input angle to the nearest n-gon
/// spoke, finds that spoke's corner, and tests whether the input lies
/// inside a small disc around the corner. Inside, behavior depends on
/// `zero`: snap to corner, collapse to origin, or scatter around the disc
/// edge. Outside, the point passes through.
///
/// # Authors
/// - Tatyana Zabanova
/// - Brad Stefanov

// Hypercrop, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    n: #{
        value: 4, min: 3, max: 50,
        caption: "N",
        hint: "Number of n-gon sides (≥ 3).",
    },
    rad: #{
        value: 1.0, min: 0.0, max: 10.0, free: true,
        caption: "Radius",
        hint: "Radius of the corner-cropping disc, relative to the n-gon corner radius.",
    },
    zero: #{
        value: 0.0, min: 0.0, max: 2.0, free: true,
        caption: "Zero",
        hint: "Behavior inside the corner disc. `> 1.5` snaps to the corner; `> 0.5` collapses to origin; else scatters around the disc edge.",
    },
};

const INFO = #{ title: "Hypercrop", name: "rhai_hypercrop", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let n = max(3.0, to_float(p.n));
    let rad = p.rad;
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let a0 = pi / n;
    let len = 1.0 / cos(a0);
    #{ coef: n / two_pi, a0: a0, len: len, d: rad * sin(a0) * len }
}

fn variation(z, p) {
    let zero = p.zero;
    let coef = p.coef;
    let a0 = p.a0;
    let len = p.len;
    let d = p.d;
    let angle = atan2(z.im, z.re);
    angle = to_float(floor(angle * coef)) / coef + a0;
    let x0 = cos(angle) * len;
    let y0 = sin(angle) * len;
    let dx = z.re - x0;
    let dy = z.im - y0;
    if (sqrt(dx * dx + dy * dy) < d) {
        if (zero > 1.5) {
            return complex(x0, y0);
        }
        if (zero > 0.5) {
            return complex(0.0, 0.0);
        }
        let rang = atan2(dy, dx);
        return complex(x0 + cos(rang) * d, y0 + sin(rang) * d);
    }
    z
}

fn variation3(v, p) {
    let zero = p.zero;
    let coef = p.coef;
    let a0 = p.a0;
    let len = p.len;
    let d = p.d;
    let angle = atan2(v.y, v.x);
    angle = to_float(floor(angle * coef)) / coef + a0;
    let x0 = cos(angle) * len;
    let y0 = sin(angle) * len;
    let dx = v.x - x0;
    let dy = v.y - y0;
    if (sqrt(dx * dx + dy * dy) < d) {
        if (zero > 1.5) {
            return vec3(x0, y0, 0.0);
        }
        if (zero > 0.5) {
            return vec3(0.0, 0.0, 0.0);
        }
        let rang = atan2(dy, dx);
        return vec3(x0 + cos(rang) * d, y0 + sin(rang) * d, 0.0);
    }
    v
}

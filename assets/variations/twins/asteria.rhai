/// RNG-blended linear/asteria warp — splits behavior based on whether the
/// input falls in a star-shaped region (inside the unit circle, with
/// corners cut out by unit-discs around the four corners `(±1, ±1)`).
/// Inside, linear pass-through dominates (with 35% chance of falling
/// through to the asteria branch). Outside, applies a square-root-bend warp
/// rotated by `alpha`.
///
/// # Authors
/// - DarkBeam

// Asteria, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    alpha: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Alpha",
        hint: "Rotation angle (× π) applied before and inverted after the asteria branch's square-root bend.",
    },
};

const INFO = #{ title: "Asteria", name: "rhai_asteria", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let pi = 3.14159265358979;
    #{ sina: sin(pi * p.alpha), cosa: cos(pi * p.alpha) }
}

fn variation(z, p, ctx) {
    let sina = p.sina;
    let cosa = p.cosa;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let x0 = w * z.re;
    let y0 = w * z.im;
    let r = x0 * x0 + y0 * y0;
    let xx = abs(x0) - 1.0;
    xx = xx * xx;
    let yy = abs(y0) - 1.0;
    yy = yy * yy;
    let r2 = sqrt(xx + yy);
    let in1 = r < 1.0;
    let out2 = r2 < 1.0;
    if (in1 && out2) {
        in1 = rand() > 0.35;
    } else {
        in1 = !in1;
    }
    let fx = 0.0;
    let fy = 0.0;
    if (in1) {
        fx = x0;
        fy = y0;
    } else {
        let xr = x0 * cosa - y0 * sina;
        let yr = x0 * sina + y0 * cosa;
        let inner = max(0.0, 1.0 - yr * yr);
        let safe_inner = (if inner < 1.0e-30 { 1.0e-30 } else { sqrt(inner) });
        let bend = 1.0 - sqrt(max(0.0, 1.0 - (1.0 - abs(yr)) * (1.0 - abs(yr))));
        let nx = xr / safe_inner * bend;
        fx = nx * cosa + yr * sina;
        fy = -nx * sina + yr * cosa;
    }
    complex(fx * inv_w, fy * inv_w)
}

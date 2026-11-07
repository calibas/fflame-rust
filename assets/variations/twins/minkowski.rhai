/// Minkowski spacetime: Lorentz boost / boost swirl / light-cone
/// inversion / Rindler unwrap on (xyz, w) with w timelike (2D: y
/// timelike), with causal coloring by the spacetime interval.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Minkowski, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 0, choices: ["Boost", "Boost Swirl", "Inversion", "Rindler Wrap"],
        caption: "Mode",
        hint: "Boost: a fixed Lorentz boost (rapidity along the boost axis) — the spacetime 'rotation' that mixes space with time and preserves the light cone. Boost Swirl: rapidity proportional to each point's own interval q — the Minkowski swirl, shearing every point along its own hyperbola shell (antisymmetric across the cone). Inversion: x/q, singular on the whole light cone — near-cone points flare outward. Rindler Wrap: Minkowski log-polar (rho = sqrt(|q|), eta = rapidity angle) — hyperbola shells flatten into strips; the spacelike and timelike wedges unwrap perpendicular to each other.",
    },
    rapidity: #{
        value: 0.5, min: -4.0, max: 4.0,
        caption: "Rapidity",
        hint: "Boost mode: the boost rapidity (hyperbolic angle — velocity = tanh(rapidity)). Also added as a constant pre-boost in the other modes, so any mode can be viewed from a moving frame.",
    },
    swirl: #{
        value: 1.0, min: -10.0, max: 10.0,
        caption: "Swirl",
        hint: "Boost Swirl mode: rapidity = Swirl x interval q, clamped to ±8. Positive shears future-ward on spacelike points and past-ward on timelike ones; negative reverses.",
    },
    bx: #{
        value: 1.0, min: -1.0, max: 1.0, free: true,
        caption: "Boost X",
        hint: "Boost axis x component (3D Boost mode; normalized internally, 2D boosts along x).",
    },
    by: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Boost Y",
        hint: "Boost axis y component.",
    },
    bz: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Boost Z",
        hint: "Boost axis z component.",
    },
    size: #{
        value: 1.0, min: 0.05, max: 4.0,
        caption: "Size",
        hint: "Output scale.",
    },
    guard: #{
        value: 0.05, min: 0.001, max: 1.0,
        caption: "Cone Guard",
        hint: "Inversion mode: floor on |q| — caps how hard near-cone points flare (the light cone is the singular set of x/q). Smaller = sharper, hotter cone flares.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Causal", "Interval"],
        caption: "Color Mode",
        hint: "Causal: palette position 0.5 + 0.5*tanh(q * DC Scale) — timelike interiors sit below palette center, spacelike outside above, and the light cone is exactly the center color: a visible causal boundary. Interval: cyclic bands by |q| (hyperbola shells become palette rings).",
    },
    dc_scale: #{
        value: 1.0, min: 0.0, max: 20.0,
        caption: "DC Scale",
        hint: "Causal: sharpness of the cone boundary. Interval: band frequency.",
    },
};

const INFO = #{ title: "Minkowski", name: "rhai_minkowski", category: "advanced_2d" };

fn variation(z, p, ctx) {
    let color = ctx.color;
    let mode = u32(to_float(p.mode));
    let rap = p.rapidity;
    let swirl = p.swirl;
    let size = p.size;
    let guard = max(1.0e-4, p.guard);
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let x = z.re;
    let t = z.im;
    let q = x * x - t * t;
    if (dc_mode == u32(1)) {
        color = 0.5 + 0.5 * tanh(q * dc_scale);
    } else if (dc_mode == u32(2)) {
        color = fract(abs(q) * dc_scale);
    }
    if (rap != 0.0) {
        let ch = cosh(rap);
        let sh = sinh(rap);
        let nx = x * ch + t * sh;
        t = x * sh + t * ch;
        x = nx;
    }
    if (mode == u32(1)) {
        let eta = clamp(swirl * q, -8.0, 8.0);
        let ch = cosh(eta);
        let sh = sinh(eta);
        let nx = x * ch + t * sh;
        t = x * sh + t * ch;
        x = nx;
    } else if (mode == u32(2)) {
        let qc = x * x - t * t;
        let dn = (if qc > 0.0 { 1.0 } else if qc < 0.0 { -1.0 } else { 0.0 }) * max(abs(qc), guard * guard);
        x = x / dn;
        t = t / dn;
    } else if (mode == u32(3)) {
        let qc = x * x - t * t;
        if (qc > 0.0) {
            let rho = sqrt(qc);
            let eta = atanh(clamp(t / x, -0.999999, 0.999999)) * (if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 });
            x = eta;
            t = clamp(ln(max(1.0e-9, rho)), -20.0, 20.0);
        } else if (qc < 0.0) {
            let rho = sqrt(-qc);
            let eta = atanh(clamp(x / t, -0.999999, 0.999999)) * (if t > 0.0 { 1.0 } else if t < 0.0 { -1.0 } else { 0.0 });
            x = clamp(ln(max(1.0e-9, rho)), -20.0, 20.0);
            t = eta;
        }
    }
    #{ z: complex(x, t) * size, color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let w_out = 0.0;
    let mode = u32(to_float(p.mode));
    let rap = p.rapidity;
    let swirl = p.swirl;
    let bx = p.bx;
    let by = p.by;
    let bz = p.bz;
    let size = p.size;
    let guard = max(1.0e-4, p.guard);
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let s = v;
    let t = ctx.w;
    let q = dot(s, s) - t * t;
    if (dc_mode == u32(1)) {
        color = 0.5 + 0.5 * tanh(q * dc_scale);
    } else if (dc_mode == u32(2)) {
        color = fract(abs(q) * dc_scale);
    }
    if (rap != 0.0) {
        let n = vec3(bx, by, bz);
        let nl = length(n);
        n = (if nl > 1.0e-6 { n / nl } else { vec3(1.0, 0.0, 0.0) });
        let par = dot(s, n);
        let ch = cosh(rap);
        let sh = sinh(rap);
        let np = par * ch + t * sh;
        t = par * sh + t * ch;
        s = s + (np - par) * n;
    }
    if (mode == u32(1)) {
        let r = length(s);
        if (r > 1.0e-9) {
            let n = s / r;
            let eta = clamp(swirl * q, -8.0, 8.0);
            let ch = cosh(eta);
            let sh = sinh(eta);
            let nr = r * ch + t * sh;
            t = r * sh + t * ch;
            s = n * nr;
        }
    } else if (mode == u32(2)) {
        let qc = dot(s, s) - t * t;
        let dn = (if qc > 0.0 { 1.0 } else if qc < 0.0 { -1.0 } else { 0.0 }) * max(abs(qc), guard * guard);
        s = s / dn;
        t = t / dn;
    } else if (mode == u32(3)) {
        let r = length(s);
        let qc = r * r - t * t;
        if (r > 1.0e-9 && qc > 0.0) {
            let n = s / r;
            let rho = sqrt(qc);
            let eta = atanh(clamp(t / r, -0.999999, 0.999999));
            s = n * clamp(ln(max(1.0e-9, rho)), -20.0, 20.0);
            t = eta;
        } else if (qc < 0.0 && abs(t) > 1.0e-9) {
            let rho = sqrt(-qc);
            let eta = atanh(clamp(r / abs(t), 0.0, 0.999999));
            let n = vec3(1.0, 0.0, 0.0);
            if (r > 1.0e-9) {
                n = s / r;
            }
            s = n * eta;
            t = (if t > 0.0 { 1.0 } else if t < 0.0 { -1.0 } else { 0.0 }) * clamp(ln(max(1.0e-9, rho)), -20.0, 20.0);
        }
    }
    w_out = t;
    #{ z: s * size, color: color, w: w_out }
}

/// Torus-section mapping — wraps the input through a torus-shaped surface
/// modulated by `n`, creating 3D donut-like structures.
///
/// # Authors
/// - gossamer light

// Taurus, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    r: #{
        value: 3.0, min: -10.0, max: 10.0, free: true,
        caption: "R",
        hint: "Torus radius.",
    },
    n: #{
        value: 5.0, min: -20.0, max: 20.0, free: true,
        caption: "N",
        hint: "Number of cosine-modulation cycles around the torus.",
    },
    inv: #{
        value: 1.5, min: -10.0, max: 10.0, free: true,
        caption: "Inv",
        hint: "Blend between fixed and modulated radius. 0 = fully modulated; 1 = fixed.",
    },
    sor: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Sor",
        hint: "Spherical-coordinate blend factor for the Z output.",
    },
};

const INFO = #{ title: "Taurus", name: "rhai_taurus", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let r_p = p.r;
    let n_p = p.n;
    let inv_p = p.inv;
    let sx = sin(z.re);
    let cx = cos(z.re);
    let sy = sin(z.im);
    let ir = (inv_p * r_p) + ((1.0 - inv_p) * (r_p * cos(n_p * z.re)));
    complex(cx * (ir + sy), sx * (ir + sy))
}

fn variation3(v, p, ctx) {
    let r_p = p.r;
    let n_p = p.n;
    let inv_p = p.inv;
    let sor_p = p.sor;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let sx = sin(v.x);
    let cx = cos(v.x);
    let sy = sin(v.y);
    let cy = cos(v.y);
    let ir = (inv_p * r_p) + ((1.0 - inv_p) * (r_p * cos(n_p * v.x)));
    let fx_unweighted = cx * (ir + sy);
    let fy_unweighted = sx * (ir + sy);
    let fz_full = w * sor_p * cy + (1.0 - sor_p) * v.y;
    vec3(fx_unweighted, fy_unweighted, fz_full * inv_w)
}

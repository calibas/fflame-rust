/// `octapol` — octagon-with-polar-core shape warp.
///
/// # Authors
/// - Xyrus02

// Octapol, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    polarweight: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Polar Weight",
        hint: "Blend toward polar coordinates inside the center circle. 0 = pass-through; larger magnitudes warp circle content onto the (φ, r) plane, scaled by log((r/rad)²) so the effect strengthens toward the center.",
    },
    radius: #{
        value: 1.0, min: -3.0, max: 3.0, free: true,
        caption: "Radius",
        hint: "Center-circle size factor. Effective radius is 0.7071 · s · |radius|; 0 disables the polar core entirely.",
    },
    s: #{
        value: 0.5, min: -2.0, max: 2.0, free: true,
        caption: "S",
        hint: "Octagon edge size — half the length of the axis-aligned edges, and the scale of the corner triangles.",
    },
    t: #{
        value: 0.5, min: -2.0, max: 2.0, free: true,
        caption: "T",
        hint: "Octagon extent — how far the diagonal edges reach beyond the inner square. The bounding half-size is s/2 + t.",
    },
};

const INFO = #{ title: "Octapol", name: "rhai_octapol", category: "plugin" };

fn prepare(p) {
    #{ a: p.s * 0.5 + p.t, rad: 0.707106781 * p.s * abs(p.radius) }
}

fn octapol_hits_rect(tl, br, p) {
    p.re >= tl.re && p.im >= tl.im && p.re <= br.re && p.im <= br.im
}

fn octapol_hits_triangle(a, b, c, p) {
    let v0 = c - a;
    let v1 = b - a;
    let v2 = p - a;
    let d00 = v0.norm_sqr();
    let d01 = (v0.re * v1.re + v0.im * v1.im);
    let d02 = (v0.re * v2.re + v0.im * v2.im);
    let d11 = v1.norm_sqr();
    let d12 = (v1.re * v2.re + v1.im * v2.im);
    let denom = d00 * d11 - d01 * d01;
    let u = 0.0;
    let v = 0.0;
    if (denom != 0.0) {
        u = (d11 * d02 - d01 * d12) / denom;
        v = (d00 * d12 - d01 * d02) / denom;
    }
    ((u + v) < 1.0) && (u > 0.0) && (v > 0.0)
}

fn variation(z, p, ctx) {
    let polarweight = p.polarweight;
    let s = p.s;
    let t = p.t;
    let a = p.a;
    let rad = p.rad;
    let q = complex(z.re * 0.15, z.im * 0.15);
    let hs = 0.5 * s;
    let va = complex(-hs, hs + t);
    let vb = complex(hs, hs + t);
    let vc_ = complex(t, hs);
    let vd = complex(t, -hs);
    let ve = complex(hs, -hs - t);
    let vf = complex(-hs, -hs - t);
    let vg = complex(-t, -hs);
    let vh = complex(-t, hs);
    let vi = complex(-hs, hs);
    let vj = complex(hs, hs);
    let vk = complex(-hs, -hs);
    let vl = complex(hs, -hs);
    let r = q.abs();
    if (rad > 0.0 && r <= rad) {
        let rd = ln((r / rad) * (r / rad));
        let phi = atan2(q.im, q.re);
        let m = rd * polarweight;
        return complex(mix(q.re, phi, m) + q.re, mix(q.im, r, m) + q.im);
    } else if (abs(q.re) <= a && abs(q.im) <= a) {
        if (octapol_hits_rect(vh, vk, q) || octapol_hits_rect(vj, vd, q) || octapol_hits_rect(va, vj, q) || octapol_hits_rect(vk, ve, q) || octapol_hits_triangle(vi, va, vh, q) || octapol_hits_triangle(vj, vb, vc_, q) || octapol_hits_triangle(vl, vd, ve, q) || octapol_hits_triangle(vk, vf, vg, q)) {
            return 2.0 * q;
        }
    }
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    q - ctx.accum * inv_w
}

fn variation3(v, p, ctx) {
    let polarweight = p.polarweight;
    let s = p.s;
    let t = p.t;
    let a = p.a;
    let rad = p.rad;
    let q = complex(v.x * 0.15, v.y * 0.15);
    let hs = 0.5 * s;
    let va = complex(-hs, hs + t);
    let vb = complex(hs, hs + t);
    let vc_ = complex(t, hs);
    let vd = complex(t, -hs);
    let ve = complex(hs, -hs - t);
    let vf = complex(-hs, -hs - t);
    let vg = complex(-t, -hs);
    let vh = complex(-t, hs);
    let vi = complex(-hs, hs);
    let vj = complex(hs, hs);
    let vk = complex(-hs, -hs);
    let vl = complex(hs, -hs);
    let r = q.abs();
    if (rad > 0.0 && r <= rad) {
        let rd = ln((r / rad) * (r / rad));
        let phi = atan2(q.im, q.re);
        let m = rd * polarweight;
        return vec3(mix(q.re, phi, m) + q.re, mix(q.im, r, m) + q.im, v.z);
    } else if (abs(q.re) <= a && abs(q.im) <= a) {
        if (octapol_hits_rect(vh, vk, q) || octapol_hits_rect(vj, vd, q) || octapol_hits_rect(va, vj, q) || octapol_hits_rect(vk, ve, q) || octapol_hits_triangle(vi, va, vh, q) || octapol_hits_triangle(vj, vb, vc_, q) || octapol_hits_triangle(vl, vd, ve, q) || octapol_hits_triangle(vk, vf, vg, q)) {
            let t1 = 2.0 * q;
            return vec3(t1.re, t1.im, v.z);
        }
    }
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let t2 = q - complex(ctx.accum.x, ctx.accum.y) * inv_w;
    vec3(t2.re, t2.im, v.z)
}

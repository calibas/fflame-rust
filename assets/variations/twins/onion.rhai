/// Onion-warp 3D variation — points inside a sphere (sized by the
/// variation's weight) stay flat with Z pushed to the bottom; outside, they
/// curl onto the sphere's top with an exponential tail. Forms onion-layer
/// structures.
///
/// # Authors
/// - chronologicaldot

// Onion, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    centre_x: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Centre X",
        hint: "X coordinate of the sphere center.",
    },
    centre_y: #{
        value: 0.0, min: -5.0, max: 5.0, free: true,
        caption: "Centre Y",
        hint: "Y coordinate of the sphere center.",
    },
};

const INFO = #{ title: "Onion", name: "rhai_onion", category: "full_3d", phase: "any" };

fn variation(z, p, ctx) {
    let cx = p.centre_x;
    let cy = p.centre_y;
    let w_raw = ctx.weight;
    let inv_w = 1.0 / (if abs(w_raw) < 1.0e-30 { 1.0e-30 } else { w_raw });
    let r0 = w_raw;
    if (r0 == 0.0) {
        r0 = 1.0;
    }
    let x0 = z.re - cx;
    let y0 = z.im - cy;
    let d0 = x0 * x0 + y0 * y0;
    let dr = sqrt(d0);
    let inv_sqrt2 = 0.7071067811865476;
    let x1 = 0.0;
    let y1 = 0.0;
    if (d0 <= r0 * r0) {
        x1 = x0;
        y1 = y0;
    } else {
        let radial = (2.0 * r0 - dr) / max(1.0e-30, dr);
        x1 = radial * x0;
        y1 = radial * y0;
    }
    complex((x1 + cx) * inv_w, (y1 + cy) * inv_w)
}

fn variation3(v, p, ctx) {
    let cx = p.centre_x;
    let cy = p.centre_y;
    let w_raw = ctx.weight;
    let inv_w = 1.0 / (if abs(w_raw) < 1.0e-30 { 1.0e-30 } else { w_raw });
    let r0 = w_raw;
    if (r0 == 0.0) {
        r0 = 1.0;
    }
    let x0 = v.x - cx;
    let y0 = v.y - cy;
    let d0 = x0 * x0 + y0 * y0;
    let dr = sqrt(d0);
    let inv_sqrt2 = 0.7071067811865476;
    let join = r0 * inv_sqrt2;
    let x1 = 0.0;
    let y1 = 0.0;
    let z1 = 0.0;
    if (d0 <= r0 * r0) {
        z1 = -sqrt(max(0.0, r0 * r0 - d0));
        x1 = x0;
        y1 = y0;
    } else if (2.0 * r0 - dr > join) {
        let radial = (2.0 * r0 - dr) / max(1.0e-30, dr);
        x1 = radial * x0;
        y1 = radial * y0;
        z1 = sqrt(max(0.0, r0 * r0 - (x1 * x1 + y1 * y1)));
    } else {
        z1 = exp(dr - r0 - (r0 - join)) - 1.0 + join;
        let radial = (2.0 * r0 - dr) / max(1.0e-30, dr);
        x1 = radial * x0;
        y1 = radial * y0;
    }
    vec3((x1 + cx) * inv_w, (y1 + cy) * inv_w, (z1 + v.z) * inv_w)
}

/// Barycentric-coordinate triangle mapping with direct-color writes — uses
/// the transform's affine `(a, b)` and `(-c, -d)` as the triangle edge
/// basis, with `(e, f)` as the origin vertex. Tests whether the input lies
/// inside the triangle via barycentric coordinates `(u, v)`. Inside, passes
/// through. Outside, either collapses to origin (`zero_edges = 1`) or
/// scatters by `scatter_area` from the nearest edge. Color is `fmod(|u +
/// v|, 1)` of the post-clamp barycentric coords (matches cpp `TC =
/// fmod(fabs(u+v), 1.0)`).

// DC Triangle, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    scatter_area: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Scatter Area",
        hint: "Random scatter amount applied to points outside the triangle. Clamped to `[-1, 1]` internally.",
    },
    zero_edges: #{
        value: false,
        caption: "Zero Edges",
        hint: "When on, out-of-triangle points collapse to the origin. When off, they scatter by `scatter_area`.",
    },
};

const INFO = #{ title: "DC Triangle", name: "rhai_dc_triangle", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    #{ a_param: clamp(p.scatter_area, -1.0, 1.0) }
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let zero_edges = to_int(to_float(p.zero_edges));
    let a_param = p.a_param;
    let xx = ctx.a;
    let xy = ctx.b;
    let yx = -ctx.c;
    let yy = -ctx.d;
    let ox = ctx.e;
    let oy = ctx.f;
    let px = z.re - ox;
    let py = z.im - oy;
    let dot00 = xx * xx + xy * xy;
    let dot01 = xx * yx + xy * yy;
    let dot02 = xx * px + xy * py;
    let dot11 = yx * yx + yy * yy;
    let dot12 = yx * px + yy * py;
    let denom = dot00 * dot11 - dot01 * dot01;
    let safe_denom = (if abs(denom) < 1.0e-30 { 1.0e-30 } else { denom });
    let u = (dot11 * dot02 - dot01 * dot12) / safe_denom;
    let v = (dot00 * dot12 - dot01 * dot02) / safe_denom;
    let inside = false;
    let f = 1.0;
    if (u + v > 1.0) {
        f = -1.0;
        if (u > v) {
            u = min(1.0, u);
            v = 1.0 - u;
        } else {
            v = min(1.0, v);
            u = 1.0 - v;
        }
    } else if (u < 0.0 || v < 0.0) {
        u = clamp(u, 0.0, 1.0);
        v = clamp(v, 0.0, 1.0);
    } else {
        inside = true;
    }
    if (zero_edges != 0 && !inside) {
        u = 0.0;
        v = 0.0;
    } else if (!inside) {
        u = u + rand() * a_param * f;
        v = v + rand() * a_param * f;
        u = clamp(u, -1.0, 1.0);
        v = clamp(v, -1.0, 1.0);
        if (u + v > 1.0 && a_param > 0.0) {
            if (u > v) {
                u = min(1.0, u);
                v = 1.0 - u;
            } else {
                v = min(1.0, v);
                u = 1.0 - v;
            }
        }
    }
    let raw = abs(u + v);
    color = raw - to_float(floor(raw));
    #{ z: complex(ox + u * xx + v * yx, oy + u * xy + v * yy), color: color }
}

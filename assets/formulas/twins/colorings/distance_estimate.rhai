// Distance Estimate, the built-in colouring's twin: the exterior
// distance to the set's boundary, d = |z| ln|z| / |dz|, from the
// derivative orbit, mapped through -log2 (or linearly, or by its square
// root) so equal palette steps are equal zoom depths of distance.

const PARAMS = #{
    scale: #{
        value: 0.05, min: 0.000001, max: 1.0, exponential: true,
        caption: "Scale",
        hint: "Palette distance per doubling of boundary distance (Log), or per unit of it (Linear, Square root).",
    },
    mapping: #{
        value: 0, choices: ["Log", "Linear", "Square root"],
        caption: "Mapping",
        hint: "How the distance becomes a palette position (Kalles Fraktaler's distance colourings): Log, equal steps per doubling; Linear, the distance itself; Square root, between the two.",
    },
    units: #{
        value: 0, choices: ["Plane", "Pixels"],
        caption: "Units",
        hint: "Measure the distance in the plane, or in pixels. In pixels the boundary is always about one pixel away, so the colours hold still as you zoom (as Fraktaler 3 and Kalles Fraktaler draw it).",
    },
};

const INFO = #{ title: "Distance Estimate", region: "outside", bailout: 1.0e4 };

// Without a derivative `ctx.dz` is its seed, and d a smooth function of
// the escape radius that looks like a distance estimate and is not one:
// flat instead, 0.5, so the exterior does not read as the inside.
fn finish(s, z, p, ctx) {
    if ctx.has_derivative {
        // |z| > 1 at escape, so ln|z| > 0.
        let r = max(z.abs(), 1.0000001);
        let deriv = max(ctx.dz.abs(), 1e-30);
        // `ctx.dz` is per render pixel: d is in render pixels.
        let d_render = max(r * ln(r) / deriv, 1e-30);
        // In output pixels, as Fraktaler 3 measures it, or in the plane.
        let d = if p.units == 1 { max(d_render / ctx.supersample, 1e-30) } else { max(d_render * ctx.render_px, 1e-30) };
        if p.mapping == 1 {
            d * p.scale
        } else if p.mapping == 2 {
            sqrt(d) * p.scale
        } else {
            -log2(d) * p.scale
        }
    } else {
        0.5
    }
}

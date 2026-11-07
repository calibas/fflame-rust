/// Eclipse-shaped X-axis clamp — inside an elliptical region sized by the
/// variation's weight, X gets shifted by `shift · w`; outside the region,
/// points pass through unchanged. Produces an eclipse silhouette.
///
/// # Authors
/// - Michael Faber

// Eclipse, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    shift: #{
        value: 0.0, min: -2.0, max: 2.0,
        caption: "Shift",
        hint: "Horizontal shift applied to points inside the eclipse region, scaled by the variation's weight.",
    },
};

const INFO = #{ title: "Eclipse", name: "rhai_eclipse", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let shift = p.shift;
    let w = ctx.weight;
    let ox = z.re;
    if (abs(z.im) <= w) {
        let c2 = sqrt(max(0.0, w * w - z.im * z.im));
        if (abs(z.re) <= c2) {
            let x_shifted = z.re + shift * w;
            if (abs(x_shifted) >= c2) {
                ox = -z.re;
            } else {
                ox = x_shifted;
            }
        }
    }
    complex(ox, z.im)
}

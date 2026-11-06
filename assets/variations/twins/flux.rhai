/// VVAR-shift Möbius warp — computes `xpw = x + w` and `xmw = x − w` (where
/// `w` is the variation weight), then emits a Möbius-style radial × angular
/// combination: `r = w·(2 + spread)·sqrt(sqrt(y² + xpw²) / sqrt(y² +
/// xmw²))` and `a = (atan2(y, xmw) − atan2(y, xpw)) / 2`. Produces flux-
/// like field-line patterns between two virtual poles at `±w`.
///
/// # Authors
/// - meckie

// Flux, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    spread: #{
        value: 0.3, min: -10.0, max: 10.0, free: true,
        caption: "Spread",
        hint: "Output magnitude scale, offset from a base value of 2.",
    },
};

const INFO = #{ title: "Flux", name: "rhai_flux", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let spread = p.spread;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let xpw = z.re + w;
    let xmw = z.re - w;
    let denom_arg = max(z.im * z.im + xmw * xmw, 1.0e-30);
    let num_arg = max(z.im * z.im + xpw * xpw, 1.0e-30);
    let avgr = w * (2.0 + spread) * sqrt(sqrt(num_arg) / sqrt(denom_arg));
    let avga = (atan2(z.im, xmw) - atan2(z.im, xpw)) * 0.5;
    complex(avgr * cos(avga) * inv_w, avgr * sin(avga) * inv_w)
}

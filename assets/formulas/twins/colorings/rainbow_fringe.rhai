// Rainbow Fringe, the built-in colouring's twin (Fraktaler 3's
// `rainbow-fringe` example): black on white with a rainbow at the
// boundary. The hue is the direction of the distance-estimate vector on
// the screen, its saturation and value come from its length L in output
// pixels -- hsv(arg / 2pi, 1 / (1 + 4L), 2L) -- decoded to linear light
// and raised to 1 / gamma.

const PARAMS = #{
    gamma: #{
        value: 2.0, min: 0.1, max: 10.0,
        caption: "Gamma",
        hint: "The colour is raised to 1/gamma: higher lifts the fringe's dark side toward the white. Fraktaler 3's example uses 2.",
    },
};

const INFO = #{ title: "Rainbow Fringe", region: "outside", bailout: 1.0e4 };

// The distance estimate's length, |z| ln|z| / |dz|, in output pixels.
fn distance(z, dz, supersample) {
    let r = max(z.abs(), 1.0000001);
    r * ln(r) / max(dz.abs(), 1e-30) / supersample
}

// sRGB to linear light, one part.
fn to_linear(c) {
    let x = clamp(c, 0.0, 1.0);
    if x <= 0.04045 { x / 12.92 } else { pow((x + 0.055) / 1.055, 2.4) }
}

// One part of hsv2sRGB (lolengine), as Fraktaler 3 has it: `k` the
// part's offset, mix written as WGSL's.
fn hsv_part(hue, sat, val, k) {
    let q = abs(fract(hue + k) * 6.0 - 3.0);
    val * (1.0 + (clamp(q - 1.0, 0.0, 1.0) - 1.0) * sat)
}

// The value the relief and auto contrast read: the distance's log, as the
// distance estimate's Log mapping has it.
fn finish(s, z, p, ctx) {
    if ctx.has_derivative { -log2(max(distance(z, ctx.dz, ctx.supersample), 1e-30)) } else { 0.0 }
}

fn color(s, z, p, ctx) {
    if ctx.has_derivative {
        let l = distance(z, ctx.dz, ctx.supersample);
        // The gradient of |z|^2 in the plane, z conj(dz), with dz scaled
        // down first so a huge derivative keeps its direction; then onto
        // the screen, conj(rotation) times it.
        let dz = ctx.dz;
        let m = max(max(abs(dz.re), abs(dz.im)), 1e-30);
        let d = complex(dz.re / m, dz.im / m);
        let g = complex(z.re * d.re + z.im * d.im, z.im * d.re - z.re * d.im);
        let rot = ctx.rotation;
        let sc = complex(g.re * rot.re + g.im * rot.im, g.im * rot.re - g.re * rot.im);
        // Fraktaler 3's vector is its conjugate.
        let hue = if max(abs(sc.re), abs(sc.im)) > 0.0 {
            let h = atan2(-sc.im, sc.re) / 6.2831855;
            h - to_float(floor(h))
        } else { 0.0 };
        let sat = clamp(1.0 / (1.0 + 4.0 * l), 0.0, 1.0);
        let val = clamp(2.0 * l, 0.0, 1.0);
        let e = 1.0 / max(p.gamma, 0.1);
        vec3(
            pow(to_linear(hsv_part(hue, sat, val, 1.0)), e),
            pow(to_linear(hsv_part(hue, sat, val, 2.0 / 3.0)), e),
            pow(to_linear(hsv_part(hue, sat, val, 1.0 / 3.0)), e)
        )
    } else {
        // No derivative, no fringe: flat mid-grey, "unavailable".
        vec3(0.21404114, 0.21404114, 0.21404114)
    }
}

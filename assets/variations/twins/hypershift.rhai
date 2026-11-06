// Hypershift, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    shift: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Shift",
        hint: "Horizontal shift applied after the first inversion. Also scales the overall transformation strength via `1 − shift²`.",
    },
    stretch: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Stretch",
        hint: "Y-axis stretching factor applied to the output.",
    },
};

const INFO = #{ title: "Hypershift", name: "rhai_hypershift" };

fn variation(z, p, ctx) {
    let shift = p.shift;
    let stretch = p.stretch;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let scale = 1.0 - shift * shift;
    let rad1 = 1.0 / max(z.re * z.re + z.im * z.im, 1.0e-30);
    let x1 = rad1 * z.re + shift;
    let y1 = rad1 * z.im;
    let rad2 = w * scale / max(x1 * x1 + y1 * y1, 1.0e-30);
    let fx = rad2 * x1 + shift;
    let fy = rad2 * y1 * stretch;
    complex(fx * inv_w, fy * inv_w)
}

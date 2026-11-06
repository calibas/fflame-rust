// Stripfit, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    dx: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "DX",
        hint: "Per-strip horizontal shift amount (scaled by `-0.5` internally).",
    },
};

const INFO = #{ title: "Stripfit", name: "rhai_stripfit" };

fn variation(z, p, ctx) {
    let dx = p.dx;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let dxp = -0.5 * dx;
    let fy = 0.0;
    let fx_extra = 0.0;
    if (z.im > 1.0) {
        let fity = (z.im + 1.0) - to_float(floor((z.im + 1.0) * 0.5)) * 2.0;
        fy = -1.0 + fity;
        fx_extra = (z.im - fity + 1.0) * dxp;
    } else if (z.im < -1.0) {
        let fity = (1.0 - z.im) - to_float(floor((1.0 - z.im) * 0.5)) * 2.0;
        fy = 1.0 - fity;
        fx_extra = (z.im + fity - 1.0) * dxp;
    } else {
        fy = z.im;
    }
    complex(z.re + fx_extra * inv_w, fy)
}

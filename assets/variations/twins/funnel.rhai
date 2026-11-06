// Funnel, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    effect: #{
        value: 8, min: -100, max: 100,
        caption: "Effect",
        hint: "Additive angular offset on each axis (scaled by π). Higher = more horizontal/vertical bias.",
    },
};

const INFO = #{ title: "Funnel", name: "rhai_funnel" };

fn variation(z, p) {
    let effect = to_float(p.effect);
    let pi = 3.14159265358979;
    let cx = cos(z.re);
    let cy = cos(z.im);
    let safe_cx = (if abs(cx) < 1.0e-30 { 1.0e-30 } else { cx });
    let safe_cy = (if abs(cy) < 1.0e-30 { 1.0e-30 } else { cy });
    let tx_raw = tanh(z.re);
    let ty_raw = tanh(z.im);
    let t1 = z.re;
    let tx = (if abs(tx_raw) <= 1.0e32 { tx_raw } else { (if t1 > 0.0 { 1.0 } else if t1 < 0.0 { -1.0 } else { 0.0 }) });
    let t2 = z.im;
    let ty = (if abs(ty_raw) <= 1.0e32 { ty_raw } else { (if t2 > 0.0 { 1.0 } else if t2 < 0.0 { -1.0 } else { 0.0 }) });
    let fx = tx * (1.0 / safe_cx + effect * pi);
    let fy = ty * (1.0 / safe_cy + effect * pi);
    complex(fx, fy)
}

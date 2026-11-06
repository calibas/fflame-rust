/// Running-bond brick-grid stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut Bricks, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    seed: #{
        value: 1000, min: 0, max: 1000000,
        caption: "Seed",
        hint: "UI-only in JWildfire; ignored by the renderer.",
    },
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    size: #{
        value: 0.9, min: 0.1, max: 0.9999,
        caption: "Size",
        hint: "Brick fill fraction within each cell (mortar gap = 1 − size).",
    },
    zoom: #{
        value: 3.0, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale (brick count): `point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut Bricks", name: "rhai_cut_bricks", category: "plugin", phase: "any", replace: true };

fn cut_bricks_brickTile(st0) {
    let st = st0;
    let mody = st.im - 2.0 * to_float(floor(st.im / 2.0));
    st = complex(st.re + (if mody < 1.0 { 0.0 } else { 1.0 }) * 0.5, st.im);
    fract(st)
}

fn cut_bricks_box(st, size0) {
    let sz = complex(0.5, 0.5) - size0 * 0.5;
    let t1 = sz + complex(1.0e-4, 1.0e-4);
    let t2 = clamp((st.re - sz.re) / (t1.re - sz.re), 0.0, 1.0);
    let t3 = clamp((st.im - sz.im) / (t1.im - sz.im), 0.0, 1.0);
    let uv = complex(t2 * t2 * (3.0 - 2.0 * t2), t3 * t3 * (3.0 - 2.0 * t3));
    let t5 = sz + complex(1.0e-4, 1.0e-4);
    let t6 = complex(1.0, 1.0) - st;
    let t7 = clamp((t6.re - sz.re) / (t5.re - sz.re), 0.0, 1.0);
    let t8 = clamp((t6.im - sz.im) / (t5.im - sz.im), 0.0, 1.0);
    let t4 = complex(t7 * t7 * (3.0 - 2.0 * t7), t8 * t8 * (3.0 - 2.0 * t8));
    uv = complex(uv.re * t4.re, uv.im * t4.im);
    uv.re * uv.im
}

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let size = p.size;
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let x = 0.0;
    let y = 0.0;
    let cx = 0.0;
    let cy = 0.0;
    if (mode == 0) {
        x = z.re;
        y = z.im;
    } else {
        x = rand();
        y = rand();
        cx = 0.5;
        cy = 0.5;
    }
    let u = complex(x * zoom, y * zoom);
    u = cut_bricks_brickTile(u);
    let color = cut_bricks_box(u, complex(size, size));
    let hidden = false;
    if (invert == 0) {
        if (color > 0.0) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (color <= 0.0) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x - cx, y - cy), hide: hide_v }
}

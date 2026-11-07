/// Rotated-tile radial-wave pattern stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut Pattern, the built-in variation's twin, translated from its WGSL.

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
    time: #{
        value: 0.9, min: -100.0, max: 100.0, free: true,
        caption: "Time",
        hint: "Animates the radial wave phase.",
    },
    zoom: #{
        value: 0.5, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut Pattern", name: "rhai_cut_pattern", category: "plugin", phase: "any", replace: true };

fn cut_pattern_rotate2D(st0, angle) {
    let st = complex(st0.re - 0.5, st0.im - 0.5);
    let c = cos(angle);
    let s = sin(angle);
    st = complex(c * st.re - s * st.im, s * st.re + c * st.im);
    complex(st.re + 0.5, st.im + 0.5)
}

fn cut_pattern_rotateTilePattern(st0) {
    let PI = 3.14159265359;
    let st = st0 * 2.0;
    let index = 0.0;
    index = index + (if st.re - 2.0 * to_float(floor(st.re / 2.0)) < 1.0 { 0.0 } else { 1.0 });
    index = index + (if st.im - 2.0 * to_float(floor(st.im / 2.0)) < 1.0 { 0.0 } else { 1.0 }) * 2.0;
    st = fract(st);
    if (index == 0.0) {
        st = cut_pattern_rotate2D(st, PI() * 2.0 * 1.0 / 4.0);
    } else if (index == 1.0) {
        st = cut_pattern_rotate2D(st, PI() * 2.0 * 3.0 / 4.0);
    } else if (index == 2.0) {
        st = cut_pattern_rotate2D(st, PI() * 2.0 * 1.0 / 4.0);
    } else if (index == 3.0) {
        st = cut_pattern_rotate2D(st, PI() * 2.0 * 3.0 / 4.0);
    }
    st
}

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let time = p.time;
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let PI = 3.14159265359;
    let x = 0.0;
    let y = 0.0;
    if (mode == 0) {
        x = z.re;
        y = z.im;
    } else {
        x = rand() - 0.5;
        y = rand() - 0.5;
    }
    let uv = complex(x, y) * zoom;
    uv = fract(uv);
    uv = cut_pattern_rotateTilePattern(uv);
    let pos = complex(0.0, 5.0) - uv;
    let radius = pos.abs();
    let r = sin(radius * sin(uv.im * PI() * 5.0 + time + cos(sin(uv.re * PI() * 3.0) * PI() * 2.0 + sin(uv.im * PI() * 15.0))) * 1.0 * sin(uv.im * PI() + sin(uv.re * PI() * 5.0)));
    let color = cos(r * PI() * 2.0 + PI() * 2.0) * 0.9 + sin(r * PI() * 2.0 + PI() * 2.0) * 0.7;
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
    #{ z: complex(x, y), hide: hide_v }
}

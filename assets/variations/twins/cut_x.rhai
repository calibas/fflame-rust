/// Rotated absolute-value cross (45° fold) stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut X, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    zoom: #{
        value: 1.0, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `uv = point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
    size: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Size",
        hint: "Half-thickness of the cross arms.",
    },
};

const INFO = #{ title: "Cut X", name: "rhai_cut_x", category: "plugin", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let size = p.size;
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
    let t1 = complex(x, y) * zoom;
    let st = complex(abs(t1.re), abs(t1.im));
    let ca = cos(PI() / 4.0);
    let sa = sin(PI() / 4.0);
    st = complex(ca * st.re - sa * st.im, sa * st.re + ca * st.im);
    st = complex(abs(st.re), abs(st.im));
    let t2 = st.im - size;
    let t3 = clamp((t2 - 0.0) / (0.009 - 0.0), 0.0, 1.0);
    let line = (t3 * t3 * (3.0 - 2.0 * t3));
    let color = mix(0.0, 1.0, line);
    let hidden = false;
    if (invert == 0) {
        if (color < 0.1) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (color >= 0.1) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x, y), hide: hide_v }
}

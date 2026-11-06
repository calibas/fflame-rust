/// Recursive circle-subtraction design stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut CircleDesign, the built-in variation's twin, translated from its WGSL.

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
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Time",
        hint: "Animates the per-level radius shrink.",
    },
    zoom: #{
        value: 10.0, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale (cell repeat): `point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut CircleDesign", name: "rhai_cut_circdes", category: "plugin", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let time = p.time;
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
    let t1 = fract(u);
    u = complex(t1.re - 0.5, t1.im - 0.5);
    let r = 0.8;
    let d = (if r < u.abs() { 0.0 } else { 1.0 });
    for i in 0..9 {
        r = r * (0.5 + 0.1 * sin(to_float(i) + time));
        if ((i % 2) == 0) {
            d = d - (if r < (u + complex(0.0, r)).abs() { 0.0 } else { 1.0 });
            d = d - (if r < (u + complex(r, 0.0)).abs() { 0.0 } else { 1.0 });
            d = d - (if r < (u - complex(0.0, r)).abs() { 0.0 } else { 1.0 });
            d = d - (if r < (u - complex(r, 0.0)).abs() { 0.0 } else { 1.0 });
        } else {
            d = d + (if r < (u + complex(0.0, r)).abs() { 0.0 } else { 1.0 });
            d = d + (if r < (u + complex(r, 0.0)).abs() { 0.0 } else { 1.0 });
            d = d + (if r < (u - complex(0.0, r)).abs() { 0.0 } else { 1.0 });
            d = d + (if r < (u - complex(r, 0.0)).abs() { 0.0 } else { 1.0 });
        }
    }
    let color = d;
    let hidden = false;
    if (invert == 0) {
        if (color > 0.5) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (color <= 0.5) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x - cx, y - cy), hide: hide_v }
}

/// Layered glyph-blob stencil mask (nested ring sampling).
///
/// # Authors
/// - Jesus Sosa

// Cut Glypho, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    zoom: #{
        value: 1.0, min: 0.1, max: 50.0,
        caption: "Zoom",
        hint: "Pattern scale: `uv = point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
    f1: #{
        value: 0.115, min: -10.0, max: 10.0, free: true,
        caption: "F1",
        hint: "Blob radius factor (smoothstep edge `f1·n`).",
    },
    f2: #{
        value: 0.75, min: -10.0, max: 10.0, free: true,
        caption: "F2",
        hint: "Banding threshold (`step(f2, fract(...))`).",
    },
    f3: #{
        value: 1.5, min: -10.0, max: 10.0, free: true,
        caption: "F3",
        hint: "Accumulation gain and band frequency.",
    },
};

const INFO = #{ title: "Cut Glypho", name: "rhai_cut_glypho", category: "plugin", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let f1 = p.f1;
    let f2 = p.f2;
    let f3 = p.f3;
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
    let m = 0.0;
    let stp = PI() / 5.0;
    let n = 1.0;
    while n < 2.5 {
        let i = 0.0001;
        while i < 2.0 * PI() {
            let uvi = uv * n + complex(cos(i + n * stp * 0.5) * 0.4, sin(i + n * stp * 0.5) * 0.4);
            let l = uvi.abs();
            let t1 = f1 * n;
            let t2 = clamp((l - t1) / (0.0 - t1), 0.0, 1.0);
            m = m + (t2 * t2 * (3.0 - 2.0 * t2)) * f3;
            i = i + stp;
        }
        n = n + 0.5;
    }
    m = (if fract(m * f3) < f2 { 0.0 } else { 1.0 });
    let color = m;
    let hidden = false;
    if (invert == 0) {
        if (color == 0.0) {
            hidden = true;
        }
    } else if (color > 0.0) {
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x, y), hide: hide_v }
}

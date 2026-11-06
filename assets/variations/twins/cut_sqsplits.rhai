/// Radially-split square-tile stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut SqSplits, the built-in variation's twin, translated from its WGSL.

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
        hint: "Phase of the radial split rotation.",
    },
    zoom: #{
        value: 2.0, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `uv = point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut SqSplits", name: "rhai_cut_sqsplits", category: "plugin", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let time = p.time;
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let x = 0.0;
    let y = 0.0;
    if (mode == 0) {
        x = z.re;
        y = z.im;
    } else {
        x = rand() - 0.5;
        y = rand() - 0.5;
    }
    let uu = complex(x * zoom, y * zoom);
    let t1 = fract(uu);
    let uf = complex(t1.re - 0.5, t1.im - 0.5);
    let ui = floor(uu - 2.0 * floor(uu / 2.0));
    let t = time * 0.35;
    let a = atan2(uf.re, uf.im) / 6.3;
    let r = fract(t + t + 2.0 * (if ui.re != ui.im { a } else { 1.0 - a }));
    let sharp = 180.0 * uf.abs();
    let k = clamp((r - 0.5) * sharp, 0.0, 1.0) + clamp(1.0 - r * sharp, 0.0, 1.0);
    let color = 1.0 - k;
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

/// Square-to-circle morph tiling stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut SqCir, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call (the usual stencil mode).",
    },
    zoom: #{
        value: 2.0, min: 0.1, max: 50.0,
        caption: "Zoom",
        hint: "Pattern scale: `uv = point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
    power: #{
        value: 2.0, min: 0.0, max: 2.0,
        caption: "Power",
        hint: "Superellipse exponent: 2 = circle, →0 pinches toward a plus/asterisk.",
    },
};

const INFO = #{ title: "Cut SqCir", name: "rhai_cut_sqcir", category: "plugin", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let power = p.power;
    let x = 0.0;
    let y = 0.0;
    if (mode == 0) {
        x = z.re;
        y = z.im;
    } else {
        x = rand() - 0.5;
        y = rand() - 0.5;
    }
    let uvx = x * zoom;
    let uvy = y * zoom;
    let lhs = pow(abs(uvx - uvy), power) + pow(abs(uvy + uvx), power);
    let color = (if lhs <= 1.0 { 1 } else { 0 });
    let hidden = false;
    if (invert == 0) {
        if (color == 0) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (color == 1) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x, y), hide: hide_v }
}

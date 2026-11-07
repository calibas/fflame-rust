/// XOR/AND/OR bit-pattern stencil of the integer grid.
///
/// # Authors
/// - Jesus Sosa

// Cut Booleans, the built-in variation's twin, translated from its WGSL.

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
    type: #{
        value: 0, choices: ["Xor", "And", "Or"],
        caption: "Type",
        hint: "Bitwise op applied to the integer grid coordinates.",
    },
    zoom: #{
        value: 250.0, min: -1000.0, max: 1000.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `uv = point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut Booleans", name: "rhai_cut_booleans", category: "plugin", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let btype = to_int(to_float(p.type));
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
    let u = complex(x, y) * zoom;
    let ix = to_int(u.re);
    let iy = to_int(u.im);
    let temp = 0;
    if (btype == 0) {
        temp = ix ^ iy;
    } else if (btype == 1) {
        temp = ix & iy;
    } else {
        temp = ix | iy;
    }
    let color = sin(to_float(temp));
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

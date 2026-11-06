// Tile HLP, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    width: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Width",
        hint: "Stripe width along X. Smaller width = more, narrower stripes.",
    },
};

const INFO = #{ title: "Tile HLP", name: "rhai_tile_hlp" };

fn variation(z, p, ctx) {
    let width = p.width;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let pi = 3.14159265358979;
    let safe_width = (if width == 0.0 { 1.0e-30 } else { width });
    let width2 = safe_width * w;
    let x = z.re / safe_width;
    let aux = 0.0;
    if (x > 0.0) {
        aux = x - to_float(floor(x));
    } else {
        aux = x + to_float(ceil(-x));
    }
    aux = cos(aux * pi);
    let aux2 = 0.0;
    if (aux < rand() * 2.0 - 1.0) {
        aux2 = (if x > 0.0 { -width2 } else { width2 });
    }
    let fpx = z.re * w + aux2;
    let fpy = w * z.im;
    complex(fpx * inv_w, fpy * inv_w)
}

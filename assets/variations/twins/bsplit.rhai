/// Tangent-split warp — outputs `(cos(y)/tan(x), -y/sin(x))` with user-
/// adjustable X/Y shifts. Degenerate at `x = 0` and `x = π` (sin = 0);
/// those points contribute zero.
///
/// # Authors
/// - Raykoid666
/// - chronologicaldot

// BSplit, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "X shift",
        hint: "X-axis shift added before the trig terms.",
    },
    y: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Y shift",
        hint: "Y-axis shift added before the trig terms.",
    },
};

const INFO = #{ title: "BSplit", name: "rhai_bsplit", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p, ctx) {
    let hidden = ctx.hide;
    let x_shift = p.x;
    let y_shift = p.y;
    let pi = 3.14159265358979;
    let arg = z.re + x_shift;
    let s = sin(arg);
    if (s == 0.0 || arg == pi) {
        hidden = true;
        return #{ z: complex(0.0, 0.0), hide: hidden };
    }
    let c = cos(arg);
    #{ z: complex(cos(z.im + y_shift) * c / s, (-z.im + y_shift) / s), hide: hidden }
}

fn variation3(v, p, ctx) {
    let hidden = ctx.hide;
    let x_shift = p.x;
    let y_shift = p.y;
    let pi = 3.14159265358979;
    let arg = v.x + x_shift;
    let s = sin(arg);
    if (s == 0.0 || arg == pi) {
        hidden = true;
    }
    let c = cos(arg);
    #{ z: vec3(cos(v.y + y_shift) * c / s, (-v.y + y_shift) / s, v.z), hide: hidden }
}

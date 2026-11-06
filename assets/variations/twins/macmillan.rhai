/// Self-iterating McMillan-map walker. The McMillan map is the discrete
/// dynamical system `y' = -x + 2a · y/(1+y²) + b·y` (from Edwin McMillan's
/// 1971 nonlinear-dynamics work); each iteration applies the 2nd-order form
/// twice from a per-thread state `(xa, x, y)` initialized to `(startx,
/// startx, starty)`. Ignores the input position and emits the post-
/// iteration `(x, y)` as a deterministic trajectory; also writes a color
/// register `vc` derived from the running accumulator plus this iteration's
/// own contribution. First port to exercise the full feature set
/// (`needs_accum + needs_transform + writes_color + wgsl_state_init`).

// MacMillan, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.6, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "McMillan-map parameter `a` — coefficient of the nonlinear term `2a · y/(1+y²)`. The classic chaotic regime is around `a ≈ 1.6`.",
    },
    b: #{
        value: 0.4, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "McMillan-map parameter `b` — linear coefficient on `y`. Together with `a` determines whether the trajectory is bounded, periodic, or chaotic.",
    },
    N: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "N",
        hint: "Declared but unused in the body — the map is hardcoded to two iterations per call. Possibly a port omission (upstream cpp may use `N` as an inner-loop bound). Tracked in TODO.",
    },
    startx: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Start X",
        hint: "Initial X coordinate for the per-thread state (read once via `wgsl_state_init` at thread start). Sets both `xa` and `x` slots.",
    },
    starty: #{
        value: 0.1, min: -10.0, max: 10.0, free: true,
        caption: "Start Y",
        hint: "Initial Y coordinate for the per-thread state.",
    },
};

const INFO = #{ title: "MacMillan", name: "rhai_macmillan", category: "advanced_2d" };

fn init(p) {
    let startx = p.startx;
    let starty = p.starty;
    #{ xa: startx, x: startx, y: starty }
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let a = p.a;
    let b = p.b;
    let xa = this.xa;
    let x = this.x;
    let y = this.y;
    x = y;
    y = -xa + 2.0 * a * (y / (1.0 + y * y)) + b * y;
    xa = x;
    x = y;
    y = -xa + 2.0 * a * (y / (1.0 + y * y)) + b * y;
    let weight = ctx.weight;
    let fpx = ctx.accum.re + weight * x;
    let fpy = ctx.accum.im + weight * y;
    let s = abs(0.5 * (fpx + fpy + 1.0));
    color = s - to_float(floor(s));
    this.xa = x;
    this.x = x;
    this.y = y;
    #{ z: complex(x, y), color: color }
}

/// Hexagonal vertex sequencer for snowflake designs. Places successive
/// iterations at the 6 vertices of a unit hexagon (when internal state
/// `rswtch ≤ 1`) or the 3 vertices of an inscribed triangle (when `rswtch >
/// 1`), cycling through them with internal `fcycle`/`bcycle` counters and
/// re-randomizing `rswtch ∈ {0, 1, 2}` each time a cycle completes. `majp`
/// controls Z-plane behavior: `|majp| ≤ 1` puts all points on a single Z
/// plane; `|majp| > 1` splits into two planes separated by `±(|majp| - 1) ·
/// 0.5` along Z, sign picked randomly per iteration. Uses `needs_accum +
/// needs_transform` to implement cpp's replacement-style FPx/FPy via the
/// `(desired - accum) / weight` workaround.
///
/// # Authors
/// - Larry Berlin

// Hexaplay 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    majp: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Major Plane",
        hint: "Major-plane threshold for Z behavior. `|majp| ≤ 1` = all points on a single Z plane (no Z split). `|majp| > 1` = points split into two planes separated by `±(|majp| - 1) · 0.5` along Z; sign picked randomly per iteration. Unused in 2D mode (Z param).",
    },
    scale: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Input-blend scale. Internally pre-multiplied by 0.5; the X/Y output is `(accum · (scale - 1) + p · scale) / weight + vertex_offset`.",
    },
    zlift: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "Z Lift",
        hint: "Z input scale: `oz = p.z · 0.5 · zlift / weight`. Unused in 2D mode.",
    },
};

const INFO = #{ title: "Hexaplay 3D", name: "rhai_hexaplay3d", category: "full_3d" };

fn init(p) {
    let r = rand();
    #{ rswtch: to_float(floor(r * 3.0)), fcycle: 0.0, bcycle: 0.0 }
}

fn hex_seg60_2d(loc) {
    let hlift = 0.86602540378443864;
    let t1 = (loc);
    if t1 == u32(0) {
        complex(1.0, 0.0)
    } else if t1 == u32(1) {
        complex(0.5, hlift)
    } else if t1 == u32(2) {
        complex(-0.5, hlift)
    } else if t1 == u32(3) {
        complex(-1.0, 0.0)
    } else if t1 == u32(4) {
        complex(-0.5, -hlift)
    } else {
        complex(0.5, -hlift)
    }
}

fn hex_seg120_2d(loc) {
    let hlift = 0.86602540378443864;
    let t1 = (loc);
    if t1 == u32(0) {
        complex(1.0, 0.0)
    } else if t1 == u32(1) {
        complex(-0.5, hlift)
    } else {
        complex(-0.5, -hlift)
    }
}

fn hex_seg60_3d(loc) {
    let hlift = 0.86602540378443864;
    let t1 = (loc);
    if t1 == u32(0) {
        complex(1.0, 0.0)
    } else if t1 == u32(1) {
        complex(0.5, hlift)
    } else if t1 == u32(2) {
        complex(-0.5, hlift)
    } else if t1 == u32(3) {
        complex(-1.0, 0.0)
    } else if t1 == u32(4) {
        complex(-0.5, -hlift)
    } else {
        complex(0.5, -hlift)
    }
}

fn hex_seg120_3d(loc) {
    let hlift = 0.86602540378443864;
    let t1 = (loc);
    if t1 == u32(0) {
        complex(1.0, 0.0)
    } else if t1 == u32(1) {
        complex(-0.5, hlift)
    } else {
        complex(-0.5, -hlift)
    }
}

fn variation(z, p, ctx) {
    let majp = p.majp;
    let scale_param = p.scale;
    let rswtch = this.rswtch;
    let fcycle = this.fcycle;
    let bcycle = this.bcycle;
    if (fcycle > 5.0) {
        fcycle = 0.0;
        rswtch = to_float(floor(rand() * 3.0));
    }
    if (bcycle > 2.0) {
        bcycle = 0.0;
        rswtch = to_float(floor(rand() * 3.0));
    }
    let weight = ctx.weight;
    let safe_w = (if abs(weight) < 1.0e-30 { 1.0e-30 } else { weight });
    let scale = scale_param * 0.5;
    let ox = 0.0;
    let oy = 0.0;
    if (rswtch <= 1.0) {
        let loc = u32(clamp(fcycle, 0.0, 5.0));
        let v = hex_seg60_2d(loc);
        ox = (ctx.accum.re * (scale - 1.0) + z.re * scale) / safe_w + v.re;
        oy = (ctx.accum.im * (scale - 1.0) + z.im * scale) / safe_w + v.im;
        fcycle = fcycle + 1.0;
    } else {
        let loc = u32(clamp(bcycle, 0.0, 2.0));
        let v = hex_seg120_2d(loc);
        ox = (ctx.accum.re * (scale - 1.0) + z.re * scale) / safe_w + v.re;
        oy = (ctx.accum.im * (scale - 1.0) + z.im * scale) / safe_w + v.im;
        bcycle = bcycle + 1.0;
    }
    this.rswtch = rswtch;
    this.fcycle = fcycle;
    this.bcycle = bcycle;
    complex(ox, oy)
}

fn variation3(q, p, ctx) {
    let majp = p.majp;
    let scale_param = p.scale;
    let zlift = p.zlift;
    let rswtch = this.rswtch;
    let fcycle = this.fcycle;
    let bcycle = this.bcycle;
    if (fcycle > 5.0) {
        fcycle = 0.0;
        rswtch = to_float(floor(rand() * 3.0));
    }
    if (bcycle > 2.0) {
        bcycle = 0.0;
        rswtch = to_float(floor(rand() * 3.0));
    }
    let weight = ctx.weight;
    let safe_w = (if abs(weight) < 1.0e-30 { 1.0e-30 } else { weight });
    let scale = scale_param * 0.5;
    let pos_neg = 1.0;
    if (rand() < 0.5) {
        pos_neg = -1.0;
    }
    let abmajp = abs(majp);
    let oz_extra = 0.0;
    if (abmajp > 1.0) {
        let boost = (abmajp - 1.0) * 0.5;
        oz_extra = pos_neg * boost;
    }
    let ox = 0.0;
    let oy = 0.0;
    if (rswtch <= 1.0) {
        let loc = u32(clamp(fcycle, 0.0, 5.0));
        let v = hex_seg60_3d(loc);
        ox = (ctx.accum.x * (scale - 1.0) + q.x * scale) / safe_w + v.re;
        oy = (ctx.accum.y * (scale - 1.0) + q.y * scale) / safe_w + v.im;
        fcycle = fcycle + 1.0;
    } else {
        let loc = u32(clamp(bcycle, 0.0, 2.0));
        let v = hex_seg120_3d(loc);
        ox = (ctx.accum.x * (scale - 1.0) + q.x * scale) / safe_w + v.re;
        oy = (ctx.accum.y * (scale - 1.0) + q.y * scale) / safe_w + v.im;
        bcycle = bcycle + 1.0;
    }
    let oz = (q.z * 0.5 * zlift + oz_extra) / safe_w;
    this.rswtch = rswtch;
    this.fcycle = fcycle;
    this.bcycle = bcycle;
    vec3(ox, oy, oz)
}

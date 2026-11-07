/// Voronoi metaball-field stencil mask (hash-jittered cells).
///
/// # Authors
/// - Jesus Sosa

// Cut MetaBalls, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    zoom: #{
        value: 7.0, min: 0.1, max: 50.0,
        caption: "Zoom",
        hint: "Pattern scale (cell count): `point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
    time: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Time",
        hint: "Animates the per-cell metaball jitter.",
    },
};

const INFO = #{ title: "Cut MetaBalls", name: "rhai_cut_metaballs", category: "plugin", phase: "any", replace: true };

fn cut_metaballs_random2(pp) {
    let t1 = complex(127.1, 311.7);
    let t2 = complex(269.5, 183.3);
    let tm = complex((pp.re * t1.re + pp.im * t1.im), (pp.re * t2.re + pp.im * t2.im));
    fract(complex(sin(tm.re), sin(tm.im)) * 43758.5453)
}

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let time = p.time;
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
    let st = complex(x * zoom, y * zoom);
    let i_st = floor(st);
    let f_st = fract(st);
    let m_dist = 1.0;
    for j in -1..=1 {
        for i in -1..=1 {
            let neighbor = complex(to_float(i), to_float(j));
            let offset = cut_metaballs_random2(i_st + neighbor);
            let t1 = offset * 6.2831;
            let tmp = complex(t1.re + time, t1.im + time);
            let t2 = complex(sin(tmp.re), sin(tmp.im)) * 0.5;
            offset = complex(t2.re + 0.5, t2.im + 0.5);
            let pos = neighbor + offset - f_st;
            let dist = pos.abs();
            m_dist = min(m_dist, m_dist * dist);
        }
    }
    let color = (if m_dist < 0.060 { 0.0 } else { 1.0 });
    let hidden = false;
    if (invert == 0) {
        if (color == 0.0) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (color > 0.0) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x - cx, y - cy), hide: hide_v }
}

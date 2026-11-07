/// Pre-phase Z displacement driven by `*vc` — same formula as
/// `dc_ztransl` but runs in the PRE phase (modifies the input to the
/// normal-phase variation chain rather than contributing to the
/// accumulator). cpp/Java replace `pAffineTP.{x,y,z}` outright with
/// weighted values; in our pre-phase model the body returns the new
/// input directly.
///
/// # Authors
/// - Xyrus02

// Pre DC Z Translation, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x0: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "X0",
        hint: "Lower edge of the color-register input range.",
    },
    x1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X1",
        hint: "Upper edge of the color-register input range.",
    },
    factor: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Factor",
        hint: "Multiplier on the normalized `zf` value.",
    },
    overwrite: #{
        value: true,
        caption: "Overwrite",
        hint: "When on, Z = `w · zf`. When off, Z = `w · input_z · zf`.",
    },
    clamp: #{
        value: false,
        caption: "Clamp",
        hint: "When on, `zf` is clamped to `[0, 1]`.",
    },
};

const INFO = #{ title: "Pre DC Z Translation", name: "rhai_pre_dcztransl", category: "full_3d", phase: "pre", z: "preserve_z" };

fn prepare(p) {
    let lo = min(p.x0, p.x1);
    let hi = max(p.x0, p.x1);
    let span = hi - lo;
    #{ x1_m_x0: (if span == 0.0 { 1.0e-30 } else { span }) }
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let w = ctx.weight;
    #{ z: complex(w * z.re, w * z.im), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let x0_u = p.x0;
    let x1_u = p.x1;
    let factor = p.factor;
    let overwrite = to_int(to_float(p.overwrite));
    let clamp_on = to_int(to_float(p.clamp));
    let x1_m_x0 = p.x1_m_x0;
    let w = ctx.weight;
    let lo = min(x0_u, x1_u);
    let tc = color;
    let zf = factor * (tc - lo) / x1_m_x0;
    if (clamp_on != 0) {
        zf = clamp(zf, 0.0, 1.0);
    }
    let nz = (if overwrite == 0 { w * v.z * zf } else { w * zf });
    #{ z: vec3(w * v.x, w * v.y, nz), color: color }
}

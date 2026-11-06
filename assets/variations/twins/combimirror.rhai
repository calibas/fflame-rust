/// Combined mirror — scales the affine input by the variation weight, then
/// applies four independent random mirror branches (point, vertical,
/// horizontal, Z), each negating its axis about a movable centre and
/// shifting the colour register by a per-branch amount. `*mirror`
/// parameters act as probabilities in `[0, 2]` (the branch fires with
/// probability `mirror/2`).
/// 
/// # Authors
/// - Thomas Michels
/// - Brad Stefanov

// Combimirror, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    vmirror: #{
        value: 1.0, min: 0.0, max: 2.0, free: true,
        caption: "V Mirror",
        hint: "Vertical-mirror probability (fires when a random < vmirror/2). Negates X about `vmove`.",
    },
    vmove: #{
        value: 0.05, min: -10.0, max: 10.0, free: true,
        caption: "V Move",
        hint: "X offset added after the vertical mirror: `x → -x + vmove`.",
    },
    hmirror: #{
        value: 0.5, min: 0.0, max: 2.0, free: true,
        caption: "H Mirror",
        hint: "Horizontal-mirror probability (fires when a random < hmirror/2). Negates Y about `hmove`.",
    },
    hmove: #{
        value: 0.35, min: -10.0, max: 10.0, free: true,
        caption: "H Move",
        hint: "Y offset added after the horizontal mirror: `y → -y + hmove`.",
    },
    zmirror: #{
        value: 0.0, min: 0.0, max: 2.0, free: true,
        caption: "Z Mirror",
        hint: "Z-mirror probability (fires when a random < zmirror/2). Negates Z about `zmove`. 3D only.",
    },
    zmove: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Z Move",
        hint: "Z offset added after the Z mirror: `z → -z + zmove`.",
    },
    pmirror: #{
        value: 0.0, min: 0.0, max: 2.0, free: true,
        caption: "P Mirror",
        hint: "Point-mirror probability — note this branch fires when a random > pmirror/2 (so 0 = always, 2 = never). Negates X and Y about (pmovex, pmovey).",
    },
    pmovex: #{
        value: 0.05, min: -10.0, max: 10.0, free: true,
        caption: "P Move X",
        hint: "X offset added after the point mirror: `x → -x + pmovex`.",
    },
    pmovey: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "P Move Y",
        hint: "Y offset added after the point mirror: `y → -y + pmovey`.",
    },
    vcolorshift: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "V Color Shift",
        hint: "Colour-register shift applied when the vertical mirror fires. Visible colour requires the transform's Direct Color slider > 0.",
    },
    hcolorshift: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "H Color Shift",
        hint: "Colour-register shift applied when the horizontal mirror fires.",
    },
    zcolorshift: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Z Color Shift",
        hint: "Colour-register shift applied when the Z mirror fires.",
    },
    pcolorshift: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "P Color Shift",
        hint: "Colour-register shift applied when the point mirror fires.",
    },
};

const INFO = #{ title: "Combimirror", name: "rhai_combimirror", category: "full_3d", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let color = ctx.color;
    let vmirror = p.vmirror;
    let vmove = p.vmove;
    let hmirror = p.hmirror;
    let hmove = p.hmove;
    let pmirror = p.pmirror;
    let pmovex = p.pmovex;
    let pmovey = p.pmovey;
    let vcolorshift = p.vcolorshift;
    let hcolorshift = p.hcolorshift;
    let pcolorshift = p.pcolorshift;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let px = w * z.re;
    let py = w * z.im;
    if (rand() > pmirror * 0.5) {
        px = -px + pmovex;
        py = -py + pmovey;
        let c = color + pcolorshift;
        color = c - to_float(trunc(c));
    }
    if (rand() < vmirror * 0.5) {
        px = -px + vmove;
        let c = color + vcolorshift;
        color = c - to_float(trunc(c));
    }
    if (rand() < hmirror * 0.5) {
        py = -py + hmove;
        let c = color + hcolorshift;
        color = c - to_float(trunc(c));
    }
    let _z_draw = rand();
    #{ z: complex(px * inv_w, py * inv_w), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let vmirror = p.vmirror;
    let vmove = p.vmove;
    let hmirror = p.hmirror;
    let hmove = p.hmove;
    let zmirror = p.zmirror;
    let zmove = p.zmove;
    let pmirror = p.pmirror;
    let pmovex = p.pmovex;
    let pmovey = p.pmovey;
    let vcolorshift = p.vcolorshift;
    let hcolorshift = p.hcolorshift;
    let zcolorshift = p.zcolorshift;
    let pcolorshift = p.pcolorshift;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let px = w * v.x;
    let py = w * v.y;
    let pz = w * v.z;
    if (rand() > pmirror * 0.5) {
        px = -px + pmovex;
        py = -py + pmovey;
        let c = color + pcolorshift;
        color = c - to_float(trunc(c));
    }
    if (rand() < vmirror * 0.5) {
        px = -px + vmove;
        let c = color + vcolorshift;
        color = c - to_float(trunc(c));
    }
    if (rand() < hmirror * 0.5) {
        py = -py + hmove;
        let c = color + hcolorshift;
        color = c - to_float(trunc(c));
    }
    if (rand() < zmirror * 0.5) {
        pz = -pz + zmove;
        let c = color + zcolorshift;
        color = c - to_float(trunc(c));
    }
    #{ z: vec3(px * inv_w, py * inv_w, pz * inv_w), color: color }
}

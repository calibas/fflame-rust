/// Post-phase per-axis mirroring — for each enabled axis (`xaxis`, `yaxis`,
/// `zaxis`), with 50% probability per iteration flips the corresponding
/// output coordinate via `coord = −scale · (coord + shift)`. Each axis's
/// RNG draw is independent, so multiple axes can flip in the same
/// iteration. Each firing branch also shifts the color register by the
/// corresponding `*colorshift` (Java: `pVarTP.color = fmod(pVarTP.color +
/// shift, 1)`), letting mirrored copies take different palette regions.

// Post Mirror WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    xaxis: #{
        value: true,
        caption: "X Axis",
        hint: "Enable the X-axis mirror branch.",
    },
    yaxis: #{
        value: false,
        caption: "Y Axis",
        hint: "Enable the Y-axis mirror branch.",
    },
    zaxis: #{
        value: false,
        caption: "Z Axis",
        hint: "Enable the Z-axis mirror branch. 3D only (the color shift still applies in 2D).",
    },
    xshift: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "X Shift",
        hint: "X mirror offset — applied as `-scale·(x + xshift)` when the X branch fires.",
    },
    yshift: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Shift",
        hint: "Y mirror offset.",
    },
    zshift: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Z Shift",
        hint: "Z mirror offset.",
    },
    xscale: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X Scale",
        hint: "X output scale (applied in both the X-branch and Y-branch paths).",
    },
    yscale: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y Scale",
        hint: "Y output scale (applied in both branch paths).",
    },
    xcolorshift: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "X Color Shift",
        hint: "Color-register shift applied when the X mirror branch fires. Visible color requires the transform's Direct Color slider > 0.",
    },
    ycolorshift: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Y Color Shift",
        hint: "Color-register shift applied when the Y mirror branch fires.",
    },
    zcolorshift: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Z Color Shift",
        hint: "Color-register shift applied when the Z mirror branch fires.",
    },
};

const INFO = #{ title: "Post Mirror WF", name: "rhai_post_mirror_wf", category: "full_3d", phase: "post" };

fn variation(z, p, ctx) {
    let color = ctx.color;
    let xaxis = to_int(to_float(p.xaxis));
    let yaxis = to_int(to_float(p.yaxis));
    let zaxis = to_int(to_float(p.zaxis));
    let xshift = p.xshift;
    let yshift = p.yshift;
    let xscale = p.xscale;
    let yscale = p.yscale;
    let x = z.re;
    let y = z.im;
    if (xaxis > 0 && rand() < 0.5) {
        x = xscale * (-x - xshift);
        y = yscale * y;
        let craw = color + p.xcolorshift;
        color = craw - to_float(trunc(craw));
    }
    if (yaxis > 0 && rand() < 0.5) {
        x = xscale * x;
        y = yscale * (-y - yshift);
        let craw = color + p.ycolorshift;
        color = craw - to_float(trunc(craw));
    }
    if (zaxis > 0 && rand() < 0.5) {
        let craw = color + p.zcolorshift;
        color = craw - to_float(trunc(craw));
    }
    #{ z: complex(x, y), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let xaxis = to_int(to_float(p.xaxis));
    let yaxis = to_int(to_float(p.yaxis));
    let zaxis = to_int(to_float(p.zaxis));
    let xshift = p.xshift;
    let yshift = p.yshift;
    let zshift = p.zshift;
    let xscale = p.xscale;
    let yscale = p.yscale;
    let x = v.x;
    let y = v.y;
    let z = v.z;
    if (xaxis > 0 && rand() < 0.5) {
        x = xscale * (-x - xshift);
        y = yscale * y;
        let craw = color + p.xcolorshift;
        color = craw - to_float(trunc(craw));
    }
    if (yaxis > 0 && rand() < 0.5) {
        x = xscale * x;
        y = yscale * (-y - yshift);
        let craw = color + p.ycolorshift;
        color = craw - to_float(trunc(craw));
    }
    if (zaxis > 0 && rand() < 0.5) {
        z = -z - zshift;
        let craw = color + p.zcolorshift;
        color = craw - to_float(trunc(craw));
    }
    #{ z: vec3(x, y, z), color: color }
}

/// Quasiconformal warp: an exact log-affine map or a spatially-varying
/// Beltrami field.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Quasiconformal, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    power: #{
        value: 1.0, min: -4.0, max: 4.0, free: true,
        caption: "Power",
        hint: "Exact mode: real part of the conformal exponent a — the radial power (z ↦ z^power). Field mode: angular winding of the stretch orientation (how many times the ellipse axis turns going once around the center).",
    },
    spiral: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "Spiral",
        hint: "Exact mode: imaginary part of a — couples radius into angle so the map spirals (the logarithmic-spiral part of z ↦ z^a). Field mode: radial winding of the stretch orientation (turns per e-fold of radius).",
    },
    dilation: #{
        value: 0.35, min: 0.0, max: 0.98,
        caption: "Dilation",
        hint: "The quasiconformal strength k = |μ/a|, the Beltrami modulus. 0 = conformal (a pure power-spiral, circles stay circles); higher values squash circles into ellipses of dilatation K = (1+k)/(1−k). Clamped below 1 (k→1 is the degenerate non-invertible limit).",
    },
    stretch_angle: #{
        value: 0.0, min: -360.0, max: 360.0, type: "angle",
        caption: "Stretch Angle",
        hint: "Base orientation ψ of the quasiconformal stretch, in degrees — the direction along which infinitesimal circles are stretched into ellipses (the axis rotates around the plane on top of this base).",
    },
    cx: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Center X",
        hint: "X of the fixed center: the map is taken about (cx, cy). In Exact mode the center and infinity are the two fixed points.",
    },
    cy: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Center Y",
        hint: "Y of the fixed center.",
    },
    mode: #{
        value: 0, choices: ["Exact", "Field"],
        caption: "Mode",
        hint: "Exact: the closed-form log-affine map — a genuine global quasiconformal map with uniform dilatation (Teichmüller-type). Field: the pointwise Beltrami deformation with a spatially-varying stretch orientation (power/spiral become its angular/radial winding) — a locally-QC swirl, not an exact global map.",
    },
    dc_mode: #{
        value: 0, choices: ["Off", "Orientation", "Phase"],
        caption: "Color Mode",
        hint: "Direct-color source (needs the transform's Direct Color > 0). Orientation: palette by the local ellipse-axis angle (arg of the Beltrami coefficient) — the QC-specific quantity. Phase: palette by the accumulated spiral phase of the output.",
    },
    dc_scale: #{
        value: 1.0, min: 0.1, max: 8.0,
        caption: "Color Scale",
        hint: "Palette-index multiplier for the color modes (wrapped with fract).",
    },
};

const INFO = #{ title: "Quasiconformal", name: "rhai_quasiconformal", category: "advanced_2d" };

fn variation(z, p, ctx) {
    let color = ctx.color;
    let qc_in = z;
    let power = p.power;
    let spiral = p.spiral;
    let k = clamp(p.dilation, 0.0, 0.98);
    let psi = p.stretch_angle * 0.01745329252;
    let cx = p.cx;
    let cy = p.cy;
    let mode = u32(to_float(p.mode));
    let dc_mode = u32(to_float(p.dc_mode));
    let dc_scale = p.dc_scale;
    let zx = qc_in.re - cx;
    let zy = qc_in.im - cy;
    let big_l = 0.5 * ln(max(1.0e-20, zx * zx + zy * zy));
    let theta = atan2(zy, zx);
    let outx = 0.0;
    let outy = 0.0;
    let beta = 0.0;
    let phase_out = 0.0;
    if (mode == u32(0)) {
        let cps = cos(psi);
        let sps = sin(psi);
        let mr = k * (power * cps - spiral * sps);
        let mi = k * (power * sps + spiral * cps);
        let re_omega = (power + mr) * big_l + (mi - spiral) * theta;
        let im_omega = (spiral + mi) * big_l + (power - mr) * theta;
        let rr = exp(re_omega);
        outx = cx + rr * cos(im_omega);
        outy = cy + rr * sin(im_omega);
        beta = psi + 2.0 * theta;
        phase_out = im_omega;
    } else {
        let ph = power * theta + spiral * big_l + psi;
        let mur = k * cos(ph);
        let mui = k * sin(ph);
        outx = cx + zx + (mur * zx + mui * zy);
        outy = cy + zy + (mui * zx - mur * zy);
        beta = ph;
        phase_out = atan2(outy - cy, outx - cx);
    }
    if (dc_mode == u32(1)) {
        color = fract(beta * 0.15915494309 * dc_scale);
    } else if (dc_mode == u32(2)) {
        color = fract(phase_out * 0.15915494309 * dc_scale);
    }
    #{ z: complex(outx, outy), color: color }
}

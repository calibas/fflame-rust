/// 3D swirl with Z modulation — re-emits the input radius and angle in
/// cartesian form (XY output passes the input through unchanged), plus a Z
/// output `sin(6·cos(rad) − n·ang)` that introduces a sinusoidal Z
/// modulation parameterized by `n`. Matches JWildfire's `Swirl3DWFFunc`
/// (uses `getPrecalcAtanYX()` = `atan2(y, x)`).
///
/// Also a direct-color variation: JWF writes `pVarTP.color =
/// |sin(6·cos(rad) − n·ang)|` UNCONDITIONALLY — the same value as the
/// Z output, which is why its palette stripes track Z position. The
/// write was originally skipped citing the batch-60 spirograph3D
/// compromise, but that compromise concerned CONDITIONAL color
/// writes; an unconditional `*vc` write is exactly what
/// `Feature::WritesColor` models, and `direct_color` defaults to 1.0
/// so JWF's replace-the-color semantics hold for imported flames
/// (JWF .flame files carry no pluginColor attribute).

// Swirl 3D WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    n: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "N",
        hint: "Angular multiplier on the Z-output sine: `sin(6·cos(rad) − n·ang)`. Also drives the direct-color stripes (color = |z output|).",
    },
};

const INFO = #{ title: "Swirl 3D WF", name: "rhai_swirl3d_wf", category: "full_3d", phase: "any" };

fn variation(z, p, ctx) {
    let color = ctx.color;
    let n = p.n;
    let small = 1.0e-30;
    let rad = sqrt(z.re * z.re + z.im * z.im) + small;
    let ang = atan2(z.im, z.re);
    color = abs(sin(6.0 * cos(rad) - n * ang));
    #{ z: complex(rad * cos(ang), rad * sin(ang)), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let n = p.n;
    let small = 1.0e-30;
    let rad = sqrt(v.x * v.x + v.y * v.y) + small;
    let ang = atan2(v.y, v.x);
    let z_out = sin(6.0 * cos(rad) - n * ang);
    color = abs(z_out);
    #{ z: vec3(rad * cos(ang), rad * sin(ang), z_out), color: color }
}

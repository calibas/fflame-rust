/// 3D spherical with per-axis scales — emits `(x, y, z) / (x_scale²·x² +
/// y_scale²·y² + zz² + ε)` where `zz = x_scale · z` (upstream typo:
/// cpp+Java both use `x_scale` instead of `z_scale` in the Z denominator
/// term, preserved). Per-axis scales tune the asymmetry of the inverse-
/// distance scaling.
///
/// # Authors
/// - Xyrus02

// Sph 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X scale",
        hint: "X-axis scale on the inverse-distance denominator.",
    },
    y: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y scale",
        hint: "Y-axis scale.",
    },
    z: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Z scale",
        hint: "Z-axis scale — note: due to an upstream typo the body actually uses `x_scale` for `zz`, so this parameter is effectively unused. Preserved for preset compatibility.",
    },
};

const INFO = #{ title: "Sph 3D", name: "rhai_sph3d", category: "full_3d", phase: "any" };

fn variation(z, p) {
    let x_scale = p.x;
    let y_scale = p.y;
    let xx = x_scale * z.re;
    let yy = y_scale * z.im;
    let denom = max(1.0e-30, xx * xx + yy * yy);
    let r = 1.0 / denom;
    complex(z.re * r, z.im * r)
}

fn variation3(v, p) {
    let x_scale = p.x;
    let y_scale = p.y;
    let xx = x_scale * v.x;
    let yy = y_scale * v.y;
    let zz = x_scale * v.z;
    let denom = max(1.0e-30, xx * xx + yy * yy + zz * zz);
    let r = 1.0 / denom;
    vec3(v.x * r, v.y * r, v.z * r)
}

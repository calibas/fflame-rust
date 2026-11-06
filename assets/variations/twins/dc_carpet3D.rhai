/// 3D Sierpinski-carpet IFS with direct-color writes and color-coupled Z.
///
/// Picks one of the four unit-square corners by independent ±1 sign
/// choices on X and Y (with `stretch_x` / `stretch_y` controlling the
/// magnitude of the corner offset), reapplies the transform's affine
/// matrix to the offset position, and scales each axis by `scale_x` /
/// `scale_y`. Color is updated via a fmod mixing rule combining the
/// previous color value with `color_a..color_f` and the parity of the
/// corner choice; the new color then drives Z via
/// `dz = color · scale_z + offset_z`, with `reset_z > 0` switching
/// Z-accumulation off so the color-derived `dz` becomes Z directly.
///
/// Visible color contribution requires the transform's Direct Color
/// slider to be > 0 (the per-iteration final color is
/// `c_base + direct_color · (vc − c_base)`).
///
/// # Authors
/// - Xyrus02
/// - Brad Stefanov

// DC Carpet 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    origin: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Origin",
        hint: "Scales the per-iteration color-mix amplitude (precomputed as `H = 0.1 · origin`).",
    },
    color_a: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Color A",
        hint: "Multiplier on the previous color value in the color mix.",
    },
    color_b: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Color B",
        hint: "Bias term in `hh = H · (color_b − x0⊕y0 − 1)`.",
    },
    color_c: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Color C",
        hint: "Bias added to `hh` before multiplying by the previous color.",
    },
    color_d: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Color D",
        hint: "Bias subtracted from `hh` before multiplying by the `x0⊕y0` parity term.",
    },
    color_e: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "Color E",
        hint: "Multiplier on the parity-term contribution to the color mix.",
    },
    color_f: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Color F",
        hint: "fmod modulus on the final color value — controls the color repeat period.",
    },
    stretch_x: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Stretch X",
        hint: "Magnitude of the ±1 X corner offset added to the input. Larger pulls corners further apart along X.",
    },
    stretch_y: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Stretch Y",
        hint: "Magnitude of the ±1 Y corner offset added to the input.",
    },
    scale_x: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale X",
        hint: "Post-affine X scale applied to the carpet position.",
    },
    scale_y: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale Y",
        hint: "Post-affine Y scale applied to the carpet position.",
    },
    scale_z: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale Z",
        hint: "Color-to-Z coupling multiplier — Z output includes `color · scale_z`.",
    },
    offset_z: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Offset Z",
        hint: "Constant Z bump added to the color-driven Z term per iteration.",
    },
    reset_z: #{
        value: 0.0, min: 0.0, max: 1.0, free: true,
        caption: "Reset Z",
        hint: "When > 0, the color-derived `dz` overrides Z rather than accumulating onto the pre-existing Z.",
    },
};

const INFO = #{ title: "DC Carpet 3D", name: "rhai_dc_carpet3d", category: "full_3d", phase: "any", z: "preserve_z" };

fn prepare(p) {
    #{ h: 0.1 * p.origin }
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let color_a = p.color_a;
    let color_b = p.color_b;
    let color_c = p.color_c;
    let color_d = p.color_d;
    let color_e = p.color_e;
    let color_f = p.color_f;
    let stretch_x = p.stretch_x;
    let stretch_y = p.stretch_y;
    let scale_x = p.scale_x;
    let scale_y = p.scale_y;
    let h = p.h;
    let x0_i = (if rand() < 0.5 { -1 } else { 1 });
    let y0_i = (if rand() > 0.5 { -1 } else { 1 });
    let x0 = to_float(x0_i);
    let y0 = to_float(y0_i);
    let x0_xor_y0 = to_float(x0_i ^ y0_i);
    let hh = h * (color_b - x0_xor_y0 - 1.0);
    let current_vc = color;
    let mod_div = (if abs(color_f) < 1.0e-30 { 1.0e-30 } else { color_f });
    let raw = abs(current_vc * color_a * (color_c + hh) + x0_xor_y0 * (color_d - hh) * color_e);
    color = raw - to_float(floor(raw / mod_div)) * mod_div;
    let x = z.re + x0 * stretch_x;
    let y = z.im + y0 * stretch_y;
    let nx = (ctx.a * x + ctx.b * y + ctx.e) * scale_x;
    let ny = (ctx.c * x + ctx.d * y + ctx.f) * scale_y;
    #{ z: complex(nx, ny), color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let color_a = p.color_a;
    let color_b = p.color_b;
    let color_c = p.color_c;
    let color_d = p.color_d;
    let color_e = p.color_e;
    let color_f = p.color_f;
    let stretch_x = p.stretch_x;
    let stretch_y = p.stretch_y;
    let scale_x = p.scale_x;
    let scale_y = p.scale_y;
    let scale_z = p.scale_z;
    let offset_z = p.offset_z;
    let reset_z = p.reset_z;
    let h = p.h;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let x0_i = (if rand() < 0.5 { -1 } else { 1 });
    let y0_i = (if rand() > 0.5 { -1 } else { 1 });
    let x0 = to_float(x0_i);
    let y0 = to_float(y0_i);
    let x0_xor_y0 = to_float(x0_i ^ y0_i);
    let hh = h * (color_b - x0_xor_y0 - 1.0);
    let current_vc = color;
    let mod_div = (if abs(color_f) < 1.0e-30 { 1.0e-30 } else { color_f });
    let raw = abs(current_vc * color_a * (color_c + hh) + x0_xor_y0 * (color_d - hh) * color_e);
    let new_vc = raw - to_float(floor(raw / mod_div)) * mod_div;
    color = new_vc;
    let dz = new_vc * scale_z + offset_z;
    let nz = (if reset_z > 0.0 { dz * inv_w } else { v.z + dz * inv_w });
    let x = v.x + x0 * stretch_x;
    let y = v.y + y0 * stretch_y;
    let nx = (ctx.a * x + ctx.b * y + ctx.e) * scale_x;
    let ny = (ctx.c * x + ctx.d * y + ctx.f) * scale_y;
    #{ z: vec3(nx, ny, nz), color: color }
}

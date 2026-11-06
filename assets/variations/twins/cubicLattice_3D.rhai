/// 8-corner cubic lattice — a simpler relative of [`CUBIC3D`]. Same
/// 8-vertex node scheme each iteration, but the core blend is a
/// straight `(accum + p) × fill × angular + sign × w` rather than
/// `cubic3D`'s smoothed mix. The `style` param is a two-stage
/// switch instead of a continuous warp: `|style| ≥ 2` turns on the
/// angular `cos(atan2(x,z))` / `sin(atan2(y,z))` weights; anything
/// below that pins all three weights to 1 (pure cube). `xpand`
/// controls vertex pull (default 0.2) with the same linear / `sqrt`
/// ramp as `cubic3D`. Lattice spacing scales with the variation
/// weight (`lattd = w` here, vs `w × 0.5` in `cubic3D`).
///
/// # Authors
/// - Larry Berlin

// Cubic Lattice 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    xpand: #{
        value: 0.2, min: -10.0, max: 10.0, free: true,
        caption: "Xpand",
        hint: "How aggressively each step is pulled toward a cube vertex. Linear (`fill = xpand × 0.5`) up through ±1, `sqrt` past that.",
    },
    style: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Style",
        hint: "Two-stage switch on the angular weights. `|style| ≥ 2` enables per-axis `cos(atan2(x, z))` / `sin(atan2(y, z))` warps that bias the lattice toward Z; anything below that pins the angular weights at 1 (pure cube).",
    },
};

const INFO = #{ title: "Cubic Lattice 3D", name: "rhai_cubiclattice_3d", category: "full_3d" };

fn variation(z, p, ctx) {
    let xpand = p.xpand;
    let style = p.style;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let use_node = u32(rand() * 8.0);
    let sign_x = (if (use_node & u32(4)) != u32(0) { -1.0 } else { 1.0 });
    let sign_y = (if (use_node & u32(1)) != u32(0) { -1.0 } else { 1.0 });
    let fill = (if abs(xpand) <= 1.0 { xpand * 0.5 } else { sqrt(abs(xpand)) * 0.5 });
    let exnze = 1.0;
    let wynze = 1.0;
    if (abs(style) >= 2.0) {
        exnze = 0.0;
        let t1 = z.im;
        wynze = (if t1 > 0.0 { 1.0 } else if t1 < 0.0 { -1.0 } else { 0.0 });
    }
    let return_x = ((ctx.accum.re + z.re) * fill * exnze - ctx.accum.re) * inv_w + sign_x;
    let return_y = ((ctx.accum.im + z.im) * fill * wynze - ctx.accum.im) * inv_w + sign_y;
    complex(return_x, return_y)
}

fn variation3(v, p, ctx) {
    let xpand = p.xpand;
    let style = p.style;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let use_node = u32(rand() * 8.0);
    let sign_x = (if (use_node & u32(4)) != u32(0) { -1.0 } else { 1.0 });
    let sign_y = (if (use_node & u32(1)) != u32(0) { -1.0 } else { 1.0 });
    let sign_z = (if (use_node & u32(2)) != u32(0) { -1.0 } else { 1.0 });
    let fill = (if abs(xpand) <= 1.0 { xpand * 0.5 } else { sqrt(abs(xpand)) * 0.5 });
    let exnze = 1.0;
    let wynze = 1.0;
    let znxy = 1.0;
    if (abs(style) >= 2.0) {
        exnze = cos(atan2(v.x, v.z));
        wynze = sin(atan2(v.y, v.z));
        znxy = (exnze + wynze) * 0.5;
    }
    let return_x = ((ctx.accum.x + v.x) * fill * exnze - ctx.accum.x) * inv_w + sign_x;
    let return_y = ((ctx.accum.y + v.y) * fill * wynze - ctx.accum.y) * inv_w + sign_y;
    let return_z = ((ctx.accum.z + v.z) * fill * znxy - ctx.accum.z) * inv_w + sign_z;
    vec3(return_x, return_y, return_z)
}

/// 8-corner cubic lattice with smooth-blended angular mixing — a
/// chaos-game scatter that biases each step toward one of the unit
/// cube's vertices, then blends it back toward the original point
/// based on a per-axis angular weight derived from the input
/// direction. `xpand` controls how aggressively each step pulls
/// toward the cube vertex (default 0.25, with a `sqrt(xpand)` ramp
/// once `|xpand| > 1`). `style` warps the per-axis angular weights:
/// 0 leaves them at 1 (pure cube), positive values introduce a
/// `1 - cos(atan2(x, z))` warp that softens the lattice along Z,
/// and `|style| > 1` further compresses the response.
///
/// # Authors
/// - Larry Berlin

// Cubic 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    xpand: #{
        value: 0.25, min: -10.0, max: 10.0, free: true,
        caption: "Xpand",
        hint: "How aggressively each step is pulled toward a cube vertex. Up through ±1 the response is linear (`fill = xpand × 0.5`); past that it follows `sqrt(xpand) × 0.5` so the lattice doesn't blow out at large values.",
    },
    style: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Style",
        hint: "Angular warp on the per-axis weights derived from `cos(atan2(x, z))` and `sin(atan2(y, z))`. 0 disables the warp (per-axis weights are exactly 1); positive values soften the lattice along Z. `|style| > 1` introduces a secondary compression on the average XY/Z response.",
    },
};

const INFO = #{ title: "Cubic 3D", name: "rhai_cubic3d", category: "full_3d" };

fn variation(z, p, ctx) {
    let xpand = p.xpand;
    let style = p.style;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let use_node = u32(rand() * 8.0);
    let sign_x = (if (use_node & u32(4)) != u32(0) { -1.0 } else { 1.0 });
    let sign_y = (if (use_node & u32(1)) != u32(0) { -1.0 } else { 1.0 });
    let smoothed = (if abs(w) <= 0.5 { w * 2.0 } else { 1.0 });
    let smooth_style = style;
    if (abs(style) > 1.0) {
        smooth_style = (if style > 1.0 { 1.0 + (style - 1.0) * 0.25 } else { (style + 1.0) * 0.25 - 1.0 });
    }
    let fill = (if abs(xpand) <= 1.0 { xpand * 0.5 } else { sqrt(abs(xpand)) * 0.5 });
    let one_minus_fill = 1.0 - fill;
    let exnze = 1.0 - smooth_style;
    let t1 = z.im;
    let wynze = 1.0 - smooth_style * (1.0 - (if t1 > 0.0 { 1.0 } else if t1 < 0.0 { -1.0 } else { 0.0 }));
    let return_x = (-ctx.accum.re * smoothed * one_minus_fill * exnze + z.re * smoothed * fill * exnze) * inv_w + sign_x * 0.5;
    let return_y = (-ctx.accum.im * smoothed * one_minus_fill * wynze + z.im * smoothed * fill * wynze) * inv_w + sign_y * 0.5;
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
    let smoothed = (if abs(w) <= 0.5 { w * 2.0 } else { 1.0 });
    let smooth_style = style;
    if (abs(style) > 1.0) {
        smooth_style = (if style > 1.0 { 1.0 + (style - 1.0) * 0.25 } else { (style + 1.0) * 0.25 - 1.0 });
    }
    let fill = (if abs(xpand) <= 1.0 { xpand * 0.5 } else { sqrt(abs(xpand)) * 0.5 });
    let one_minus_fill = 1.0 - fill;
    let exnze = 1.0 - smooth_style * (1.0 - cos(atan2(v.x, v.z)));
    let wynze = 1.0 - smooth_style * (1.0 - sin(atan2(v.y, v.z)));
    let znxy_inner = (exnze + wynze) * 0.5;
    let znxy = (if smooth_style > 1.0 { 1.0 - smooth_style * (1.0 - znxy_inner * smooth_style) } else { 1.0 - smooth_style * (1.0 - znxy_inner) });
    let return_x = (-ctx.accum.x * smoothed * one_minus_fill * exnze + v.x * smoothed * fill * exnze) * inv_w + sign_x * 0.5;
    let return_y = (-ctx.accum.y * smoothed * one_minus_fill * wynze + v.y * smoothed * fill * wynze) * inv_w + sign_y * 0.5;
    let return_z = (-ctx.accum.z * smoothed * one_minus_fill * znxy + v.z * smoothed * fill * znxy) * inv_w + sign_z * 0.5;
    vec3(return_x, return_y, return_z)
}

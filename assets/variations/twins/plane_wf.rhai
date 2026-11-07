/// Plane scatter — blur-style variation that ignores its input `p`
/// and draws a random point on an axis-aligned plane. Two of the
/// three coordinates come from `(rand − 0.5) × size`; the third is
/// fixed at `position` (which axis is the "fixed" one is controlled
/// by the `axis` param: XY plane → fixed Z, YZ plane → fixed X, ZX
/// plane → fixed Y, default ZX). Optionally writes a direct color
/// scalar to the iteration's `vc` register derived from the two
/// random coordinates (modes U / V / UV map to the first random,
/// second random, or their product); `direct_color = 0` disables
/// the write.
///
/// JWildfire's image-related modes (`CM_COLORMAP`, image
/// displacement, `calc_color_idx`, `receive_only_shadows`) require
/// texture sampling we don't support and are silently no-ops here —
/// the `colormap` mode falls back to the `U` color path so a flame
/// that requested an image colormap still renders something.
///
/// # Authors
/// - Andreas Maschke

// Plane WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    position: #{
        value: 3.0, min: -100.0, max: 100.0, free: true,
        caption: "Position",
        hint: "Fixed coordinate along the axis the plane is perpendicular to. For axis=XY the plane sits at z=position; for YZ at x=position; for ZX at y=position.",
    },
    size: #{
        value: 10.0, min: -100.0, max: 100.0, free: true,
        caption: "Size",
        hint: "Plane edge length. The two free axes get `(random − 0.5) × size` so positive `size` scatters in a square of side `size` centered on the axis.",
    },
    axis: #{
        value: 2, choices: ["XY (fix Z)", "YZ (fix X)", "ZX (fix Y)"],
        caption: "Axis",
        hint: "Which two axes form the plane. The third axis is fixed at `position`.",
    },
    direct_color: #{
        value: true,
        caption: "Direct Color",
        hint: "Write a direct color scalar to `vc` based on the two random plane coordinates. Off skips the write and lets the standard color evolution carry through.",
    },
    color_mode: #{
        value: 3, choices: ["Colormap (→ U fallback)", "U", "V", "UV"],
        caption: "Color Mode",
        hint: "How the direct-color scalar is derived from the two random coordinates u, v ∈ [0,1). `U` and `V` write that coordinate directly, `UV` writes the product, `Colormap` would sample an image in JWildfire (no texture support here — falls back to U).",
    },
};

const INFO = #{ title: "Plane WF", name: "rhai_plane_wf", category: "full_3d", phase: "any" };

fn variation(z, p, ctx) {
    let color = ctx.color;
    let position = p.position;
    let size = p.size;
    let axis = to_int(to_float(p.axis));
    let direct_color = to_int(to_float(p.direct_color));
    let color_mode = to_int(to_float(p.color_mode));
    let u = 0.5 - rand();
    let v = 0.5 - rand();
    let x = 0.0;
    let y = 0.0;
    if (axis == 0) {
        x = u * size;
        y = v * size;
    } else if (axis == 1) {
        x = position;
        y = u * size;
    } else {
        x = u * size;
        y = position;
    }
    if (direct_color != 0) {
        let tc = u + 0.5;
        if (color_mode == 2) {
            tc = v + 0.5;
        } else if (color_mode == 3) {
            tc = (u + 0.5) * (v + 0.5);
        }
        color = clamp(tc, 0.0, 1.0);
    }
    #{ z: complex(x, y), color: color }
}

fn variation3(q, p, ctx) {
    let color = ctx.color;
    let position = p.position;
    let size = p.size;
    let axis = to_int(to_float(p.axis));
    let direct_color = to_int(to_float(p.direct_color));
    let color_mode = to_int(to_float(p.color_mode));
    let u = 0.5 - rand();
    let v = 0.5 - rand();
    let x = 0.0;
    let y = 0.0;
    let z = 0.0;
    if (axis == 0) {
        x = u * size;
        y = v * size;
        z = position;
    } else if (axis == 1) {
        x = position;
        y = u * size;
        z = v * size;
    } else {
        x = u * size;
        y = position;
        z = v * size;
    }
    if (direct_color != 0) {
        let tc = u + 0.5;
        if (color_mode == 2) {
            tc = v + 0.5;
        } else if (color_mode == 3) {
            tc = (u + 0.5) * (v + 0.5);
        }
        color = clamp(tc, 0.0, 1.0);
    }
    #{ z: vec3(x, y, z), color: color }
}

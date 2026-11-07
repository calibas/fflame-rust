/// Stereogram: dual-plot left/right stereo pair as a final transform —
/// camera-space eye fork (off-axis), convergence plane, parallel /
/// cross / L-R-L triptych panel layouts, per-panel band clipping.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// Stereogram, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    baseline: #{
        value: 0.033, min: 0.001, max: 0.5,
        caption: "Baseline",
        hint: "Eye separation as a FRACTION of the convergence distance (~1/30 is comfortable). Flame coordinate scales vary by orders of magnitude, so an absolute baseline would be meaningless as a saved param. Larger = stronger depth, harder fusion.",
    },
    z_conv: #{
        value: 1.0, min: 0.05, max: 10.0, free: true,
        caption: "Convergence",
        hint: "Camera-space depth of the zero-parallax plane — content at this depth sits exactly on the 'screen'; nearer pops out, farther recedes. Requires Perspective > 0 in the View panel (orthographic projection has no parallax, so the panels come out identical). In Triptych mode keep this near the content's actual depth: the L-R-L eye alternation turns any convergence offset into UNEVEN panel spacing (one pair gap grows by exactly what the other shrinks).",
    },
    view: #{
        value: 0, choices: ["Parallel", "Cross", "Triptych L-R-L"],
        caption: "View",
        hint: "Panel arrangement. Parallel: left-eye image in the left panel (relaxed/wall-eyed viewing; keep panels small on screen). Cross: panels swapped for cross-eyed viewing — no size limit. Triptych L-R-L: three panels serving BOTH methods at once (view 1-2 parallel, 2-3 cross); set Convergence near the content depth or the panel spacing goes uneven. If depth looks inverted (near reads as far), you are fusing with the other method — switch modes. The gap between panels shows the background color (Colors panel); painted white/black bars would need a tonemap-stage fill and are not implemented.",
    },
    panel_width: #{
        value: 2.0, min: 0.1, max: 10.0,
        caption: "Panel Width",
        hint: "Width of each panel in image-plane units (panel centers sit one width apart). Also the horizontal band each eye's content is clipped to — points projecting outside their own panel are dropped, so eyes never cross-contaminate.",
    },
    gap: #{
        value: 0.1, min: 0.0, max: 0.9,
        caption: "Panel Gap",
        hint: "Dead gutter between panels, as a fraction of Panel Width. Must stay wider than any screen-space blur footprint (density estimation etc.) or the filters smear one eye into the other at the seam.",
    },
    eye_weight: #{
        value: 1.0, min: 0.05, max: 2.0,
        caption: "Eye Weight",
        hint: "Density weight per emitted panel point. 1 renders each panel at full weight (the tonemap sees ~2x the mono density — uniformly, so exposure compensates); 0.5 restores mono-equivalent total brightness.",
    },
};

const INFO = #{ title: "Stereogram", name: "rhai_stereogram", category: "advanced_2d", z: "preserve_z", emits: 3 };

fn build_camera_matrix(yaw, pitch, bank, roll) {
    let sy = sin(yaw);
    let cy = cos(yaw);
    let sp = sin(pitch);
    let cp = cos(pitch);
    let sb = sin(bank);
    let cb = cos(bank);
    let sr = sin(roll);
    let cr = cos(roll);
    [cy * cb * cr - sy * cp * sr + sy * sp * sb * cr, -sy * cb * cr - cy * cp * sr + cy * sp * sb * cr, sp * sr + cp * sb * cr, cy * cb * sr + sy * cp * cr + sy * sp * sb * sr, -sy * cb * sr + cy * cp * cr + cy * sp * sb * sr, -sp * cr + cp * sb * sr, -cy * sb + sy * sp * cb, sy * sb + cy * sp * cb, cp * cb]
}

fn camera_transform(p, camera_matrix, camera_pos) {
    let p_t = p - camera_pos;
    let x = camera_matrix[3 * 0 + 0] * p_t.x + camera_matrix[3 * 1 + 0] * p_t.y + camera_matrix[3 * 2 + 0] * p_t.z;
    let y = camera_matrix[3 * 0 + 1] * p_t.x + camera_matrix[3 * 1 + 1] * p_t.y + camera_matrix[3 * 2 + 1] * p_t.z;
    let z = camera_matrix[3 * 0 + 2] * p_t.x + camera_matrix[3 * 1 + 2] * p_t.y + camera_matrix[3 * 2 + 2] * p_t.z;
    vec3(x, y, z)
}

fn variation(z, p) {
    let view = u32(to_float(p.view));
    let panel_w = max(1.0e-3, p.panel_width);
    let gap = clamp(p.gap, 0.0, 0.9);
    let eye_w = p.eye_weight;
    suppress_main();
    let band_half = 0.5 * panel_w * (1.0 - gap);
    let n = (if view == u32(2) { u32(3) } else { u32(2) });
    for i_n in 0..to_int(n) {
        let i = u32(i_n);
        let c = (to_float(i) - 0.5 * to_float(n - u32(1))) * panel_w;
        if (abs(z.re) <= band_half) {
            emit(complex(z.re + c, z.im), eye_w);
        }
    }
    z
}

fn variation3(v, p, ctx) {
    let baseline = p.baseline;
    let z_conv = max(1.0e-3, p.z_conv);
    let view = u32(to_float(p.view));
    let panel_w = max(1.0e-3, p.panel_width);
    let gap = clamp(p.gap, 0.0, 0.9);
    let eye_w = p.eye_weight;
    suppress_main();
    let campos = vec3(ctx.camera_x, ctx.camera_y, ctx.camera_z);
    let m = build_camera_matrix(0.0, -ctx.camera_pitch, -ctx.camera_bank, ctx.camera_yaw);
    let cam = camera_transform(v, m, campos);
    let persp = ctx.perspective;
    let zr = 1.0 - persp * cam.z;
    if (zr < 1.0e-3) {
        return v;
    }
    let b = baseline * z_conv;
    let zr_conv = max(1.0e-3, 1.0 - persp * z_conv);
    let band_half = 0.5 * panel_w * (1.0 - gap);
    let n = (if view == u32(2) { u32(3) } else { u32(2) });
    for i_n in 0..to_int(n) {
        let i = u32(i_n);
        let e = 0.0;
        let c = 0.0;
        if (view == u32(2)) {
            e = (if i == u32(1) { 1.0 } else { -1.0 });
            c = (to_float(i) - 1.0) * panel_w;
        } else {
            e = to_float(i) * 2.0 - 1.0;
            let side = (if view == u32(1) { -e } else { e });
            c = side * 0.5 * panel_w;
        }
        let x_cam = cam.x - e * 0.5 * b + (e * 0.5 * b / zr_conv + c) * zr;
        let u = x_cam / zr;
        if (abs(u - c) <= band_half) {
            let camv = vec3(x_cam, cam.y, cam.z);
            let t1 = [m[0], m[3], m[6], m[1], m[4], m[7], m[2], m[5], m[8]];
            emit(vec3((t1[0] * camv.x + t1[3] * camv.y + t1[6] * camv.z), (t1[1] * camv.x + t1[4] * camv.y + t1[7] * camv.z), (t1[2] * camv.x + t1[5] * camv.y + t1[8] * camv.z)) + campos, eye_w);
        }
    }
    v
}

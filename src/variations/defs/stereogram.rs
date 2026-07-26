//! `stereogram` — dual-plot stereo pair from one render (original).
//!
//! A final-transform variation that emits each iteration's point twice —
//! once per eye, each into its own panel — producing a free-fusion
//! stereogram in a single render. Dual-plot rather than a 50/50
//! stochastic split, deliberately: a split gives each eye a disjoint
//! random subset of the trajectory, so the residual density noise is
//! independent between eyes and fusion degrades into shimmer exactly in
//! the high-detail regions where the depth cue is strongest. Dual-plot
//! gives both eyes the identical sample set differing only by
//! projection — correlated noise, clean fusion. Same RNG seed comes for
//! free since it is one trajectory.
//!
//! Geometry: off-axis, not toe-in. The fork happens on the 3D point in
//! CAMERA space (depth-dependent disparity `x ∓ b/2`), with the camera
//! axes kept parallel — no keystone, no vertical parallax. The variation
//! runs in world space, so it round-trips through the module's own
//! camera machinery: `camera_transform` forward (the same roll-less
//! matrix `project_3d_full` builds), fork + panel placement in camera
//! space, `transpose(M)` back — the standard plot projection then lands
//! everything correctly, at any camera orientation, with no renderer
//! plumbing. Constant IMAGE-space shifts (convergence, panel centers)
//! require world offsets scaled by the Apophysis divisor `zr = 1 −
//! persp·z` — that is what keeps them flat in raster space while the
//! `∓b/2` term alone carries depth.
//!
//! Convergence: a point at camera depth `z_conv` projects identically in
//! both panels (zero parallax — the "screen plane"). Nearer points get
//! crossed disparity, farther points uncrossed.
//!
//! Panel discipline: each eye's content is band-clipped to its own panel
//! (with a `gap` gutter) by projecting the candidate and simply not
//! emitting out-of-band points — left-eye content can never bleed into
//! the right panel. The main (center) plot is suppressed via `CanHide`;
//! only the emitted panels appear.
//!
//! Requirements & limits (see docs/projects/multi-emit-stereograms.md):
//! 3D render mode with `perspective_strength > 0` (orthographic has no
//! parallax — physically correct, artistically useless). Solid-mode
//! occlusion/shadows are unsupported (two interleaved "objects" fight
//! one depth buffer). The 2D body is a defined degenerate: side-by-side
//! duplicate, zero disparity. Screen-space `rotation` (3D roll) rotates
//! the whole pair including the seam — keep it 0 for viewing.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Stereogram: dual-plot left/right stereo pair as a final transform —
/// camera-space eye fork (off-axis), convergence plane, parallel /
/// cross / L-R-L triptych panel layouts, per-panel band clipping.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static STEREOGRAM: VariationDef = VariationDef {
    name: "stereogram",
    aliases: &[],
    display_name: "Stereogram",
    // Advanced2D per the Only3D rule: the 2D body is a defined (if
    // degenerate) side-by-side duplicate, not a broken stub.
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    // CanHide: the center plot is suppressed; only the per-eye emissions
    // appear. PlotEmits(3): two eyes, or three panels in Triptych mode.
    features: &[Feature::CanHide, Feature::PlotEmits(3)],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("baseline", "Baseline", float, 0.033, 0.001, 0.5, "Eye separation as a FRACTION of the convergence distance (~1/30 is comfortable). Flame coordinate scales vary by orders of magnitude, so an absolute baseline would be meaningless as a saved param. Larger = stronger depth, harder fusion."),
        param!("z_conv", "Convergence", unlimited_float, 1.0, 0.05, 10.0, "Camera-space depth of the zero-parallax plane — content at this depth sits exactly on the 'screen'; nearer pops out, farther recedes. Requires Perspective > 0 in the View panel (orthographic projection has no parallax, so the panels come out identical). In Triptych mode keep this near the content's actual depth: the L-R-L eye alternation turns any convergence offset into UNEVEN panel spacing (one pair gap grows by exactly what the other shrinks)."),
        param!("view", "View", enum, 0, &["Parallel", "Cross", "Triptych L-R-L"], "Panel arrangement. Parallel: left-eye image in the left panel (relaxed/wall-eyed viewing; keep panels small on screen). Cross: panels swapped for cross-eyed viewing — no size limit. Triptych L-R-L: three panels serving BOTH methods at once (view 1-2 parallel, 2-3 cross); set Convergence near the content depth or the panel spacing goes uneven. If depth looks inverted (near reads as far), you are fusing with the other method — switch modes. The gap between panels shows the background color (Colors panel); painted white/black bars would need a tonemap-stage fill and are not implemented."),
        param!("panel_width", "Panel Width", float, 2.0, 0.1, 10.0, "Width of each panel in image-plane units (panel centers sit one width apart). Also the horizontal band each eye's content is clipped to — points projecting outside their own panel are dropped, so eyes never cross-contaminate."),
        param!("gap", "Panel Gap", float, 0.1, 0.0, 0.9, "Dead gutter between panels, as a fraction of Panel Width. Must stay wider than any screen-space blur footprint (density estimation etc.) or the filters smear one eye into the other at the seam."),
        param!("eye_weight", "Eye Weight", float, 1.0, 0.05, 2.0, "Density weight per emitted panel point. 1 renders each panel at full weight (the tonemap sees ~2x the mono density — uniformly, so exposure compensates); 0.5 restores mono-equivalent total brightness."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_stereogram(p: vec2<f32>, xform_id: u32, variation_id: u32, hide: ptr<function, bool>) -> vec2<f32> {
    let view = u32(get_param(xform_id, variation_id, 2u));
    let panel_w = max(get_param(xform_id, variation_id, 3u), 1e-3);
    let gap = clamp(get_param(xform_id, variation_id, 4u), 0.0, 0.9);
    let eye_w = get_param(xform_id, variation_id, 5u);

    // 2D render mode: no depth, no disparity — a defined degenerate
    // (side-by-side duplicate) so the variation never renders garbage.
    *hide = true;
    let band_half = 0.5 * panel_w * (1.0 - gap);
    let n = select(2u, 3u, view == 2u);
    for (var i = 0u; i < n; i = i + 1u) {
        let c = (f32(i) - 0.5 * f32(n - 1u)) * panel_w;
        if (abs(p.x) <= band_half) {
            emit_plot_weighted(vec2<f32>(p.x + c, p.y), eye_w);
        }
    }
    return p;
}
"#;

const WGSL_3D: &str = r#"
fn variation_stereogram(p: vec3<f32>, xform_id: u32, variation_id: u32, hide: ptr<function, bool>) -> vec3<f32> {
    let baseline = get_param(xform_id, variation_id, 0u);
    let z_conv = max(get_param(xform_id, variation_id, 1u), 1e-3);
    let view = u32(get_param(xform_id, variation_id, 2u));
    let panel_w = max(get_param(xform_id, variation_id, 3u), 1e-3);
    let gap = clamp(get_param(xform_id, variation_id, 4u), 0.0, 0.9);
    let eye_w = get_param(xform_id, variation_id, 5u);

    // Only the emitted panels plot; the center image is suppressed.
    *hide = true;

    // World -> camera, with the SAME roll-less matrix project_3d_full
    // builds (see utilities.wgsl — the slot mapping is deliberate, roll
    // is a post-projection screen rotation). camera_transform computes
    // M·(p − campos) in WGSL column semantics, so the inverse below is
    // transpose(M)·cam + campos.
    let campos = vec3<f32>(params.camera_x, params.camera_y, params.camera_z);
    let m = build_camera_matrix(
        0.0,
        -params.camera_rotation_x,
        -params.camera_bank,
         params.camera_rotation_y,
    );
    let cam = camera_transform(p, m, campos);

    let persp = params.perspective_strength;
    let zr = 1.0 - persp * cam.z;
    if (zr < 1e-3) {
        // Behind the camera / inside the clip halo: nothing to emit
        // (apply_perspective would reject it anyway).
        return p;
    }
    let b = baseline * z_conv;
    let zr_conv = max(1.0 - persp * z_conv, 1e-3);
    let band_half = 0.5 * panel_w * (1.0 - gap);

    // Panels: (eye, center). Parallel = left eye on the left panel;
    // Cross = swapped; Triptych L-R-L = both viewing methods at once.
    let n = select(2u, 3u, view == 2u);
    for (var i = 0u; i < n; i = i + 1u) {
        var e: f32;
        var c: f32;
        if (view == 2u) {
            // L-R-L at centers -w, 0, +w.
            e = select(-1.0, 1.0, i == 1u);
            c = (f32(i) - 1.0) * panel_w;
        } else {
            e = f32(i) * 2.0 - 1.0;                  // -1 left eye, +1 right
            let side = select(e, -e, view == 1u);    // Cross swaps panels
            c = side * 0.5 * panel_w;
        }

        // Off-axis eye: depth-dependent disparity (∓b/2) plus the
        // constant image-space terms (convergence + panel center),
        // which need the ·zr scaling to stay flat in raster space.
        let x_cam = cam.x - e * 0.5 * b + (e * 0.5 * b / zr_conv + c) * zr;

        // Band-clip to this panel (pre pan/rotate/zoom image plane):
        // out-of-panel content is dropped, never smeared into the
        // neighbouring eye.
        let u = x_cam / zr;
        if (abs(u - c) <= band_half) {
            let camv = vec3<f32>(x_cam, cam.y, cam.z);
            emit_plot_weighted(transpose(m) * camv + campos, eye_w);
        }
    }
    return p;
}
"#;

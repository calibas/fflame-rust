// Post-process depth-of-field for solid rendering — TRUE 2-D disk
// gather (dense, adaptive radius). Four passes:
//   1. coc_main  — per-pixel CoC from bilaterally smoothed depth
//   2. max_main  — horizontal running max of the CoC field
//   3. max_main  — vertical running max (⇒ dilated max-reach field)
//   4. dof_main  — dense 2-D scatter-as-gather disk blur
//
// At-splat DoF is compiled out under SOLID by design: jittering plot
// positions lands samples at pixels whose depth they don't belong to,
// corrupting the nearest-depth buffer and the occlusion test. This
// pass chain is the replacement: it runs between shade and tonemap on
// the HDR pre-tonemap image (shade output when lighting is on, else
// the raw accumulator), reading the per-pixel nearest-depth region.
//
// Quality lessons baked in (all field-reported):
// - Sparse jittered taps (v1) = Monte-Carlo sand on flame-range data.
// - Separable H+V passes (v2) = axis-aligned streaks: a
//   VARIABLE-radius blur is not separable, and a flat 1-D kernel run
//   twice has a square point spread. Only a dense 2-D disk matches
//   the at-splat reference (uniform disk via sqrt(rand)·coc).
// - The nearest-depth field carries shell-scale Monte-Carlo noise, so
//   CoC comes from a bilaterally smoothed depth (coc_main); raw depth
//   is still used for occlusion suppression so edges stay honest.
//
// The dense 2-D gather is O(R²) per pixel, so the loop bound adapts:
// the dilated max-reach field (passes 2-3) bounds how far any light
// can spread INTO each pixel — sharp regions loop a few taps, only
// genuinely defocused areas pay the full disk.
//
// Circle of confusion MATCHES the at-splat formula so the DoF sliders
// mean the same thing in both pipelines:
//   camera_z = -d (stored depth is -camera_space.z, positive in front)
//   coc_px   = |camera_z - focus| * strength * 0.1
//              * min(width, height) * 0.25 * zoom
//
// Scatter-as-gather: a tap SPREADS over its own CoC as a flat disk
// with a soft rim, energy-normalized by its disk area (wider spreads
// deposit less per pixel). Foreground bleeds over silhouettes
// naturally; taps meaningfully BEHIND the center surface are
// suppressed toward their sharpness ratio (background can't bleed
// over sharp foreground edges), with a distance-lenient margin so a
// sloped surface doesn't suppress its own far taps. Output is
// renormalized ONLY when overcomplete: at coverage boundaries the
// spread light partially fills the pixel and alpha fades with it, so
// the log tonemap melts silhouettes exactly like at-splat DoF does.
// All four channels blur together — alpha carries density for the
// tonemap's log math; leaving it sharp would re-sharpen brightness.
//
// All reads are textureLoad / raw buffer indexing — WASM-clean.

struct DofParams {
    width: u32,
    height: u32,
    // Word offset of the depth region inside the bound buffer
    // (interactive = W*H*4; exporters may pass 0 for a dedicated one).
    depth_word_offset: u32,
    // CoC clamp in pixels (also the max-filter half-width).
    radius: u32,
    // dof_focus_distance (camera_space.z units, matches at-splat).
    focus: f32,
    // dof_blur_strength * 0.1 * min(w,h) * 0.25 * zoom — the full
    // |camera_z - focus| -> pixels factor, host-precomputed.
    coc_scale: f32,
    // Surface shell thickness (world units) — the bilateral depth
    // window scale for the CoC prepass.
    surface_thickness: f32,
    _pad0: f32,
    // max_main direction: (1,0) then (0,1). Unused by coc/dof passes.
    dir_x: i32,
    dir_y: i32,
    _pad1: i32,
    _pad2: i32,
}

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var<storage, read> depth_buf: array<u32>;
@group(0) @binding(2) var<uniform> dp: DofParams;
@group(0) @binding(3) var dof_out: texture_storage_2d<rgba32float, write>;
// Smoothed CoC field (r = CoC px, g = 1 when the pixel has depth).
// Passes that don't read it bind a stand-in.
@group(0) @binding(4) var coc_tex: texture_2d<f32>;
// Dilated max-reach field (r = neighborhood max CoC). Only dof_main
// reads it; other passes bind a stand-in.
@group(0) @binding(5) var max_tex: texture_2d<f32>;

fn depth_at(px: i32, py: i32) -> f32 {
    if (px < 0 || py < 0 || px >= i32(dp.width) || py >= i32(dp.height)) {
        return 3.0e38;
    }
    let enc = depth_buf[dp.depth_word_offset + u32(py) * dp.width + u32(px)];
    if (enc == 0u) {
        return 3.0e38;
    }
    let ord = ~enc;
    let bits = select(~ord, ord ^ 0x80000000u, (ord & 0x80000000u) != 0u);
    return bitcast<f32>(bits);
}

fn coc_from_depth(d: f32) -> f32 {
    return min(abs(-d - dp.focus) * dp.coc_scale, f32(dp.radius));
}

// ── Pass 1: CoC prepass ─────────────────────────────────────────────
// Bilateral 9×9 depth mean (same-surface window, like normals.wgsl):
// the raw nearest-depth field carries shell-scale Monte-Carlo noise
// that would otherwise modulate the blur disk per pixel.
@compute @workgroup_size(8, 8, 1)
fn coc_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= dp.width || gid.y >= dp.height) {
        return;
    }
    let px = i32(gid.x);
    let py = i32(gid.y);
    let d_c = depth_at(px, py);
    if (d_c > 1.0e37) {
        textureStore(dof_out, vec2<i32>(px, py), vec4<f32>(0.0, 0.0, 0.0, 0.0));
        return;
    }
    let win = max(dp.surface_thickness * 4.0, 0.02 * (abs(d_c) + 1.0));
    var dsum = 0.0;
    var n = 0.0;
    for (var oy = -4; oy <= 4; oy = oy + 1) {
        for (var ox = -4; ox <= 4; ox = ox + 1) {
            let nd = depth_at(px + ox, py + oy);
            if (nd > 1.0e37 || abs(nd - d_c) > win) {
                continue;
            }
            dsum = dsum + nd;
            n = n + 1.0;
        }
    }
    let d = dsum / max(n, 1.0);
    textureStore(dof_out, vec2<i32>(px, py), vec4<f32>(coc_from_depth(d), 1.0, 0.0, 0.0));
}

// ── Passes 2+3: separable running max of the CoC field ──────────────
// (A max filter IS separable.) Result: how far any neighbor's disk
// could possibly reach into this pixel — the dof_main loop bound.
@compute @workgroup_size(8, 8, 1)
fn max_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= dp.width || gid.y >= dp.height) {
        return;
    }
    let px = i32(gid.x);
    let py = i32(gid.y);
    var m = 0.0;
    let r = i32(dp.radius);
    for (var i = -r; i <= r; i = i + 1) {
        let sx = px + i * dp.dir_x;
        let sy = py + i * dp.dir_y;
        if (sx < 0 || sy < 0 || sx >= i32(dp.width) || sy >= i32(dp.height)) {
            continue;
        }
        let v = textureLoad(coc_tex, vec2<i32>(sx, sy), 0);
        // A disk at distance |i| reaches us only if its radius covers
        // the distance — subtracting |i| tightens the bound (and lets
        // fully sharp rows collapse to 0).
        m = max(m, v.r - f32(abs(i)));
    }
    let c = textureLoad(coc_tex, vec2<i32>(px, py), 0);
    textureStore(dof_out, vec2<i32>(px, py), vec4<f32>(max(m, c.r), c.g, 0.0, 0.0));
}

// ── Pass 4: dense 2-D disk gather ───────────────────────────────────
@compute @workgroup_size(8, 8, 1)
fn dof_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= dp.width || gid.y >= dp.height) {
        return;
    }
    let px = i32(gid.x);
    let py = i32(gid.y);
    let center = textureLoad(input_tex, vec2<i32>(px, py), 0);

    // Adaptive loop bound: nothing can reach us from beyond the
    // dilated max-reach (+rim slack).
    let reach_in = textureLoad(max_tex, vec2<i32>(px, py), 0).r;
    let rr = i32(ceil(min(reach_in + 1.5, f32(dp.radius) + 1.5)));
    if (rr < 1) {
        textureStore(dof_out, vec2<i32>(px, py), center);
        return;
    }

    let d_c = depth_at(px, py);
    let coc_c = textureLoad(coc_tex, vec2<i32>(px, py), 0).r;
    // Behind-margin base for background suppression (depth-relative).
    let behind = 0.05 * (abs(d_c) + 1.0);

    var csum = vec4<f32>(0.0);
    var wsum = 0.0;
    for (var oy = -rr; oy <= rr; oy = oy + 1) {
        let sy = py + oy;
        if (sy < 0 || sy >= i32(dp.height)) {
            continue;
        }
        for (var ox = -rr; ox <= rr; ox = ox + 1) {
            let sx = px + ox;
            if (sx < 0 || sx >= i32(dp.width)) {
                continue;
            }
            let ct = textureLoad(coc_tex, vec2<i32>(sx, sy), 0);
            if (ct.g < 0.5) {
                // Empty pixel: no light to spread.
                continue;
            }
            // Flat disk with a soft rim — the at-splat kernel shape.
            let reach = ct.r;
            let rim = clamp(reach * 0.5, 0.71, 1.5);
            let dist = sqrt(f32(ox * ox + oy * oy));
            if (dist > reach + rim) {
                continue;
            }
            // Energy conservation: the disk's light spreads over its
            // area (π·reach² + 1 keeps the sharp case sane: a reach-0
            // tap deposits everything in its own pixel).
            var w = clamp((reach + rim - dist) / rim, 0.0, 1.0)
                / (3.14159265 * reach * reach + 1.0);
            // Background suppression, distance-lenient: a tap behind
            // the center surface contributes at most the center's own
            // blur ratio; the margin grows with distance so a sloped
            // surface's far taps aren't treated as background.
            if (d_c < 1.0e37) {
                let d_t = depth_at(sx, sy);
                if (d_t < 1.0e37 && d_t > d_c + behind * (1.0 + 0.25 * dist)) {
                    w = w * clamp((coc_c + 0.5) / (reach + 0.5), 0.0, 1.0);
                }
            }
            csum = csum + textureLoad(input_tex, vec2<i32>(sx, sy), 0) * w;
            wsum = wsum + w;
        }
    }

    if (wsum < 1e-6) {
        textureStore(dof_out, vec2<i32>(px, py), center);
        return;
    }
    // Renormalize ONLY when overcomplete: at coverage boundaries the
    // spread light partially fills the pixel; letting rgb+alpha fade
    // there is what melts silhouettes (the log tonemap dims fading
    // density, matching the at-splat reference).
    textureStore(dof_out, vec2<i32>(px, py), csum / max(wsum, 1.0));
}

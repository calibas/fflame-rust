// Post-process depth-of-field for solid rendering (gather blur).
//
// At-splat DoF is compiled out under SOLID by design: jittering plot
// positions lands samples at pixels whose depth they don't belong to,
// corrupting the nearest-depth buffer and the occlusion test. This pass
// is the replacement: it runs between shade and tonemap on the HDR
// pre-tonemap image (shade output when lighting is on, else the raw
// accumulator), reading the same per-pixel nearest-depth region the
// solid pipeline maintains.
//
// Circle of confusion MATCHES the at-splat formula exactly so the DoF
// sliders mean the same thing in both pipelines:
//   camera_z = -d (stored depth is -camera_space.z, positive in front)
//   coc_px   = |camera_z - focus| * strength * 0.1
//              * min(width, height) * 0.25 * zoom
//
// Scatter-as-gather: each output pixel gathers a golden-angle spiral of
// taps; a tap contributes when its OWN CoC disk reaches the center
// (foreground naturally bleeds over silhouettes), weighted by
// 1/(coc^2+1) for energy conservation (a sample's light spreads over
// its disk area). Background taps behind an in-focus center are
// suppressed to their coverage ratio so the background can't bleed over
// sharp foreground edges. All four channels blur together — alpha
// carries density for the tonemap's log math, and leaving it sharp
// would re-sharpen brightness at blurred edges.
//
// All reads are textureLoad / raw buffer indexing — WASM-clean.

struct DofParams {
    width: u32,
    height: u32,
    // Word offset of the depth region inside the bound buffer
    // (interactive = W*H*4; exporters may pass 0 for a dedicated one).
    depth_word_offset: u32,
    // Tap count actually used (<= MAX_TAPS); host scales with radius.
    taps: u32,
    // dof_focus_distance (camera_space.z units, matches at-splat).
    focus: f32,
    // dof_blur_strength * 0.1 * min(w,h) * 0.25 * zoom — the full
    // |camera_z - focus| -> pixels factor, host-precomputed.
    coc_scale: f32,
    // CoC clamp in pixels (performance + firefly-smear guard).
    max_radius: f32,
    _pad0: f32,
}

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var<storage, read> depth_buf: array<u32>;
@group(0) @binding(2) var<uniform> dp: DofParams;
@group(0) @binding(3) var dof_out: texture_storage_2d<rgba32float, write>;

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

// CoC radius in pixels for stored depth d; empty pixels (no depth)
// return 0 — they contribute no light of their own.
fn coc_px(d: f32) -> f32 {
    if (d > 1.0e37) {
        return 0.0;
    }
    return min(abs(-d - dp.focus) * dp.coc_scale, dp.max_radius);
}

// Per-pixel hash in [0, 1) — rotates/jitters the spiral so the fixed
// tap pattern reads as fine noise instead of rings.
fn dof_hash(px: i32, py: i32) -> f32 {
    var h = u32(px) * 374761393u + u32(py) * 668265263u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    return f32(h & 0xFFFFu) / 65536.0;
}

@compute @workgroup_size(8, 8, 1)
fn dof_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= dp.width || gid.y >= dp.height) {
        return;
    }
    let px = i32(gid.x);
    let py = i32(gid.y);
    let center = textureLoad(input_tex, vec2<i32>(px, py), 0);
    let d_c = depth_at(px, py);
    let coc_c = coc_px(d_c);

    // Search radius: the center's own CoC (out-of-focus pixels spread
    // wide) OR nearby foreground bleed found by a sparse 8-probe ring —
    // an in-focus pixel next to a blurred foreground edge must still
    // gather that bleed.
    var r_search = coc_c;
    let probe_r = dp.max_radius * 0.7071;
    for (var k = 0; k < 8; k = k + 1) {
        let a = f32(k) * 0.7853981634; // 2*PI / 8
        let ox = i32(round(cos(a) * probe_r));
        let oy = i32(round(sin(a) * probe_r));
        let cp = coc_px(depth_at(px + ox, py + oy));
        // The probe's disk must reach us to matter.
        let dist = sqrt(f32(ox * ox + oy * oy));
        if (cp >= dist * 0.5) {
            r_search = max(r_search, cp);
        }
    }
    r_search = min(r_search, dp.max_radius);

    if (r_search < 0.5) {
        // Fully sharp neighborhood — pass through.
        textureStore(dof_out, vec2<i32>(px, py), center);
        return;
    }

    // Behind-margin for background suppression: taps meaningfully
    // behind the center's surface can't bleed over it beyond their
    // coverage ratio.
    let behind = 0.05 * (abs(d_c) + 1.0);

    let jitter = dof_hash(px, py);
    var csum = vec4<f32>(0.0);
    var wsum = 0.0;
    let n = i32(dp.taps);
    for (var i = 0; i < n; i = i + 1) {
        // Golden-angle spiral, jitter-rotated per pixel.
        let fi = (f32(i) + jitter) / f32(n);
        let ang = f32(i) * 2.3999632297 + jitter * 6.2831853;
        let rad = sqrt(fi) * r_search;
        let ox = i32(round(cos(ang) * rad));
        let oy = i32(round(sin(ang) * rad));
        let sx = px + ox;
        let sy = py + oy;
        if (sx < 0 || sy < 0 || sx >= i32(dp.width) || sy >= i32(dp.height)) {
            continue;
        }
        let d_t = depth_at(sx, sy);
        if (d_t > 1.0e37) {
            // Empty pixel: no light to spread.
            continue;
        }
        let dist = sqrt(f32(ox * ox + oy * oy));
        let coc_t = coc_px(d_t);
        // Coverage: the tap's blur disk must reach the center pixel
        // (+0.5 so in-focus taps still cover their own pixel).
        var w = clamp(coc_t + 0.5 - dist, 0.0, 1.0);
        if (w <= 0.0) {
            continue;
        }
        // Energy conservation: light spreads over the disk area.
        w = w / (coc_t * coc_t + 1.0);
        // Background suppression: a tap behind the center surface
        // contributes at most the center's own blur ratio (sharp
        // foreground edges stay sharp against a blurred background).
        if (d_t > d_c + behind) {
            w = w * clamp((coc_c + 0.5) / (coc_t + 0.5), 0.0, 1.0);
        }
        let s = textureLoad(input_tex, vec2<i32>(sx, sy), 0);
        csum = csum + s * w;
        wsum = wsum + w;
    }

    if (wsum < 1e-6) {
        textureStore(dof_out, vec2<i32>(px, py), center);
        return;
    }
    textureStore(dof_out, vec2<i32>(px, py), csum / wsum);
}

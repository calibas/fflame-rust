// Accumulate pass — consumes the sample stream produced by the unified
// iteration shader (with OUTPUT_HISTOGRAM_DIRECT=false) and atomic-adds
// each sample into the right slot of a histogram buffer.
//
// The histogram is laid out flat row-major (same as the direct-histogram
// output the interactive renderer writes to, see main_template.wgsl):
// 4× u32 per pixel — R, G, B, density. Per-pixel base index =
// (pixel_y - bound_y_start) × bound_width + (pixel_x - bound_x_start),
// with `bound_*` describing the histogram region this dispatch is bound
// to.
//
// The bound region lets one dispatch act on a sub-tile of the full
// histogram, so a multi-tile render can pipeline iterate→accumulate
// per tile (Phase 5) or per tile-batch. Samples landing outside the
// bound region get dropped — bin them with an outer dispatch loop, not
// inside the shader.
//
// One thread per sample (workgroup_size 64 along x, dispatch sized to
// the sample-count read back from the counter). See
// docs/projects/unified-render-pipeline.md.

struct Sample {
    x: f32,
    y: f32,
    r: f32,
    g: f32,
    b: f32,
    // Density weight (depth-density compensation; 1.0 = neutral).
    // Scales all four histogram adds below.
    weight: f32,
    _pad2: f32,
    _pad3: f32,
}

struct AccumulateParams {
    // X/Y origin of the histogram region the dispatch is bound to,
    // in full-image pixel coords. Samples landing outside
    // [bound_x, bound_x + bound_width) × [bound_y, bound_y + bound_height)
    // are dropped.
    bound_x: u32,
    bound_y: u32,
    bound_width: u32,
    bound_height: u32,

    // Sample count actually written by the iteration shader (read from
    // the atomic sample counter on host before dispatch). Threads with
    // global_id.x >= sample_count exit early.
    sample_count: u32,

    // Color scale applied to clamp(r,g,b) before atomic-adding into the
    // histogram, mirroring the direct path's `histogram_color_scale`
    // multiplier so the accumulation precision matches across modes.
    color_scale: f32,

    _pad0: u32,
    _pad1: u32,
}

@group(0) @binding(0) var<storage, read> samples: array<Sample>;
@group(0) @binding(1) var<uniform> ap: AccumulateParams;
@group(0) @binding(2) var<storage, read_write> histogram: array<atomic<u32>>;

@compute @workgroup_size(64, 1, 1)
fn accumulate_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= ap.sample_count) {
        return;
    }
    let s = samples[idx];

    // Reject samples outside the bound histogram region. Coords come
    // from the iteration shader as f32 pixel positions (it converts
    // from world space and stores `f32(pixel.x), f32(pixel.y)`); cast
    // to i32 so out-of-range values fail the bounds check rather than
    // wrapping when rounded into u32.
    let px = i32(s.x);
    let py = i32(s.y);
    let bx0 = i32(ap.bound_x);
    let by0 = i32(ap.bound_y);
    let bx1 = bx0 + i32(ap.bound_width);
    let by1 = by0 + i32(ap.bound_height);
    if (px < bx0 || px >= bx1 || py < by0 || py >= by1) {
        return;
    }

    let local_x = u32(px - bx0);
    let local_y = u32(py - by0);
    let pixel_idx = local_y * ap.bound_width + local_x;
    let base = pixel_idx * 4u;

    // All four channels carry the sample's weight so the color
    // recovery ratio Σcolor/Σdensity is weight-invariant. Weight is
    // 1.0 unless depth-density compensation produced it.
    let weighted_scale = ap.color_scale * s.weight;
    let r_u32 = u32(clamp(s.r, 0.0, 1.0) * weighted_scale);
    let g_u32 = u32(clamp(s.g, 0.0, 1.0) * weighted_scale);
    let b_u32 = u32(clamp(s.b, 0.0, 1.0) * weighted_scale);
    let density_u32 = u32(weighted_scale);

    atomicAdd(&histogram[base + 0u], r_u32);
    atomicAdd(&histogram[base + 1u], g_u32);
    atomicAdd(&histogram[base + 2u], b_u32);
    atomicAdd(&histogram[base + 3u], density_u32);
}

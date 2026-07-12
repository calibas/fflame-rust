// Solid-rendering density statistics: one small reduction over the
// accumulator's alpha channel (cumulative accepted hit count per pixel).
//
// Purpose: hard-solid occlusion culls most dispatched samples, but the
// tonemap normalizes brightness by DISPATCHED iterations — so solids
// render dark by exactly the culled fraction. This pass measures the
// real accepted total; the host derives survival_fraction =
// sum_alpha / samples_dispatched and scales the tonemap's
// sample_density by it (see FlameRenderer::update_density_stats).
//
// Single-workgroup strided sum: 256 threads × ~8k loads at 1080p, run
// every N frames while solid occlusion is active. f32 partials are
// plenty for a brightness scalar (~1% accuracy needed).

struct StatsParams {
    width: u32,
    height: u32,
    _pad0: u32,
    _pad1: u32,
}

@group(0) @binding(0) var accum_tex: texture_2d<f32>;
@group(0) @binding(1) var<uniform> params: StatsParams;
@group(0) @binding(2) var<storage, read_write> out_sum: array<f32>;

var<workgroup> partial: array<f32, 256>;

@compute @workgroup_size(256, 1, 1)
fn stats_main(@builtin(local_invocation_id) lid: vec3<u32>) {
    let n = params.width * params.height;
    var sum = 0.0;
    var i = lid.x;
    loop {
        if (i >= n) {
            break;
        }
        let px = i % params.width;
        let py = i / params.width;
        sum = sum + textureLoad(accum_tex, vec2<i32>(i32(px), i32(py)), 0).a;
        i = i + 256u;
    }
    partial[lid.x] = sum;
    workgroupBarrier();

    var stride = 128u;
    loop {
        if (stride == 0u) {
            break;
        }
        if (lid.x < stride) {
            partial[lid.x] = partial[lid.x] + partial[lid.x + stride];
        }
        workgroupBarrier();
        stride = stride / 2u;
    }
    if (lid.x == 0u) {
        out_sum[0] = partial[0];
    }
}

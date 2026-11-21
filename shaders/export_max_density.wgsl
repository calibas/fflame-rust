// The largest density word in a span of RGBD histogram words (4 a
// pixel, density the 4th), folded into one atomic maximum. The tiled
// exporter's whole-export u32 sums read it to fold themselves into f64
// before they wrap (src/export/fold.rs; docs/projects/exact-accumulator.md).
// Colour words never exceed their pixel's density word, so the density
// is the one to watch.
//
// One dispatch a span (a tile's RGBD region, the blur splat buffer),
// bound with its offset and size; a grid-stride loop covers any length.

@group(0) @binding(0) var<storage, read> words: array<u32>;
@group(0) @binding(1) var<storage, read_write> result: atomic<u32>;

var<workgroup> best: array<u32, 256>;

@compute @workgroup_size(256, 1, 1)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
    @builtin(num_workgroups) groups: vec3<u32>,
) {
    let pixels = arrayLength(&words) / 4u;
    let stride = groups.x * 256u;
    var m = 0u;
    for (var i = gid.x; i < pixels; i = i + stride) {
        m = max(m, words[i * 4u + 3u]);
    }
    best[li] = m;
    workgroupBarrier();
    for (var s = 128u; s > 0u; s = s / 2u) {
        if (li < s) {
            best[li] = max(best[li], best[li + s]);
        }
        workgroupBarrier();
    }
    if (li == 0u) {
        atomicMax(&result, best[0]);
    }
}

// Analytic-blur convolution pass. Each eligible transform's deterministic
// mean splats were written to its own slice of `blur_in` during the chaos-game
// pass (see the HAS_ANALYTIC_BLUR routing in main_template.wgsl). This pass
// convolves each slice with that transform's host-built kernel — the sampled,
// pixel-space image of the variation's blur offset distribution — and ADDS the
// result into the main histogram. Convolving the mean delta with the offset
// distribution reproduces the stochastic blur splat by construction, but from
// a fraction of the samples.
//
// Runs once per batch, after the chaos-game pass and before the spatial filter
// + accumulate. The main histogram already holds the direct (non-blur) samples;
// this adds the blur transforms' contribution on top.
//
// One thread per output pixel, so the histogram read-modify-write needs no
// atomics: every output pixel is owned by exactly one invocation, and the
// chaos-game pass that also writes the histogram has already completed.

struct ConvolveParams {
    width: u32,
    height: u32,
    count: u32,        // number of active blur slots (<= MAX_BLUR_BUFFERS)
    _pad: u32,
    // Per-slot (half, weight_offset, _, _). `half` = kernel half-extent in
    // pixels; `weight_offset` = first weight index in `kernel_weights`.
    slot_meta: array<vec4<u32>, 4>,
}

@group(0) @binding(0) var<storage, read> blur_in: array<u32>;
@group(0) @binding(1) var<storage, read_write> histogram_out: array<u32>;
@group(0) @binding(2) var<uniform> params: ConvolveParams;
@group(0) @binding(3) var<storage, read> kernel_weights: array<f32>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.width || gid.y >= params.height) {
        return;
    }
    let x = i32(gid.x);
    let y = i32(gid.y);
    let w = i32(params.width);
    let h = i32(params.height);
    let plane = params.width * params.height;  // floats-per-channel-group stride

    var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    for (var s: u32 = 0u; s < params.count; s = s + 1u) {
        let half = i32(params.slot_meta[s].x);
        let woff = params.slot_meta[s].y;
        let size = 2 * half + 1;
        let slot_base = s * plane;

        // out[p] = Σ_d  blur[p - d] · kernel[d],  d ∈ [-half, half]².
        for (var dy: i32 = -half; dy <= half; dy = dy + 1) {
            let sy = y - dy;
            if (sy < 0 || sy >= h) { continue; }
            let krow = (dy + half) * size; // first kernel index of this row
            let qrow = (slot_base + u32(sy) * params.width) * 4u;
            for (var dx: i32 = -half; dx <= half; dx = dx + 1) {
                let sx = x - dx;
                if (sx < 0 || sx >= w) { continue; }
                // Signed kernel index (column = dx + half), cast once.
                let weight = kernel_weights[woff + u32(krow + dx + half)];
                if (weight == 0.0) { continue; }
                let q = qrow + u32(sx) * 4u;
                acc = acc + weight * vec4<f32>(
                    f32(blur_in[q + 0u]),
                    f32(blur_in[q + 1u]),
                    f32(blur_in[q + 2u]),
                    f32(blur_in[q + 3u]),
                );
            }
        }
    }

    let hb = (u32(y) * params.width + u32(x)) * 4u;
    // Additive: preserve the direct samples already in the histogram.
    histogram_out[hb + 0u] = histogram_out[hb + 0u] + u32(acc.x + 0.5);
    histogram_out[hb + 1u] = histogram_out[hb + 1u] + u32(acc.y + 0.5);
    histogram_out[hb + 2u] = histogram_out[hb + 2u] + u32(acc.z + 0.5);
    histogram_out[hb + 3u] = histogram_out[hb + 3u] + u32(acc.w + 0.5);
}

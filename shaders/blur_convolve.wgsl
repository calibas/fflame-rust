// Analytic-blur stage 2/3: convolution (at low resolution). Convolve each
// transform's downsampled mean-splat slice with that transform's host-built
// kernel — the sampled, pixel-space image of the variation's blur offset
// distribution, built at the SAME low-res scale (linear map ÷ D). Output is a
// per-slot low-res convolved slice consumed by the upscale stage.
//
// Convolving the mean delta-field with the offset distribution reproduces the
// stochastic blur splat by construction, but from a fraction of the samples
// and — because it runs at low res — a fraction of the kernel taps.
//
// One thread per low-res pixel; writes every pixel, so no clear is needed.

struct ConvolveParams {
    full_width: u32,
    full_height: u32,
    lowres_width: u32,
    lowres_height: u32,
    downscale: u32,
    count: u32,
    _pad0: u32,
    _pad1: u32,
    slot_meta: array<vec4<u32>, 4>,  // (lowres_half, weight_offset, _, _)
}

@group(0) @binding(0) var<storage, read> blur_low: array<u32>;
@group(0) @binding(1) var<storage, read_write> blur_conv: array<u32>;
@group(0) @binding(2) var<uniform> params: ConvolveParams;
@group(0) @binding(3) var<storage, read> kernel_weights: array<f32>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.lowres_width || gid.y >= params.lowres_height) {
        return;
    }
    let x = i32(gid.x);
    let y = i32(gid.y);
    let w = i32(params.lowres_width);
    let h = i32(params.lowres_height);
    let plane = params.lowres_width * params.lowres_height;
    let out_idx = (gid.y * params.lowres_width + gid.x) * 4u;

    for (var s: u32 = 0u; s < params.count; s = s + 1u) {
        let half = i32(params.slot_meta[s].x);
        let woff = params.slot_meta[s].y;
        let size = 2 * half + 1;
        let slot_base = s * plane;

        var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        // out[p] = Σ_d  blur[p - d] · kernel[d],  d ∈ [-half, half]².
        for (var dy: i32 = -half; dy <= half; dy = dy + 1) {
            let sy = y - dy;
            if (sy < 0 || sy >= h) { continue; }
            let krow = (dy + half) * size;
            let qrow = (slot_base + u32(sy) * params.lowres_width) * 4u;
            for (var dx: i32 = -half; dx <= half; dx = dx + 1) {
                let sx = x - dx;
                if (sx < 0 || sx >= w) { continue; }
                let weight = kernel_weights[woff + u32(krow + dx + half)];
                if (weight == 0.0) { continue; }
                let q = qrow + u32(sx) * 4u;
                acc = acc + weight * vec4<f32>(
                    f32(blur_low[q + 0u]),
                    f32(blur_low[q + 1u]),
                    f32(blur_low[q + 2u]),
                    f32(blur_low[q + 3u]),
                );
            }
        }
        let o = slot_base * 4u + out_idx;
        blur_conv[o + 0u] = u32(acc.x + 0.5);
        blur_conv[o + 1u] = u32(acc.y + 0.5);
        blur_conv[o + 2u] = u32(acc.z + 0.5);
        blur_conv[o + 3u] = u32(acc.w + 0.5);
    }
}

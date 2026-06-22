// Analytic-blur stage 1/3: downsample. The chaos-game pass splats blur means
// at FULL resolution (one slice per transform). The blur is low-frequency, so
// we convolve it at reduced resolution where the kernel is proportionally
// small — this is what keeps the convolution from being O(radius²) at full
// res. Here we sum each D×D full-res cell into one low-res texel (energy
// preserving: a rebin, not an average). See docs/projects/analytic-blur-buffer.md.

struct ConvolveParams {
    full_width: u32,
    full_height: u32,
    lowres_width: u32,
    lowres_height: u32,
    downscale: u32,    // D
    count: u32,        // active blur slots
    _pad0: u32,
    _pad1: u32,
    slot_meta: array<vec4<u32>, 4>,  // (lowres_half, weight_offset, _, _)
}

@group(0) @binding(0) var<storage, read> blur_full: array<u32>;
@group(0) @binding(1) var<storage, read_write> blur_low: array<u32>;
@group(0) @binding(2) var<uniform> params: ConvolveParams;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.lowres_width || gid.y >= params.lowres_height) {
        return;
    }
    let d = params.downscale;
    let full_plane = params.full_width * params.full_height;
    let low_plane = params.lowres_width * params.lowres_height;

    for (var s: u32 = 0u; s < params.count; s = s + 1u) {
        var sum = vec4<u32>(0u, 0u, 0u, 0u);
        for (var j: u32 = 0u; j < d; j = j + 1u) {
            let fy = gid.y * d + j;
            if (fy >= params.full_height) { continue; }
            for (var i: u32 = 0u; i < d; i = i + 1u) {
                let fx = gid.x * d + i;
                if (fx >= params.full_width) { continue; }
                let q = (s * full_plane + fy * params.full_width + fx) * 4u;
                sum = sum + vec4<u32>(
                    blur_full[q + 0u], blur_full[q + 1u],
                    blur_full[q + 2u], blur_full[q + 3u],
                );
            }
        }
        let o = (s * low_plane + gid.y * params.lowres_width + gid.x) * 4u;
        blur_low[o + 0u] = sum.x;
        blur_low[o + 1u] = sum.y;
        blur_low[o + 2u] = sum.z;
        blur_low[o + 3u] = sum.w;
    }
}

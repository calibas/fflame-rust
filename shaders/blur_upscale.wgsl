// Analytic-blur stage 3/3: upscale + add. Bilinearly upsample each slot's
// low-res convolved slice back to full resolution and ADD it into the main
// histogram. The blur is smooth, so bilinear upsampling is faithful.
//
// Energy: the downsample summed D×D cells into one low-res texel, so each
// low-res value is a density integral over a D×D full-res region. Bilinear
// upsampling reconstructs that integral at every full-res pixel, so we divide
// by D² to spread it back — the total density is preserved end-to-end and the
// per-pixel color ratio Σcolor/Σdensity is unchanged. See
// docs/projects/analytic-blur-buffer.md.
//
// One thread per full-res pixel; additive, so it preserves the direct
// (non-blur) samples already in the histogram.

struct ConvolveParams {
    full_width: u32,
    full_height: u32,
    lowres_width: u32,
    lowres_height: u32,
    downscale: u32,
    count: u32,
    _pad0: u32,
    _pad1: u32,
    slot_meta: array<vec4<u32>, 4>,
}

@group(0) @binding(0) var<storage, read> blur_conv: array<u32>;
@group(0) @binding(1) var<storage, read_write> histogram_out: array<u32>;
@group(0) @binding(2) var<uniform> params: ConvolveParams;

fn texel(slot_base: u32, x: i32, y: i32) -> vec4<f32> {
    let cx = u32(clamp(x, 0, i32(params.lowres_width) - 1));
    let cy = u32(clamp(y, 0, i32(params.lowres_height) - 1));
    let q = (slot_base + cy * params.lowres_width + cx) * 4u;
    return vec4<f32>(
        f32(blur_conv[q + 0u]), f32(blur_conv[q + 1u]),
        f32(blur_conv[q + 2u]), f32(blur_conv[q + 3u]),
    );
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.full_width || gid.y >= params.full_height) {
        return;
    }
    let d = f32(params.downscale);
    let plane = params.lowres_width * params.lowres_height;

    // Map this full-res pixel center to continuous low-res coordinates.
    let gx = (f32(gid.x) + 0.5) / d - 0.5;
    let gy = (f32(gid.y) + 0.5) / d - 0.5;
    let x0 = i32(floor(gx));
    let y0 = i32(floor(gy));
    let fx = gx - floor(gx);
    let fy = gy - floor(gy);
    let w00 = (1.0 - fx) * (1.0 - fy);
    let w10 = fx * (1.0 - fy);
    let w01 = (1.0 - fx) * fy;
    let w11 = fx * fy;

    var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    for (var s: u32 = 0u; s < params.count; s = s + 1u) {
        let base = s * plane;
        acc = acc
            + w00 * texel(base, x0, y0)
            + w10 * texel(base, x0 + 1, y0)
            + w01 * texel(base, x0, y0 + 1)
            + w11 * texel(base, x0 + 1, y0 + 1);
    }

    let inv = 1.0 / (d * d);
    let hb = (gid.y * params.full_width + gid.x) * 4u;
    histogram_out[hb + 0u] = histogram_out[hb + 0u] + u32(acc.x * inv + 0.5);
    histogram_out[hb + 1u] = histogram_out[hb + 1u] + u32(acc.y * inv + 0.5);
    histogram_out[hb + 2u] = histogram_out[hb + 2u] + u32(acc.z * inv + 0.5);
    histogram_out[hb + 3u] = histogram_out[hb + 3u] + u32(acc.w * inv + 0.5);
}

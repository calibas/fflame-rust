// Analytic-blur stage 3/3: upscale + add. Upsample each slot's low-res
// convolved slice back to full resolution with a cubic B-spline filter and ADD
// it into the main histogram.
//
// Why B-spline and not bilinear: for a large blur the low-res grid is coarse
// (downscale D can reach ~10), and bilinear upsampling of a coarse blob shows
// its piecewise-linear facets as visible bands / polygonal blobs. The cubic
// B-spline is C²-continuous and slightly smoothing (no ringing), so the
// reconstructed gradient is band-free. Its 4-tap-per-axis weights sum to 1, so
// energy is preserved exactly like bilinear.
//
// Energy: the downsample summed D×D cells into one low-res texel, so each
// low-res value is a density integral over a D×D full-res region. Upsampling
// reconstructs that integral at every full-res pixel, so we divide by D² to
// spread it back — total density is preserved end-to-end and the per-pixel
// color ratio Σcolor/Σdensity is unchanged. See
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

// One horizontal cubic tap-row: the four columns x0-1..x0+2 at row y,
// weighted by `wx` (B-spline weights for offsets -1,0,1,2).
fn w_row(slot_base: u32, x0: i32, y: i32, wx: vec4<f32>) -> vec4<f32> {
    return wx[0] * texel(slot_base, x0 - 1, y)
         + wx[1] * texel(slot_base, x0,     y)
         + wx[2] * texel(slot_base, x0 + 1, y)
         + wx[3] * texel(slot_base, x0 + 2, y);
}

// Cubic B-spline basis weights for the four taps at offsets (-1, 0, 1, 2)
// around a sample at fractional position t ∈ [0,1). Sum to 1; smooth, no
// negative lobes → no ringing.
fn bspline4(t: f32) -> vec4<f32> {
    let t2 = t * t;
    let t3 = t2 * t;
    let w0 = (1.0 - 3.0 * t + 3.0 * t2 - t3) / 6.0;       // (1-t)³/6
    let w1 = (4.0 - 6.0 * t2 + 3.0 * t3) / 6.0;
    let w2 = (1.0 + 3.0 * t + 3.0 * t2 - 3.0 * t3) / 6.0;
    let w3 = t3 / 6.0;
    return vec4<f32>(w0, w1, w2, w3);
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
    let wx = bspline4(gx - floor(gx));
    let wy = bspline4(gy - floor(gy));

    var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    for (var s: u32 = 0u; s < params.count; s = s + 1u) {
        let base = s * plane;
        // Separable cubic: Σ_j wy[j] · (Σ_i wx[i] · texel(x0-1+i, y0-1+j)).
        for (var j: i32 = 0; j < 4; j = j + 1) {
            let row = w_row(base, x0, y0 - 1 + j, wx);
            acc = acc + wy[j] * row;
        }
    }

    let inv = 1.0 / (d * d);
    let hb = (gid.y * params.full_width + gid.x) * 4u;
    histogram_out[hb + 0u] = histogram_out[hb + 0u] + u32(max(acc.x, 0.0) * inv + 0.5);
    histogram_out[hb + 1u] = histogram_out[hb + 1u] + u32(max(acc.y, 0.0) * inv + 0.5);
    histogram_out[hb + 2u] = histogram_out[hb + 2u] + u32(max(acc.z, 0.0) * inv + 0.5);
    histogram_out[hb + 3u] = histogram_out[hb + 3u] + u32(max(acc.w, 0.0) * inv + 0.5);
}

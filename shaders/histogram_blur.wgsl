// Separable Gaussian blur on the per-batch histogram. Mirrors Apophysis's
// `filter` attribute — a small Gaussian applied to per-iteration samples so
// the grain doesn't manifest as visible per-pixel dots at low density.
//
// The histogram holds raw u32-scaled sums: r_sum, g_sum, b_sum, density per
// pixel. Each pixel writes its weighted-average neighborhood. Doing this on
// the histogram (pre-accumulate, pre-tonemap) keeps the blur in linear
// units across all four channels uniformly — color and density spread
// together, which is automatically density-weighted.
//
// Dispatched twice per batch when filter_radius > 0:
//   1. Horizontal pass: primary → scratch
//   2. Vertical pass:   scratch → primary
// The accumulate pass that runs immediately afterward reads the primary
// buffer unchanged.

struct BlurParams {
    width: u32,
    height: u32,
    radius_pixels: f32,     // spatial Gaussian sigma in pixels (Apo's `filter`)
    direction: u32,         // 0 = horizontal, 1 = vertical
    // Bilateral density-similarity sigma (u32 histogram density units).
    // Per-tap weight = spatial_gaussian * exp(-Δdensity² / 2σ_d²).
    // Large value → uniform Gaussian; small value → tight edge preservation.
    density_sigma: f32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var<storage, read> histogram_in: array<u32>;
@group(0) @binding(1) var<storage, read_write> histogram_out: array<u32>;
@group(0) @binding(2) var<uniform> params: BlurParams;

// Kernel half-width: cap at 8 px so the inner loop stays bounded.
// For Apo's typical filter=0.5 the relevant taps are within ±2 px.
const KERNEL_HALF_MAX: i32 = 8;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    if (global_id.x >= params.width || global_id.y >= params.height) {
        return;
    }

    let x = i32(global_id.x);
    let y = i32(global_id.y);
    let w = i32(params.width);
    let h = i32(params.height);
    let sigma = max(params.radius_pixels, 0.0001);
    // 3σ window captures >99% of the Gaussian; clamp to the static cap so the
    // loop count is bounded and WGSL can unroll predictably.
    let kernel_half = min(i32(ceil(3.0 * sigma)), KERNEL_HALF_MAX);
    let inv_2sigma2 = 1.0 / (2.0 * sigma * sigma);

    // Bilateral density-similarity. The standard separable-bilateral
    // approximation: each pass compares each tap to the input buffer's
    // center value. On the H pass this is the original density; on the V
    // pass it's the H-blurred density. Slightly less correct than a true
    // 2D bilateral but visually indistinguishable at small radii.
    let center_idx = (u32(y) * params.width + u32(x)) * 4u;
    let center_density = f32(histogram_in[center_idx + 3u]);
    let sigma_d = max(params.density_sigma, 1.0);
    let inv_2sigma_d2 = 1.0 / (2.0 * sigma_d * sigma_d);

    // Accumulate weighted sums (f32) and total weight, then convert back to u32.
    var sum_r: f32 = 0.0;
    var sum_g: f32 = 0.0;
    var sum_b: f32 = 0.0;
    var sum_d: f32 = 0.0;
    var weight_total: f32 = 0.0;

    for (var k: i32 = -KERNEL_HALF_MAX; k <= KERNEL_HALF_MAX; k = k + 1) {
        // Mask out taps beyond the dynamic kernel_half so larger radii
        // dispatch the same loop count without contributing junk.
        if (abs(k) > kernel_half) { continue; }

        var nx: i32 = x;
        var ny: i32 = y;
        if (params.direction == 0u) {
            nx = clamp(x + k, 0, w - 1);
        } else {
            ny = clamp(y + k, 0, h - 1);
        }
        let neighbor_idx = (u32(ny) * params.width + u32(nx)) * 4u;

        let tap_r = f32(histogram_in[neighbor_idx + 0u]);
        let tap_g = f32(histogram_in[neighbor_idx + 1u]);
        let tap_b = f32(histogram_in[neighbor_idx + 2u]);
        let tap_d = f32(histogram_in[neighbor_idx + 3u]);

        let fk = f32(k);
        let w_spatial = exp(-fk * fk * inv_2sigma2);
        let delta = tap_d - center_density;
        let w_density = exp(-delta * delta * inv_2sigma_d2);
        let w_tap = w_spatial * w_density;

        sum_r = sum_r + tap_r * w_tap;
        sum_g = sum_g + tap_g * w_tap;
        sum_b = sum_b + tap_b * w_tap;
        sum_d = sum_d + tap_d * w_tap;
        weight_total = weight_total + w_tap;
    }

    let inv_w = 1.0 / max(weight_total, 1e-6);
    let out_idx = (u32(y) * params.width + u32(x)) * 4u;
    histogram_out[out_idx + 0u] = u32(sum_r * inv_w);
    histogram_out[out_idx + 1u] = u32(sum_g * inv_w);
    histogram_out[out_idx + 2u] = u32(sum_b * inv_w);
    histogram_out[out_idx + 3u] = u32(sum_d * inv_w);
}

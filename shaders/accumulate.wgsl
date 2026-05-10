// Accumulation shader — cumulative-add version (Phase 8b of the
// accumulator-unification project). The previous EMA-blend version
// stored a running-average of recent batches' colors and a
// nonlinearly-scaled density (`prev.a + density × 0.01 × blend_factor`),
// which made `iterations_per_thread` and frame count both leak into
// brightness because the stored alpha didn't equal a clean
// "iterations per pixel" quantity.
//
// Cumulative add matches what every reference fractal flame
// renderer does (flam3, Apophysis 7X, Ember/Fractorium, JWildfire,
// cuburn). The accumulator stores:
//   - rgb: density-weighted running average color, in [0,1]
//   - a:   raw iteration count for this pixel (sum of hits)
// Combined with the scale-invariant `sample_density = total_iters /
// pixel_count` in the tonemap (Phase 8a), the product `alpha × k2`
// in the tonemap formula stays stationary as samples accumulate —
// brightness no longer drifts with sample count, ipt becomes a pure
// speed knob.
//
// Uses ping-pong textures to work around read-write limitations.
// See docs/projects/accumulator-unification.md.

struct AccumulateParams {
    width: u32,
    height: u32,
    blend_factor: f32, // Now repurposed: blend_factor ≥ 0.99 is the
                      // "overwrite" signal (clear prev before adding).
                      // Previously also drove EMA strength.
    histogram_color_scale: f32, // Must match compute shader value
    target_iterations_per_pixel: u32, // Per-pixel convergence threshold (0 = disabled)
    _pad0: f32,
    background_r: f32, // Unused - kept for struct layout compatibility
    background_g: f32,
    background_b: f32,
    _pad1: f32,
}

@group(0) @binding(0) var previous_accumulation: texture_2d<f32>;
@group(0) @binding(1) var<storage, read> histogram: array<u32>;
@group(0) @binding(2) var output_texture: texture_storage_2d<rgba32float, write>;
@group(0) @binding(3) var<uniform> params: AccumulateParams;
@group(0) @binding(4) var<storage, read> iteration_counts: array<u32>;  // Per-pixel iteration counts

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = vec2<i32>(i32(global_id.x), i32(global_id.y));

    // Bounds check
    if (global_id.x >= params.width || global_id.y >= params.height) {
        return;
    }

    // Overwrite mode (slider-drag etc): discard prev so this dispatch
    // starts fresh. Lets the host signal "reset accumulation" without
    // a separate clear pass.
    let is_overwrite_mode = params.blend_factor >= 0.99;
    let prev_raw = textureLoad(previous_accumulation, pixel, 0);
    let prev = select(prev_raw, vec4<f32>(0.0), is_overwrite_mode);

    // Read this batch's contributions to this pixel from the
    // histogram. Compute shader stores 4× u32 per pixel: scaled R, G,
    // B sums and density (where density = histogram_color_scale × N
    // and the color sums are histogram_color_scale × Σ color).
    let pixel_idx = global_id.y * params.width + global_id.x;
    let base_idx = pixel_idx * 4u;
    let r_sum = f32(histogram[base_idx + 0u]);
    let g_sum = f32(histogram[base_idx + 1u]);
    let b_sum = f32(histogram[base_idx + 2u]);
    let scaled_density = f32(histogram[base_idx + 3u]);

    // Per-pixel convergence gate: once a pixel's iteration count hits
    // the user's target, stop accumulating into it (keeps the rest of
    // the image catching up while bright spots don't run away).
    let pixel_iterations = iteration_counts[pixel_idx];
    let has_some_density = prev.a > 0.01;
    let is_converged = params.target_iterations_per_pixel > 0u && has_some_density && pixel_iterations >= params.target_iterations_per_pixel;
    let convergence_gate = select(1.0, 0.0, is_converged);

    // Convert scaled density to raw iteration count. histogram_color_scale
    // is the multiplier the compute shader applies to keep u32 atomic
    // adds in a useful precision range; we undo it here so the stored
    // alpha is "iteration count for this pixel," matching the units
    // the tonemap's sample_density expects (iters / pixel_count).
    let color_scale = max(params.histogram_color_scale, 1.0);
    let new_density = (scaled_density / color_scale) * convergence_gate;
    let total_density = prev.a + new_density;

    // No new contribution this pixel this dispatch — keep prev. (In
    // overwrite mode, prev was already cleared above, so this stores
    // zero, which is the right "fresh start" behavior.)
    if (new_density <= 0.0) {
        textureStore(output_texture, pixel, prev);
        return;
    }

    // This batch's average color at this pixel: Σ scaled_color / scaled_density.
    // The histogram_color_scale factor in numerator and denominator
    // cancels, so this is the unscaled batch-average color in [0,1].
    let new_color = clamp(
        vec3<f32>(r_sum, g_sum, b_sum) / max(scaled_density, 1.0),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );

    // Density-weighted running mean. Mathematically identical to
    // "store r_sum cumulative, divide by alpha at tonemap time" but
    // keeps RGB in [0,1] so the tonemap shader's existing input
    // expectation (color × log_scale) stays right.
    //   prev_avg × prev_density + new_avg × new_density = total_color_sum
    //   total_color_sum / total_density = new_running_avg
    let cumulative_rgb = (prev.rgb * prev.a + new_color * new_density) / total_density;

    textureStore(output_texture, pixel, vec4<f32>(cumulative_rgb, total_density));
}

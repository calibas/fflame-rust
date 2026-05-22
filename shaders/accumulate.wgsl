// Accumulation shader — adaptive blend (Phase 8c follow-up). The
// accumulator stores:
//   - rgb: density-weighted running mean color, in [0,1]
//   - a:   raw iteration count for this pixel (sum of hits, cumulative)
//
// Density is always cumulative (matches `sample_density = total_iters
// / pixel_count` in the tonemap). Color update uses an adaptive blend
// rate:
//
//     effective_blend = max(new_density / total_density, blend_factor)
//
// `new_density / total_density` is the cumulative-mean contribution
// rate (1.0 at first batch, → 0 as samples grow). `blend_factor` is
// a user-controlled floor.
//
// `blend_factor = 0`  → effective = cumulative rate. Pure
//                       density-weighted running mean — matches the
//                       reference Fractal Flame algorithm. Asymptotic
//                       precision floor on f32 is ~10^9 iters/pixel
//                       (mantissa caps the `prev * prev_density +
//                       new * new_density` sum precision).
// `blend_factor > 0`  → effective floors at this rate once cumulative
//                       drops below it. Each batch keeps contributing
//                       at least `blend_factor` × (new_color − prev),
//                       which stays well above f32 precision for any
//                       practical floor. Slight recency bias in
//                       exchange for unbounded sample-count headroom.
//                       Set this to ~0.001 for high-quality renders
//                       past ~10^9 iters.
// `blend_factor ≥ 0.99` → overwrite mode (clear prev before add).
//                         Used by the host as the parameter-change
//                         reset signal.
//
// Pre-Phase-8b this was a true EMA `prev * (1-α) + new * α` with
// nonlinearly-scaled density `prev.a + density × 0.01 × blend_factor`.
// That made the stored alpha not match "iterations per pixel," which
// leaked iters_per_thread and frame count into brightness.
//
// Uses ping-pong textures to work around read-write limitations.
// See docs/projects/accumulator-unification.md.

struct AccumulateParams {
    width: u32,
    height: u32,
    // EMA blend strength when use_fixed_ema != 0. Also overloaded as
    // the "overwrite" signal: blend_factor ≥ 0.99 means clear prev
    // before add (used for parameter-change reset).
    blend_factor: f32,
    histogram_color_scale: f32, // Must match compute shader value
    // Mode flag (was _pad0):
    //   0 = cumulative-mean — α per pixel = new_density / total_density.
    //       Matches the reference Fractal Flame algorithm. Brightness
    //       correct from the first frame. Hits an f32 precision wall
    //       around ~10^9 iters/pixel where each batch's contribution
    //       drops below the storage's smallest representable change.
    //   1 = fixed EMA — α = blend_factor, applied uniformly. First
    //       frames are dim (stored value bootstraps from 0 at rate α);
    //       steady state is the running EMA mean. Precision-stable
    //       indefinitely because each batch's contribution stays at
    //       a constant proportion of the stored value, well above
    //       f32 precision for any reasonable blend rate.
    use_fixed_ema: u32,
    background_r: f32, // Unused - kept for struct layout compatibility
    background_g: f32,
    background_b: f32,
    _pad1: f32,
}

@group(0) @binding(0) var previous_accumulation: texture_2d<f32>;
@group(0) @binding(1) var<storage, read> histogram: array<u32>;
@group(0) @binding(2) var output_texture: texture_storage_2d<rgba32float, write>;
@group(0) @binding(3) var<uniform> params: AccumulateParams;

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

    // Convert scaled density to raw iteration count. histogram_color_scale
    // is the multiplier the compute shader applies to keep u32 atomic
    // adds in a useful precision range; we undo it here so the stored
    // alpha is "iteration count for this pixel," matching the units
    // the tonemap's sample_density expects (iters / pixel_count).
    let color_scale = max(params.histogram_color_scale, 1.0);
    let new_density = scaled_density / color_scale;
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

    // Pick α based on mode (see AccumulateParams::use_fixed_ema doc):
    //   0 → cumulative-mean rate (α = new_density / total_density)
    //   1 → fixed EMA at blend_factor
    // Lerp formulation `prev * (1-α) + new * α` is used in both modes;
    // it keeps both terms in [0,1] range and is algebraically identical
    // to the density-weighted mean for cumulative mode.
    let cumulative_alpha = new_density / total_density;
    let alpha = select(cumulative_alpha, params.blend_factor, params.use_fixed_ema != 0u);
    let blended_rgb = prev.rgb * (1.0 - alpha) + new_color * alpha;

    textureStore(output_texture, pixel, vec4<f32>(blended_rgb, total_density));
}

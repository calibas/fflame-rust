// Accumulation shader - adds new samples to existing accumulation buffer
// Uses ping-pong textures to work around read-write limitations

struct AccumulateParams {
    width: u32,
    height: u32,
    blend_factor: f32, // samples_this_frame / samples_accumulated
    histogram_color_scale: f32, // Must match compute shader value
    low_density_smoothing: f32, // 0.0 = no smoothing, 1.0 = max smoothing
    density_compression_strength: f32, // 0.0 = linear, 5.0 = strong compression
    target_iterations_per_pixel: u32, // Per-pixel convergence threshold (0 = disabled)
    _pad0: f32,
    _pad1: f32, // Padding for alignment
    _pad2: f32,
    _pad3: f32,
    _pad4: f32,
}

@group(0) @binding(0) var previous_accumulation: texture_2d<f32>;
@group(0) @binding(1) var<storage, read> histogram: array<u32>;
@group(0) @binding(2) var output_texture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> params: AccumulateParams;
@group(0) @binding(4) var<storage, read> iteration_counts: array<u32>;  // Per-pixel iteration counts

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = vec2<i32>(i32(global_id.x), i32(global_id.y));

    // Bounds check
    if (global_id.x >= params.width || global_id.y >= params.height) {
        return;
    }

    // Load previous accumulation
    let prev = textureLoad(previous_accumulation, pixel, 0);

    // Read histogram values for this pixel (4 u32 words: R, G, B, density)
    let pixel_idx = global_id.y * params.width + global_id.x;
    let base_idx = pixel_idx * 4u;  // 4 words per pixel

    // Read 4 separate u32 values (no unpacking needed)
    let r_sum = f32(histogram[base_idx + 0u]);
    let g_sum = f32(histogram[base_idx + 1u]);
    let b_sum = f32(histogram[base_idx + 2u]);
    let density = f32(histogram[base_idx + 3u]);

    // Convert back to float color (average)
    // Density includes scale (density = sum of pixel_scale per hit)
    // So we divide by density only, not (density × pixel_scale)
    var new_color = vec3<f32>(0.0);
    if (density > 0.0) {
        new_color = vec3<f32>(
            r_sum / density,
            g_sum / density,
            b_sum / density
        );

        // Clamp to valid range
        new_color = clamp(new_color, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    // Blend new samples with previous accumulation
    // Check per-pixel convergence: has this pixel received enough iterations?
    let pixel_iterations = iteration_counts[pixel_idx];

    // Only apply convergence check if pixel has some accumulated density
    // This prevents empty spots on first frame when dense pixels immediately hit the limit
    let has_some_density = prev.a > 0.01;
    let is_converged = params.target_iterations_per_pixel > 0u && has_some_density && pixel_iterations >= params.target_iterations_per_pixel;

    // Gate blend_factor based on convergence (hard cutoff)
    // If converged, stop all accumulation (convergence_gate = 0.0)
    let convergence_gate = select(1.0, 0.0, is_converged);

    // Adaptive blending based on accumulated density to reduce low-density noise
    let density_threshold = 0.1;
    let density_factor = mix(1.0, min(prev.a / density_threshold, 1.0), params.low_density_smoothing);

    // Apply density compression to slow accumulation in bright areas
    // Formula adjusted for actual density scale (0-100+)
    // Use linear term instead of squared to avoid saturation
    let compression_factor = 1.0 / (1.0 + prev.a * params.density_compression_strength * 0.01);

    // Multiply all factors together: global blend × density × compression × convergence
    let adjusted_blend = params.blend_factor * density_factor * compression_factor * convergence_gate;

    // ALWAYS blend (like ce58657), even when density==0, to prevent smearing during interactive editing
    let rgb_accumulated = prev.rgb * (1.0 - adjusted_blend) + new_color * adjusted_blend;

    // Alpha (density) handling:
    // - Normal mode (blend_factor < 1.0): Additive accumulation
    // - Overwrite mode (blend_factor == 1.0): Replace previous alpha
    // This prevents progressive brightness during live preview drag
    let new_alpha = density * 0.01 * params.blend_factor * convergence_gate;
    let alpha_accumulated = select(
        prev.a + new_alpha,        // Normal: accumulate
        new_alpha,                 // Overwrite: replace
        params.blend_factor >= 0.99  // Check for overwrite mode (float comparison tolerance)
    );

    // Write to output
    textureStore(output_texture, pixel, vec4<f32>(rgb_accumulated, alpha_accumulated));
}

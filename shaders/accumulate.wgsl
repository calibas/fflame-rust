// Accumulation shader - adds new samples to existing accumulation buffer
// Uses ping-pong textures to work around read-write limitations

struct AccumulateParams {
    width: u32,
    height: u32,
    blend_factor: f32, // samples_this_frame / samples_accumulated
    histogram_color_scale: f32, // Must match compute shader value
    low_density_smoothing: f32, // 0.0 = no smoothing, 1.0 = max smoothing
    _pad: vec3<f32>, // Padding for alignment
}

@group(0) @binding(0) var previous_accumulation: texture_2d<f32>;
@group(0) @binding(1) var<storage, read> histogram: array<u32>;
@group(0) @binding(2) var output_texture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> params: AccumulateParams;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = vec2<i32>(i32(global_id.x), i32(global_id.y));

    // Bounds check
    if (global_id.x >= params.width || global_id.y >= params.height) {
        return;
    }

    // Load previous accumulation
    let prev = textureLoad(previous_accumulation, pixel, 0);

    // Read u16 packed histogram values for this pixel
    let pixel_idx = global_id.y * params.width + global_id.x;
    let base_idx = pixel_idx * 2u;

    // Unpack 2× u32 into 4× u16 (RGBA + density)
    let packed_rg = histogram[base_idx + 0u];
    let packed_bd = histogram[base_idx + 1u];

    // Extract u16 values using bit masks and shifts
    let r_sum = f32(packed_rg & 0xFFFFu);
    let g_sum = f32((packed_rg >> 16u) & 0xFFFFu);
    let b_sum = f32(packed_bd & 0xFFFFu);
    let density = f32((packed_bd >> 16u) & 0xFFFFu);

    // Convert back to float color (average)
    // Note: Values were scaled by histogram_color_scale during accumulation
    let color_scale = params.histogram_color_scale;
    var new_color = vec3<f32>(0.0);
    if (density > 0.0) {
        // Simple division - just accept that overflow will cause color wrapping
        // This is a known limitation of the u16 packing approach
        new_color = vec3<f32>(
            r_sum / (density * color_scale),
            g_sum / (density * color_scale),
            b_sum / (density * color_scale)
        );

        // Clamp to valid range
        new_color = clamp(new_color, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    // Blend RGB with previous accumulation (weighted average by sample count)
    // IMPORTANT: Only blend if this pixel received hits this batch (density > 0)
    // Otherwise we'd pull low-density areas toward black/grey
    var rgb_accumulated = prev.rgb;
    if (density > 0.0) {
        // Adaptive blending based on accumulated density to reduce low-density noise
        // Low-density pixels (prev.a ≈ 0) get reduced blend weight to suppress noise
        // High-density pixels (prev.a >> 0) use full blend_factor for accurate accumulation

        // Smoothing factor: 0.0 = no smoothing, 1.0 = maximum smoothing
        // When smoothing = 0, density_factor = 1.0 (no reduction)
        // When smoothing = 1, density_factor ramps from 0.0 to 1.0 as density increases
        let density_threshold = 0.1; // Density at which full blending is reached
        let density_factor = mix(1.0, min(prev.a / density_threshold, 1.0), params.low_density_smoothing);

        let adjusted_blend = params.blend_factor * density_factor;
        rgb_accumulated = prev.rgb * (1.0 - adjusted_blend) + new_color * adjusted_blend;
    }

    // Alpha (density) accumulates additively, but needs to be scaled by blend_factor
    // to account for the fact that blend_factor represents samples_this_batch / total_samples
    // Each hit adds 0.01 alpha, scaled by the batch's weight in the total
    let alpha_accumulated = prev.a + (density * 0.01 * params.blend_factor);

    // Write to output
    textureStore(output_texture, pixel, vec4<f32>(rgb_accumulated, alpha_accumulated));
}

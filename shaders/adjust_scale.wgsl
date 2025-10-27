// Adjust per-pixel scale values based on accumulated density
// Runs after compute pass, before accumulate pass
// Prevents overflow by reducing scale in high-density areas
// Maximizes precision by increasing scale in low-density areas

struct AdjustScaleParams {
    width: u32,
    height: u32,
    overflow_threshold: f32,      // Max safe accumulated value (default: 50000)
    high_density_threshold: f32,  // Density to trigger scale reduction (default: 100)
    low_density_threshold: f32,   // Density to trigger scale increase (default: 10)
    scale_adjust_rate: f32,       // How aggressively to adjust (default: 0.1)
    min_scale: f32,               // Minimum allowed scale (default: 1.0)
    max_scale: f32,               // Maximum allowed scale (default: 100.0)
}

@group(0) @binding(0) var accumulation_texture: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> scale_buffer: array<u32>;
@group(0) @binding(2) var<uniform> params: AdjustScaleParams;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // Bounds check
    if (global_id.x >= params.width || global_id.y >= params.height) {
        return;
    }

    let pixel_idx = global_id.y * params.width + global_id.x;

    // Load current scale (unpacked u32, one per pixel)
    let current_scale = f32(scale_buffer[pixel_idx]);

    // Read accumulated color and density from accumulation texture
    let pixel_color = textureLoad(accumulation_texture, vec2<i32>(global_id.xy), 0);

    // Alpha channel contains accumulated density (normalized, 0-1 range)
    // Values typically range from 0.0 (no hits) to ~1.0 (high density over time)
    let density = pixel_color.a * 100.0;  // Scale to 0-100 range for threshold comparison

    // RGB channels contain accumulated color (already tone-mapped in accumulation buffer)
    // Estimate overflow risk from color intensity
    let max_color = max(max(pixel_color.r, pixel_color.g), pixel_color.b);
    let max_accumulated = max_color * 65535.0;  // Approximate accumulated value

    // Determine new scale based on density and overflow risk
    var new_scale = current_scale;

    // HIGH PRIORITY: Prevent overflow
    // If any channel is approaching overflow threshold, reduce scale aggressively
    if (max_accumulated > params.overflow_threshold) {
        let overflow_risk = max_accumulated / params.overflow_threshold;
        // Reduce scale proportionally to overflow risk
        new_scale = current_scale / (1.0 + params.scale_adjust_rate * overflow_risk);
    }
    // High density: reduce scale to prevent future overflow
    else if (density > params.high_density_threshold) {
        let density_factor = density / params.high_density_threshold;
        new_scale = current_scale / (1.0 + params.scale_adjust_rate * density_factor * 0.5);
    }
    // Low density: increase scale for better precision
    else if (density < params.low_density_threshold && density > 0.0) {
        let sparsity_factor = params.low_density_threshold / density;
        new_scale = current_scale * (1.0 + params.scale_adjust_rate * sparsity_factor * 0.2);
    }

    // Clamp to valid range
    new_scale = clamp(new_scale, params.min_scale, params.max_scale);

    // Write back to scale buffer (unpacked, direct write - no race condition)
    scale_buffer[pixel_idx] = u32(new_scale);
}

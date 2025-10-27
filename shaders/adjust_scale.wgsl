// Adjust per-pixel scale values based on histogram density
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

@group(0) @binding(0) var<storage, read> histogram: array<u32>;
@group(0) @binding(1) var<storage, read_write> scale_buffer: array<u32>;
@group(0) @binding(2) var<uniform> params: AdjustScaleParams;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // Bounds check
    if (global_id.x >= params.width || global_id.y >= params.height) {
        return;
    }

    let pixel_idx = global_id.y * params.width + global_id.x;
    let base_idx = pixel_idx * 2u;

    // Load current scale
    let scale_word_idx = pixel_idx / 2u;
    let scale_word = scale_buffer[scale_word_idx];
    let is_odd = (pixel_idx % 2u) == 1u;

    let current_scale = f32(select(
        scale_word & 0xFFFFu,          // Even pixel (low 16 bits)
        (scale_word >> 16u) & 0xFFFFu, // Odd pixel (high 16 bits)
        is_odd
    ));

    // Read histogram values
    let packed_rg = histogram[base_idx + 0u];
    let packed_bd = histogram[base_idx + 1u];

    let r_sum = f32(packed_rg & 0xFFFFu);
    let g_sum = f32((packed_rg >> 16u) & 0xFFFFu);
    let b_sum = f32(packed_bd & 0xFFFFu);
    let density = f32((packed_bd >> 16u) & 0xFFFFu);

    // Calculate maximum accumulated value
    let max_accumulated = max(max(r_sum, g_sum), b_sum);

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

    // Write back to scale buffer
    let new_scale_u16 = u32(new_scale);

    if (is_odd) {
        // Odd pixel: high 16 bits
        let new_word = (scale_word & 0xFFFFu) | (new_scale_u16 << 16u);
        scale_buffer[scale_word_idx] = new_word;
    } else {
        // Even pixel: low 16 bits
        let new_word = (scale_word & 0xFFFF0000u) | new_scale_u16;
        scale_buffer[scale_word_idx] = new_word;
    }
}

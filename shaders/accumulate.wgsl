// Accumulation shader - adds new samples to existing accumulation buffer
// Uses ping-pong textures to work around read-write limitations

struct AccumulateParams {
    width: u32,
    height: u32,
    blend_factor: f32, // 1.0 / (samples_accumulated + 1)
    _pad0: f32,
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

    // Read histogram values for this pixel
    let pixel_idx = global_id.y * params.width + global_id.x;
    let base_idx = pixel_idx * 4u;

    // Read histogram values (u32)
    let r_sum = f32(histogram[base_idx + 0u]);
    let g_sum = f32(histogram[base_idx + 1u]);
    let b_sum = f32(histogram[base_idx + 2u]);
    let density = f32(histogram[base_idx + 3u]);

    // Convert back to float color (average)
    let color_scale = 10000.0;
    var new_color = vec3<f32>(0.0);
    if (density > 0.0) {
        new_color = vec3<f32>(
            r_sum / (density * color_scale),
            g_sum / (density * color_scale),
            b_sum / (density * color_scale)
        );
    }

    // Blend RGB with previous accumulation (weighted average by sample count)
    // blend_factor = samples_this_frame / total_samples
    let rgb_accumulated = prev.rgb * (1.0 - params.blend_factor) + new_color * params.blend_factor;

    // Alpha (density) represents total sample count, so accumulate it
    // Each hit in the old system added 0.01 alpha
    let alpha_accumulated = prev.a + (density * 0.01);

    // Write to output
    textureStore(output_texture, pixel, vec4<f32>(rgb_accumulated, alpha_accumulated));
}

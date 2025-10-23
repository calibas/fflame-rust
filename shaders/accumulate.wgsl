// Accumulation shader - adds new samples to existing accumulation buffer
// Uses ping-pong textures to work around read-write limitations

struct AccumulateParams {
    width: u32,
    height: u32,
    blend_factor: f32, // 1.0 / (samples_accumulated + 1)
    _pad0: f32,
}

@group(0) @binding(0) var previous_accumulation: texture_2d<f32>;
@group(0) @binding(1) var new_samples: texture_2d<f32>;
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

    // Load new samples
    let new_sample = textureLoad(new_samples, pixel, 0);

    // Blend RGB with previous accumulation (weighted average by sample count)
    // blend_factor = samples_this_frame / total_samples
    let rgb_accumulated = prev.rgb * (1.0 - params.blend_factor) + new_sample.rgb * params.blend_factor;

    // Alpha (density) represents total sample count, so just add it
    // Each sample writes 0.01 alpha, so alpha = total_samples * 0.01
    let alpha_accumulated = prev.a + new_sample.a;

    // Write to output
    textureStore(output_texture, pixel, vec4<f32>(rgb_accumulated, alpha_accumulated));
}

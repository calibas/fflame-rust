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

    // Blend with previous accumulation
    // Simple average: accumulated = (accumulated * n + new_sample) / (n + 1)
    // Equivalent to: accumulated = accumulated * (1 - 1/(n+1)) + new_sample * (1/(n+1))
    let accumulated = prev * (1.0 - params.blend_factor) + new_sample * params.blend_factor;

    // Write to output
    textureStore(output_texture, pixel, accumulated);
}

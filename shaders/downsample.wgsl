// Downsample shader for supersampling
// Uses box filter (simple average) to reduce resolution
// from internal render size to display size

struct DownsampleParams {
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    scale: u32,  // 2 for 2x, 4 for 4x supersampling
    _padding: vec3<u32>,
}

@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var dst_texture: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> params: DownsampleParams;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // Output pixel coordinates
    let dst_x = global_id.x;
    let dst_y = global_id.y;

    // Bounds check
    if (dst_x >= params.dst_width || dst_y >= params.dst_height) {
        return;
    }

    // Source pixel coordinates (top-left of sample region)
    let src_x = dst_x * params.scale;
    let src_y = dst_y * params.scale;

    // Box filter: average all pixels in the scale x scale region
    var color_sum = vec4<f32>(0.0);
    var sample_count = 0u;

    for (var dy = 0u; dy < params.scale; dy = dy + 1u) {
        for (var dx = 0u; dx < params.scale; dx = dx + 1u) {
            let sample_x = src_x + dx;
            let sample_y = src_y + dy;

            if (sample_x < params.src_width && sample_y < params.src_height) {
                color_sum += textureLoad(src_texture, vec2<i32>(i32(sample_x), i32(sample_y)), 0);
                sample_count = sample_count + 1u;
            }
        }
    }

    // Average
    let final_color = select(
        vec4<f32>(0.0),
        color_sum / f32(sample_count),
        sample_count > 0u
    );

    textureStore(dst_texture, vec2<i32>(i32(dst_x), i32(dst_y)), final_color);
}

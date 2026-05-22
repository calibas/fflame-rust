// Accumulation shader for tiled rendering
// Reads from a specific tile's portion of the histogram buffer

struct AccumulateParams {
    width: u32,       // Tile width
    height: u32,      // Tile height
    blend_factor: f32,
    histogram_color_scale: f32,
    _pad0: f32,
    background_r: f32,
    background_g: f32,
    background_b: f32,
    _pad1: f32,
}

struct TileInfo {
    tile_index: u32,      // Which tile we're processing
    tile_size: u32,       // Size of each tile (square)
    _padding: vec2<u32>,
}

@group(0) @binding(0) var previous_accumulation: texture_2d<f32>;
@group(0) @binding(1) var<storage, read> histogram: array<u32>;
@group(0) @binding(2) var output_texture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> params: AccumulateParams;
@group(0) @binding(4) var<uniform> tile_info: TileInfo;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = vec2<i32>(i32(global_id.x), i32(global_id.y));

    // Bounds check against tile dimensions
    if (global_id.x >= params.width || global_id.y >= params.height) {
        return;
    }

    // Load previous accumulation
    let prev = textureLoad(previous_accumulation, pixel, 0);

    // Calculate offset into tiled histogram buffer
    let pixels_per_tile = tile_info.tile_size * tile_info.tile_size;
    let tile_offset = tile_info.tile_index * pixels_per_tile * 4u;  // 4 words per pixel

    // Local pixel index within tile
    let local_pixel_idx = global_id.y * params.width + global_id.x;
    let base_idx = tile_offset + local_pixel_idx * 4u;

    // Read histogram values
    let r_sum = f32(histogram[base_idx + 0u]);
    let g_sum = f32(histogram[base_idx + 1u]);
    let b_sum = f32(histogram[base_idx + 2u]);
    let density = f32(histogram[base_idx + 3u]);

    // Detect overwrite mode
    let is_overwrite_mode = params.blend_factor >= 0.99;

    if (density == 0.0) {
        let output = select(prev, vec4<f32>(0.0, 0.0, 0.0, 0.0), is_overwrite_mode);
        textureStore(output_texture, pixel, output);
        return;
    }

    // Convert histogram to averaged color
    let new_color = clamp(vec3<f32>(
        r_sum / density,
        g_sum / density,
        b_sum / density
    ), vec3<f32>(0.0), vec3<f32>(1.0));

    // Blend RGB
    let blend_trust = clamp(prev.a / 0.05, 0.0, 1.0);
    let blended_rgb = prev.rgb * (1.0 - params.blend_factor) + new_color * params.blend_factor;
    let rgb_accumulated = mix(new_color, blended_rgb, blend_trust);

    // Alpha accumulation
    let new_alpha = density * 0.01 * params.blend_factor;
    let alpha_accumulated = select(
        prev.a + new_alpha,
        new_alpha,
        params.blend_factor >= 0.99
    );

    textureStore(output_texture, pixel, vec4<f32>(rgb_accumulated, alpha_accumulated));
}

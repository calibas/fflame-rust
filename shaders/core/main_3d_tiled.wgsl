// Main compute shader entry point for 3D tiled rendering
// Samples are routed to appropriate tile buffers based on screen coordinates

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let thread_id = global_id.x;

    // Initialize RNG
    var rng = rng_init(thread_id, params.seed);

    // Starting point (random in [-1, 1])
    var current = vec3<f32>(
        rng_nextf(&rng) * 2.0 - 1.0,
        rng_nextf(&rng) * 2.0 - 1.0,
        rng_nextf(&rng) * 2.0 - 1.0
    );

    var color = vec3<f32>(1.0, 1.0, 1.0);
    var color_index = 0.0;

    // Pre-calculate tile buffer size (pixels per tile × 4 channels)
    let tile_buffer_size = tile_params.tile_size * tile_params.tile_size * 4u;

    // Iterate
    for (var i = 0u; i < params.iterations_per_thread; i++) {
        let old_pos = current;

        // Select random transform
        let rand_val = rng_nextf(&rng);
        let xform_idx = select_transform(rand_val);
        let xform = transforms[xform_idx];

        // Opacity check
        if (rng_nextf(&rng) >= xform.opacity) {
            continue;
        }

        // Apply affine + variations (3D)
        let affine_p = apply_affine_3d(xform, current);
        current = apply_variations_3d(xform, xform_idx, affine_p, &rng);

        // Calculate speed
        let speed = length(current - old_pos);

        // Update color
        if (params.color_mode == 0u) {
            let symmetry = xform.color_speed;
            let colorC1 = (1.0 + symmetry) / 2.0;
            let colorC2 = xform.color * (1.0 - symmetry) / 2.0;
            color_index = color_index * colorC1 + colorC2;
        } else {
            let speed_color = speed_to_color(speed);
            color = mix(color, speed_color, params.speed_factor);
        }

        // Skip burn-in
        if (i >= params.burn_in) {
            // Apply final transform if present
            var final_pos = current;
            if (params.has_final_transform != 0u) {
                let final_xform = transforms[params.final_transform_index];
                let affine_p = apply_affine_3d(final_xform, current);
                final_pos = apply_variations_3d(final_xform, params.final_transform_index, affine_p, &rng);
            }

            // Convert to FULL-RESOLUTION pixel coordinates (3D projection)
            let pixel = world_to_pixel_3d(final_pos);

            // Check bounds against full resolution
            if (pixel.x >= 0 && pixel.x < i32(tile_params.full_width) &&
                pixel.y >= 0 && pixel.y < i32(tile_params.full_height)) {

                // Calculate which tile and local offset
                let tile_info = pixel_to_tile(u32(pixel.x), u32(pixel.y));
                let tile_idx = tile_info.x;
                let local_idx = u32(tile_info.y);

                // Only write if this pixel belongs to a tile in the current chunk
                if (tile_idx >= 0) {
                    // Determine final color
                    var final_color: vec3<f32>;
                    if (params.color_mode == 0u) {
                        final_color = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(color_index, 0.5), 0.0).rgb;
                    } else {
                        final_color = color;
                    }

                    // Calculate histogram offset for this tile
                    let tile_offset = u32(tile_idx) * tile_buffer_size;
                    let base_idx = tile_offset + local_idx * 4u;

                    let color_scale = params.histogram_color_scale;

                    let r_u32 = u32(clamp(final_color.r, 0.0, 1.0) * color_scale);
                    let g_u32 = u32(clamp(final_color.g, 0.0, 1.0) * color_scale);
                    let b_u32 = u32(clamp(final_color.b, 0.0, 1.0) * color_scale);
                    let density_u32 = u32(color_scale);

                    atomicAdd(&histogram[base_idx + 0u], r_u32);
                    atomicAdd(&histogram[base_idx + 1u], g_u32);
                    atomicAdd(&histogram[base_idx + 2u], b_u32);
                    atomicAdd(&histogram[base_idx + 3u], density_u32);

                    // Update iteration counts
                    let count_offset = u32(tile_idx) * tile_params.tile_size * tile_params.tile_size;
                    atomicAdd(&iteration_counts[count_offset + local_idx], 1u);
                }
            }
        }
    }
}

// Convert path hash to RGB color using golden ratio hue distribution
fn path_hash_to_color(hash: u32) -> vec3<f32> {
    let golden_ratio = 0.618033988749895;
    let hue = fract(f32(hash) * golden_ratio);
    let h = hue * 6.0;
    let i = floor(h);
    let f = h - i;
    let q = 1.0 - f;
    var r: f32; var g: f32; var b: f32;
    let sector = i32(i) % 6;
    if (sector == 0) { r = 1.0; g = f; b = 0.0; }
    else if (sector == 1) { r = q; g = 1.0; b = 0.0; }
    else if (sector == 2) { r = 0.0; g = 1.0; b = f; }
    else if (sector == 3) { r = 0.0; g = q; b = 1.0; }
    else if (sector == 4) { r = f; g = 0.0; b = 1.0; }
    else { r = 1.0; g = 0.0; b = q; }
    return vec3<f32>(r, g, b);
}

// Main compute shader entry point for 2D tiled rendering
// Samples are routed to appropriate tile buffers based on screen coordinates

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let thread_id = global_id.x;

    // Initialize RNG
    var rng = rng_init(thread_id, params.seed);

    // Starting point (random in [-1, 1])
    var current = vec2<f32>(
        rng_nextf(&rng) * 2.0 - 1.0,
        rng_nextf(&rng) * 2.0 - 1.0
    );

    var color = vec3<f32>(1.0, 1.0, 1.0);
    var color_index = 0.0;

    // Path tracking for PathMap mode (using hash for tiled since no path buffer)
    var path_hash = 0u;

    // Xaos tracking: previous transform index for xaos-weighted selection
    var prev_xform_idx = 0u;

    // Pre-calculate tile buffer size (pixels per tile × 4 channels)
    let tile_buffer_size = tile_params.tile_size * tile_params.tile_size * 4u;

    // Iterate
    for (var i = 0u; i < params.iterations_per_thread; i++) {
        let old_pos = current;

        // Select random transform with xaos weighting
        let rand_val = rng_nextf(&rng);
        let xform_idx = select_transform_xaos(rand_val, prev_xform_idx);
        prev_xform_idx = xform_idx;
        let xform = transforms[xform_idx];

        // Opacity check
        if (rng_nextf(&rng) >= xform.opacity) {
            continue;
        }

        // Apophysis 3-step color flow (see main_template.wgsl for details).
        var c_base: f32 = color_index;
        if (params.color_mode == 0u) {
            let symmetry = xform.color_speed;
            c_base = color_index * (1.0 + symmetry) * 0.5 + xform.color * (1.0 - symmetry) * 0.5;
        }
        var vc: f32 = c_base;

        // Apply affine + variations (Step 2)
        let affine_p = apply_affine(xform, current);
        current = apply_variations(xform, xform_idx, affine_p, &rng, &vc);

        // Apply post-affine if enabled for this transform
        if (xform.post_enabled > 0.5) {
            current = apply_post_affine(xform, current);
        }

        // Calculate speed
        let speed = length(current - old_pos);

        // Step 3 / speed-mode / path-map color update
        if (params.color_mode == 0u) {
            color_index = c_base + xform.direct_color * (vc - c_base);
        } else if (params.color_mode == 1u) {
            let speed_color = speed_to_color(speed);
            color = mix(color, speed_color, params.speed_factor);
        } else {
            // Path map mode: use hash for tiled (simpler, no separate buffer)
            path_hash = (path_hash << params.bits_per_transform) | (xform_idx & ((1u << params.bits_per_transform) - 1u));
        }

        // Skip burn-in
        if (i >= params.burn_in) {
            // Apply final transform if present
            var final_pos = current;
            if (params.has_final_transform != 0u) {
                let final_xform = transforms[params.final_transform_index];
                var final_vc: f32 = color_index;
                let affine_p = apply_affine(final_xform, current);
                final_pos = apply_variations(final_xform, params.final_transform_index, affine_p, &rng, &final_vc);
                // Post-affine on final transform
                if (final_xform.post_enabled > 0.5) {
                    final_pos = apply_post_affine(final_xform, final_pos);
                }
            }

            // Convert to FULL-RESOLUTION pixel coordinates
            let pixel = world_to_pixel(final_pos);

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
                    } else if (params.color_mode == 1u) {
                        final_color = color;
                    } else {
                        final_color = path_hash_to_color(path_hash);
                    }

                    // Calculate histogram offset for this tile
                    // Each tile has tile_size × tile_size × 4 words
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

                    // Also update iteration counts (for per-pixel tracking)
                    // Each tile has tile_size × tile_size count entries
                    let count_offset = u32(tile_idx) * tile_params.tile_size * tile_params.tile_size;
                    atomicAdd(&iteration_counts[count_offset + local_idx], 1u);
                }
            }
        }
    }
}

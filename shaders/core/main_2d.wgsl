// Main compute shader entry point for 2D mode
@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let thread_id = global_id.x;

    // Initialize RNG
    var rng = rng_init(thread_id, params.seed);

    // Starting point (random in [-1, 1])
    let initial_x = rng_nextf(&rng) * 2.0 - 1.0;
    let initial_y = rng_nextf(&rng) * 2.0 - 1.0;
    var current = vec2<f32>(initial_x, initial_y);

    var color = vec3<f32>(1.0, 1.0, 1.0);
    var color_index = 0.0;  // For palette mode

    // Path tracking for PathMap mode
    // Stores first 32 iterations losslessly (4 bits per transform, supports up to 16 transforms)
    // path[0] = iterations 0-7, path[1] = 8-15, path[2] = 16-23, path[3] = 24-31
    // Also stores initial_x, initial_y for complete path reconstruction
    var path = array<u32, 4>(0u, 0u, 0u, 0u);
    var path_iteration = 0u;  // Count of iterations stored in path

    // Iterate
    for (var i = 0u; i < params.iterations_per_thread; i++) {
        // Save old position for speed calculation
        let old_pos = current;

        // Select random transform
        let rand_val = rng_nextf(&rng);
        let xform_idx = select_transform(rand_val);
        let xform = transforms[xform_idx];

        // Opacity check (stochastic transparency)
        if (rng_nextf(&rng) >= xform.opacity) {
            continue;  // Skip this iteration (don't plot)
        }

        // Apply affine + variations
        let affine_p = apply_affine(xform, current);
        current = apply_variations(xform, xform_idx, affine_p, &rng);

        // Calculate speed (distance traveled)
        let speed = length(current - old_pos);

        // Update color based on color mode
        if (params.color_mode == 0u) {
            // Palette mode: Apophysis color coordinate evolution
            // Formula: new_c = old_c * (1 + symmetry)/2 + transform_color * (1 - symmetry)/2
            // where symmetry = color_speed (-1 to 1)
            let symmetry = xform.color_speed;
            let colorC1 = (1.0 + symmetry) / 2.0;
            let colorC2 = xform.color * (1.0 - symmetry) / 2.0;
            color_index = color_index * colorC1 + colorC2;
        } else if (params.color_mode == 1u) {
            // Speed mode: blend with speed-based color
            let speed_color = speed_to_color(speed);
            color = mix(color, speed_color, params.speed_factor);
        } else {
            // Path map mode: store iterations losslessly
            // Each u32 holds 8 iterations at 4 bits each
            // For FirstAfterBurnIn mode (1), only track path after burn-in
            let should_track = (params.path_capture_mode != 1u) || (i >= params.burn_in);
            if (should_track) {
                if (params.path_tracking_mode == 0u) {
                    // First mode: store first 32 iterations, then stop
                    if (path_iteration < 32u) {
                        let slot = path_iteration / 8u;  // Which u32 (0-3)
                        let pos = (path_iteration % 8u) * 4u;  // Bit position within u32 (0,4,8,12,16,20,24,28)
                        path[slot] = path[slot] | ((xform_idx & 0xFu) << pos);
                        path_iteration = path_iteration + 1u;
                    }
                } else {
                    // Recent mode: rolling window of 32 most recent iterations
                    // Shift all values left by 4 bits, insert new value at low end of path[0]
                    // path[3] loses its highest 4 bits, gains from path[2]'s highest 4 bits, etc.
                    path[3] = (path[3] << 4u) | (path[2] >> 28u);
                    path[2] = (path[2] << 4u) | (path[1] >> 28u);
                    path[1] = (path[1] << 4u) | (path[0] >> 28u);
                    path[0] = (path[0] << 4u) | (xform_idx & 0xFu);
                    path_iteration = min(path_iteration + 1u, 32u);
                }
            }
        }

        // Skip burn-in iterations
        if (i >= params.burn_in) {
            // Apply final transform if present
            var final_pos = current;
            if (params.has_final_transform != 0u) {
                let final_xform = transforms[params.final_transform_index];
                let affine_p = apply_affine(final_xform, current);
                final_pos = apply_variations(final_xform, params.final_transform_index, affine_p, &rng);
            }

            // Convert to pixel coordinates
            let pixel = world_to_pixel(final_pos);

            // Check bounds
            if (pixel.x >= 0 && pixel.x < i32(params.width) &&
                pixel.y >= 0 && pixel.y < i32(params.height)) {

                let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);

                // Determine final color based on mode
                var final_color: vec3<f32>;
                if (params.color_mode == 0u) {
                    // Palette mode: sample from palette texture using color_index
                    final_color = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(color_index, 0.5), 0.0).rgb;
                } else if (params.color_mode == 1u) {
                    // Speed mode: uses accumulated RGB color
                    final_color = color;
                } else {
                    // Path map mode: store path to buffer
                    // Color will be computed in tonemap pass from path buffer

                    // Capture mode determines when to write:
                    // 0 = FirstHit: only write if no path stored yet
                    // 1 = FirstAfterBurnIn: same as FirstHit (we're already past burn-in here)
                    // 2 = LastHit: always overwrite
                    let should_write = (params.path_capture_mode == 2u) ||
                                       (path_buffer[pixel_idx].iteration_count == 0u);

                    if (should_write) {
                        path_buffer[pixel_idx].path0 = path[0];
                        path_buffer[pixel_idx].path1 = path[1];
                        path_buffer[pixel_idx].path2 = path[2];
                        path_buffer[pixel_idx].path3 = path[3];
                        path_buffer[pixel_idx].iteration_count = path_iteration;
                        path_buffer[pixel_idx].initial_x = initial_x;
                        path_buffer[pixel_idx].initial_y = initial_y;
                    }

                    // Use white for histogram (actual color computed in tonemap from path buffer)
                    final_color = vec3<f32>(1.0, 1.0, 1.0);
                }

                // Atomic accumulation to histogram buffer
                // Write RGB as 4× u32 (unpacked, full 32-bit precision)
                let base_idx = pixel_idx * 4u;  // 4 words per pixel (R, G, B, density)

                // Use global color scale from params (uniform constant, fast access)
                let color_scale = params.histogram_color_scale;

                // Convert colors to u32 using global scale
                // No packing needed - each channel gets its own u32 word
                let r_u32 = u32(clamp(final_color.r, 0.0, 1.0) * color_scale);
                let g_u32 = u32(clamp(final_color.g, 0.0, 1.0) * color_scale);
                let b_u32 = u32(clamp(final_color.b, 0.0, 1.0) * color_scale);
                let density_u32 = u32(color_scale);  // Density includes scale (u32 prevents overflow)

                // Atomic add to histogram (4 separate u32 words)
                atomicAdd(&histogram[base_idx + 0u], r_u32);
                atomicAdd(&histogram[base_idx + 1u], g_u32);
                atomicAdd(&histogram[base_idx + 2u], b_u32);
                atomicAdd(&histogram[base_idx + 3u], density_u32);

                // Increment iteration count for this pixel (for per-pixel convergence tracking)
                atomicAdd(&iteration_counts[pixel_idx], 1u);
            }
        }
    }
}

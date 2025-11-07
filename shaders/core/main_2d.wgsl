// Main compute shader entry point for 2D mode
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
    var color_index = 0.0;  // For palette mode

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
            let xform_color_value = (xform.color.r + xform.color.g + xform.color.b) / 3.0;
            let symmetry = xform.color_speed;
            let colorC1 = (1.0 + symmetry) / 2.0;
            let colorC2 = xform_color_value * (1.0 - symmetry) / 2.0;
            color_index = color_index * colorC1 + colorC2;
        } else {
            // Speed mode: blend with speed-based color
            let speed_color = speed_to_color(speed);
            color = mix(color, speed_color, params.speed_factor);
        }

        // Skip burn-in iterations
        if (i >= params.burn_in) {
            // Convert to pixel coordinates
            let pixel = world_to_pixel(current);

            // Check bounds
            if (pixel.x >= 0 && pixel.x < i32(params.width) &&
                pixel.y >= 0 && pixel.y < i32(params.height)) {

                // Determine final color based on mode
                var final_color: vec3<f32>;
                if (params.color_mode == 0u) {
                    // Palette mode: sample from palette texture using color_index
                    final_color = textureSampleLevel(palette_texture, palette_sampler, color_index, 0.0).rgb;
                } else {
                    // Speed mode: uses accumulated RGB color
                    final_color = color;
                }

                // Atomic accumulation to histogram buffer
                // Write RGB as 4× u32 (unpacked, full 32-bit precision)
                let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);
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

// Main compute shader entry point for 2D mode (simplified - no path tracking)
// This variant is used when PathMap color mode and path filters are both disabled.
// Saves ~5 registers per thread and removes per-iteration branch overhead.
//
// Uses hard-coded constants (compiled at shader build time):
//   NUM_TRANSFORMS, COLOR_MODE, HAS_FINAL_TRANSFORM, FINAL_TRANSFORM_INDEX
// These enable dead code elimination and loop unrolling optimizations.
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

        // Select random transform (uses hard-coded NUM_TRANSFORMS)
        let rand_val = rng_nextf(&rng);
        let xform_idx = select_transform_const(rand_val);
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

        // Update color based on color mode (hard-coded COLOR_MODE enables dead code elimination)
        if (COLOR_MODE == 0u) {
            // Palette mode: Apophysis color coordinate evolution
            let symmetry = xform.color_speed;
            let colorC1 = (1.0 + symmetry) / 2.0;
            let colorC2 = xform.color * (1.0 - symmetry) / 2.0;
            color_index = color_index * colorC1 + colorC2;
        } else if (COLOR_MODE == 1u) {
            // Speed mode: blend with speed-based color
            let speed_color = speed_to_color(speed);
            color = mix(color, speed_color, params.speed_factor);
        }
        // Note: COLOR_MODE == 2 (PathMap) uses the full shader with path tracking

        // Skip burn-in iterations
        if (i >= params.burn_in) {
            // Apply final transform if present (hard-coded HAS_FINAL_TRANSFORM eliminates branch)
            var final_pos = current;
            if (HAS_FINAL_TRANSFORM) {
                let final_xform = transforms[FINAL_TRANSFORM_INDEX];
                let affine_p = apply_affine(final_xform, current);
                final_pos = apply_variations(final_xform, FINAL_TRANSFORM_INDEX, affine_p, &rng);
            }

            // Convert to pixel coordinates
            let pixel = world_to_pixel(final_pos);

            // Check bounds
            if (pixel.x >= 0 && pixel.x < i32(params.width) &&
                pixel.y >= 0 && pixel.y < i32(params.height)) {

                let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);

                // Determine final color based on mode (hard-coded COLOR_MODE)
                var final_color: vec3<f32>;
                if (COLOR_MODE == 0u) {
                    // Palette mode: sample from palette texture using color_index
                    final_color = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(color_index, 0.5), 0.0).rgb;
                } else {
                    // Speed mode: uses accumulated RGB color
                    final_color = color;
                }

                // Atomic accumulation to histogram buffer
                let base_idx = pixel_idx * 4u;
                let color_scale = params.histogram_color_scale;

                let r_u32 = u32(clamp(final_color.r, 0.0, 1.0) * color_scale);
                let g_u32 = u32(clamp(final_color.g, 0.0, 1.0) * color_scale);
                let b_u32 = u32(clamp(final_color.b, 0.0, 1.0) * color_scale);
                let density_u32 = u32(color_scale);

                atomicAdd(&histogram[base_idx + 0u], r_u32);
                atomicAdd(&histogram[base_idx + 1u], g_u32);
                atomicAdd(&histogram[base_idx + 2u], b_u32);
                atomicAdd(&histogram[base_idx + 3u], density_u32);

                // Increment iteration count for this pixel
                atomicAdd(&iteration_counts[pixel_idx], 1u);
            }
        }
    }
}

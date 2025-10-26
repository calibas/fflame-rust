// Main compute shader entry point for 3D mode
@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let thread_id = global_id.x;

    // Initialize RNG
    var rng = rng_init(thread_id, params.seed);

    // Starting point (random in [-1, 1], including Z for 3D)
    var current = vec3<f32>(
        rng_nextf(&rng) * 2.0 - 1.0,
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

        // Apply affine + variations
        let affine_p = apply_affine(xform, current);
        current = apply_variations(xform, xform_idx, affine_p, &rng);

        // Calculate speed (distance traveled)
        let speed = length(current - old_pos);

        // Update color based on color mode
        if (params.color_mode == 0u) {
            // Transform color mode: blend with transform color
            color = mix(color, xform.color, xform.color_speed);
        } else if (params.color_mode == 1u) {
            // Palette mode: blend color index
            let xform_color_value = (xform.color.r + xform.color.g + xform.color.b) / 3.0;
            color_index = mix(color_index, xform_color_value, xform.color_speed);
        } else {
            // Speed mode: blend with speed-based color
            let speed_color = speed_to_color(speed);
            color = mix(color, speed_color, params.speed_factor);
        }

        // Skip burn-in iterations
        if (i >= params.burn_in) {
            // Convert to pixel coordinates (3D version with camera rotation)
            let pixel = world_to_pixel_3d(current);

            // Check bounds
            if (pixel.x >= 0 && pixel.x < i32(params.width) &&
                pixel.y >= 0 && pixel.y < i32(params.height)) {

                // Determine final color based on mode
                var final_color: vec3<f32>;
                if (params.color_mode == 0u) {
                    final_color = color;
                } else if (params.color_mode == 1u) {
                    // Sample from palette texture
                    final_color = textureSampleLevel(palette_texture, palette_sampler, color_index, 0.0).rgb;
                } else {
                    // Speed mode uses accumulated color
                    final_color = color;
                }

                // Atomic accumulation to histogram buffer (F16 PACKED)
                // Pack RGBA as 4× f16 into 2× u32 for HDR support
                let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);
                let base_idx = pixel_idx * 2u;

                // Pack R and G into first u32 (2× f16)
                let packed_rg = pack2x16float(vec2<f32>(final_color.r, final_color.g));

                // Pack B and density into second u32 (2× f16)
                // Density as float allows fractional accumulation
                let packed_bd = pack2x16float(vec2<f32>(final_color.b, 1.0));

                // Two atomic operations (2× reduction from unpacked, HDR-capable!)
                atomicAdd(&histogram[base_idx + 0u], packed_rg);
                atomicAdd(&histogram[base_idx + 1u], packed_bd);
            }
        }
    }
}

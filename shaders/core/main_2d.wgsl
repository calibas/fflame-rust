// Local pixel cache entry for reducing atomic contention
struct LocalPixel {
    pixel_idx: u32,
    r: u32,
    g: u32,
    b: u32,
    density: u32,
}

const CACHE_SIZE: u32 = 16u;
const INVALID_PIXEL_IDX: u32 = 0xFFFFFFFFu;

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

    // Initialize local cache
    var local_cache: array<LocalPixel, CACHE_SIZE>;
    for (var c = 0u; c < CACHE_SIZE; c++) {
        local_cache[c].pixel_idx = INVALID_PIXEL_IDX;
        local_cache[c].r = 0u;
        local_cache[c].g = 0u;
        local_cache[c].b = 0u;
        local_cache[c].density = 0u;
    }

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
            // Convert to pixel coordinates
            let pixel = world_to_pixel(current);

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

                // Local cache accumulation (reduces atomic contention)
                let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);

                // Scale color to integers (0.0-1.0 → 0-10000 for precision)
                let color_scale = 10000.0;
                let r = u32(clamp(final_color.r, 0.0, 1.0) * color_scale);
                let g = u32(clamp(final_color.g, 0.0, 1.0) * color_scale);
                let b = u32(clamp(final_color.b, 0.0, 1.0) * color_scale);

                // Check if pixel is already in cache
                var cache_hit = false;
                for (var c = 0u; c < CACHE_SIZE; c++) {
                    if (local_cache[c].pixel_idx == pixel_idx) {
                        // Cache hit - accumulate locally
                        local_cache[c].r += r;
                        local_cache[c].g += g;
                        local_cache[c].b += b;
                        local_cache[c].density += 1u;
                        cache_hit = true;
                        break;
                    }
                }

                // Cache miss - find empty slot or evict LRU
                if (!cache_hit) {
                    var slot_idx = 0u;
                    var found_empty = false;

                    // Look for empty slot
                    for (var c = 0u; c < CACHE_SIZE; c++) {
                        if (local_cache[c].pixel_idx == INVALID_PIXEL_IDX) {
                            slot_idx = c;
                            found_empty = true;
                            break;
                        }
                    }

                    // If no empty slot, flush slot 0 to global histogram
                    if (!found_empty) {
                        if (local_cache[slot_idx].density > 0u) {
                            let base_idx = local_cache[slot_idx].pixel_idx * 4u;
                            atomicAdd(&histogram[base_idx + 0u], local_cache[slot_idx].r);
                            atomicAdd(&histogram[base_idx + 1u], local_cache[slot_idx].g);
                            atomicAdd(&histogram[base_idx + 2u], local_cache[slot_idx].b);
                            atomicAdd(&histogram[base_idx + 3u], local_cache[slot_idx].density);
                        }
                    }

                    // Insert new entry
                    local_cache[slot_idx].pixel_idx = pixel_idx;
                    local_cache[slot_idx].r = r;
                    local_cache[slot_idx].g = g;
                    local_cache[slot_idx].b = b;
                    local_cache[slot_idx].density = 1u;
                }
            }
        }
    }

    // Flush all remaining cache entries to global histogram
    for (var c = 0u; c < CACHE_SIZE; c++) {
        if (local_cache[c].pixel_idx != INVALID_PIXEL_IDX && local_cache[c].density > 0u) {
            let base_idx = local_cache[c].pixel_idx * 4u;
            atomicAdd(&histogram[base_idx + 0u], local_cache[c].r);
            atomicAdd(&histogram[base_idx + 1u], local_cache[c].g);
            atomicAdd(&histogram[base_idx + 2u], local_cache[c].b);
            atomicAdd(&histogram[base_idx + 3u], local_cache[c].density);
        }
    }
}

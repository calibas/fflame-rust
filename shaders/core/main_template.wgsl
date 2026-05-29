// Main compute shader template
// This template generates variants via conditional compilation:
//   - 2D mode (vec2 points) vs 3D mode (vec3 points)
//   - Simple (no path tracking) vs Full (with path tracking)
//   - Standard vs Xaos (chaos-weighted transform selection)
//
// Conditional markers:
//   {{#if RENDER_3D}} ... {{else}} ... {{/if}}
//   {{#if PATH_TRACKING}} ... {{/if}}
//   {{#if XAOS_ENABLED}} ... {{else}} ... {{/if}}
//
// Uses hard-coded constants (compiled at shader build time):
//   NUM_TRANSFORMS, COLOR_MODE, HAS_POST_AFFINE
// These enable dead code elimination and loop unrolling optimizations.

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let thread_id = global_id.x;

    // Initialize RNG
    var rng = rng_init(thread_id, params.seed);

    // Starting point (random in [-1, 1])
{{#if PATH_TRACKING}}
    // Store initial coordinates for path reconstruction
    let initial_x = rng_nextf(&rng) * 2.0 - 1.0;
    let initial_y = rng_nextf(&rng) * 2.0 - 1.0;
{{#if RENDER_3D}}
    var current = vec3<f32>(
        initial_x,
        initial_y,
        rng_nextf(&rng) * 2.0 - 1.0
    );
{{else}}
    var current = vec2<f32>(initial_x, initial_y);
{{/if}}
{{else}}
{{#if RENDER_3D}}
    var current = vec3<f32>(
        rng_nextf(&rng) * 2.0 - 1.0,
        rng_nextf(&rng) * 2.0 - 1.0,
        rng_nextf(&rng) * 2.0 - 1.0
    );
{{else}}
    var current = vec2<f32>(
        rng_nextf(&rng) * 2.0 - 1.0,
        rng_nextf(&rng) * 2.0 - 1.0
    );
{{/if}}
{{/if}}

    var color = vec3<f32>(1.0, 1.0, 1.0);
    var color_index = 0.0;  // For palette mode

{{#if PATH_TRACKING}}
    // Path tracking for PathMap mode
    // Stores first 32 iterations losslessly (4 bits per transform, supports up to 16 transforms)
    // path[0] = iterations 0-7, path[1] = 8-15, path[2] = 16-23, path[3] = 24-31
    // Also stores initial_x, initial_y for complete path reconstruction
    var path = array<u32, 4>(0u, 0u, 0u, 0u);
    var path_iteration = 0u;  // Count of iterations stored in path
{{/if}}

{{#if XAOS_ENABLED}}
    // Track previous transform for xaos-weighted selection
    var prev_xform_idx = 0u;
{{/if}}

    // Per-thread state initialization for stateful variations that need
    // values beyond zero-fill (var<private> thread_state is already zeroed
    // by WGSL spec; this block runs the wgsl_state_init fragments declared
    // by active variations). Emitted by shader builder; empty for flames
    // with no custom-init variations.
//__STATE_INIT_BLOCK__

    // Iterate
    for (var i = 0u; i < params.iterations_per_thread; i++) {
        // Save old position for speed calculation
        let old_pos = current;

        // Select random transform
        let rand_val = rng_nextf(&rng);
{{#if XAOS_ENABLED}}
        // Xaos: probability modified by transition weights from previous transform
        let xform_idx = select_transform_xaos(rand_val, prev_xform_idx);
        prev_xform_idx = xform_idx;
{{else}}
        // Standard: uses hard-coded NUM_TRANSFORMS for loop unrolling
        let xform_idx = select_transform_const(rand_val);
{{/if}}
        let xform = transforms[xform_idx];

        // Opacity check (stochastic transparency)
        // Note: We still apply the transform even when opacity=0 - opacity affects
        // visibility only, not IFS dynamics. Transform must update position for correct chaos game.
        let should_plot = rng_nextf(&rng) < xform.opacity;

{{#if HAS_DC}}
        // Apophysis 3-step color flow (XForm.pas:312-313, 1067, 1078-1081),
        // emitted only when at least one active variation has writes_color: true:
        //   Step 1: c_base = color_speed-blended palette index
        //   Step 2: normal + linked variations run; DC variations write *vc
        //   Step 3: color_index = c_base + direct_color * (vc - c_base)
        // (Final variations may also write *vc but it's discarded — Final
        //  is a plot-time filter, not part of dynamics.)
        var c_base: f32 = color_index;
        if (COLOR_MODE == 0u) {
            let symmetry = xform.color_speed;
            c_base = color_index * (1.0 + symmetry) * 0.5 + xform.color * (1.0 - symmetry) * 0.5;
        }
        var vc: f32 = c_base;

        // Apply NORMAL transform: affine + variations + post-affine.
        let affine_p = apply_affine(xform, current);
        current = apply_variations(xform, xform_idx, affine_p, &rng, &vc);
{{else}}
        // No DC variations active: original 2-step flow — variations first,
        // then color_speed blend. Bit-identical to pre-direct-color codebase.
        let affine_p = apply_affine(xform, current);
        current = apply_variations(xform, xform_idx, affine_p, &rng);
{{/if}}
        if (HAS_POST_AFFINE) {
            if (xform.post_enabled > 0.5) {
                current = apply_post_affine(xform, current);
            }
        }

{{#if HAS_ATTACHMENTS}}
        // LINKED CHAIN — deterministic dynamics extension.
        // Each Linked transform's output feeds the next iteration; their
        // variations contribute to color flow (DC writes affect *vc).
        // Gated by HAS_ATTACHMENTS — when no Linked or Final exists in
        // the flame, the per-iteration `attachments[xform_idx]` load is
        // stripped from the compiled shader entirely.
        // See docs/projects/per-transform-linked-and-final.md.
        let attach = attachments[xform_idx];
        for (var li = 0u; li < attach.linked_count; li = li + 1u) {
            let lid = attach.linked[li];
            let lxform = transforms[lid];
            let laff = apply_affine(lxform, current);
{{#if HAS_DC}}
            current = apply_variations(lxform, lid, laff, &rng, &vc);
{{else}}
            current = apply_variations(lxform, lid, laff, &rng);
{{/if}}
            if (HAS_POST_AFFINE) {
                if (lxform.post_enabled > 0.5) {
                    current = apply_post_affine(lxform, current);
                }
            }
        }
{{/if}}

        // After Linked chain: current = P_linked (feeds forward as
        // next iteration's input). Speed and color flow use P_linked.
        let speed = length(current - old_pos);

{{#if HAS_DC}}
        // Step 3 (palette mode), or speed-based color (speed mode).
        if (COLOR_MODE == 0u) {
            color_index = c_base + xform.direct_color * (vc - c_base);
        } else if (COLOR_MODE == 1u) {
            let speed_color = speed_to_color(speed);
            color = mix(color, speed_color, params.speed_factor);
        }
{{else}}
        // Original Step 1 (palette mode), or speed-based color (speed mode).
        if (COLOR_MODE == 0u) {
            let symmetry = xform.color_speed;
            let colorC1 = (1.0 + symmetry) / 2.0;
            let colorC2 = xform.color * (1.0 - symmetry) / 2.0;
            color_index = color_index * colorC1 + colorC2;
        } else if (COLOR_MODE == 1u) {
            let speed_color = speed_to_color(speed);
            color = mix(color, speed_color, params.speed_factor);
        }
{{/if}}
{{#if PATH_TRACKING}}
        // Note: COLOR_MODE == 2 (PathMap) handled below with path buffer writes
{{else}}
        // Note: COLOR_MODE == 2 (PathMap) uses the full shader with path tracking
{{/if}}

{{#if PATH_TRACKING}}
        // Path tracking: needed for path map mode OR when filters are active
        let needs_path_tracking = (COLOR_MODE == 2u) || (params.num_path_filters > 0u);
        if (needs_path_tracking) {
            // For FirstAfterBurnIn mode (1), only track path after burn-in
            let should_track = (params.path_capture_mode != 1u) || (i >= params.burn_in);
            if (should_track) {
                if (params.path_tracking_mode == 0u) {
                    // First mode: store first 32 iterations, then stop writing to path array
                    if (path_iteration < 32u) {
                        let slot = path_iteration / 8u;  // Which u32 (0-3)
                        let pos = (path_iteration % 8u) * 4u;  // Bit position within u32 (0,4,8,12,16,20,24,28)
                        path[slot] = path[slot] | ((xform_idx & 0xFu) << pos);
                    }
                } else {
                    // Recent mode: rolling window of 32 most recent iterations
                    // Shift all values left by 4 bits, insert new value at low end of path[0]
                    // path[3] loses its highest 4 bits, gains from path[2]'s highest 4 bits, etc.
                    path[3] = (path[3] << 4u) | (path[2] >> 28u);
                    path[2] = (path[2] << 4u) | (path[1] >> 28u);
                    path[1] = (path[1] << 4u) | (path[0] >> 28u);
                    path[0] = (path[0] << 4u) | (xform_idx & 0xFu);
                }
                // Always increment - this is the actual iteration count (not capped at 32)
                path_iteration = path_iteration + 1u;

                // Check path filters - terminate thread if path matches blocklist
                if (check_path_filters(path, path_iteration)) {
                    break;
                }
            }
        }
{{/if}}

        // Skip burn-in iterations
        if (i >= params.burn_in) {
{{#if HAS_ATTACHMENTS}}
            // FINAL CHAIN — pure plot-time filter. Each Final's variations
            // and affine reshape what gets plotted but DON'T feed forward.
            // DC writes from Final variations are discarded for color_index
            // (color was already locked in after the Linked chain).
            var final_pos = current;
            for (var fi = 0u; fi < attach.final_count; fi = fi + 1u) {
                let fid = attach.final_[fi];
                let fxform = transforms[fid];
                let faff = apply_affine(fxform, final_pos);
{{#if HAS_DC}}
                var final_vc: f32 = color_index;  // discarded after the call
                final_pos = apply_variations(fxform, fid, faff, &rng, &final_vc);
{{else}}
                final_pos = apply_variations(fxform, fid, faff, &rng);
{{/if}}
                if (HAS_POST_AFFINE) {
                    if (fxform.post_enabled > 0.5) {
                        final_pos = apply_post_affine(fxform, final_pos);
                    }
                }
            }
{{else}}
            // No attachments: skip the chain — plot the post-Linked
            // (== post-Normal) point directly.
            let final_pos = current;
{{/if}}

            // Convert to pixel coordinates
{{#if RENDER_3D}}
            var pixel = world_to_pixel_3d(final_pos);

            // Apply depth of field blur (3D mode only)
            if (params.dof_blur_strength > 0.0) {
                // Transform to camera space to get depth along view direction
                let camera_matrix = build_camera_matrix(params.camera_rotation_x, params.camera_rotation_y);
                let camera_space = camera_transform(final_pos, camera_matrix, params.camera_z);
                let depth = camera_space.z;  // Z in camera space = depth from camera

                // Calculate blur amount based on distance from focus plane (in world units)
                let blur_world = (depth - params.dof_focus_distance) * params.dof_blur_strength;

                // Convert world-space blur to pixel-space blur
                // Same scale factor as world_to_pixel_3d: min(width, height) * 0.25 * zoom
                let pixel_scale = f32(min(params.width, params.height)) * 0.25 * params.zoom;
                let blur_pixels = abs(blur_world) * pixel_scale;

                // Generate random offset in disk shape (uniform disk distribution)
                let angle = rng_nextf(&rng) * 6.28318530718;  // 2*PI
                let radius = sqrt(rng_nextf(&rng)) * blur_pixels;
                pixel = pixel + vec2<i32>(i32(cos(angle) * radius), i32(sin(angle) * radius));
            }
{{else}}
            let pixel = world_to_pixel(final_pos);
{{/if}}

            // Check bounds and opacity (only plot if both pass)
            if (pixel.x >= 0 && pixel.x < i32(params.width) &&
                pixel.y >= 0 && pixel.y < i32(params.height) && should_plot) {

                let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);

                // Determine final color based on mode (hard-coded COLOR_MODE).
                // Initialized to white so the PathMap COLOR_MODE branch is
                // always defined: when PATH_TRACKING is true the path-map
                // body below overwrites it, when PATH_TRACKING is false
                // (high-res export) the white default falls through. WGSL
                // requires `var final_color` be initialized on every path
                // even when COLOR_MODE is a compile-time constant.
                var final_color: vec3<f32> = vec3<f32>(1.0, 1.0, 1.0);
                if (COLOR_MODE == 0u) {
                    // Palette mode: sample from palette texture using color_index.
                    // Palette is sRGB-encoded; decode to linear for accumulation.
                    let palette_srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(color_index, 0.5), 0.0).rgb;
                    final_color = srgb_to_linear(palette_srgb);
                } else if (COLOR_MODE == 1u) {
                    // Speed mode: uses accumulated RGB color
                    final_color = color;
{{#if PATH_TRACKING}}
                } else {
                    // Path map mode: store path to buffer
                    // Color will be computed in tonemap pass from path buffer

                    // Capture mode determines when to write:
                    // 0 = FirstHit: only write if no path stored yet
                    // 1 = FirstAfterBurnIn: same as FirstHit (we're already past burn-in here)
                    // 2 = DeepestHit: overwrite only if new path has more iterations
                    let existing_count = path_buffer[pixel_idx].iteration_count;
                    let should_write = (params.path_capture_mode == 2u && path_iteration > existing_count) ||
                                       (params.path_capture_mode != 2u && existing_count == 0u);

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
{{/if}}
                }

{{#if RENDER_3D}}
                // Apply depth fog (3D mode only, blend toward background color)
                if (params.fog_strength > 0.0) {
                    // Get camera-space depth
                    // In camera space, objects in front have negative Z (looking down -Z axis)
                    // Negate to get positive depth where larger = further from camera
                    let camera_matrix = build_camera_matrix(params.camera_rotation_x, params.camera_rotation_y);
                    let camera_space = camera_transform(final_pos, camera_matrix, params.camera_z);
                    let fog_depth = -camera_space.z;  // Negate: distant objects have larger depth

                    // Exponential fog: fog_factor increases with distance beyond fog_start
                    let fog_distance = max(fog_depth - params.fog_start, 0.0);
                    let fog_factor = 1.0 - exp(-params.fog_strength * fog_distance);

                    // Blend toward background color
                    let background = vec3<f32>(params.background_r, params.background_g, params.background_b);
                    final_color = mix(final_color, background, fog_factor);
                }
{{/if}}

{{#if OUTPUT_HISTOGRAM_DIRECT}}
                // Direct-histogram path (sub-4K single-tile render). Atomic
                // accumulation into a single full-resolution histogram buffer.
                // Gated by OUTPUT_HISTOGRAM_DIRECT. See
                // docs/projects/unified-render-pipeline.md.
                //
                // Write RGB as 4× u32 (unpacked, full 32-bit precision).
                let base_idx = pixel_idx * 4u;  // 4 words per pixel (R, G, B, density)

                // Hardcoded color scale (was `params.histogram_color_scale`,
                // formerly a user-tunable slider — removed because the value
                // cancels in the color-recovery math
                // `(scale × Σ color) / (scale × N) = Σ color / N`, and u32
                // overflow is unreachable under any plausible iteration count).
                // Must match the const in `accumulate.wgsl`.
                let color_scale = 100.0;

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
{{else}}
                // Sample-emit path (multi-tile render). Stream one Sample
                // per plotted point to a buffer; a host-driven accumulate
                // pass scatters samples into per-tile histograms. Used when
                // the full-resolution histogram exceeds the storage-buffer
                // binding size limit. See
                // docs/projects/unified-render-pipeline.md.
                //
                // Allocate a slot via atomic counter and write the sample.
                // Same plot-time coords + final_color the direct path uses
                // — keeps feature parity (DOF, fog, opacity, path map, etc.)
                // identical between the two output strategies.
                let sample_idx = atomicAdd(&sample_counter.count, 1u);
                if (sample_idx < arrayLength(&samples)) {
                    samples[sample_idx] = Sample(
                        f32(pixel.x),
                        f32(pixel.y),
                        clamp(final_color.r, 0.0, 1.0),
                        clamp(final_color.g, 0.0, 1.0),
                        clamp(final_color.b, 0.0, 1.0),
                        0.0, 0.0, 0.0
                    );
                }
{{/if}}
            }
        }
    }
}

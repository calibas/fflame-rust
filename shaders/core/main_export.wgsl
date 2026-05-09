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

// Main compute shader entry point for 3D export
// Outputs samples to buffer for CPU-side histogram accumulation

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

    // Path tracking for PathMap mode (using hash for export since no path buffer)
    var path_hash = 0u;

    // Xaos tracking: previous transform index for xaos-weighted selection
    var prev_xform_idx = 0u;

    // Per-thread state initialization (see main_template.wgsl).
//__STATE_INIT_BLOCK__

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

{{#if HAS_DC}}
        // Apophysis 3-step color flow (see main_template.wgsl for details).
        var c_base: f32 = color_index;
        if (params.color_mode == 0u) {
            let symmetry = xform.color_speed;
            c_base = color_index * (1.0 + symmetry) * 0.5 + xform.color * (1.0 - symmetry) * 0.5;
        }
        var vc: f32 = c_base;

        // NORMAL transform: affine + variations + post-affine
        let affine_p = apply_affine(xform, current);
        current = apply_variations(xform, xform_idx, affine_p, &rng, &vc);
{{else}}
        // No DC variations: original 2-step flow.
        let affine_p = apply_affine(xform, current);
        current = apply_variations(xform, xform_idx, affine_p, &rng);
{{/if}}
        if (xform.post_enabled > 0.5) {
            current = apply_post_affine(xform, current);
        }

{{#if HAS_ATTACHMENTS}}
        // LINKED CHAIN — see main_template.wgsl for full doc.
        // Gated by HAS_ATTACHMENTS — when no Linked or Final exists in
        // the flame, the per-iteration `attachments[xform_idx]` load is
        // stripped from the compiled shader entirely.
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
            if (lxform.post_enabled > 0.5) {
                current = apply_post_affine(lxform, current);
            }
        }
{{/if}}

        // Speed uses post-Linked position (= P_linked).
        let speed = length(current - old_pos);

{{#if HAS_DC}}
        // Step 3 / speed-mode / path-map color update
        if (params.color_mode == 0u) {
            color_index = c_base + xform.direct_color * (vc - c_base);
        } else if (params.color_mode == 1u) {
            let speed_color = speed_to_color(speed);
            color = mix(color, speed_color, params.speed_factor);
        } else {
            path_hash = (path_hash << params.bits_per_transform) | (xform_idx & ((1u << params.bits_per_transform) - 1u));
        }
{{else}}
        // Original Step 1: color_speed blend (palette mode), or speed/path-map.
        if (params.color_mode == 0u) {
            let symmetry = xform.color_speed;
            let colorC1 = (1.0 + symmetry) / 2.0;
            let colorC2 = xform.color * (1.0 - symmetry) / 2.0;
            color_index = color_index * colorC1 + colorC2;
        } else if (params.color_mode == 1u) {
            let speed_color = speed_to_color(speed);
            color = mix(color, speed_color, params.speed_factor);
        } else {
            path_hash = (path_hash << params.bits_per_transform) | (xform_idx & ((1u << params.bits_per_transform) - 1u));
        }
{{/if}}

        // Skip burn-in
        if (i >= params.burn_in) {
{{#if HAS_ATTACHMENTS}}
            // FINAL CHAIN — plot-time filter; output not fed forward.
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
                if (fxform.post_enabled > 0.5) {
                    final_pos = apply_post_affine(fxform, final_pos);
                }
            }
{{else}}
            // No attachments: skip the chain — plot the post-Linked
            // (== post-Normal) point directly.
            let final_pos = current;
{{/if}}

            // Convert to pixel coordinates (3D projection)
            let pixel = world_to_pixel_3d(final_pos);

            // Check bounds
            if (pixel.x >= 0 && pixel.x < i32(params.width) &&
                pixel.y >= 0 && pixel.y < i32(params.height)) {

                // Determine final color
                var final_color: vec3<f32>;
                if (params.color_mode == 0u) {
                    final_color = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(color_index, 0.5), 0.0).rgb;
                } else if (params.color_mode == 1u) {
                    final_color = color;
                } else {
                    final_color = path_hash_to_color(path_hash);
                }

                // Allocate sample slot using atomic counter
                let sample_idx = atomicAdd(&sample_counter.count, 1u);

                // Write sample to buffer
                samples[sample_idx] = Sample(
                    f32(pixel.x),
                    f32(pixel.y),
                    clamp(final_color.r, 0.0, 1.0),
                    clamp(final_color.g, 0.0, 1.0),
                    clamp(final_color.b, 0.0, 1.0),
                    0.0, 0.0, 0.0  // padding
                );
            }
        }
    }
}

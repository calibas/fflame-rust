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

    // Fuse (burn-in) countdown: plot only when 0. Starts at the
    // configured burn-in and is RESET by the bad-value respawn below
    // (JWF re-fuses re-randomized points the same way), so a respawned
    // point re-converges onto the attractor before contributing.
    var fuse = params.burn_in;

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
{{/if}}
{{#if HAS_RGB}}
        // Direct-RGB register, sentinel-init to black. A variation with
        // `Feature::WritesRgb` opts in and is expected to overwrite *vrc;
        // if it doesn't (or no Final variation reaches the plot site
        // with vrc set), the unmodified black is what gets blended via
        // direct_color at plot time. See the `Feature::WritesRgb` doc
        // for the contract.
        var vrc: vec3<f32> = vec3<f32>(0.0);
{{/if}}

        // Apply NORMAL transform: affine + variations + post-affine.
        // The 4-way call shape (HAS_DC × HAS_RGB) keeps each combination
        // as its own variant rather than an inline conditional, because
        // the template processor only does whole-block substitution.
        let affine_p = apply_affine(xform, current);
{{#if HAS_DC}}
{{#if HAS_RGB}}
        current = apply_variations(xform, xform_idx, affine_p, &rng, &vc, &vrc);
{{else}}
        current = apply_variations(xform, xform_idx, affine_p, &rng, &vc);
{{/if}}
{{else}}
{{#if HAS_RGB}}
        current = apply_variations(xform, xform_idx, affine_p, &rng, &vrc);
{{else}}
        current = apply_variations(xform, xform_idx, affine_p, &rng);
{{/if}}
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
{{#if HAS_RGB}}
            current = apply_variations(lxform, lid, laff, &rng, &vc, &vrc);
{{else}}
            current = apply_variations(lxform, lid, laff, &rng, &vc);
{{/if}}
{{else}}
{{#if HAS_RGB}}
            current = apply_variations(lxform, lid, laff, &rng, &vrc);
{{else}}
            current = apply_variations(lxform, lid, laff, &rng);
{{/if}}
{{/if}}
            if (HAS_POST_AFFINE) {
                if (lxform.post_enabled > 0.5) {
                    current = apply_post_affine(lxform, current);
                }
            }
        }
{{/if}}

        // flam3/JWF-style bad-value recovery, in two tiers:
        //
        // X/Y divergence → full respawn + re-fuse, like JWF's
        // validateState() → preFuseIter() (x, y ∈ [-1, 1], z = 0).
        // The magnitude check (rather than NaN compares, which WGSL
        // compilers may assume away) catches growth before it reaches
        // f32 infinity and poisons everything downstream.
{{#if RENDER_3D}}
        //
        // Z divergence → SATURATE at ±1e32 instead of respawning.
        // Z compounds legitimately through always-z variations under
        // preserve_z=true (JWF-rando25: edisc at weight 15 multiplies
        // z by 15 per pass — f32 would hit the threshold every ~35
        // iterations, but JWF's f64 cruises ~260 iterations before
        // its Inf-triggered respawn, so respawning that often starves
        // the attractor visibly). Saturation keeps the X/Y dynamics
        // identical to JWF's long-lived points, and every consumer
        // treats a saturated z exactly like JWF treats Inf: flat
        // views never read it (zero camera-matrix coefficient times
        // a FINITE huge value is 0, not the NaN that Inf produced),
        // and pitched views reject the sample at the bounds check.
        // Non-finite z (NaN from 0·∞ inside a variation) falls to 0,
        // approximating JWF's eventual respawn for those points.
        let z_sat = clamp(current.z, -1e32, 1e32);
        current.z = select(0.0, z_sat, abs(z_sat) <= 1e32);
        // JWF's f64 does eventually reach actual Inf and respawn the
        // point — for explosive growth that's roughly a couple
        // hundred iterations into the divergence. A point parked on
        // our saturation rail would otherwise live (and stay
        // invisible in pitched views) forever; emulate JWF's
        // amortized respawn cycle with a 1/256 per-iteration chance
        // while saturated.
        var z_railed_respawn = false;
        if (abs(current.z) >= 1e32) {
            z_railed_respawn = rng_nextf(&rng) < 0.00390625;
        }
{{else}}
        let z_railed_respawn = false;
{{/if}}
        let bad_value = z_railed_respawn ||
                        !(abs(current.x) <= 1e32) ||
                        !(abs(current.y) <= 1e32);
        if (bad_value) {
{{#if RENDER_3D}}
            current = vec3<f32>(
                rng_nextf(&rng) * 2.0 - 1.0,
                rng_nextf(&rng) * 2.0 - 1.0,
                0.0
            );
{{else}}
            current = vec2<f32>(
                rng_nextf(&rng) * 2.0 - 1.0,
                rng_nextf(&rng) * 2.0 - 1.0
            );
{{/if}}
            fuse = params.burn_in;
            continue;
        }

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
            // (fuse == 0 — also re-armed by the bad-value respawn)
            let should_track = (params.path_capture_mode != 1u) || (fuse == 0u);
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

        // Skip burn-in / re-fuse iterations (the respawn above resets
        // the countdown so recovering points don't plot mid-flight)
        if (fuse > 0u) {
            fuse = fuse - 1u;
        } else {
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
{{/if}}
{{#if HAS_RGB}}
                var final_vrc: vec3<f32> = vec3<f32>(0.0);  // discarded after the call
{{/if}}
{{#if HAS_DC}}
{{#if HAS_RGB}}
                final_pos = apply_variations(fxform, fid, faff, &rng, &final_vc, &final_vrc);
{{else}}
                final_pos = apply_variations(fxform, fid, faff, &rng, &final_vc);
{{/if}}
{{else}}
{{#if HAS_RGB}}
                final_pos = apply_variations(fxform, fid, faff, &rng, &final_vrc);
{{else}}
                final_pos = apply_variations(fxform, fid, faff, &rng);
{{/if}}
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

            // Post-symmetry — gated entirely by HAS_POST_SYMMETRY.
            // When false, sym_count is 1u and the loop runs exactly
            // once with sym_k=0 (passes final_pos through unchanged),
            // letting the compiler fully strip it. When true,
            // sym_count is 1 (None at runtime — shouldn't happen given
            // the compile-time gate, but defensive), 2 (axis modes),
            // or `order` (Point mode), and each iteration plots one
            // mirrored/rotated copy of the same sample.
{{#if HAS_POST_SYMMETRY}}
            let sym_count = select(
                select(2u, max(params.post_symmetry.order, 1u), params.post_symmetry.kind == 3u),
                1u,
                params.post_symmetry.kind == 0u
            );
{{else}}
            let sym_count = 1u;
{{/if}}

            // Pre-compute the iteration's base color OUTSIDE the
            // symmetry loop. It depends only on color_index / color /
            // (none for path-map) — none of which change between the
            // K symmetric copies. Hoisting the palette texture sample
            // alone gives a (K-1)/K speedup for palette mode at high
            // Point-symmetry orders. Default of white covers the
            // path-map COLOR_MODE branch (and any unhandled mode);
            // fog inside the loop reads from this base into a local
            // copy so its per-copy depth modulation doesn't bleed.
            var base_final_color: vec3<f32> = vec3<f32>(1.0, 1.0, 1.0);
            if (COLOR_MODE == 0u) {
                let palette_srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(color_index, 0.5), 0.0).rgb;
                base_final_color = srgb_to_linear(palette_srgb);
            } else if (COLOR_MODE == 1u) {
                base_final_color = color;
            }
{{#if HAS_RGB}}
            // Direct-RGB-writing variations override (or blend into) the
            // palette/speed-derived color. Same direct_color slider gates
            // it as the palette-index DC path: 0 keeps the existing color,
            // 1 fully replaces with vrc, in-between mixes. Path-map mode
            // (COLOR_MODE == 2) keeps the default white init regardless.
            base_final_color = mix(base_final_color, vrc, xform.direct_color);
{{/if}}

            for (var sym_k: u32 = 0u; sym_k < sym_count; sym_k = sym_k + 1u) {
{{#if HAS_POST_SYMMETRY}}
{{#if RENDER_3D}}
                let plot_pos = post_symmetry_copy(final_pos, sym_k);
{{else}}
                // 2D path: lift to vec3 with Z=0 so the symmetry helper
                // (vec3 only) accepts it, then strip Z back off for
                // world_to_pixel.
                let plot_pos = post_symmetry_copy(vec3<f32>(final_pos, 0.0), sym_k).xy;
{{/if}}
{{else}}
                let plot_pos = final_pos;
{{/if}}

            // Per-sample histogram weight (1.0 = neutral). Only the
            // 3D depth-density compensation below changes it; the 2D
            // path always plots at weight 1.
            var density_weight = 1.0;

            // Convert to pixel coordinates
{{#if RENDER_3D}}
            var pixel = world_to_pixel_3d(plot_pos);

            // Depth-density compensation (radiance-preserving splats).
            // Perspective magnifies a structure at camera depth z by
            // 1/zr per axis (zr = 1 − persp·z), spreading its samples
            // over 1/zr² more pixels — so apparent density scales by
            // zr², and structures fade as the camera approaches. Weight
            // each sample by zr^(−2s) to cancel it: near samples gain,
            // far samples (compressed, artificially dense) lose, and
            // the focal plane (zr = 1) is invariant at any strength.
            // Clamped to [1/64, 64] to bound noise amplification near
            // the perspective singularity and to keep far samples from
            // truncating to zero in the u32 histogram.
            if (params.depth_density_compensation > 0.0 && abs(params.perspective_strength) > 1e-6) {
                let camera_matrix = build_camera_matrix(
                    // Same slot mapping as project_3d_to_2d_apophysis;
                    // only z matters here and z is roll-invariant.
                    -params.rotation,
                    -params.camera_rotation_x,
                    -params.camera_bank,
                     params.camera_rotation_y,
                );
                let camera_space = camera_transform(plot_pos, camera_matrix, vec3<f32>(params.camera_x, params.camera_y, params.camera_z));
                let zr = 1.0 - params.perspective_strength * camera_space.z;
                // Behind-camera / near-singularity samples keep weight
                // 1 — they're clipped by apply_perspective anyway.
                if (zr > 1e-3) {
                    density_weight = clamp(pow(zr, -2.0 * params.depth_density_compensation), 0.015625, 64.0);
                }
            }

            // Apply depth of field blur (3D mode only)
            if (params.dof_blur_strength > 0.0) {
                // Transform to camera space to get depth along view direction.
                // 4-angle matrix per JWildfire's createProjectionMatrix —
                // see utilities.wgsl `build_camera_matrix`. The negated
                // yaw mirrors JWildfire's caller-side `-getCamYaw()`.
                let camera_matrix = build_camera_matrix(
                    // Same yaw↔roll slot swap + sign convention as
                    // project_3d_to_2d_apophysis. See its call site
                    // for the empirical-tuning derivation.
                    -params.rotation,            // matrix yaw  ← our roll
                    -params.camera_rotation_x,   // pitch
                    -params.camera_bank,        // bank
                     params.camera_rotation_y,   // matrix roll ← our yaw
                );
                let camera_space = camera_transform(plot_pos, camera_matrix, vec3<f32>(params.camera_x, params.camera_y, params.camera_z));
                let depth = camera_space.z;  // Z in camera space = depth from camera

                // Calculate blur amount based on distance from focus plane (in world units).
                // Multiplied by 0.1 so `dof_blur_strength` carries the same magnitude
                // as Apophysis's `cam_dof` attribute — user-facing values copy across
                // directly (Apo 0.19 → ours 0.19), and the slider stops feeling 10×
                // too touchy.
                let blur_world = (depth - params.dof_focus_distance) * params.dof_blur_strength * 0.1;

                // Convert world-space blur to pixel-space blur
                // Same scale factor as world_to_pixel_3d: min(width, height) * 0.25 * zoom
                let pixel_scale = f32(min(params.width, params.height)) * 0.25 * params.zoom;
                let blur_pixels = abs(blur_world) * pixel_scale;

                // Generate random offset in disk shape (uniform disk distribution).
                // round() before the i32() cast — `i32(x)` truncates toward
                // zero, which makes the [-1, 1) bin around the center pixel
                // 2 units wide vs 1 unit for every other pixel. The wider
                // central bin sucks up the diagonal samples (whose
                // components stay in (-1, 1) at moderate radii) while
                // cardinal samples escape first, producing a "+" pattern
                // with a hot center spot. round() makes all pixel bins
                // 1 unit wide and symmetric around the center.
                let angle = rng_nextf(&rng) * 6.28318530718;  // 2*PI
                let radius = sqrt(rng_nextf(&rng)) * blur_pixels;
                pixel = pixel + vec2<i32>(
                    i32(round(cos(angle) * radius)),
                    i32(round(sin(angle) * radius))
                );
            }
{{else}}
            let pixel = world_to_pixel(plot_pos);
{{/if}}

            // Check bounds and opacity (only plot if both pass)
            if (pixel.x >= 0 && pixel.x < i32(params.width) &&
                pixel.y >= 0 && pixel.y < i32(params.height) && should_plot) {

                let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);

                // Per-copy local copy of the hoisted base color. We
                // need a `var` so fog (below) can modulate it without
                // affecting the next symmetric copy's plot.
                var final_color: vec3<f32> = base_final_color;

{{#if PATH_TRACKING}}
                // Path-tracking write — depends on pixel_idx so it
                // must stay inside the symmetry loop (each symmetric
                // copy records the same path at its own pixel).
                if (COLOR_MODE == 2u) {
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
                }
{{/if}}

{{#if RENDER_3D}}
                // Far density fade (3D mode only): genuinely thin out
                // far samples by scaling their histogram DENSITY
                // weight, unlike fog which recolors at full density.
                // Gaussian falloff borrowed from JWildfire's
                // diminish-Z curve: weight = exp(-zdist² · strength)
                // for zdist = fade_start − camera_z > 0 (camera-space
                // z is more negative the farther the sample). Far
                // structures fade to nothing instead of tinting.
                // Composes multiplicatively with the depth-density
                // compensation weight above.
                if (params.far_density_fade > 0.0) {
                    let camera_matrix = build_camera_matrix(
                        // Same slot mapping as project_3d_to_2d_apophysis;
                        // only z matters here and z is roll-invariant.
                        -params.rotation,
                        -params.camera_rotation_x,
                        -params.camera_bank,
                         params.camera_rotation_y,
                    );
                    let camera_space = camera_transform(plot_pos, camera_matrix, vec3<f32>(params.camera_x, params.camera_y, params.camera_z));
                    let zdist = params.far_density_fade_start - camera_space.z;
                    if (zdist > 0.0) {
                        density_weight *= exp(-zdist * zdist * params.far_density_fade);
                    }
                }

                // Apply depth fog (3D mode only, blend toward background color)
                if (params.fog_strength > 0.0) {
                    // Get camera-space depth — same 4-angle matrix as DoF above.
                    // In camera space, objects in front have negative Z (looking
                    // down -Z axis); negate to get positive depth.
                    let camera_matrix = build_camera_matrix(
                        // Same yaw↔roll slot swap + sign convention
                        // as utilities.wgsl `project_3d_to_2d_apophysis`.
                        -params.rotation,           // matrix yaw  ← our roll
                        -params.camera_rotation_x,  // pitch
                        -params.camera_bank,        // bank
                         params.camera_rotation_y,  // matrix roll ← our yaw
                    );
                    let camera_space = camera_transform(plot_pos, camera_matrix, vec3<f32>(params.camera_x, params.camera_y, params.camera_z));
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

                // Convert colors to u32 using global scale. All four
                // channels carry the same density_weight so the color
                // recovery ratio Σcolor/Σdensity is weight-invariant.
                let weighted_scale = color_scale * density_weight;
                let r_u32 = u32(clamp(final_color.r, 0.0, 1.0) * weighted_scale);
                let g_u32 = u32(clamp(final_color.g, 0.0, 1.0) * weighted_scale);
                let b_u32 = u32(clamp(final_color.b, 0.0, 1.0) * weighted_scale);
                let density_u32 = u32(weighted_scale);  // Density includes scale (u32 prevents overflow)

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
                        density_weight, 0.0, 0.0
                    );
                }
{{/if}}
            }
            }  // end for (sym_k = 0..sym_count) — post-symmetry loop
        }

{{#if FLATTEN_Z_PER_ITER}}
        // JWildfire `preserve_z=false` (default): flatten Z at end
        // of each iteration so it doesn't accumulate across the
        // chaos game. Without this, variations that scale Z by an
        // amount > 1 (e.g. `spherical` at high weight) drive Z to
        // ±∞ within a handful of iterations; the camera transform's
        // `0·∞ = NaN` then poisons pixel coords and most samples
        // get rejected at the bounds check — visible as a dim or
        // black image. Cost is one f32 store per iteration in 3D;
        // gate strips it from 2D shaders and from 3D shaders with
        // preserve_z=true.
        current.z = 0.0;
{{/if}}
    }
}

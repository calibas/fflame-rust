//! GPU buffer update logic
//!
//! Extracted from mod.rs to reduce file size and improve maintainability.

use super::App;

impl App {
    /// Process pending config actions and update GPU buffers accordingly.
    ///
    /// This handles:
    /// - Flame updates (transforms, variations)
    /// - View parameter updates (zoom, pan, rotation, camera)
    /// - Palette and color mode updates
    /// - Tone curve updates
    /// - Shader rebuilds (when variation set changes)
    /// - Accumulation resets
    ///
    /// Returns whether overwrite mode should be used for the current frame.
    pub(super) fn process_gpu_updates(&mut self, view_changed_by_keyboard: bool) -> bool {

        // Get pending actions from ConfigManager (includes animation's changes now)
        let actions = self.config_manager.get_pending_actions();

        // Determine if any GPU updates are needed
        let needs_update = actions.reset_accumulation
            || actions.update_flame
            || actions.update_palette
            || actions.update_tone_curve
            || actions.update_view
            || actions.rebuild_shader
            || view_changed_by_keyboard;

        if needs_update {
            if let Some(ref mut renderer) = self.flame_renderer {
                // Get current config for updates
                let update_config = self.config_manager.active_config();

                let mut update_encoder =
                    self.gpu
                        .device
                        .create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                            label: Some("Update Encoder"),
                        });

                // Update flame if UpdateAction indicates (includes preview mode live updates)
                if actions.update_flame {
                    // Sync App's working flame copy from ConfigManager before
                    // we hand it to the renderer. Without this, swapping the
                    // editing_target (Phase 6) wouldn't take effect: the
                    // renderer would keep rendering the previously-synced
                    // flame even though ConfigManager points elsewhere.
                    self.flame = update_config.flame.clone();
                    renderer.update_flame(
                        &self.gpu.device,
                        &self.gpu.queue,
                        &self.flame,
                        self.config_manager.system_settings().iterations_per_thread,
                        self.config_manager.system_settings().burn_in,
                        update_config.zoom,
                        update_config.pan_x,
                        update_config.pan_y,
                        update_config.rotation,
                        update_config.camera_rotation_x,
                        update_config.camera_rotation_y,
                        update_config.camera_z,
                        update_config.speed_factor,
                        update_config.dof_focus_distance,
                        update_config.dof_blur_strength,
                        update_config.fog_strength,
                        update_config.fog_start,
                        update_config.background_color,
                    );
                }

                // Update view parameters (includes view changes and iteration changes)
                if actions.update_view || view_changed_by_keyboard {
                    renderer.set_deterministic_rng(update_config.deterministic_rng);
                    renderer.update_iterations(
                        &self.gpu.queue,
                        self.config_manager.system_settings().iterations_per_thread,
                        self.config_manager.system_settings().burn_in,
                        update_config.zoom,
                        update_config.pan_x,
                        update_config.pan_y,
                        update_config.rotation,
                        update_config.camera_rotation_x,
                        update_config.camera_rotation_y,
                        update_config.camera_z,
                        update_config.speed_factor,
                    );
                }

                // Update palette if needed (also handles color mode changes)
                if actions.update_palette {
                    // Check if palette texture size needs to change
                    if renderer.palette_size() != update_config.palette_size {
                        renderer.set_palette_size(
                            &self.gpu.device,
                            &self.gpu.queue,
                            &update_config.flame,
                            update_config.palette_size,
                        );
                    }

                    // Get palette directly from ConfigManager
                    renderer.update_palette(
                        &self.gpu.device,
                        &self.gpu.queue,
                        &update_config.palette,
                        update_config.palette_rotation,
                        update_config.palette_squeeze,
                        update_config.palette_squeeze_mode,
                        update_config.palette_squeeze_falloff,
                        update_config.palette_log_strength,
                        update_config.palette_reverse,
                    );

                    // Update color mode in GPU params (ColorMode changes trigger update_palette)
                    renderer.set_color_mode(
                        &self.gpu.queue,
                        update_config.color_mode,
                        self.config_manager.system_settings().iterations_per_thread,
                        self.config_manager.system_settings().burn_in,
                        update_config.zoom,
                        update_config.pan_x,
                        update_config.pan_y,
                        update_config.rotation,
                        update_config.camera_rotation_x,
                        update_config.camera_rotation_y,
                        update_config.camera_z,
                        update_config.speed_factor,
                    );

                    // Update path buffer allocation and shaders based on color_mode (PathMap needs buffers)
                    renderer.update_path_features(
                        &self.gpu.device,
                        &self.gpu.queue,
                        &update_config.flame,
                    );
                }

                // Update tone curve LUT if changed
                if actions.update_tone_curve {
                    renderer.update_curve_lut(&self.gpu.queue, &update_config.tonemap_curve);
                }

                // Rebuild shader if variation set changed
                if actions.rebuild_shader {
                    // TODO: Implement shader rebuild logic when variation system supports it
                    // For now, this would require recreating the compute pipeline
                }

                // Handle accumulation reset based on change type
                let should_full_reset = actions.reset_accumulation || view_changed_by_keyboard;
                let has_view_or_color_change = actions.update_view || actions.update_palette;

                if should_full_reset {
                    // Structural changes: Clear buffer and reset counters (blank frame expected)
                    renderer.reset(
                        &mut update_encoder,
                        &self.gpu.queue,
                        self.config_manager.system_settings().iterations_per_thread,
                        update_config.zoom,
                        update_config.pan_x,
                        update_config.pan_y,
                        update_config.rotation,
                        update_config.camera_rotation_x,
                        update_config.camera_rotation_y,
                        update_config.camera_z,
                        update_config.speed_factor,
                    );
                    self.frames_since_accumulation = 0;
                    self.rendering_complete = false; // Reset completion flag
                    self.clear_paths_next_frame = true; // Clear path buffer on full reset
                } else if has_view_or_color_change
                    && renderer.total_iterations() >= update_config.max_iterations
                {
                    // View/color changes when fractal has stopped iterating:
                    // Reset counter to restart iteration (smooth transition via overwrite mode)
                    renderer.reset_iteration_counter();
                    self.frames_since_accumulation = 0;
                    self.rendering_complete = false; // Reset completion flag
                    self.clear_paths_next_frame = true; // Clear path buffer when restarting
                }

                self.gpu.queue.submit(std::iter::once(update_encoder.finish()));
            }
        }

        // Update overwrite mode based on changes
        self.update_overwrite_mode(&actions, view_changed_by_keyboard);

        // Clear pending actions after executing them
        self.config_manager.clear_pending_actions();

        // Return current overwrite mode state
        self.use_overwrite_next_frame
    }

    /// Update the overwrite mode flag based on recent changes.
    ///
    /// Overwrite mode keeps accumulation in "replace" mode for smooth transitions
    /// during parameter dragging. It stays ON for 100ms after the last change,
    /// then resets iteration counter for a clean rebuild.
    ///
    /// Also updates the RenderModeFSM to track Overwrite state for brightness boost.
    fn update_overwrite_mode(
        &mut self,
        actions: &crate::config::UpdateAction,
        view_changed_by_keyboard: bool,
    ) {
        use crate::animation::PlaybackState;
        use web_time::Instant;

        // Note: Excludes tone_curve (post-processing only, doesn't affect accumulation buffer)
        let had_changes = actions.update_view || actions.update_palette || actions.update_flame;
        let now = Instant::now();

        // Track previous overwrite state to detect transitions
        let was_overwrite = self.use_overwrite_next_frame;

        // Check if animation is currently playing
        let is_animation_playing = self.animation_controller.state == PlaybackState::Playing;

        if had_changes && !actions.reset_accumulation {
            // Changes happened → enable overwrite mode and update timestamp
            self.use_overwrite_next_frame = true;
            self.last_param_change_time = Some(now);

            // Update FSM to Overwrite state (for brightness boost tracking)
            // Only if not animating (animation takes priority)
            if !is_animation_playing {
                self.render_mode.enter_overwrite(self.config_manager.active_config());
            }
        } else if !had_changes {
            // No changes this frame → check if we're still within the smooth transition window
            if let Some(last_change) = self.last_param_change_time {
                let time_since_change = now.duration_since(last_change);
                // Keep overwrite ON for 100ms after last change (~6 frames at 60fps)
                self.use_overwrite_next_frame = time_since_change.as_millis() < 100;

                // When overwrite window expires, reset iteration counter for clean rebuild
                // BUT skip this during animation playback - we want continuous accumulation
                if was_overwrite && !self.use_overwrite_next_frame && !is_animation_playing {
                    // Exit overwrite mode in FSM
                    self.render_mode.exit_overwrite();

                    if let Some(ref mut renderer) = self.flame_renderer {
                        // Cumulative-mean mode keeps `samples_in_buffer`
                        // aligned with the (un-cleared) accumulator
                        // texture; fixed-EMA mode zeros both together
                        // since the next step clears the buffer too.
                        // Misaligning these in cumulative mode produces
                        // a one-frame bright flash at overwrite-exit:
                        // sample_density goes to ~0 while accumulator
                        // density values remain non-zero, the shader's
                        // `density / sample_density` ratio explodes,
                        // apply_levels saturates to 1, Levels briefly
                        // disables for that frame.
                        if renderer.use_dynamic_blend() {
                            renderer.reset_iteration_counter_keep_buffer();
                        } else {
                            renderer.reset_iteration_counter();
                        }

                        // In fixed-EMA mode, also clear the
                        // accumulation textures. Without this, the
                        // last drag-frame's data sits in the buffer
                        // and dominates the EMA's post-drag
                        // bootstrap — the user reported it looking
                        // "way too bright" until ~1/blend_factor
                        // frames had averaged it out. Clearing both
                        // ping-pong halves (clear_accumulation_buffers
                        // wraps clear_all) makes the next frame read
                        // zero from previous_accumulation regardless
                        // of which texture is current. Cumulative
                        // mode skips this — there the leftover is one
                        // batch's worth of valid samples that dilutes
                        // naturally.
                        if !renderer.use_dynamic_blend() {
                            let mut clear_encoder = self.gpu.device.create_command_encoder(
                                &egui_wgpu::wgpu::CommandEncoderDescriptor {
                                    label: Some("Overwrite-exit accumulation clear"),
                                },
                            );
                            renderer.clear_accumulation_buffers(&mut clear_encoder, &self.gpu.queue);
                            self.gpu.queue.submit(std::iter::once(clear_encoder.finish()));
                        }

                        self.rendering_complete = false; // Reset completion flag
                        self.clear_paths_next_frame = true; // Clear path buffer for clean rebuild
                        log::debug!(
                            "Overwrite window expired → reset iteration counter (and accumulation in fixed-EMA mode)"
                        );
                    }
                }
            } else {
                self.use_overwrite_next_frame = false;
            }
        }
        // If reset_accumulation=true, disable overwrite (let normal accumulation work after reset)

        // Suppress unused warning (keyboard changes are handled in process_gpu_updates)
        let _ = view_changed_by_keyboard;
    }

    /// Determine if overwrite mode should be used for the current frame.
    ///
    /// Overwrite mode is used when:
    /// - Active parameter changes are happening (drag, scroll)
    /// - Animation is playing (keeps accumulation stable)
    /// - Fractal has reached max_iterations (allows live updates)
    pub(super) fn should_use_overwrite(&self) -> bool {
        use crate::animation::PlaybackState;

        let has_stopped = self
            .flame_renderer
            .as_ref()
            .map(|r| r.total_iterations() >= self.config_manager.active_config().max_iterations)
            .unwrap_or(false);

        let is_animation_playing = self.animation_controller.state == PlaybackState::Playing;

        self.use_overwrite_next_frame || is_animation_playing || has_stopped
    }
}

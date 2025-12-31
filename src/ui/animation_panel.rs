//! Animation playback panel UI
//!
//! Provides controls for playing, pausing, and scrubbing through animations,
//! as well as loading and saving animation files.

use egui::Ui;
use rust_i18n::t;
use crate::animation::{Animation, AnimationController, AnimationQualityMode, LoopMode, PlaybackState};
use crate::animation::export::{VideoCodec, HardwareAccel};

/// Export progress state for UI display
#[derive(Clone, Default)]
pub struct ExportProgress {
    /// Whether export is currently in progress
    pub is_exporting: bool,
    /// Current frame being rendered (0-indexed)
    pub current_frame: u32,
    /// Total frames to render
    pub total_frames: u32,
    /// Time per frame in seconds (for ETA calculation)
    pub seconds_per_frame: f64,
    /// Status message
    pub status: String,
}

impl ExportProgress {
    /// Get progress as a fraction (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        if self.total_frames == 0 {
            0.0
        } else {
            self.current_frame as f32 / self.total_frames as f32
        }
    }

    /// Get estimated time remaining in seconds
    pub fn eta_seconds(&self) -> f64 {
        let remaining_frames = self.total_frames.saturating_sub(self.current_frame);
        remaining_frames as f64 * self.seconds_per_frame
    }
}

/// Export settings for animation rendering
#[derive(Clone)]
pub struct AnimationExportSettings {
    /// Output video file path
    pub output_path: std::path::PathBuf,
    /// Frame width
    pub width: u32,
    /// Frame height
    pub height: u32,
    /// Frames per second
    pub fps: u32,
    /// Iterations per thread
    pub iterations_per_thread: u32,
    /// Video codec
    pub video_codec: VideoCodec,
    /// Hardware acceleration
    pub hardware_accel: HardwareAccel,
    /// Video quality (CRF)
    pub video_quality: u8,
    /// Encoding preset (speed/quality tradeoff)
    pub preset: crate::animation::export::EncodingPreset,
    /// Encoding tune (optimization target, CPU only)
    pub tune: crate::animation::export::EncodingTune,
}

impl Default for AnimationExportSettings {
    fn default() -> Self {
        Self {
            output_path: std::path::PathBuf::from("./animation.mp4"),
            width: 1920,
            height: 1080,
            fps: 30,
            iterations_per_thread: 256,
            video_codec: VideoCodec::H265,
            hardware_accel: HardwareAccel::None,
            video_quality: 12,
            preset: crate::animation::export::EncodingPreset::default(),
            tune: crate::animation::export::EncodingTune::default(),
        }
    }
}

/// Response from animation panel rendering
#[derive(Default)]
pub struct AnimationPanelResponse {
    /// Animation to load (from file picker)
    pub load_animation: Option<Animation>,
    /// Save current animation to file
    pub save_animation: bool,
    /// Export animation request with settings
    pub export_animation: Option<AnimationExportSettings>,
    /// Create a new empty animation
    pub new_animation: bool,
    /// Timeline was scrubbed (slider dragged or frame stepped) - needs fractal update
    pub seek_changed: bool,
    /// Trigger animation load file picker (WASM only - handled in app render loop)
    #[cfg(target_arch = "wasm32")]
    pub trigger_animation_load: bool,
}

/// Render animation panel content
pub fn render_animation_content(
    ui: &mut Ui,
    controller: &mut AnimationController,
    export_settings: &mut AnimationExportSettings,
    export_progress: &ExportProgress,
) -> AnimationPanelResponse {
    let mut response = AnimationPanelResponse::default();

    // Animation info header
    if let Some(ref animation) = controller.animation {
        ui.horizontal(|ui| {
            ui.strong(&animation.name);
            ui.label(t!("animation_panel.duration_format", duration = format!("{:.1}", animation.duration)));
        });
    } else {
        ui.label(t!("animation_panel.no_animation"));
    }

    ui.separator();

    // Playback controls
    render_playback_controls(ui, controller, &mut response);

    ui.separator();

    // Timeline scrubber
    response.seek_changed = render_timeline(ui, controller);

    ui.separator();

    // Speed control
    render_speed_control(ui, controller);

    ui.separator();

    // Loop mode selector
    render_loop_mode(ui, controller);

    ui.separator();

    // Quality mode selector
    render_quality_mode(ui, controller);

    ui.separator();

    // Load/Save buttons
    render_file_controls(ui, controller, &mut response);

    ui.separator();

    // Export animation section
    render_export_controls(ui, controller, export_settings, export_progress, &mut response);

    response
}

/// Render play/pause/stop buttons
fn render_playback_controls(ui: &mut Ui, controller: &mut AnimationController, _response: &mut AnimationPanelResponse) {
    let has_animation = controller.animation.is_some();

    ui.horizontal(|ui| {
        // Play/Pause toggle button
        let play_pause_text = match controller.state {
            PlaybackState::Playing => t!("animation_panel.pause"),
            PlaybackState::Paused => t!("animation_panel.resume"),
            PlaybackState::Stopped => t!("animation_panel.play"),
        };

        if ui.add_enabled(has_animation, egui::Button::new(play_pause_text.as_ref())).clicked() {
            match controller.state {
                PlaybackState::Playing => {
                    controller.pause();
                }
                PlaybackState::Paused | PlaybackState::Stopped => {
                    controller.play();
                }
            }
        }

        // Stop button
        let can_stop = controller.state != PlaybackState::Stopped;
        if ui.add_enabled(has_animation && can_stop, egui::Button::new(t!("animation_panel.stop"))).clicked() {
            controller.stop();
        }

        // Skip to start
        if ui.add_enabled(has_animation, egui::Button::new("⏮")).on_hover_text(t!("animation_panel.go_to_start")).clicked() {
            controller.seek(0.0);
        }

        // Skip to end
        if ui.add_enabled(has_animation, egui::Button::new("⏭")).on_hover_text(t!("animation_panel.go_to_end")).clicked() {
            if let Some(ref animation) = controller.animation {
                controller.seek(animation.duration);
            }
        }
    });

    // Show playback state
    let state_text = match controller.state {
        PlaybackState::Playing => {
            let dir = if controller.direction() < 0.0 { "◀" } else { "▶" };
            t!("animation_panel.playing", direction = dir).to_string()
        }
        PlaybackState::Paused => t!("animation_panel.paused").to_string(),
        PlaybackState::Stopped => t!("animation_panel.stopped").to_string(),
    };
    ui.label(state_text);
}

/// Render timeline scrubber with time display
/// Returns true if the timeline was scrubbed (slider dragged or frame stepped)
fn render_timeline(ui: &mut Ui, controller: &mut AnimationController) -> bool {
    let has_animation = controller.animation.is_some();
    let duration = controller.animation.as_ref().map(|a| a.duration).unwrap_or(1.0);
    let mut seek_changed = false;

    // Time display
    ui.horizontal(|ui| {
        ui.label(t!("animation_panel.time_format",
            current = format!("{:.2}", controller.current_time),
            total = format!("{:.2}", duration)
        ));

        // Progress percentage
        let progress = if duration > 0.0 {
            (controller.current_time / duration * 100.0) as u32
        } else {
            0
        };
        ui.label(t!("animation_panel.progress_percent", percent = progress));
    });

    // Timeline slider
    let mut time = controller.current_time;
    let slider = egui::Slider::new(&mut time, 0.0..=duration)
        .show_value(false)
        .clamping(egui::SliderClamping::Always);

    let response = ui.add_enabled(has_animation, slider);

    if response.changed() {
        controller.seek(time);
        seek_changed = true;
    }

    // Frame stepping (when not playing)
    if controller.state != PlaybackState::Playing {
        ui.horizontal(|ui| {
            let step = 1.0 / 60.0; // ~16ms frame step

            if ui.add_enabled(has_animation, egui::Button::new(t!("animation_panel.frame_back"))).clicked() {
                controller.seek((controller.current_time - step).max(0.0));
                seek_changed = true;
            }

            if ui.add_enabled(has_animation, egui::Button::new(t!("animation_panel.frame_forward"))).clicked() {
                controller.seek((controller.current_time + step).min(duration));
                seek_changed = true;
            }
        });
    }

    seek_changed
}

/// Render playback speed control
fn render_speed_control(ui: &mut Ui, controller: &mut AnimationController) {
    ui.horizontal(|ui| {
        ui.label(t!("animation_panel.speed"));

        // Quick speed buttons
        if ui.small_button("0.25x").clicked() {
            controller.speed = 0.25;
        }
        if ui.small_button("0.5x").clicked() {
            controller.speed = 0.5;
        }
        if ui.small_button("1x").clicked() {
            controller.speed = 1.0;
        }
        if ui.small_button("2x").clicked() {
            controller.speed = 2.0;
        }
    });

    // Fine speed slider
    ui.add(
        egui::Slider::new(&mut controller.speed, 0.1..=4.0)
            .text(t!("animation_panel.playback_speed"))
            .suffix("x")
            .logarithmic(true)
    );
}

/// Render loop mode selector
fn render_loop_mode(ui: &mut Ui, controller: &mut AnimationController) {
    if let Some(ref mut animation) = controller.animation {
        ui.horizontal(|ui| {
            ui.label(t!("animation_panel.loop_mode"));

            egui::ComboBox::from_id_salt("loop_mode")
                .selected_text(loop_mode_label(animation.loop_mode))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut animation.loop_mode, LoopMode::Once, loop_mode_label(LoopMode::Once));
                    ui.selectable_value(&mut animation.loop_mode, LoopMode::Loop, loop_mode_label(LoopMode::Loop));
                    ui.selectable_value(&mut animation.loop_mode, LoopMode::PingPong, loop_mode_label(LoopMode::PingPong));
                });
        });
    }
}

/// Get display label for loop mode
fn loop_mode_label(mode: LoopMode) -> String {
    match mode {
        LoopMode::Once => t!("animation_panel.loop_once").to_string(),
        LoopMode::Loop => t!("animation_panel.loop_repeat").to_string(),
        LoopMode::PingPong => t!("animation_panel.loop_pingpong").to_string(),
    }
}

/// Render quality mode selector
fn render_quality_mode(ui: &mut Ui, controller: &mut AnimationController) {
    ui.horizontal(|ui| {
        ui.label(t!("animation_panel.quality"));

        egui::ComboBox::from_id_salt("quality_mode")
            .selected_text(quality_mode_label(controller.quality_mode))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut controller.quality_mode,
                    AnimationQualityMode::Responsive,
                    quality_mode_label(AnimationQualityMode::Responsive),
                );
                ui.selectable_value(
                    &mut controller.quality_mode,
                    AnimationQualityMode::HighQuality,
                    quality_mode_label(AnimationQualityMode::HighQuality),
                );
            });
    });

    // Show tooltip explaining the mode
    let tooltip = match controller.quality_mode {
        AnimationQualityMode::Responsive => t!("animation_panel.quality_responsive_tooltip"),
        AnimationQualityMode::HighQuality => t!("animation_panel.quality_high_tooltip"),
    };
    ui.small(tooltip.as_ref());
}

/// Get display label for quality mode
fn quality_mode_label(mode: AnimationQualityMode) -> String {
    match mode {
        AnimationQualityMode::Responsive => t!("animation_panel.quality_responsive").to_string(),
        AnimationQualityMode::HighQuality => t!("animation_panel.quality_high").to_string(),
    }
}

/// Render load/save file controls
fn render_file_controls(ui: &mut Ui, controller: &AnimationController, response: &mut AnimationPanelResponse) {
    let has_animation = controller.animation.is_some();

    ui.horizontal(|ui| {
        // New animation button
        if ui.button(t!("animation_panel.new")).on_hover_text(t!("animation_panel.new_tooltip").as_ref()).clicked() {
            response.new_animation = true;
        }

        if ui.button(t!("animation_panel.load")).on_hover_text(t!("animation_panel.load_tooltip").as_ref()).clicked() {
            // Trigger file load dialog
            #[cfg(not(target_arch = "wasm32"))]
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Animation", &["anim", "json"])
                    .pick_file()
                {
                    if let Ok(contents) = std::fs::read_to_string(&path) {
                        match Animation::from_json(&contents) {
                            Ok(animation) => {
                                response.load_animation = Some(animation);
                            }
                            Err(e) => {
                                log::error!("Failed to parse animation: {}", e);
                            }
                        }
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: use native file picker, handled in app render loop
                response.trigger_animation_load = true;
            }
        }

        if ui.add_enabled(has_animation, egui::Button::new(t!("animation_panel.save"))).on_hover_text(t!("animation_panel.save_tooltip").as_ref()).clicked() {
            response.save_animation = true;
        }
    });

    // Info about file format
    ui.small(t!("animation_panel.file_format_hint"));
}

/// Render export controls for high-quality animation rendering
fn render_export_controls(
    ui: &mut Ui,
    controller: &AnimationController,
    settings: &mut AnimationExportSettings,
    progress: &ExportProgress,
    response: &mut AnimationPanelResponse,
) {
    let has_animation = controller.animation.is_some();

    egui::CollapsingHeader::new(t!("animation_panel.export_header"))
        .default_open(progress.is_exporting) // Auto-expand when exporting
        .show(ui, |ui| {
            // Show progress bar when exporting
            if progress.is_exporting {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(&progress.status);
                });

                ui.add(egui::ProgressBar::new(progress.progress())
                    .text(t!("animation_panel.frame_progress",
                        current = progress.current_frame + 1,
                        total = progress.total_frames))
                    .animate(true));

                let eta = progress.eta_seconds();
                if eta > 0.0 {
                    let eta_min = (eta / 60.0).floor() as u32;
                    let eta_sec = (eta % 60.0).floor() as u32;
                    ui.label(t!("animation_panel.eta", min = eta_min, sec = format!("{:02}", eta_sec)));
                }

                ui.separator();
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                let ffmpeg_available = crate::animation::export::is_ffmpeg_available();

                if !ffmpeg_available {
                    ui.horizontal(|ui| {
                        ui.label(t!("animation_panel.ffmpeg_not_found"));
                    });
                    ui.small(t!("animation_panel.ffmpeg_hint"));
                    ui.separator();
                }

                ui.add_enabled_ui(has_animation && !progress.is_exporting && ffmpeg_available, |ui| {
                    // Output file path
                    ui.horizontal(|ui| {
                        ui.label(t!("animation_panel.output"));
                        let path_str = settings.output_path.to_string_lossy().to_string();
                        let mut path_display = path_str.clone();
                        if ui.text_edit_singleline(&mut path_display).changed() {
                            settings.output_path = std::path::PathBuf::from(&path_display);
                        }

                        if ui.button(t!("animation_panel.browse")).clicked() {
                            let extension = settings.video_codec.extension();
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title(t!("animation_panel.save_video_as").as_ref())
                                .add_filter("Video", &[extension])
                                .set_file_name(&format!("animation.{}", extension))
                                .save_file()
                            {
                                settings.output_path = path;
                            }
                        }
                    });

                    // Codec selection
                    ui.horizontal(|ui| {
                        ui.label(t!("animation_panel.codec"));
                        let old_codec = settings.video_codec;
                        egui::ComboBox::from_id_salt("video_codec")
                            .selected_text(settings.video_codec.display_name())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut settings.video_codec, VideoCodec::H264, VideoCodec::H264.display_name());
                                ui.selectable_value(&mut settings.video_codec, VideoCodec::H265, VideoCodec::H265.display_name());
                                ui.selectable_value(&mut settings.video_codec, VideoCodec::VP9, VideoCodec::VP9.display_name());
                            });
                        // Update extension when codec changes
                        if old_codec != settings.video_codec {
                            if let Some(stem) = settings.output_path.file_stem() {
                                let new_name = format!("{}.{}", stem.to_string_lossy(), settings.video_codec.extension());
                                if let Some(parent) = settings.output_path.parent() {
                                    settings.output_path = parent.join(new_name);
                                } else {
                                    settings.output_path = std::path::PathBuf::from(new_name);
                                }
                            }
                        }
                    });

                    // Hardware acceleration
                    ui.horizontal(|ui| {
                        ui.label(t!("animation_panel.encoder"));
                        let old_accel = settings.hardware_accel;
                        egui::ComboBox::from_id_salt("hardware_accel")
                            .selected_text(settings.hardware_accel.display_name())
                            .show_ui(ui, |ui| {
                                for &accel in HardwareAccel::all() {
                                    // Only show options that support the current codec
                                    let supported = accel.supports_codec(settings.video_codec);
                                    ui.add_enabled_ui(supported, |ui| {
                                        let label = if supported {
                                            accel.display_name().to_string()
                                        } else {
                                            t!("animation_panel.encoder_not_available",
                                                encoder = accel.display_name(),
                                                codec = settings.video_codec.display_name()).to_string()
                                        };
                                        ui.selectable_value(&mut settings.hardware_accel, accel, label);
                                    });
                                }
                            });
                        // Reset to software if current accel doesn't support new codec
                        if old_accel != settings.hardware_accel || !settings.hardware_accel.supports_codec(settings.video_codec) {
                            if !settings.hardware_accel.supports_codec(settings.video_codec) {
                                settings.hardware_accel = HardwareAccel::None;
                            }
                        }
                    });

                    // Quality slider
                    ui.horizontal(|ui| {
                        ui.label(t!("animation_panel.quality"));
                        ui.add(egui::Slider::new(&mut settings.video_quality, 0..=51).text(t!("animation_panel.crf").as_ref()));
                    });
                    ui.small(t!("animation_panel.quality_hint"));

                    // Preset dropdown
                    ui.horizontal(|ui| {
                        ui.label("Preset:");

                        use crate::animation::export::EncodingPreset;
                        let available_presets = EncodingPreset::available_for(settings.hardware_accel);

                        if available_presets.is_empty() {
                            ui.label("(not supported)");
                        } else {
                            let old_accel = settings.hardware_accel;

                            // Reset preset to default if hardware accel changed
                            if let Some(prev_accel) = ui.memory_mut(|mem| {
                                mem.data.get_temp::<HardwareAccel>(egui::Id::new("last_hw_accel"))
                            }) {
                                if prev_accel != old_accel {
                                    settings.preset = EncodingPreset::default_for(old_accel);
                                }
                            }
                            ui.memory_mut(|mem| {
                                mem.data.insert_temp(egui::Id::new("last_hw_accel"), old_accel);
                            });

                            egui::ComboBox::from_id_salt("preset_combo")
                                .selected_text(settings.preset.display_name())
                                .show_ui(ui, |ui| {
                                    for preset in available_presets {
                                        ui.selectable_value(
                                            &mut settings.preset,
                                            preset,
                                            preset.display_name()
                                        );
                                    }
                                });
                        }
                    });
                    ui.small("Faster = quick encoding (larger file), Slower = better compression");

                    // Tune dropdown (CPU encoders only)
                    if settings.hardware_accel == HardwareAccel::None {
                        ui.horizontal(|ui| {
                            ui.label("Tune:");

                            use crate::animation::export::EncodingTune;
                            egui::ComboBox::from_id_salt("tune_combo")
                                .selected_text(settings.tune.display_name())
                                .show_ui(ui, |ui| {
                                    for &tune in EncodingTune::all() {
                                        ui.selectable_value(
                                            &mut settings.tune,
                                            tune,
                                            tune.display_name()
                                        );
                                    }
                                });
                        });
                        ui.small("Animation tune recommended for fractal flames");
                    }

                    ui.separator();

                    // Resolution
                    ui.horizontal(|ui| {
                        ui.label(t!("animation_panel.resolution"));
                        ui.add(egui::DragValue::new(&mut settings.width).range(100..=7680).suffix("w"));
                        ui.label("×");
                        ui.add(egui::DragValue::new(&mut settings.height).range(100..=4320).suffix("h"));
                    });

                    // Quick resolution presets
                    ui.horizontal(|ui| {
                        if ui.small_button("720p").clicked() {
                            settings.width = 1280;
                            settings.height = 720;
                        }
                        if ui.small_button("1080p").clicked() {
                            settings.width = 1920;
                            settings.height = 1080;
                        }
                        if ui.small_button("4K").clicked() {
                            settings.width = 3840;
                            settings.height = 2160;
                        }
                    });

                    // FPS
                    ui.horizontal(|ui| {
                        ui.label(t!("animation_panel.frame_rate"));
                        ui.add(egui::DragValue::new(&mut settings.fps).range(1..=120).suffix(" fps"));

                        if ui.small_button("24").clicked() { settings.fps = 24; }
                        if ui.small_button("30").clicked() { settings.fps = 30; }
                        if ui.small_button("60").clicked() { settings.fps = 60; }
                    });

                    // Iterations
                    ui.horizontal(|ui| {
                        ui.label(t!("animation_panel.iterations_thread"));
                        ui.add(egui::DragValue::new(&mut settings.iterations_per_thread).range(32..=4096));
                    });

                    // Estimate
                    if let Some(ref animation) = controller.animation {
                        let total_frames = (animation.duration * settings.fps as f64).ceil() as u32;
                        ui.separator();
                        ui.label(t!("animation_panel.total_frames",
                            frames = total_frames,
                            duration = format!("{:.1}", animation.duration),
                            fps = settings.fps));
                    }

                    ui.separator();

                    // Export button
                    if ui.button(t!("animation_panel.export_video")).clicked() {
                        response.export_animation = Some(settings.clone());
                    }
                });
            }

            #[cfg(target_arch = "wasm32")]
            {
                ui.label(t!("animation_panel.export_not_available_wasm"));
            }
        });
}

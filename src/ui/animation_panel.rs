//! Animation playback panel UI
//!
//! Provides controls for playing, pausing, and scrubbing through animations,
//! as well as loading and saving animation files.

use egui::Ui;
use crate::animation::{Animation, AnimationController, AnimationQualityMode, LoopMode, PlaybackState};
use crate::animation::export::VideoCodec;

/// Export settings for animation rendering
#[derive(Clone)]
pub struct AnimationExportSettings {
    /// Output directory
    pub output_dir: std::path::PathBuf,
    /// Frame width
    pub width: u32,
    /// Frame height
    pub height: u32,
    /// Frames per second
    pub fps: u32,
    /// Export transparent PNGs
    pub transparent: bool,
    /// Iterations per thread
    pub iterations_per_thread: u32,
    /// Encode to video after rendering
    pub encode_video: bool,
    /// Video codec
    pub video_codec: VideoCodec,
    /// Video quality (CRF)
    pub video_quality: u8,
    /// Output video filename (without extension)
    pub video_name: String,
    /// Delete PNG frames after video encoding
    pub cleanup_frames: bool,
}

impl Default for AnimationExportSettings {
    fn default() -> Self {
        Self {
            output_dir: std::path::PathBuf::from("./animation_export"),
            width: 1920,
            height: 1080,
            fps: 30,
            transparent: false,
            iterations_per_thread: 256,
            encode_video: false,
            video_codec: VideoCodec::H264,
            video_quality: 18,
            video_name: "animation".to_string(),
            cleanup_frames: false,
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
}

/// Render animation panel content
pub fn render_animation_content(
    ui: &mut Ui,
    controller: &mut AnimationController,
    export_settings: &mut AnimationExportSettings,
) -> AnimationPanelResponse {
    let mut response = AnimationPanelResponse::default();

    // Animation info header
    if let Some(ref animation) = controller.animation {
        ui.horizontal(|ui| {
            ui.strong(&animation.name);
            ui.label(format!("({:.1}s)", animation.duration));
        });
    } else {
        ui.label("No animation loaded");
    }

    ui.separator();

    // Playback controls
    render_playback_controls(ui, controller);

    ui.separator();

    // Timeline scrubber
    render_timeline(ui, controller);

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
    render_file_controls(ui, &mut response);

    ui.separator();

    // Export animation section
    render_export_controls(ui, controller, export_settings, &mut response);

    response
}

/// Render play/pause/stop buttons
fn render_playback_controls(ui: &mut Ui, controller: &mut AnimationController) {
    let has_animation = controller.animation.is_some();

    ui.horizontal(|ui| {
        // Play/Pause toggle button
        let play_pause_text = match controller.state {
            PlaybackState::Playing => "⏸ Pause",
            PlaybackState::Paused => "▶ Resume",
            PlaybackState::Stopped => "▶ Play",
        };

        if ui.add_enabled(has_animation, egui::Button::new(play_pause_text)).clicked() {
            match controller.state {
                PlaybackState::Playing => controller.pause(),
                PlaybackState::Paused | PlaybackState::Stopped => controller.play(),
            }
        }

        // Stop button
        let can_stop = controller.state != PlaybackState::Stopped;
        if ui.add_enabled(has_animation && can_stop, egui::Button::new("⏹ Stop")).clicked() {
            controller.stop();
        }

        // Skip to start
        if ui.add_enabled(has_animation, egui::Button::new("⏮")).on_hover_text("Go to start").clicked() {
            controller.seek(0.0);
        }

        // Skip to end
        if ui.add_enabled(has_animation, egui::Button::new("⏭")).on_hover_text("Go to end").clicked() {
            if let Some(ref animation) = controller.animation {
                controller.seek(animation.duration);
            }
        }
    });

    // Show playback state
    let state_text = match controller.state {
        PlaybackState::Playing => {
            let dir = if controller.direction() < 0.0 { "◀" } else { "▶" };
            format!("Playing {}", dir)
        }
        PlaybackState::Paused => "Paused".to_string(),
        PlaybackState::Stopped => "Stopped".to_string(),
    };
    ui.label(state_text);
}

/// Render timeline scrubber with time display
fn render_timeline(ui: &mut Ui, controller: &mut AnimationController) {
    let has_animation = controller.animation.is_some();
    let duration = controller.animation.as_ref().map(|a| a.duration).unwrap_or(1.0);

    // Time display
    ui.horizontal(|ui| {
        ui.label(format!("{:.2}s / {:.2}s", controller.current_time, duration));

        // Progress percentage
        let progress = if duration > 0.0 {
            (controller.current_time / duration * 100.0) as u32
        } else {
            0
        };
        ui.label(format!("({}%)", progress));
    });

    // Timeline slider
    let mut time = controller.current_time;
    let slider = egui::Slider::new(&mut time, 0.0..=duration)
        .show_value(false)
        .clamping(egui::SliderClamping::Always);

    let response = ui.add_enabled(has_animation, slider);

    if response.changed() {
        controller.seek(time);
    }

    // Frame stepping (when paused)
    if controller.state != PlaybackState::Playing {
        ui.horizontal(|ui| {
            let step = 1.0 / 60.0; // ~16ms frame step

            if ui.add_enabled(has_animation, egui::Button::new("◀ Frame")).clicked() {
                controller.seek((controller.current_time - step).max(0.0));
            }

            if ui.add_enabled(has_animation, egui::Button::new("Frame ▶")).clicked() {
                controller.seek((controller.current_time + step).min(duration));
            }
        });
    }
}

/// Render playback speed control
fn render_speed_control(ui: &mut Ui, controller: &mut AnimationController) {
    ui.horizontal(|ui| {
        ui.label("Speed:");

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
            .text("Playback Speed")
            .suffix("x")
            .logarithmic(true)
    );
}

/// Render loop mode selector
fn render_loop_mode(ui: &mut Ui, controller: &mut AnimationController) {
    if let Some(ref mut animation) = controller.animation {
        ui.horizontal(|ui| {
            ui.label("Loop Mode:");

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
fn loop_mode_label(mode: LoopMode) -> &'static str {
    match mode {
        LoopMode::Once => "Once (Stop at end)",
        LoopMode::Loop => "Loop (Repeat)",
        LoopMode::PingPong => "Ping-Pong (Bounce)",
    }
}

/// Render quality mode selector
fn render_quality_mode(ui: &mut Ui, controller: &mut AnimationController) {
    ui.horizontal(|ui| {
        ui.label("Quality:");

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
        AnimationQualityMode::Responsive => "Fast preview - each frame updates immediately",
        AnimationQualityMode::HighQuality => "Better quality - batches 4 frames for smoother results (slight latency)",
    };
    ui.small(tooltip);
}

/// Get display label for quality mode
fn quality_mode_label(mode: AnimationQualityMode) -> &'static str {
    match mode {
        AnimationQualityMode::Responsive => "Responsive (Fast)",
        AnimationQualityMode::HighQuality => "High Quality (Batched)",
    }
}

/// Render load/save file controls
fn render_file_controls(ui: &mut Ui, response: &mut AnimationPanelResponse) {
    ui.horizontal(|ui| {
        if ui.button("📂 Load Animation").clicked() {
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
        }

        if ui.button("💾 Save Animation").clicked() {
            response.save_animation = true;
        }
    });

    // Info about file format
    ui.small("Animation files are JSON format (.anim or .json)");
}

/// Render export controls for high-quality animation rendering
fn render_export_controls(
    ui: &mut Ui,
    controller: &AnimationController,
    settings: &mut AnimationExportSettings,
    response: &mut AnimationPanelResponse,
) {
    let has_animation = controller.animation.is_some();

    egui::CollapsingHeader::new("🎬 Export Animation")
        .default_open(false)
        .show(ui, |ui| {
            ui.add_enabled_ui(has_animation, |ui| {
                // Output directory
                ui.horizontal(|ui| {
                    ui.label("Output:");
                    let dir_str = settings.output_dir.to_string_lossy().to_string();
                    let mut dir_display = dir_str.clone();
                    if ui.text_edit_singleline(&mut dir_display).changed() {
                        settings.output_dir = std::path::PathBuf::from(&dir_display);
                    }

                    #[cfg(not(target_arch = "wasm32"))]
                    if ui.button("Browse...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Select Output Directory")
                            .pick_folder()
                        {
                            settings.output_dir = path;
                        }
                    }
                });

                // Resolution
                ui.horizontal(|ui| {
                    ui.label("Resolution:");
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
                    ui.label("Frame Rate:");
                    ui.add(egui::DragValue::new(&mut settings.fps).range(1..=120).suffix(" fps"));

                    if ui.small_button("24").clicked() { settings.fps = 24; }
                    if ui.small_button("30").clicked() { settings.fps = 30; }
                    if ui.small_button("60").clicked() { settings.fps = 60; }
                });

                // Iterations
                ui.horizontal(|ui| {
                    ui.label("Iterations/thread:");
                    ui.add(egui::DragValue::new(&mut settings.iterations_per_thread).range(64..=4096));
                });

                // Transparent
                ui.checkbox(&mut settings.transparent, "Transparent background");

                // Estimate
                if let Some(ref animation) = controller.animation {
                    let total_frames = (animation.duration * settings.fps as f64).ceil() as u32;
                    ui.separator();
                    ui.label(format!("Total frames: {} ({:.1}s × {} fps)",
                        total_frames, animation.duration, settings.fps));
                }

                ui.separator();

                // Video encoding section
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let ffmpeg_available = crate::animation::export::is_ffmpeg_available();

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut settings.encode_video, "Encode to video");
                        if !ffmpeg_available {
                            ui.label("⚠").on_hover_text("ffmpeg not found. Install ffmpeg to enable video encoding.");
                        }
                    });

                    if settings.encode_video {
                        ui.add_enabled_ui(ffmpeg_available, |ui| {
                            // Codec selection
                            ui.horizontal(|ui| {
                                ui.label("Codec:");
                                egui::ComboBox::from_id_salt("video_codec")
                                    .selected_text(settings.video_codec.display_name())
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut settings.video_codec, VideoCodec::H264, VideoCodec::H264.display_name());
                                        ui.selectable_value(&mut settings.video_codec, VideoCodec::H265, VideoCodec::H265.display_name());
                                        ui.selectable_value(&mut settings.video_codec, VideoCodec::VP9, VideoCodec::VP9.display_name());
                                    });
                            });

                            // Quality slider
                            ui.horizontal(|ui| {
                                ui.label("Quality:");
                                ui.add(egui::Slider::new(&mut settings.video_quality, 0..=51).text("CRF"));
                            });
                            ui.small("Lower = better quality, larger file. 18 = visually lossless");

                            // Output name
                            ui.horizontal(|ui| {
                                ui.label("Video name:");
                                ui.text_edit_singleline(&mut settings.video_name);
                                ui.label(format!(".{}", settings.video_codec.extension()));
                            });

                            // Cleanup option
                            ui.checkbox(&mut settings.cleanup_frames, "Delete PNGs after encoding");
                        });
                    }

                    ui.separator();
                }

                // Export button
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let button_text = if settings.encode_video {
                        "🎬 Export Animation + Video"
                    } else {
                        "🎬 Export Animation to PNG Sequence"
                    };

                    if ui.button(button_text).clicked() {
                        response.export_animation = Some(settings.clone());
                    }
                }

                #[cfg(target_arch = "wasm32")]
                {
                    ui.label("Animation export not available in web version");
                }
            });
        });
}

/// Show animation track summary (collapsible)
pub fn render_track_summary(ui: &mut Ui, controller: &AnimationController) {
    if let Some(ref animation) = controller.animation {
        egui::CollapsingHeader::new(format!("Tracks ({})", animation.tracks.len() + animation.circular_tracks.len()))
            .default_open(false)
            .show(ui, |ui| {
                // Regular tracks
                for (path, track) in &animation.tracks {
                    let track_type = match &track.source {
                        crate::animation::TrackSource::Keyframes { keyframes } => {
                            format!("Keyframes ({})", keyframes.len())
                        }
                        crate::animation::TrackSource::Oscillator { oscillator_type, .. } => {
                            format!("{:?}", oscillator_type)
                        }
                    };
                    ui.label(format!("  {} → {}", path, track_type));
                }

                // Circular tracks
                for circular in &animation.circular_tracks {
                    ui.label(format!("  Circular → {}, {}", circular.target_x, circular.target_y));
                }
            });
    }
}

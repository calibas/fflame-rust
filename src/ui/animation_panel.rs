//! Animation playback panel UI
//!
//! Provides controls for playing, pausing, and scrubbing through animations,
//! as well as loading and saving animation files.
//!
//! Layout:
//! ┌──────────────────────────────────────────────────────────────────────────┐
//! │ TOP LEFT (Playback)           │ TOP RIGHT (File & Export)               │
//! │ [▶][⏸][⏹] [◀][▶] Speed:[1x▼] │ Name: [________] [Save][Load]           │
//! │ Duration: [10.0s] Loop: [▼]   │ [Export Animation]                      │
//! ├──────────────────────────────────────────────────────────────────────────┤
//! │ TIMELINE SCRUBBER (300px+ wide)                                          │
//! │ |----●-------------------------------|  0.0s                      10.0s │
//! ├──────────────────────────────────────────────────────────────────────────┤
//! │ TRACKS SECTION                                                           │
//! │ [+ Add Track]                                                            │
//! │ Track list with bars and keyframe visualization                          │
//! └──────────────────────────────────────────────────────────────────────────┘

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
    /// Timeline was scrubbed (slider dragged or frame stepped) - needs fractal update
    pub seek_changed: bool,
    /// Open the Export Animation panel
    pub open_export_panel: bool,
    /// Timeline layout for aligning tracks with scrubber
    pub timeline_layout: Option<TimelineLayout>,
    /// Trigger animation load file picker (WASM only - handled in app render loop)
    #[cfg(target_arch = "wasm32")]
    pub trigger_animation_load: bool,
}

/// Render animation panel content
///
/// New layout:
/// - Top section: Left (playback) | Right (file/export)
/// - Timeline scrubber (300px+ wide)
/// - Tracks section (rendered via track_editor)
pub fn render_animation_content(
    ui: &mut Ui,
    controller: &mut AnimationController,
    _export_settings: &mut AnimationExportSettings, // Will be used in Phase 5 Export panel
    export_progress: &ExportProgress,
) -> AnimationPanelResponse {
    let mut response = AnimationPanelResponse::default();

    // Ensure animation always exists (Phase 1 change: remove "New Animation" button)
    // The caller should ensure an animation exists before calling this function

    // ═══════════════════════════════════════════════════════════════════════════
    // TOP SECTION: Playback (left) | File & Export (right)
    // ═══════════════════════════════════════════════════════════════════════════
    render_top_section(ui, controller, &mut response);

    ui.separator();

    // ═══════════════════════════════════════════════════════════════════════════
    // TIMELINE SCRUBBER (300px+ wide)
    // ═══════════════════════════════════════════════════════════════════════════
    let (seek_changed, timeline_layout) = render_timeline_scrubber(ui, controller);
    response.seek_changed = seek_changed;
    response.timeline_layout = timeline_layout;

    ui.separator();

    // ═══════════════════════════════════════════════════════════════════════════
    // QUALITY MODE (affects playback quality)
    // ═══════════════════════════════════════════════════════════════════════════
    render_quality_mode(ui, controller);

    ui.separator();

    // ═══════════════════════════════════════════════════════════════════════════
    // EXPORT PROGRESS (shown only when exporting)
    // ═══════════════════════════════════════════════════════════════════════════
    if export_progress.is_exporting {
        render_export_progress(ui, export_progress);
        ui.separator();
    }

    response
}

/// Render top section with playback controls on left and file/export on right
fn render_top_section(
    ui: &mut Ui,
    controller: &mut AnimationController,
    response: &mut AnimationPanelResponse,
) {
    // Use columns for the split layout
    ui.columns(2, |columns| {
        // ═══════════════════════════════════════════════════════════════════════
        // LEFT COLUMN: Playback Controls
        // ═══════════════════════════════════════════════════════════════════════
        render_playback_controls_left(&mut columns[0], controller);

        // ═══════════════════════════════════════════════════════════════════════
        // RIGHT COLUMN: File & Export Controls
        // ═══════════════════════════════════════════════════════════════════════
        render_file_controls_right(&mut columns[1], controller, response);
    });
}

/// Render playback controls (left side of top section)
fn render_playback_controls_left(ui: &mut Ui, controller: &mut AnimationController) {
    let has_animation = controller.animation.is_some();

    // Row 1: Play/Pause/Stop and Step buttons
    ui.horizontal(|ui| {
        // Play button
        let is_playing = controller.state == PlaybackState::Playing;
        if ui.add_enabled(has_animation && !is_playing, egui::Button::new("▶"))
            .on_hover_text(t!("animation_panel.play"))
            .clicked()
        {
            controller.play();
        }

        // Pause button
        if ui.add_enabled(has_animation && is_playing, egui::Button::new("⏸"))
            .on_hover_text(t!("animation_panel.pause"))
            .clicked()
        {
            controller.pause();
        }

        // Stop button
        let can_stop = controller.state != PlaybackState::Stopped;
        if ui.add_enabled(has_animation && can_stop, egui::Button::new("⏹"))
            .on_hover_text(t!("animation_panel.stop"))
            .clicked()
        {
            controller.stop();
        }

        ui.separator();

        // Step back
        if ui.add_enabled(has_animation, egui::Button::new("◀"))
            .on_hover_text(t!("animation_panel.frame_back"))
            .clicked()
        {
            let step = 1.0 / 60.0;
            controller.seek((controller.current_time - step).max(0.0));
        }

        // Step forward
        if ui.add_enabled(has_animation, egui::Button::new("▶"))
            .on_hover_text(t!("animation_panel.frame_forward"))
            .clicked()
        {
            let step = 1.0 / 60.0;
            let duration = controller.animation.as_ref().map(|a| a.duration).unwrap_or(1.0);
            controller.seek((controller.current_time + step).min(duration));
        }

        ui.separator();

        // Speed dropdown
        ui.label(t!("animation_panel.speed"));
        egui::ComboBox::from_id_salt("playback_speed")
            .selected_text(format!("{:.2}x", controller.speed))
            .width(60.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut controller.speed, 0.25, "0.25x");
                ui.selectable_value(&mut controller.speed, 0.5, "0.5x");
                ui.selectable_value(&mut controller.speed, 1.0, "1x");
                ui.selectable_value(&mut controller.speed, 2.0, "2x");
                ui.selectable_value(&mut controller.speed, 4.0, "4x");
            });
    });

    // Row 2: Duration and Loop Mode
    ui.horizontal(|ui| {
        // Duration input
        ui.label(t!("animation_panel.duration"));
        if let Some(ref mut animation) = controller.animation {
            ui.add(egui::DragValue::new(&mut animation.duration)
                .range(0.1..=3600.0)
                .speed(0.1)
                .suffix("s"));
        } else {
            ui.label("--");
        }

        ui.separator();

        // Loop mode dropdown
        ui.label(t!("animation_panel.loop_mode"));
        if let Some(ref mut animation) = controller.animation {
            egui::ComboBox::from_id_salt("loop_mode_top")
                .selected_text(loop_mode_label(animation.loop_mode))
                .width(80.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut animation.loop_mode, LoopMode::Once, loop_mode_label(LoopMode::Once));
                    ui.selectable_value(&mut animation.loop_mode, LoopMode::Loop, loop_mode_label(LoopMode::Loop));
                    ui.selectable_value(&mut animation.loop_mode, LoopMode::PingPong, loop_mode_label(LoopMode::PingPong));
                });
        } else {
            ui.label("--");
        }
    });
}

/// Render file and export controls (right side of top section)
fn render_file_controls_right(
    ui: &mut Ui,
    controller: &mut AnimationController,
    response: &mut AnimationPanelResponse,
) {
    let has_animation = controller.animation.is_some();

    // Row 1: Name and Save/Load
    ui.horizontal(|ui| {
        ui.label(t!("animation_panel.name"));
        if let Some(ref mut animation) = controller.animation {
            ui.add(egui::TextEdit::singleline(&mut animation.name).desired_width(120.0));
        } else {
            let mut empty = String::new();
            ui.add_enabled(false, egui::TextEdit::singleline(&mut empty).desired_width(120.0));
        }

        // Save button
        if ui.add_enabled(has_animation, egui::Button::new(t!("animation_panel.save")))
            .on_hover_text(t!("animation_panel.save_tooltip"))
            .clicked()
        {
            response.save_animation = true;
        }

        // Load button
        if ui.button(t!("animation_panel.load"))
            .on_hover_text(t!("animation_panel.load_tooltip"))
            .clicked()
        {
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
                response.trigger_animation_load = true;
            }
        }
    });

    // Row 2: Export Animation button
    ui.horizontal(|ui| {
        if ui.add_enabled(has_animation, egui::Button::new(t!("animation_panel.export_animation")))
            .on_hover_text(t!("animation_panel.export_animation_tooltip"))
            .clicked()
        {
            response.open_export_panel = true;
        }
    });
}

/// Layout information for timeline alignment between scrubber and tracks
#[derive(Clone, Copy, Debug)]
pub struct TimelineLayout {
    /// Left edge of the timeline bar area (X coordinate)
    pub bar_left: f32,
    /// Right edge of the timeline bar area (X coordinate)
    pub bar_right: f32,
    /// Animation duration in seconds
    pub duration: f64,
    /// Current time position
    pub current_time: f64,
}

impl TimelineLayout {
    /// Convert a time value to X coordinate
    pub fn time_to_x(&self, time: f64) -> f32 {
        if self.duration <= 0.0 {
            return self.bar_left;
        }
        let t = (time / self.duration).clamp(0.0, 1.0) as f32;
        self.bar_left + t * (self.bar_right - self.bar_left)
    }

    /// Convert X coordinate to time value
    pub fn x_to_time(&self, x: f32) -> f64 {
        if self.bar_right <= self.bar_left {
            return 0.0;
        }
        let t = ((x - self.bar_left) / (self.bar_right - self.bar_left)).clamp(0.0, 1.0);
        t as f64 * self.duration
    }

    /// Get the X coordinate for the current position line
    pub fn position_x(&self) -> f32 {
        self.time_to_x(self.current_time)
    }
}

/// Render timeline scrubber with time display (300px+ wide)
/// Returns (seek_changed, timeline_layout) for track alignment
pub fn render_timeline_scrubber(ui: &mut Ui, controller: &mut AnimationController) -> (bool, Option<TimelineLayout>) {
    let has_animation = controller.animation.is_some();
    let duration = controller.animation.as_ref().map(|a| a.duration).unwrap_or(1.0);
    let mut seek_changed = false;

    // Time display row
    ui.horizontal(|ui| {
        ui.label(format!("{:.2}s", controller.current_time));
        ui.separator();
        ui.label(format!("{:.2}s", duration));

        // Progress percentage
        let progress = if duration > 0.0 {
            (controller.current_time / duration * 100.0) as u32
        } else {
            0
        };
        ui.label(format!("({}%)", progress));
    });

    // Timeline slider - ensure minimum width of 300px
    let available_width = ui.available_width();
    let slider_width = available_width.max(300.0);

    let mut time = controller.current_time;
    let mut layout: Option<TimelineLayout> = None;

    ui.horizontal(|ui| {
        ui.spacing_mut().slider_width = slider_width - 20.0; // Leave some margin

        let slider = egui::Slider::new(&mut time, 0.0..=duration)
            .show_value(false)
            .clamping(egui::SliderClamping::Always);

        let response = ui.add_enabled(has_animation, slider);

        // Capture the slider rect for timeline layout
        let rect = response.rect;
        layout = Some(TimelineLayout {
            bar_left: rect.left(),
            bar_right: rect.right(),
            duration,
            current_time: controller.current_time,
        });

        if response.changed() {
            controller.seek(time);
            seek_changed = true;
        }
    });

    (seek_changed, layout)
}

/// Render export progress (shown only when exporting)
fn render_export_progress(ui: &mut Ui, progress: &ExportProgress) {
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

/// Render export controls for high-quality animation rendering
/// NOTE: This will be moved to a separate Export Animation panel in Phase 5
#[allow(dead_code)]
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
                        ui.label(t!("animation_panel.preset"));

                        use crate::animation::export::EncodingPreset;
                        let available_presets = EncodingPreset::available_for(settings.hardware_accel);

                        if available_presets.is_empty() {
                            ui.label(t!("animation_panel.preset_not_supported"));
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
                    ui.small(t!("animation_panel.preset_hint"));

                    // Tune dropdown (CPU encoders only)
                    if settings.hardware_accel == HardwareAccel::None {
                        ui.horizontal(|ui| {
                            ui.label(t!("animation_panel.tune"));

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
                        ui.small(t!("animation_panel.tune_hint"));
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

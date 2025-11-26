//! Animation playback panel UI
//!
//! Provides controls for playing, pausing, and scrubbing through animations,
//! as well as loading and saving animation files.

use egui::Ui;
use crate::animation::{Animation, AnimationController, LoopMode, PlaybackState};

/// Response from animation panel rendering
#[derive(Default)]
pub struct AnimationPanelResponse {
    /// Animation to load (from file picker)
    pub load_animation: Option<Animation>,
    /// Save current animation to file
    pub save_animation: bool,
}

/// Render animation panel content
pub fn render_animation_content(
    ui: &mut Ui,
    controller: &mut AnimationController,
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

    // Load/Save buttons
    render_file_controls(ui, &mut response);

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

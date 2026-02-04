//! Audio panel for audio-reactive animations
//!
//! Provides UI controls for:
//! - Loading and analyzing audio files
//! - Live audio capture from microphone
//! - Signal monitoring (meters and values)
//! - Audio export settings

#[cfg(feature = "audio")]
use crate::audio::{
    AudioCapture, AudioManager, AudioPlayer, CaptureState, PlaybackState,
};
use rust_i18n::t;

/// State for the audio panel
#[derive(Default)]
pub struct AudioPanelState {
    /// Selected input device index (for live capture)
    pub selected_device_index: usize,
    /// Device list cache (refreshed on panel open)
    pub device_list: Vec<String>,
    /// Whether to show all available signals
    pub show_all_signals: bool,
    /// Audio file path being loaded
    pub loading_path: Option<String>,
}

impl AudioPanelState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Refresh the list of available audio devices
    #[cfg(feature = "audio")]
    pub fn refresh_device_list(&mut self) {
        self.device_list = AudioCapture::list_devices();
        if self.device_list.is_empty() {
            self.device_list.push("Default".to_string());
        }
    }
}

/// Render the audio panel content
#[cfg(feature = "audio")]
pub fn render_audio_panel(
    ui: &mut egui::Ui,
    audio_manager: &mut AudioManager,
    audio_player: &mut AudioPlayer,
    audio_capture: &mut AudioCapture,
    panel_state: &mut AudioPanelState,
    load_audio_file: &mut bool,
) {
    // Section: Audio File
    egui::CollapsingHeader::new(t!("audio.file_section"))
        .default_open(true)
        .show(ui, |ui| {
            render_file_section(ui, audio_manager, audio_player, panel_state, load_audio_file);
        });

    ui.add_space(8.0);

    // Section: Playback Controls (if audio loaded)
    if audio_manager.has_audio() {
        egui::CollapsingHeader::new(t!("audio.playback_section"))
            .default_open(true)
            .show(ui, |ui| {
                render_playback_section(ui, audio_player, audio_manager);
            });

        ui.add_space(8.0);
    }

    // Section: Live Capture
    egui::CollapsingHeader::new(t!("audio.live_section"))
        .default_open(false)
        .show(ui, |ui| {
            render_live_capture_section(ui, audio_capture, panel_state);
        });

    ui.add_space(8.0);

    // Section: Signal Monitor
    egui::CollapsingHeader::new(t!("audio.signals_section"))
        .default_open(true)
        .show(ui, |ui| {
            render_signal_monitor(ui, audio_manager, audio_capture, panel_state);
        });
}

/// Render audio file loading section
#[cfg(feature = "audio")]
fn render_file_section(
    ui: &mut egui::Ui,
    audio_manager: &AudioManager,
    _audio_player: &AudioPlayer,
    panel_state: &mut AudioPanelState,
    load_audio_file: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label(t!("audio.file_label"));

        if let Some(audio_data) = audio_manager.audio_data() {
            // Show file info
            ui.label(format!(
                "{:.1}s | {}Hz | {}ch",
                audio_data.duration(),
                audio_data.sample_rate,
                audio_data.channels
            ));
        } else {
            ui.label(t!("audio.no_file_loaded"));
        }
    });

    ui.horizontal(|ui| {
        if ui.button(t!("audio.load_button")).clicked() {
            *load_audio_file = true;
        }

        if audio_manager.has_audio() {
            if ui.button(t!("audio.analyze_button")).clicked() {
                // Note: This is a simplified trigger - actual analysis
                // happens in the app loop to avoid blocking UI
            }
        }
    });

    // Show analysis progress
    if audio_manager.is_analyzing() {
        let progress = audio_manager.analysis_progress();
        ui.add(egui::ProgressBar::new(progress).text(t!("audio.analyzing")));
    } else if audio_manager.has_audio() && !audio_manager.available_signals().is_empty() {
        ui.label(format!(
            "{}: {} {}",
            t!("audio.analysis_complete"),
            audio_manager.available_signals().len(),
            t!("audio.signals_available")
        ));
    }

    // Show loading state
    if let Some(ref path) = panel_state.loading_path {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(format!("Loading: {}", path));
        });
    }
}

/// Render playback controls
#[cfg(feature = "audio")]
fn render_playback_section(
    ui: &mut egui::Ui,
    audio_player: &mut AudioPlayer,
    audio_manager: &AudioManager,
) {
    ui.horizontal(|ui| {
        // Play/Pause button
        let play_text = match audio_player.state() {
            PlaybackState::Playing => t!("audio.pause"),
            _ => t!("audio.play"),
        };

        if ui.button(play_text.as_ref()).clicked() {
            match audio_player.state() {
                PlaybackState::Playing => audio_player.pause(),
                _ => {
                    let _ = audio_player.play();
                }
            }
        }

        // Stop button
        if ui.button(t!("audio.stop")).clicked() {
            audio_player.stop();
        }
    });

    // Position display and scrubber
    if let Some(duration) = audio_manager.duration() {
        let position = audio_player.position_seconds();

        ui.horizontal(|ui| {
            ui.label(format!(
                "{:.1}s / {:.1}s",
                position, duration
            ));
        });

        // Simple progress bar (read-only for now)
        let progress = (position / duration) as f32;
        ui.add(egui::ProgressBar::new(progress.clamp(0.0, 1.0)));
    }
}

/// Render live capture controls
#[cfg(feature = "audio")]
fn render_live_capture_section(
    ui: &mut egui::Ui,
    audio_capture: &mut AudioCapture,
    panel_state: &mut AudioPanelState,
) {
    // Refresh device list button
    ui.horizontal(|ui| {
        if ui.button(t!("audio.refresh_devices")).clicked() {
            panel_state.refresh_device_list();
        }

        // Device dropdown
        if !panel_state.device_list.is_empty() {
            egui::ComboBox::from_label("")
                .selected_text(
                    panel_state
                        .device_list
                        .get(panel_state.selected_device_index)
                        .cloned()
                        .unwrap_or_else(|| "Default".to_string()),
                )
                .show_ui(ui, |ui| {
                    for (i, device) in panel_state.device_list.iter().enumerate() {
                        if ui
                            .selectable_label(i == panel_state.selected_device_index, device)
                            .clicked()
                        {
                            panel_state.selected_device_index = i;
                        }
                    }
                });
        }
    });

    // Start/Stop capture button
    ui.horizontal(|ui| {
        let capture_state = audio_capture.state();
        let button_text = match capture_state {
            CaptureState::Capturing => t!("audio.stop_capture"),
            _ => t!("audio.start_capture"),
        };

        if ui.button(button_text.as_ref()).clicked() {
            match capture_state {
                CaptureState::Capturing => {
                    audio_capture.stop();
                }
                _ => {
                    let device_name = panel_state
                        .device_list
                        .get(panel_state.selected_device_index)
                        .map(|s| s.as_str());
                    if let Err(e) = audio_capture.start_device(device_name) {
                        log::error!("Failed to start capture: {}", e);
                    }
                }
            }
        }

        // Status indicator
        let status_text = match capture_state {
            CaptureState::Capturing => t!("audio.capturing"),
            CaptureState::Stopped => t!("audio.stopped"),
            #[cfg(target_arch = "wasm32")]
            CaptureState::RequestingPermission => t!("audio.requesting_permission"),
            #[cfg(target_arch = "wasm32")]
            CaptureState::Error => t!("audio.capture_error"),
        };
        ui.label(status_text.as_ref());
    });

    // Level meter when capturing
    if audio_capture.is_capturing() {
        let amplitude = audio_capture.amplitude();
        ui.horizontal(|ui| {
            ui.label(t!("audio.level"));
            ui.add(egui::ProgressBar::new(amplitude).desired_width(150.0));
            ui.label(format!("{:.0} dB", 20.0 * amplitude.max(0.001).log10()));
        });
    }
}

/// Render signal monitor section
#[cfg(feature = "audio")]
fn render_signal_monitor(
    ui: &mut egui::Ui,
    audio_manager: &AudioManager,
    audio_capture: &AudioCapture,
    panel_state: &mut AudioPanelState,
) {
    // Toggle for showing all signals
    ui.checkbox(&mut panel_state.show_all_signals, t!("audio.show_all_signals"));

    // Get signals from both sources
    let offline_signals = audio_manager.available_signals();
    let live_signals = audio_capture.signal_names();

    let has_offline = !offline_signals.is_empty();
    let is_capturing = audio_capture.is_capturing();

    if !has_offline && !is_capturing {
        ui.label(t!("audio.no_signals"));
        return;
    }

    egui::Grid::new("signal_monitor_grid")
        .num_columns(3)
        .spacing([8.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            // Header
            ui.label(t!("audio.signal_name"));
            ui.label(t!("audio.signal_value"));
            ui.label(t!("audio.signal_meter"));
            ui.end_row();

            // Core signals to always show
            let core_signals = [
                "amplitude",
                "energy_low",
                "energy_mid",
                "energy_high",
                "spectral_centroid",
                "spectral_flux",
                "onset",
            ];

            // Show live signals if capturing
            if is_capturing {
                for signal_name in &core_signals {
                    let live_name = format!("live_{}", signal_name);
                    if let Some(value) = audio_capture.get_live_value(&live_name) {
                        render_signal_row(ui, &live_name, value, *signal_name == "onset");
                    }
                }
            }

            // Show offline signals if available
            if has_offline {
                for signal_name in &core_signals {
                    if let Some(signal) = audio_manager.get_signal(signal_name) {
                        // For offline signals, we'd need the current playback time
                        // For now, just show the signal exists
                        let value = signal.data.first().copied().unwrap_or(0.0);
                        render_signal_row(ui, signal_name, value, *signal_name == "onset");
                    }
                }
            }

            // Show additional signals if expanded
            if panel_state.show_all_signals {
                for signal_name in &offline_signals {
                    if !core_signals.contains(signal_name) {
                        if let Some(signal) = audio_manager.get_signal(signal_name) {
                            let value = signal.data.first().copied().unwrap_or(0.0);
                            render_signal_row(ui, signal_name, value, false);
                        }
                    }
                }
            }
        });
}

/// Render a single signal row in the monitor grid
#[cfg(feature = "audio")]
fn render_signal_row(ui: &mut egui::Ui, name: &str, value: f32, is_trigger: bool) {
    // Signal name
    ui.label(name);

    // Value (formatted)
    if is_trigger {
        let status = if value > 0.5 { "[ON]" } else { "[ ]" };
        ui.label(status);
    } else {
        ui.label(format!("{:.2}", value));
    }

    // Meter bar
    if is_trigger {
        // Trigger indicator (filled box when active)
        let (rect, _response) = ui.allocate_exact_size(egui::vec2(20.0, 14.0), egui::Sense::hover());
        let color = if value > 0.5 {
            egui::Color32::GREEN
        } else {
            egui::Color32::DARK_GRAY
        };
        ui.painter().rect_filled(rect, 2.0, color);
    } else {
        ui.add(egui::ProgressBar::new(value.clamp(0.0, 1.0)).desired_width(100.0));
    }

    ui.end_row();
}

/// Placeholder for when audio feature is not enabled
#[cfg(not(feature = "audio"))]
pub fn render_audio_panel(ui: &mut egui::Ui) {
    ui.label(t!("audio.feature_disabled"));
    ui.label(t!("audio.enable_feature_hint"));
}

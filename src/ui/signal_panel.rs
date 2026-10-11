//! Signal panel for managing all signal sources
//!
//! Provides UI controls for:
//! - Loading and analyzing audio files
//! - Live audio capture from microphone
//! - Signal generators (procedural waveforms)
//! - Signal file save/load
//! - Signal monitoring (meters and values)

use crate::audio::{
    AudioCapture, AudioManager, AudioPlayer, CaptureState, PlaybackState,
};
use crate::signal::generator::{GeneratorConfig, WaveformType, SAMPLE_RATE};
use rust_i18n::t;
use std::collections::HashSet;

/// The duration generators render over when no animation is loaded.
pub const DEFAULT_GENERATOR_DURATION: f64 = 10.0;

/// State for the signal panel
pub struct SignalPanelState {
    /// Selected input device index (for live capture)
    pub selected_device_index: usize,
    /// Device list cache (refreshed on panel open)
    pub device_list: Vec<String>,
    /// Whether to show all available signals
    pub show_all_signals: bool,
    /// Audio file path being loaded
    pub loading_path: Option<String>,
    /// User-overridden BPM (None = use auto-detected)
    pub user_bpm: Option<f32>,
    /// Signal generators (procedural waveforms)
    pub generators: Vec<GeneratorConfig>,
    /// Counter for generating unique default names
    generator_counter: usize,
    /// Names of signals loaded from .signal files (tracked for UI display)
    pub loaded_signal_files: Vec<String>,
    /// The duration the generators' signals were last rendered over.
    /// A signal is sampled over a fixed length, so when the animation's
    /// duration changes they are rendered again (`sync_generators`) --
    /// otherwise, past the old length, a track held the last sample.
    generated_duration: f64,
    /// Names under which THIS panel has put a generator signal into the
    /// `SignalManager`. A generator may replace or remove only these: a
    /// name another source owns (audio analysis, a `.signal` file) is
    /// left alone, so typing a new name that passes through `amplitude`
    /// does not replace the audio's `amplitude` and then delete it on
    /// the next keystroke.
    generated_names: HashSet<String>,
}

impl Default for SignalPanelState {
    fn default() -> Self {
        Self {
            selected_device_index: 0,
            device_list: Vec::new(),
            show_all_signals: false,
            loading_path: None,
            user_bpm: None,
            generators: Vec::new(),
            generator_counter: 0,
            loaded_signal_files: Vec::new(),
            generated_duration: f64::NAN,
            generated_names: HashSet::new(),
        }
    }
}

impl SignalPanelState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Restore generators from a loaded animation into panel state and SignalManager.
    /// Called from both desktop (panel_viewer) and WASM (ui_handlers) animation load paths.
    pub fn restore_generators(
        &mut self,
        generators: Vec<GeneratorConfig>,
        signal_manager: &mut crate::signal::SignalManager,
        animation_duration: f64,
    ) {
        // The previous generators' signals go with them.
        let previous = std::mem::replace(&mut self.generators, generators);
        for gen in &previous {
            self.retire_signal_name(&gen.name, signal_manager);
        }
        if !self.generators.is_empty() {
            self.generator_counter = self.generators.len();
            log::info!("Restored {} generators from animation", self.generators.len());
        }
        self.regenerate_all(signal_manager, animation_duration);
    }

    /// Keep the generators' signals current: render them all again
    /// when the duration differs from the one they were rendered over,
    /// and render any that are missing (the first frame after a load).
    ///
    /// The app calls this once per frame, so it holds whether or not
    /// the panel is open, and again just before a video export copies
    /// the signals.
    pub fn sync_generators(
        &mut self,
        signal_manager: &mut crate::signal::SignalManager,
        duration: f64,
    ) {
        if duration != self.generated_duration {
            self.regenerate_all(signal_manager, duration);
            return;
        }
        for idx in 0..self.generators.len() {
            if signal_manager.get(&self.generators[idx].name).is_none() {
                self.publish_generator(idx, signal_manager, duration);
            }
        }
    }

    fn regenerate_all(&mut self, signal_manager: &mut crate::signal::SignalManager, duration: f64) {
        for idx in 0..self.generators.len() {
            self.publish_generator(idx, signal_manager, duration);
        }
        self.generated_duration = duration;
    }

    /// Render generator `idx` into the manager under its name, unless
    /// that name belongs to a signal from another source. Returns
    /// whether it did.
    fn publish_generator(
        &mut self,
        idx: usize,
        signal_manager: &mut crate::signal::SignalManager,
        duration: f64,
    ) -> bool {
        let Some(gen) = self.generators.get(idx) else { return false };
        if signal_manager.get(&gen.name).is_some() && !self.generated_names.contains(&gen.name) {
            return false;
        }
        signal_manager.insert(gen.generate_signal(duration, SAMPLE_RATE));
        self.generated_names.insert(gen.name.clone());
        true
    }

    /// Take a generator's old name out of the manager -- unless another
    /// generator still uses it, or the signal there was never this
    /// panel's.
    fn retire_signal_name(&mut self, name: &str, signal_manager: &mut crate::signal::SignalManager) {
        if self.generators.iter().any(|g| g.name == name) {
            return;
        }
        if self.generated_names.remove(name) {
            signal_manager.remove(name);
        }
    }

    /// Rename generator `idx`. The name field calls this on every
    /// keystroke, so the old name is retired each time: otherwise every
    /// intermediate name stayed behind as a signal.
    pub fn rename_generator(
        &mut self,
        idx: usize,
        new_name: String,
        signal_manager: &mut crate::signal::SignalManager,
        duration: f64,
    ) {
        let Some(gen) = self.generators.get_mut(idx) else { return };
        let old = std::mem::replace(&mut gen.name, new_name);
        self.retire_signal_name(&old, signal_manager);
        self.publish_generator(idx, signal_manager, duration);
    }

    /// Whether generator `idx`'s name is held by a signal from another
    /// source, so the generator's own signal is not in the manager.
    fn generator_name_taken(&self, idx: usize, signal_manager: &crate::signal::SignalManager) -> bool {
        self.generators.get(idx).is_some_and(|g| {
            signal_manager.get(&g.name).is_some() && !self.generated_names.contains(&g.name)
        })
    }

    /// Refresh the list of available audio devices
    pub fn refresh_device_list(&mut self) {
        self.device_list = AudioCapture::list_devices();
        if self.device_list.is_empty() {
            self.device_list.push("Default".to_string());
        }
    }
}

/// Render the signal panel content
pub fn render_signal_panel(
    ui: &mut egui::Ui,
    audio_manager: &mut AudioManager,
    audio_player: &mut AudioPlayer,
    audio_capture: &mut AudioCapture,
    panel_state: &mut SignalPanelState,
    signal_manager: &mut crate::signal::SignalManager,
    current_time: f64,
    animation_duration: f64,
    load_audio_file: &mut bool,
    load_signal_file: &mut bool,
    save_signal_file: &mut Option<String>,
) {
    // Top-level section: Audio File
    egui::CollapsingHeader::new(t!("audio.file_section"))
        .default_open(true)
        .show(ui, |ui| {
            render_file_section(ui, audio_manager, audio_player, panel_state, load_audio_file);
        });

    ui.add_space(4.0);

    // Sub-section: Playback Controls (if audio loaded)
    if audio_manager.has_audio() {
        egui::CollapsingHeader::new(t!("audio.playback_section"))
            .default_open(true)
            .show(ui, |ui| {
                render_playback_section(ui, audio_player, audio_manager);
            });

        ui.add_space(4.0);
    }

    // Top-level sectionn: Live Capture
    egui::CollapsingHeader::new(t!("audio.live_section"))
        .default_open(false)
        .show(ui, |ui| {
            render_live_capture_section(ui, audio_capture, panel_state);
        });

    ui.add_space(8.0);

    // Top-level section: Signal Generators
    egui::CollapsingHeader::new(t!("signal.generators_section"))
        .default_open(false)
        .show(ui, |ui| {
            render_generators_section(ui, panel_state, signal_manager, animation_duration);
        });

    ui.add_space(8.0);

    // Top-level section: Signal Files
    egui::CollapsingHeader::new(t!("signal.files_section"))
        .default_open(false)
        .show(ui, |ui| {
            render_signal_files_section(ui, signal_manager, panel_state, load_signal_file, save_signal_file);
        });

    ui.add_space(8.0);

    // Top-level section: Signal Monitor (always visible)
    egui::CollapsingHeader::new(t!("audio.signals_section"))
        .default_open(true)
        .show(ui, |ui| {
            render_signal_monitor(ui, audio_manager, audio_capture, signal_manager, current_time, panel_state);
        });
}

/// Render audio file loading section
fn render_file_section(
    ui: &mut egui::Ui,
    audio_manager: &AudioManager,
    _audio_player: &AudioPlayer,
    panel_state: &mut SignalPanelState,
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

    // No Analyze button: analysis runs when the file loads (the one
    // that was here did nothing).
    if ui.button(t!("audio.load_button")).clicked() {
        *load_audio_file = true;
    }

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
fn render_live_capture_section(
    ui: &mut egui::Ui,
    audio_capture: &mut AudioCapture,
    panel_state: &mut SignalPanelState,
) {
    // Auto-populate device list on first render
    if panel_state.device_list.is_empty() {
        panel_state.refresh_device_list();
    }

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
                            // Switching while live has to actually switch.
                            // The selection alone changes nothing in the
                            // capture thread, so the old device kept
                            // streaming while the UI claimed the new one was
                            // selected — the control silently lied. Restart
                            // on the new device; if it will not open, stay
                            // stopped so the button reflects reality rather
                            // than leaving the previous device running under
                            // a label that names a different one.
                            if audio_capture.is_capturing() {
                                audio_capture.stop();
                                match audio_capture.start_device(Some(device.as_str())) {
                                    Ok(()) => log::info!("Audio capture switched to '{device}'"),
                                    Err(e) => log::error!(
                                        "Could not switch audio capture to '{device}': {e}"
                                    ),
                                }
                            }
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

    // Show error details when capture failed (WASM only — Error state doesn't exist on desktop)
    #[cfg(target_arch = "wasm32")]
    if audio_capture.state() == CaptureState::Error {
        if let Some(msg) = audio_capture.error_message() {
            ui.colored_label(egui::Color32::from_rgb(255, 120, 120), t!(msg));
        }
    }

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

/// Render signal generators section
fn render_generators_section(
    ui: &mut egui::Ui,
    panel_state: &mut SignalPanelState,
    signal_manager: &mut crate::signal::SignalManager,
    animation_duration: f64,
) {
    // Add generator button
    if ui.small_button(t!("signal.add_generator")).clicked() {
        panel_state.generator_counter += 1;
        let name = format!("gen_{}", panel_state.generator_counter);
        let config = GeneratorConfig::new(name);
        panel_state.generators.push(config);
    }

    if panel_state.generators.is_empty() {
        ui.label(t!("signal.no_generators"));
        return;
    }

    // Current for this duration, none missing (the app does this every
    // frame too).
    panel_state.sync_generators(signal_manager, animation_duration);
    let taken: Vec<bool> = (0..panel_state.generators.len())
        .map(|i| panel_state.generator_name_taken(i, signal_manager))
        .collect();

    // Track changes: (index, what changed) — apply after UI to avoid borrow issues
    let mut to_delete: Option<usize> = None;
    let mut regenerate: Vec<usize> = Vec::new();
    let mut renames: Vec<(usize, String)> = Vec::new();

    egui::Grid::new("generators_grid")
        .num_columns(6)
        .spacing([4.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            // Header row
            ui.label(t!("signal.generator_name"));
            ui.label(t!("signal.generator_waveform"));
            ui.label(t!("signal.generator_frequency"));
            ui.label(t!("signal.generator_phase"));
            ui.label(""); // delete column
            ui.end_row();

            for (i, gen) in panel_state.generators.iter_mut().enumerate() {
                let mut changed = false;

                // Name (editable text). Edited on a copy and applied by
                // `rename_generator`, which retires the old name.
                let mut name = gen.name.clone();
                let name_response = ui.add(
                    egui::TextEdit::singleline(&mut name)
                        .desired_width(80.0)
                );
                super::vkb_sync(ui, &name_response, &name);
                let name_changed = name_response.changed();
                if taken.get(i).copied().unwrap_or(false) {
                    name_response.on_hover_text(t!("signal.generator_name_taken"));
                }
                if name_changed {
                    renames.push((i, name));
                }

                // Waveform dropdown
                let prev_waveform = gen.waveform;
                egui::ComboBox::from_id_salt(format!("gen_wave_{}", i))
                    .selected_text(gen.waveform.display_name())
                    .width(80.0)
                    .show_ui(ui, |ui| {
                        for wt in WaveformType::ALL {
                            ui.selectable_value(&mut gen.waveform, *wt, wt.display_name());
                        }
                    });
                if gen.waveform != prev_waveform {
                    changed = true;
                }

                // Frequency DragValue (Hz)
                let freq_response = ui.add(
                    super::VkbDragValue::new(&mut gen.frequency)
                        .range(0.01..=100.0)
                        .speed(0.01)
                        .fixed_decimals(2)
                        .suffix(" Hz")
                );
                if freq_response.changed() {
                    changed = true;
                }

                // Phase DragValue (0-1)
                let phase_response = ui.add(
                    super::VkbDragValue::new(&mut gen.phase)
                        .range(0.0..=1.0)
                        .speed(0.01)
                        .fixed_decimals(2)
                );
                if phase_response.changed() {
                    changed = true;
                }

                // Delete button
                if ui.small_button(t!("signal.generator_delete")).clicked() {
                    to_delete = Some(i);
                }

                ui.end_row();

                if changed {
                    regenerate.push(i);
                }
            }
        });

    // Renames, then other edits, while the indices still hold
    for (idx, name) in renames {
        panel_state.rename_generator(idx, name, signal_manager, animation_duration);
    }
    for idx in regenerate {
        panel_state.publish_generator(idx, signal_manager, animation_duration);
    }

    // Apply deletions
    if let Some(idx) = to_delete {
        let removed = panel_state.generators.remove(idx);
        panel_state.retire_signal_name(&removed.name, signal_manager);
    }
}

/// Render signal files section (load/save .signal binary files)
fn render_signal_files_section(
    ui: &mut egui::Ui,
    signal_manager: &mut crate::signal::SignalManager,
    panel_state: &mut SignalPanelState,
    load_signal_file: &mut bool,
    save_signal_file: &mut Option<String>,
) {
    // Load button
    if ui.button(t!("signal.load_signal_file")).clicked() {
        *load_signal_file = true;
    }

    if panel_state.loaded_signal_files.is_empty() {
        ui.label(t!("signal.no_signal_files"));
        return;
    }

    // List loaded signal files
    let mut to_remove: Option<usize> = None;

    for (i, name) in panel_state.loaded_signal_files.iter().enumerate() {
        ui.horizontal(|ui| {
            // Signal name and duration
            if let Some(signal) = signal_manager.get(name) {
                let duration = signal.data.len() as f64 / signal.sample_rate;
                ui.label(format!("{} ({:.1}s)", name, duration));
            } else {
                ui.label(name.as_str());
            }

            // Save button
            if ui.small_button(t!("signal.save_signal")).clicked() {
                *save_signal_file = Some(name.clone());
            }

            // Remove button
            if ui.small_button(t!("signal.generator_delete")).clicked() {
                to_remove = Some(i);
            }
        });
    }

    // Apply removal
    if let Some(idx) = to_remove {
        let name = panel_state.loaded_signal_files.remove(idx);
        signal_manager.remove(&name);
    }
}

/// Render signal monitor section
fn render_signal_monitor(
    ui: &mut egui::Ui,
    audio_manager: &mut AudioManager,
    audio_capture: &AudioCapture,
    signal_manager: &crate::signal::SignalManager,
    current_time: f64,
    panel_state: &mut SignalPanelState,
) {
    // Toggle for showing all signals
    ui.checkbox(&mut panel_state.show_all_signals, t!("audio.show_all_signals"));

    // Get signals from both sources (owned to avoid borrow conflicts with audio_manager)
    let offline_signals: Vec<String> = audio_manager.available_signals().into_iter().map(|s| s.to_string()).collect();
    let _live_signals = audio_capture.signal_names();
    let generator_signal_names: Vec<String> = panel_state.generators.iter().map(|g| g.name.clone()).collect();

    let has_offline = !offline_signals.is_empty();
    let has_generators = !generator_signal_names.is_empty();
    let is_capturing = audio_capture.is_capturing();

    if !has_offline && !is_capturing && !has_generators {
        ui.label(t!("audio.no_signals"));
        return;
    }

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

    // Pre-extract detected BPM (avoids borrow conflict inside grid)
    let detected_bpm = audio_manager.get_signal("bpm")
        .and_then(|s| s.data.first().copied());

    // Track BPM changes to apply after grid rendering
    let mut bpm_changed: Option<f32> = None;

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

            // Show live signals if capturing
            if is_capturing {
                for signal_name in &core_signals {
                    let live_name = format!("live_{}", signal_name);
                    if let Some(value) = audio_capture.get_live_value(&live_name) {
                        render_signal_row(ui, &live_name, value, *signal_name == "onset");
                    }
                }

                // Show live BPM and beat phase
                if let Some(bpm) = audio_capture.get_live_value("live_bpm") {
                    if bpm > 0.0 {
                        ui.label("live_bpm");
                        ui.label(format!("{:.1}", bpm));
                        ui.label(""); // no meter for BPM
                        ui.end_row();
                    }
                }
                if let Some(phase) = audio_capture.get_live_value("live_beat_phase") {
                    render_signal_row(ui, "live_beat_phase", phase, false);
                }
            }

            // Show offline signals if available
            if has_offline {
                for signal_name in &core_signals {
                    if let Some(signal) = audio_manager.get_signal(signal_name) {
                        let value = signal.value_at(current_time).unwrap_or(0.0);
                        render_signal_row(ui, signal_name, value, *signal_name == "onset");
                    }
                }
            }

            // Show BPM if detected (editable)
            if let Some(det_bpm) = detected_bpm {
                let mut edit_bpm = panel_state.user_bpm.unwrap_or(det_bpm);

                ui.label("bpm");
                let response = ui.add(
                    super::VkbDragValue::new(&mut edit_bpm)
                        .range(30.0..=300.0)
                        .speed(0.1)
                        .fixed_decimals(1)
                );
                // Reset button to restore auto-detected BPM
                if panel_state.user_bpm.is_some() {
                    if ui.small_button("↺").clicked() {
                        panel_state.user_bpm = None;
                        bpm_changed = Some(det_bpm);
                    }
                } else {
                    ui.label(""); // Placeholder
                }
                ui.end_row();

                // If user changed BPM via drag, record it
                if response.changed() {
                    panel_state.user_bpm = Some(edit_bpm);
                    bpm_changed = Some(edit_bpm);
                }
            }

            if has_offline {
                if let Some(beat_signal) = audio_manager.get_signal("beat") {
                    let value = beat_signal.value_at(current_time).unwrap_or(0.0);
                    render_signal_row(ui, "beat", value, true);
                }
                if let Some(phase_signal) = audio_manager.get_signal("beat_phase") {
                    let value = phase_signal.value_at(current_time).unwrap_or(0.0);
                    render_signal_row(ui, "beat_phase", value, false);
                }
            }

            // Show generator signals from SignalManager
            if has_generators {
                for gen_name in &generator_signal_names {
                    if let Some(signal) = signal_manager.get(gen_name) {
                        let value = signal.value_at(current_time).unwrap_or(0.0);
                        render_signal_row(ui, gen_name, value, false);
                    }
                }
            }

            // Show additional signals if expanded
            if panel_state.show_all_signals {
                for signal_name in &offline_signals {
                    if !core_signals.contains(&signal_name.as_str()) {
                        if let Some(signal) = audio_manager.get_signal(signal_name) {
                            let value = signal.value_at(current_time).unwrap_or(0.0);
                            render_signal_row(ui, signal_name, value, false);
                        }
                    }
                }
            }
        });

    // Apply BPM change after grid (avoids borrow conflict)
    if let Some(new_bpm) = bpm_changed {
        regenerate_beat_signals(audio_manager, new_bpm);
    }
}

/// Render a single signal row in the monitor grid
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

/// Regenerate beat and beat_phase signals from a given BPM.
///
/// Uses the existing onset signal to anchor the beat grid, then generates
/// a sawtooth beat_phase (0→1 ramp) and trigger beat signal.
fn regenerate_beat_signals(audio_manager: &mut AudioManager, bpm: f32) {
    use crate::signal::{Signal, SignalType};

    // Get the onset signal for anchoring the beat grid
    let (onset_data, signal_rate) = match audio_manager.get_signal("onset") {
        Some(onset) => (onset.data.clone(), onset.sample_rate),
        None => return, // No onset data, can't regenerate
    };

    if bpm <= 0.0 {
        return;
    }

    let beat_period_samples = signal_rate * 60.0 / bpm as f64;
    let num_samples = onset_data.len();

    // Find the first strong onset to anchor the beat grid
    let first_onset = onset_data.iter().position(|&v| v > 0.0).unwrap_or(0);

    // Generate beat_phase: 0→1 sawtooth locked to BPM
    let mut beat_phase = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let offset = (i as f64 - first_onset as f64).rem_euclid(beat_period_samples);
        let p = (offset / beat_period_samples) as f32;
        beat_phase.push(p.clamp(0.0, 1.0));
    }

    // Generate beat trigger: 1.0 on frames where beat_phase wraps around
    let mut beat = vec![0.0f32; num_samples];
    for i in 1..num_samples {
        if beat_phase[i] < beat_phase[i - 1] - 0.5 {
            beat[i] = 1.0;
        }
    }
    if !beat_phase.is_empty() && beat_phase[0] < 0.01 {
        beat[0] = 1.0;
    }

    // Update the BPM scalar signal
    audio_manager.insert_signal(
        "bpm".to_string(),
        Signal::new("bpm".to_string(), 1.0, SignalType::Scalar, vec![bpm]),
    );

    // Update beat_phase and beat signals
    audio_manager.insert_signal(
        "beat_phase".to_string(),
        Signal::new("beat_phase".to_string(), signal_rate, SignalType::Continuous, beat_phase),
    );
    audio_manager.insert_signal(
        "beat".to_string(),
        Signal::new("beat".to_string(), signal_rate, SignalType::Trigger, beat),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::{Signal, SignalManager, SignalType};

    fn panel_with(names: &[&str]) -> SignalPanelState {
        let mut state = SignalPanelState::new();
        state.generators = names.iter().map(|n| GeneratorConfig::new(n.to_string())).collect();
        state
    }

    /// Renaming retires the old name: the field renames on every
    /// keystroke, and each intermediate name used to stay behind.
    #[test]
    fn a_renamed_generator_leaves_no_old_signal() {
        let mut signals = SignalManager::new();
        let mut state = panel_with(&["A"]);
        state.sync_generators(&mut signals, 2.0);
        assert!(signals.get("A").is_some());

        state.rename_generator(0, "B".into(), &mut signals, 2.0);
        assert!(signals.get("A").is_none(), "the old name must go");
        assert!(signals.get("B").is_some());

        // A name another generator still uses stays.
        let mut state = panel_with(&["A", "A"]);
        state.sync_generators(&mut signals, 2.0);
        state.rename_generator(0, "C".into(), &mut signals, 2.0);
        assert!(signals.get("A").is_some(), "generator 2 still uses it");
    }

    /// A rename that passes through an audio signal's name neither
    /// replaces nor (on the next keystroke) deletes it.
    #[test]
    fn a_rename_through_an_audio_name_leaves_the_audio_alone() {
        let mut signals = SignalManager::new();
        signals.insert(Signal::new("amplitude".into(), 10.0, SignalType::Continuous, vec![0.25; 4]));
        let mut state = panel_with(&["gen_1"]);
        state.sync_generators(&mut signals, 2.0);

        state.rename_generator(0, "amplitude".into(), &mut signals, 2.0);
        assert_eq!(signals.get("amplitude").unwrap().data, vec![0.25; 4], "not replaced");
        assert!(state.generator_name_taken(0, &signals));
        state.rename_generator(0, "amplitude_lfo".into(), &mut signals, 2.0);
        assert_eq!(signals.get("amplitude").unwrap().data, vec![0.25; 4], "not deleted");
        assert!(signals.get("amplitude_lfo").is_some());
    }

    /// A longer animation renders the generators again over the new
    /// length; before, past the old one a track held the last sample.
    #[test]
    fn a_duration_change_regenerates_the_generators() {
        let mut signals = SignalManager::new();
        let mut state = panel_with(&["lfo"]);
        state.sync_generators(&mut signals, 2.0);
        assert_eq!(signals.get("lfo").unwrap().data.len(), 200);

        let longer = 3.333;
        state.sync_generators(&mut signals, longer);
        assert_eq!(
            signals.get("lfo").unwrap().data.len(),
            (longer * SAMPLE_RATE).ceil() as usize
        );
    }

    /// Loading an animation's generators replaces the previous ones'
    /// signals.
    #[test]
    fn restoring_generators_retires_the_previous_ones() {
        let mut signals = SignalManager::new();
        let mut state = panel_with(&["old"]);
        state.sync_generators(&mut signals, 2.0);
        state.restore_generators(vec![GeneratorConfig::new("new".into())], &mut signals, 4.0);
        assert!(signals.get("old").is_none());
        assert_eq!(signals.get("new").unwrap().data.len(), 400);
    }
}


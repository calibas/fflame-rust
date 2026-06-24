//! Unified export status — one shared state for ALL export paths (PNG direct /
//! high-res / video), shown by a single global overlay and routed to the
//! existing toast system ([`super::EguiLayer::show_api_notification`]) on
//! completion.
//!
//! Replaces the former per-panel `PngExportProgress` + animation `ExportProgress`
//! structs, the two progress-callback traits, and the window-title hack. An
//! export thread writes [`ExportStatus`] (directly or via [`UiReporter`]); the
//! main loop reads it each frame to draw the overlay and to drain the terminal
//! [`ExportStatus::toast`] into a notification.

use std::sync::{Arc, Mutex};

/// Which kind of export is running (for the overlay's headline + icon).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportKind {
    Png,
    Video,
}

impl ExportKind {
    fn noun(self) -> &'static str {
        match self {
            ExportKind::Png => "PNG",
            ExportKind::Video => "video",
        }
    }
}

/// Shared, cloneable export status. One `Arc<Mutex<ExportStatus>>` lives on the
/// app and is handed to each export thread.
///
/// - `active` gates the overlay and disables export buttons / pauses main-loop
///   iteration while a render is running.
/// - `fraction` + `label` + `detail` drive the overlay's bar and text.
/// - `toast` is the terminal message, set once when the export ends and drained
///   by the main loop into the toast notification system.
#[derive(Clone, Default)]
pub struct ExportStatus {
    pub active: bool,
    pub kind: Option<ExportKind>,
    /// Headline, e.g. `"Exporting PNG · 6000×6000"`.
    pub label: String,
    /// Sub-line, e.g. `"Frame 12/120 · ETA 1m 20s"` or `"Tonemapping…"`.
    pub detail: String,
    /// Overall progress in `[0, 1]`.
    pub fraction: f32,
    /// Terminal toast `(message, is_error)`, drained by the main loop.
    pub toast: Option<(String, bool)>,
}

impl ExportStatus {
    /// Begin an export: mark active, set the headline, clear prior progress.
    pub fn begin(&mut self, kind: ExportKind, label: impl Into<String>) {
        self.active = true;
        self.kind = Some(kind);
        self.label = label.into();
        self.detail = String::new();
        self.fraction = 0.0;
        self.toast = None;
    }

    /// Update incremental progress (clamped) and the detail line.
    pub fn set(&mut self, fraction: f32, detail: impl Into<String>) {
        self.fraction = fraction.clamp(0.0, 1.0);
        self.detail = detail.into();
    }

    /// End the export successfully and queue a success toast.
    pub fn finish_ok(&mut self, message: impl Into<String>) {
        self.active = false;
        self.fraction = 1.0;
        self.detail = String::new();
        self.toast = Some((message.into(), false));
    }

    /// End the export with an error and queue an error toast.
    pub fn finish_err(&mut self, message: impl Into<String>) {
        self.active = false;
        self.detail = String::new();
        self.toast = Some((message.into(), true));
    }

    /// Headline for the overlay, e.g. `"⏳ Exporting PNG"`. Falls back to a
    /// generic string if `kind`/`label` aren't set.
    pub fn headline(&self) -> String {
        if !self.label.is_empty() {
            self.label.clone()
        } else if let Some(kind) = self.kind {
            format!("Exporting {}", kind.noun())
        } else {
            "Exporting".to_string()
        }
    }
}

/// [`crate::export::ExportReporter`] that writes the shared [`ExportStatus`] so
/// the in-app overlay updates live. The terminal toast is set by the spawning
/// code (which knows the output path / error), not here.
pub struct UiReporter {
    status: Arc<Mutex<ExportStatus>>,
}

impl UiReporter {
    pub fn new(status: Arc<Mutex<ExportStatus>>) -> Self {
        Self { status }
    }
}

impl crate::export::ExportReporter for UiReporter {
    fn progress(&mut self, fraction: f32, detail: &str) {
        if let Ok(mut s) = self.status.lock() {
            s.set(fraction, detail);
        }
    }
}

/// Render the global export-progress overlay (bottom-center, just above where
/// the toast notification sits). Drawn every frame while an export is active,
/// regardless of which panels are docked. Mirrors the toast's foreground-area
/// styling for visual consistency.
pub fn render_export_overlay(ctx: &egui::Context, status: &ExportStatus) {
    if !status.active {
        return;
    }

    let headline = status.headline();
    let fraction = status.fraction;
    let detail = status.detail.clone();

    egui::Area::new(egui::Id::new("export_progress_overlay"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -110.0))
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 40, 235))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(16, 10))
                .show(ui, |ui| {
                    ui.set_min_width(320.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(format!("⏳ {headline}"))
                                .color(egui::Color32::WHITE)
                                .size(14.0),
                        );
                        ui.add(
                            egui::ProgressBar::new(fraction)
                                .show_percentage()
                                .desired_width(320.0),
                        );
                        if !detail.is_empty() {
                            ui.label(
                                egui::RichText::new(&detail)
                                    .color(egui::Color32::from_gray(200))
                                    .size(12.0),
                            );
                        }
                    });
                });
        });

    // Keep repainting so the bar animates even when the UI is otherwise idle.
    ctx.request_repaint();
}

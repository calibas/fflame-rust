//! Unified export status — one shared state for ALL export paths (PNG direct /
//! high-res / video), shown by the menu bar's progress bar
//! ([`super::render_progress`]) and routed to the existing toast system
//! ([`super::EguiLayer::show_api_notification`]) on completion.
//!
//! Replaces the former per-panel `PngExportProgress` + animation `ExportProgress`
//! structs, the two progress-callback traits, and the window-title hack. An
//! export thread writes [`ExportStatus`] (directly or via [`UiReporter`]); the
//! main loop reads it each frame to fill the progress bar and to drain the terminal
//! [`ExportStatus::toast`] into a notification.

use std::sync::{Arc, Mutex};

/// Which kind of export is running (for the progress bar's headline).
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
/// - `active` hands the progress bar to the export and disables export buttons / pauses main-loop
///   iteration while a render is running.
/// - `fraction` + `label` + `detail` drive the progress bar and its hover text.
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

    /// Headline for the progress bar's hover text, e.g. `"⏳ Exporting PNG"`. Falls back to a
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
/// the menu bar's progress bar updates live. The terminal toast is set by the spawning
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

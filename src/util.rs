// Use web-time for WASM compatibility (provides Instant on all platforms)
use web_time::{Duration, Instant};

/// Performance snapshot for logging/export
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PerformanceSnapshot {
    pub version: String,
    pub build_number: u32,
    pub git_hash: String,
    pub fps: f64,
    pub frame_time_ms: f64,
    pub frame_count: u64,
    pub compute_time_ms: f64,
    pub accumulate_time_ms: f64,
    pub tonemap_time_ms: f64,
    pub ui_time_ms: f64,
    pub timestamp: String,
}

/// Performance metrics tracker with detailed component timing
#[derive(serde::Serialize, serde::Deserialize)]
pub struct PerformanceMetrics {
    #[serde(skip, default = "Instant::now")]
    last_frame_time: Instant,
    #[serde(skip, default)]
    frame_times: Vec<Duration>,
    max_samples: usize,
    fps: f64,
    frame_time_ms: f64,
    frame_count: u64,

    // Component timing
    pub compute_time_ms: f64,
    pub accumulate_time_ms: f64,
    pub tonemap_time_ms: f64,
    pub ui_time_ms: f64,
    pub submit_time_ms: f64,
    pub present_time_ms: f64,
    pub render_time_ms: f64,  // Total time spent in render() function

    // Version and build info (captured at creation, not serialized/deserialized)
    #[serde(skip)]
    pub version_info: Option<crate::version::VersionInfo>,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            last_frame_time: Instant::now(),
            frame_times: Vec::with_capacity(60),
            max_samples: 60,
            fps: 0.0,
            frame_time_ms: 0.0,
            frame_count: 0,
            compute_time_ms: 0.0,
            accumulate_time_ms: 0.0,
            tonemap_time_ms: 0.0,
            ui_time_ms: 0.0,
            submit_time_ms: 0.0,
            present_time_ms: 0.0,
            render_time_ms: 0.0,
            version_info: Some(crate::version::VersionInfo::current()),
        }
    }

    /// Export performance statistics as JSON (includes version info)
    pub fn export_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Get version-tagged performance snapshot
    pub fn snapshot(&self) -> PerformanceSnapshot {
        PerformanceSnapshot {
            version: self.version_info.as_ref().map(|v| v.full_version()).unwrap_or_else(|| "unknown".to_string()),
            build_number: self.version_info.as_ref().map(|v| v.build_number).unwrap_or(0),
            git_hash: self.version_info.as_ref().map(|v| v.git_hash.to_string()).unwrap_or_else(|| "unknown".to_string()),
            fps: self.fps,
            frame_time_ms: self.frame_time_ms,
            frame_count: self.frame_count,
            compute_time_ms: self.compute_time_ms,
            accumulate_time_ms: self.accumulate_time_ms,
            tonemap_time_ms: self.tonemap_time_ms,
            ui_time_ms: self.ui_time_ms,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Log performance snapshot to console (WASM-compatible)
    pub fn log_snapshot(&self) {
        let snapshot = self.snapshot();
        log::info!("Performance Snapshot:");
        log::info!("  Version: {}", snapshot.version);
        log::info!("  Build: #{}", snapshot.build_number);
        log::info!("  Git: {}", snapshot.git_hash);
        log::info!("  FPS: {:.1}", snapshot.fps);
        log::info!("  Frame Time: {:.2}ms", snapshot.frame_time_ms);
        log::info!("  Frame Count: {}", snapshot.frame_count);
        log::info!("  Compute: {:.2}ms", snapshot.compute_time_ms);
        log::info!("  Accumulate: {:.2}ms", snapshot.accumulate_time_ms);
        log::info!("  Tonemap: {:.2}ms", snapshot.tonemap_time_ms);
        log::info!("  UI: {:.2}ms", snapshot.ui_time_ms);
        log::info!("  Timestamp: {}", snapshot.timestamp);
    }

    /// Export snapshot to browser console (WASM only)
    #[cfg(target_arch = "wasm32")]
    pub fn export_to_console(&self) {
        use wasm_bindgen::prelude::*;

        let snapshot = self.snapshot();
        if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
            web_sys::console::log_1(&JsValue::from_str(&format!("Performance Snapshot:\n{}", json)));
        }
    }

    /// Export snapshot to browser console (no-op on desktop)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn export_to_console(&self) {
        // No-op on desktop - use export_json() instead
        log::warn!("export_to_console() is WASM-only. Use export_json() on desktop.");
    }

    /// Update metrics at the end of each frame
    pub fn update(&mut self) {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame_time);
        self.last_frame_time = now;

        self.frame_times.push(delta);
        if self.frame_times.len() > self.max_samples {
            self.frame_times.remove(0);
        }

        // Calculate average over recent frames
        if !self.frame_times.is_empty() {
            let total: Duration = self.frame_times.iter().sum();
            let avg = total / self.frame_times.len() as u32;

            self.frame_time_ms = avg.as_secs_f64() * 1000.0;
            self.fps = if avg.as_secs_f64() > 0.0 {
                1.0 / avg.as_secs_f64()
            } else {
                0.0
            };
        }

        self.frame_count += 1;
    }

    pub fn fps(&self) -> f64 {
        self.fps
    }

    pub fn frame_time_ms(&self) -> f64 {
        self.frame_time_ms
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get min/max frame times for the current sample window
    pub fn frame_time_range(&self) -> (f64, f64) {
        if self.frame_times.is_empty() {
            return (0.0, 0.0);
        }

        let min = self.frame_times.iter().min().unwrap().as_secs_f64() * 1000.0;
        let max = self.frame_times.iter().max().unwrap().as_secs_f64() * 1000.0;
        (min, max)
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

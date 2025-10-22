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
    #[serde(skip, default = "Instant::now")]
    last_display_update: Instant,
    #[serde(skip, default)]
    frame_times: Vec<Duration>,
    max_samples: usize,

    // Displayed values (updated twice per second)
    fps: f64,
    frame_time_ms: f64,
    frame_count: u64,

    // Display update interval (500ms = twice per second)
    #[serde(skip)]
    display_update_interval: Duration,

    // Component timing - displayed values (smoothed, updated twice per second)
    pub compute_time_ms: f64,
    pub accumulate_time_ms: f64,
    pub tonemap_time_ms: f64,
    pub ui_time_ms: f64,
    pub submit_time_ms: f64,
    pub present_time_ms: f64,
    pub render_time_ms: f64,  // Total time spent in render() function

    // Component timing - accumulators for averaging
    #[serde(skip)]
    compute_times: Vec<f64>,
    #[serde(skip)]
    accumulate_times: Vec<f64>,
    #[serde(skip)]
    tonemap_times: Vec<f64>,
    #[serde(skip)]
    ui_times: Vec<f64>,
    #[serde(skip)]
    submit_times: Vec<f64>,
    #[serde(skip)]
    present_times: Vec<f64>,
    #[serde(skip)]
    render_times: Vec<f64>,

    // Version and build info (captured at creation, not serialized/deserialized)
    #[serde(skip)]
    pub version_info: Option<crate::version::VersionInfo>,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            last_frame_time: now,
            last_display_update: now,
            frame_times: Vec::with_capacity(60),
            max_samples: 60,
            fps: 0.0,
            frame_time_ms: 0.0,
            frame_count: 0,
            display_update_interval: Duration::from_millis(500), // Update display twice per second
            compute_time_ms: 0.0,
            accumulate_time_ms: 0.0,
            tonemap_time_ms: 0.0,
            ui_time_ms: 0.0,
            submit_time_ms: 0.0,
            present_time_ms: 0.0,
            render_time_ms: 0.0,
            compute_times: Vec::new(),
            accumulate_times: Vec::new(),
            tonemap_times: Vec::new(),
            ui_times: Vec::new(),
            submit_times: Vec::new(),
            present_times: Vec::new(),
            render_times: Vec::new(),
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

        // Always track frame times for averaging
        self.frame_times.push(delta);
        if self.frame_times.len() > self.max_samples {
            self.frame_times.remove(0);
        }

        // Always increment frame count
        self.frame_count += 1;

        // Check if it's time to update displayed values (twice per second)
        let time_since_display_update = now.duration_since(self.last_display_update);
        if time_since_display_update >= self.display_update_interval {
            // Calculate average FPS and frame time over recent frames
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

            // Update component timing displays with averages
            self.compute_time_ms = Self::average_and_clear(&mut self.compute_times);
            self.accumulate_time_ms = Self::average_and_clear(&mut self.accumulate_times);
            self.tonemap_time_ms = Self::average_and_clear(&mut self.tonemap_times);
            self.ui_time_ms = Self::average_and_clear(&mut self.ui_times);
            self.submit_time_ms = Self::average_and_clear(&mut self.submit_times);
            self.present_time_ms = Self::average_and_clear(&mut self.present_times);
            self.render_time_ms = Self::average_and_clear(&mut self.render_times);

            self.last_display_update = now;
        }
    }

    /// Helper to calculate average and clear a timing vector
    fn average_and_clear(times: &mut Vec<f64>) -> f64 {
        if times.is_empty() {
            return 0.0;
        }
        let avg = times.iter().sum::<f64>() / times.len() as f64;
        times.clear();
        avg
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

    /// Record component timings (accumulated for averaging, updated twice per second)
    pub fn record_compute_time(&mut self, time_ms: f64) {
        self.compute_times.push(time_ms);
    }

    pub fn record_accumulate_time(&mut self, time_ms: f64) {
        self.accumulate_times.push(time_ms);
    }

    pub fn record_tonemap_time(&mut self, time_ms: f64) {
        self.tonemap_times.push(time_ms);
    }

    pub fn record_ui_time(&mut self, time_ms: f64) {
        self.ui_times.push(time_ms);
    }

    pub fn record_submit_time(&mut self, time_ms: f64) {
        self.submit_times.push(time_ms);
    }

    pub fn record_present_time(&mut self, time_ms: f64) {
        self.present_times.push(time_ms);
    }

    pub fn record_render_time(&mut self, time_ms: f64) {
        self.render_times.push(time_ms);
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

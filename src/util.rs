// Use web-time for WASM compatibility (provides Instant on all platforms)
use web_time::{Duration, Instant};

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
        }
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

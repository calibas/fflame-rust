use std::time::{Duration, Instant};

/// Performance metrics tracker
pub struct PerformanceMetrics {
    last_frame_time: Instant,
    frame_times: Vec<Duration>,
    max_samples: usize,
    fps: f64,
    frame_time_ms: f64,
    frame_count: u64,
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
        }
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

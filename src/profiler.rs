/// GPU and CPU profiling utilities for performance analysis
use web_time::Instant;
use egui_wgpu::wgpu::{Device, Queue};

/// GPU timestamp query pool for measuring GPU pass durations
///
/// **WASM Support:** GPU profiling works on WASM but requires async context.
/// For WASM builds, use CPU timing (PerformanceMetrics) for synchronous profiling.
pub struct GpuProfiler {
    query_set: Option<egui_wgpu::wgpu::QuerySet>,
    resolve_buffer: Option<egui_wgpu::wgpu::Buffer>,
    read_buffer: Option<egui_wgpu::wgpu::Buffer>,
    query_count: u32,
    enabled: bool,
}

impl GpuProfiler {
    pub fn new(device: &Device) -> Self {
        // Check if timestamp queries are supported
        let features = device.features();
        let enabled = features.contains(egui_wgpu::wgpu::Features::TIMESTAMP_QUERY);

        if !enabled {
            log::warn!("GPU timestamp queries not supported on this device");
            return Self {
                query_set: None,
                resolve_buffer: None,
                read_buffer: None,
                query_count: 0,
                enabled: false,
            };
        }

        // Create query set for 10 queries (5 passes × 2 timestamps each)
        let query_count = 10;
        let query_set = device.create_query_set(&egui_wgpu::wgpu::QuerySetDescriptor {
            label: Some("GPU Profiler Query Set"),
            ty: egui_wgpu::wgpu::QueryType::Timestamp,
            count: query_count,
        });

        // Buffer to resolve query results to
        let resolve_buffer = device.create_buffer(&egui_wgpu::wgpu::BufferDescriptor {
            label: Some("GPU Profiler Resolve Buffer"),
            size: (query_count as u64) * 8, // 8 bytes per timestamp
            usage: egui_wgpu::wgpu::BufferUsages::QUERY_RESOLVE | egui_wgpu::wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Buffer to read query results from
        let read_buffer = device.create_buffer(&egui_wgpu::wgpu::BufferDescriptor {
            label: Some("GPU Profiler Read Buffer"),
            size: (query_count as u64) * 8,
            usage: egui_wgpu::wgpu::BufferUsages::MAP_READ | egui_wgpu::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            query_set: Some(query_set),
            resolve_buffer: Some(resolve_buffer),
            read_buffer: Some(read_buffer),
            query_count,
            enabled: true,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn query_set(&self) -> Option<&egui_wgpu::wgpu::QuerySet> {
        self.query_set.as_ref()
    }

    /// Begin a profiling scope (write start timestamp)
    pub fn begin_scope(&self, encoder: &mut egui_wgpu::wgpu::CommandEncoder, query_index: u32) {
        if let Some(query_set) = &self.query_set {
            encoder.write_timestamp(query_set, query_index * 2);
        }
    }

    /// End a profiling scope (write end timestamp)
    pub fn end_scope(&self, encoder: &mut egui_wgpu::wgpu::CommandEncoder, query_index: u32) {
        if let Some(query_set) = &self.query_set {
            encoder.write_timestamp(query_set, query_index * 2 + 1);
        }
    }

    /// Resolve query results and copy to read buffer
    pub fn resolve(&self, encoder: &mut egui_wgpu::wgpu::CommandEncoder) {
        if let (Some(query_set), Some(resolve_buffer), Some(read_buffer)) =
            (&self.query_set, &self.resolve_buffer, &self.read_buffer)
        {
            encoder.resolve_query_set(query_set, 0..self.query_count, resolve_buffer, 0);
            encoder.copy_buffer_to_buffer(
                resolve_buffer,
                0,
                read_buffer,
                0,
                self.query_count as u64 * 8,
            );
        }
    }

    /// Read GPU timestamps asynchronously
    pub async fn read_timestamps(&self, queue: &Queue) -> Option<Vec<u64>> {
        if !self.enabled {
            return None;
        }

        let read_buffer = self.read_buffer.as_ref()?;

        // Map the buffer for reading
        let buffer_slice = read_buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();

        buffer_slice.map_async(egui_wgpu::wgpu::MapMode::Read, move |result| {
            tx.send(result).ok();
        });

        // Wait for mapping to complete
        queue.submit(std::iter::empty());
        #[cfg(not(target_arch = "wasm32"))]
        {
            use pollster::FutureExt;
            let _ = rx.block_on().ok()?;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = rx.await.ok()?;
        }

        // Read the data
        let data = buffer_slice.get_mapped_range();
        let timestamps: Vec<u64> = data
            .chunks_exact(8)
            .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
            .collect();

        drop(data);
        read_buffer.unmap();

        Some(timestamps)
    }

    /// Calculate duration between two query indices (in milliseconds)
    pub fn calculate_duration(
        timestamps: &[u64],
        query_index: u32,
        timestamp_period: f32,
    ) -> f64 {
        let start = timestamps[query_index as usize * 2];
        let end = timestamps[query_index as usize * 2 + 1];
        let delta_ns = (end - start) as f64 * timestamp_period as f64;
        delta_ns / 1_000_000.0 // Convert to milliseconds
    }
}

/// CPU profiling scope using RAII
pub struct CpuScope {
    start: Instant,
    name: &'static str,
}

impl CpuScope {
    pub fn new(name: &'static str) -> Self {
        Self {
            start: Instant::now(),
            name,
        }
    }

    pub fn elapsed_ms(&self) -> f64 {
        self.start.elapsed().as_secs_f64() * 1000.0
    }
}

impl Drop for CpuScope {
    fn drop(&mut self) {
        let elapsed = self.elapsed_ms();
        if elapsed > 1.0 {
            // Only log if > 1ms
            log::debug!("{}: {:.2}ms", self.name, elapsed);
        }
    }
}

/// Macro for easy CPU profiling
#[macro_export]
macro_rules! profile_scope {
    ($name:expr) => {
        let _scope = $crate::profiler::CpuScope::new($name);
    };
}

/// Comprehensive frame profiling data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FrameProfile {
    pub frame_number: u64,
    pub total_cpu_ms: f64,
    pub gpu_compute_ms: f64,
    pub gpu_accumulate_ms: f64,
    pub gpu_tonemap_ms: f64,
    pub cpu_ui_ms: f64,
    pub cpu_submit_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_time: Option<String>,
}

impl FrameProfile {
    pub fn new(frame_number: u64) -> Self {
        let version_info = crate::version::get_version_info();
        Self {
            frame_number,
            total_cpu_ms: 0.0,
            gpu_compute_ms: 0.0,
            gpu_accumulate_ms: 0.0,
            gpu_tonemap_ms: 0.0,
            cpu_ui_ms: 0.0,
            cpu_submit_ms: 0.0,
            version: Some(version_info.full_version()),
            build_time: Some(version_info.build_time.to_string()),
        }
    }

    pub fn print_summary(&self) {
        println!("Frame {} Profile:", self.frame_number);
        println!("  Total CPU: {:.2}ms", self.total_cpu_ms);
        println!("  GPU Compute: {:.2}ms", self.gpu_compute_ms);
        println!("  GPU Accumulate: {:.2}ms", self.gpu_accumulate_ms);
        println!("  GPU Tonemap: {:.2}ms", self.gpu_tonemap_ms);
        println!("  CPU UI: {:.2}ms", self.cpu_ui_ms);
        println!("  CPU Submit: {:.2}ms", self.cpu_submit_ms);
    }

    pub fn total_gpu_ms(&self) -> f64 {
        self.gpu_compute_ms + self.gpu_accumulate_ms + self.gpu_tonemap_ms
    }
}

/// Profile history for averaging and analysis
pub struct ProfileHistory {
    profiles: Vec<FrameProfile>,
    max_samples: usize,
}

impl ProfileHistory {
    pub fn new(max_samples: usize) -> Self {
        Self {
            profiles: Vec::with_capacity(max_samples),
            max_samples,
        }
    }

    pub fn add(&mut self, profile: FrameProfile) {
        self.profiles.push(profile);
        if self.profiles.len() > self.max_samples {
            self.profiles.remove(0);
        }
    }

    pub fn average(&self) -> Option<FrameProfile> {
        if self.profiles.is_empty() {
            return None;
        }

        let count = self.profiles.len() as f64;
        let mut avg = FrameProfile::new(0);

        for profile in &self.profiles {
            avg.total_cpu_ms += profile.total_cpu_ms;
            avg.gpu_compute_ms += profile.gpu_compute_ms;
            avg.gpu_accumulate_ms += profile.gpu_accumulate_ms;
            avg.gpu_tonemap_ms += profile.gpu_tonemap_ms;
            avg.cpu_ui_ms += profile.cpu_ui_ms;
            avg.cpu_submit_ms += profile.cpu_submit_ms;
        }

        avg.total_cpu_ms /= count;
        avg.gpu_compute_ms /= count;
        avg.gpu_accumulate_ms /= count;
        avg.gpu_tonemap_ms /= count;
        avg.cpu_ui_ms /= count;
        avg.cpu_submit_ms /= count;

        Some(avg)
    }

    pub fn percentile(&self, p: f64) -> Option<f64> {
        if self.profiles.is_empty() {
            return None;
        }

        let mut times: Vec<f64> = self.profiles.iter().map(|p| p.total_cpu_ms).collect();
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let index = ((times.len() as f64 - 1.0) * p).round() as usize;
        Some(times[index])
    }
}

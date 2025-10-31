use std::collections::HashMap;
use wgpu::*;
use crate::shader_builder_v2::ShaderBuilder;
use crate::scene::transforms::Flame;

/// Manages shader compilation and pipeline caching
/// Only recompiles shaders when the set of active variations changes
pub struct ShaderCache {
    /// Currently active variation names and weights
    active_variations: HashMap<String, f32>,

    /// Compiled shader source (for debugging/inspection)
    pub shader_source_2d: String,
    pub shader_source_3d: String,

    /// Compute pipelines
    pub compute_pipeline_2d: ComputePipeline,
    pub compute_pipeline_3d: ComputePipeline,
}

impl ShaderCache {
    /// Create a new shader cache with initial flame configuration
    pub fn new(device: &Device, flame: &Flame, bind_group_layout: &BindGroupLayout) -> Self {
        let builder = ShaderBuilder::new(crate::variations::global_registry().clone());
        let active_variations = flame.extract_active_variations();

        log::info!("Initial shader compilation with {} active variations", active_variations.len());

        // Build initial shaders
        let shader_source_2d = builder.build_trajectory_2d(&active_variations);
        let shader_source_3d = builder.build_trajectory_3d(&active_variations);

        // Create pipelines
        let compute_pipeline_2d = Self::create_compute_pipeline(
            device,
            bind_group_layout,
            &shader_source_2d,
            "Trajectory 2D (Initial)"
        );

        let compute_pipeline_3d = Self::create_compute_pipeline(
            device,
            bind_group_layout,
            &shader_source_3d,
            "Trajectory 3D (Initial)"
        );

        Self {
            active_variations,
            shader_source_2d,
            shader_source_3d,
            compute_pipeline_2d,
            compute_pipeline_3d,
        }
    }

    /// Check if shaders need recompilation and rebuild if necessary
    /// Returns true if shaders were recompiled
    pub fn ensure_current(&mut self, device: &Device, bind_group_layout: &BindGroupLayout, flame: &Flame) -> bool {
        let needed = flame.extract_active_variations();

        // Only compare which variations are active (keys), not their weights
        // Weights don't affect shader compilation, only which variations are included
        if needed.keys().collect::<std::collections::HashSet<_>>()
            == self.active_variations.keys().collect::<std::collections::HashSet<_>>() {
            return false; // No rebuild needed
        }

        log::info!(
            "Recompiling shaders: variations changed from {} to {} active",
            self.active_variations.len(),
            needed.len()
        );

        // Rebuild shaders
        let builder = ShaderBuilder::new(crate::variations::global_registry().clone());
        self.shader_source_2d = builder.build_trajectory_2d(&needed);
        self.shader_source_3d = builder.build_trajectory_3d(&needed);

        // Recreate pipelines
        self.compute_pipeline_2d = Self::create_compute_pipeline(
            device,
            bind_group_layout,
            &self.shader_source_2d,
            "Trajectory 2D (Recompiled)"
        );

        self.compute_pipeline_3d = Self::create_compute_pipeline(
            device,
            bind_group_layout,
            &self.shader_source_3d,
            "Trajectory 3D (Recompiled)"
        );

        self.active_variations = needed;

        true // Rebuilt
    }

    /// Create a compute pipeline from shader source
    fn create_compute_pipeline(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        source: &str,
        label: &str,
    ) -> ComputePipeline {
        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(label),
            source: ShaderSource::Wgsl(source.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some(&format!("{} Layout", label)),
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }

    /// Get the current 2D pipeline
    pub fn pipeline_2d(&self) -> &ComputePipeline {
        &self.compute_pipeline_2d
    }

    /// Get the current 3D pipeline
    pub fn pipeline_3d(&self) -> &ComputePipeline {
        &self.compute_pipeline_3d
    }

    /// Get the active variation set (for debugging)
    pub fn active_variations(&self) -> &HashMap<String, f32> {
        &self.active_variations
    }
}

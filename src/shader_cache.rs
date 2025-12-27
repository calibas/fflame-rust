use std::collections::HashMap;
use egui_wgpu::wgpu::*;
use crate::shader_builder_v2::{ShaderBuilder, ShaderConstants};
use crate::scene::transforms::{Flame, RenderMode};
use crate::config::FractalConfig;

/// Manages shader compilation and pipeline caching
/// Only recompiles shaders when the set of active variations changes,
/// path_features_enabled state changes, or shader constants change.
///
/// Optimizes by only building the shader for the current render mode.
/// The unused mode's pipeline is a copy of the active one (valid but unused).
pub struct ShaderCache {
    /// Currently active variation names and weights
    active_variations: HashMap<String, f32>,

    /// Whether path features (PathMap mode or path filters) are enabled
    /// When false, uses simplified shaders without path tracking code
    path_features_enabled: bool,

    /// Hard-coded shader constants (trigger rebuild when changed)
    constants: ShaderConstants,

    /// Current render mode (determines which shader is actually built)
    current_render_mode: RenderMode,

    /// Compiled shader source (for debugging/inspection)
    pub shader_source_2d: String,
    pub shader_source_3d: String,

    /// Compute pipelines
    pub compute_pipeline_2d: ComputePipeline,
    pub compute_pipeline_3d: ComputePipeline,
}

impl ShaderCache {
    /// Create a new shader cache with initial flame configuration
    /// Initially uses simplified shaders (path_features_enabled = false)
    /// Only builds the shader for the flame's render mode (2D or 3D)
    pub fn new(device: &Device, flame: &Flame, bind_group_layout: &BindGroupLayout) -> Self {
        let builder = ShaderBuilder::new(crate::variations::global_registry().clone());
        let active_variations = flame.extract_active_variations();
        let path_features_enabled = false;  // Start with simplified shaders
        let constants = ShaderConstants::default();
        let render_mode = flame.render_mode;

        log::info!(
            "Initial shader compilation with {} active variations, path_features={}, mode={:?}",
            active_variations.len(),
            path_features_enabled,
            render_mode
        );

        // Only build the shader for the current render mode
        let is_3d = render_mode == RenderMode::ThreeD;
        let shader_source = builder.build_from_template(
            &active_variations,
            is_3d,
            path_features_enabled,
            &constants,
        );

        // Create pipeline for the active mode
        let compute_pipeline = Self::create_compute_pipeline(
            device,
            bind_group_layout,
            &shader_source,
            if is_3d { "Trajectory 3D (Initial)" } else { "Trajectory 2D (Initial)" }
        );

        // For the unused mode, just clone the active pipeline (it won't be used)
        let (shader_source_2d, shader_source_3d, compute_pipeline_2d, compute_pipeline_3d) = if is_3d {
            (shader_source.clone(), shader_source, compute_pipeline.clone(), compute_pipeline)
        } else {
            (shader_source.clone(), shader_source, compute_pipeline.clone(), compute_pipeline)
        };

        Self {
            active_variations,
            path_features_enabled,
            constants,
            current_render_mode: render_mode,
            shader_source_2d,
            shader_source_3d,
            compute_pipeline_2d,
            compute_pipeline_3d,
        }
    }

    /// Extract shader constants from a FractalConfig
    pub fn constants_from_config(config: &FractalConfig) -> ShaderConstants {
        ShaderConstants {
            num_transforms: config.flame.transforms.len() as u32,
            color_mode: config.color_mode as u32,
            has_final_transform: config.flame.final_transform.is_some(),
            final_transform_index: config.flame.transforms.len() as u32, // Final is after regular transforms
        }
    }

    /// Check if shaders need recompilation and rebuild if necessary
    /// Returns true if shaders were recompiled
    pub fn ensure_current(&mut self, device: &Device, bind_group_layout: &BindGroupLayout, flame: &Flame) -> bool {
        self.ensure_current_with_path_features(device, bind_group_layout, flame, self.path_features_enabled)
    }

    /// Check if shaders need recompilation, with explicit path_features_enabled state
    /// Returns true if shaders were recompiled
    pub fn ensure_current_with_path_features(
        &mut self,
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        flame: &Flame,
        path_features_enabled: bool,
    ) -> bool {
        // Use current constants (caller should use ensure_current_full for constant updates)
        self.ensure_current_full(device, bind_group_layout, flame, path_features_enabled, self.constants.clone())
    }

    /// Full shader update check with explicit path features and constants
    /// Returns true if shaders were recompiled
    pub fn ensure_current_full(
        &mut self,
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        flame: &Flame,
        path_features_enabled: bool,
        constants: ShaderConstants,
    ) -> bool {
        let needed = flame.extract_active_variations();
        let render_mode = flame.render_mode;

        // Check if variations changed (only keys matter, not weights)
        let variations_changed = needed.keys().collect::<std::collections::HashSet<_>>()
            != self.active_variations.keys().collect::<std::collections::HashSet<_>>();

        // Check if path features state changed
        let path_features_changed = path_features_enabled != self.path_features_enabled;

        // Check if hard-coded constants changed
        let constants_changed = constants != self.constants;

        // Check if render mode changed
        let mode_changed = render_mode != self.current_render_mode;

        if !variations_changed && !path_features_changed && !constants_changed && !mode_changed {
            return false; // No rebuild needed
        }

        if variations_changed {
            log::info!(
                "Recompiling shaders: variations changed from {} to {} active",
                self.active_variations.len(),
                needed.len()
            );
        }
        if path_features_changed {
            log::info!(
                "Recompiling shaders: path_features changed from {} to {}",
                self.path_features_enabled,
                path_features_enabled
            );
        }
        if constants_changed {
            log::info!(
                "Recompiling shaders: constants changed (num_transforms: {}->{}, color_mode: {}->{}, has_final: {}->{})",
                self.constants.num_transforms, constants.num_transforms,
                self.constants.color_mode, constants.color_mode,
                self.constants.has_final_transform, constants.has_final_transform,
            );
        }
        if mode_changed {
            log::info!(
                "Recompiling shaders: render mode changed from {:?} to {:?}",
                self.current_render_mode, render_mode
            );
        }

        // Only rebuild the shader for the current render mode
        let builder = ShaderBuilder::new(crate::variations::global_registry().clone());
        let is_3d = render_mode == RenderMode::ThreeD;

        if is_3d {
            self.shader_source_3d = builder.build_from_template(&needed, true, path_features_enabled, &constants);
            self.compute_pipeline_3d = Self::create_compute_pipeline(
                device,
                bind_group_layout,
                &self.shader_source_3d,
                if path_features_enabled { "Trajectory 3D (Path)" } else { "Trajectory 3D (Simple)" }
            );
            // Copy to 2D slot (unused but must be valid)
            self.shader_source_2d = self.shader_source_3d.clone();
            self.compute_pipeline_2d = self.compute_pipeline_3d.clone();
        } else {
            self.shader_source_2d = builder.build_from_template(&needed, false, path_features_enabled, &constants);
            self.compute_pipeline_2d = Self::create_compute_pipeline(
                device,
                bind_group_layout,
                &self.shader_source_2d,
                if path_features_enabled { "Trajectory 2D (Path)" } else { "Trajectory 2D (Simple)" }
            );
            // Copy to 3D slot (unused but must be valid)
            self.shader_source_3d = self.shader_source_2d.clone();
            self.compute_pipeline_3d = self.compute_pipeline_2d.clone();
        }

        self.active_variations = needed;
        self.path_features_enabled = path_features_enabled;
        self.constants = constants;
        self.current_render_mode = render_mode;

        true // Rebuilt
    }

    /// Get current path_features_enabled state
    pub fn path_features_enabled(&self) -> bool {
        self.path_features_enabled
    }

    /// Get current shader constants
    pub fn constants(&self) -> &ShaderConstants {
        &self.constants
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

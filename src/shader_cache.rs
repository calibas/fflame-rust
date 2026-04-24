use std::collections::HashMap;
use egui_wgpu::wgpu::*;
use crate::shader_builder_v2::{ShaderBuilder, ShaderConstants};
use crate::scene::transforms::{Flame, RenderMode};
use crate::config::FractalConfig;

/// Manages shader compilation and pipeline caching
/// Only recompiles shaders when the set of active variations changes,
/// path_features_enabled state changes, xaos_enabled state changes,
/// or shader constants change.
///
/// Optimizes by only building the shader for the current render mode.
/// The unused mode's pipeline is a copy of the active one (valid but unused).
pub struct ShaderCache {
    /// Currently active variation names and weights
    active_variations: HashMap<String, f32>,

    /// Whether path features (PathMap mode or path filters) are enabled
    /// When false, uses simplified shaders without path tracking code
    path_features_enabled: bool,

    /// Whether xaos (chaos-weighted transform selection) is enabled
    /// When false, uses standard transform selection for better performance
    xaos_enabled: bool,

    /// Hard-coded shader constants (trigger rebuild when changed)
    constants: ShaderConstants,

    /// Current render mode (determines which shader is actually built)
    current_render_mode: RenderMode,

    /// Last seen variation registry version. Bumped when variations are added
    /// or removed at runtime (e.g., via the API). Forces a shader rebuild even
    /// when the flame's variation key set is unchanged — needed because a flame
    /// can reference a variation by name before it has been fetched.
    last_registry_version: u64,

    /// Compiled shader source (for debugging/inspection)
    pub shader_source_2d: String,
    pub shader_source_3d: String,

    /// Compute pipelines
    pub compute_pipeline_2d: ComputePipeline,
    pub compute_pipeline_3d: ComputePipeline,
}

impl ShaderCache {
    /// Create a new shader cache with initial flame configuration
    /// Initially uses simplified shaders (path_features_enabled = false, xaos_enabled = false)
    /// Only builds the shader for the flame's render mode (2D or 3D)
    pub fn new(device: &Device, flame: &Flame, bind_group_layout: &BindGroupLayout) -> Self {
        let builder = ShaderBuilder::new(crate::variations::global_registry().clone());
        let active_variations = flame.extract_active_variations();
        let path_features_enabled = false;  // Start with simplified shaders
        let xaos_enabled = flame.has_xaos();  // Enable xaos if flame uses it

        // Derive constants from actual flame (not defaults) to ensure shader matches initial state
        // This prevents mismatch when switching between presets with different transform counts
        let num_transforms = flame.transforms.len().max(1) as u32;  // Ensure at least 1 for safety
        let constants = ShaderConstants {
            num_transforms,
            color_mode: 0,  // Will be updated via ensure_current_full when config loads
            has_final_transform: flame.final_transform.is_some(),
            final_transform_index: flame.transforms.len() as u32,
            has_post_affine: flame.has_post_affine(),
            inlined_transforms: None,
            cumulative_weights: None,
        };
        let render_mode = flame.render_mode;

        log::info!(
            "Initial shader compilation with {} active variations, path_features={}, xaos={}, mode={:?}",
            active_variations.len(),
            path_features_enabled,
            xaos_enabled,
            render_mode
        );

        // Only build the shader for the current render mode
        let is_3d = render_mode == RenderMode::ThreeD;
        let shader_source = builder.build_from_template(
            &active_variations,
            is_3d,
            path_features_enabled,
            xaos_enabled,
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

        let last_registry_version = crate::variations::global_registry().version();

        Self {
            active_variations,
            path_features_enabled,
            xaos_enabled,
            constants,
            current_render_mode: render_mode,
            last_registry_version,
            shader_source_2d,
            shader_source_3d,
            compute_pipeline_2d,
            compute_pipeline_3d,
        }
    }

    /// Extract shader constants from a FractalConfig
    ///
    /// When `shader_builder_v2::should_use_inlined_constants()` returns true,
    /// generates fully inlined constants for maximum performance (CLI export mode).
    /// Otherwise, generates non-inlined constants for interactive mode.
    pub fn constants_from_config(config: &FractalConfig) -> ShaderConstants {
        // Check if inlined constants mode is enabled (CLI export)
        if crate::shader_builder_v2::should_use_inlined_constants() {
            let registry = crate::variations::global_registry();
            ShaderConstants::with_inlined_transforms(
                &config.flame,
                &registry,
                config.color_mode as u32,
            )
        } else {
            // Interactive mode - no inlining to avoid constant shader rebuilds
            // Ensure at least 1 transform to prevent shader overflow (NUM_TRANSFORMS - 1u)
            let num_transforms = config.flame.transforms.len().max(1) as u32;
            ShaderConstants {
                num_transforms,
                color_mode: config.color_mode as u32,
                has_final_transform: config.flame.final_transform.is_some(),
                final_transform_index: config.flame.transforms.len() as u32,
                has_post_affine: config.flame.has_post_affine(),
                inlined_transforms: None,
                cumulative_weights: None,
            }
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
        let xaos_enabled = flame.has_xaos();

        // Check if variations changed (only keys matter, not weights)
        let variations_changed = needed.keys().collect::<std::collections::HashSet<_>>()
            != self.active_variations.keys().collect::<std::collections::HashSet<_>>();

        // Check if path features state changed
        let path_features_changed = path_features_enabled != self.path_features_enabled;

        // Check if xaos state changed
        let xaos_changed = xaos_enabled != self.xaos_enabled;

        // Check if hard-coded constants changed
        let constants_changed = constants != self.constants;

        // Check if render mode changed
        let mode_changed = render_mode != self.current_render_mode;

        // Check if the variation registry itself changed (e.g., a new API
        // variation was fetched). The flame's variation key set won't change
        // in that case, but the WGSL we need to compile does.
        let current_registry_version = crate::variations::global_registry().version();
        let registry_changed = current_registry_version != self.last_registry_version;

        if !variations_changed && !path_features_changed && !xaos_changed && !constants_changed && !mode_changed && !registry_changed {
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
        if xaos_changed {
            log::info!(
                "Recompiling shaders: xaos changed from {} to {}",
                self.xaos_enabled,
                xaos_enabled
            );
        }
        if constants_changed {
            log::info!(
                "Recompiling shaders: constants changed (num_transforms: {}->{}, color_mode: {}->{}, has_final: {}->{}, has_post_affine: {}->{})",
                self.constants.num_transforms, constants.num_transforms,
                self.constants.color_mode, constants.color_mode,
                self.constants.has_final_transform, constants.has_final_transform,
                self.constants.has_post_affine, constants.has_post_affine,
            );
        }
        if mode_changed {
            log::info!(
                "Recompiling shaders: render mode changed from {:?} to {:?}",
                self.current_render_mode, render_mode
            );
        }
        if registry_changed {
            log::info!(
                "Recompiling shaders: variation registry version changed from {} to {}",
                self.last_registry_version, current_registry_version
            );
        }

        // Only rebuild the shader for the current render mode
        let builder = ShaderBuilder::new(crate::variations::global_registry().clone());
        let is_3d = render_mode == RenderMode::ThreeD;

        if is_3d {
            self.shader_source_3d = builder.build_from_template(&needed, true, path_features_enabled, xaos_enabled, &constants);
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
            self.shader_source_2d = builder.build_from_template(&needed, false, path_features_enabled, xaos_enabled, &constants);
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
        self.xaos_enabled = xaos_enabled;
        self.constants = constants;
        self.current_render_mode = render_mode;
        self.last_registry_version = current_registry_version;

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

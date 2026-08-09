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

    /// Init compute pipeline for variations with `wgsl_init`. `None` when no
    /// active variation in the current flame has init. Rebuilt alongside the
    /// main pipelines whenever the active variation set changes.
    pub init_pipeline: Option<ComputePipeline>,

    /// Init shader source (for debugging/inspection); `None` when no init
    /// variation is active.
    pub init_shader_source: Option<String>,

    /// Total (xform_idx, init-bearing-variation) pair count for the current
    /// init shader. Used to size the dispatch (`ceil(pair_count / 64)`).
    pub init_pair_count: u32,

    /// Bind group layout for the init pipeline. Single binding: the
    /// variation_params storage buffer with read_write access.
    pub init_bind_group_layout: BindGroupLayout,

    /// Per-flame WGSL specialization key. Captures inputs to the variation
    /// specializers (currently just `synth`'s active mode set) so the cache
    /// can rebuild when they change. Without this, the cache would early-out
    /// on `variations_changed = false` and the user would see stale dispatch
    /// after mid-flame `synth.mode` edits. Format: each variation that
    /// specializes contributes one `(name, key)` entry; comparing the whole
    /// `Vec` is the change detector. See `compute_specialization_key`.
    specialization_key: Vec<(String, String)>,

    /// Signature of the (global xform_id, init-bearing variation) pairs the
    /// init shader bakes (see `build_init_shader`). Changes when a
    /// structural edit shifts xform_ids — e.g. adding/removing a Linked
    /// transform pushes Finals to new global ids — even though the
    /// active-variation set and constants are unchanged. Without comparing
    /// this, the cache would early-out and the init pipeline would keep
    /// initializing derived params (e.g. `squish`'s `inv_power`) at the OLD
    /// slot, leaving the moved transform's params zeroed and collapsing the
    /// fractal to a line. See `compute_init_signature`.
    init_signature: Vec<(u32, String)>,

    /// Ordered list of active variation names (`active_variation_names_ordered`)
    /// — the order the per-flame local index map assigns local indices in,
    /// which the shader's packed `get_param` offsets are baked from. The
    /// `variations_changed` check only compares the key *set*, so reordering
    /// two variations (same set, different `variation_order`) wouldn't
    /// rebuild — leaving the shader reading each variation's params at the
    /// other's old offset (params appear swapped). Comparing this catches it.
    variation_order_signature: Vec<String>,

    /// How many times the pipelines have been rebuilt, and the wall time
    /// spent doing it (WGSL generation + module + pipeline creation).
    /// Phase 0 of docs/projects/sticky-shader-compilation.md: the point
    /// of the sticky superset is driving this counter to a constant, so
    /// it has to be observable. Includes the initial build in `new`.
    rebuilds: u64,
    rebuild_ms_total: f64,

    /// Shader changes served from `lru` without compiling.
    cache_hits: u64,

    /// Recently compiled pipelines, most-recent first. See
    /// [`CachedPipelines`] and `PIPELINE_CACHE_CAP`.
    lru: Vec<CachedPipelines>,
}

/// One compiled pipeline set, keyed by the WGSL that produced it.
///
/// Layer A of docs/projects/sticky-shader-compilation.md. The key is the
/// generated source itself (main + init), which subsumes every change
/// detector `ensure_current_full` performs and any added later — codegen
/// is a pure function of (flame, flags, constants), so identical text IS
/// an identical pipeline. The init shader must be part of the key: two
/// flames with identical main WGSL can bake different (xform, variation)
/// init pairs when an init-bearing variation sits on a different
/// transform, and a hit that swapped in the wrong init pipeline would
/// zero that transform's derived params.
///
/// The u64 is a prefilter; a candidate must also match the full sources,
/// so a hash collision degrades to a miss rather than binding the wrong
/// pipeline to a flame.
struct CachedPipelines {
    key: u64,
    source: String,
    pipeline: ComputePipeline,
    init_source: Option<String>,
    init_pipeline: Option<ComputePipeline>,
    init_pair_count: u32,
}

/// LRU capacity. Sized for the residual axes the sticky superset cannot
/// collapse (transform count, color mode, occasional flags) — measured
/// joint cardinality in a random batch is small. Each entry holds its
/// WGSL (~150-350 KB) plus compiled pipeline objects, so 8 is a few MB.
const PIPELINE_CACHE_CAP: usize = 8;

impl ShaderCache {
    /// (rebuilds, cache hits, total compile ms) — see the field docs.
    /// A hit is a shader *change* served from the LRU without compiling;
    /// renders where nothing changed at all take the early-out and count
    /// as neither.
    pub fn rebuild_stats(&self) -> (u64, u64, f64) {
        (self.rebuilds, self.cache_hits, self.rebuild_ms_total)
    }

    /// Everything that decides pipeline identity, hashed. See
    /// [`CachedPipelines`] for why source text is the right key.
    fn cache_key(source: &str, init_source: Option<&str>) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut h);
        init_source.hash(&mut h);
        h.finish()
    }

    /// Create a new shader cache with initial flame configuration
    /// Initially uses simplified shaders (path_features_enabled = false, xaos_enabled = false)
    /// Only builds the shader for the flame's render mode (2D or 3D)
    pub fn new(device: &Device, flame: &Flame, bind_group_layout: &BindGroupLayout) -> Self {
        let new_started = web_time::Instant::now();
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
            has_post_affine: flame.has_post_affine(),
            has_attachments: flame.has_attachments(),
            has_post_symmetry: flame.post_symmetry.ty != crate::scene::transforms::PostSymmetryType::None,
            // Bootstrap defaults to 2D (render_mode / preserve_z are
            // config-level since v3 and not available here); the real values
            // arrive via `constants_from_config` on config load, which
            // triggers a rebuild if they differ.
            has_analytic_blur: flame.analytic_blur_active(&crate::variations::global_registry(), RenderMode::TwoD),
            flatten_z_per_iter: false,
            solid_enabled: false,
            probe: false,
            census: false,
            attachment_cap: flame.attachment_cap() as u32,
            inlined_transforms: None,
            cumulative_weights: None,
            // Bootstrap constants before config load; real fx_priority
            // overrides are filled in by `constants_from_config`.
            variation_priorities: std::collections::BTreeMap::new(),
        };
        let render_mode = RenderMode::TwoD;

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
            flame,
            &active_variations,
            is_3d,
            path_features_enabled,
            xaos_enabled,
            // Interactive renderer always uses the direct-histogram
            // output strategy. HighResExporter is the only caller that
            // flips this to false (sample-emit).
            true,
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

        // Build init pipeline alongside the main pipeline. Returns None /
        // None / 0 when no active variation in this flame has `wgsl_init`.
        let init_bind_group_layout = Self::create_init_bind_group_layout(device);
        let init_shader_source = builder.build_init_shader(flame, &active_variations);
        let (init_pipeline, init_pair_count) = Self::build_init_resources(
            device,
            &init_bind_group_layout,
            init_shader_source.as_deref(),
        );

        // Seed the pipeline LRU with the initial build, so returning to
        // the startup flame after visiting others is a hit like any other.
        let lru = vec![CachedPipelines {
            key: Self::cache_key(&shader_source_2d, init_shader_source.as_deref()),
            source: shader_source_2d.clone(),
            pipeline: compute_pipeline_2d.clone(),
            init_source: init_shader_source.clone(),
            init_pipeline: init_pipeline.clone(),
            init_pair_count,
        }];

        let specialization_key = Self::compute_specialization_key(flame, &active_variations);
        let init_signature = Self::compute_init_signature(flame);
        let variation_order_signature =
            flame.active_variation_names_ordered(&crate::variations::global_registry());
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
            init_pipeline,
            init_shader_source,
            init_pair_count,
            init_bind_group_layout,
            specialization_key,
            init_signature,
            variation_order_signature,
            rebuilds: 1,
            rebuild_ms_total: new_started.elapsed().as_secs_f64() * 1000.0,
            cache_hits: 0,
            lru,
        }
    }

    /// Compute the init-pair signature: the sorted list of `(global xform_id,
    /// variation name)` for every transform whose active variation set
    /// includes an init-bearing (`wgsl_init`) variation. MUST enumerate
    /// xform_ids in the same order `build_init_shader` bakes them (normals,
    /// linkeds, finals, then subflame xforms) so a structural shift is
    /// detected. Sorted so the unordered `variations` HashMap iteration
    /// doesn't produce spurious differences.
    fn compute_init_signature(flame: &crate::scene::transforms::Flame) -> Vec<(u32, String)> {
        let registry = crate::variations::global_registry();
        let mut sig: Vec<(u32, String)> = Vec::new();
        let mut emit = |xform_id: u32, xform: &crate::scene::transforms::Transform| {
            for (name, weight) in &xform.variations {
                if weight.abs() < 1e-6 {
                    continue;
                }
                if let Some(info) = registry.get(name) {
                    if info.wgsl_source_init.is_some() {
                        sig.push((xform_id, name.clone()));
                    }
                }
            }
        };
        let mut next_idx: u32 = 0;
        for xform in flame.transforms.iter() {
            emit(next_idx, xform);
            next_idx += 1;
        }
        for xform in flame.linked_transforms.iter() {
            emit(next_idx, xform);
            next_idx += 1;
        }
        for xform in flame.final_transforms.iter() {
            emit(next_idx, xform);
            next_idx += 1;
        }
        let mut sub_offset: u32 = 0;
        for sf in flame.subflames.iter() {
            for xform in sf.transforms.iter() {
                emit(crate::scene::transforms::SUBFLAME_XFORM_ID_BASE + sub_offset, xform);
                sub_offset += 1;
            }
            for xform in sf.final_transforms.iter() {
                emit(crate::scene::transforms::SUBFLAME_XFORM_ID_BASE + sub_offset, xform);
                sub_offset += 1;
            }
        }
        sig.sort();
        sig
    }

    /// Build the per-flame specialization key. Each entry is `(variation_name,
    /// opaque_key)`; comparing the whole `Vec` is the cache change detector.
    ///
    /// Today: synth contributes `("synth", "<sorted comma-joined modes>")` when
    /// any transform has synth active. The mode list ends up baked into the
    /// generated WGSL (see `synth::specialize_wgsl_*`), so any change here
    /// must force a shader rebuild.
    fn compute_specialization_key(
        flame: &Flame,
        active_variations: &HashMap<String, f32>,
    ) -> Vec<(String, String)> {
        let mut key: Vec<(String, String)> = Vec::new();
        if active_variations.contains_key("synth") {
            let modes = crate::variations::defs::synth::synth_modes_in_flame(flame);
            let joined = modes
                .iter()
                .map(|m| m.to_string())
                .collect::<Vec<_>>()
                .join(",");
            key.push(("synth".to_string(), joined));
        }
        if active_variations.contains_key("mondrianomies") {
            let k = crate::variations::defs::mondrianomies::specialization_key(flame);
            if !k.is_empty() {
                key.push(("mondrianomies".to_string(), k));
            }
        }
        key
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
            {
                let mut constants = ShaderConstants::with_inlined_transforms(
                    &config.flame,
                    &registry,
                    config.color_mode as u32,
                    config.render_mode,
                    config.preserve_z,
                );
                constants.solid_enabled = config.solid_strength > 0.0
                    && matches!(config.render_mode, crate::scene::transforms::RenderMode::ThreeD);
                constants
            }
        } else {
            // Interactive mode - no inlining to avoid constant shader rebuilds
            // Ensure at least 1 transform to prevent shader overflow (NUM_TRANSFORMS - 1u)
            let num_transforms = config.flame.transforms.len().max(1) as u32;
            // fx_priority phase overrides still need resolving even in the
            // non-inlined path — they're baked into the per-flame dispatch
            // (the interactive shader can't read per-transform priorities at
            // runtime). Needs the same local index map the buffer populator
            // uses so var indices line up with `xform.variations[idx]`.
            let registry = crate::variations::global_registry();
            let id_map = crate::scene::transforms::compute_local_index_map(
                config.flame.active_variation_names_ordered(&registry),
            );
            ShaderConstants {
                num_transforms,
                color_mode: config.color_mode as u32,
                has_post_affine: config.flame.has_post_affine(),
                has_attachments: config.flame.has_attachments(),
                has_post_symmetry: config.flame.post_symmetry.ty != crate::scene::transforms::PostSymmetryType::None,
                has_analytic_blur: config.flame.analytic_blur_active(&registry, config.render_mode),
                flatten_z_per_iter: matches!(config.render_mode, crate::scene::transforms::RenderMode::ThreeD)
                    && !config.preserve_z,
                solid_enabled: config.solid_strength > 0.0
                    && matches!(config.render_mode, crate::scene::transforms::RenderMode::ThreeD),
                probe: false,
            census: false,
                attachment_cap: config.flame.attachment_cap() as u32,
                inlined_transforms: None,
                cumulative_weights: None,
                variation_priorities: crate::shader_builder_v2::collect_phase_overrides(
                    &config.flame, &registry, &id_map,
                ),
            }
        }
    }

    /// Check if shaders need recompilation and rebuild if necessary
    /// Returns true if shaders were recompiled
    pub fn ensure_current(&mut self, device: &Device, bind_group_layout: &BindGroupLayout, flame: &Flame, render_mode: RenderMode) -> bool {
        self.ensure_current_with_path_features(device, bind_group_layout, flame, self.path_features_enabled, render_mode)
    }

    /// Check if shaders need recompilation, with explicit path_features_enabled state
    /// Returns true if shaders were recompiled
    pub fn ensure_current_with_path_features(
        &mut self,
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        flame: &Flame,
        path_features_enabled: bool,
        render_mode: RenderMode,
    ) -> bool {
        // Use current constants (caller should use ensure_current_full for constant updates)
        self.ensure_current_full(device, bind_group_layout, flame, path_features_enabled, self.constants.clone(), render_mode)
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
        // Scene-level render mode (config-level since v3); the desired mode,
        // compared against the cache's `current_render_mode` below.
        render_mode: RenderMode,
    ) -> bool {
        let needed = flame.extract_active_variations();
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

        // Check if the per-flame WGSL specialization inputs changed (e.g.
        // synth.mode flipped between transforms). The variation key set
        // hasn't changed, but the generated WGSL has, so a rebuild is needed.
        let new_specialization_key = Self::compute_specialization_key(flame, &needed);
        let specialization_changed = new_specialization_key != self.specialization_key;

        // Detect when a structural edit shifted the init shader's baked
        // (xform_id, init-variation) pairs without changing the active set
        // (e.g. adding/removing a Linked transform moves Finals to new
        // global ids). Otherwise the init pipeline keeps writing derived
        // params at the old slot — leaving the moved transform's init params
        // zeroed (squish's inv_power → collapse-to-line).
        let new_init_signature = Self::compute_init_signature(flame);
        let init_changed = new_init_signature != self.init_signature;

        // Detect a variation-ORDER change (same active set, different
        // local index assignment) — reordering two variations shifts the
        // packed get_param offsets the shader baked, so without a rebuild
        // each variation reads the other's params (params appear swapped).
        let new_variation_order =
            flame.active_variation_names_ordered(&crate::variations::global_registry());
        let order_changed = new_variation_order != self.variation_order_signature;

        if !variations_changed && !path_features_changed && !xaos_changed
            && !constants_changed && !mode_changed && !registry_changed
            && !specialization_changed && !init_changed && !order_changed
        {
            return false; // No rebuild needed
        }
        let rebuild_started = web_time::Instant::now();

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
                "Recompiling shaders: constants changed (num_transforms: {}->{}, color_mode: {}->{}, has_post_affine: {}->{})",
                self.constants.num_transforms, constants.num_transforms,
                self.constants.color_mode, constants.color_mode,
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
        if specialization_changed {
            log::info!(
                "Recompiling shaders: specialization key changed from {:?} to {:?}",
                self.specialization_key, new_specialization_key
            );
        }

        // Something changed. Build the WGSL once — it is both the cache
        // key and, on a miss, the thing to compile.
        let builder = ShaderBuilder::new(crate::variations::global_registry().clone());
        let is_3d = render_mode == RenderMode::ThreeD;

        // Interactive renderer always uses the direct-histogram output
        // strategy — sample-emit is reserved for HighResExporter.
        let output_histogram_direct = true;
        let source = builder.build_from_template(
            flame,
            &needed,
            is_3d,
            path_features_enabled,
            xaos_enabled,
            output_histogram_direct,
            &constants,
        );
        let init_source = builder.build_init_shader(flame, &needed);
        let key = Self::cache_key(&source, init_source.as_deref());

        let hit_pos = self.lru.iter().position(|e| {
            e.key == key && e.source == source && e.init_source == init_source
        });

        if let Some(pos) = hit_pos {
            // Layer A hit: the pipelines for this exact WGSL already
            // exist — install them and skip the compile entirely.
            let entry = self.lru.remove(pos);
            self.shader_source_2d = entry.source.clone();
            self.shader_source_3d = entry.source.clone();
            self.compute_pipeline_2d = entry.pipeline.clone();
            self.compute_pipeline_3d = entry.pipeline.clone();
            self.init_shader_source = entry.init_source.clone();
            self.init_pipeline = entry.init_pipeline.clone();
            self.init_pair_count = entry.init_pair_count;
            self.lru.insert(0, entry);
            self.cache_hits += 1;
            log::info!("Shader change served from pipeline cache (no compile)");
        } else {
            let pipeline = Self::create_compute_pipeline(
                device,
                bind_group_layout,
                &source,
                match (is_3d, path_features_enabled) {
                    (true, true) => "Trajectory 3D (Path)",
                    (true, false) => "Trajectory 3D (Simple)",
                    (false, true) => "Trajectory 2D (Path)",
                    (false, false) => "Trajectory 2D (Simple)",
                },
            );
            let (init_pipeline, init_pair_count) = Self::build_init_resources(
                device,
                &self.init_bind_group_layout,
                init_source.as_deref(),
            );

            // Both mode slots get the same pipeline, as always — only the
            // active mode's shader is real; the other slot must be valid.
            self.shader_source_2d = source.clone();
            self.shader_source_3d = source.clone();
            self.compute_pipeline_2d = pipeline.clone();
            self.compute_pipeline_3d = pipeline.clone();
            self.init_shader_source = init_source.clone();
            self.init_pipeline = init_pipeline.clone();
            self.init_pair_count = init_pair_count;

            self.lru.insert(
                0,
                CachedPipelines {
                    key,
                    source,
                    pipeline,
                    init_source,
                    init_pipeline,
                    init_pair_count,
                },
            );
            self.lru.truncate(PIPELINE_CACHE_CAP);
            self.rebuilds += 1;
            self.rebuild_ms_total += rebuild_started.elapsed().as_secs_f64() * 1000.0;
        }

        self.active_variations = needed;
        self.path_features_enabled = path_features_enabled;
        self.xaos_enabled = xaos_enabled;
        self.constants = constants;
        self.current_render_mode = render_mode;
        self.last_registry_version = current_registry_version;
        self.specialization_key = new_specialization_key;
        self.init_signature = new_init_signature;
        self.variation_order_signature = new_variation_order;

        true // Shader changed (compiled or served from cache)
    }

    /// Get current path_features_enabled state
    pub fn path_features_enabled(&self) -> bool {
        self.path_features_enabled
    }

    /// Get current shader constants
    pub fn constants(&self) -> &ShaderConstants {
        &self.constants
    }

    /// Parse the assembled WGSL before the device sees it.
    ///
    /// `create_shader_module` reports a malformed module through wgpu's
    /// uncaptured-error handler, which **panics** by default — so a
    /// single downloaded variation with bad WGSL took the whole app
    /// down, and the message pointed at a line in generated source the
    /// user has never seen.
    ///
    /// Parsing here costs a few ms on a cache miss (builds are cached;
    /// this is not per frame) and turns that into a recoverable error
    /// with the culprits named. naga is wgpu's own front end, reached
    /// through its re-export, so this is the same parser that would
    /// have rejected it a moment later.
    ///
    /// Syntax only — a semantic error (calling a helper the flame did
    /// not splice in) still reaches the device. Catching those needs a
    /// full `naga::valid::Validator` pass; worth doing if downloaded
    /// shaders ever start failing that way, but syntax is what a
    /// hand-authored or truncated download gets wrong.
    ///
    /// **Desktop only.** The wasm build takes wgpu with only `webgpu` +
    /// `wgsl`, so `wgpu::naga` is configured out — on the web the
    /// *browser* is the WGSL front end, and bundling naga purely to
    /// pre-check would add a parser to a binary whose size budget the
    /// Endless Gallery work exists to protect. There, the browser
    /// reports through the same uncaptured-error handler installed in
    /// `gpu::device`, so a bad shader still costs the render rather than
    /// the session — it just arrives without the culprit named.
    #[cfg(not(target_arch = "wasm32"))]
    fn validate_wgsl(source: &str, label: &str) -> Result<(), String> {
        match wgpu::naga::front::wgsl::parse_str(source) {
            Ok(_) => Ok(()),
            Err(e) => {
                // Name the non-core variations in play: with the built-in
                // corpus compiling in CI, a downloaded one is the
                // overwhelmingly likely culprit and the user can act on
                // the name.
                let registry = crate::variations::global_registry();
                let suspects: Vec<&str> = registry
                    .all()
                    .iter()
                    .filter(|v| v.provenance.is_third_party())
                    .map(|v| v.name.as_str())
                    .collect();
                let blame = if suspects.is_empty() {
                    "No downloaded variations are registered, so this is a \
                     bug in the built-in shader assembly."
                        .to_string()
                } else {
                    format!(
                        "Downloaded variations currently registered: {}. \
                         Clearing the variation cache will restore rendering.",
                        suspects.join(", ")
                    )
                };
                Err(format!(
                    "Generated WGSL for `{label}` failed to parse.\n{}\n\n{blame}",
                    e.emit_to_string(source)
                ))
            }
        }
    }

    /// WASM stub — the browser validates. See the desktop version.
    #[cfg(target_arch = "wasm32")]
    fn validate_wgsl(_source: &str, _label: &str) -> Result<(), String> {
        Ok(())
    }

    /// Create a compute pipeline from shader source
    fn create_compute_pipeline(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        source: &str,
        label: &str,
    ) -> ComputePipeline {
        if let Err(msg) = Self::validate_wgsl(source, label) {
            // Logged rather than swallowed: the caller has no error
            // channel today, so handing the source on unchanged keeps
            // the existing behaviour (wgpu will reject it) while making
            // the REASON legible first. Previously the panic arrived
            // with no attribution at all.
            log::error!("{msg}");
        }
        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(label),
            source: ShaderSource::Wgsl(source.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some(&format!("{} Layout", label)),
            bind_group_layouts: &[Some(bind_group_layout)],
            immediate_size: 0,
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

    /// Create the bind group layout for the init compute shader.
    /// Single binding: the variation_params storage buffer with read_write access.
    pub fn create_init_bind_group_layout(device: &Device) -> BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Variation Init BGL"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }

    /// Build the init shader source and pipeline for the given flame, if any
    /// active variation has `wgsl_init`. Returns `(source, pipeline, pair_count)`
    /// where `pair_count` is the number of (xform, init-bearing-variation)
    /// pairs the dispatch will cover.
    /// Compile the init pipeline from an already-built init shader.
    ///
    /// The source is built by the caller (`builder.build_init_shader`)
    /// because it is part of the pipeline cache key — the caller needs it
    /// before deciding whether to compile at all.
    fn build_init_resources(
        device: &Device,
        init_bind_group_layout: &BindGroupLayout,
        source: Option<&str>,
    ) -> (Option<ComputePipeline>, u32) {
        let Some(source) = source else {
            return (None, 0);
        };

        // Count "case Nu: {" lines to determine pair count for dispatch sizing.
        let pair_count = source
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                t.starts_with("case ") && t.contains("u: {")
            })
            .count() as u32;

        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Variation Init"),
            source: ShaderSource::Wgsl(source.to_string().into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Variation Init Layout"),
            bind_group_layouts: &[Some(init_bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Variation Init"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("init_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        (Some(pipeline), pair_count)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod pipeline_lru_tests {
    //! Layer A of docs/projects/sticky-shader-compilation.md, tested
    //! through the real path: a FlameRenderer on a real device, configs
    //! loaded the way the app loads them. Skip cleanly without a GPU.

    use crate::config::FractalConfig;
    use crate::renderer::compute_kernel::FlameRenderer;

    fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .ok()?;
            let al = adapter.limits();
            let mut limits = wgpu::Limits::default();
            limits.max_storage_buffer_binding_size = al.max_storage_buffer_binding_size;
            limits.max_buffer_size = al.max_buffer_size;
            limits.max_storage_buffers_per_shader_stage = al.max_storage_buffers_per_shader_stage;
            adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("lru test"),
                    required_features: wgpu::Features::CLEAR_TEXTURE,
                    required_limits: limits,
                    memory_hints: wgpu::MemoryHints::Performance,
                    experimental_features: Default::default(),
                    trace: Default::default(),
                })
                .await
                .ok()
        })
    }

    /// One transform per name, weight 1, mild affine.
    fn config_with(names: &[&str]) -> FractalConfig {
        use crate::scene::transforms::{Flame, Transform};
        let mut flame = Flame::new();
        flame.transforms.clear();
        for name in names {
            let mut t = Transform::new();
            t.a = 0.5;
            t.e = 0.5;
            t.weight = 1.0;
            t.set_variation(name, 1.0);
            flame.transforms.push(t);
        }
        let mut config = FractalConfig::default();
        config.flame = flame;
        config
    }

    fn load(
        renderer: &mut FlameRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &FractalConfig,
    ) {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("lru test load"),
        });
        renderer.load_config(device, &mut enc, queue, config, &config.palette, 1, 0);
        queue.submit(Some(enc.finish()));
    }

    fn renderer_for(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &FractalConfig,
    ) -> FlameRenderer {
        let mut r = FlameRenderer::with_palette_size(
            device,
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
            64,
            64,
            &config.flame,
            config.palette_size,
        );
        // These tests exercise the pipeline LRU in isolation. Under the
        // (default-on) sticky superset the union GROWS across loads, so
        // A -> B -> A produces one merged map and the third load is an
        // early-out rather than the LRU revisit being tested. Layer B
        // has its own tests in renderer::sticky.
        r.set_sticky_enabled(false);
        r
    }

    /// The Layer A contract: revisiting a flame is a hit, not a rebuild.
    /// A, then B, then A again used to compile three times; the third is
    /// now served from the LRU.
    #[test]
    fn revisiting_a_flame_is_a_hit_not_a_rebuild() {
        let Some((device, queue)) = device() else {
            eprintln!("skipped: no GPU adapter");
            return;
        };
        let a = config_with(&["linear", "spherical"]);
        let b = config_with(&["swirl", "horseshoe"]);

        let mut r = renderer_for(&device, &queue, &a);
        load(&mut r, &device, &queue, &a); // no change: early-out
        let (rebuilds0, hits0, _) = r.shader_rebuild_stats();
        assert_eq!(
            (rebuilds0, hits0),
            (1, 0),
            "creation compiled once; loading A again is an early-out, not a hit"
        );

        load(&mut r, &device, &queue, &b);
        let (rebuilds1, hits1, _) = r.shader_rebuild_stats();
        assert_eq!((rebuilds1, hits1), (2, 0), "B is new: a rebuild");

        load(&mut r, &device, &queue, &a);
        let (rebuilds2, hits2, _) = r.shader_rebuild_stats();
        assert_eq!(
            (rebuilds2, hits2),
            (2, 1),
            "returning to A must be a hit, not a third compile"
        );
        r.destroy();
    }

    /// Two flames with IDENTICAL main WGSL but the init-bearing variation
    /// on a different transform must be different cache entries: the init
    /// shader bakes (xform, variation) pairs, and serving one flame's
    /// init pipeline for the other would write derived params at the
    /// wrong slot — zeroing the moved transform's params, the
    /// collapse-to-line bug the init signature exists to prevent. This is
    /// why the init source is part of the key.
    #[test]
    fn init_variation_placement_forks_the_cache_key() {
        let Some((device, queue)) = device() else {
            eprintln!("skipped: no GPU adapter");
            return;
        };
        use crate::scene::transforms::{Flame, Transform};

        // Same variation SET and the same union ORDER (linear first,
        // ripple second) on both, same transform count — identical main
        // WGSL. Only the ripple placement differs: t0 vs t1.
        let make = |ripple_on: usize| {
            let mut flame = Flame::new();
            flame.transforms.clear();
            for i in 0..2usize {
                let mut t = Transform::new();
                t.a = 0.5;
                t.e = 0.5;
                t.weight = 1.0;
                t.set_variation("linear", 1.0);
                if i == ripple_on {
                    t.set_variation("ripple", 0.5);
                }
                flame.transforms.push(t);
            }
            let mut config = FractalConfig::default();
            config.flame = flame;
            config
        };
        let x = make(0);
        let y = make(1);

        let mut r = renderer_for(&device, &queue, &x);
        load(&mut r, &device, &queue, &y);
        let (rebuilds, hits, _) = r.shader_rebuild_stats();
        assert_eq!(
            (rebuilds, hits),
            (2, 0),
            "moving the init variation to another transform must compile, not hit"
        );

        load(&mut r, &device, &queue, &x);
        let (rebuilds, hits, _) = r.shader_rebuild_stats();
        assert_eq!((rebuilds, hits), (2, 1), "returning to the first is a legitimate hit");
        r.destroy();
    }

    /// Beyond the cap the least-recently-used entry is gone and costs a
    /// rebuild again — while recent entries are still resident.
    #[test]
    fn eviction_beyond_the_cap_forces_a_rebuild() {
        let Some((device, queue)) = device() else {
            eprintln!("skipped: no GPU adapter");
            return;
        };
        let names = [
            "linear",
            "sinusoidal",
            "spherical",
            "swirl",
            "horseshoe",
            "polar",
            "disc",
            "heart",
            "diamond",
        ];
        assert_eq!(names.len(), super::PIPELINE_CACHE_CAP + 1);

        let first = config_with(&[names[0]]);
        let mut r = renderer_for(&device, &queue, &first);
        for name in &names[1..] {
            load(&mut r, &device, &queue, &config_with(&[name]));
        }
        let (rebuilds, hits, _) = r.shader_rebuild_stats();
        assert_eq!((rebuilds, hits), (9, 0), "nine distinct flames, nine compiles");

        // names[0] was pushed out by the ninth entry.
        load(&mut r, &device, &queue, &first);
        let (rebuilds, hits, _) = r.shader_rebuild_stats();
        assert_eq!(
            (rebuilds, hits),
            (10, 0),
            "the evicted flame must compile again, not hit a ghost entry"
        );

        // ...and a recent entry is still resident.
        load(&mut r, &device, &queue, &config_with(&[names[8]]));
        let (rebuilds, hits, _) = r.shader_rebuild_stats();
        assert_eq!((rebuilds, hits), (10, 1), "a recent entry is still a hit");
        r.destroy();
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod validate_tests {
    use super::ShaderCache;

    /// Malformed WGSL must be caught with a legible message before the
    /// device sees it.
    ///
    /// Previously `create_shader_module` handed it straight to wgpu,
    /// whose default handler panics — so one downloaded variation with
    /// bad shader code killed the app, and the message pointed at a
    /// line of generated source the user had never seen.
    #[test]
    fn malformed_wgsl_is_rejected_with_a_readable_message() {
        let bad = "fn broken( -> vec2<f32> { return; }";
        let err = ShaderCache::validate_wgsl(bad, "Test Shader")
            .expect_err("malformed WGSL must not validate");
        assert!(err.contains("Test Shader"), "names the shader: {err}");
        // naga's rendered diagnostic, not just a type name.
        assert!(err.contains("broken") || err.contains("expected"), "{err}");
    }

    /// Valid WGSL passes — a validator that rejects working shaders
    /// would be worse than the panic it replaces.
    #[test]
    fn valid_wgsl_passes() {
        let good = "fn f(p: vec2<f32>) -> vec2<f32> { return p * 2.0; }";
        assert!(ShaderCache::validate_wgsl(good, "Test Shader").is_ok());
    }

    /// With no downloaded variations registered, the message must not
    /// blame one — it says the assembly itself is at fault.
    #[test]
    fn the_message_does_not_invent_a_culprit() {
        let err = ShaderCache::validate_wgsl("fn broken( ->", "T").unwrap_err();
        assert!(
            err.contains("built-in shader assembly"),
            "with nothing downloaded, blame belongs at home: {err}"
        );
    }
}

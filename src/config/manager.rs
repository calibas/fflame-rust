/// Configuration manager - central authority for all config changes
///
/// # Overview
/// ConfigManager is the single source of truth for all configuration state.
/// All UI controls should use ConfigManager methods instead of directly modifying config.
///
/// # Key Features
/// - Single gateway for all parameter updates
/// - Delta-based undo/redo with lazy throttling
/// - Selective updates based on change type
/// - Human-readable change descriptions
/// - Centralized update action tracking (what needs GPU updates)
///
/// # Usage Patterns
///
/// ## Reading Config Values
/// ```rust
/// // Get immutable reference to active config (includes live preview during drag)
/// let config = config_manager.active_config();
/// let zoom = config.zoom;
/// let exposure = config.exposure;
/// ```
///
/// ## Setting Config Values (Immediate Undo)
/// ```rust
/// // For discrete controls (buttons, checkboxes, dropdowns)
/// config_manager.update_param(ConfigPath::TonemapMode, ToneMapMode::Linear.into(), false)?;
/// config_manager.update_param(ConfigPath::ColorMode, ColorMode::Palette.into(), false)?;
/// ```
///
/// ## Setting Config Values (Lazy Undo)
/// ```rust
/// // For continuous controls (sliders, drag handles) - throttles undo capture
/// config_manager.update_param(ConfigPath::Zoom, 2.5.into(), true)?;
/// config_manager.update_param(ConfigPath::Exposure, 1.8.into(), true)?;
///
/// // Force commit preview when drag ends (optional, auto-commits on next non-lazy update)
/// if !mouse_down && config_manager.is_in_preview_mode() {
///     config_manager.force_commit_preview(&ConfigPath::Zoom)?;
/// }
/// ```
///
/// ## Handling GPU Updates
/// ```rust
/// // After all UI updates each frame, check what needs updating
/// let actions = config_manager.get_pending_actions();
///
/// // Execute needed updates
/// if actions.update_view {
///     renderer.update_view(...);
///     renderer.reset(...);  // if actions.reset_accumulation
/// }
/// if actions.update_flame {
///     renderer.update_flame(...);
/// }
/// if actions.update_palette {
///     renderer.update_palette(...);
/// }
/// if actions.update_tone_curve {
///     renderer.update_curve_lut(...);
/// }
///
/// // Clear actions after handling
/// config_manager.clear_pending_actions();
/// ```
///
/// ## Requesting Actions Without Config Changes
/// ```rust
/// // Reset button - doesn't change config, just requests buffer clear
/// config_manager.request_reset();
///
/// // Next get_pending_actions() will include reset_accumulation=true
/// ```
///
/// ## Undo/Redo
/// ```rust
/// if config_manager.can_undo() {
///     config_manager.undo()?;
/// }
/// if config_manager.can_redo() {
///     config_manager.redo()?;
/// }
/// ```

use super::delta::{
    AffineParam, ConfigChange, ConfigDelta, ConfigPath, ConfigValue, UpdateType,
};
use super::fractal_config::FractalConfig;
use std::time::Duration;
use web_time::Instant;

/// Actions needed after configuration changes
///
/// ConfigManager tracks changes and provides this struct to tell the App layer
/// exactly what GPU/renderer updates are needed. This centralizes all "what needs
/// updating" logic in one place.
#[derive(Debug, Clone, Default)]
pub struct UpdateAction {
    /// Reset accumulation buffers and restart rendering from scratch
    /// Needed when: flame changes, view changes, color mode changes, palette changes (non-preview)
    pub reset_accumulation: bool,

    /// Update flame parameters on GPU (transforms, variations, weights)
    /// Needed when: flame changes, or during preview mode (live updates)
    pub update_flame: bool,

    /// Update palette texture on GPU
    /// Needed when: palette changes (including preview mode)
    pub update_palette: bool,

    /// Update tone curve LUT texture
    /// Needed when: tone curve changes
    pub update_tone_curve: bool,

    /// Update view transform on GPU (zoom, pan, rotation, camera)
    /// Needed when: any view parameter changes
    pub update_view: bool,

    /// Rebuild shader pipeline (variation changes require recompilation)
    /// Needed when: active variations change
    pub rebuild_shader: bool,

}

impl UpdateAction {
    /// No actions needed
    pub fn none() -> Self {
        Self::default()
    }

    /// Create from UpdateType (used when building from delta changes)
    pub fn from_update_type(update_type: UpdateType, in_preview_mode: bool) -> Self {
        match update_type {
            UpdateType::None => Self::none(),

            UpdateType::ViewOnly => Self {
                update_view: true,
                reset_accumulation: !in_preview_mode, // Preview uses overwrite mode
                ..Default::default()
            },

            UpdateType::ToneMappingOnly => Self {
                update_tone_curve: true,
                // No reset - tone mapping is post-processing only
                ..Default::default()
            },

            UpdateType::ColorOnly => Self {
                update_palette: true,
                reset_accumulation: !in_preview_mode, // Preview uses overwrite mode
                ..Default::default()
            },

            UpdateType::IterationReset => Self {
                update_flame: true,
                reset_accumulation: !in_preview_mode, // Preview uses overwrite mode
                rebuild_shader: false, // TODO: detect variation changes
                ..Default::default()
            },
        }
    }

    /// Merge two actions (take union of all flags)
    pub fn merge(&mut self, other: &UpdateAction) {
        self.reset_accumulation |= other.reset_accumulation;
        self.update_flame |= other.update_flame;
        self.update_palette |= other.update_palette;
        self.update_tone_curve |= other.update_tone_curve;
        self.update_view |= other.update_view;
        self.rebuild_shader |= other.rebuild_shader;
    }
}

/// Central manager for configuration state and undo/redo
pub struct ConfigManager {
    /// Current configuration (last captured state)
    current: FractalConfig,

    /// Preview configuration (live state during lazy updates)
    /// When Some: shows live preview, deltas computed from current
    /// When None: not in preview mode
    preview: Option<FractalConfig>,

    /// Undo stack (deltas, not full configs)
    undo_stack: Vec<ConfigChange>,

    /// Redo stack
    redo_stack: Vec<ConfigChange>,

    /// Maximum undo history
    max_undo_depth: usize,

    /// Last time we created a lazy undo point
    last_lazy_undo: Option<Instant>,

    /// Lazy undo throttle duration (500ms)
    lazy_throttle: Duration,

    /// Pending actions accumulated since last get_pending_actions() call
    /// This tracks what GPU updates are needed based on recent changes
    pending_actions: UpdateAction,

    /// Whether the current preview requires overwrite rendering
    /// True for iteration-affecting parameters (view, flame, color)
    /// False for post-processing parameters (tone mapping)
    preview_needs_overwrite: bool,
}

impl ConfigManager {
    pub fn new(config: FractalConfig) -> Self {
        Self {
            current: config,
            preview: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_undo_depth: 500,  // ~5MB max memory (500 states × ~10KB each)
            last_lazy_undo: None,
            lazy_throttle: Duration::from_millis(500),
            pending_actions: UpdateAction::none(),
            preview_needs_overwrite: false,
        }
    }

    /// Apply a single parameter change
    pub fn update_param(
        &mut self,
        path: ConfigPath,
        new_value: ConfigValue,
        lazy: bool,
    ) -> Result<UpdateType, ConfigError> {
        if lazy {
            // Lazy mode: Update preview, capture on throttle

            // Determine if this parameter type needs overwrite rendering
            let update_type = path.update_type();
            let needs_overwrite = matches!(update_type,
                UpdateType::ViewOnly | UpdateType::IterationReset | UpdateType::ColorOnly
            );

            // Create preview if it doesn't exist (first update in drag sequence)
            if self.preview.is_none() {
                self.preview = Some(self.current.clone());
                self.preview_needs_overwrite = needs_overwrite;
                log::trace!("Created preview from current (overwrite={})", needs_overwrite);
            }

            // Get preview value (will exist now)
            let preview_value = self.get_value(&path)?;

            // Check if actually changed from preview
            if preview_value.approx_eq(&new_value) {
                return Ok(UpdateType::None);
            }

            // Update preview with new value
            self.set_value_in_preview(&path, new_value.clone())?;
            log::trace!("Updated preview: {} = {}", path, new_value);

            // Check if we should capture this change
            let should_capture = self.should_capture_lazy_undo();

            if should_capture {
                // Capture delta from current → preview
                let old_value_in_current = {
                    let temp_preview = self.preview.take();
                    let val = self.get_value(&path)?;
                    self.preview = temp_preview;
                    val
                };

                log::debug!("Lazy capture: {} = {} → {}", path, old_value_in_current, new_value);

                let delta = ConfigDelta::new(path.clone(), old_value_in_current, new_value.clone());
                let change = ConfigChange::single(delta);
                let update_type = change.update_type();

                self.push_undo(change);

                // Commit preview to current (clone to keep preview active - prevents blink)
                self.current = self.preview.clone().unwrap();
                log::debug!("  -> Committed preview to current, undo stack len: {}", self.undo_stack.len());

                // Record action for GPU updates
                self.record_action(update_type);

                return Ok(update_type);
            }

            // No capture yet, but still record action for GPU updates during preview
            let update_type = path.update_type();
            self.record_action(update_type);
            Ok(update_type)

        } else {
            // Non-lazy mode: Update current directly and capture immediately

            let old_value = self.get_value(&path)?;

            // Check if actually changed
            if old_value.approx_eq(&new_value) {
                return Ok(UpdateType::None);
            }

            log::debug!("Immediate capture: {} = {} → {}", path, old_value, new_value);

            // Create delta and capture
            let delta = ConfigDelta::new(path.clone(), old_value, new_value.clone());
            let change = ConfigChange::single(delta);
            let update_type = change.update_type();

            self.push_undo(change);

            // Apply change to current
            self.set_value(&path, new_value)?;

            // Record action for GPU updates
            self.record_action(update_type);

            Ok(update_type)
        }
    }

    /// Apply a batch of changes (single undo point)
    pub fn update_batch(
        &mut self,
        changes: Vec<(ConfigPath, ConfigValue)>,
        description: String,
        lazy: bool,
    ) -> Result<UpdateType, ConfigError> {
        if lazy {
            // Lazy mode: Update preview, capture on throttle (same logic as update_param)

            // Determine if this batch needs overwrite rendering (check first path's update type)
            let needs_overwrite = if !changes.is_empty() {
                let first_update_type = changes[0].0.update_type();
                matches!(first_update_type,
                    UpdateType::ViewOnly | UpdateType::IterationReset | UpdateType::ColorOnly
                )
            } else {
                false
            };

            // Create preview if it doesn't exist (first update in drag sequence)
            if self.preview.is_none() {
                self.preview = Some(self.current.clone());
                self.preview_needs_overwrite = needs_overwrite;
                log::trace!("Created preview from current (batch, overwrite={})", needs_overwrite);
            }

            // Create deltas from preview to new values
            let mut deltas = Vec::new();
            for (path, new_value) in changes {
                let preview_value = self.get_value(&path)?; // Gets from preview
                if !preview_value.approx_eq(&new_value) {
                    deltas.push(ConfigDelta::new(path.clone(), preview_value, new_value.clone()));
                    // Update preview with new value
                    self.set_value_in_preview(&path, new_value)?;
                }
            }

            if deltas.is_empty() {
                return Ok(UpdateType::None);
            }

            let change = ConfigChange::batch(deltas, description);
            let update_type = change.update_type();

            // Check if we should capture this change
            let should_capture = self.should_capture_lazy_undo();

            if should_capture {
                // Capture delta from current → preview
                let deltas_from_current: Vec<ConfigDelta> = change.deltas.iter().map(|delta| {
                    // Get old value from current (not preview)
                    let old_val_in_current = {
                        let temp_preview = self.preview.take();
                        let val = self.get_value(&delta.path).unwrap();
                        self.preview = temp_preview;
                        val
                    };
                    ConfigDelta::new(delta.path.clone(), old_val_in_current, delta.new_value.clone())
                }).collect();

                let change_from_current = ConfigChange::batch(deltas_from_current, change.description.clone());
                self.push_undo(change_from_current);

                // Commit preview to current (clone to keep preview active - prevents blink)
                self.current = self.preview.clone().unwrap();
                log::debug!("  -> Committed batch preview to current, undo stack len: {}", self.undo_stack.len());

                // Record action for GPU updates
                self.record_action(update_type);

                return Ok(update_type);
            }

            // No capture yet, but still record action for GPU updates during preview
            self.record_action(update_type);
            Ok(update_type)

        } else {
            // Non-lazy mode: Update current directly and capture immediately
            let mut deltas = Vec::new();

            // Create deltas for all changes
            for (path, new_value) in changes {
                let old_value = self.get_value(&path)?;
                if !old_value.approx_eq(&new_value) {
                    deltas.push(ConfigDelta::new(path, old_value, new_value));
                }
            }

            if deltas.is_empty() {
                return Ok(UpdateType::None);
            }

            let change = ConfigChange::batch(deltas, description);

            // Capture undo point
            self.push_undo(change.clone());

            // Apply all changes
            for delta in &change.deltas {
                self.set_value(&delta.path, delta.new_value.clone())?;
            }

            let update_type = change.update_type();

            // Record action for GPU updates
            self.record_action(update_type);

            Ok(update_type)
        }
    }

    /// Undo last change
    pub fn undo(&mut self) -> Result<UpdateType, ConfigError> {
        // Clear preview mode before undo (if active)
        self.preview = None;
        self.preview_needs_overwrite = false;

        let change = self
            .undo_stack
            .pop()
            .ok_or(ConfigError::EmptyUndoStack)?;

        log::debug!("Undo: {}", change.description);

        // Check if this is a snapshot-based undo
        if let Some(snapshot) = &change.snapshot {
            log::debug!("  Restoring full config snapshot");
            self.current = (**snapshot).clone();
            self.redo_stack.push(change);
            return Ok(UpdateType::IterationReset); // Full config change
        }

        // Delta-based undo (original behavior)
        for delta in &change.deltas {
            log::debug!("  Original delta: {} → {}", delta.old_value, delta.new_value);
        }

        let inverted = change.invert();

        // Apply inverted deltas
        for delta in &inverted.deltas {
            log::debug!("  Applying: {} ← {}", delta.path, delta.new_value);
            self.set_value(&delta.path, delta.new_value.clone())?;
        }

        // Push to redo stack
        self.redo_stack.push(change);

        log::debug!("  Undo stack now: {} items, Redo stack now: {} items",
            self.undo_stack.len(), self.redo_stack.len());

        let update_type = inverted.update_type();

        // Record action for GPU updates
        self.record_action(update_type);

        Ok(update_type)
    }

    /// Redo last undone change
    pub fn redo(&mut self) -> Result<UpdateType, ConfigError> {
        // Clear preview mode before redo (if active)
        self.preview = None;
        self.preview_needs_overwrite = false;

        let change = self
            .redo_stack
            .pop()
            .ok_or(ConfigError::EmptyRedoStack)?;

        log::debug!("Redo: {}", change.description);

        // Check if this is a snapshot-based redo
        if let Some(snapshot) = &change.snapshot {
            log::debug!("  Restoring full config snapshot");
            self.current = (**snapshot).clone();
            self.undo_stack.push(change);
            return Ok(UpdateType::IterationReset); // Full config change
        }

        // Delta-based redo (original behavior)
        for delta in &change.deltas {
            log::debug!("  Delta: {} → {}", delta.old_value, delta.new_value);
            log::debug!("  Applying: {} → {}", delta.path, delta.new_value);
            self.set_value(&delta.path, delta.new_value.clone())?;
        }

        // Push back to undo stack (WITHOUT clearing redo stack!)
        self.undo_stack.push(change.clone());

        // Trim undo stack if needed
        if self.undo_stack.len() > self.max_undo_depth {
            self.undo_stack.remove(0);
        }

        log::debug!("  Undo stack now: {} items, Redo stack now: {} items",
            self.undo_stack.len(), self.redo_stack.len());

        let update_type = change.update_type();

        // Record action for GPU updates
        self.record_action(update_type);

        Ok(update_type)
    }

    /// Check if we should capture lazy undo (throttling logic)
    fn should_capture_lazy_undo(&mut self) -> bool {
        let now = Instant::now();

        match self.last_lazy_undo {
            None => {
                // First lazy change - always capture
                self.last_lazy_undo = Some(now);
                true
            }
            Some(last) => {
                let elapsed = now.duration_since(last);
                if elapsed >= self.lazy_throttle {
                    // Enough time passed - capture
                    self.last_lazy_undo = Some(now);
                    true
                } else {
                    // Too soon - skip
                    false
                }
            }
        }
    }

    /// Reset lazy undo timer (call on drag end to ensure final state is captured)
    pub fn reset_lazy_undo(&mut self) {
        self.last_lazy_undo = None;
    }

    /// Check if ConfigManager is in preview mode (during lazy drag)
    ///
    /// This returns true only if:
    /// 1. A preview is active (lazy drag in progress), AND
    /// 2. The parameter being previewed requires overwrite rendering
    ///
    /// Tone mapping parameters use lazy undo but NOT preview mode since
    /// they're post-processing and don't need overwrite rendering.
    pub fn is_in_preview_mode(&self) -> bool {
        self.preview.is_some() && self.preview_needs_overwrite
    }

    /// Push change to undo stack, maintaining depth limit
    fn push_undo(&mut self, change: ConfigChange) {
        log::debug!("PUSH_UNDO: {}", change.description);
        for delta in &change.deltas {
            log::debug!("  Delta: {} → {}", delta.old_value, delta.new_value);
        }

        self.undo_stack.push(change);

        // Trim if over limit
        if self.undo_stack.len() > self.max_undo_depth {
            self.undo_stack.remove(0);
        }

        // Clear redo stack (new change invalidates redo)
        let redo_cleared = self.redo_stack.len();
        self.redo_stack.clear();

        log::debug!("  Undo stack now: {} items, Redo stack cleared ({} items removed)",
            self.undo_stack.len(), redo_cleared);
    }

    /// Extract value from any FractalConfig by path (helper for undo/redo)
    fn get_value_from_config(
        config: &FractalConfig,
        path: &ConfigPath,
    ) -> Result<ConfigValue, ConfigError> {
        match path {
            // View
            ConfigPath::Zoom => Ok(config.zoom.into()),
            ConfigPath::PanX => Ok(config.pan_x.into()),
            ConfigPath::PanY => Ok(config.pan_y.into()),
            ConfigPath::Rotation => Ok(config.rotation.into()),
            ConfigPath::CameraRotationX => Ok(config.camera_rotation_x.into()),
            ConfigPath::CameraRotationY => Ok(config.camera_rotation_y.into()),
            ConfigPath::CameraZ => Ok(config.camera_z.into()),

            // Tone mapping
            ConfigPath::Exposure => Ok(config.exposure.into()),
            ConfigPath::Gamma => Ok(config.gamma.into()),
            ConfigPath::GammaThreshold => Ok(config.gamma_threshold.into()),
            ConfigPath::Brightness => Ok(config.brightness.into()),
            ConfigPath::Vibrancy => Ok(config.vibrancy.into()),
            ConfigPath::Saturation => Ok(config.saturation.into()),
            ConfigPath::HueShift => Ok(config.hue_shift.into()),
            ConfigPath::ValueScale => Ok(config.value_scale.into()),
            ConfigPath::DensityScale => Ok(config.density_scale.into()),
            ConfigPath::TonemapMode => Ok(config.tonemap_mode.into()),
            ConfigPath::TonemapCurve => Ok(config.tonemap_curve.clone().into()),
            ConfigPath::UseCurve => Ok(config.use_curve.into()),

            // Color
            ConfigPath::ColorMode => Ok(config.color_mode.into()),
            ConfigPath::PaletteIndex => Ok((config.palette_index as u32).into()),
            ConfigPath::Palette => {
                // Return embedded palette if it exists, otherwise None
                match &config.palette {
                    Some(pal) => Ok(ConfigValue::Palette(pal.clone())),
                    None => Err(ConfigError::TypeMismatch), // No embedded palette
                }
            }
            ConfigPath::PaletteRotation => Ok(config.palette_rotation.into()),
            ConfigPath::SpeedFactor => Ok(config.speed_factor.into()),
            ConfigPath::BackgroundColor => Ok(config.background_color.into()),

            // Rendering settings
            ConfigPath::HistogramColorScale => Ok(config.histogram_color_scale.into()),
            ConfigPath::LowDensitySmoothing => Ok(config.low_density_smoothing.into()),
            ConfigPath::DensityCompressionStrength => {
                Ok(config.density_compression_strength.into())
            }
            ConfigPath::BlendFactor => Ok(config.blend_factor.into()),
            ConfigPath::UseDynamicBlend => Ok(config.use_dynamic_blend.into()),
            ConfigPath::TargetIterationsPerPixel => {
                Ok(config.target_iterations_per_pixel.into())
            }
            ConfigPath::IterationsPerThread => Ok(config.iterations_per_thread.into()),
            ConfigPath::SpeedMultiplier => Ok(config.speed_multiplier.into()),
            ConfigPath::MaxIterations => Ok(config.max_iterations.into()),
            ConfigPath::DeterministicRng => Ok(config.deterministic_rng.into()),

            // Transforms
            ConfigPath::TransformCount => {
                Ok((config.flame.transforms.len() as u32).into())
            }
            ConfigPath::TransformWeight { index } => {
                let xform = config
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.weight.into())
            }
            ConfigPath::TransformColor { index } => {
                let xform = config
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.color.into())
            }
            ConfigPath::TransformColorSpeed { index } => {
                let xform = config
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.color_speed.into())
            }
            ConfigPath::TransformOpacity { index } => {
                let xform = config
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.opacity.into())
            }
            ConfigPath::TransformAffine { index, param } => {
                let xform = config
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let value = match param {
                    AffineParam::A => xform.a,
                    AffineParam::B => xform.b,
                    AffineParam::C => xform.c,
                    AffineParam::D => xform.d,
                    AffineParam::E => xform.e,
                    AffineParam::F => xform.f,
                    AffineParam::G => xform.g,
                };
                Ok(value.into())
            }
            ConfigPath::TransformVariation { index, variation } => {
                let xform = config
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let weight = xform.variations.get(variation).copied().unwrap_or(0.0);
                Ok(weight.into())
            }
            ConfigPath::TransformVariationParam {
                index,
                variation,
                param,
            } => {
                let xform = config
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;

                // Use same default lookup as UI to ensure undo history shows correct values
                let value = xform.get_variation_param_or_default(
                    variation,
                    param,
                    &crate::variations::global_registry()
                );
                Ok(value.into())
            }

            // Final Transform
            ConfigPath::FinalTransformEnabled => {
                Ok(config.flame.final_transform.is_some().into())
            }
            ConfigPath::FinalTransformAffine { param } => {
                let final_xform = config
                    .flame
                    .final_transform
                    .as_ref()
                    .ok_or(ConfigError::InvalidIndex)?;
                let value = match param {
                    AffineParam::A => final_xform.a,
                    AffineParam::B => final_xform.b,
                    AffineParam::C => final_xform.c,
                    AffineParam::D => final_xform.d,
                    AffineParam::E => final_xform.e,
                    AffineParam::F => final_xform.f,
                    AffineParam::G => final_xform.g,
                };
                Ok(value.into())
            }
            ConfigPath::FinalTransformColor => {
                let final_xform = config
                    .flame
                    .final_transform
                    .as_ref()
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(final_xform.color.into())
            }
            ConfigPath::FinalTransformColorSpeed => {
                let final_xform = config
                    .flame
                    .final_transform
                    .as_ref()
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(final_xform.color_speed.into())
            }
            ConfigPath::FinalTransformVariation { variation } => {
                let final_xform = config
                    .flame
                    .final_transform
                    .as_ref()
                    .ok_or(ConfigError::InvalidIndex)?;
                let weight = final_xform.variations.get(variation).copied().unwrap_or(0.0);
                Ok(weight.into())
            }
            ConfigPath::FinalTransformVariationParam { variation, param } => {
                let final_xform = config
                    .flame
                    .final_transform
                    .as_ref()
                    .ok_or(ConfigError::InvalidIndex)?;
                let value = final_xform.get_variation_param_or_default(
                    variation,
                    param,
                    &crate::variations::global_registry()
                );
                Ok(value.into())
            }

            // Flame
            ConfigPath::RenderMode => Ok(config.flame.render_mode.into()),
            ConfigPath::ProjectionType => Ok(config.flame.projection.into()),
        }
    }

    /// Get value from config by path
    /// Returns preview value if in preview mode, otherwise current value
    pub fn get_value(&self, path: &ConfigPath) -> Result<ConfigValue, ConfigError> {
        // Use preview if available, otherwise current
        let config = self.preview.as_ref().unwrap_or(&self.current);
        Self::get_value_from_config(config, path)
    }

    /// Set value in config by path
    fn set_value(&mut self, path: &ConfigPath, value: ConfigValue) -> Result<(), ConfigError> {
        match path {
            // View
            ConfigPath::Zoom => {
                self.current.zoom = value.try_into()?;
            }
            ConfigPath::PanX => {
                self.current.pan_x = value.try_into()?;
            }
            ConfigPath::PanY => {
                self.current.pan_y = value.try_into()?;
            }
            ConfigPath::Rotation => {
                self.current.rotation = value.try_into()?;
            }
            ConfigPath::CameraRotationX => {
                self.current.camera_rotation_x = value.try_into()?;
            }
            ConfigPath::CameraRotationY => {
                self.current.camera_rotation_y = value.try_into()?;
            }
            ConfigPath::CameraZ => {
                self.current.camera_z = value.try_into()?;
            }

            // Tone mapping
            ConfigPath::Exposure => {
                self.current.exposure = value.try_into()?;
            }
            ConfigPath::Gamma => {
                self.current.gamma = value.try_into()?;
            }
            ConfigPath::GammaThreshold => {
                self.current.gamma_threshold = value.try_into()?;
            }
            ConfigPath::Brightness => {
                self.current.brightness = value.try_into()?;
            }
            ConfigPath::Vibrancy => {
                self.current.vibrancy = value.try_into()?;
            }
            ConfigPath::Saturation => {
                self.current.saturation = value.try_into()?;
            }
            ConfigPath::HueShift => {
                self.current.hue_shift = value.try_into()?;
            }
            ConfigPath::ValueScale => {
                self.current.value_scale = value.try_into()?;
            }
            ConfigPath::DensityScale => {
                self.current.density_scale = value.try_into()?;
            }
            ConfigPath::TonemapMode => {
                self.current.tonemap_mode = value.try_into()?;
            }
            ConfigPath::TonemapCurve => {
                self.current.tonemap_curve = value.try_into()?;
            }
            ConfigPath::UseCurve => {
                self.current.use_curve = value.try_into()?;
            }

            // Color
            ConfigPath::ColorMode => {
                self.current.color_mode = value.try_into()?;
            }
            ConfigPath::PaletteIndex => {
                let idx: u32 = value.try_into()?;
                self.current.palette_index = idx as usize;
                // Clear embedded palette when selecting from library
                self.current.palette = None;
            }
            ConfigPath::Palette => {
                if let ConfigValue::Palette(mut palette) = value {
                    // Safety: Never allow built-in flag to be true in config.palette
                    // Built-ins should only exist in the library
                    if palette.built_in {
                        log::warn!("Attempted to set built-in palette in config.palette - forcing built_in=false");
                        palette.built_in = false;
                    }

                    // Update embedded palette data
                    self.current.palette = Some(palette);
                }
            }
            ConfigPath::PaletteRotation => {
                self.current.palette_rotation = value.try_into()?;
            }
            ConfigPath::SpeedFactor => {
                self.current.speed_factor = value.try_into()?;
            }
            ConfigPath::BackgroundColor => {
                self.current.background_color = value.try_into()?;
            }

            // Rendering settings
            ConfigPath::HistogramColorScale => {
                self.current.histogram_color_scale = value.try_into()?;
            }
            ConfigPath::LowDensitySmoothing => {
                self.current.low_density_smoothing = value.try_into()?;
            }
            ConfigPath::DensityCompressionStrength => {
                self.current.density_compression_strength = value.try_into()?;
            }
            ConfigPath::BlendFactor => {
                self.current.blend_factor = value.try_into()?;
            }
            ConfigPath::UseDynamicBlend => {
                self.current.use_dynamic_blend = value.try_into()?;
            }
            ConfigPath::TargetIterationsPerPixel => {
                self.current.target_iterations_per_pixel = value.try_into()?;
            }
            ConfigPath::IterationsPerThread => {
                self.current.iterations_per_thread = value.try_into()?;
            }
            ConfigPath::SpeedMultiplier => {
                self.current.speed_multiplier = value.try_into()?;
            }
            ConfigPath::MaxIterations => {
                self.current.max_iterations = value.try_into()?;
            }
            ConfigPath::DeterministicRng => {
                self.current.deterministic_rng = value.try_into()?;
            }

            // Transforms
            ConfigPath::TransformCount => {
                // Can't directly set count - must add/remove transforms
                return Err(ConfigError::ReadOnlyParameter);
            }
            ConfigPath::TransformWeight { index } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                xform.weight = value.try_into()?;
            }
            ConfigPath::TransformColor { index } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                xform.color = value.try_into()?;
            }
            ConfigPath::TransformColorSpeed { index } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                xform.color_speed = value.try_into()?;
            }
            ConfigPath::TransformOpacity { index } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                xform.opacity = value.try_into()?;
            }
            ConfigPath::TransformAffine { index, param } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                match param {
                    AffineParam::A => xform.a = new_value,
                    AffineParam::B => xform.b = new_value,
                    AffineParam::C => xform.c = new_value,
                    AffineParam::D => xform.d = new_value,
                    AffineParam::E => xform.e = new_value,
                    AffineParam::F => xform.f = new_value,
                    AffineParam::G => xform.g = new_value,
                }
            }
            ConfigPath::TransformVariation { index, variation } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let weight: f32 = value.try_into()?;
                if weight == 0.0 {
                    xform.variations.remove(variation);
                } else {
                    xform.variations.insert(variation.clone(), weight);
                }
            }
            ConfigPath::TransformVariationParam {
                index,
                variation,
                param,
            } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                let key = format!("{}.{}", variation, param);
                xform.variation_params.insert(key, new_value);
            }

            // Final Transform
            ConfigPath::FinalTransformEnabled => {
                let enabled: bool = value.try_into()?;
                if enabled && self.current.flame.final_transform.is_none() {
                    // Create new final transform with identity affine
                    self.current.flame.final_transform = Some(crate::scene::transforms::Transform::new());
                } else if !enabled {
                    // Remove final transform
                    self.current.flame.final_transform = None;
                }
            }
            ConfigPath::FinalTransformAffine { param } => {
                let final_xform = self
                    .current
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                match param {
                    AffineParam::A => final_xform.a = new_value,
                    AffineParam::B => final_xform.b = new_value,
                    AffineParam::C => final_xform.c = new_value,
                    AffineParam::D => final_xform.d = new_value,
                    AffineParam::E => final_xform.e = new_value,
                    AffineParam::F => final_xform.f = new_value,
                    AffineParam::G => final_xform.g = new_value,
                }
            }
            ConfigPath::FinalTransformColor => {
                let final_xform = self
                    .current
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                final_xform.color = value.try_into()?;
            }
            ConfigPath::FinalTransformColorSpeed => {
                let final_xform = self
                    .current
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                final_xform.color_speed = value.try_into()?;
            }
            ConfigPath::FinalTransformVariation { variation } => {
                let final_xform = self
                    .current
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                let weight: f32 = value.try_into()?;
                if weight > 0.0 {
                    final_xform.variations.insert(variation.clone(), weight);
                } else {
                    final_xform.variations.remove(variation);
                }
            }
            ConfigPath::FinalTransformVariationParam { variation, param } => {
                let final_xform = self
                    .current
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                let key = format!("{}.{}", variation, param);
                final_xform.variation_params.insert(key, new_value);
            }

            // Flame
            ConfigPath::RenderMode => {
                self.current.flame.render_mode = value.try_into()?;
            }
            ConfigPath::ProjectionType => {
                self.current.flame.projection = value.try_into()?;
            }
        }

        Ok(())
    }

    /// Set value in preview config by path
    /// Panics if preview doesn't exist (caller must ensure preview is created first)
    fn set_value_in_preview(&mut self, path: &ConfigPath, value: ConfigValue) -> Result<(), ConfigError> {
        let preview = self.preview.as_mut().expect("set_value_in_preview called but preview is None");

        match path {
            // View
            ConfigPath::Zoom => {
                preview.zoom = value.try_into()?;
            }
            ConfigPath::PanX => {
                preview.pan_x = value.try_into()?;
            }
            ConfigPath::PanY => {
                preview.pan_y = value.try_into()?;
            }
            ConfigPath::Rotation => {
                preview.rotation = value.try_into()?;
            }
            ConfigPath::CameraRotationX => {
                preview.camera_rotation_x = value.try_into()?;
            }
            ConfigPath::CameraRotationY => {
                preview.camera_rotation_y = value.try_into()?;
            }
            ConfigPath::CameraZ => {
                preview.camera_z = value.try_into()?;
            }

            // Tone mapping
            ConfigPath::Exposure => {
                preview.exposure = value.try_into()?;
            }
            ConfigPath::Gamma => {
                preview.gamma = value.try_into()?;
            }
            ConfigPath::GammaThreshold => {
                preview.gamma_threshold = value.try_into()?;
            }
            ConfigPath::Brightness => {
                preview.brightness = value.try_into()?;
            }
            ConfigPath::Vibrancy => {
                preview.vibrancy = value.try_into()?;
            }
            ConfigPath::Saturation => {
                preview.saturation = value.try_into()?;
            }
            ConfigPath::HueShift => {
                preview.hue_shift = value.try_into()?;
            }
            ConfigPath::ValueScale => {
                preview.value_scale = value.try_into()?;
            }
            ConfigPath::DensityScale => {
                preview.density_scale = value.try_into()?;
            }
            ConfigPath::TonemapMode => {
                preview.tonemap_mode = value.try_into()?;
            }
            ConfigPath::TonemapCurve => {
                preview.tonemap_curve = value.try_into()?;
            }
            ConfigPath::UseCurve => {
                preview.use_curve = value.try_into()?;
            }

            // Color
            ConfigPath::ColorMode => {
                preview.color_mode = value.try_into()?;
            }
            ConfigPath::PaletteIndex => {
                let idx: u32 = value.try_into()?;
                preview.palette_index = idx as usize;
                // Clear embedded palette when selecting from library
                preview.palette = None;
            }
            ConfigPath::Palette => {
                if let ConfigValue::Palette(mut palette) = value {
                    // Safety: Never allow built-in flag in preview either
                    if palette.built_in {
                        palette.built_in = false;
                    }
                    preview.palette = Some(palette);
                }
            }
            ConfigPath::PaletteRotation => {
                preview.palette_rotation = value.try_into()?;
            }
            ConfigPath::SpeedFactor => {
                preview.speed_factor = value.try_into()?;
            }
            ConfigPath::BackgroundColor => {
                preview.background_color = value.try_into()?;
            }

            // Rendering settings
            ConfigPath::HistogramColorScale => {
                preview.histogram_color_scale = value.try_into()?;
            }
            ConfigPath::LowDensitySmoothing => {
                preview.low_density_smoothing = value.try_into()?;
            }
            ConfigPath::DensityCompressionStrength => {
                preview.density_compression_strength = value.try_into()?;
            }
            ConfigPath::BlendFactor => {
                preview.blend_factor = value.try_into()?;
            }
            ConfigPath::UseDynamicBlend => {
                preview.use_dynamic_blend = value.try_into()?;
            }
            ConfigPath::TargetIterationsPerPixel => {
                preview.target_iterations_per_pixel = value.try_into()?;
            }
            ConfigPath::IterationsPerThread => {
                preview.iterations_per_thread = value.try_into()?;
            }
            ConfigPath::SpeedMultiplier => {
                preview.speed_multiplier = value.try_into()?;
            }
            ConfigPath::MaxIterations => {
                preview.max_iterations = value.try_into()?;
            }
            ConfigPath::DeterministicRng => {
                preview.deterministic_rng = value.try_into()?;
            }

            // Transforms
            ConfigPath::TransformCount => {
                // Can't directly set count - must add/remove transforms
                return Err(ConfigError::ReadOnlyParameter);
            }
            ConfigPath::TransformWeight { index } => {
                let xform = preview
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                xform.weight = value.try_into()?;
            }
            ConfigPath::TransformColor { index } => {
                let xform = preview
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                xform.color = value.try_into()?;
            }
            ConfigPath::TransformColorSpeed { index } => {
                let xform = preview
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                xform.color_speed = value.try_into()?;
            }
            ConfigPath::TransformOpacity { index } => {
                let xform = preview
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                xform.opacity = value.try_into()?;
            }
            ConfigPath::TransformAffine { index, param } => {
                let xform = preview
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                match param {
                    AffineParam::A => xform.a = new_value,
                    AffineParam::B => xform.b = new_value,
                    AffineParam::C => xform.c = new_value,
                    AffineParam::D => xform.d = new_value,
                    AffineParam::E => xform.e = new_value,
                    AffineParam::F => xform.f = new_value,
                    AffineParam::G => xform.g = new_value,
                }
            }
            ConfigPath::TransformVariation { index, variation } => {
                let xform = preview
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let weight: f32 = value.try_into()?;
                if weight == 0.0 {
                    xform.variations.remove(variation);
                } else {
                    xform.variations.insert(variation.clone(), weight);
                }
            }
            ConfigPath::TransformVariationParam {
                index,
                variation,
                param,
            } => {
                let xform = preview
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                let key = format!("{}.{}", variation, param);
                xform.variation_params.insert(key, new_value);
            }

            // Final Transform
            ConfigPath::FinalTransformEnabled => {
                let enabled: bool = value.try_into()?;
                if enabled && preview.flame.final_transform.is_none() {
                    // Create new final transform with identity affine
                    preview.flame.final_transform = Some(crate::scene::transforms::Transform::new());
                } else if !enabled {
                    // Remove final transform
                    preview.flame.final_transform = None;
                }
            }
            ConfigPath::FinalTransformAffine { param } => {
                let final_xform = preview
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                match param {
                    AffineParam::A => final_xform.a = new_value,
                    AffineParam::B => final_xform.b = new_value,
                    AffineParam::C => final_xform.c = new_value,
                    AffineParam::D => final_xform.d = new_value,
                    AffineParam::E => final_xform.e = new_value,
                    AffineParam::F => final_xform.f = new_value,
                    AffineParam::G => final_xform.g = new_value,
                }
            }
            ConfigPath::FinalTransformColor => {
                let final_xform = preview
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                final_xform.color = value.try_into()?;
            }
            ConfigPath::FinalTransformColorSpeed => {
                let final_xform = preview
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                final_xform.color_speed = value.try_into()?;
            }
            ConfigPath::FinalTransformVariation { variation } => {
                let final_xform = preview
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                let weight: f32 = value.try_into()?;
                if weight > 0.0 {
                    final_xform.variations.insert(variation.clone(), weight);
                } else {
                    final_xform.variations.remove(variation);
                }
            }
            ConfigPath::FinalTransformVariationParam { variation, param } => {
                let final_xform = preview
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                let key = format!("{}.{}", variation, param);
                final_xform.variation_params.insert(key, new_value);
            }

            // Flame
            ConfigPath::RenderMode => {
                preview.flame.render_mode = value.try_into()?;
            }
            ConfigPath::ProjectionType => {
                preview.flame.projection = value.try_into()?;
            }
        }

        Ok(())
    }

    /// Force commit preview to current (call on drag end)
    /// Creates final undo entry if preview differs from current
    /// This ensures changes are captured even if drag ended before throttle fired
    pub fn force_commit_preview(&mut self, path: &ConfigPath) -> Result<UpdateType, ConfigError> {
        if let Some(preview) = self.preview.take() {
            self.preview_needs_overwrite = false;  // Clear overwrite flag
            log::debug!("Force commit for path: {:?}", path);

            // Check if preview actually differs from current
            let current_value = Self::get_value_from_config(&self.current, path)?;
            let preview_value = Self::get_value_from_config(&preview, path)?;

            log::debug!("Force commit: Comparing {} (current) vs {} (preview)", current_value, preview_value);

            if current_value != preview_value {
                // Create final undo entry (preview differs from last capture)
                let delta = ConfigDelta::new(path.clone(), current_value.clone(), preview_value.clone());
                let change = ConfigChange::single(delta);
                self.push_undo(change);
                log::debug!("Force commit: Created final undo entry {} → {}", current_value, preview_value);
            } else {
                log::debug!("Force commit: No changes to capture (preview == current)");
            }

            // Commit preview to current
            self.current = preview;

            // Return update type based on path
            let update_type = path.update_type();

            // Record action for GPU updates
            self.record_action(update_type);

            Ok(update_type)
        } else {
            Ok(UpdateType::None)
        }
    }

    /// Get current config (read-only)
    /// Returns last captured/committed state, NOT live preview
    /// Use active_config() if you want to see live values during drag
    pub fn config(&self) -> &FractalConfig {
        &self.current
    }

    /// Get active config (read-only)
    /// Returns preview if in preview mode, otherwise current
    /// Use this to read live values for rendering
    pub fn active_config(&self) -> &FractalConfig {
        self.preview.as_ref().unwrap_or(&self.current)
    }

    /// Get mutable config (for operations that need it - use sparingly!)
    pub fn config_mut(&mut self) -> &mut FractalConfig {
        &mut self.current
    }

    /// Load a complete config (e.g., preset, imported file)
    /// This creates two undo entries:
    /// 1. Snapshot of old state (for undo)
    /// 2. Snapshot of new state (for redo after undo)
    /// Use this for atomic operations like loading presets
    pub fn load_config(&mut self, new_config: FractalConfig, description: String) -> Result<(), ConfigError> {
        // Clear any preview state
        self.preview = None;
        self.preview_needs_overwrite = false;

        // Create snapshot of current state (for undo)
        let old_snapshot = ConfigChange::snapshot(
            self.current.clone(),
            format!("Before: {}", description),
        );
        self.push_undo(old_snapshot);

        // Replace current config
        self.current = new_config.clone();

        // Create snapshot of new state (for redo after undo)
        let new_snapshot = ConfigChange::snapshot(
            new_config,
            description,
        );
        self.push_undo(new_snapshot);

        // Record full config import action
        let mut action = UpdateAction::none();
        action.update_flame = true;
        action.update_view = true;
        action.update_palette = true;
        action.update_tone_curve = true;
        action.reset_accumulation = true;
        self.pending_actions.merge(&action);

        Ok(())
    }

    /// Get undo stack (for displaying in undo window)
    pub fn undo_history(&self) -> &[ConfigChange] {
        &self.undo_stack
    }

    /// Get redo stack
    pub fn redo_history(&self) -> &[ConfigChange] {
        &self.redo_stack
    }

    /// Check if can undo
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if can redo
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Get pending GPU update actions based on recent changes
    ///
    /// This method analyzes all changes since the last call and returns
    /// a consolidated UpdateAction telling the App layer what needs updating.
    ///
    /// Call this once per frame after all UI updates, execute the actions,
    /// then call clear_pending_actions().
    pub fn get_pending_actions(&self) -> UpdateAction {
        self.pending_actions.clone()
    }

    /// Clear pending actions after executing them
    ///
    /// Call this after handling the UpdateAction from get_pending_actions()
    pub fn clear_pending_actions(&mut self) {
        self.pending_actions = UpdateAction::none();
    }

    /// Request an explicit accumulation reset (e.g., from Reset button)
    ///
    /// This sets the reset_accumulation flag without modifying any config state.
    /// Useful for UI actions that need to clear buffers without changing parameters.
    pub fn request_reset(&mut self) {
        self.pending_actions.reset_accumulation = true;
    }

    /// Record an action for later retrieval
    ///
    /// Called internally when config changes occur
    fn record_action(&mut self, update_type: UpdateType) {
        let in_preview = self.is_in_preview_mode();
        let action = UpdateAction::from_update_type(update_type, in_preview);
        self.pending_actions.merge(&action);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    TypeMismatch,
    InvalidIndex,
    EmptyUndoStack,
    EmptyRedoStack,
    ReadOnlyParameter,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ConfigError::TypeMismatch => write!(f, "Config value type mismatch"),
            ConfigError::InvalidIndex => write!(f, "Invalid transform index"),
            ConfigError::EmptyUndoStack => write!(f, "Nothing to undo"),
            ConfigError::EmptyRedoStack => write!(f, "Nothing to redo"),
            ConfigError::ReadOnlyParameter => write!(f, "Parameter is read-only"),
        }
    }
}

impl std::error::Error for ConfigError {}

// TryFrom implementations for extracting values from ConfigValue
impl TryFrom<ConfigValue> for f32 {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::Float(f) => Ok(f),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for i32 {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::Int(i) => Ok(i),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for u32 {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::UInt(u) => Ok(u),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for u64 {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::UInt64(u) => Ok(u),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for bool {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::Bool(b) => Ok(b),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for [f32; 3] {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::ColorRgb(c) => Ok(c),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

use crate::scene::tonemap::{ToneMapMode, ToneCurve};
use crate::scene::palette::ColorMode;
use crate::scene::transforms::{RenderMode, ProjectionType};

impl TryFrom<ConfigValue> for ToneMapMode {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::ToneMapMode(m) => Ok(m),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for ColorMode {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::ColorMode(m) => Ok(m),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for RenderMode {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::RenderMode(m) => Ok(m),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for ProjectionType {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::ProjectionType(p) => Ok(p),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for ToneCurve {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::ToneCurve(c) => Ok(c),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::fractal_config::FractalConfig;

    #[test]
    fn test_get_set_exposure() {
        let config = FractalConfig::default();
        let mut manager = ConfigManager::new(config);

        // Get initial value
        let value = manager.get_value(&ConfigPath::Exposure).unwrap();
        assert!(value.approx_eq(&ConfigValue::Float(1.0)));

        // Set new value
        manager
            .set_value(&ConfigPath::Exposure, 2.0.into())
            .unwrap();
        assert_eq!(manager.current.exposure, 2.0);
    }

    #[test]
    fn test_update_param_lazy() {
        let config = FractalConfig::default();
        let mut manager = ConfigManager::new(config);

        // First lazy update - should capture
        let update1 = manager
            .update_param(ConfigPath::Exposure, 2.0.into(), true)
            .unwrap();
        assert_eq!(update1, UpdateType::ToneMappingOnly);
        assert_eq!(manager.undo_stack.len(), 1);

        // Immediate second update - should NOT capture (throttled)
        let update2 = manager
            .update_param(ConfigPath::Exposure, 3.0.into(), true)
            .unwrap();
        assert_eq!(update2, UpdateType::ToneMappingOnly);
        assert_eq!(manager.undo_stack.len(), 1); // Still 1!
    }

    #[test]
    fn test_undo_redo_sequence() {
        // Test a longer sequence to catch redo bugs
        let config = FractalConfig::default();
        let mut manager = ConfigManager::new(config);

        // Start at exposure = 1.0
        assert!(manager.config().exposure == 1.0);

        // Change to 2.0
        manager.update_param(ConfigPath::Exposure, 2.0.into(), false).unwrap();
        assert!(manager.config().exposure == 2.0);

        // Change to 3.0
        manager.update_param(ConfigPath::Exposure, 3.0.into(), false).unwrap();
        assert!(manager.config().exposure == 3.0);

        // Change to 4.0
        manager.update_param(ConfigPath::Exposure, 4.0.into(), false).unwrap();
        assert!(manager.config().exposure == 4.0);

        // Undo: should go back to 3.0
        manager.undo().unwrap();
        assert!(manager.config().exposure == 3.0, "After 1st undo, expected 3.0, got {}", manager.config().exposure);

        // Undo: should go back to 2.0
        manager.undo().unwrap();
        assert!(manager.config().exposure == 2.0, "After 2nd undo, expected 2.0, got {}", manager.config().exposure);

        // Redo: should go back to 3.0
        manager.redo().unwrap();
        assert!(manager.config().exposure == 3.0, "After 1st redo, expected 3.0, got {}", manager.config().exposure);

        // Redo: should go back to 4.0
        manager.redo().unwrap();
        assert!(manager.config().exposure == 4.0, "After 2nd redo, expected 4.0, got {}", manager.config().exposure);
    }

    #[test]
    fn test_undo_redo() {
        let config = FractalConfig::default();
        let mut manager = ConfigManager::new(config);

        // Make change
        manager
            .update_param(ConfigPath::Exposure, 2.0.into(), false)
            .unwrap();
        assert_eq!(manager.current.exposure, 2.0);

        // Undo
        manager.undo().unwrap();
        assert_eq!(manager.current.exposure, 1.0);

        // Redo
        manager.redo().unwrap();
        assert_eq!(manager.current.exposure, 2.0);
    }

    #[test]
    fn test_batch_update() {
        let config = FractalConfig::default();
        let mut manager = ConfigManager::new(config);

        let changes = vec![
            (ConfigPath::Zoom, ConfigValue::Float(2.0)),
            (ConfigPath::PanX, ConfigValue::Float(1.0)),
            (ConfigPath::PanY, ConfigValue::Float(-1.0)),
        ];

        let update = manager
            .update_batch(changes, "Reset View".to_string(), false)
            .unwrap();

        assert_eq!(update, UpdateType::ViewOnly);
        assert_eq!(manager.undo_stack.len(), 1);
        assert_eq!(manager.undo_stack[0].deltas.len(), 3);
        assert_eq!(manager.undo_stack[0].description, "Reset View");
    }
}

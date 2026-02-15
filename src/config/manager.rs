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
/// ```rust,ignore
/// // Get immutable reference to active config (includes live preview during drag)
/// let config = config_manager.active_config();
/// let zoom = config.zoom;
/// let exposure = config.exposure;
/// ```
///
/// ## Setting Config Values (Immediate Undo)
/// ```rust,ignore
/// // For discrete controls (buttons, checkboxes, dropdowns)
/// config_manager.update_param(ConfigPath::TonemapMode, ToneMapMode::Linear.into(), false)?;
/// config_manager.update_param(ConfigPath::ColorMode, ColorMode::Palette.into(), false)?;
/// ```
///
/// ## Setting Config Values (Lazy Undo)
/// ```rust,ignore
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
/// ```rust,ignore
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
/// ```rust,ignore
/// // Reset button - doesn't change config, just requests buffer clear
/// config_manager.request_reset();
///
/// // Next get_pending_actions() will include reset_accumulation=true
/// ```
///
/// ## Undo/Redo
/// ```rust,ignore
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

/// Maximum duration for coalescing - total span from first to last change
const MAX_COALESCE_SPAN: Duration = Duration::from_millis(3000);

/// Inactivity threshold - pausing longer than this creates a new undo point
const COALESCE_INACTIVITY_THRESHOLD: Duration = Duration::from_millis(500);

/// Check if a config path supports undo point coalescing
/// By default, all paths support coalescing (enabled for continuous controls)
/// Only paths in this exclusion list will create immediate undo points
fn supports_coalescing(path: &ConfigPath) -> bool {
    match path {
        // Add paths here that should NOT coalesce (discrete actions):
        // ConfigPath::RenderMode => false,
        // ConfigPath::ProjectionType => false,
        // ConfigPath::ColorMode => false,
        _ => true,  // Default: all parameters support coalescing
    }
}

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
    pub fn from_update_type(update_type: UpdateType) -> Self {
        match update_type {
            UpdateType::None => Self::none(),

            UpdateType::ViewOnly => Self {
                update_view: true,
                reset_accumulation: false, // Never reset - use overwrite mode for smooth updates
                ..Default::default()
            },

            UpdateType::ToneMappingOnly => Self {
                update_tone_curve: true,
                // No reset - tone mapping is post-processing only
                ..Default::default()
            },

            UpdateType::ColorOnly => Self {
                update_palette: true,
                reset_accumulation: false, // Never reset - use overwrite mode for smooth updates
                ..Default::default()
            },

            UpdateType::IterationReset => Self {
                update_flame: true,
                reset_accumulation: false, // Don't reset - use overwrite mode for smooth transition
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
    // ===== Fractal State (undo/redo enabled) =====
    /// Current configuration (last captured state)
    current: FractalConfig,

    /// Preview configuration (live state during lazy updates)
    /// When Some: shows live preview, deltas computed from current
    /// When None: not in preview mode
    preview: Option<FractalConfig>,

    /// Full history (unified undo/redo timeline)
    /// Position points to "current state" in history
    /// Items before position = past states (can undo)
    /// Items at/after position = future states (can redo)
    history: Vec<ConfigChange>,

    /// Current position in history (0 = initial state, history.len() = head)
    /// Invariant: 0 <= position <= history.len()
    position: usize,

    /// Maximum undo history
    max_undo_depth: usize,

    // ===== System State (no undo/redo, immediate disk save) =====
    /// System settings - device-specific preferences
    /// Changes to these settings:
    /// - Do NOT create undo deltas
    /// - Save to disk immediately
    /// - Still return UpdateType for GPU synchronization
    system_settings: crate::storage::SystemSettings,

    /// Pending actions accumulated since last get_pending_actions() call
    /// This tracks what GPU updates are needed based on recent changes
    pending_actions: UpdateAction,

    /// Active modify session (for transform editing with snapshot on commit)
    /// When Some: transform edits don't create history entries
    /// When None: normal operation
    modify_session: Option<ModifySession>,

    /// Animation mode flag
    /// When true: all update_param calls are silent (no undo entries)
    /// This allows users to tweak settings during animation playback
    /// without corrupting the undo history
    animation_mode: bool,
}

/// Session state for transform modification (triangle editor, etc.)
struct ModifySession {
    /// Index of transform being modified
    transform_index: usize,
    /// Initial state captured at session start
    initial_transform: crate::scene::transforms::Transform,
}

impl ConfigManager {
    pub fn new(config: FractalConfig) -> Self {
        // Load system settings from disk (or use defaults)
        let system_settings = crate::storage::SystemSettings::load();

        Self {
            current: config,
            preview: None,
            history: Vec::new(),
            position: 0,  // Start at beginning (no history yet)
            max_undo_depth: 500,  // ~5MB max memory (500 states × ~10KB each)
            system_settings,
            pending_actions: UpdateAction::none(),
            modify_session: None,
            animation_mode: false,
        }
    }

    /// Set animation mode
    ///
    /// When true, all update_param calls become silent (no undo entries).
    /// This allows tweaking settings during animation without corrupting undo history.
    pub fn set_animation_mode(&mut self, enabled: bool) {
        self.animation_mode = enabled;
    }

    /// Check if animation mode is active
    pub fn is_animation_mode(&self) -> bool {
        self.animation_mode
    }

    /// Apply a single parameter change
    ///
    /// All changes apply immediately and create history entries.
    /// Coalescing (in push_undo) automatically merges rapid changes to same parameter.
    ///
    /// Note: When animation_mode is true, delegates to update_param_silent
    /// to avoid creating undo entries during animation playback.
    pub fn update_param(
        &mut self,
        path: ConfigPath,
        new_value: ConfigValue,
    ) -> Result<UpdateType, ConfigError> {
        // Animation mode: use silent updates (no undo entries)
        if self.animation_mode {
            return self.update_param_silent(path, new_value);
        }

        // Special case: modify session (skip history, commit on session end)
        if self.modify_session.is_some() {
            self.set_value(&path, new_value)?;
            let update_type = path.update_type();
            self.record_action(update_type);
            return Ok(update_type);
        }

        // Normal mode: update current and capture
        let old_value = self.get_value(&path)?;

        if old_value.approx_eq(&new_value) {
            return Ok(UpdateType::None);
        }

        let delta = ConfigDelta::new(path.clone(), old_value, new_value.clone());
        let change = ConfigChange::single(delta);
        let update_type = change.update_type();

        self.push_undo(change);  // Coalescing happens here automatically
        self.set_value(&path, new_value)?;
        self.record_action(update_type);

        Ok(update_type)
    }

    /// Apply a batch of changes (single undo point)
    ///
    /// Batch changes always create immediate history entries (no coalescing).
    /// Use this for grouped parameter changes like triangle editor affine updates.
    pub fn update_batch(
        &mut self,
        changes: Vec<(ConfigPath, ConfigValue)>,
        description: String,
    ) -> Result<UpdateType, ConfigError> {
        // Special case: modify session (skip history, commit on session end)
        if self.modify_session.is_some() {
            for (path, value) in changes {
                self.set_value(&path, value)?;
            }
            let update_type = UpdateType::IterationReset;  // Assume worst case for modify session
            self.record_action(update_type);
            return Ok(update_type);
        }

        // Normal mode: create deltas and capture
        let mut deltas = Vec::new();
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
        let update_type = change.update_type();

        self.push_undo(change.clone());  // Batch changes skip coalescing

        for delta in &change.deltas {
            self.set_value(&delta.path, delta.new_value.clone())?;
            // Record each delta's update type separately so all necessary actions are merged
            // (e.g., batch with ColorMode + TonemapCurve needs both update_palette AND update_tone_curve)
            self.record_action(delta.path.update_type());
        }

        Ok(update_type)
    }

    /// Update a system setting (device-specific preference)
    ///
    /// System settings are NOT tracked for undo/redo (they're device preferences, not artistic choices).
    /// However, they DO return UpdateType so the GPU knows what needs updating.
    /// Changes are saved to disk immediately.
    ///
    /// # Example
    /// ```rust,ignore
    /// // Change iterations per thread (triggers IterationReset)
    /// config_manager.update_system_setting(
    ///     ConfigPath::SystemIterationsPerThread,
    ///     256.into()
    /// )?;
    /// ```
    pub fn update_system_setting(
        &mut self,
        path: ConfigPath,
        new_value: ConfigValue,
    ) -> Result<UpdateType, ConfigError> {
        // Verify this is a System* path
        match &path {
            ConfigPath::SystemIterationsPerThread => {
                let value: u32 = new_value.try_into()?;
                self.system_settings.iterations_per_thread = value;
            }
            ConfigPath::SystemBurnIn => {
                let value: u32 = new_value.try_into()?;
                self.system_settings.burn_in = value;
            }
            ConfigPath::SystemVsyncEnabled => {
                let value: bool = new_value.try_into()?;
                self.system_settings.vsync_enabled = value;
            }
            ConfigPath::SystemTargetFps => {
                let value: f32 = new_value.try_into()?;
                self.system_settings.target_fps = value;
            }
            ConfigPath::SystemExportWidth => {
                let value: u32 = new_value.try_into()?;
                self.system_settings.default_export_width = value;
            }
            ConfigPath::SystemExportHeight => {
                let value: u32 = new_value.try_into()?;
                self.system_settings.default_export_height = value;
            }
            ConfigPath::SystemLanguage => {
                // Extract String from ConfigValue manually
                let value = match new_value {
                    ConfigValue::String(s) => s,
                    _ => return Err(ConfigError::TypeMismatch),
                };
                self.system_settings.language = value;
            }
            ConfigPath::SystemShowHelpOnStartup => {
                let value: bool = new_value.try_into()?;
                self.system_settings.show_help_on_startup = value;
            }
            _ => {
                return Err(ConfigError::InvalidPath(
                    "Not a system setting path. Use update_param() for FractalConfig changes.".to_string()
                ));
            }
        }

        // Save to disk immediately (system settings persist across sessions)
        self.system_settings.save()
            .map_err(|e| ConfigError::InvalidPath(format!("Failed to save system settings: {}", e)))?;

        // Determine what GPU updates are needed and record them
        let update_type = path.update_type();
        self.record_action(update_type);

        Ok(update_type)
    }

    /// Update a parameter silently (no undo point created)
    ///
    /// Used by the animation system during playback to update parameters
    /// without creating undo history entries. This allows animations to
    /// run smoothly without polluting the undo stack.
    ///
    /// # Arguments
    /// * `path` - The ConfigPath identifying which parameter to update
    /// * `new_value` - The new value to set
    ///
    /// # Returns
    /// * `Ok(UpdateType)` - The type of GPU update needed
    /// * `Err(ConfigError)` - If the path is invalid or value type doesn't match
    ///
    /// # Example
    /// ```rust,ignore
    /// // Animation controller updates zoom during playback
    /// config_manager.update_param_silent(ConfigPath::Zoom, 2.5.into())?;
    /// ```
    pub fn update_param_silent(
        &mut self,
        path: ConfigPath,
        new_value: ConfigValue,
    ) -> Result<UpdateType, ConfigError> {
        // Skip if value hasn't changed
        let old_value = self.get_value(&path)?;
        if old_value.approx_eq(&new_value) {
            return Ok(UpdateType::None);
        }

        // Apply value directly (no history, no preview)
        self.set_value(&path, new_value)?;

        // Determine update type and record action
        let update_type = path.update_type();
        self.record_action(update_type);

        Ok(update_type)
    }

    /// Undo last change
    pub fn undo(&mut self) -> Result<UpdateType, ConfigError> {
        // Clear preview mode before undo (if active)
        self.preview = None;

        if self.position == 0 {
            return Err(ConfigError::EmptyUndoStack);
        }

        // Move position back
        self.position -= 1;

        let change = &self.history[self.position];
        log::debug!("Undo: {} (position now: {})", change.description, self.position);

        // Check if this is a snapshot-based undo
        if let Some(snapshot) = &change.snapshot {
            match snapshot {
                crate::config::SnapshotData::FullConfig { before, .. } => {
                    log::debug!("  Restoring full config snapshot (before)");
                    self.current = (**before).clone();
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::AddTransform { index, xaos_before, .. } => {
                    log::debug!("  Undoing add transform at index {}", index);
                    if *index < self.current.flame.transforms.len() {
                        self.current.flame.transforms.remove(*index);
                        // Restore xaos matrix to pre-add state (not incremental delete)
                        self.current.flame.xaos = xaos_before.clone();
                    }
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::DeleteTransform { index, transform, xaos_before } => {
                    log::debug!("  Undoing delete transform (re-insert at index {})", index);
                    if *index <= self.current.flame.transforms.len() {
                        self.current.flame.transforms.insert(*index, transform.clone());
                        // Restore xaos matrix to pre-delete state (not incremental add)
                        self.current.flame.xaos = xaos_before.clone();
                    }
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::ModifyTransform { index, before, .. } => {
                    log::debug!("  Undoing modify transform (restore before state at index {})", index);
                    if *index < self.current.flame.transforms.len() {
                        self.current.flame.transforms[*index] = before.clone();
                    }
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::AddColorEffect { index, .. } => {
                    log::debug!("  Undoing add color effect at index {}", index);
                    if *index < self.current.color_effects.len() {
                        self.current.color_effects.remove(*index);
                    }
                    return Ok(UpdateType::ToneMappingOnly);
                }

                crate::config::SnapshotData::DeleteColorEffect { index, effect } => {
                    log::debug!("  Undoing delete color effect (re-insert at index {})", index);
                    if *index <= self.current.color_effects.len() {
                        self.current.color_effects.insert(*index, effect.clone());
                    }
                    return Ok(UpdateType::ToneMappingOnly);
                }

                crate::config::SnapshotData::AddDensityEffect { index, .. } => {
                    log::debug!("  Undoing add density effect at index {}", index);
                    if *index < self.current.density_effects.len() {
                        self.current.density_effects.remove(*index);
                    }
                    return Ok(UpdateType::ToneMappingOnly);
                }

                crate::config::SnapshotData::DeleteDensityEffect { index, effect } => {
                    log::debug!("  Undoing delete density effect (re-insert at index {})", index);
                    if *index <= self.current.density_effects.len() {
                        self.current.density_effects.insert(*index, effect.clone());
                    }
                    return Ok(UpdateType::ToneMappingOnly);
                }

                crate::config::SnapshotData::MoveColorEffect { from_index, to_index } => {
                    log::debug!("  Undoing move color effect {} -> {} (moving back)", from_index, to_index);
                    // Undo: move from to_index back to from_index
                    if *to_index < self.current.color_effects.len() {
                        let effect = self.current.color_effects.remove(*to_index);
                        let insert_at = (*from_index).min(self.current.color_effects.len());
                        self.current.color_effects.insert(insert_at, effect);
                    }
                    return Ok(UpdateType::ToneMappingOnly);
                }

                crate::config::SnapshotData::MoveDensityEffect { from_index, to_index } => {
                    log::debug!("  Undoing move density effect {} -> {} (moving back)", from_index, to_index);
                    // Undo: move from to_index back to from_index
                    if *to_index < self.current.density_effects.len() {
                        let effect = self.current.density_effects.remove(*to_index);
                        let insert_at = (*from_index).min(self.current.density_effects.len());
                        self.current.density_effects.insert(insert_at, effect);
                    }
                    return Ok(UpdateType::ToneMappingOnly);
                }
            }
        }

        // Delta-based undo - apply inverted deltas
        for delta in &change.deltas {
            log::debug!("  Original delta: {} → {}", delta.old_value, delta.new_value);
        }

        let inverted = change.invert();

        // Apply inverted deltas
        for delta in &inverted.deltas {
            log::debug!("  Applying: {} ← {}", delta.path, delta.new_value);
            self.set_value(&delta.path, delta.new_value.clone())?;
        }

        log::debug!("  History: {} items, Position: {}",
            self.history.len(), self.position);

        let update_type = inverted.update_type();

        // Record action for GPU updates
        self.record_action(update_type);

        Ok(update_type)
    }

    /// Redo last undone change
    pub fn redo(&mut self) -> Result<UpdateType, ConfigError> {
        // Clear preview mode before redo (if active)
        self.preview = None;

        if self.position >= self.history.len() {
            return Err(ConfigError::EmptyRedoStack);
        }

        // Clone the change to avoid borrow issues
        let change = self.history[self.position].clone();
        log::debug!("Redo: {}", change.description);

        // Check if this is a snapshot-based redo
        if let Some(snapshot) = &change.snapshot {
            match snapshot {
                crate::config::SnapshotData::FullConfig { after, .. } => {
                    log::debug!("  Restoring full config snapshot (after)");
                    self.current = (**after).clone();
                    self.position += 1;
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::AddTransform { index, transform, .. } => {
                    log::debug!("  Redoing add transform at index {}", index);
                    if *index <= self.current.flame.transforms.len() {
                        self.current.flame.on_transform_added(*index);
                        self.current.flame.transforms.insert(*index, transform.clone());
                    }
                    self.position += 1;
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::DeleteTransform { index, .. } => {
                    log::debug!("  Redoing delete transform (remove at index {})", index);
                    if *index < self.current.flame.transforms.len() {
                        self.current.flame.transforms.remove(*index);
                        self.current.flame.on_transform_deleted(*index);
                    }
                    self.position += 1;
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::ModifyTransform { index, after, .. } => {
                    log::debug!("  Redoing modify transform (restore after state at index {})", index);
                    if *index < self.current.flame.transforms.len() {
                        self.current.flame.transforms[*index] = after.clone();
                    }
                    self.position += 1;
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::AddColorEffect { index, effect } => {
                    log::debug!("  Redoing add color effect at index {}", index);
                    if *index <= self.current.color_effects.len() {
                        self.current.color_effects.insert(*index, effect.clone());
                    }
                    self.position += 1;
                    return Ok(UpdateType::ToneMappingOnly);
                }

                crate::config::SnapshotData::DeleteColorEffect { index, .. } => {
                    log::debug!("  Redoing delete color effect (remove at index {})", index);
                    if *index < self.current.color_effects.len() {
                        self.current.color_effects.remove(*index);
                    }
                    self.position += 1;
                    return Ok(UpdateType::ToneMappingOnly);
                }

                crate::config::SnapshotData::AddDensityEffect { index, effect } => {
                    log::debug!("  Redoing add density effect at index {}", index);
                    if *index <= self.current.density_effects.len() {
                        self.current.density_effects.insert(*index, effect.clone());
                    }
                    self.position += 1;
                    return Ok(UpdateType::ToneMappingOnly);
                }

                crate::config::SnapshotData::DeleteDensityEffect { index, .. } => {
                    log::debug!("  Redoing delete density effect (remove at index {})", index);
                    if *index < self.current.density_effects.len() {
                        self.current.density_effects.remove(*index);
                    }
                    self.position += 1;
                    return Ok(UpdateType::ToneMappingOnly);
                }

                crate::config::SnapshotData::MoveColorEffect { from_index, to_index } => {
                    log::debug!("  Redoing move color effect {} -> {}", from_index, to_index);
                    if *from_index < self.current.color_effects.len() {
                        let effect = self.current.color_effects.remove(*from_index);
                        let insert_at = (*to_index).min(self.current.color_effects.len());
                        self.current.color_effects.insert(insert_at, effect);
                    }
                    self.position += 1;
                    return Ok(UpdateType::ToneMappingOnly);
                }

                crate::config::SnapshotData::MoveDensityEffect { from_index, to_index } => {
                    log::debug!("  Redoing move density effect {} -> {}", from_index, to_index);
                    if *from_index < self.current.density_effects.len() {
                        let effect = self.current.density_effects.remove(*from_index);
                        let insert_at = (*to_index).min(self.current.density_effects.len());
                        self.current.density_effects.insert(insert_at, effect);
                    }
                    self.position += 1;
                    return Ok(UpdateType::ToneMappingOnly);
                }
            }
        }

        // Delta-based redo - apply deltas forward
        for delta in &change.deltas {
            log::debug!("  Delta: {} → {}", delta.old_value, delta.new_value);
            log::debug!("  Applying: {} → {}", delta.path, delta.new_value);
            self.set_value(&delta.path, delta.new_value.clone())?;
        }

        // Move position forward
        self.position += 1;

        log::debug!("  History: {} items, Position: {}",
            self.history.len(), self.position);

        let update_type = change.update_type();

        // Record action for GPU updates
        self.record_action(update_type);

        Ok(update_type)
    }

    /// Push change to history, maintaining depth limit and truncating future
    fn push_undo(&mut self, change: ConfigChange) {
        log::debug!("PUSH_UNDO: {}", change.description);
        for delta in &change.deltas {
            log::debug!("  Delta: {} → {}", delta.old_value, delta.new_value);
        }

        // Truncate future history if not at head (clear redo)
        let future_cleared = if self.position < self.history.len() {
            let count = self.history.len() - self.position;
            self.history.truncate(self.position);
            count
        } else {
            0
        };

        // Check if we should coalesce with the last change
        let should_coalesce = self.should_coalesce(&change);

        if should_coalesce {
            // Replace last change instead of adding new one
            let last_idx = self.history.len() - 1;
            log::debug!("  COALESCING with previous change (within {}ms inactivity, {}ms total span)",
                COALESCE_INACTIVITY_THRESHOLD.as_millis(), MAX_COALESCE_SPAN.as_millis());

            // Update the last change's new_value, description, and last_update_time
            // Keep the original old_value and timestamp from the first change in the sequence
            for (i, new_delta) in change.deltas.iter().enumerate() {
                if let Some(old_delta) = self.history[last_idx].deltas.get_mut(i) {
                    old_delta.new_value = new_delta.new_value.clone();
                    // NOTE: We do NOT update timestamp - it preserves when the sequence started
                }
            }

            // Update description to reflect final state
            self.history[last_idx].description = change.description.clone();

            // Update last_update_time to track when the most recent change occurred
            self.history[last_idx].last_update_time = change.deltas
                .first()
                .map(|d| d.timestamp)
                .unwrap_or_else(web_time::Instant::now);

            log::debug!("  History: {} items (coalesced), Position: {}",
                self.history.len(), self.position);
        } else {
            // Add new change at current position
            self.history.push(change);
            self.position = self.history.len();

            // Trim if over limit (remove oldest)
            if self.history.len() > self.max_undo_depth {
                self.history.remove(0);
                self.position = self.position.saturating_sub(1);
            }

            if future_cleared > 0 {
                log::debug!("  History: {} items, Position: {}, Future cleared: {} items",
                    self.history.len(), self.position, future_cleared);
            } else {
                log::debug!("  History: {} items, Position: {}",
                    self.history.len(), self.position);
            }
        }
    }

    /// Check if new change should be coalesced with last history entry
    fn should_coalesce(&self, new_change: &ConfigChange) -> bool {
        // Never coalesce snapshots
        if new_change.snapshot.is_some() {
            return false;
        }

        // Never coalesce if no deltas (safety check)
        if new_change.deltas.is_empty() {
            return false;
        }

        // Only coalesce if at head of history
        if self.position == 0 || self.position != self.history.len() {
            return false;
        }

        let last_change = &self.history[self.position - 1];

        // Must have same number of deltas (same parameters being changed)
        if last_change.deltas.len() != new_change.deltas.len() {
            return false;
        }

        // Check each delta
        for (old_delta, new_delta) in last_change.deltas.iter().zip(new_change.deltas.iter()) {
            // Must be same path
            if old_delta.path != new_delta.path {
                return false;
            }

            // Path must support coalescing
            if !supports_coalescing(&new_delta.path) {
                return false;
            }
        }

        // Check inactivity threshold: pausing for 500ms+ creates new undo point
        // Use last_update_time (most recent change) not timestamp (first change)
        let time_since_last = new_change.timestamp.duration_since(last_change.last_update_time);
        if time_since_last > COALESCE_INACTIVITY_THRESHOLD {
            return false;
        }

        // Check maximum coalesce span: total duration from first to last change
        // timestamp = first change, new timestamp = current change
        let total_span = new_change.timestamp.duration_since(last_change.timestamp);
        if total_span > MAX_COALESCE_SPAN {
            return false;
        }

        true
    }

    /// Extract value from any FractalConfig by path (helper for undo/redo)
    fn get_value_from_config(
        config: &FractalConfig,
        path: &ConfigPath,
    ) -> Result<ConfigValue, ConfigError> {
        match path {
            // View
            ConfigPath::Zoom => Ok(config.zoom.into()),
            ConfigPath::Pan => Ok((config.pan_x, config.pan_y).into()),
            ConfigPath::PanX => Ok(config.pan_x.into()),
            ConfigPath::PanY => Ok(config.pan_y.into()),
            ConfigPath::Rotation => Ok(config.rotation.into()),
            ConfigPath::CameraRotationX => Ok(config.camera_rotation_x.into()),
            ConfigPath::CameraRotationY => Ok(config.camera_rotation_y.into()),
            ConfigPath::CameraZ => Ok(config.camera_z.into()),
            ConfigPath::DofFocusDistance => Ok(config.dof_focus_distance.into()),
            ConfigPath::DofBlurStrength => Ok(config.dof_blur_strength.into()),
            ConfigPath::FogStrength => Ok(config.fog_strength.into()),
            ConfigPath::FogStart => Ok(config.fog_start.into()),

            // Tone mapping
            ConfigPath::Exposure => Ok(config.exposure.into()),
            ConfigPath::Gamma => Ok(config.gamma.into()),
            ConfigPath::GammaThreshold => Ok(config.gamma_threshold.into()),
            ConfigPath::Brightness => Ok(config.brightness.into()),
            ConfigPath::Vibrancy => Ok(config.vibrancy.into()),
            ConfigPath::Saturation => Ok(config.saturation.into()),
            ConfigPath::HueShift => Ok(config.hue_shift.into()),
            ConfigPath::AlphaBlendLow => Ok(config.alpha_blend_low.into()),
            ConfigPath::AlphaBlendHigh => Ok(config.alpha_blend_high.into()),
            ConfigPath::DensityScale => Ok(config.density_scale.into()),
            ConfigPath::TonemapMode => Ok(config.tonemap_mode.into()),
            ConfigPath::TonemapCurve => Ok(config.tonemap_curve.clone().into()),
            ConfigPath::UseCurve => Ok(config.use_curve.into()),
            // Levels controls
            ConfigPath::LevelsLow => Ok(config.levels_low.into()),
            ConfigPath::LevelsHigh => Ok(config.levels_high.into()),
            ConfigPath::LevelsGamma => Ok(config.levels_gamma.into()),

            // Color
            ConfigPath::ColorMode => Ok(config.color_mode.into()),
            ConfigPath::PathMapStyle => Ok(config.path_map_style.into()),
            ConfigPath::PathCaptureMode => Ok(config.path_capture_mode.into()),
            ConfigPath::PathTrackingMode => Ok(config.path_tracking_mode.into()),
            ConfigPath::PaletteIndex => {
                // PaletteIndex is deprecated - return 0 for backward compatibility
                Ok(0u32.into())
            }
            ConfigPath::Palette => {
                // Return the palette directly (always present)
                Ok(ConfigValue::Palette(config.palette.clone()))
            }
            ConfigPath::PaletteRotation => Ok(config.palette_rotation.into()),
            ConfigPath::PaletteSize => Ok((config.palette_size as f32).into()),
            ConfigPath::PaletteSqueeze => Ok(config.palette_squeeze.into()),
            ConfigPath::SpeedFactor => Ok(config.speed_factor.into()),
            ConfigPath::BackgroundColor => Ok(config.background_color.into()),
            ConfigPath::BackgroundColorR => Ok(config.background_color[0].into()),
            ConfigPath::BackgroundColorG => Ok(config.background_color[1].into()),
            ConfigPath::BackgroundColorB => Ok(config.background_color[2].into()),

            // Rendering settings
            ConfigPath::HistogramColorScale => Ok(config.histogram_color_scale.into()),
            ConfigPath::BlendFactor => Ok(config.blend_factor.into()),
            ConfigPath::UseDynamicBlend => Ok(config.use_dynamic_blend.into()),
            ConfigPath::TargetIterationsPerPixel => {
                Ok(config.target_iterations_per_pixel.into())
            }
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
            ConfigPath::TransformPostAffineEnabled { index } => {
                let xform = config
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_affine_enabled.into())
            }
            ConfigPath::TransformPostAffine { index, param } => {
                let xform = config
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let value = match param {
                    AffineParam::A => xform.post_a,
                    AffineParam::B => xform.post_b,
                    AffineParam::C => xform.post_c,
                    AffineParam::D => xform.post_d,
                    AffineParam::E => xform.post_e,
                    AffineParam::F => xform.post_f,
                    AffineParam::G => xform.post_g,
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
            // High-level transform operations (translate, rotate, scale)
            ConfigPath::TransformOriginX { index } => {
                let xform = config
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.origin_x().into())
            }
            ConfigPath::TransformOriginY { index } => {
                let xform = config
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.origin_y().into())
            }
            ConfigPath::TransformRotation { index } => {
                let xform = config
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.rotation().into())
            }
            ConfigPath::TransformScale { index } => {
                let xform = config
                    .flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.scale().into())
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
            ConfigPath::FinalTransformPostAffineEnabled => {
                let final_xform = config
                    .flame
                    .final_transform
                    .as_ref()
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(final_xform.post_affine_enabled.into())
            }
            ConfigPath::FinalTransformPostAffine { param } => {
                let final_xform = config
                    .flame
                    .final_transform
                    .as_ref()
                    .ok_or(ConfigError::InvalidIndex)?;
                let value = match param {
                    AffineParam::A => final_xform.post_a,
                    AffineParam::B => final_xform.post_b,
                    AffineParam::C => final_xform.post_c,
                    AffineParam::D => final_xform.post_d,
                    AffineParam::E => final_xform.post_e,
                    AffineParam::F => final_xform.post_f,
                    AffineParam::G => final_xform.post_g,
                };
                Ok(value.into())
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
            ConfigPath::FinalTransformOriginX => {
                let final_xform = config
                    .flame
                    .final_transform
                    .as_ref()
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(final_xform.origin_x().into())
            }
            ConfigPath::FinalTransformOriginY => {
                let final_xform = config
                    .flame
                    .final_transform
                    .as_ref()
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(final_xform.origin_y().into())
            }
            ConfigPath::FinalTransformRotation => {
                let final_xform = config
                    .flame
                    .final_transform
                    .as_ref()
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(final_xform.rotation().into())
            }
            ConfigPath::FinalTransformScale => {
                let final_xform = config
                    .flame
                    .final_transform
                    .as_ref()
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(final_xform.scale().into())
            }

            // Flame
            ConfigPath::RenderMode => Ok(config.flame.render_mode.into()),
            ConfigPath::PerspectiveStrength => Ok(config.flame.perspective_strength.into()),

            // Effects
            ConfigPath::DensityEffectEnabled { index } => {
                let effect = config
                    .density_effects
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(effect.enabled.into())
            }
            ConfigPath::DensityEffectParam { index, param } => {
                let effect = config
                    .density_effects
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(effect.get_param(param).into())
            }
            ConfigPath::ColorEffectEnabled { index } => {
                let effect = config
                    .color_effects
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(effect.enabled.into())
            }
            ConfigPath::ColorEffectParam { index, param } => {
                let effect = config
                    .color_effects
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(effect.get_param(param).into())
            }

            // Add/Remove operations don't have a "get" value
            ConfigPath::AddColorEffect { .. }
            | ConfigPath::RemoveColorEffect { .. }
            | ConfigPath::AddDensityEffect { .. }
            | ConfigPath::RemoveDensityEffect { .. } => {
                Err(ConfigError::InvalidOperation)
            }

            // Xaos (chaos-weighted transform transitions)
            ConfigPath::Xaos { src, dst } => {
                let weight = config.flame.get_xaos(*src, *dst);
                Ok(weight.into())
            }

            // Solo transform (-1 = None, 0+ = Some(index))
            ConfigPath::SoloTransform => {
                let value = match config.flame.solo_transform {
                    None => -1i32,
                    Some(idx) => idx as i32,
                };
                Ok(value.into())
            }

            // System Settings - These should NOT be called via get_value (they're not in FractalConfig)
            // Use config_manager.system_settings() instead
            ConfigPath::SystemIterationsPerThread
            | ConfigPath::SystemVsyncEnabled
            | ConfigPath::SystemTargetFps
            | ConfigPath::SystemExportWidth
            | ConfigPath::SystemExportHeight
            | ConfigPath::SystemLanguage
            | ConfigPath::SystemBurnIn
            | ConfigPath::SystemShowHelpOnStartup => {
                panic!("System settings should not be accessed via get_value(). Use config_manager.system_settings() instead.");
            }
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
            ConfigPath::Pan => {
                let (x, y): (f32, f32) = value.try_into()?;
                self.current.pan_x = x;
                self.current.pan_y = y;
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
            ConfigPath::DofFocusDistance => {
                self.current.dof_focus_distance = value.try_into()?;
            }
            ConfigPath::DofBlurStrength => {
                self.current.dof_blur_strength = value.try_into()?;
            }
            ConfigPath::FogStrength => {
                self.current.fog_strength = value.try_into()?;
            }
            ConfigPath::FogStart => {
                self.current.fog_start = value.try_into()?;
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
            ConfigPath::AlphaBlendLow => {
                self.current.alpha_blend_low = value.try_into()?;
            }
            ConfigPath::AlphaBlendHigh => {
                self.current.alpha_blend_high = value.try_into()?;
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
            // Levels controls
            ConfigPath::LevelsLow => {
                self.current.levels_low = value.try_into()?;
            }
            ConfigPath::LevelsHigh => {
                self.current.levels_high = value.try_into()?;
            }
            ConfigPath::LevelsGamma => {
                self.current.levels_gamma = value.try_into()?;
            }

            // Color
            ConfigPath::ColorMode => {
                self.current.color_mode = value.try_into()?;
            }
            ConfigPath::PathMapStyle => {
                self.current.path_map_style = value.try_into()?;
            }
            ConfigPath::PathCaptureMode => {
                self.current.path_capture_mode = value.try_into()?;
            }
            ConfigPath::PathTrackingMode => {
                self.current.path_tracking_mode = value.try_into()?;
            }
            ConfigPath::PaletteIndex => {
                // PaletteIndex is deprecated - ignore updates
                log::warn!("Attempted to set deprecated PaletteIndex - use Palette instead");
            }
            ConfigPath::Palette => {
                if let ConfigValue::Palette(mut palette) = value {
                    // Safety: Never allow built-in flag to be true in config.palette
                    // Built-ins should only exist in the library
                    if palette.built_in {
                        log::warn!("Attempted to set built-in palette in config.palette - forcing built_in=false");
                        palette.built_in = false;
                    }

                    // Update palette data
                    self.current.palette = palette;
                }
            }
            ConfigPath::PaletteRotation => {
                self.current.palette_rotation = value.try_into()?;
            }
            ConfigPath::PaletteSize => {
                let v: f32 = value.try_into()?;
                self.current.palette_size = (v as u32).clamp(256, 4096);
            }
            ConfigPath::PaletteSqueeze => {
                let v: f32 = value.try_into()?;
                self.current.palette_squeeze = v.clamp(0.1, 16.0);
            }
            ConfigPath::SpeedFactor => {
                self.current.speed_factor = value.try_into()?;
            }
            ConfigPath::BackgroundColor => {
                let c: [f32; 3] = value.try_into()?;
                self.current.background_color = [
                    c[0].clamp(0.0, 1.0),
                    c[1].clamp(0.0, 1.0),
                    c[2].clamp(0.0, 1.0),
                ];
            }
            ConfigPath::BackgroundColorR => {
                let v: f32 = value.try_into()?;
                self.current.background_color[0] = v.clamp(0.0, 1.0);
            }
            ConfigPath::BackgroundColorG => {
                let v: f32 = value.try_into()?;
                self.current.background_color[1] = v.clamp(0.0, 1.0);
            }
            ConfigPath::BackgroundColorB => {
                let v: f32 = value.try_into()?;
                self.current.background_color[2] = v.clamp(0.0, 1.0);
            }

            // Rendering settings
            ConfigPath::HistogramColorScale => {
                self.current.histogram_color_scale = value.try_into()?;
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
            ConfigPath::TransformPostAffineEnabled { index } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                xform.post_affine_enabled = value.try_into()?;
            }
            ConfigPath::TransformPostAffine { index, param } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                match param {
                    AffineParam::A => xform.post_a = new_value,
                    AffineParam::B => xform.post_b = new_value,
                    AffineParam::C => xform.post_c = new_value,
                    AffineParam::D => xform.post_d = new_value,
                    AffineParam::E => xform.post_e = new_value,
                    AffineParam::F => xform.post_f = new_value,
                    AffineParam::G => xform.post_g = new_value,
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
                // Use NaN as sentinel value to signal removal
                if weight.is_nan() {
                    xform.variations.remove(variation);
                } else {
                    // Always insert/update - don't auto-remove at 0.0
                    // This allows variations to stay visible at weight 0
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
            // High-level transform operations (translate, rotate, scale)
            ConfigPath::TransformOriginX { index } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                xform.set_origin_x(new_value);
            }
            ConfigPath::TransformOriginY { index } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                xform.set_origin_y(new_value);
            }
            ConfigPath::TransformRotation { index } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                xform.set_rotation(new_value);
            }
            ConfigPath::TransformScale { index } => {
                let xform = self
                    .current
                    .flame
                    .transforms
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                xform.set_scale(new_value);
            }

            // Final Transform
            ConfigPath::FinalTransformEnabled => {
                let enabled: bool = value.try_into()?;
                if enabled && self.current.flame.final_transform.is_none() {
                    // Create new final transform with identity affine and linear variation
                    let mut final_xform = crate::scene::transforms::Transform::new();
                    final_xform.set_variation("linear", 1.0);
                    self.current.flame.final_transform = Some(final_xform);
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
            ConfigPath::FinalTransformPostAffineEnabled => {
                let final_xform = self
                    .current
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                final_xform.post_affine_enabled = value.try_into()?;
            }
            ConfigPath::FinalTransformPostAffine { param } => {
                let final_xform = self
                    .current
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                match param {
                    AffineParam::A => final_xform.post_a = new_value,
                    AffineParam::B => final_xform.post_b = new_value,
                    AffineParam::C => final_xform.post_c = new_value,
                    AffineParam::D => final_xform.post_d = new_value,
                    AffineParam::E => final_xform.post_e = new_value,
                    AffineParam::F => final_xform.post_f = new_value,
                    AffineParam::G => final_xform.post_g = new_value,
                }
            }
            ConfigPath::FinalTransformVariation { variation } => {
                let final_xform = self
                    .current
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                let weight: f32 = value.try_into()?;
                // Use NaN as sentinel value to signal removal
                if weight.is_nan() {
                    final_xform.variations.remove(variation);
                } else {
                    // Always insert/update - don't auto-remove at 0.0
                    final_xform.variations.insert(variation.clone(), weight);
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
            ConfigPath::FinalTransformOriginX => {
                let final_xform = self
                    .current
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                final_xform.set_origin_x(new_value);
            }
            ConfigPath::FinalTransformOriginY => {
                let final_xform = self
                    .current
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                final_xform.set_origin_y(new_value);
            }
            ConfigPath::FinalTransformRotation => {
                let final_xform = self
                    .current
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                final_xform.set_rotation(new_value);
            }
            ConfigPath::FinalTransformScale => {
                let final_xform = self
                    .current
                    .flame
                    .final_transform
                    .as_mut()
                    .ok_or(ConfigError::InvalidIndex)?;
                let new_value: f32 = value.try_into()?;
                final_xform.set_scale(new_value);
            }

            // Flame
            ConfigPath::RenderMode => {
                self.current.flame.render_mode = value.try_into()?;
            }
            ConfigPath::PerspectiveStrength => {
                self.current.flame.perspective_strength = value.try_into()?;
            }

            // Effects
            ConfigPath::DensityEffectEnabled { index } => {
                let effect = self
                    .current
                    .density_effects
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                effect.enabled = value.try_into()?;
            }
            ConfigPath::DensityEffectParam { index, param } => {
                let effect = self
                    .current
                    .density_effects
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                effect.set_param(param, value.try_into()?);
            }
            ConfigPath::ColorEffectEnabled { index } => {
                let effect = self
                    .current
                    .color_effects
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                effect.enabled = value.try_into()?;
            }
            ConfigPath::ColorEffectParam { index, param } => {
                let effect = self
                    .current
                    .color_effects
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                effect.set_param(param, value.try_into()?);
            }

            // Add/Remove effect operations
            ConfigPath::AddColorEffect { effect_type } => {
                use crate::effects::EffectInstance;
                self.current.color_effects.push(EffectInstance::new(effect_type));
            }
            ConfigPath::RemoveColorEffect { index } => {
                if *index >= self.current.color_effects.len() {
                    return Err(ConfigError::InvalidIndex);
                }
                self.current.color_effects.remove(*index);
            }
            ConfigPath::AddDensityEffect { effect_type } => {
                use crate::effects::EffectInstance;
                self.current.density_effects.push(EffectInstance::new(effect_type));
            }
            ConfigPath::RemoveDensityEffect { index } => {
                if *index >= self.current.density_effects.len() {
                    return Err(ConfigError::InvalidIndex);
                }
                self.current.density_effects.remove(*index);
            }

            // Xaos (chaos-weighted transform transitions)
            ConfigPath::Xaos { src, dst } => {
                let weight: f32 = value.try_into()?;
                self.current.flame.set_xaos(*src, *dst, weight);
            }

            // Solo transform (-1 = None, 0+ = Some(index))
            ConfigPath::SoloTransform => {
                let idx: i32 = value.try_into()?;
                self.current.flame.solo_transform = if idx < 0 {
                    None
                } else {
                    Some(idx as usize)
                };
            }

            // System Settings - These should NOT be called via apply_value (they're not in FractalConfig)
            // Use config_manager.update_system_setting() instead
            ConfigPath::SystemIterationsPerThread
            | ConfigPath::SystemVsyncEnabled
            | ConfigPath::SystemTargetFps
            | ConfigPath::SystemExportWidth
            | ConfigPath::SystemExportHeight
            | ConfigPath::SystemLanguage
            | ConfigPath::SystemBurnIn
            | ConfigPath::SystemShowHelpOnStartup => {
                panic!("System settings should not be modified via apply_value(). Use config_manager.update_system_setting() instead.");
            }
        }

        Ok(())
    }

    /// Force commit preview to current (call on drag end)
    /// Creates final undo entry if preview differs from current
    /// This ensures changes are captured even if drag ended before throttle fired
    pub fn force_commit_preview(&mut self, path: &ConfigPath) -> Result<UpdateType, ConfigError> {
        if let Some(preview) = self.preview.take() {
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

    /// Get system settings (read-only)
    /// System settings are device-specific preferences that don't belong in FractalConfig
    pub fn system_settings(&self) -> &crate::storage::SystemSettings {
        &self.system_settings
    }

    /// Get mutable system settings (for operations that need it)
    pub fn system_settings_mut(&mut self) -> &mut crate::storage::SystemSettings {
        &mut self.system_settings
    }

    /// Load a complete config (e.g., preset, imported file)
    /// Creates single bidirectional snapshot for efficient undo/redo
    /// Use this for atomic operations like loading presets
    pub fn load_config(&mut self, new_config: FractalConfig, description: String) -> Result<(), ConfigError> {
        // Clear any preview state
        self.preview = None;

        // Create single bidirectional snapshot
        let change = ConfigChange::full_config_snapshot(
            self.current.clone(),  // before
            new_config.clone(),    // after
            description,
        );

        self.push_undo(change);

        // Replace current config
        self.current = new_config;

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

    /// Load a complete config silently (no undo entry)
    /// Used when restoring base config after animation stops
    /// The undo entry should have already been created by handle_animation_exit
    pub fn load_config_silent(&mut self, new_config: FractalConfig) -> Result<(), ConfigError> {
        // Clear any preview state
        self.preview = None;

        // Replace current config (no undo entry)
        self.current = new_config;

        // Record full config import action for GPU updates
        let mut action = UpdateAction::none();
        action.update_flame = true;
        action.update_view = true;
        action.update_palette = true;
        action.update_tone_curve = true;
        action.reset_accumulation = true;
        self.pending_actions.merge(&action);

        Ok(())
    }

    /// Load a complete config with explicit before/after states
    /// Used for animation undo where we track the pre-animation state separately
    /// The current config is NOT modified (we're just recording the transition)
    pub fn load_config_with_explicit_before(
        &mut self,
        before_config: FractalConfig,
        after_config: FractalConfig,
        description: String,
    ) -> Result<(), ConfigError> {
        // Clear any preview state
        self.preview = None;

        // Create bidirectional snapshot with explicit before/after
        let change = ConfigChange::full_config_snapshot(
            before_config,   // before (pre-animation state)
            after_config,    // after (post-animation state, should match current)
            description,
        );

        self.push_undo(change);

        // Note: We don't modify self.current because it already matches after_config
        // (the animation has been applying changes continuously)

        Ok(())
    }

    /// Apply a structural change (transform add/delete)
    /// Creates specialized snapshot and updates config atomically
    pub fn apply_structural_change(&mut self, change: ConfigChange) -> Result<(), ConfigError> {
        // Apply the change based on snapshot type
        if let Some(snapshot) = &change.snapshot {
            match snapshot {
                crate::config::SnapshotData::AddTransform { index, transform, .. } => {
                    if *index <= self.current.flame.transforms.len() {
                        // Update xaos matrix before inserting (needs old transform list for detection)
                        self.current.flame.on_transform_added(*index);
                        self.current.flame.transforms.insert(*index, transform.clone());
                    } else {
                        return Err(ConfigError::InvalidIndex);
                    }
                }
                crate::config::SnapshotData::DeleteTransform { index, .. } => {
                    if *index < self.current.flame.transforms.len() {
                        self.current.flame.transforms.remove(*index);
                        // Update xaos matrix after removing (needs new transform list)
                        self.current.flame.on_transform_deleted(*index);
                    } else {
                        return Err(ConfigError::InvalidIndex);
                    }
                }
                crate::config::SnapshotData::ModifyTransform { index, after, .. } => {
                    if *index < self.current.flame.transforms.len() {
                        self.current.flame.transforms[*index] = after.clone();
                    } else {
                        return Err(ConfigError::InvalidIndex);
                    }
                }
                crate::config::SnapshotData::FullConfig { after, .. } => {
                    self.current = (**after).clone();
                }
                crate::config::SnapshotData::AddColorEffect { index, effect } => {
                    if *index <= self.current.color_effects.len() {
                        self.current.color_effects.insert(*index, effect.clone());
                    } else {
                        return Err(ConfigError::InvalidIndex);
                    }
                    // Record in history and return early with correct update type
                    self.push_undo(change);
                    self.record_action(UpdateType::ToneMappingOnly);
                    return Ok(());
                }
                crate::config::SnapshotData::DeleteColorEffect { index, .. } => {
                    if *index < self.current.color_effects.len() {
                        self.current.color_effects.remove(*index);
                    } else {
                        return Err(ConfigError::InvalidIndex);
                    }
                    self.push_undo(change);
                    self.record_action(UpdateType::ToneMappingOnly);
                    return Ok(());
                }
                crate::config::SnapshotData::AddDensityEffect { index, effect } => {
                    if *index <= self.current.density_effects.len() {
                        self.current.density_effects.insert(*index, effect.clone());
                    } else {
                        return Err(ConfigError::InvalidIndex);
                    }
                    self.push_undo(change);
                    self.record_action(UpdateType::ToneMappingOnly);
                    return Ok(());
                }
                crate::config::SnapshotData::DeleteDensityEffect { index, .. } => {
                    if *index < self.current.density_effects.len() {
                        self.current.density_effects.remove(*index);
                    } else {
                        return Err(ConfigError::InvalidIndex);
                    }
                    self.push_undo(change);
                    self.record_action(UpdateType::ToneMappingOnly);
                    return Ok(());
                }

                crate::config::SnapshotData::MoveColorEffect { from_index, to_index } => {
                    if *from_index >= self.current.color_effects.len() {
                        return Err(ConfigError::InvalidIndex);
                    }
                    let effect = self.current.color_effects.remove(*from_index);
                    let insert_at = (*to_index).min(self.current.color_effects.len());
                    self.current.color_effects.insert(insert_at, effect);
                    self.push_undo(change);
                    self.record_action(UpdateType::ToneMappingOnly);
                    return Ok(());
                }

                crate::config::SnapshotData::MoveDensityEffect { from_index, to_index } => {
                    if *from_index >= self.current.density_effects.len() {
                        return Err(ConfigError::InvalidIndex);
                    }
                    let effect = self.current.density_effects.remove(*from_index);
                    let insert_at = (*to_index).min(self.current.density_effects.len());
                    self.current.density_effects.insert(insert_at, effect);
                    self.push_undo(change);
                    self.record_action(UpdateType::ToneMappingOnly);
                    return Ok(());
                }
            }
        } else {
            return Err(ConfigError::InvalidOperation);
        }

        // Record in history
        self.push_undo(change);

        // Record GPU update action
        self.record_action(UpdateType::IterationReset);

        Ok(())
    }

    /// Start a modify session for a transform
    /// Captures initial state but doesn't create history entry yet
    /// All updates during session will apply to config but not create history
    /// Call commit_modify_transform() to create ModifyTransform snapshot
    pub fn start_modify_transform(&mut self, index: usize) -> Result<(), ConfigError> {
        // Can't start a new session if one is already active
        if self.modify_session.is_some() {
            return Err(ConfigError::InvalidOperation);
        }

        // Validate index
        if index >= self.current.flame.transforms.len() {
            return Err(ConfigError::InvalidIndex);
        }

        // Capture initial state
        let initial_transform = self.current.flame.transforms[index].clone();

        self.modify_session = Some(ModifySession {
            transform_index: index,
            initial_transform,
        });

        log::debug!("Started modify session for transform {}", index);
        Ok(())
    }

    /// Commit the active modify session
    /// Creates ModifyTransform snapshot with before/after states
    /// Returns error if no session is active
    pub fn commit_modify_transform(&mut self, description: String) -> Result<UpdateType, ConfigError> {
        let session = self.modify_session.take()
            .ok_or(ConfigError::InvalidOperation)?;

        // Clear preview state (session is ending)
        self.preview = None;

        let index = session.transform_index;
        let before = session.initial_transform;
        let after = self.current.flame.transforms[index].clone();

        // Check if transform actually changed (avoid no-op snapshots)
        let changed = before.a != after.a || before.b != after.b || before.c != after.c
            || before.d != after.d || before.e != after.e || before.f != after.f
            || before.g != after.g;

        if !changed {
            log::debug!("Modify session commit: no changes detected, skipping snapshot");
            return Ok(UpdateType::None);
        }

        // Create ModifyTransform snapshot
        let change = ConfigChange::modify_transform_snapshot(index, before, after, description);

        // Record in history
        self.push_undo(change);

        // Record GPU update action
        self.record_action(UpdateType::IterationReset);

        log::debug!("Committed modify session for transform {}", index);
        Ok(UpdateType::IterationReset)
    }

    /// Cancel the active modify session
    /// Restores transform to initial state and discards changes
    /// Returns error if no session is active
    pub fn cancel_modify_transform(&mut self) -> Result<(), ConfigError> {
        let session = self.modify_session.take()
            .ok_or(ConfigError::InvalidOperation)?;

        // Clear preview state (session is ending)
        self.preview = None;

        // Restore initial state
        let index = session.transform_index;
        self.current.flame.transforms[index] = session.initial_transform;

        // Record GPU update action (need to restore visual state)
        self.record_action(UpdateType::IterationReset);

        log::debug!("Cancelled modify session for transform {}", index);
        Ok(())
    }

    /// Check if a modify session is currently active
    pub fn is_in_modify_session(&self) -> bool {
        self.modify_session.is_some()
    }

    /// Get full history (unified timeline)
    pub fn history(&self) -> &[ConfigChange] {
        &self.history
    }

    /// Get current position in history
    pub fn position(&self) -> usize {
        self.position
    }

    /// Get undo history (items before current position) - for backward compatibility
    pub fn undo_history(&self) -> &[ConfigChange] {
        &self.history[..self.position]
    }

    /// Get redo history (items at/after current position) - for backward compatibility
    pub fn redo_history(&self) -> &[ConfigChange] {
        &self.history[self.position..]
    }

    /// Check if can undo
    pub fn can_undo(&self) -> bool {
        self.position > 0
    }

    /// Check if can redo
    pub fn can_redo(&self) -> bool {
        self.position < self.history.len()
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

    /// Request a full GPU re-sync (all buffers: flame, palette, view, tone curve).
    /// Used after GPU reinitialization when all renderer state has been lost.
    pub fn request_full_resync(&mut self) {
        self.pending_actions.reset_accumulation = true;
        self.pending_actions.update_flame = true;
        self.pending_actions.update_palette = true;
        self.pending_actions.update_view = true;
        self.pending_actions.update_tone_curve = true;
    }

    /// Record an action for later retrieval
    ///
    /// Called internally when config changes occur
    fn record_action(&mut self, update_type: UpdateType) {
        let action = UpdateAction::from_update_type(update_type);
        self.pending_actions.merge(&action);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    TypeMismatch,
    InvalidIndex,
    InvalidOperation,
    EmptyUndoStack,
    EmptyRedoStack,
    ReadOnlyParameter,
    InvalidPath(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ConfigError::TypeMismatch => write!(f, "Config value type mismatch"),
            ConfigError::InvalidIndex => write!(f, "Invalid transform index"),
            ConfigError::InvalidOperation => write!(f, "Invalid operation"),
            ConfigError::EmptyUndoStack => write!(f, "Nothing to undo"),
            ConfigError::EmptyRedoStack => write!(f, "Nothing to redo"),
            ConfigError::ReadOnlyParameter => write!(f, "Parameter is read-only"),
            ConfigError::InvalidPath(msg) => write!(f, "Invalid config path: {}", msg),
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

impl TryFrom<ConfigValue> for (f32, f32) {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::Vec2(x, y) => Ok((x, y)),
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
use crate::scene::palette::{ColorMode, PathCaptureMode, PathMapStyle, PathTrackingMode};
use crate::scene::transforms::RenderMode;

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

impl TryFrom<ConfigValue> for PathMapStyle {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::PathMapStyle(m) => Ok(m),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for PathCaptureMode {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::PathCaptureMode(m) => Ok(m),
            _ => Err(ConfigError::TypeMismatch),
        }
    }
}

impl TryFrom<ConfigValue> for PathTrackingMode {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::PathTrackingMode(m) => Ok(m),
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
            .update_param(ConfigPath::Exposure, 2.0.into())
            .unwrap();
        assert_eq!(update1, UpdateType::ToneMappingOnly);
        assert_eq!(manager.history.len(), 1);

        // Immediate second update - should NOT capture (throttled)
        let update2 = manager
            .update_param(ConfigPath::Exposure, 3.0.into())
            .unwrap();
        assert_eq!(update2, UpdateType::ToneMappingOnly);
        assert_eq!(manager.history.len(), 1); // Still 1!
    }

    #[test]
    fn test_undo_redo_sequence() {
        // Test a longer sequence to catch redo bugs
        // Use different parameters to avoid coalescing
        let config = FractalConfig::default();
        let mut manager = ConfigManager::new(config);

        // Initial state
        assert!(manager.config().exposure == 1.0);
        assert!(manager.config().gamma == 2.2);
        assert!(manager.config().brightness == 1.0);
        assert!(manager.config().zoom == 1.0);

        // Change 1: exposure
        manager.update_param(ConfigPath::Exposure, 2.0.into()).unwrap();
        assert!(manager.config().exposure == 2.0);

        // Change 2: gamma
        manager.update_param(ConfigPath::Gamma, 3.0.into()).unwrap();
        assert!(manager.config().gamma == 3.0);

        // Change 3: brightness
        manager.update_param(ConfigPath::Brightness, 1.5.into()).unwrap();
        assert!(manager.config().brightness == 1.5);

        // Change 4: zoom
        manager.update_param(ConfigPath::Zoom, 2.0.into()).unwrap();
        assert!(manager.config().zoom == 2.0);

        // Undo: should revert zoom
        manager.undo().unwrap();
        assert!(manager.config().zoom == 1.0, "After 1st undo, expected zoom=1.0, got {}", manager.config().zoom);
        assert!(manager.config().brightness == 1.5);

        // Undo: should revert brightness
        manager.undo().unwrap();
        assert!(manager.config().brightness == 1.0, "After 2nd undo, expected brightness=1.0, got {}", manager.config().brightness);
        assert!(manager.config().gamma == 3.0);

        // Redo: should restore brightness
        manager.redo().unwrap();
        assert!(manager.config().brightness == 1.5, "After 1st redo, expected brightness=1.5, got {}", manager.config().brightness);

        // Redo: should restore zoom
        manager.redo().unwrap();
        assert!(manager.config().zoom == 2.0, "After 2nd redo, expected zoom=2.0, got {}", manager.config().zoom);
    }

    #[test]
    fn test_undo_redo() {
        let config = FractalConfig::default();
        let mut manager = ConfigManager::new(config);

        // Make change
        manager
            .update_param(ConfigPath::Exposure, 2.0.into())
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
            (ConfigPath::Pan, ConfigValue::Vec2(1.0, -1.0)),
            (ConfigPath::Rotation, ConfigValue::Float(45.0)),
        ];

        let update = manager
            .update_batch(changes, "Reset View".to_string())
            .unwrap();

        assert_eq!(update, UpdateType::ViewOnly);
        assert_eq!(manager.history.len(), 1);
        assert_eq!(manager.history[0].deltas.len(), 3);
        assert_eq!(manager.history[0].description, "Reset View");
    }
}

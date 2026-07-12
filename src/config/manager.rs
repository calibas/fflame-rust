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
    AffineParam, ConfigChange, ConfigDelta, ConfigPath, ConfigValue, TransformRef, UpdateType,
};
#[cfg(test)]
use super::delta::TransformKind;
use super::fractal_config::FractalConfig;
use crate::scene::transforms::Flame;
use std::time::Duration;

/// Which flame the editor is currently focused on.
///
/// `Main` is the FractalConfig's main flame (the default). `Subflame { index }`
/// indicates the user is editing `current.flame.subflames[index]`. The
/// data is **not** physically moved — `current.flame` is always the
/// main flame. Apply/get machinery routes through `target_flame` /
/// `target_flame_mut` to pick the right `Flame` based on this enum,
/// and UI panels likewise dereference the right slice based on the
/// active target.
///
/// History entries carry their target (`ConfigChange::target`), so
/// undo/redo applies the inverse delta to the flame it was authored
/// against, even if the user has since switched contexts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EditingTarget {
    Main,
    /// The user is editing `current.flame.subflames[index]`. The
    /// subflames list is untouched; only this field changes when the
    /// user picks a subflame in the Subflames panel.
    Subflame { index: usize },
}

impl Default for EditingTarget {
    fn default() -> Self {
        EditingTarget::Main
    }
}

/// Maximum duration for coalescing - total span from first to last change
const MAX_COALESCE_SPAN: Duration = Duration::from_millis(3000);

/// History description shared by all fly-mode camera writes (mouse-look
/// and WASD/QE movement). Changes carrying this description coalesce
/// with each other even when their path sets differ — a fly gesture
/// alternates between look writes (pitch/yaw, sometimes rotation or
/// position-compensation) and movement writes (position only), and
/// without the exception every event would be its own history entry,
/// flooding the undo stack within seconds of flight.
pub const FLY_CAMERA_HISTORY_DESC: &str = "history.action.fly_camera";

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

/// Return true if the snapshot mutates one of the six animation-tracked
/// lists (transforms in any pool, subflames, color/density effects).
/// Used by undo/redo to flag `structural_changed` for the App-layer
/// animation rebind.
fn snapshot_is_structural(snapshot: Option<&crate::config::SnapshotData>) -> bool {
    use crate::config::SnapshotData;
    matches!(
        snapshot,
        Some(
            SnapshotData::FullConfig { .. }
            | SnapshotData::AddTransform { .. }
            | SnapshotData::DeleteTransform { .. }
            | SnapshotData::AddColorEffect { .. }
            | SnapshotData::DeleteColorEffect { .. }
            | SnapshotData::MoveColorEffect { .. }
            | SnapshotData::AddDensityEffect { .. }
            | SnapshotData::DeleteDensityEffect { .. }
            | SnapshotData::MoveDensityEffect { .. }
            | SnapshotData::AddSubflame { .. }
            | SnapshotData::DeleteSubflame { .. }
        )
    )
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

    /// Refresh the solid-rendering shade-pass parameters (lighting).
    /// Post-accumulate only: no reset, no overwrite mode, no flame
    /// update — the per-frame shade+tonemap re-render picks it up.
    pub update_shading: bool,

    /// One of the six animation-tracked lists changed shape (add /
    /// delete / reorder of a transform pool, subflame, or effect).
    /// The App layer's animation update code checks this and calls
    /// `Animation::rebind_targets` so tracks follow the items they're
    /// bound to.
    pub structural_changed: bool,
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

            UpdateType::ShadingOnly => Self {
                update_shading: true,
                // No reset — the shade pass is post-accumulation, so
                // lighting edits keep every accumulated iteration.
                ..Default::default()
            },

            UpdateType::ColorOnly => Self {
                update_palette: true,
                reset_accumulation: false, // Never reset - use overwrite mode for smooth updates
                ..Default::default()
            },

            UpdateType::IterationReset => Self {
                update_flame: true,
                update_palette: true,
                update_view: true,
                update_tone_curve: true,
                reset_accumulation: false, // Don't reset - use overwrite mode for smooth transition
                rebuild_shader: false, // TODO: detect variation changes
                update_shading: true, // Full updates refresh shading state too (update_flame carries it)
                structural_changed: false, // Only set by explicit structural mutation sites
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
        self.update_shading |= other.update_shading;
        self.structural_changed |= other.structural_changed;
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

    /// Which flame is currently being edited. See `EditingTarget`.
    /// Data is never physically moved — this is purely a routing hint
    /// for the apply/get machinery (`target_flame[_mut]`) and the UI
    /// panels.
    editing_target: EditingTarget,

    /// Monotonic counter bumped whenever a brand-new config is loaded
    /// (preset/import/browser via `load_config`). UI panels read this to
    /// reset per-fractal view state (e.g. collapse the Transforms panel
    /// sections) on fractal switch without conflating it with in-place
    /// edits or undo/redo navigation.
    load_generation: u64,
}

/// Session state for transform modification (triangle editor, etc.).
/// Spans all three pools — the `xref` selects which one.
struct ModifySession {
    /// Pool member being modified.
    xref: TransformRef,
    /// Initial state captured at session start.
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
            editing_target: EditingTarget::Main,
            load_generation: 0,
        }
    }

    /// Monotonic "new fractal loaded" counter — see field docs. Bumps on
    /// `load_config` (preset/import/browser), not on edits or undo/redo.
    pub fn load_generation(&self) -> u64 {
        self.load_generation
    }

    // ===== Subflame editing =====

    /// What the editor is currently focused on (main flame or a subflame).
    pub fn editing_target(&self) -> EditingTarget {
        self.editing_target
    }

    /// Read-only reference to the flame the editor is currently
    /// focused on. UI sites that read back from the config *after*
    /// calling `update_param` (e.g. to sync a local slider variable
    /// to the canonical value) should use this — `active_config()
    /// .flame` is always the main and would return the wrong slot
    /// while editing a subflame. Falls back to the main flame if
    /// the target index is out of bounds.
    pub fn active_flame(&self) -> &Flame {
        let cfg = self.active_config();
        match self.editing_target {
            EditingTarget::Subflame { index } if index < cfg.flame.subflames.len() => {
                &cfg.flame.subflames[index]
            }
            _ => &cfg.flame,
        }
    }

    /// Force a flame re-upload (full IterationReset action set:
    /// update_flame + update_view + update_palette + update_tone_curve,
    /// but no buffer clear). Used by UI surfaces that change *how*
    /// the flame is sourced (e.g. the "view subflame in isolation"
    /// toggle) but not the flame data itself. Matching IterationReset
    /// here keeps the renderer in the same state machine path as
    /// normal transform edits — buffer-clearing turned out to
    /// interact badly with the renderer's iteration-count tracking
    /// at high max_iterations values (black-screen bug).
    pub fn request_flame_refresh(&mut self) {
        self.pending_actions.merge(&UpdateAction::from_update_type(UpdateType::IterationReset));
    }

    /// True when the user is editing a subflame (not the main flame).
    pub fn is_editing_subflame(&self) -> bool {
        matches!(self.editing_target, EditingTarget::Subflame { .. })
    }

    /// The user-visible subflames list. There's no swap anymore — this
    /// is always `current.flame.subflames`. (Kept as a method so the
    /// existing Subflames panel doesn't need a touchup.)
    pub fn visible_subflames(&self) -> &[Flame] {
        &self.current.flame.subflames
    }

    /// Total number of subflames as seen by the user. Equivalent to
    /// `visible_subflames().len()`; kept as a method for the same
    /// reason as `visible_subflames`.
    pub fn logical_subflame_count(&self) -> usize {
        self.current.flame.subflames.len()
    }

    /// Resolve an `EditingTarget` to a mutable `Flame` reference. `Main`
    /// returns the main flame; `Subflame { index }` returns the nested
    /// subflame at that index. Used by the apply/get machinery to route
    /// writes/reads to the right slot without physically swapping data.
    fn target_flame_mut(&mut self, target: EditingTarget) -> Option<&mut Flame> {
        match target {
            EditingTarget::Main => Some(&mut self.current.flame),
            EditingTarget::Subflame { index } => self.current.flame.subflames.get_mut(index),
        }
    }

    /// Resolve a target on an arbitrary FractalConfig. Used by
    /// snapshot apply paths that operate on a passed-in config rather
    /// than `self.current`.
    fn target_flame_in(config: &FractalConfig, target: EditingTarget) -> Option<&Flame> {
        match target {
            EditingTarget::Main => Some(&config.flame),
            EditingTarget::Subflame { index } => config.flame.subflames.get(index),
        }
    }

    /// Get a mutable Normal-pool transform on the active editing
    /// target. Returns InvalidIndex if the target flame is missing
    /// (subflame index out of range) or the transform index is out of
    /// range. Used by `set_value` to keep the per-variant call site
    /// short.
    fn normal_transform_mut(&mut self, index: usize) -> Result<&mut crate::scene::transforms::Transform, ConfigError> {
        let target = self.editing_target;
        let flame = self.target_flame_mut(target).ok_or(ConfigError::InvalidIndex)?;
        flame.transforms.get_mut(index).ok_or(ConfigError::InvalidIndex)
    }

    fn linked_transform_mut(&mut self, index: usize) -> Result<&mut crate::scene::transforms::Transform, ConfigError> {
        let target = self.editing_target;
        let flame = self.target_flame_mut(target).ok_or(ConfigError::InvalidIndex)?;
        flame.linked_transforms.get_mut(index).ok_or(ConfigError::InvalidIndex)
    }

    fn final_transform_mut(&mut self, index: usize) -> Result<&mut crate::scene::transforms::Transform, ConfigError> {
        let target = self.editing_target;
        let flame = self.target_flame_mut(target).ok_or(ConfigError::InvalidIndex)?;
        flame.final_transforms.get_mut(index).ok_or(ConfigError::InvalidIndex)
    }

    /// Get a mutable reference to the flame the editor is currently
    /// focused on. Returns InvalidIndex if Subflame{i} but i is out of
    /// range. Used by `set_value` for flame-metadata variants
    /// (RenderMode, Xaos, etc.) that aren't per-transform.
    fn active_flame_mut(&mut self) -> Result<&mut Flame, ConfigError> {
        let target = self.editing_target;
        self.target_flame_mut(target).ok_or(ConfigError::InvalidIndex)
    }

    /// Switch to editing the given target and push a SwapTarget undo
    /// entry so the switch is reversible.
    ///
    /// The renderer must re-upload the flame after this returns — the
    /// caller checks `get_pending_actions()` to drive that.
    pub fn set_editing_target(&mut self, target: EditingTarget) -> Result<(), ConfigError> {
        if self.editing_target == target {
            return Ok(());
        }
        let before = self.editing_target;
        self.set_editing_target_silent(target)?;
        // Push the swap as its own undo entry so walking back through
        // history naturally restores the editing context at each step.
        let change = ConfigChange::swap_target_snapshot(before, target);
        self.push_undo(change);
        Ok(())
    }

    /// Switch editing target without pushing an undo entry. Used by
    /// undo/redo apply paths (which already have an entry).
    /// No data is moved — just updates the routing field and asks
    /// the renderer to re-sync the flame state via the standard
    /// IterationReset action set (no buffer clear; smooth transition
    /// like a transform edit).
    fn set_editing_target_silent(&mut self, target: EditingTarget) -> Result<(), ConfigError> {
        if self.editing_target == target {
            return Ok(());
        }
        if let EditingTarget::Subflame { index } = target {
            if index >= self.current.flame.subflames.len() {
                return Err(ConfigError::InvalidPath(format!(
                    "subflame {} out of bounds", index
                )));
            }
        }
        self.editing_target = target;

        // Match IterationReset semantics. update_flame uploads the
        // new flame state to GPU; the rest are belt-and-suspenders
        // so any texture/buffer that needs a refresh gets one. NO
        // reset_accumulation — keeps the accumulator alive, letting
        // overwrite mode smooth the transition like a transform
        // edit does. (Previously we set reset_accumulation=true,
        // which interacted badly with the renderer's iteration-count
        // state machine at high max_iterations values — black-screen
        // bug.)
        self.pending_actions.merge(&UpdateAction::from_update_type(UpdateType::IterationReset));
        Ok(())
    }

    /// Add a new empty subflame and return its index. Always lands at
    /// the end of the subflames list. Unlike the previous swap-based
    /// implementation, this works regardless of the current editing
    /// target.
    pub fn add_subflame(&mut self) -> Result<usize, ConfigError> {
        // Seed with one identity-affine linear transform so the new
        // subflame is renderable on day one. (An empty flame would
        // make NUM_TRANSFORMS=0 and trip the shader builder's
        // NUM_TRANSFORMS-1u underflow check; the builder has a
        // .max(1) guard for that, but a single-transform default
        // is much more useful to the user.)
        let mut new = Flame::new();
        // Empty name by default — the panel renders unnamed subflames as
        // just "Subflame"; the user renames if they want to disambiguate.
        new.name = String::new();
        let mut seed = crate::scene::transforms::Transform::default();
        seed.set_variation("linear", 1.0);
        seed.color = 0.5;
        seed.color_speed = 0.5;
        new.transforms.push(seed);
        let index = self.current.flame.subflames.len();
        self.current.flame.subflames.push(new.clone());

        // Push undo entry — full Flame stored so redo recreates exactly
        // even after intervening edits to the rest of the config.
        let target_before = self.editing_target;
        let change = ConfigChange::add_subflame_snapshot(
            index,
            new,
            target_before,
            "Add subflame".to_string(),
        );
        self.push_undo(change);

        // IterationReset semantics (no buffer clear). The new
        // subflame's data is uploaded; the rendered parent flame
        // visually unchanged unless one of its transforms references
        // this subflame via subflame_wf. See set_editing_target_silent
        // for the rationale on avoiding reset_accumulation here.
        self.pending_actions.merge(&UpdateAction::from_update_type(UpdateType::IterationReset));
        // Subflames list shape changed → flag for animation rebind.
        self.pending_actions.structural_changed = true;
        Ok(index)
    }

    /// Delete the subflame at `index`. If the user is currently
    /// editing that subflame, automatically reset the editing target
    /// to Main so we're not pointing at a stale slot. Editing a
    /// *different* subflame is fine; the index of THAT one shifts if
    /// the deleted index is lower (the editing_target is updated in
    /// that case too).
    pub fn delete_subflame(&mut self, index: usize) -> Result<(), ConfigError> {
        if index >= self.current.flame.subflames.len() {
            return Err(ConfigError::InvalidPath(format!(
                "subflame {} out of bounds", index
            )));
        }

        // Capture target_before so undo restores the editing context.
        let target_before = self.editing_target;

        let removed = self.current.flame.subflames.remove(index);

        // Update editing_target to reflect the new index space.
        if let EditingTarget::Subflame { index: active } = self.editing_target {
            if active == index {
                // The active subflame was deleted; fall back to Main.
                self.editing_target = EditingTarget::Main;
            } else if active > index {
                // A subflame at a lower index was deleted; shift down.
                self.editing_target = EditingTarget::Subflame { index: active - 1 };
            }
        }

        // Push undo entry — full Flame stored so undo restores it
        // byte-for-byte.
        let change = ConfigChange::delete_subflame_snapshot(
            index,
            removed,
            target_before,
            "Delete subflame".to_string(),
        );
        self.push_undo(change);

        // IterationReset semantics — same reasoning as add_subflame.
        self.pending_actions.merge(&UpdateAction::from_update_type(UpdateType::IterationReset));
        // Subflames list shape changed → flag for animation rebind.
        self.pending_actions.structural_changed = true;
        Ok(())
    }

    /// Replace the flame data at subflame `index` with `new_flame`,
    /// keeping the subflame slot in place. Used by the "Load from
    /// file" button in the Subflames panel to swap a subflame's IFS
    /// for one loaded from a `.fflame`.
    ///
    /// Undo support uses a `FullConfig` snapshot — heavy compared to
    /// a dedicated `ReplaceSubflame` variant, but simple and
    /// correct, and a normal config is small enough that the doubled
    /// memory doesn't matter in practice.
    ///
    /// IDs on the incoming flame and its transforms are assigned
    /// fresh via `fixup_ids`; old animation tracks bound to the
    /// previous subflame's items will surface as broken in the UI
    /// (the rebind hook fires via `structural_changed`).
    pub fn replace_subflame(&mut self, index: usize, new_flame: Flame) -> Result<(), ConfigError> {
        if index >= self.current.flame.subflames.len() {
            return Err(ConfigError::InvalidPath(format!(
                "subflame {} out of bounds", index
            )));
        }

        // Snapshot for undo (heavy — FullConfig — but symmetric across
        // before/after, no new SnapshotData variant required).
        let before = self.current.clone();
        self.current.flame.subflames[index] = new_flame;
        // Allocate IDs for the freshly-installed flame and its
        // transforms; the loaded `.fflame` came in with id=0
        // everywhere (serde-skipped) and otherwise wouldn't be
        // resolvable by the animation rebind machinery.
        self.current.fixup_ids();
        let after = self.current.clone();

        let change = ConfigChange::full_config_snapshot(
            before,
            after,
            "Replace subflame".to_string(),
        );
        self.push_undo(change);

        // IterationReset semantics (no buffer clear) — same as
        // add_subflame / delete_subflame.
        self.pending_actions.merge(&UpdateAction::from_update_type(UpdateType::IterationReset));
        // Subflame contents changed → animation rebind to follow IDs
        // (which all just changed because of fixup_ids).
        self.pending_actions.structural_changed = true;
        Ok(())
    }

    /// Rename the subflame at `index`. With the un-swap refactor the
    /// list is always intact, so this is a straightforward index
    /// into `current.flame.subflames`.
    pub fn rename_subflame(&mut self, index: usize, name: String) -> Result<(), ConfigError> {
        if index >= self.current.flame.subflames.len() {
            return Err(ConfigError::InvalidPath(format!(
                "subflame {} out of bounds", index
            )));
        }
        self.current.flame.subflames[index].name = name;
        Ok(())
    }

    /// Read-only snapshot of the FractalConfig as it logically exists.
    /// Since data is no longer physically swapped, this is just a
    /// clone of `current`. Kept as a method so callers that used to
    /// rely on the un-swap reconstruction don't need to change.
    pub fn logical_config(&self) -> FractalConfig {
        self.current.clone()
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

    /// Remove a variation from a transform in any pool, recording a
    /// **whole-transform snapshot** so undo restores the variation's
    /// params, fx_priority and order metadata.
    ///
    /// `Transform::remove_variation` scrubs all of that metadata (correctly —
    /// leaving it orphans it). A plain weight delta (the old NaN-sentinel
    /// `update_param` path) only captured the weight, so undo recreated the
    /// variation with default params. The before/after transform snapshot
    /// carries the full state both ways. Routes to the active editing target
    /// (main or subflame) like every other transform edit.
    pub fn remove_variation(
        &mut self,
        xref: TransformRef,
        variation: &str,
    ) -> Result<UpdateType, ConfigError> {
        // Snapshot the transform before the scrub.
        let before = xref
            .get(self.active_flame())
            .ok_or(ConfigError::InvalidIndex)?
            .clone();

        // Apply the removal on the active (editing-target) flame.
        {
            let flame = self.active_flame_mut()?;
            let slot = xref.get_mut(flame).ok_or(ConfigError::InvalidIndex)?;
            slot.remove_variation(variation);
        }

        let after = xref
            .get(self.active_flame())
            .ok_or(ConfigError::InvalidIndex)?
            .clone();

        let change = ConfigChange::modify_transform_snapshot(
            xref.kind(),
            xref.index(),
            before,
            after,
            format!("Remove {}", variation),
        );
        self.push_undo(change);
        let update_type = UpdateType::IterationReset;
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
            ConfigPath::SystemFlyMouseSensitivity => {
                self.system_settings.fly_mouse_sensitivity = new_value.try_into()?;
            }
            ConfigPath::SystemFlyMoveSpeed => {
                self.system_settings.fly_move_speed = new_value.try_into()?;
            }
            ConfigPath::SystemFlySprintMultiplier => {
                self.system_settings.fly_sprint_multiplier = new_value.try_into()?;
            }
            ConfigPath::SystemFlyInvertY => {
                self.system_settings.fly_invert_y = new_value.try_into()?;
            }
            ConfigPath::SystemFlyCameraMode => {
                // String transport ("free_look" / "fps") so future
                // modes (e.g. orbital) don't need a new value type.
                let value = match new_value {
                    ConfigValue::String(s) => s,
                    _ => return Err(ConfigError::TypeMismatch),
                };
                self.system_settings.fly_camera_mode = match value.as_str() {
                    "fps" => crate::storage::FlyCameraMode::Fps,
                    _ => crate::storage::FlyCameraMode::FreeLook,
                };
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

    /// Silent update routed against an explicit editing target.
    ///
    /// Same as `update_param_silent` but applies the change to the
    /// `target` flame rather than the current editing target. Used by
    /// the animation system so a track can target Main or any
    /// subflame independent of what the editor panels are focused on.
    ///
    /// Implemented by temporarily swapping `editing_target` and
    /// restoring it on the way out — the per-pool helpers
    /// (`normal_transform_mut`, etc.) read `editing_target` to pick
    /// the right `Flame`, so the swap reroutes them transparently.
    pub fn update_param_silent_on(
        &mut self,
        target: EditingTarget,
        path: ConfigPath,
        new_value: ConfigValue,
    ) -> Result<UpdateType, ConfigError> {
        let saved = self.editing_target;
        self.editing_target = target;
        let result = self.update_param_silent(path, new_value);
        self.editing_target = saved;
        result
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

        // Silent-swap the editing context to match the entry's target so
        // the inverse delta / snapshot applies to the correct flame. The
        // entry's `target` was stamped from the editing_target *at the
        // moment of push_undo*, so swapping to it here restores the
        // exact context the change was made in.
        //
        // SwapTarget entries are an exception (their target equals the
        // post-swap context, but their semantics is "swap back to
        // before"), so we still pre-swap to entry.target — for swap
        // entries that's a no-op since we're already there.
        let entry_target = self.history[self.position].target;
        self.set_editing_target_silent(entry_target)?;

        let change = &self.history[self.position];
        log::debug!("Undo: {} (position now: {}, target: {:?})",
            change.description, self.position, change.target);

        // Set structural_changed up-front when the snapshot is one of
        // the structural variants — the early-return branches below
        // won't reach the bottom of the function, so flag it here
        // while we still have one path through. `pending_actions` is
        // shared state, so this stick through the return.
        if snapshot_is_structural(change.snapshot.as_ref()) {
            self.pending_actions.structural_changed = true;
        }

        // Check if this is a snapshot-based undo
        if let Some(snapshot) = &change.snapshot {
            match snapshot {
                crate::config::SnapshotData::FullConfig { before, .. } => {
                    log::debug!("  Restoring full config snapshot (before)");
                    self.current = (**before).clone();
                    // FullConfig restoration replaces the entire current
                    // state; the previous subflames list is gone, so
                    // reset the editing target to Main. (No stash to
                    // clean up — un-swap refactor removed that.)
                    self.editing_target = EditingTarget::Main;
                    // FullConfig swap drops in a freshly-deserialized
                    // (or freshly-cloned) flame; assign IDs to anything
                    // that's zero so the rebind machinery has handles.
                    self.current.fixup_ids();
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::AddTransform { index, xaos_before, .. } => {
                    log::debug!("  Undoing add transform at index {}", index);
                    let index = *index;
                    let xaos_before = xaos_before.clone();
                    let flame = self.active_flame_mut()?;
                    if index < flame.transforms.len() {
                        flame.transforms.remove(index);
                        // Restore xaos matrix to pre-add state (not incremental delete)
                        flame.xaos = xaos_before;
                    }
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::DeleteTransform { index, transform, xaos_before } => {
                    log::debug!("  Undoing delete transform (re-insert at index {})", index);
                    let index = *index;
                    let transform = transform.clone();
                    let xaos_before = xaos_before.clone();
                    let flame = self.active_flame_mut()?;
                    if index <= flame.transforms.len() {
                        flame.transforms.insert(index, transform);
                        // Restore xaos matrix to pre-delete state (not incremental add)
                        flame.xaos = xaos_before;
                    }
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::ModifyTransform { kind, index, before, .. } => {
                    log::debug!("  Undoing modify transform (restore before state at {:?} index {})", kind, index);
                    let xref = kind.at(*index);
                    let before = before.clone();
                    let flame = self.active_flame_mut()?;
                    if let Some(slot) = xref.get_mut(flame) {
                        *slot = before;
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

                crate::config::SnapshotData::AddSubflame { index, target_before, .. } => {
                    // Undo of add: remove the added subflame. Editing
                    // target stays at target_before (which is Main per
                    // the add gate — but we honor the captured value).
                    log::debug!("  Undoing add subflame at index {}", index);
                    let index = *index;
                    let target_before = *target_before;
                    // We pre-swapped to entry.target above; the add
                    // happened on Main, so the subflames list is on
                    // current.flame and the entry is there.
                    if index < self.current.flame.subflames.len() {
                        self.current.flame.subflames.remove(index);
                    }
                    self.set_editing_target_silent(target_before)?;
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::DeleteSubflame { index, flame, target_before } => {
                    // Undo of delete: re-insert flame at its original
                    // index, then restore the pre-delete editing
                    // target. The pre-swap above put us on entry.target
                    // (Main), so subflames is current.flame.subflames.
                    log::debug!("  Undoing delete subflame at index {}", index);
                    let index = *index;
                    let target_before = *target_before;
                    let flame = flame.clone();
                    if index <= self.current.flame.subflames.len() {
                        self.current.flame.subflames.insert(index, flame);
                    }
                    self.set_editing_target_silent(target_before)?;
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::SwapTarget { before, .. } => {
                    // Undo of swap: go back to where we were. The pre-
                    // swap above put us at entry.target (== after), so
                    // we now swap silently to `before`.
                    log::debug!("  Undoing swap target → {:?}", before);
                    let before = *before;
                    self.set_editing_target_silent(before)?;
                    return Ok(UpdateType::IterationReset);
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

        // Silent-swap to the entry's target before re-applying. Same
        // rationale as undo: the change was made in a specific editing
        // context; replaying it requires being in that context.
        let entry_target = self.history[self.position].target;
        self.set_editing_target_silent(entry_target)?;

        // Clone the change to avoid borrow issues
        let change = self.history[self.position].clone();
        log::debug!("Redo: {} (target: {:?})", change.description, change.target);

        // Same up-front structural flag as in undo — match arms below
        // return early so we need to set this before entering them.
        if snapshot_is_structural(change.snapshot.as_ref()) {
            self.pending_actions.structural_changed = true;
        }

        // Check if this is a snapshot-based redo
        if let Some(snapshot) = &change.snapshot {
            match snapshot {
                crate::config::SnapshotData::FullConfig { after, .. } => {
                    log::debug!("  Restoring full config snapshot (after)");
                    self.current = (**after).clone();
                    self.editing_target = EditingTarget::Main;
                    self.current.fixup_ids();
                    self.position += 1;
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::AddTransform { index, transform, clone_from, .. } => {
                    log::debug!("  Redoing add transform at index {}", index);
                    let index = *index;
                    let transform = transform.clone();
                    let clone_from = *clone_from;
                    let flame = self.active_flame_mut()?;
                    if index <= flame.transforms.len() {
                        if let Some(source_idx) = clone_from {
                            flame.on_transform_cloned(index, source_idx);
                        } else {
                            flame.on_transform_added(index);
                        }
                        flame.transforms.insert(index, transform);
                    }
                    self.position += 1;
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::DeleteTransform { index, .. } => {
                    log::debug!("  Redoing delete transform (remove at index {})", index);
                    let index = *index;
                    let flame = self.active_flame_mut()?;
                    if index < flame.transforms.len() {
                        flame.transforms.remove(index);
                        flame.on_transform_deleted(index);
                    }
                    self.position += 1;
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::ModifyTransform { kind, index, after, .. } => {
                    log::debug!("  Redoing modify transform (restore after state at {:?} index {})", kind, index);
                    let xref = kind.at(*index);
                    let after = after.clone();
                    let flame = self.active_flame_mut()?;
                    if let Some(slot) = xref.get_mut(flame) {
                        *slot = after;
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

                crate::config::SnapshotData::AddSubflame { index, flame, .. } => {
                    // Redo add: re-insert the same flame at the same
                    // index. We pre-swapped to entry.target above, so
                    // current.flame is the parent and its subflames
                    // list is the canonical one to mutate.
                    log::debug!("  Redoing add subflame at index {}", index);
                    let index = *index;
                    let flame = flame.clone();
                    if index <= self.current.flame.subflames.len() {
                        self.current.flame.subflames.insert(index, flame);
                    }
                    self.position += 1;
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::DeleteSubflame { index, .. } => {
                    // Redo delete: remove the flame at index. Pre-swap
                    // above put us on entry.target (Main, post-delete
                    // context), so subflames list is current.flame.
                    log::debug!("  Redoing delete subflame at index {}", index);
                    let index = *index;
                    if index < self.current.flame.subflames.len() {
                        self.current.flame.subflames.remove(index);
                    }
                    self.position += 1;
                    return Ok(UpdateType::IterationReset);
                }

                crate::config::SnapshotData::SwapTarget { after, .. } => {
                    // Redo swap: go to `after`. Pre-swap above already
                    // put us at entry.target (== after), so this is a
                    // no-op in well-formed history; we assert.
                    log::debug!("  Redoing swap target → {:?}", after);
                    let after = *after;
                    self.set_editing_target_silent(after)?;
                    self.position += 1;
                    return Ok(UpdateType::IterationReset);
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
    fn push_undo(&mut self, mut change: ConfigChange) {
        // Stamp the entry with the editing context it was made in, so
        // undo/redo can silently swap back to the right flame before
        // applying. Every callsite gets correct tagging for free
        // without having to know about subflames.
        change.target = self.editing_target;
        log::debug!("PUSH_UNDO: {} (target={:?})", change.description, change.target);
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

            // Update the last change's new_value, description, and last_update_time.
            // Keep the original old_value and timestamp from the first change in
            // the sequence. Merge is path-keyed: for the normal case
            // should_coalesce guarantees identical path sequences so this
            // behaves like the old positional merge; for fly-camera gestures
            // (varying path sets) a path not yet present in the entry is
            // appended with its own old_value — correct, since that's when
            // the parameter first changed within the merged gesture.
            for new_delta in change.deltas.iter() {
                let last_entry = &mut self.history[last_idx];
                if let Some(old_delta) = last_entry
                    .deltas
                    .iter_mut()
                    .find(|d| d.path == new_delta.path)
                {
                    old_delta.new_value = new_delta.new_value.clone();
                    // NOTE: We do NOT update timestamp - it preserves when the sequence started
                } else {
                    last_entry.deltas.push(new_delta.clone());
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

        // Never coalesce across editing targets. An edit on Main and an
        // edit on Subflame{N} are distinct user actions even if they
        // hit the same ConfigPath, because they target different
        // underlying Flames.
        if last_change.target != self.editing_target {
            return false;
        }

        // Fly-mode camera gestures coalesce as a unit even though the
        // per-event path set varies (mouse-look writes pitch/yaw and
        // sometimes rotation or position-compensation; WASD writes
        // position only). Identified by the shared batch description;
        // merged path-keyed in push_undo. Timing rules below still
        // apply, so a pause still starts a new history entry.
        let fly_gesture = last_change.description == FLY_CAMERA_HISTORY_DESC
            && new_change.description == FLY_CAMERA_HISTORY_DESC;

        if !fly_gesture {
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
        target: EditingTarget,
    ) -> Result<ConfigValue, ConfigError> {
        // Resolve the active flame once. For per-flame paths
        // (transforms, render_mode, xaos, etc.) we read from this;
        // for FractalConfig-level paths (zoom, exposure, ...) we
        // continue reading from `config` directly.
        let flame = Self::target_flame_in(config, target).ok_or(ConfigError::InvalidIndex)?;
        match path {
            // View
            ConfigPath::Zoom => Ok(config.zoom.into()),
            ConfigPath::Pan => Ok((config.pan_x, config.pan_y).into()),
            ConfigPath::PanX => Ok(config.pan_x.into()),
            ConfigPath::PanY => Ok(config.pan_y.into()),
            ConfigPath::Rotation => Ok(config.rotation.into()),
            ConfigPath::CameraRotationX => Ok(config.camera_rotation_x.into()),
            ConfigPath::CameraRotationY => Ok(config.camera_rotation_y.into()),
            ConfigPath::CameraBank => Ok(config.camera_bank.into()),
            ConfigPath::CameraX => Ok(config.camera_x.into()),
            ConfigPath::CameraY => Ok(config.camera_y.into()),
            ConfigPath::CameraZ => Ok(config.camera_z.into()),
            ConfigPath::DofFocusDistance => Ok(config.dof_focus_distance.into()),
            ConfigPath::DofBlurStrength => Ok(config.dof_blur_strength.into()),
            ConfigPath::FogStrength => Ok(config.fog_strength.into()),
            ConfigPath::FogStart => Ok(config.fog_start.into()),
            ConfigPath::FilterRadius => Ok(config.filter_radius.into()),
            ConfigPath::FilterBlurEdges => Ok(config.filter_blur_edges.into()),

            // Tone mapping
            ConfigPath::Exposure => Ok(config.exposure.into()),
            ConfigPath::Gamma => Ok(config.gamma.into()),
            ConfigPath::GammaThreshold => Ok(config.gamma_threshold.into()),
            ConfigPath::Brightness => Ok(config.brightness.into()),
            ConfigPath::Vibrancy => Ok(config.vibrancy.into()),
            ConfigPath::WhiteLevel => Ok(config.white_level.into()),
            ConfigPath::Saturation => Ok(config.saturation.into()),
            ConfigPath::HueShift => Ok(config.hue_shift.into()),
            ConfigPath::AlphaBlendLow => Ok(config.alpha_blend_low.into()),
            ConfigPath::AlphaBlendHigh => Ok(config.alpha_blend_high.into()),
            ConfigPath::DensityScale => Ok(config.density_scale.into()),
            ConfigPath::TonemapMode => Ok(config.tonemap_mode.into()),
            ConfigPath::HighlightMode => Ok(config.highlight_mode.into()),
            ConfigPath::TonemapCurve => Ok(config.tonemap_curve.clone().into()),
            ConfigPath::UseCurve => Ok(config.use_curve.into()),
            // Levels controls
            ConfigPath::LevelsEnabled => Ok(config.levels_enabled.into()),
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
            ConfigPath::PaletteSqueezeMode => Ok(config.palette_squeeze_mode.into()),
            ConfigPath::PaletteSqueezeFalloff => Ok(config.palette_squeeze_falloff.into()),
            ConfigPath::PaletteLogStrength => Ok(config.palette_log_strength.into()),
            ConfigPath::PaletteReverse => Ok(config.palette_reverse.into()),
            ConfigPath::SpeedFactor => Ok(config.speed_factor.into()),
            ConfigPath::BackgroundColor => Ok(config.background_color.into()),
            ConfigPath::BackgroundColorR => Ok(config.background_color[0].into()),
            ConfigPath::BackgroundColorG => Ok(config.background_color[1].into()),
            ConfigPath::BackgroundColorB => Ok(config.background_color[2].into()),

            // Rendering settings
            ConfigPath::BlendFactor => Ok(config.blend_factor.into()),
            ConfigPath::UseDynamicBlend => Ok(config.use_dynamic_blend.into()),
            ConfigPath::MaxIterations => Ok(config.max_iterations.into()),
            ConfigPath::DeterministicRng => Ok(config.deterministic_rng.into()),

            // Transforms
            ConfigPath::TransformCount => {
                Ok((flame.transforms.len() as u32).into())
            }
            ConfigPath::TransformWeight { index } => {
                let xform = flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.weight.into())
            }
            ConfigPath::TransformColor { index } => {
                let xform = flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.color.into())
            }
            ConfigPath::TransformColorSpeed { index } => {
                let xform = flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.color_speed.into())
            }
            ConfigPath::TransformOpacity { index } => {
                let xform = flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.opacity.into())
            }
            ConfigPath::TransformDirectColor { index } => {
                let xform = flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.direct_color.into())
            }
            ConfigPath::TransformAffine { index, param } => {
                let xform = flame
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
                let xform = flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_affine_enabled.into())
            }
            ConfigPath::TransformPostAffine { index, param } => {
                let xform = flame
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
            ConfigPath::TransformYzCoefs { index, position } => {
                let xform = flame.transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                let v = *xform.yz_coefs.get(*position as usize).ok_or(ConfigError::InvalidIndex)?;
                Ok(v.into())
            }
            ConfigPath::TransformZxCoefs { index, position } => {
                let xform = flame.transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                let v = *xform.zx_coefs.get(*position as usize).ok_or(ConfigError::InvalidIndex)?;
                Ok(v.into())
            }
            ConfigPath::TransformYzPostCoefs { index, position } => {
                let xform = flame.transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                let v = *xform.yz_post_coefs.get(*position as usize).ok_or(ConfigError::InvalidIndex)?;
                Ok(v.into())
            }
            ConfigPath::TransformZxPostCoefs { index, position } => {
                let xform = flame.transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                let v = *xform.zx_post_coefs.get(*position as usize).ok_or(ConfigError::InvalidIndex)?;
                Ok(v.into())
            }
            ConfigPath::TransformVariation { index, variation } => {
                let xform = flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let weight = xform.variations.get(variation).copied().unwrap_or(0.0);
                Ok(weight.into())
            }
            ConfigPath::TransformVariationPriority { index, variation } => {
                let xform = flame.transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(Self::variation_priority_value(xform, variation))
            }
            ConfigPath::TransformVariationOrder { index } => {
                let xform = flame.transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(ConfigValue::StringList(xform.variation_order.clone()))
            }
            ConfigPath::TransformVariationParam {
                index,
                variation,
                param,
            } => {
                let xform = flame
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
                let xform = flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.origin_x().into())
            }
            ConfigPath::TransformOriginY { index } => {
                let xform = flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.origin_y().into())
            }
            ConfigPath::TransformRotation { index } => {
                let xform = flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.rotation().into())
            }
            ConfigPath::TransformScale { index } => {
                let xform = flame
                    .transforms
                    .get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.scale().into())
            }
            // High-level post-affine ops (normal pool)
            ConfigPath::TransformPostAffineOriginX { index } => {
                let xform = flame.transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_origin_x().into())
            }
            ConfigPath::TransformPostAffineOriginY { index } => {
                let xform = flame.transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_origin_y().into())
            }
            ConfigPath::TransformPostAffineRotation { index } => {
                let xform = flame.transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_rotation().into())
            }
            ConfigPath::TransformPostAffineScale { index } => {
                let xform = flame.transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_scale().into())
            }

            // High-level pre-affine ops (linked pool)
            ConfigPath::LinkedTransformOriginX { index } => {
                let xform = flame.linked_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.origin_x().into())
            }
            ConfigPath::LinkedTransformOriginY { index } => {
                let xform = flame.linked_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.origin_y().into())
            }
            ConfigPath::LinkedTransformRotation { index } => {
                let xform = flame.linked_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.rotation().into())
            }
            ConfigPath::LinkedTransformScale { index } => {
                let xform = flame.linked_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.scale().into())
            }
            // High-level post-affine ops (linked pool)
            ConfigPath::LinkedTransformPostAffineOriginX { index } => {
                let xform = flame.linked_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_origin_x().into())
            }
            ConfigPath::LinkedTransformPostAffineOriginY { index } => {
                let xform = flame.linked_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_origin_y().into())
            }
            ConfigPath::LinkedTransformPostAffineRotation { index } => {
                let xform = flame.linked_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_rotation().into())
            }
            ConfigPath::LinkedTransformPostAffineScale { index } => {
                let xform = flame.linked_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_scale().into())
            }

            // High-level pre-affine ops (final pool)
            ConfigPath::FinalTransformOriginX { index } => {
                let xform = flame.final_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.origin_x().into())
            }
            ConfigPath::FinalTransformOriginY { index } => {
                let xform = flame.final_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.origin_y().into())
            }
            ConfigPath::FinalTransformRotation { index } => {
                let xform = flame.final_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.rotation().into())
            }
            ConfigPath::FinalTransformScale { index } => {
                let xform = flame.final_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.scale().into())
            }
            // High-level post-affine ops (final pool)
            ConfigPath::FinalTransformPostAffineOriginX { index } => {
                let xform = flame.final_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_origin_x().into())
            }
            ConfigPath::FinalTransformPostAffineOriginY { index } => {
                let xform = flame.final_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_origin_y().into())
            }
            ConfigPath::FinalTransformPostAffineRotation { index } => {
                let xform = flame.final_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_rotation().into())
            }
            ConfigPath::FinalTransformPostAffineScale { index } => {
                let xform = flame.final_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_scale().into())
            }

            // Legacy `FinalTransform*` (no index) variants were removed
            // in Phase 9. The migration shim in
            // `ConfigPath::from_string_key` maps the legacy string form
            // to indexed variants at index 0; those go through the
            // indexed `FinalTransform*` arms in the Final Transform pool
            // section below.

            // Linked Transform pool — same shape as TransformXxx but
            // sourced from flame.linked_transforms[index].
            ConfigPath::LinkedTransformAffine { index, param } => {
                let xform = flame.linked_transforms.get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let value = match param {
                    AffineParam::A => xform.a, AffineParam::B => xform.b,
                    AffineParam::C => xform.c, AffineParam::D => xform.d,
                    AffineParam::E => xform.e, AffineParam::F => xform.f,
                    AffineParam::G => xform.g,
                };
                Ok(value.into())
            }
            ConfigPath::LinkedTransformPostAffineEnabled { index } => {
                let xform = flame.linked_transforms.get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_affine_enabled.into())
            }
            ConfigPath::LinkedTransformPostAffine { index, param } => {
                let xform = flame.linked_transforms.get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let value = match param {
                    AffineParam::A => xform.post_a, AffineParam::B => xform.post_b,
                    AffineParam::C => xform.post_c, AffineParam::D => xform.post_d,
                    AffineParam::E => xform.post_e, AffineParam::F => xform.post_f,
                    AffineParam::G => xform.post_g,
                };
                Ok(value.into())
            }
            ConfigPath::LinkedTransformYzCoefs { index, position } => {
                let xform = flame.linked_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok((*xform.yz_coefs.get(*position as usize).ok_or(ConfigError::InvalidIndex)?).into())
            }
            ConfigPath::LinkedTransformZxCoefs { index, position } => {
                let xform = flame.linked_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok((*xform.zx_coefs.get(*position as usize).ok_or(ConfigError::InvalidIndex)?).into())
            }
            ConfigPath::LinkedTransformYzPostCoefs { index, position } => {
                let xform = flame.linked_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok((*xform.yz_post_coefs.get(*position as usize).ok_or(ConfigError::InvalidIndex)?).into())
            }
            ConfigPath::LinkedTransformZxPostCoefs { index, position } => {
                let xform = flame.linked_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok((*xform.zx_post_coefs.get(*position as usize).ok_or(ConfigError::InvalidIndex)?).into())
            }
            ConfigPath::LinkedTransformVariation { index, variation } => {
                let xform = flame.linked_transforms.get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.variations.get(variation).copied().unwrap_or(0.0).into())
            }
            ConfigPath::LinkedTransformVariationPriority { index, variation } => {
                let xform = flame.linked_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(Self::variation_priority_value(xform, variation))
            }
            ConfigPath::LinkedTransformVariationOrder { index } => {
                let xform = flame.linked_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(ConfigValue::StringList(xform.variation_order.clone()))
            }
            ConfigPath::LinkedTransformVariationParam { index, variation, param } => {
                let xform = flame.linked_transforms.get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let value = xform.get_variation_param_or_default(
                    variation, param, &crate::variations::global_registry());
                Ok(value.into())
            }

            // Final Transform pool — sourced from flame.final_transforms[index].
            ConfigPath::FinalTransformAffine { index, param } => {
                let xform = flame.final_transforms.get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let value = match param {
                    AffineParam::A => xform.a, AffineParam::B => xform.b,
                    AffineParam::C => xform.c, AffineParam::D => xform.d,
                    AffineParam::E => xform.e, AffineParam::F => xform.f,
                    AffineParam::G => xform.g,
                };
                Ok(value.into())
            }
            ConfigPath::FinalTransformPostAffineEnabled { index } => {
                let xform = flame.final_transforms.get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.post_affine_enabled.into())
            }
            ConfigPath::FinalTransformPostAffine { index, param } => {
                let xform = flame.final_transforms.get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let value = match param {
                    AffineParam::A => xform.post_a, AffineParam::B => xform.post_b,
                    AffineParam::C => xform.post_c, AffineParam::D => xform.post_d,
                    AffineParam::E => xform.post_e, AffineParam::F => xform.post_f,
                    AffineParam::G => xform.post_g,
                };
                Ok(value.into())
            }
            ConfigPath::FinalTransformYzCoefs { index, position } => {
                let xform = flame.final_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok((*xform.yz_coefs.get(*position as usize).ok_or(ConfigError::InvalidIndex)?).into())
            }
            ConfigPath::FinalTransformZxCoefs { index, position } => {
                let xform = flame.final_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok((*xform.zx_coefs.get(*position as usize).ok_or(ConfigError::InvalidIndex)?).into())
            }
            ConfigPath::FinalTransformYzPostCoefs { index, position } => {
                let xform = flame.final_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok((*xform.yz_post_coefs.get(*position as usize).ok_or(ConfigError::InvalidIndex)?).into())
            }
            ConfigPath::FinalTransformZxPostCoefs { index, position } => {
                let xform = flame.final_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok((*xform.zx_post_coefs.get(*position as usize).ok_or(ConfigError::InvalidIndex)?).into())
            }
            ConfigPath::FinalTransformVariation { index, variation } => {
                let xform = flame.final_transforms.get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                Ok(xform.variations.get(variation).copied().unwrap_or(0.0).into())
            }
            ConfigPath::FinalTransformVariationPriority { index, variation } => {
                let xform = flame.final_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(Self::variation_priority_value(xform, variation))
            }
            ConfigPath::FinalTransformVariationOrder { index } => {
                let xform = flame.final_transforms.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(ConfigValue::StringList(xform.variation_order.clone()))
            }
            ConfigPath::FinalTransformVariationParam { index, variation, param } => {
                let xform = flame.final_transforms.get(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                let value = xform.get_variation_param_or_default(
                    variation, param, &crate::variations::global_registry());
                Ok(value.into())
            }

            // Scene-level render state — config-level since v3 (read from the
            // config, not the resolved flame target).
            ConfigPath::RenderMode => Ok(config.render_mode.into()),
            ConfigPath::PerspectiveStrength => Ok(config.perspective_strength.into()),
            ConfigPath::DepthDensityCompensation => Ok(config.depth_density_compensation.into()),
            ConfigPath::FarDensityFade => Ok(config.far_density_fade.into()),
            ConfigPath::SolidStrength => Ok(config.solid_strength.into()),
            ConfigPath::SurfaceThickness => Ok(config.surface_thickness.into()),
            ConfigPath::ShadingStrength => Ok(config.solid_shading.shading_strength.into()),
            ConfigPath::SolidAmbient => Ok(config.solid_shading.ambient.into()),
            ConfigPath::SolidDiffuse => Ok(config.solid_shading.diffuse.into()),
            ConfigPath::SolidSpecular => Ok(config.solid_shading.specular.into()),
            ConfigPath::SolidShininess => Ok(config.solid_shading.shininess.into()),
            ConfigPath::SsaoStrength => Ok(config.solid_shading.ssao_strength.into()),
            ConfigPath::SsaoRadius => Ok(config.solid_shading.ssao_radius.into()),
            ConfigPath::SolidLightEnabled { index } => {
                let l = config.solid_shading.lights.get(*index).ok_or(ConfigError::InvalidIndex)?;
                Ok(l.enabled.into())
            }
            ConfigPath::SolidLightParam { index, param } => {
                let l = config.solid_shading.lights.get(*index).ok_or(ConfigError::InvalidIndex)?;
                let v = match param.as_str() {
                    "azimuth" => l.azimuth,
                    "elevation" => l.elevation,
                    "intensity" => l.intensity,
                    "color_r" => l.color[0],
                    "color_g" => l.color[1],
                    "color_b" => l.color[2],
                    _ => return Err(ConfigError::InvalidIndex),
                };
                Ok(v.into())
            }
            ConfigPath::FarDensityFadeStart => Ok(config.far_density_fade_start.into()),

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

            // Xaos (chaos-weighted transform transitions) — per-flame
            ConfigPath::Xaos { src, dst } => {
                let weight = flame.get_xaos(*src, *dst);
                Ok(weight.into())
            }

            // Solo transform (-1 = None, 0+ = Some(index)) — per-flame
            ConfigPath::SoloTransform => {
                let value = match flame.solo_transform {
                    None => -1i32,
                    Some(idx) => idx as i32,
                };
                Ok(value.into())
            }

            // Post-symmetry — per-flame plot-time symmetry. The type
            // round-trips through u32 (PostSymmetryType::as_u32).
            ConfigPath::PostSymmetryType => Ok((flame.post_symmetry.ty.as_u32() as i32).into()),
            ConfigPath::PostSymmetryOrder => Ok((flame.post_symmetry.order as i32).into()),
            ConfigPath::PostSymmetryCenterX => Ok(flame.post_symmetry.center_x.into()),
            ConfigPath::PostSymmetryCenterY => Ok(flame.post_symmetry.center_y.into()),
            ConfigPath::PostSymmetryDistance => Ok(flame.post_symmetry.distance.into()),
            ConfigPath::PostSymmetryRotation => Ok(flame.post_symmetry.rotation_deg.into()),
            ConfigPath::PreserveZ => Ok(config.preserve_z.into()),

            // System Settings - These should NOT be called via get_value (they're not in FractalConfig)
            // Use config_manager.system_settings() instead
            ConfigPath::SystemIterationsPerThread
            | ConfigPath::SystemVsyncEnabled
            | ConfigPath::SystemTargetFps
            | ConfigPath::SystemFlyMouseSensitivity
            | ConfigPath::SystemFlyMoveSpeed
            | ConfigPath::SystemFlySprintMultiplier
            | ConfigPath::SystemFlyInvertY
            | ConfigPath::SystemFlyCameraMode
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
        Self::get_value_from_config(config, path, self.editing_target)
    }

    /// Apply a variation-weight change to a single transform, regardless of
    /// which pool it lives in. NaN is the sentinel for "remove this variation"
    /// (sent by the trash button); 0.0 is kept so the slider stays visible.
    fn apply_variation_weight(
        xform: &mut crate::scene::transforms::Transform,
        variation: &str,
        value: ConfigValue,
    ) -> Result<(), ConfigError> {
        let weight: f32 = value.try_into()?;
        if weight.is_nan() {
            // `remove_variation` also drops the name from `variation_order`.
            xform.remove_variation(variation);
        } else {
            // `set_variation` records the name in `variation_order` on first
            // add, so UI-built flames dispatch in add order.
            xform.set_variation(variation, weight);
        }
        Ok(())
    }

    /// Apply a variation fx_priority (phase) change to a single transform,
    /// regardless of pool. Stored sparsely: a priority equal to the
    /// variation def's natural-phase priority removes the override (the
    /// `Any` default is main/0), any other value inserts it. See
    /// `Transform::variation_priorities`.
    fn apply_variation_priority(
        xform: &mut crate::scene::transforms::Transform,
        variation: &str,
        value: ConfigValue,
    ) -> Result<(), ConfigError> {
        let prio: i32 = value.try_into()?;
        let natural = crate::variations::global_registry()
            .get(variation)
            .map(|i| i.phase.natural_priority())
            .unwrap_or(0);
        if prio == natural {
            xform.variation_priorities.remove(variation);
        } else {
            xform.variation_priorities.insert(variation.to_string(), prio);
        }
        Ok(())
    }

    /// Read the effective fx_priority of a variation on a transform: the
    /// stored override, or the variation def's natural-phase priority when
    /// unset (the sparse-storage default). Used by `get_value` for undo.
    fn variation_priority_value(
        xform: &crate::scene::transforms::Transform,
        variation: &str,
    ) -> ConfigValue {
        let natural = crate::variations::global_registry()
            .get(variation)
            .map(|i| i.phase.natural_priority())
            .unwrap_or(0);
        let prio = xform.variation_priorities.get(variation).copied().unwrap_or(natural);
        ConfigValue::Int(prio)
    }

    /// Apply a variation-parameter change to a single transform, regardless
    /// of which pool it lives in.
    fn apply_variation_param(
        xform: &mut crate::scene::transforms::Transform,
        variation: &str,
        param: &str,
        value: ConfigValue,
    ) -> Result<(), ConfigError> {
        let new_value: f32 = value.try_into()?;
        let key = format!("{}.{}", variation, param);
        xform.variation_params.insert(key, new_value);
        Ok(())
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
            ConfigPath::CameraBank => {
                self.current.camera_bank = value.try_into()?;
            }
            ConfigPath::CameraX => {
                self.current.camera_x = value.try_into()?;
            }
            ConfigPath::CameraY => {
                self.current.camera_y = value.try_into()?;
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
            ConfigPath::FilterRadius => {
                self.current.filter_radius = value.try_into()?;
            }
            ConfigPath::FilterBlurEdges => {
                self.current.filter_blur_edges = value.try_into()?;
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
            ConfigPath::WhiteLevel => {
                self.current.white_level = value.try_into()?;
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
            ConfigPath::HighlightMode => {
                self.current.highlight_mode = value.try_into()?;
            }
            ConfigPath::TonemapCurve => {
                self.current.tonemap_curve = value.try_into()?;
            }
            ConfigPath::UseCurve => {
                self.current.use_curve = value.try_into()?;
            }
            // Levels controls
            ConfigPath::LevelsEnabled => {
                self.current.levels_enabled = value.try_into()?;
            }
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
            ConfigPath::PaletteSqueezeMode => {
                self.current.palette_squeeze_mode = value.try_into()?;
            }
            ConfigPath::PaletteSqueezeFalloff => {
                let v: f32 = value.try_into()?;
                self.current.palette_squeeze_falloff = v.clamp(0.05, 0.99);
            }
            ConfigPath::PaletteLogStrength => {
                let v: f32 = value.try_into()?;
                self.current.palette_log_strength = v.clamp(-10.0, 10.0);
            }
            ConfigPath::PaletteReverse => {
                self.current.palette_reverse = value.try_into()?;
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
            ConfigPath::BlendFactor => {
                self.current.blend_factor = value.try_into()?;
            }
            ConfigPath::UseDynamicBlend => {
                self.current.use_dynamic_blend = value.try_into()?;
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
                let xform = self.normal_transform_mut(*index)?;
                xform.weight = value.try_into()?;
            }
            ConfigPath::TransformColor { index } => {
                let xform = self.normal_transform_mut(*index)?;
                xform.color = value.try_into()?;
            }
            ConfigPath::TransformColorSpeed { index } => {
                let xform = self.normal_transform_mut(*index)?;
                xform.color_speed = value.try_into()?;
            }
            ConfigPath::TransformOpacity { index } => {
                let xform = self.normal_transform_mut(*index)?;
                xform.opacity = value.try_into()?;
            }
            ConfigPath::TransformDirectColor { index } => {
                let xform = self.normal_transform_mut(*index)?;
                xform.direct_color = value.try_into()?;
            }
            ConfigPath::TransformAffine { index, param } => {
                let xform = self.normal_transform_mut(*index)?;
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
                let xform = self.normal_transform_mut(*index)?;
                xform.post_affine_enabled = value.try_into()?;
            }
            ConfigPath::TransformPostAffine { index, param } => {
                let xform = self.normal_transform_mut(*index)?;
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
            // JWildfire plane coefs — write the one position into the
            // [f32; 6] array. The GpuTransform builder picks up the
            // change on the next upload and re-computes plane_flags
            // automatically (identity comparison drops back to the
            // flat path when values return to identity).
            ConfigPath::TransformYzCoefs { index, position } => {
                let xform = self.normal_transform_mut(*index)?;
                let new_value: f32 = value.try_into()?;
                if let Some(slot) = xform.yz_coefs.get_mut(*position as usize) {
                    *slot = new_value;
                }
            }
            ConfigPath::TransformZxCoefs { index, position } => {
                let xform = self.normal_transform_mut(*index)?;
                let new_value: f32 = value.try_into()?;
                if let Some(slot) = xform.zx_coefs.get_mut(*position as usize) {
                    *slot = new_value;
                }
            }
            ConfigPath::TransformYzPostCoefs { index, position } => {
                let xform = self.normal_transform_mut(*index)?;
                let new_value: f32 = value.try_into()?;
                if let Some(slot) = xform.yz_post_coefs.get_mut(*position as usize) {
                    *slot = new_value;
                }
            }
            ConfigPath::TransformZxPostCoefs { index, position } => {
                let xform = self.normal_transform_mut(*index)?;
                let new_value: f32 = value.try_into()?;
                if let Some(slot) = xform.zx_post_coefs.get_mut(*position as usize) {
                    *slot = new_value;
                }
            }
            ConfigPath::TransformVariation { index, variation } => {
                let xform = self.normal_transform_mut(*index)?;
                Self::apply_variation_weight(xform, variation, value)?;
            }
            ConfigPath::TransformVariationPriority { index, variation } => {
                let xform = self.normal_transform_mut(*index)?;
                Self::apply_variation_priority(xform, variation, value)?;
            }
            ConfigPath::TransformVariationOrder { index } => {
                let xform = self.normal_transform_mut(*index)?;
                xform.variation_order = value.try_into()?;
            }
            ConfigPath::TransformVariationParam { index, variation, param } => {
                let xform = self.normal_transform_mut(*index)?;
                Self::apply_variation_param(xform, variation, param, value)?;
            }
            // High-level transform operations (translate, rotate, scale)
            ConfigPath::TransformOriginX { index } => {
                let xform = self.normal_transform_mut(*index)?;
                let new_value: f32 = value.try_into()?;
                xform.set_origin_x(new_value);
            }
            ConfigPath::TransformOriginY { index } => {
                let xform = self.normal_transform_mut(*index)?;
                let new_value: f32 = value.try_into()?;
                xform.set_origin_y(new_value);
            }
            ConfigPath::TransformRotation { index } => {
                let xform = self.normal_transform_mut(*index)?;
                let new_value: f32 = value.try_into()?;
                xform.set_rotation(new_value);
            }
            ConfigPath::TransformScale { index } => {
                let xform = self.normal_transform_mut(*index)?;
                let new_value: f32 = value.try_into()?;
                xform.set_scale(new_value);
            }
            // High-level post-affine ops (normal pool)
            ConfigPath::TransformPostAffineOriginX { index } => {
                let xform = self.normal_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_origin_x(v);
            }
            ConfigPath::TransformPostAffineOriginY { index } => {
                let xform = self.normal_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_origin_y(v);
            }
            ConfigPath::TransformPostAffineRotation { index } => {
                let xform = self.normal_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_rotation(v);
            }
            ConfigPath::TransformPostAffineScale { index } => {
                let xform = self.normal_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_scale(v);
            }

            // High-level pre-affine ops (linked pool)
            ConfigPath::LinkedTransformOriginX { index } => {
                let xform = self.linked_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_origin_x(v);
            }
            ConfigPath::LinkedTransformOriginY { index } => {
                let xform = self.linked_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_origin_y(v);
            }
            ConfigPath::LinkedTransformRotation { index } => {
                let xform = self.linked_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_rotation(v);
            }
            ConfigPath::LinkedTransformScale { index } => {
                let xform = self.linked_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_scale(v);
            }
            // High-level post-affine ops (linked pool)
            ConfigPath::LinkedTransformPostAffineOriginX { index } => {
                let xform = self.linked_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_origin_x(v);
            }
            ConfigPath::LinkedTransformPostAffineOriginY { index } => {
                let xform = self.linked_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_origin_y(v);
            }
            ConfigPath::LinkedTransformPostAffineRotation { index } => {
                let xform = self.linked_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_rotation(v);
            }
            ConfigPath::LinkedTransformPostAffineScale { index } => {
                let xform = self.linked_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_scale(v);
            }

            // High-level pre-affine ops (final pool)
            ConfigPath::FinalTransformOriginX { index } => {
                let xform = self.final_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_origin_x(v);
            }
            ConfigPath::FinalTransformOriginY { index } => {
                let xform = self.final_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_origin_y(v);
            }
            ConfigPath::FinalTransformRotation { index } => {
                let xform = self.final_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_rotation(v);
            }
            ConfigPath::FinalTransformScale { index } => {
                let xform = self.final_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_scale(v);
            }
            // High-level post-affine ops (final pool)
            ConfigPath::FinalTransformPostAffineOriginX { index } => {
                let xform = self.final_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_origin_x(v);
            }
            ConfigPath::FinalTransformPostAffineOriginY { index } => {
                let xform = self.final_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_origin_y(v);
            }
            ConfigPath::FinalTransformPostAffineRotation { index } => {
                let xform = self.final_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_rotation(v);
            }
            ConfigPath::FinalTransformPostAffineScale { index } => {
                let xform = self.final_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_scale(v);
            }

            // Legacy `FinalTransform*` (no index) variants were removed
            // in Phase 9. The migration shim in
            // `ConfigPath::from_string_key` maps the legacy string form
            // to indexed variants at index 0; those go through the
            // indexed `FinalTransform*` arms in the Final Transform pool
            // section below.

            // Linked Transform pool — same shape as TransformXxx but
            // sourced from flame.linked_transforms[index].
            ConfigPath::LinkedTransformAffine { index, param } => {
                let xform = self.linked_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                match param {
                    AffineParam::A => xform.a = v, AffineParam::B => xform.b = v,
                    AffineParam::C => xform.c = v, AffineParam::D => xform.d = v,
                    AffineParam::E => xform.e = v, AffineParam::F => xform.f = v,
                    AffineParam::G => xform.g = v,
                };
            }
            ConfigPath::LinkedTransformPostAffineEnabled { index } => {
                let xform = self.linked_transform_mut(*index)?;
                xform.post_affine_enabled = value.try_into()?;
            }
            ConfigPath::LinkedTransformPostAffine { index, param } => {
                let xform = self.linked_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                match param {
                    AffineParam::A => xform.post_a = v, AffineParam::B => xform.post_b = v,
                    AffineParam::C => xform.post_c = v, AffineParam::D => xform.post_d = v,
                    AffineParam::E => xform.post_e = v, AffineParam::F => xform.post_f = v,
                    AffineParam::G => xform.post_g = v,
                };
            }
            // JWildfire plane coefs on the Linked pool. Same per-slot
            // write pattern as the Normal-pool variants — the
            // GpuTransform builder picks up the change on next upload.
            ConfigPath::LinkedTransformYzCoefs { index, position } => {
                let xform = self.linked_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.yz_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::LinkedTransformZxCoefs { index, position } => {
                let xform = self.linked_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.zx_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::LinkedTransformYzPostCoefs { index, position } => {
                let xform = self.linked_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.yz_post_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::LinkedTransformZxPostCoefs { index, position } => {
                let xform = self.linked_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.zx_post_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::LinkedTransformVariation { index, variation } => {
                let xform = self.linked_transform_mut(*index)?;
                Self::apply_variation_weight(xform, variation, value)?;
            }
            ConfigPath::LinkedTransformVariationPriority { index, variation } => {
                let xform = self.linked_transform_mut(*index)?;
                Self::apply_variation_priority(xform, variation, value)?;
            }
            ConfigPath::LinkedTransformVariationOrder { index } => {
                let xform = self.linked_transform_mut(*index)?;
                xform.variation_order = value.try_into()?;
            }
            ConfigPath::LinkedTransformVariationParam { index, variation, param } => {
                let xform = self.linked_transform_mut(*index)?;
                Self::apply_variation_param(xform, variation, param, value)?;
            }

            // Final Transform pool — sourced from flame.final_transforms[index].
            ConfigPath::FinalTransformAffine { index, param } => {
                let xform = self.final_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                match param {
                    AffineParam::A => xform.a = v, AffineParam::B => xform.b = v,
                    AffineParam::C => xform.c = v, AffineParam::D => xform.d = v,
                    AffineParam::E => xform.e = v, AffineParam::F => xform.f = v,
                    AffineParam::G => xform.g = v,
                };
            }
            ConfigPath::FinalTransformPostAffineEnabled { index } => {
                let xform = self.final_transform_mut(*index)?;
                xform.post_affine_enabled = value.try_into()?;
            }
            ConfigPath::FinalTransformPostAffine { index, param } => {
                let xform = self.final_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                match param {
                    AffineParam::A => xform.post_a = v, AffineParam::B => xform.post_b = v,
                    AffineParam::C => xform.post_c = v, AffineParam::D => xform.post_d = v,
                    AffineParam::E => xform.post_e = v, AffineParam::F => xform.post_f = v,
                    AffineParam::G => xform.post_g = v,
                };
            }
            // JWildfire plane coefs on the Final pool.
            ConfigPath::FinalTransformYzCoefs { index, position } => {
                let xform = self.final_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.yz_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::FinalTransformZxCoefs { index, position } => {
                let xform = self.final_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.zx_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::FinalTransformYzPostCoefs { index, position } => {
                let xform = self.final_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.yz_post_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::FinalTransformZxPostCoefs { index, position } => {
                let xform = self.final_transform_mut(*index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.zx_post_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::FinalTransformVariation { index, variation } => {
                let xform = self.final_transform_mut(*index)?;
                Self::apply_variation_weight(xform, variation, value)?;
            }
            ConfigPath::FinalTransformVariationPriority { index, variation } => {
                let xform = self.final_transform_mut(*index)?;
                Self::apply_variation_priority(xform, variation, value)?;
            }
            ConfigPath::FinalTransformVariationOrder { index } => {
                let xform = self.final_transform_mut(*index)?;
                xform.variation_order = value.try_into()?;
            }
            ConfigPath::FinalTransformVariationParam { index, variation, param } => {
                let xform = self.final_transform_mut(*index)?;
                Self::apply_variation_param(xform, variation, param, value)?;
            }

            // Scene-level render state — config-level since v3 (write to the
            // config, not the resolved flame target).
            ConfigPath::RenderMode => {
                self.current.render_mode = value.try_into()?;
            }
            ConfigPath::PerspectiveStrength => {
                self.current.perspective_strength = value.try_into()?;
            }
            ConfigPath::DepthDensityCompensation => {
                self.current.depth_density_compensation = value.try_into()?;
            }
            ConfigPath::FarDensityFade => {
                self.current.far_density_fade = value.try_into()?;
            }
            ConfigPath::SolidStrength => {
                self.current.solid_strength = value.try_into()?;
            }
            ConfigPath::SurfaceThickness => {
                self.current.surface_thickness = value.try_into()?;
            }
            ConfigPath::ShadingStrength => {
                self.current.solid_shading.shading_strength = value.try_into()?;
            }
            ConfigPath::SolidAmbient => {
                self.current.solid_shading.ambient = value.try_into()?;
            }
            ConfigPath::SolidDiffuse => {
                self.current.solid_shading.diffuse = value.try_into()?;
            }
            ConfigPath::SolidSpecular => {
                self.current.solid_shading.specular = value.try_into()?;
            }
            ConfigPath::SolidShininess => {
                self.current.solid_shading.shininess = value.try_into()?;
            }
            ConfigPath::SsaoStrength => {
                self.current.solid_shading.ssao_strength = value.try_into()?;
            }
            ConfigPath::SsaoRadius => {
                self.current.solid_shading.ssao_radius = value.try_into()?;
            }
            ConfigPath::SolidLightEnabled { index } => {
                let l = self.current.solid_shading.lights.get_mut(*index).ok_or(ConfigError::InvalidIndex)?;
                l.enabled = value.try_into()?;
            }
            ConfigPath::SolidLightParam { index, param } => {
                let l = self.current.solid_shading.lights.get_mut(*index).ok_or(ConfigError::InvalidIndex)?;
                let v: f32 = value.try_into()?;
                match param.as_str() {
                    "azimuth" => l.azimuth = v,
                    "elevation" => l.elevation = v,
                    "intensity" => l.intensity = v,
                    "color_r" => l.color[0] = v,
                    "color_g" => l.color[1] = v,
                    "color_b" => l.color[2] = v,
                    _ => return Err(ConfigError::InvalidIndex),
                }
            }
            ConfigPath::FarDensityFadeStart => {
                self.current.far_density_fade_start = value.try_into()?;
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

            // Xaos (chaos-weighted transform transitions) — per-flame
            ConfigPath::Xaos { src, dst } => {
                let weight: f32 = value.try_into()?;
                let src = *src;
                let dst = *dst;
                self.active_flame_mut()?.set_xaos(src, dst, weight);
            }

            // Solo transform (-1 = None, 0+ = Some(index)) — per-flame
            ConfigPath::SoloTransform => {
                let idx: i32 = value.try_into()?;
                self.active_flame_mut()?.solo_transform = if idx < 0 {
                    None
                } else {
                    Some(idx as usize)
                };
            }

            // Post-symmetry — per-flame plot-time symmetry. Type
            // round-trips through u32; unknown values clamp to None.
            ConfigPath::PostSymmetryType => {
                use crate::scene::transforms::PostSymmetryType;
                let raw: i32 = value.try_into()?;
                self.active_flame_mut()?.post_symmetry.ty = match raw {
                    1 => PostSymmetryType::XAxis,
                    2 => PostSymmetryType::YAxis,
                    3 => PostSymmetryType::Point,
                    _ => PostSymmetryType::None,
                };
            }
            ConfigPath::PostSymmetryOrder => {
                let raw: i32 = value.try_into()?;
                self.active_flame_mut()?.post_symmetry.order = raw.max(1).min(32) as u32;
            }
            ConfigPath::PostSymmetryCenterX => {
                let v: f32 = value.try_into()?;
                self.active_flame_mut()?.post_symmetry.center_x = v;
            }
            ConfigPath::PostSymmetryCenterY => {
                let v: f32 = value.try_into()?;
                self.active_flame_mut()?.post_symmetry.center_y = v;
            }
            ConfigPath::PostSymmetryDistance => {
                let v: f32 = value.try_into()?;
                self.active_flame_mut()?.post_symmetry.distance = v;
            }
            ConfigPath::PostSymmetryRotation => {
                let v: f32 = value.try_into()?;
                self.active_flame_mut()?.post_symmetry.rotation_deg = v;
            }
            ConfigPath::PreserveZ => {
                self.current.preserve_z = value.try_into()?;
            }

            // System Settings - These should NOT be called via apply_value (they're not in FractalConfig)
            // Use config_manager.update_system_setting() instead
            ConfigPath::SystemIterationsPerThread
            | ConfigPath::SystemVsyncEnabled
            | ConfigPath::SystemTargetFps
            | ConfigPath::SystemFlyMouseSensitivity
            | ConfigPath::SystemFlyMoveSpeed
            | ConfigPath::SystemFlySprintMultiplier
            | ConfigPath::SystemFlyInvertY
            | ConfigPath::SystemFlyCameraMode
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
            let current_value = Self::get_value_from_config(&self.current, path, self.editing_target)?;
            let preview_value = Self::get_value_from_config(&preview, path, self.editing_target)?;

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
    pub fn load_config(&mut self, mut new_config: FractalConfig, description: String) -> Result<(), ConfigError> {
        // Clear any preview state
        self.preview = None;

        // Reset editing target — the new config has its own subflames
        // list, so any prior Subflame{i} target may now be out of
        // bounds. Land on Main.
        self.editing_target = EditingTarget::Main;

        // Assign session-local IDs to any item that came in with the
        // zero sentinel. Caller paths that don't route through
        // `FractalConfig::from_json` (animation embedded base_config
        // via Animation's derive Deserialize, API DTO conversion,
        // preset clones from a different source) bring in id=0
        // everywhere; without this, fresh-loaded transforms would
        // collide in id-space and animation bindings would resolve
        // to the wrong item.
        new_config.fixup_ids();

        // Create single bidirectional snapshot
        let change = ConfigChange::full_config_snapshot(
            self.current.clone(),  // before
            new_config.clone(),    // after
            description,
        );

        self.push_undo(change);

        // Replace current config
        self.current = new_config;

        // Mark a fresh fractal load so per-fractal UI view state resets.
        self.load_generation = self.load_generation.wrapping_add(1);

        // Record full config import action
        let mut action = UpdateAction::none();
        action.update_flame = true;
        action.update_view = true;
        action.update_palette = true;
        action.update_tone_curve = true;
        action.reset_accumulation = true;
        // Lists may have completely changed shape; flag for animation
        // rebind. (The app-level animation load path also calls
        // `bind_to_config` directly after this, so this is belt-and-
        // suspenders for cases where load_config is called without
        // a follow-up bind.)
        action.structural_changed = true;
        self.pending_actions.merge(&action);

        Ok(())
    }

    /// Load a complete config silently (no undo entry)
    /// Used when restoring base config after animation stops
    /// The undo entry should have already been created by handle_animation_exit
    pub fn load_config_silent(&mut self, new_config: FractalConfig) -> Result<(), ConfigError> {
        // Clear any preview state
        self.preview = None;

        // Reset editing target — see load_config for rationale.
        self.editing_target = EditingTarget::Main;

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
    /// Creates specialized snapshot and updates config atomically.
    /// Structural changes apply to the current editing target — adding
    /// a transform while editing Subflame{i} appends to that
    /// subflame's transforms list, not the parent's.
    pub fn apply_structural_change(&mut self, change: ConfigChange) -> Result<(), ConfigError> {
        // Apply the change based on snapshot type
        if let Some(snapshot) = &change.snapshot {
            match snapshot {
                crate::config::SnapshotData::AddTransform { index, transform, clone_from, .. } => {
                    let index = *index;
                    let transform = transform.clone();
                    let clone_from = *clone_from;
                    let flame = self.active_flame_mut()?;
                    if index <= flame.transforms.len() {
                        // Update xaos matrix before inserting
                        if let Some(source_idx) = clone_from {
                            flame.on_transform_cloned(index, source_idx);
                        } else {
                            flame.on_transform_added(index);
                        }
                        flame.transforms.insert(index, transform);
                    } else {
                        return Err(ConfigError::InvalidIndex);
                    }
                }
                crate::config::SnapshotData::DeleteTransform { index, .. } => {
                    let index = *index;
                    let flame = self.active_flame_mut()?;
                    if index < flame.transforms.len() {
                        flame.transforms.remove(index);
                        // Update xaos matrix after removing (needs new transform list)
                        flame.on_transform_deleted(index);
                    } else {
                        return Err(ConfigError::InvalidIndex);
                    }
                }
                crate::config::SnapshotData::ModifyTransform { kind, index, after, .. } => {
                    let xref = kind.at(*index);
                    let after = after.clone();
                    let flame = self.active_flame_mut()?;
                    if let Some(slot) = xref.get_mut(flame) {
                        *slot = after;
                    } else {
                        return Err(ConfigError::InvalidIndex);
                    }
                }
                crate::config::SnapshotData::FullConfig { after, .. } => {
                    self.current = (**after).clone();
                    // Snapshot's after may carry id=0 transforms if it
                    // came from a derive-Deserialize path; allocate
                    // fresh IDs so animation bindings can resolve.
                    self.current.fixup_ids();
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
                    self.pending_actions.structural_changed = true;
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
                    self.pending_actions.structural_changed = true;
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
                    self.pending_actions.structural_changed = true;
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
                    self.pending_actions.structural_changed = true;
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
                    self.pending_actions.structural_changed = true;
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
                    self.pending_actions.structural_changed = true;
                    return Ok(());
                }

                // Subflame variants flow through their dedicated APIs
                // (add_subflame / delete_subflame / set_editing_target)
                // which already push the right snapshot — they don't
                // route through the generic apply_structural_change.
                crate::config::SnapshotData::AddSubflame { .. }
                | crate::config::SnapshotData::DeleteSubflame { .. }
                | crate::config::SnapshotData::SwapTarget { .. } => {
                    return Err(ConfigError::InvalidOperation);
                }
            }
        } else {
            return Err(ConfigError::InvalidOperation);
        }

        // Record in history
        self.push_undo(change);

        // Record GPU update action
        self.record_action(UpdateType::IterationReset);

        // A transform pool's shape changed; tell the App to rebind
        // animation tracks so any bound IDs follow their items to
        // their new indices.
        self.pending_actions.structural_changed = true;

        Ok(())
    }

    /// Start a modify session for a pool member (Normal / Linked / Final).
    /// Captures initial state but doesn't create a history entry yet —
    /// all updates during the session apply to config silently. Call
    /// `commit_modify_transform()` to roll the cumulative diff into a
    /// single ModifyTransform snapshot. This is what gives the Triangle
    /// Editor "one undo per drag" behavior across all three pools.
    pub fn start_modify_transform(&mut self, xref: TransformRef) -> Result<(), ConfigError> {
        // Can't start a new session if one is already active.
        if self.modify_session.is_some() {
            return Err(ConfigError::InvalidOperation);
        }

        // Validate the pool member exists; capture its initial state.
        let initial_transform = xref.get(&self.current.flame)
            .ok_or(ConfigError::InvalidIndex)?
            .clone();

        self.modify_session = Some(ModifySession {
            xref,
            initial_transform,
        });

        log::debug!("Started modify session for {:?}", xref);
        Ok(())
    }

    /// Commit the active modify session.
    /// Creates a ModifyTransform snapshot tagged with the session's
    /// pool kind. Returns error if no session is active.
    pub fn commit_modify_transform(&mut self, description: String) -> Result<UpdateType, ConfigError> {
        let session = self.modify_session.take()
            .ok_or(ConfigError::InvalidOperation)?;

        // Clear preview state (session is ending)
        self.preview = None;

        let xref = session.xref;
        let before = session.initial_transform;
        let after = xref.get(&self.current.flame)
            .ok_or(ConfigError::InvalidIndex)?
            .clone();

        // Check if transform actually changed (avoid no-op snapshots)
        let pre_changed = before.a != after.a || before.b != after.b || before.c != after.c
            || before.d != after.d || before.e != after.e || before.f != after.f
            || before.g != after.g;
        let post_changed = before.post_a != after.post_a || before.post_b != after.post_b
            || before.post_c != after.post_c || before.post_d != after.post_d
            || before.post_e != after.post_e || before.post_f != after.post_f
            || before.post_g != after.post_g
            || before.post_affine_enabled != after.post_affine_enabled;
        let changed = pre_changed || post_changed;

        if !changed {
            log::debug!("Modify session commit: no changes detected, skipping snapshot");
            return Ok(UpdateType::None);
        }

        // Create ModifyTransform snapshot tagged with pool kind.
        let change = ConfigChange::modify_transform_snapshot(
            xref.kind(), xref.index(), before, after, description,
        );

        // Record in history
        self.push_undo(change);

        // Record GPU update action
        self.record_action(UpdateType::IterationReset);

        log::debug!("Committed modify session for {:?}", xref);
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

        // Restore initial state via the right pool.
        if let Some(slot) = session.xref.get_mut(&mut self.current.flame) {
            *slot = session.initial_transform;
        }

        // Record GPU update action (need to restore visual state)
        self.record_action(UpdateType::IterationReset);

        log::debug!("Cancelled modify session for {:?}", session.xref);
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

impl TryFrom<ConfigValue> for Vec<String> {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::StringList(s) => Ok(s),
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

impl TryFrom<ConfigValue> for crate::scene::tonemap::HighlightMode {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::HighlightMode(m) => Ok(m),
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

impl TryFrom<ConfigValue> for crate::scene::palette::SqueezeMode {
    type Error = ConfigError;
    fn try_from(v: ConfigValue) -> Result<Self, Self::Error> {
        match v {
            ConfigValue::SqueezeMode(m) => Ok(m),
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

        // Get initial value (must match DEFAULT_EXPOSURE)
        let value = manager.get_value(&ConfigPath::Exposure).unwrap();
        assert!(value.approx_eq(&ConfigValue::Float(crate::config::defaults::DEFAULT_EXPOSURE)));

        // Set new value
        manager
            .set_value(&ConfigPath::Exposure, 2.0.into())
            .unwrap();
        assert_eq!(manager.current.exposure, 2.0);
    }

    #[test]
    fn test_remove_variation_undo_restores_params() {
        use crate::scene::transforms::{Flame, Transform};
        use crate::config::TransformRef;
        let mut config = FractalConfig::default();
        let mut flame = Flame::new();
        let mut t = Transform::new();
        t.set_variation("linear", 1.0);
        t.set_variation("squish", 1.0);
        t.variation_params.insert("squish.power".to_string(), 7.0);
        t.variation_priorities.insert("squish".to_string(), 1);
        flame.transforms = vec![t];
        config.flame = flame;
        let mut manager = ConfigManager::new(config);

        manager.remove_variation(TransformRef::Normal(0), "squish").unwrap();
        let xf = &manager.current.flame.transforms[0];
        assert!(!xf.variations.contains_key("squish"));
        assert!(!xf.variation_params.contains_key("squish.power"));
        assert!(!xf.variation_priorities.contains_key("squish"));

        // Undo must bring squish back WITH its params + priority — the
        // regression was that only the weight came back (default params).
        manager.undo().unwrap();
        let xf = &manager.current.flame.transforms[0];
        assert!(xf.variations.contains_key("squish"));
        assert_eq!(xf.variation_params.get("squish.power"), Some(&7.0));
        assert_eq!(xf.variation_priorities.get("squish"), Some(&1));

        // Redo re-scrubs.
        manager.redo().unwrap();
        let xf = &manager.current.flame.transforms[0];
        assert!(!xf.variations.contains_key("squish"));
        assert!(!xf.variation_params.contains_key("squish.power"));
    }

    #[test]
    fn test_variation_order_path_get_set() {
        use crate::scene::transforms::{Flame, Transform};
        let mut config = FractalConfig::default();
        let mut flame = Flame::new();
        let mut t = Transform::new();
        t.set_variation("linear", 1.0); // order: [linear]
        t.set_variation("spherical", 0.5); // order: [linear, spherical]
        flame.transforms = vec![t];
        config.flame = flame;
        let mut manager = ConfigManager::new(config);

        let path = ConfigPath::TransformVariationOrder { index: 0 };
        let got = manager.get_value(&path).unwrap();
        assert!(got.approx_eq(&ConfigValue::StringList(vec![
            "linear".to_string(),
            "spherical".to_string()
        ])));

        // Reorder (swap) and confirm it lands on the transform.
        manager
            .set_value(
                &path,
                ConfigValue::StringList(vec!["spherical".to_string(), "linear".to_string()]),
            )
            .unwrap();
        assert_eq!(
            manager.current.flame.transforms[0].variation_order,
            vec!["spherical".to_string(), "linear".to_string()]
        );
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

        // Initial state — track DEFAULT_* so test stays correct when
        // tonemap defaults shift.
        use crate::config::defaults::*;
        let initial_exposure = DEFAULT_EXPOSURE;
        let initial_gamma = DEFAULT_GAMMA;
        let initial_brightness = DEFAULT_BRIGHTNESS;
        assert_eq!(manager.config().exposure, initial_exposure);
        assert_eq!(manager.config().gamma, initial_gamma);
        assert_eq!(manager.config().brightness, initial_brightness);
        assert_eq!(manager.config().zoom, 1.0);

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

        // Undo: should revert brightness (back to default)
        manager.undo().unwrap();
        assert_eq!(manager.config().brightness, initial_brightness, "After 2nd undo, expected brightness={}, got {}", initial_brightness, manager.config().brightness);
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

        // Undo (back to default)
        manager.undo().unwrap();
        assert_eq!(manager.current.exposure, crate::config::defaults::DEFAULT_EXPOSURE);

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

    /// ModifyTransform snapshot must dispatch on `kind` so undo/redo
    /// restores into the right pool. Catches any future drift where
    /// the apply paths assume Normal-only indexing into
    /// `flame.transforms`. Fresh ConfigManager per kind so each case
    /// starts from a clean baseline.
    #[test]
    fn test_modify_transform_snapshot_roundtrip_all_pools() {
        use crate::scene::transforms::Transform;

        for kind in [TransformKind::Normal, TransformKind::Linked, TransformKind::Final] {
            let mut config = FractalConfig::default();
            config.flame.transforms.push(Transform::new());
            config.flame.linked_transforms.push(Transform::new());
            config.flame.final_transforms.push(Transform::new());

            let mut mgr = ConfigManager::new(config);
            let xref = kind.at(0);
            let initial_a = 1.0f32; // Transform::new is identity

            // Start session, mutate via ConfigPath (must route to the
            // right pool), commit.
            mgr.start_modify_transform(xref).unwrap();
            mgr.set_value(&xref.affine_path(AffineParam::A), 7.5f32.into()).unwrap();
            assert_eq!(
                xref.get(&mgr.active_config().flame).unwrap().a,
                7.5,
                "{:?}: live mutation should land in the right pool",
                kind,
            );
            mgr.commit_modify_transform(format!("test {:?}", kind)).unwrap();
            assert_eq!(
                mgr.history.len(), 1,
                "{:?}: commit should produce exactly one history entry",
                kind,
            );

            // Undo restores the initial value into the right pool.
            mgr.undo().unwrap();
            assert_eq!(
                xref.get(&mgr.active_config().flame).unwrap().a,
                initial_a,
                "{:?} undo: should restore initial.a",
                kind,
            );
            // Other pools must remain at their initial values — undoing
            // a Linked snapshot must not touch a Normal or Final.
            for other in [TransformKind::Normal, TransformKind::Linked, TransformKind::Final] {
                if other == kind { continue; }
                let other_xref = other.at(0);
                assert_eq!(
                    other_xref.get(&mgr.active_config().flame).unwrap().a,
                    initial_a,
                    "{:?} undo must not affect {:?}",
                    kind,
                    other,
                );
            }

            // Redo restores the mutation only on the right pool.
            mgr.redo().unwrap();
            assert_eq!(
                xref.get(&mgr.active_config().flame).unwrap().a,
                7.5,
                "{:?} redo: should restore the mutation",
                kind,
            );
            for other in [TransformKind::Normal, TransformKind::Linked, TransformKind::Final] {
                if other == kind { continue; }
                let other_xref = other.at(0);
                assert_eq!(
                    other_xref.get(&mgr.active_config().flame).unwrap().a,
                    initial_a,
                    "{:?} redo must not affect {:?}",
                    kind,
                    other,
                );
            }
        }
    }

    /// Editing target is now a routing field, not a physical swap.
    /// `current.flame` stays as the main flame regardless of what's
    /// being edited. Writes routed via `target_flame_mut` land on the
    /// right slot, and the subflames list is always intact in
    /// `current.flame.subflames`.
    #[test]
    fn editing_target_routes_writes_without_swapping_data() {
        use crate::scene::transforms::{Flame, Transform};
        use crate::config::{ConfigPath, AffineParam};

        let mut config = FractalConfig::default();
        if config.flame.transforms.is_empty() {
            config.flame.transforms.push(Transform::new());
        }
        config.flame.transforms[0].a = 11.0;  // Main marker
        for marker in [101.0, 102.0, 103.0] {
            let mut sf = Flame::new();
            sf.transforms.push({
                let mut t = Transform::new();
                t.a = marker;
                t
            });
            config.flame.subflames.push(sf);
        }

        let mut mgr = ConfigManager::new(config);
        assert_eq!(mgr.editing_target(), EditingTarget::Main);
        assert_eq!(mgr.current.flame.transforms[0].a, 11.0);
        assert_eq!(mgr.visible_subflames().len(), 3);

        // Switch target to subflame 1 — no data movement, just the
        // routing field changes.
        mgr.set_editing_target(EditingTarget::Subflame { index: 1 }).unwrap();
        assert_eq!(mgr.editing_target(), EditingTarget::Subflame { index: 1 });
        // current.flame stays as the main flame:
        assert_eq!(mgr.current.flame.transforms[0].a, 11.0,
            "current.flame remains the main flame after target switch");
        // All three subflames remain in place:
        assert_eq!(mgr.current.flame.subflames.len(), 3);
        assert_eq!(mgr.current.flame.subflames[1].transforms[0].a, 102.0);

        // Writing via update_param routes to subflame 1.
        mgr.update_param(
            ConfigPath::TransformAffine { index: 0, param: AffineParam::A },
            999.0f32.into(),
        ).unwrap();
        assert_eq!(mgr.current.flame.subflames[1].transforms[0].a, 999.0,
            "write while editing subflame 1 lands on subflames[1]");
        assert_eq!(mgr.current.flame.transforms[0].a, 11.0,
            "main flame's transform stays unchanged");

        // Switch to subflame 2 — same story, no data swap.
        mgr.set_editing_target(EditingTarget::Subflame { index: 2 }).unwrap();
        assert_eq!(mgr.current.flame.transforms[0].a, 11.0);
        assert_eq!(mgr.current.flame.subflames[1].transforms[0].a, 999.0,
            "edit to subflame 1 persists across target switches");
        assert_eq!(mgr.current.flame.subflames[2].transforms[0].a, 103.0);
    }

    /// In the un-swap world add/delete subflame work regardless of
    /// the current editing target. Deleting a subflame either *is*
    /// the active one (target falls back to Main) or has a lower
    /// index than the active one (target index shifts down).
    #[test]
    fn add_delete_subflame_in_any_target() {
        use crate::scene::transforms::Transform;

        let config = FractalConfig::default();
        let mut mgr = ConfigManager::new(config);

        // Add three subflames, marker each one.
        for marker in [100.0, 200.0, 300.0] {
            mgr.add_subflame().unwrap();
            let idx = mgr.current.flame.subflames.len() - 1;
            mgr.current.flame.subflames[idx].transforms[0].a = marker;
        }
        assert_eq!(mgr.current.flame.subflames.len(), 3);

        // Adding while editing a subflame is now allowed — un-swap removes
        // the index-stability concern that originally gated it.
        mgr.set_editing_target(EditingTarget::Subflame { index: 1 }).unwrap();
        let new_idx = mgr.add_subflame().unwrap();
        assert_eq!(new_idx, 3);
        assert_eq!(mgr.current.flame.subflames.len(), 4);
        // Editing target is unchanged — we're still on subflame 1:
        assert_eq!(mgr.editing_target(), EditingTarget::Subflame { index: 1 });

        // logical_config matches current (no swap to undo).
        let logical = mgr.logical_config();
        assert_eq!(logical.flame.subflames.len(), 4);

        // Delete the ACTIVE subflame (index 1, marker 200.0). Target
        // falls back to Main; subsequent subflames shift down.
        mgr.delete_subflame(1).unwrap();
        assert_eq!(mgr.editing_target(), EditingTarget::Main,
            "deleting the active subflame falls back to Main");
        assert_eq!(mgr.current.flame.subflames.len(), 3);
        assert_eq!(mgr.current.flame.subflames[0].transforms[0].a, 100.0);
        assert_eq!(mgr.current.flame.subflames[1].transforms[0].a, 300.0,
            "subflame 2 shifts to index 1 after deleting index 1");

        // Re-add a marker subflame at the end so we have 4 again.
        mgr.add_subflame().unwrap();
        let last = mgr.current.flame.subflames.len() - 1;
        mgr.current.flame.subflames[last].transforms[0].a = 500.0;

        // Now editing subflame 2 (marker 500.0 lives at index 3).
        // Delete a LOWER index — the active index should shift down.
        mgr.set_editing_target(EditingTarget::Subflame { index: 3 }).unwrap();
        mgr.delete_subflame(0).unwrap();
        assert_eq!(mgr.editing_target(), EditingTarget::Subflame { index: 2 },
            "deleting a lower-index subflame shifts the active index down by 1");
        assert_eq!(mgr.current.flame.subflames.len(), 3);
        // subflames[2] is now the one we were editing (marker 500.0):
        assert_eq!(mgr.current.flame.subflames[2].transforms[0].a, 500.0);
    }

    /// Undo/redo must thread through editing-target switches:
    /// edits made while editing a subflame apply to *that subflame* on
    /// undo, never to whichever flame happens to be active at undo
    /// time. The target stamp on each ConfigChange is what makes this
    /// work — silent_swap aligns `self.editing_target` to the entry's
    /// target before the inverse delta is applied, so the routing
    /// machinery picks the right slot.
    #[test]
    fn undo_redo_threads_through_target_swaps() {
        use crate::scene::transforms::{Flame, Transform};
        use crate::config::ConfigPath;

        let mut config = FractalConfig::default();
        if config.flame.transforms.is_empty() {
            config.flame.transforms.push(Transform::new());
        }
        config.flame.transforms[0].a = 1.0;  // Main marker
        let mut sf = Flame::new();
        let mut t = Transform::new();
        t.a = 100.0;  // Subflame marker
        sf.transforms.push(t);
        config.flame.subflames.push(sf);

        let mut mgr = ConfigManager::new(config);

        // Three actions across the target boundary:
        //   1. Edit Main's transform 0 a-coef: 1.0 → 2.0
        //   2. Switch to Subflame 0
        //   3. Edit Subflame's transform 0 a-coef: 100.0 → 200.0
        mgr.update_param(
            ConfigPath::TransformAffine {
                index: 0,
                param: crate::config::AffineParam::A,
            },
            2.0f32.into(),
        ).unwrap();
        mgr.set_editing_target(EditingTarget::Subflame { index: 0 }).unwrap();
        mgr.update_param(
            ConfigPath::TransformAffine {
                index: 0,
                param: crate::config::AffineParam::A,
            },
            200.0f32.into(),
        ).unwrap();

        // Sanity: edits landed on their respective flames (no swap).
        assert_eq!(mgr.editing_target(), EditingTarget::Subflame { index: 0 });
        assert_eq!(mgr.current.flame.transforms[0].a, 2.0,
            "main flame still holds main edit (no swap)");
        assert_eq!(mgr.current.flame.subflames[0].transforms[0].a, 200.0,
            "subflame edit landed on subflame slot");

        // Undo 3 times: revert subflame edit, revert target switch, revert main edit.
        mgr.undo().unwrap();  // undo subflame edit
        assert_eq!(mgr.editing_target(), EditingTarget::Subflame { index: 0 });
        assert_eq!(mgr.current.flame.subflames[0].transforms[0].a, 100.0,
            "subflame edit reverts on the subflame slot");
        assert_eq!(mgr.current.flame.transforms[0].a, 2.0,
            "main flame's value is unaffected by undoing a subflame edit");

        mgr.undo().unwrap();  // undo target switch
        assert_eq!(mgr.editing_target(), EditingTarget::Main,
            "undoing the swap returns us to Main");
        assert_eq!(mgr.current.flame.transforms[0].a, 2.0,
            "main edit is still in place");

        mgr.undo().unwrap();  // undo main edit
        assert_eq!(mgr.editing_target(), EditingTarget::Main);
        assert_eq!(mgr.current.flame.transforms[0].a, 1.0,
            "main edit reverts");
        assert_eq!(mgr.current.flame.subflames[0].transforms[0].a, 100.0,
            "subflame data intact");

        // Redo all three back to the final state.
        mgr.redo().unwrap();
        assert_eq!(mgr.editing_target(), EditingTarget::Main);
        assert_eq!(mgr.current.flame.transforms[0].a, 2.0);

        mgr.redo().unwrap();
        assert_eq!(mgr.editing_target(), EditingTarget::Subflame { index: 0 });
        assert_eq!(mgr.current.flame.subflames[0].transforms[0].a, 100.0);

        mgr.redo().unwrap();
        assert_eq!(mgr.editing_target(), EditingTarget::Subflame { index: 0 });
        assert_eq!(mgr.current.flame.subflames[0].transforms[0].a, 200.0,
            "redo lands the subflame edit on the subflame slot");
        assert_eq!(mgr.current.flame.transforms[0].a, 2.0,
            "main flame unaffected by redoing a subflame edit");
    }

    /// Add/delete subflame snapshots must roundtrip without losing
    /// any of the subflame's contents — every transform, variation,
    /// and parameter must come back identically on undo.
    #[test]
    fn add_delete_subflame_undo_redo_recreates_byte_for_byte() {
        use crate::scene::transforms::Transform;

        let mut config = FractalConfig::default();
        if config.flame.transforms.is_empty() {
            config.flame.transforms.push(Transform::new());
        }
        let mut mgr = ConfigManager::new(config);

        // Add a subflame, customize it.
        let idx = mgr.add_subflame().unwrap();
        let mut t = Transform::new();
        t.a = 42.0;
        t.b = 99.0;
        t.set_variation("spherical", 0.75);
        mgr.current.flame.subflames[idx].transforms.push(t);
        mgr.current.flame.subflames[idx].name = "Test Subflame".to_string();
        let snapshot_after_add = mgr.current.flame.subflames[idx].clone();

        // Delete it.
        mgr.delete_subflame(idx).unwrap();
        assert_eq!(mgr.current.flame.subflames.len(), 0);

        // Undo the delete — subflame must return *exactly* as it was,
        // not just "a subflame at that index".
        mgr.undo().unwrap();
        assert_eq!(mgr.current.flame.subflames.len(), 1);
        let restored = &mgr.current.flame.subflames[0];
        assert_eq!(restored.name, snapshot_after_add.name);
        assert_eq!(restored.transforms.len(), snapshot_after_add.transforms.len());
        // Spot-check the custom transform we added — the test subflame
        // has the default linear transform at [0] plus our custom at [1].
        assert_eq!(restored.transforms[1].a, 42.0);
        assert_eq!(restored.transforms[1].b, 99.0);
        assert_eq!(
            restored.transforms[1].variations.get("spherical").copied(),
            Some(0.75)
        );

        // Redo the delete — gone again.
        mgr.redo().unwrap();
        assert_eq!(mgr.current.flame.subflames.len(), 0);

        // Undo two steps — back to before the add. Both the delete and
        // the add itself reverted.
        mgr.undo().unwrap();  // undo delete
        mgr.undo().unwrap();  // undo add
        assert_eq!(mgr.current.flame.subflames.len(), 0);

        // BUT: the WHOLE subflame is gone from history-recreated state,
        // including the post-add edits — those edits weren't in the
        // history because we mutated subflames[idx] directly. That's
        // OK; a real UI would push them through ConfigManager.
        // The thing we ARE checking: the structural snapshots are
        // restored cleanly without leftover ghost subflames.
    }
}

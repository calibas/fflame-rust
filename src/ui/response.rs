/// One in-frame edit to a normal transform's attachment list (Linked or Final).
#[derive(Debug, Clone)]
pub struct AttachmentEdit {
    pub normal_index: usize,
    pub kind: AttachmentKind,
    pub op: AttachmentOp,
}

#[derive(Debug, Clone, Copy)]
pub enum AttachmentKind {
    Linked,
    Final,
}

#[derive(Debug, Clone)]
pub enum AttachmentOp {
    /// Toggle attachment at the given pool index (attach if absent, detach if present).
    Toggle(usize),
    /// Move the entry whose pool index is `pool_index` one slot earlier in the
    /// attachment list (changes execution order for that normal only).
    MoveUp(usize),
    MoveDown(usize),
}

/// Response from the UI layer indicating what actions should be taken
///
/// Note: Config-related change tracking has been moved to ConfigManager.get_pending_actions()
/// This struct now only contains non-config actions (file I/O, palette library, transforms, etc.)
pub struct UiResponse {
    // File I/O operations
    pub config_export_requested: Option<String>,
    pub config_import_requested: Option<String>,
    pub config_save_file_requested: bool,
    pub config_load_file_requested: bool,
    pub flame_xml_import_file_requested: bool,
    pub flame_xml_export_file_requested: bool,
    pub new_flame_requested: bool,
    pub random_flame_requested: bool,

    // Palette library management (not stored in config directly)
    pub palette_export_json: Option<crate::scene::palette::Palette>,
    pub palette_save_file: Option<crate::scene::palette::Palette>,
    pub palette_save_to_library: Option<crate::scene::palette::Palette>,
    pub palette_delete_from_library: Option<String>,
    pub palette_import_json: Option<String>,
    pub palette_load_file: bool,

    // Undo/redo (handled by ConfigManager but triggered from UI)
    pub undo_requested: bool,
    pub redo_requested: bool,

    // Export operations
    pub png_export_with_background: bool,
    pub png_export_transparent: bool,

    // Transform management (creates config deltas but needs special handling)
    pub add_transform: bool,
    pub delete_transform: Option<usize>,
    pub clone_transform: Option<usize>,

    // Linked-pool transform management (parallel to the normal ones above).
    pub add_linked_transform: bool,
    pub delete_linked_transform: Option<usize>,
    pub clone_linked_transform: Option<usize>,

    // Final-pool transform management. Adding a Final auto-attaches it
    // to every normal transform; ui_handlers builds the right snapshot.
    pub add_final_transform: bool,
    pub delete_final_transform: Option<usize>,
    pub clone_final_transform: Option<usize>,

    // Per-normal attachment edit (toggle / reorder Linked or Final attachments
    // on a specific normal transform). Applied as a full-config snapshot.
    pub attachment_edit: Option<AttachmentEdit>,

    // Panel open requests
    pub open_palette_editor: bool,
    pub open_palette_library: bool,
    pub open_config_dialog: bool,
    pub open_triangle_editor: bool,
    pub open_preset_library: bool,
    pub open_random_generator: bool,

    // Fractal viewport size (for matching texture dimensions to panel)
    pub fractal_viewport_size: Option<(u32, u32)>,

    // Free-fly camera: viewport drag delta in pixels (only set when
    // fly_mode is active AND the user dragged in the viewport this
    // frame). The App consumes this via `apply_fly_mouse_look` to
    // update camera_rotation_x/y.
    pub fly_mouse_drag: Option<(f32, f32)>,

    // Free-fly camera: user requested toggle (View panel button).
    pub fly_mode_toggle_requested: bool,

    // UI interaction state (for frame rate optimization)
    pub needs_repaint: bool,

    // Preset selection from library
    pub selected_preset_config: Option<crate::config::FractalConfig>,

    // File browser requests
    pub file_browser_open_requested: bool,

    // Animation export request
    pub animation_export_requested: Option<super::animation_panel::AnimationExportSettings>,

    // Subflames panel: user clicked the "Load from file" button on a
    // subflame row, requesting we replace that subflame's flame data
    // with one loaded from a `.fflame` file. Index identifies the
    // target slot in `flame.subflames`.
    pub load_subflame_into: Option<usize>,

    // Animation timeline was scrubbed (slider dragged or frame stepped)
    pub animation_seek_changed: bool,

    // Animation scrubber drag stopped or discrete seek action (frame step) - reset accumulation
    pub animation_seek_drag_stopped: bool,

    // Path filters changed (applies to renderer, not config)
    pub path_filters_changed: Option<Vec<crate::gpu::buffers::GpuPathFilter>>,

    // Generated flame from random generator panel (single). Carries the
    // scene render settings (render_mode/perspective) alongside the flame
    // since those are config-level (v3).
    pub generated_flame: Option<crate::scene::randomize::RandomFlame>,

    // Generated batch of configs from random generator (opens in File Browser)
    pub generated_batch: Option<Vec<crate::config::FractalConfig>>,

    // Audio file load requested
    pub load_audio_file: bool,

    // Signal file load requested
    pub load_signal_file: bool,

    // Signal file save requested (signal name to save)
    pub save_signal_file: Option<String>,

    // API: Save action (save new, update existing)
    pub api_save_action: ApiSaveAction,

    // API: Animation save action (from Animation Panel)
    pub api_animation_save_action: ApiAnimationSaveAction,

    // API: Flame ID loaded from Online tab
    pub loaded_api_flame_id: Option<String>,
    // API: Visibility of the flame loaded from Online tab (None = unknown)
    pub loaded_api_flame_is_public: Option<bool>,
    // API: Owner user ID of the loaded flame
    pub loaded_api_flame_user_id: Option<String>,
    // API: Number of animations linked to the loaded flame
    pub loaded_api_flame_animation_count: u32,
    // API: List of animations linked to the loaded flame
    pub loaded_api_flame_animations: Vec<crate::api::types::AnimationSummary>,
    // API: Animation ID to load (from the animations list in the panel)
    pub load_api_animation_id: Option<String>,
    // Variations panel: clear all API-loaded variations from cache+registry
    pub clear_variation_cache_requested: bool,
}

/// API save action type (flame only — animation is separate)
#[derive(Debug, Clone, Default)]
pub enum ApiSaveAction {
    #[default]
    None,
    SaveNew {
        name: String,
        upload_thumbnail: bool,
        make_public: bool,
    },
    Update {
        name: String,
        upload_thumbnail: bool,
        make_public: bool,
    },
}

/// API animation save action type
#[derive(Debug, Clone, Default)]
pub enum ApiAnimationSaveAction {
    #[default]
    None,
    /// Save as new animation linked to current flame
    SaveNew { make_public: bool },
    /// Update existing animation
    Update { make_public: bool },
}

impl Default for UiResponse {
    fn default() -> Self {
        Self {
            config_export_requested: None,
            config_import_requested: None,
            config_save_file_requested: false,
            config_load_file_requested: false,
            flame_xml_import_file_requested: false,
            flame_xml_export_file_requested: false,
            new_flame_requested: false,
            random_flame_requested: false,
            palette_export_json: None,
            palette_save_file: None,
            palette_save_to_library: None,
            palette_delete_from_library: None,
            palette_import_json: None,
            palette_load_file: false,
            undo_requested: false,
            redo_requested: false,
            png_export_with_background: false,
            png_export_transparent: false,
            add_transform: false,
            delete_transform: None,
            clone_transform: None,
            add_linked_transform: false,
            delete_linked_transform: None,
            clone_linked_transform: None,
            add_final_transform: false,
            delete_final_transform: None,
            clone_final_transform: None,
            attachment_edit: None,
            open_palette_editor: false,
            open_palette_library: false,
            open_config_dialog: false,
            open_triangle_editor: false,
            open_preset_library: false,
            open_random_generator: false,
            fractal_viewport_size: None,
            fly_mouse_drag: None,
            fly_mode_toggle_requested: false,
            needs_repaint: false,
            selected_preset_config: None,
            file_browser_open_requested: false,
            animation_export_requested: None,
            load_subflame_into: None,
            animation_seek_changed: false,
            animation_seek_drag_stopped: false,
            path_filters_changed: None,
            generated_flame: None,
            generated_batch: None,
            load_audio_file: false,
            load_signal_file: false,
            save_signal_file: None,
            api_save_action: ApiSaveAction::None,
            api_animation_save_action: ApiAnimationSaveAction::None,
            loaded_api_flame_id: None,
            loaded_api_flame_is_public: None,
            loaded_api_flame_user_id: None,
            loaded_api_flame_animation_count: 0,
            loaded_api_flame_animations: Vec::new(),
            load_api_animation_id: None,
            clear_variation_cache_requested: false,
        }
    }
}

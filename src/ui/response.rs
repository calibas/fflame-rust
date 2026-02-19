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
    pub apophysis_import_file_requested: bool,
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

    // Panel open requests
    pub open_palette_editor: bool,
    pub open_palette_library: bool,
    pub open_config_dialog: bool,
    pub open_triangle_editor: bool,
    pub open_preset_library: bool,
    pub open_random_generator: bool,

    // Fractal viewport size (for matching texture dimensions to panel)
    pub fractal_viewport_size: Option<(u32, u32)>,

    // UI interaction state (for frame rate optimization)
    pub needs_repaint: bool,

    // Preset selection from library
    pub selected_preset_config: Option<crate::config::FractalConfig>,

    // File browser requests
    pub file_browser_open_requested: bool,

    // Animation export request
    pub animation_export_requested: Option<super::animation_panel::AnimationExportSettings>,

    // Animation timeline was scrubbed (slider dragged or frame stepped)
    pub animation_seek_changed: bool,

    // Animation scrubber drag stopped or discrete seek action (frame step) - reset accumulation
    pub animation_seek_drag_stopped: bool,

    // Path filters changed (applies to renderer, not config)
    pub path_filters_changed: Option<Vec<crate::gpu::buffers::GpuPathFilter>>,

    // Generated flame from random generator panel (single)
    pub generated_flame: Option<crate::scene::transforms::Flame>,

    // Generated batch of configs from random generator (opens in File Browser)
    pub generated_batch: Option<Vec<crate::config::FractalConfig>>,

    // Audio file load requested
    pub load_audio_file: bool,

    // Signal file load requested
    pub load_signal_file: bool,

    // Signal file save requested (signal name to save)
    pub save_signal_file: Option<String>,

    // API: Save current flame online
    #[cfg(feature = "api")]
    pub save_online_requested: bool,
}

impl Default for UiResponse {
    fn default() -> Self {
        Self {
            config_export_requested: None,
            config_import_requested: None,
            config_save_file_requested: false,
            config_load_file_requested: false,
            apophysis_import_file_requested: false,
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
            open_palette_editor: false,
            open_palette_library: false,
            open_config_dialog: false,
            open_triangle_editor: false,
            open_preset_library: false,
            open_random_generator: false,
            fractal_viewport_size: None,
            needs_repaint: false,
            selected_preset_config: None,
            file_browser_open_requested: false,
            animation_export_requested: None,
            animation_seek_changed: false,
            animation_seek_drag_stopped: false,
            path_filters_changed: None,
            generated_flame: None,
            generated_batch: None,
            load_audio_file: false,
            load_signal_file: false,
            save_signal_file: None,
            #[cfg(feature = "api")]
            save_online_requested: false,
        }
    }
}

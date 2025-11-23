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

    // Palette library management (not stored in config directly)
    pub custom_palette: Option<crate::scene::palette::Palette>,
    pub palette_export_json: Option<crate::scene::palette::Palette>,
    pub palette_save_file: Option<crate::scene::palette::Palette>,
    pub palette_import_json: Option<String>,
    pub palette_load_file: bool,
    // pub palette_imported: Option<crate::scene::palette::Palette>,

    // Undo/redo (handled by ConfigManager but triggered from UI)
    pub undo_requested: bool,
    pub redo_requested: bool,

    // Export operations
    pub png_export_with_background: bool,
    pub png_export_transparent: bool,

    // Transform management (creates config deltas but needs special handling)
    pub add_transform: bool,
    pub delete_transform: Option<usize>,

    // Panel open requests
    pub open_palette_editor: bool,
    pub open_config_dialog: bool,

    // Fractal viewport size (for matching texture dimensions to panel)
    pub fractal_viewport_size: Option<(u32, u32)>,

    // UI interaction state (for frame rate optimization)
    pub needs_repaint: bool,
}

impl Default for UiResponse {
    fn default() -> Self {
        Self {
            config_export_requested: None,
            config_import_requested: None,
            config_save_file_requested: false,
            config_load_file_requested: false,
            apophysis_import_file_requested: false,
            custom_palette: None,
            palette_export_json: None,
            palette_save_file: None,
            palette_import_json: None,
            palette_load_file: false,
            // palette_imported: None,
            undo_requested: false,
            redo_requested: false,
            png_export_with_background: false,
            png_export_transparent: false,
            add_transform: false,
            delete_transform: None,
            open_palette_editor: false,
            open_config_dialog: false,
            fractal_viewport_size: None,
            needs_repaint: false,
        }
    }
}

//! Context struct for menu bar to reduce parameter count

/// Actions that can be triggered from the File menu
#[derive(Default)]
pub struct FileMenuActions {
    pub new_flame: bool,
    pub load_config: bool,
    pub save_config: bool,
    pub import_apophysis: bool,
    pub export_png: bool,
    pub quit: bool,
}

/// Actions that can be triggered from the Edit menu
#[derive(Default)]
pub struct EditMenuActions {
    pub undo: bool,
    pub redo: bool,
    pub copy_transform: bool,
    pub paste_transform: bool,
    pub duplicate_transform: bool,
}

/// Actions that can be triggered from the View menu
#[derive(Default)]
pub struct ViewMenuActions {
    pub reset_view: bool,
    pub fit_to_window: bool,
    pub zoom_in: bool,
    pub zoom_out: bool,
    pub set_mode_2d: bool,
    pub set_mode_3d: bool,
    pub show_grid: bool,
}

/// Actions that can be triggered from the Transform menu
#[derive(Default)]
pub struct TransformMenuActions {
    pub add_transform: bool,
    pub delete_transform: Option<usize>,
    pub randomize_transform: bool,
}

/// Actions that can be triggered from the Rendering menu
#[derive(Default)]
pub struct RenderingMenuActions {
    pub pause_toggle: bool,
    pub reset_accumulation: bool,
    pub set_speed: Option<u32>, // Speed multiplier (1, 2, 4, 8, 16)
    pub set_iterations_per_thread: Option<u32>, // Iterations per thread (64, 128, 256, 512, 1024)
}

/// Combined context for all menu actions
#[derive(Default)]
pub struct MenuActions {
    pub file: FileMenuActions,
    pub edit: EditMenuActions,
    pub view: ViewMenuActions,
    pub transform: TransformMenuActions,
    pub rendering: RenderingMenuActions,
}

/// Read-only state needed by menus to determine enabled/disabled state
pub struct MenuState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub is_paused: bool,
    pub render_mode_2d: bool, // true = 2D, false = 3D
}

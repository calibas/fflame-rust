//! Context struct for menu bar to reduce parameter count

/// Actions that can be triggered from the File menu
#[derive(Default)]
pub struct FileMenuActions {
    pub new_flame: bool,
    pub random_flame: bool,
    pub open_preset_library: bool,
    pub load_config: bool,
    pub save_config: bool,
    pub import_flame_xml: bool,
    pub export_flame_xml: bool,
    pub export_png: bool,
    pub export_png_transparent: bool,
    pub sign_out: bool,
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

/// Actions that can be triggered from the Rendering menu
#[derive(Default)]
pub struct RenderingMenuActions {
    pub pause_toggle: bool,
    pub reset_accumulation: bool,
    pub set_iterations_per_thread: Option<u32>, // Iterations per thread (64, 128, 256, 512, 1024)
    pub reset_to_defaults: bool,
}

/// Animation playback actions (compact menu's play/pause/stop).
///
/// These drive the AnimationController — deliberately NOT the render
/// pause. The compact menu's transport buttons used to pause the
/// renderer itself, which reads as "the app froze" on a phone; what a
/// transport row means everywhere else is the animation.
#[derive(Default)]
pub struct AnimationMenuActions {
    /// Toggle: pause if playing, play (from the current position) if not.
    pub play_pause: bool,
    /// Stop and rewind to t=0.
    pub stop: bool,
}

/// Combined context for all menu actions
#[derive(Default)]
pub struct MenuActions {
    pub file: FileMenuActions,
    pub edit: EditMenuActions,
    pub view: ViewMenuActions,
    pub rendering: RenderingMenuActions,
    pub animation: AnimationMenuActions,
    /// Menu-bar Fly Mode toggle button was clicked this frame.
    pub fly_mode_toggle: bool,
}

/// Read-only state needed by menus to determine enabled/disabled state
pub struct MenuState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub is_paused: bool,
    pub render_mode_2d: bool, // true = 2D, false = 3D
    pub online_mode: bool,
    pub has_api_flame_id: bool,
    pub api_flame_id: Option<String>,
    pub api_flame_is_public: Option<bool>,
    pub has_animation_tracks: bool,
    /// Whether the animation controller is currently playing (drives
    /// the compact menu's play/pause label).
    pub animation_playing: bool,
    pub api_animation_id: Option<String>,
    pub animation_count: u32,
    pub flame_owned: bool,
    pub animation_owned: bool,
    pub flame_name: String,
    pub auth_email: Option<String>,
    pub api_connectivity: crate::api::ApiConnectivity,
    /// Fly Mode currently active — drives the menu-bar toggle's green
    /// highlight.
    pub fly_mode_active: bool,
}

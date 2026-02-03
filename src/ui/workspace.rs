//! Workspace layout management for docking panels
//!
//! Manages the egui_dock DockState and provides default layouts
//! for different workflows (Beginner, Standard, Advanced, Export).

use egui_dock::{DockState};
use rust_i18n::t;
use serde::{Deserialize, Serialize};

/// Identifies which panel to display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelType {
    /// Fractal viewport (main fractal rendering)
    FractalViewport,
    /// Transform list, affine, variations (existing Transforms window)
    Transforms,
    /// Visual triangle editor (existing Triangle Editor window)
    TriangleEditor,
    /// Color mode, palette, tone mapping (existing Tone Mapping & Colors window)
    Colors,
    /// Palette editing (existing Palette Editor window)
    PaletteEditor,
    /// Palette library (browse and manage palette packs)
    PaletteLibrary,
    /// Unified fractal browser (presets + batch + files in tabs)
    FractalBrowser,
    /// Camera/navigation (zoom, pan, rotation, 3D camera)
    View,
    /// Performance/quality (iterations, accumulation, speed)
    Rendering,
    /// Undo/redo history browser
    History,
    /// Animation playback controls
    Animation,
    /// Performance stats and version info
    Performance,
    /// Help intro panel
    Help,
    /// Keyboard shortcuts reference
    KeyboardShortcuts,
    /// Config import/export dialog
    ConfigDialog,
    /// Path filter editor (block specific transform sequences)
    PathEditor,
    /// PNG Export panel
    Export,
    /// Random generator panel (generate random flames with settings)
    RandomGenerator,
    /// Post-processing effects panel (color and density effects)
    Effects,
    /// Xaos editor panel (chaos-weighted transform transitions)
    XaosEditor,
}

impl std::fmt::Display for PanelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let title = match self {
            PanelType::FractalViewport => t!("panels.fractal"),
            PanelType::Transforms => t!("panels.transforms"),
            PanelType::TriangleEditor => t!("panels.triangle_editor"),
            PanelType::Colors => t!("panels.colors"),
            PanelType::PaletteEditor => t!("panels.palette_editor"),
            PanelType::PaletteLibrary => t!("panels.palette_library"),
            PanelType::FractalBrowser => t!("browser.title"),
            PanelType::View => t!("panels.view"),
            PanelType::Rendering => t!("panels.rendering"),
            PanelType::History => t!("panels.history"),
            PanelType::Animation => t!("panels.animation"),
            PanelType::Performance => t!("panels.performance"),
            PanelType::Help => t!("panels.help"),
            PanelType::KeyboardShortcuts => t!("panels.keyboard_shortcuts"),
            PanelType::ConfigDialog => t!("panels.config_dialog"),
            PanelType::PathEditor => t!("panels.path_editor"),
            PanelType::Export => t!("panels.export"),
            PanelType::RandomGenerator => t!("panels.random_generator"),
            PanelType::Effects => t!("panels.effects"),
            PanelType::XaosEditor => t!("panels.xaos_editor"),
        };
        write!(f, "{}", title)
    }
}

/// Workspace layout presets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceLayout {
    /// Beginner: Fractal + Appearance only
    Beginner,
    /// Standard: Fractal + Transform Editor + Appearance + View
    Standard,
    /// Animation: Standard layout with Animation panel at bottom
    Animation,
    /// Advanced: All panels visible, split layout
    Advanced,
    /// Export: Rendering + Appearance focused
    Export,
}

/// Manages the docking workspace state
pub struct Workspace {
    /// Single dock state for all panels (dynamic docking to left/right/bottom edges)
    pub dock_state: DockState<PanelType>,
    /// Active layout preset
    pub current_layout: WorkspaceLayout,
}

impl Workspace {
    /// Create a new workspace with default Standard layout
    pub fn new() -> Self {
        Self {
            dock_state: Self::create_standard_layout(),
            current_layout: WorkspaceLayout::Standard,
        }
    }

    /// Get the default size for a panel type
    fn default_size_for_panel(panel_type: PanelType) -> egui::Vec2 {
        match panel_type {
            PanelType::FractalViewport => egui::vec2(800.0, 600.0),
            PanelType::Transforms => egui::vec2(350.0, 500.0),
            PanelType::TriangleEditor => egui::vec2(450.0, 450.0),
            PanelType::Colors => egui::vec2(350.0, 400.0),
            PanelType::PaletteEditor => egui::vec2(350.0, 450.0),
            PanelType::PaletteLibrary => egui::vec2(500.0, 500.0),
            PanelType::FractalBrowser => egui::vec2(600.0, 500.0),
            PanelType::View => egui::vec2(350.0, 350.0),
            PanelType::Rendering => egui::vec2(350.0, 350.0),
            PanelType::History => egui::vec2(350.0, 400.0),
            PanelType::Animation => egui::vec2(600.0, 300.0),
            PanelType::Performance => egui::vec2(350.0, 300.0),
            PanelType::Help => egui::vec2(400.0, 350.0),
            PanelType::KeyboardShortcuts => egui::vec2(400.0, 500.0),
            PanelType::ConfigDialog => egui::vec2(350.0, 300.0),
            PanelType::PathEditor => egui::vec2(350.0, 350.0),
            PanelType::Export => egui::vec2(350.0, 400.0),
            PanelType::RandomGenerator => egui::vec2(400.0, 450.0),
            PanelType::Effects => egui::vec2(350.0, 400.0),
            PanelType::XaosEditor => egui::vec2(500.0, 450.0),
        }
    }

    /// Apply a predefined layout
    pub fn apply_layout(&mut self, layout: WorkspaceLayout) {
        self.dock_state = match layout {
            WorkspaceLayout::Beginner => Self::create_beginner_layout(),
            WorkspaceLayout::Standard => Self::create_standard_layout(),
            WorkspaceLayout::Animation => Self::create_animation_layout(),
            WorkspaceLayout::Advanced => Self::create_advanced_layout(),
            WorkspaceLayout::Export => Self::create_export_layout(),
        };
        self.current_layout = layout;
    }

    /// Check if a panel type already exists in the dock state
    pub fn panel_exists(&self, panel_type: PanelType) -> bool {
        self.dock_state.iter_all_tabs().any(|(_, tab)| *tab == panel_type)
    }

    /// Open a panel as a floating window, or focus it if it already exists
    pub fn open_floating_panel(&mut self, panel_type: PanelType, ctx: &egui::Context) {
        if let Some((surface_index, node_index, tab_index)) = self.find_tab(panel_type) {
            // Panel exists - check if collapsed
            let is_collapsed = self.dock_state[surface_index][node_index].is_collapsed();

            if is_collapsed {
                // Nuclear option: remove collapsed window and recreate it fresh
                self.dock_state.remove_surface(surface_index);

                // Create new window
                self.dock_state.add_window(vec![panel_type]);

                // Find the newly created window (surface indices may have shifted after removal!)
                if let Some((new_surface_index, new_node_index, new_tab_index)) = self.find_tab(panel_type) {
                    // Set initial size based on panel type
                    let initial_size = Self::default_size_for_panel(panel_type);
                    if let Some(surface) = self.dock_state.get_surface_mut(new_surface_index) {
                        if let egui_dock::Surface::Window(_, state) = surface {
                            state.set_size(initial_size);
                        }
                    }

                    // Bring new window to front and activate it
                    if !new_surface_index.is_main() {
                        let window_id = egui::Id::new(format!("window {new_surface_index:?}"));
                        let layer_id = egui::LayerId::new(egui::Order::Middle, window_id);
                        ctx.move_to_top(layer_id);
                    }

                    // Focus the newly created window
                    self.dock_state.set_focused_node_and_surface((new_surface_index, new_node_index));
                    self.dock_state.set_active_tab((new_surface_index, new_node_index, new_tab_index));
                }
            } else {
                // Not collapsed - just bring to front and activate
                if !surface_index.is_main() {
                    let window_id = egui::Id::new(format!("window {surface_index:?}"));
                    let layer_id = egui::LayerId::new(egui::Order::Middle, window_id);
                    ctx.move_to_top(layer_id);
                }

                self.dock_state.set_focused_node_and_surface((surface_index, node_index));
                self.dock_state.set_active_tab((surface_index, node_index, tab_index));
            }
        } else {
            // Panel doesn't exist - create new floating window
            self.dock_state.add_window(vec![panel_type]);

            // Set initial size based on panel type
            // This ensures panels open at a reasonable size (not collapsed)
            let initial_size = Self::default_size_for_panel(panel_type);
            let surface_count = self.dock_state.iter_surfaces().count();
            if surface_count > 0 {
                let surface_index = egui_dock::SurfaceIndex(surface_count - 1);
                if let Some(surface) = self.dock_state.get_surface_mut(surface_index) {
                    if let egui_dock::Surface::Window(_, state) = surface {
                        state.set_size(initial_size);
                    }
                }
            }
        }
    }

    /// Open a panel as a floating window with specific size and centered position
    /// Size is (width, height), window will be centered on screen
    pub fn open_floating_panel_centered(&mut self, panel_type: PanelType, size: egui::Vec2, screen_size: egui::Vec2) {
        if !self.panel_exists(panel_type) {
            self.dock_state.add_window(vec![panel_type]);

            // Find the newly created window surface and set its position/size
            // Windows are added at the end of the surfaces list
            let surface_count = self.dock_state.iter_surfaces().count();
            if surface_count > 0 {
                let surface_index = egui_dock::SurfaceIndex(surface_count - 1);
                if let Some(surface) = self.dock_state.get_surface_mut(surface_index) {
                    // Surface::Window(Tree, WindowState)
                    if let egui_dock::Surface::Window(_, state) = surface {
                        let pos = egui::pos2(
                            (screen_size.x - size.x) / 2.0,
                            (screen_size.y - size.y) / 2.0,
                        );
                        state.set_position(pos);
                        state.set_size(size);
                    }
                }
            }
        }
    }

    /// Activate (focus) a panel tab if it exists, or open it as floating if not
    pub fn activate_panel(&mut self, panel_type: PanelType) {
        // Try to find and activate the tab
        if let Some((surface_index, node_index, tab_index)) = self.find_tab(panel_type) {
            self.dock_state.set_active_tab((surface_index, node_index, tab_index));
        } else {
            // Panel doesn't exist, open as floating window
            self.dock_state.add_window(vec![panel_type]);
        }
    }

    /// Find the location of a panel tab (surface, node, tab index)
    fn find_tab(&self, panel_type: PanelType) -> Option<(egui_dock::SurfaceIndex, egui_dock::NodeIndex, egui_dock::TabIndex)> {
        // Use find_tab API to locate the panel
        self.dock_state.find_tab(&panel_type)
    }

    /// Create Beginner layout: Simple tabbed panel with Colors and Transforms
    fn create_beginner_layout() -> DockState<PanelType> {
        let mut state = DockState::new(vec![PanelType::Colors]);
        let root = state.main_surface_mut();
        root.push_to_focused_leaf(PanelType::Transforms);
        state
    }

    /// Create Standard layout: Fractal in center, controls on sides
    fn create_standard_layout() -> DockState<PanelType> {
        // Start with FractalViewport in the center
        let mut state = DockState::new(vec![PanelType::FractalViewport]);

        // Split left for Transforms
        let [_fractal_node, _left_node] = state.main_surface_mut().split_left(
            egui_dock::NodeIndex::root(),
            0.25, // 25% width for left panel
            vec![PanelType::Transforms],
        );

        // Split right for other controls (Colors, View, Triangle Editor, Rendering, History)
        let [_fractal_node, _right_node] = state.main_surface_mut().split_right(
            egui_dock::NodeIndex::root(),
            0.75, // Right panel starts at 75% (takes remaining 25%)
            vec![PanelType::Colors, PanelType::View, PanelType::TriangleEditor, PanelType::Rendering, PanelType::History],
        );

        state
    }

    /// Create Animation layout: Standard layout with Animation panel at bottom center
    fn create_animation_layout() -> DockState<PanelType> {
        // Start with FractalViewport in the center
        let mut state = DockState::new(vec![PanelType::FractalViewport]);

        // Split bottom of the center area for Animation panel
        let [_top_node, _bottom_node] = state.main_surface_mut().split_below(
            egui_dock::NodeIndex::root(),
            0.75, // Animation panel takes bottom 25% of center
            vec![PanelType::Animation],
        );

        // Split left for Transforms
        let [_fractal_node, _left_node] = state.main_surface_mut().split_left(
            egui_dock::NodeIndex::root(),
            0.25, // 25% width for left panel
            vec![PanelType::Transforms],
        );

        // Split right for other controls (Colors, View, Triangle Editor, Rendering, History)
        let [_center_node, _right_node] = state.main_surface_mut().split_right(
            egui_dock::NodeIndex::root(),
            0.75, // Right panel starts at 75% (takes remaining 25%)
            vec![PanelType::Colors, PanelType::View, PanelType::TriangleEditor, PanelType::Rendering, PanelType::History],
        );

        state
    }

    /// Create Advanced layout: All panels visible in tabs
    fn create_advanced_layout() -> DockState<PanelType> {
        let mut state = DockState::new(vec![PanelType::Transforms]);
        let root = state.main_surface_mut();
        root.push_to_focused_leaf(PanelType::TriangleEditor);
        root.push_to_focused_leaf(PanelType::Colors);
        root.push_to_focused_leaf(PanelType::PaletteEditor);
        root.push_to_focused_leaf(PanelType::View);
        root.push_to_focused_leaf(PanelType::Rendering);
        root.push_to_focused_leaf(PanelType::History);
        state
    }

    /// Create Export layout: Focus on rendering and colors
    fn create_export_layout() -> DockState<PanelType> {
        let mut state = DockState::new(vec![PanelType::Rendering]);
        let root = state.main_surface_mut();
        root.push_to_focused_leaf(PanelType::Colors);
        state
    }

}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

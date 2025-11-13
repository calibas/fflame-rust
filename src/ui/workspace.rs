//! Workspace layout management for docking panels
//!
//! Manages the egui_dock DockState and provides default layouts
//! for different workflows (Beginner, Standard, Advanced, Export).

use egui_dock::{egui, DockState, NodeIndex, Style};
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
    /// Camera/navigation (zoom, pan, rotation, 3D camera)
    View,
    /// Performance/quality (iterations, accumulation, speed)
    Rendering,
    /// Undo/redo history browser
    History,
    /// Performance stats and version info
    Performance,
    /// Help and keyboard shortcuts
    Help,
    /// Config import/export dialog
    ConfigDialog,
}

impl std::fmt::Display for PanelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PanelType::FractalViewport => write!(f, "Fractal"),
            PanelType::Transforms => write!(f, "Transforms"),
            PanelType::TriangleEditor => write!(f, "Triangle Editor"),
            PanelType::Colors => write!(f, "Colors"),
            PanelType::PaletteEditor => write!(f, "Palette Editor"),
            PanelType::View => write!(f, "View"),
            PanelType::Rendering => write!(f, "Rendering"),
            PanelType::History => write!(f, "History"),
            PanelType::Performance => write!(f, "Performance"),
            PanelType::Help => write!(f, "Help"),
            PanelType::ConfigDialog => write!(f, "Config Import/Export"),
        }
    }
}

/// Workspace layout presets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceLayout {
    /// Beginner: Fractal + Appearance only
    Beginner,
    /// Standard: Fractal + Transform Editor + Appearance + View
    Standard,
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

    /// Apply a predefined layout
    pub fn apply_layout(&mut self, layout: WorkspaceLayout) {
        self.dock_state = match layout {
            WorkspaceLayout::Beginner => Self::create_beginner_layout(),
            WorkspaceLayout::Standard => Self::create_standard_layout(),
            WorkspaceLayout::Advanced => Self::create_advanced_layout(),
            WorkspaceLayout::Export => Self::create_export_layout(),
        };
        self.current_layout = layout;
    }

    /// Check if a panel type already exists in the dock state
    pub fn panel_exists(&self, panel_type: PanelType) -> bool {
        self.dock_state.iter_all_tabs().any(|(_, tab)| *tab == panel_type)
    }

    /// Open a panel as a floating window (only if it doesn't already exist)
    pub fn open_floating_panel(&mut self, panel_type: PanelType) {
        if !self.panel_exists(panel_type) {
            self.dock_state.add_window(vec![panel_type]);
        }
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
        let [_fractal_node, left_node] = state.main_surface_mut().split_left(
            egui_dock::NodeIndex::root(),
            0.25, // 25% width for left panel
            vec![PanelType::Transforms],
        );

        // Split right for other controls (Colors, View, Rendering, History)
        let [_fractal_node, right_node] = state.main_surface_mut().split_right(
            egui_dock::NodeIndex::root(),
            0.75, // Right panel starts at 75% (takes remaining 25%)
            vec![PanelType::Colors, PanelType::View, PanelType::Rendering, PanelType::History],
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

    /// Get the dock style (visual appearance)
    pub fn style() -> Style {
        Style::from_egui(&egui::Style::default())
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

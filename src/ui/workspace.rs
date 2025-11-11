//! Workspace layout management for docking panels
//!
//! Manages the egui_dock DockState and provides default layouts
//! for different workflows (Beginner, Standard, Advanced, Export).

use egui_dock::{egui, DockState, NodeIndex, Style};
use serde::{Deserialize, Serialize};

/// Identifies which panel to display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelType {
    /// Main fractal controls (transforms list, add/delete)
    Fractal,
    /// Detailed transform editing (affine, variations, triangle editor)
    TransformEditor,
    /// Visual controls (palette, color mode, tone mapping)
    Appearance,
    /// Camera/navigation (zoom, pan, rotation, 3D camera)
    View,
    /// Performance/quality (iterations, accumulation, speed)
    Rendering,
    /// Expert features (histogram scale, per-pixel limit, etc.)
    Advanced,
    /// Undo/redo history browser
    History,
}

impl std::fmt::Display for PanelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PanelType::Fractal => write!(f, "Fractal"),
            PanelType::TransformEditor => write!(f, "Transform Editor"),
            PanelType::Appearance => write!(f, "Appearance"),
            PanelType::View => write!(f, "View"),
            PanelType::Rendering => write!(f, "Rendering"),
            PanelType::Advanced => write!(f, "Advanced"),
            PanelType::History => write!(f, "History"),
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
    /// Current dock state
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

    /// Create Beginner layout: Fractal + Appearance
    fn create_beginner_layout() -> DockState<PanelType> {
        let mut state = DockState::new(vec![PanelType::Fractal]);
        let root = state.main_surface_mut();
        let [_left, _right] = root.split_right(NodeIndex::root(), 0.25, vec![PanelType::Appearance]);
        state
    }

    /// Create Standard layout: Fractal + Transform Editor | Appearance + View
    fn create_standard_layout() -> DockState<PanelType> {
        let mut state = DockState::new(vec![PanelType::Fractal]);
        let root = state.main_surface_mut();

        // Split right for Appearance
        let [left, right] = root.split_right(NodeIndex::root(), 0.25, vec![PanelType::Appearance]);

        // Add View tab to right side
        root.push_to_focused_leaf(PanelType::View);

        // Add Transform Editor tab to left side
        root.set_focused_node(left);
        root.push_to_focused_leaf(PanelType::TransformEditor);

        state
    }

    /// Create Advanced layout: All panels in split layout
    fn create_advanced_layout() -> DockState<PanelType> {
        let mut state = DockState::new(vec![PanelType::Fractal]);
        let root = state.main_surface_mut();

        // Split into left (Fractal + Transform Editor) and right (Appearance + View + Rendering + Advanced)
        let [left, right] = root.split_right(NodeIndex::root(), 0.3, vec![PanelType::Appearance]);

        // Add panels to left side
        root.set_focused_node(left);
        root.push_to_focused_leaf(PanelType::TransformEditor);
        root.push_to_focused_leaf(PanelType::History);

        // Add panels to right side
        root.set_focused_node(right);
        root.push_to_focused_leaf(PanelType::View);
        root.push_to_focused_leaf(PanelType::Rendering);
        root.push_to_focused_leaf(PanelType::Advanced);

        state
    }

    /// Create Export layout: Rendering + Appearance focused
    fn create_export_layout() -> DockState<PanelType> {
        let mut state = DockState::new(vec![PanelType::Rendering]);
        let root = state.main_surface_mut();
        let [_left, _right] = root.split_right(NodeIndex::root(), 0.25, vec![PanelType::Appearance]);
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

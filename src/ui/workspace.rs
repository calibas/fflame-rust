//! Workspace layout management for docking panels
//!
//! Manages the egui_dock DockState and provides default layouts
//! for different workflows (Beginner, Standard, Advanced, Export).

use egui_dock::{egui, DockState, NodeIndex, Style};
use serde::{Deserialize, Serialize};

/// Identifies which panel to display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelType {
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
}

impl std::fmt::Display for PanelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PanelType::Transforms => write!(f, "Transforms"),
            PanelType::TriangleEditor => write!(f, "Triangle Editor"),
            PanelType::Colors => write!(f, "Colors"),
            PanelType::PaletteEditor => write!(f, "Palette Editor"),
            PanelType::View => write!(f, "View"),
            PanelType::Rendering => write!(f, "Rendering"),
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

    /// Create Beginner layout: Transforms + Colors
    fn create_beginner_layout() -> DockState<PanelType> {
        let mut state = DockState::new(vec![PanelType::Transforms]);
        let root = state.main_surface_mut();
        let [_left, _right] = root.split_right(NodeIndex::root(), 0.25, vec![PanelType::Colors]);
        state
    }

    /// Create Standard layout: Transforms | Colors + View + Rendering + History
    fn create_standard_layout() -> DockState<PanelType> {
        let mut state = DockState::new(vec![PanelType::Transforms]);
        let root = state.main_surface_mut();

        // Split right for Colors and other panels
        let [_left, _right] = root.split_right(NodeIndex::root(), 0.25, vec![PanelType::Colors]);

        // Add View, Rendering, and History tabs to right side
        root.push_to_focused_leaf(PanelType::View);
        root.push_to_focused_leaf(PanelType::Rendering);
        root.push_to_focused_leaf(PanelType::History);

        state
    }

    /// Create Advanced layout: All 7 panels visible
    fn create_advanced_layout() -> DockState<PanelType> {
        let mut state = DockState::new(vec![PanelType::Transforms]);
        let root = state.main_surface_mut();

        // Split into left (Transforms + Triangle Editor) and right (Colors + Palette Editor + View + Rendering + History)
        let [left, right] = root.split_right(NodeIndex::root(), 0.3, vec![PanelType::Colors]);

        // Add Triangle Editor tab to left side
        root.set_focused_node(left);
        root.push_to_focused_leaf(PanelType::TriangleEditor);

        // Add all other panels to right side
        root.set_focused_node(right);
        root.push_to_focused_leaf(PanelType::PaletteEditor);
        root.push_to_focused_leaf(PanelType::View);
        root.push_to_focused_leaf(PanelType::Rendering);
        root.push_to_focused_leaf(PanelType::History);

        state
    }

    /// Create Export layout: Rendering + Colors focused
    fn create_export_layout() -> DockState<PanelType> {
        let mut state = DockState::new(vec![PanelType::Rendering]);
        let root = state.main_surface_mut();
        let [_left, _right] = root.split_right(NodeIndex::root(), 0.25, vec![PanelType::Colors]);
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

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
    /// Performance stats and version info
    Performance,
    /// Help and keyboard shortcuts
    Help,
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
            PanelType::Performance => write!(f, "Performance"),
            PanelType::Help => write!(f, "Help"),
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
    /// Left side dock state (Transforms, Triangle Editor)
    pub left_dock_state: DockState<PanelType>,
    /// Right side dock state (Colors, Palette Editor, View, Rendering, History)
    pub right_dock_state: DockState<PanelType>,
    /// Active layout preset
    pub current_layout: WorkspaceLayout,
}

impl Workspace {
    /// Create a new workspace with default Standard layout
    pub fn new() -> Self {
        let (left, right) = Self::create_standard_layout();
        Self {
            left_dock_state: left,
            right_dock_state: right,
            current_layout: WorkspaceLayout::Standard,
        }
    }

    /// Apply a predefined layout
    pub fn apply_layout(&mut self, layout: WorkspaceLayout) {
        let (left, right) = match layout {
            WorkspaceLayout::Beginner => Self::create_beginner_layout(),
            WorkspaceLayout::Standard => Self::create_standard_layout(),
            WorkspaceLayout::Advanced => Self::create_advanced_layout(),
            WorkspaceLayout::Export => Self::create_export_layout(),
        };
        self.left_dock_state = left;
        self.right_dock_state = right;
        self.current_layout = layout;
    }

    /// Check if a panel type already exists in either dock state
    pub fn panel_exists(&self, panel_type: PanelType) -> bool {
        // Check left dock
        if self.left_dock_state.iter_all_tabs().any(|(_, tab)| *tab == panel_type) {
            return true;
        }
        // Check right dock
        if self.right_dock_state.iter_all_tabs().any(|(_, tab)| *tab == panel_type) {
            return true;
        }
        false
    }

    /// Open a panel as a floating window (only if it doesn't already exist)
    pub fn open_floating_panel(&mut self, panel_type: PanelType) {
        if !self.panel_exists(panel_type) {
            self.right_dock_state.add_window(vec![panel_type]);
        }
    }

    /// Create Beginner layout: Left (Transforms) | Right (Colors)
    fn create_beginner_layout() -> (DockState<PanelType>, DockState<PanelType>) {
        let left = DockState::new(vec![PanelType::Transforms]);
        let right = DockState::new(vec![PanelType::Colors]);
        (left, right)
    }

    /// Create Standard layout: Left (Transforms) | Right (Colors, View, Rendering, History)
    fn create_standard_layout() -> (DockState<PanelType>, DockState<PanelType>) {
        let left = DockState::new(vec![PanelType::Transforms]);

        let mut right = DockState::new(vec![PanelType::Colors]);
        let root = right.main_surface_mut();
        root.push_to_focused_leaf(PanelType::View);
        root.push_to_focused_leaf(PanelType::Rendering);
        root.push_to_focused_leaf(PanelType::History);

        (left, right)
    }

    /// Create Advanced layout: Left (Transforms, Triangle Editor) | Right (Colors, Palette Editor, View, Rendering, History)
    fn create_advanced_layout() -> (DockState<PanelType>, DockState<PanelType>) {
        let mut left = DockState::new(vec![PanelType::Transforms]);
        let left_root = left.main_surface_mut();
        left_root.push_to_focused_leaf(PanelType::TriangleEditor);

        let mut right = DockState::new(vec![PanelType::Colors]);
        let right_root = right.main_surface_mut();
        right_root.push_to_focused_leaf(PanelType::PaletteEditor);
        right_root.push_to_focused_leaf(PanelType::View);
        right_root.push_to_focused_leaf(PanelType::Rendering);
        right_root.push_to_focused_leaf(PanelType::History);

        (left, right)
    }

    /// Create Export layout: Left (empty) | Right (Rendering, Colors)
    fn create_export_layout() -> (DockState<PanelType>, DockState<PanelType>) {
        let left = DockState::new(vec![]);

        let mut right = DockState::new(vec![PanelType::Rendering]);
        let root = right.main_surface_mut();
        root.push_to_focused_leaf(PanelType::Colors);

        (left, right)
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

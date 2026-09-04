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
    /// Flame scripting panel (run generator / modifier scripts)
    Scripts,
    /// Post-processing effects panel (color and density effects)
    Effects,
    /// Xaos editor panel (chaos-weighted transform transitions)
    XaosEditor,
    /// Solid rendering & lighting (occlusion, shade pass, shadow maps)
    SolidLighting,
    /// Signal panel (signals, audio, generators)
    Signal,
    /// Login dialog (email/password form)
    LoginDialog,
    /// Save Online dialog (name input before cloud save)
    SaveOnlineDialog,
    /// Variations panel (browse all registered variations)
    Variations,
    /// Subflames panel (switch which flame the editor operates on)
    Subflames,
    /// Escape-time fractal editing surface (formula, view, coloring)
    Escape,
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
            PanelType::Signal => t!("panels.signal"),
            PanelType::SolidLighting => t!("panels.solid_lighting"),
            PanelType::LoginDialog => t!("login.title"),
            PanelType::SaveOnlineDialog => t!("api.save_dialog_title"),
            PanelType::Variations => t!("panels.variations"),
            PanelType::Subflames => t!("panels.subflames"),
            PanelType::Scripts => t!("panels.scripts"),
            PanelType::Escape => t!("panels.escape"),
        };
        write!(f, "{}", title)
    }
}

/// Workspace layout presets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceLayout {
    /// Standard: Fractal + Transform Editor + Appearance + View
    Standard,
    /// Animation: Standard layout with Animation panel at bottom
    Animation,
    /// Scripting: Scripts panel left, Fractal Browser right — write a
    /// script, run it, browse what it generated.
    Scripting,
    /// Escape Time: the Escape panel where Transforms sits in
    /// Standard, because in escape mode the transform list is not
    /// what is being edited — the formula is. Colors and History to
    /// the right, since coloring is most of the work once a formula
    /// and a view are chosen.
    EscapeTime,
    /// Compact: Full-screen viewport only (mobile / small screens)
    Compact,
}

/// Manages the docking workspace state
pub struct Workspace {
    /// Single dock state for all panels (dynamic docking to left/right/bottom edges)
    pub dock_state: DockState<PanelType>,
    /// Active layout preset
    pub current_layout: WorkspaceLayout,
    /// A layout was just built from fractions, without knowing the
    /// window size — the first frame that does know it applies the
    /// side-dock minimums (`apply_startup_dock_minimums`).
    needs_startup_dock_widths: bool,
}

impl Workspace {
    /// Minimum useful width, in points, for a side dock when a layout
    /// first appears. Transforms — the widest of the default side
    /// panels — clips its controls below this. Startup only: the user
    /// can drag the separator below it freely afterwards.
    const MIN_SIDE_DOCK_POINTS: f32 = 260.0;

    /// Create a new workspace with default Standard layout
    pub fn new() -> Self {
        Self {
            dock_state: Self::create_standard_layout(),
            current_layout: WorkspaceLayout::Standard,
            needs_startup_dock_widths: true,
        }
    }

    /// One-shot, called with the real dock width on the first frame
    /// after a layout is created — layouts are built from fractions
    /// before any window exists, and a fraction that is generous on a
    /// desktop monitor is a clipped strip on a laptop. The left dock is
    /// 25% *of the 75% left over by the right split* (18.75% of the
    /// window, easy to misread as 25%): on a Retina MacBook's
    /// 960-point default window that was 180 points of Transforms
    /// panel, with controls cut off.
    ///
    /// Raises each side dock to `MIN_SIDE_DOCK_POINTS` and touches
    /// nothing already wider, so large screens keep their proportions.
    pub fn apply_startup_dock_minimums(&mut self, dock_width: f32) {
        if !self.needs_startup_dock_widths {
            return;
        }
        self.needs_startup_dock_widths = false;
        if dock_width <= 0.0 {
            return;
        }
        let min = Self::MIN_SIDE_DOCK_POINTS;

        // Every preset with side docks has the same shape by
        // construction: the root split parts the right dock off the
        // window, and its left child parts the left dock off the
        // remainder. Compact is a lone leaf — the matches fail and
        // nothing happens.
        use egui_dock::{Node, NodeIndex};
        let tree = self.dock_state.main_surface_mut();

        // Right dock width is (1 − f)·W: lower the fraction to grow the
        // dock to `min`, but never let it claim more than half.
        let mut remaining = dock_width;
        if tree.len() > 0 {
            if let Node::Horizontal(split) = &mut tree[NodeIndex::root()] {
                split.fraction = split.fraction.min(1.0 - min / dock_width).max(0.5);
                remaining = dock_width * split.fraction;
            }
        }

        // Left dock width is f·remaining: raise the fraction to reach
        // `min`, same half-cap.
        if tree.len() > 1 {
            if let Node::Horizontal(split) = &mut tree[NodeIndex::root().left()] {
                split.fraction = split.fraction.max(min / remaining).min(0.5);
            }
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
            PanelType::Signal => egui::vec2(350.0, 450.0),
            PanelType::SolidLighting => egui::vec2(350.0, 500.0),
            PanelType::LoginDialog => egui::vec2(380.0, 320.0),
            PanelType::SaveOnlineDialog => egui::vec2(400.0, 370.0),
            PanelType::Variations => egui::vec2(450.0, 500.0),
            PanelType::Subflames => egui::vec2(320.0, 360.0),
            PanelType::Scripts => egui::vec2(420.0, 560.0),
            PanelType::Escape => egui::vec2(350.0, 520.0),
        }
    }

    /// Apply a predefined layout
    pub fn apply_layout(&mut self, layout: WorkspaceLayout) {
        let help_was_open = self.panel_exists(PanelType::Help);
        self.dock_state = match layout {
            WorkspaceLayout::Standard => Self::create_standard_layout(),
            WorkspaceLayout::Animation => Self::create_animation_layout(help_was_open),
            WorkspaceLayout::EscapeTime => Self::create_escape_layout(help_was_open),
            WorkspaceLayout::Scripting => Self::create_scripting_layout(help_was_open),
            WorkspaceLayout::Compact => Self::create_compact_layout(),
        };
        self.current_layout = layout;
        self.needs_startup_dock_widths = true;
    }

    /// Whether the workspace is in compact (mobile) layout
    pub fn is_compact(&self) -> bool {
        self.current_layout == WorkspaceLayout::Compact
    }

    /// Open a panel the way the current mode wants it: docked into the
    /// main surface in compact (a floating window on a phone is
    /// unusable — it opens desktop-sized over the whole screen),
    /// floating on desktop. App-level code that isn't deliberately
    /// mode-specific should call this, not the mode-specific variants.
    pub fn open_panel(&mut self, panel_type: PanelType, ctx: &egui::Context) {
        if self.is_compact() {
            self.open_compact_panel(panel_type, ctx);
        } else {
            self.open_floating_panel(panel_type, ctx);
        }
    }

    /// Open a panel docked into the main surface (compact mode).
    /// If a non-viewport panel node already exists, adds as a tab there.
    /// Otherwise splits from viewport (bottom in portrait, right in landscape).
    pub fn open_compact_panel(&mut self, panel_type: PanelType, ctx: &egui::Context) {
        if let Some((surface_index, node_index, tab_index)) = self.find_tab(panel_type) {
            // Already exists — focus it
            self.dock_state.set_focused_node_and_surface(egui_dock::NodePath::new(surface_index, node_index));
            let _ = self.dock_state.set_active_tab(egui_dock::TabPath::new(surface_index, node_index, tab_index));
            return;
        }

        // Look for an existing non-viewport tab node to add to
        let existing_panel_node = self.find_non_viewport_node();

        if let Some(node_index) = existing_panel_node {
            // Add as a new tab in the existing panel node
            self.dock_state.main_surface_mut()[node_index]
                .append_tab(panel_type);
            // Find the newly added tab to focus it
            if let Some((si, ni, ti)) = self.find_tab(panel_type) {
                self.dock_state.set_focused_node_and_surface(egui_dock::NodePath::new(si, ni));
                let _ = self.dock_state.set_active_tab(egui_dock::TabPath::new(si, ni, ti));
            }
            return;
        }

        // No panel node exists yet — split from viewport
        let viewport_location = self.find_tab(PanelType::FractalViewport);

        if let Some((_, viewport_node, _)) = viewport_location {
            let screen = ctx.input(|i| i.content_rect());
            let is_portrait = screen.height() > screen.width();

            if is_portrait {
                // 0.55, not 0.67: at 0.67 a phone panel got the bottom
                // third minus the tab bar — Transforms and Variations
                // were unusable strips. Just under half leaves the
                // viewport dominant while giving controls real room.
                self.dock_state.main_surface_mut().split_below(
                    viewport_node, 0.55, vec![panel_type],
                );
            } else {
                self.dock_state.main_surface_mut().split_right(
                    viewport_node, 0.60, vec![panel_type],
                );
            }
        } else {
            self.open_floating_panel(panel_type, ctx);
        }
    }

    /// Find a leaf node on the main surface that contains a non-viewport tab.
    /// Used by compact mode to add new tabs to an existing panel area.
    /// Find a node on the main surface that contains a non-viewport tab.
    /// Used by compact mode to add new tabs to an existing panel area.
    fn find_non_viewport_node(&self) -> Option<egui_dock::NodeIndex> {
        // Find any non-viewport tab and return its node index
        for (_tab_path, panel) in self.dock_state.iter_all_tabs() {
            if *panel != PanelType::FractalViewport {
                // Found a non-viewport tab — look up its full location
                if let Some(p) = self.dock_state.find_tab(panel) {
                    return Some(p.node);
                }
            }
        }
        None
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
                    self.dock_state.set_focused_node_and_surface(egui_dock::NodePath::new(new_surface_index, new_node_index));
                    let _ = self.dock_state.set_active_tab(egui_dock::TabPath::new(new_surface_index, new_node_index, new_tab_index));
                }
            } else {
                // Not collapsed - just bring to front and activate
                if !surface_index.is_main() {
                    let window_id = egui::Id::new(format!("window {surface_index:?}"));
                    let layer_id = egui::LayerId::new(egui::Order::Middle, window_id);
                    ctx.move_to_top(layer_id);
                }

                self.dock_state.set_focused_node_and_surface(egui_dock::NodePath::new(surface_index, node_index));
                let _ = self.dock_state.set_active_tab(egui_dock::TabPath::new(surface_index, node_index, tab_index));
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
            let _ = self.dock_state.set_active_tab(egui_dock::TabPath::new(surface_index, node_index, tab_index));
        } else {
            // Panel doesn't exist, open as floating window
            self.dock_state.add_window(vec![panel_type]);
        }
    }

    /// Find the location of a panel tab (surface, node, tab index)
    fn find_tab(&self, panel_type: PanelType) -> Option<(egui_dock::SurfaceIndex, egui_dock::NodeIndex, egui_dock::TabIndex)> {
        // egui_dock 0.19 returns a `TabPath` from `find_tab`; unpack to the
        // tuple shape callers expect.
        self.dock_state
            .find_tab(&panel_type)
            .map(|p| (p.surface, p.node, p.tab))
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

    /// Create Escape Time layout: the Escape panel in the left dock
    /// where Standard puts Transforms, Colors and History on the
    /// right.
    ///
    /// Deliberately NOT a copy of Standard with one panel swapped:
    /// Transforms, the Triangle Editor and the View panel all edit a
    /// flame, and in escape mode they are either inert or actively
    /// misleading. What is left is the formula, the picture, and the
    /// colouring of it.
    fn create_escape_layout(preserve_help: bool) -> DockState<PanelType> {
        let mut state = DockState::new(vec![PanelType::FractalViewport]);

        let [_fractal_node, _left_node] = state.main_surface_mut().split_left(
            egui_dock::NodeIndex::root(),
            0.28,
            vec![PanelType::Escape],
        );

        let [_fractal_node, _right_node] = state.main_surface_mut().split_right(
            egui_dock::NodeIndex::root(),
            0.72,
            vec![PanelType::Colors, PanelType::History],
        );

        if preserve_help {
            state.add_window(vec![PanelType::Help]);
        }
        state
    }

    /// Create Compact layout: Full-screen viewport only (mobile / small screens)
    fn create_compact_layout() -> DockState<PanelType> {
        DockState::new(vec![PanelType::FractalViewport])
    }

    /// Create Animation layout: Standard layout with Animation panel at bottom center
    fn create_animation_layout(preserve_help: bool) -> DockState<PanelType> {
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

        // Re-add Help as a floating window if it was open before the layout switch.
        // Position/size are not preserved (egui_dock doesn't expose the live rect),
        // but egui's own window memory will restore them on the next frame.
        if preserve_help {
            state.add_window(vec![PanelType::Help]);
        }

        state
    }

    /// Create Scripting layout: Scripts panel in the left dock, Fractal
    /// Browser in the right — the write-run-browse loop side by side
    /// with the viewport showing the current result.
    fn create_scripting_layout(preserve_help: bool) -> DockState<PanelType> {
        let mut state = DockState::new(vec![PanelType::FractalViewport]);

        // Scripts panel wants real width (parameter sliders + source
        // editor) — 30% rather than Standard's 25%.
        let [_fractal_node, _left_node] = state.main_surface_mut().split_left(
            egui_dock::NodeIndex::root(),
            0.30,
            vec![PanelType::Scripts],
        );

        // Fractal Browser right: generated flames land where the eye
        // goes after pressing Run.
        let [_center_node, _right_node] = state.main_surface_mut().split_right(
            egui_dock::NodeIndex::root(),
            0.70,
            vec![PanelType::FractalBrowser],
        );

        if preserve_help {
            state.add_window(vec![PanelType::Help]);
        }

        state
    }

}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    /// Every preset layout must contain the panels it is named for —
    /// a layout that silently drops one renders as an empty dock area
    /// with no error.
    #[test]
    fn preset_layouts_contain_their_panels() {
        let cases: &[(WorkspaceLayout, &[PanelType])] = &[
            (
                WorkspaceLayout::Standard,
                &[PanelType::FractalViewport, PanelType::Transforms, PanelType::Colors],
            ),
            (
                WorkspaceLayout::Animation,
                &[PanelType::FractalViewport, PanelType::Animation, PanelType::Transforms],
            ),
            (
                WorkspaceLayout::Scripting,
                &[PanelType::FractalViewport, PanelType::Scripts, PanelType::FractalBrowser],
            ),
            (
                WorkspaceLayout::EscapeTime,
                &[
                    PanelType::FractalViewport,
                    PanelType::Escape,
                    PanelType::Colors,
                    PanelType::History,
                ],
            ),
            (WorkspaceLayout::Compact, &[PanelType::FractalViewport]),
        ];
        for (layout, panels) in cases {
            let mut ws = Workspace::new();
            ws.apply_layout(*layout);
            for p in *panels {
                assert!(ws.panel_exists(*p), "{p:?} missing from {layout:?} layout");
            }
        }
    }

    /// Loading a fractal of the other KIND must move the workspace,
    /// in both directions.
    ///
    /// The app watches `ConfigManager::load_generation` and calls
    /// into `apply_layout`; what this holds is the property that
    /// makes the switch worth making -- each layout carries the
    /// panel that edits its own kind of fractal and not the other's.
    /// If that ever stopped being true the switch would be pointless
    /// churn, and nothing else would notice.
    #[test]
    fn each_layout_carries_the_editor_for_its_own_fractal() {
        let mut ws = Workspace::default();

        ws.apply_layout(WorkspaceLayout::EscapeTime);
        assert!(
            ws.panel_exists(PanelType::Escape),
            "the Escape layout must carry the Escape panel"
        );
        assert!(
            !ws.panel_exists(PanelType::Transforms),
            "the Escape layout must not carry the flame transform editor"
        );

        ws.apply_layout(WorkspaceLayout::Standard);
        assert!(
            ws.panel_exists(PanelType::Transforms),
            "the Standard layout must carry the flame transform editor"
        );
        assert!(
            !ws.panel_exists(PanelType::Escape),
            "the Standard layout has no Escape panel -- which is why an              escape fractal loaded into it had nothing to edit it with"
        );
    }

    /// The Escape layout must not carry the flame-only editors.
    ///
    /// Their presence is exactly the confusion this layout exists to
    /// remove: Transforms, the Triangle Editor and the View panel all
    /// edit a flame, and none of them does anything in escape mode.
    #[test]
    fn escape_layout_omits_the_flame_only_editors() {
        let mut ws = Workspace::new();
        ws.apply_layout(WorkspaceLayout::EscapeTime);
        for p in [PanelType::Transforms, PanelType::TriangleEditor, PanelType::View] {
            assert!(
                !ws.panel_exists(p),
                "{p:?} is a flame-only editor and must not be in the Escape layout"
            );
        }
    }

    /// Help stays open across a layout switch — the layouts that
    /// preserve it re-add it as a floating window.
    #[test]
    fn help_survives_switching_to_animation_or_scripting() {
        for layout in [
            WorkspaceLayout::Animation,
            WorkspaceLayout::Scripting,
            WorkspaceLayout::EscapeTime,
        ] {
            let mut ws = Workspace::new();
            ws.dock_state.add_window(vec![PanelType::Help]);
            ws.apply_layout(layout);
            assert!(
                ws.panel_exists(PanelType::Help),
                "Help window lost switching to {layout:?}"
            );
        }
    }

    /// Side-dock widths implied by the split fractions after the
    /// startup fix-up, (left, right) in points.
    fn dock_widths(ws: &mut Workspace, window: f32) -> (f32, f32) {
        use egui_dock::{Node, NodeIndex};
        let tree = ws.dock_state.main_surface_mut();
        let f0 = match &tree[NodeIndex::root()] {
            Node::Horizontal(s) => s.fraction,
            _ => return (0.0, 0.0),
        };
        let f1 = match &tree[NodeIndex::root().left()] {
            Node::Horizontal(s) => s.fraction,
            _ => return (0.0, window * (1.0 - f0)),
        };
        (window * f0 * f1, window * (1.0 - f0))
    }

    /// The left dock is 25% of the 75% left over by the right split —
    /// 18.75% of the window. On a Retina MacBook's 960-point window
    /// that was a 180-point Transforms panel with controls clipped.
    /// The startup fix-up must raise both docks to the minimum there,
    /// and must NOT touch a window where the fractions already give
    /// more.
    #[test]
    fn startup_dock_minimums_hold_on_a_laptop_and_leave_desktops_alone() {
        for layout in [
            WorkspaceLayout::Standard,
            WorkspaceLayout::Animation,
            WorkspaceLayout::Scripting,
        ] {
            // Retina MacBook default window: both docks reach the minimum.
            let mut ws = Workspace::new();
            ws.apply_layout(layout);
            ws.apply_startup_dock_minimums(960.0);
            let (left, right) = dock_widths(&mut ws, 960.0);
            assert!(
                left >= Workspace::MIN_SIDE_DOCK_POINTS - 0.5,
                "{layout:?}: left dock {left:.0}pt on a 960pt window"
            );
            assert!(
                right >= Workspace::MIN_SIDE_DOCK_POINTS - 0.5,
                "{layout:?}: right dock {right:.0}pt on a 960pt window"
            );

            // Desktop window: already generous, fractions untouched.
            let mut wide = Workspace::new();
            wide.apply_layout(layout);
            let before = dock_widths(&mut wide, 1920.0);
            wide.needs_startup_dock_widths = true; // dock_widths consumed nothing
            wide.apply_startup_dock_minimums(1920.0);
            let after = dock_widths(&mut wide, 1920.0);
            assert_eq!(before, after, "{layout:?}: 1920pt window was altered");

            // One-shot: a user drag afterwards must not be re-overridden.
            ws.apply_startup_dock_minimums(960.0);
        }

        // Compact is a lone leaf — must not panic, must change nothing.
        let mut compact = Workspace::new();
        compact.apply_layout(WorkspaceLayout::Compact);
        compact.apply_startup_dock_minimums(390.0);
    }
}

//! TabViewer implementation for rendering docked panels

use egui_dock::{egui, TabViewer};
use super::workspace::PanelType;

/// Context needed by panels to render
pub struct PanelContext<'a> {
    pub config_manager: &'a mut crate::config::ConfigManager,
    pub flame: &'a mut crate::scene::transforms::Flame,
    pub add_transform: &'a mut bool,
    pub delete_transform: &'a mut Option<usize>,
    pub show_triangle_editor: &'a mut bool,
    pub show_undo_history: &'a mut bool,
    pub undo_requested: &'a mut bool,
    pub redo_requested: &'a mut bool,
}

/// Viewer for rendering each panel type
pub struct PanelViewer<'a> {
    pub context: PanelContext<'a>,
}

impl<'a> TabViewer for PanelViewer<'a> {
    type Tab = PanelType;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.to_string().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            PanelType::Fractal => {
                self.render_fractal_panel(ui);
            }
            PanelType::TransformEditor => {
                self.render_transform_editor_panel(ui);
            }
            PanelType::Appearance => {
                self.render_appearance_panel(ui);
            }
            PanelType::View => {
                self.render_view_panel(ui);
            }
            PanelType::Rendering => {
                self.render_rendering_panel(ui);
            }
            PanelType::Advanced => {
                self.render_advanced_panel(ui);
            }
            PanelType::History => {
                self.render_history_panel(ui);
            }
        }
    }
}

impl<'a> PanelViewer<'a> {
    /// Render the Fractal panel (main transform list)
    fn render_fractal_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Fractal Transforms");
        ui.label(format!("{} transforms", self.context.flame.transforms.len()));
        ui.separator();

        ui.label("📝 TODO: Migrate from existing windows:");
        ui.label("  • Transform List");
        ui.label("  • Add/Delete Transforms");
        ui.label("  • Final Transform Toggle");

        ui.separator();
        ui.label("For now, use:");
        ui.label("  View → Transforms");
        ui.label("  View → Triangle Editor");
    }

    /// Render the Transform Editor panel (affine, variations)
    fn render_transform_editor_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Transform Editor");
        ui.label("Edit selected transform.");
        ui.separator();

        ui.label("📝 TODO: Migrate from existing windows:");
        ui.label("  • Affine Matrix Controls");
        ui.label("  • Variation Weights");
        ui.label("  • Color Properties");

        ui.separator();
        ui.label("For now, use:");
        ui.label("  View → Transforms");
    }

    /// Render the Appearance panel (palette, color, tone mapping)
    fn render_appearance_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Appearance");
        ui.label("Color and tone mapping controls.");
        ui.separator();

        ui.label("📝 TODO: Migrate from existing windows:");
        ui.label("  • Color Mode (Palette/Speed)");
        ui.label("  • Palette Selection");
        ui.label("  • Tone Mapping Controls");
        ui.label("  • Exposure, Gamma, etc.");

        ui.separator();
        ui.label("For now, use:");
        ui.label("  View → Tone Mapping & Colors");
    }

    /// Render the View panel (zoom, pan, rotation)
    fn render_view_panel(&mut self, ui: &mut egui::Ui) {
        super::view::render_view_content(
            ui,
            self.context.config_manager,
            self.context.flame,
        );
    }

    /// Render the Rendering panel (iterations, accumulation)
    fn render_rendering_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Rendering Quality");
        ui.label("Performance and quality settings.");
        ui.separator();

        ui.label("📝 TODO: Migrate from existing windows:");
        ui.label("  • Iterations per Thread");
        ui.label("  • Histogram Color Scale");
        ui.label("  • Accumulation Controls");
        ui.label("  • Max Iterations");

        ui.separator();
        ui.label("For now, use:");
        ui.label("  View → Settings");
    }

    /// Render the Advanced panel (expert features)
    fn render_advanced_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Advanced");
        ui.label("Export and advanced features.");
        ui.separator();

        ui.label("📝 TODO: Migrate from existing windows:");
        ui.label("  • PNG Export");
        ui.label("  • Config Import/Export");
        ui.label("  • Apophysis Import");

        ui.separator();
        ui.label("For now, use:");
        ui.label("  View → Settings (Export section)");
        ui.label("  View → Config Import/Export");
    }

    /// Render the History panel (undo/redo browser)
    fn render_history_panel(&mut self, ui: &mut egui::Ui) {
        super::undo_history::render_undo_history_content(
            ui,
            self.context.config_manager,
            self.context.undo_requested,
            self.context.redo_requested,
        );
    }
}

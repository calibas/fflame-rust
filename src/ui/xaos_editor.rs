//! Xaos Editor Panel
//!
//! Provides a grid-based UI for editing xaos (chaos-weighted transform transitions).
//! Features:
//! - N×N grid showing transition weights between transforms
//! - "View To" / "View From" toggle (like Apophysis)
//! - Direct editing of weight values
//! - Reset all to 1.0 button

use egui::Color32;
use rust_i18n::t;

use crate::config::{ConfigManager, ConfigPath, UpdateType};
use crate::scene::transforms::Flame;

/// State for the xaos editor panel
#[derive(Default)]
pub struct XaosEditorState {
    /// View mode: true = "View To" (columns show where FROM can go TO)
    /// false = "View From" (rows show where TO came FROM)
    pub view_to_mode: bool,
    /// Whether the "Link Transforms" dialog is open
    pub link_dialog_open: bool,
    /// Selected pre-transform index for linking (1-based in UI, 0-based here)
    pub link_pre: usize,
    /// Selected post-transform index for linking (1-based in UI, 0-based here)
    pub link_post: usize,
}

/// Get transform color for visual identification (matches transforms.rs)
fn get_transform_color(index: usize) -> Color32 {
    let colors = [
        Color32::from_rgb(255, 100, 100), // Red
        Color32::from_rgb(100, 255, 100), // Green
        Color32::from_rgb(100, 100, 255), // Blue
        Color32::from_rgb(255, 255, 100), // Yellow
        Color32::from_rgb(255, 100, 255), // Magenta
        Color32::from_rgb(100, 255, 255), // Cyan
        Color32::from_rgb(255, 150, 100), // Orange
        Color32::from_rgb(150, 100, 255), // Purple
    ];
    colors[index % colors.len()]
}

/// Render the xaos editor panel content
pub fn render_xaos_editor_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    flame: &Flame,
    state: &mut XaosEditorState,
) -> UpdateType {
    let mut max_update = UpdateType::None;
    let num_transforms = flame.transforms.len();

    // Header
    ui.horizontal(|ui| {
        ui.heading(t!("xaos_editor.title"));
        ui.add_space(10.0);

        // View mode toggle
        ui.label(t!("xaos_editor.view_mode"));
        if ui.selectable_label(state.view_to_mode, t!("xaos_editor.view_to"))
            .on_hover_text(t!("xaos_editor.view_to_tooltip"))
            .clicked()
        {
            state.view_to_mode = true;
        }
        if ui.selectable_label(!state.view_to_mode, t!("xaos_editor.view_from"))
            .on_hover_text(t!("xaos_editor.view_from_tooltip"))
            .clicked()
        {
            state.view_to_mode = false;
        }
    });

    ui.add_space(4.0);

    // Action buttons
    ui.horizontal(|ui| {
        // Reset all to 1.0 button (effectively disables xaos)
        if ui.button(t!("xaos_editor.reset_all"))
            .on_hover_text(t!("xaos_editor.reset_all_tooltip"))
            .clicked()
        {
            // Batch update all xaos weights to 1.0
            let mut changes = Vec::new();
            for src in 0..num_transforms {
                for dst in 0..num_transforms {
                    changes.push((
                        ConfigPath::Xaos { src, dst },
                        1.0f32.into(),
                    ));
                }
            }
            if !changes.is_empty() {
                if let Ok(update) = config_manager.update_batch(
                    changes,
                    "xaos_editor.reset_all".to_string(),
                ) {
                    max_update = max_update.max(update);
                }
            }
        }

        // Set all to 0.0 (isolate all) button
        if ui.button(t!("xaos_editor.isolate_all"))
            .on_hover_text(t!("xaos_editor.isolate_all_tooltip"))
            .clicked()
        {
            let mut changes = Vec::new();
            for src in 0..num_transforms {
                for dst in 0..num_transforms {
                    // Set diagonal to 1.0 (self-transition), others to 0.0
                    let value = if src == dst { 1.0 } else { 0.0 };
                    changes.push((
                        ConfigPath::Xaos { src, dst },
                        value.into(),
                    ));
                }
            }
            if !changes.is_empty() {
                if let Ok(update) = config_manager.update_batch(
                    changes,
                    "xaos_editor.isolate_all".to_string(),
                ) {
                    max_update = max_update.max(update);
                }
            }
        }
    });

    ui.add_space(8.0);

    // Don't show grid if there are no transforms
    if num_transforms == 0 {
        ui.label(t!("xaos_editor.no_transforms"));
        return max_update;
    }

    // Determine axis labels based on view mode
    let (col_axis_label, row_axis_label) = if state.view_to_mode {
        (t!("xaos_editor.to_label"), t!("xaos_editor.from_label"))
    } else {
        (t!("xaos_editor.from_label"), t!("xaos_editor.to_label"))
    };

    // Scrollable area for large grids
    egui::ScrollArea::both()
        .max_width(ui.available_width())
        .max_height(ui.available_height())
        .show(ui, |ui| {
            // Use Grid for layout
            egui::Grid::new("xaos_grid")
                .spacing([2.0, 2.0])
                .min_col_width(36.0)
                .show(ui, |ui| {
                    // First row: corner cell with both axis labels + column headers
                    // Corner cell: axis labels in opposite corners
                    ui.allocate_ui(egui::vec2(44.0, 32.0), |ui| {
                        let rect = ui.available_rect_before_wrap();
                        // Column axis label (To) - top right corner
                        ui.painter().text(
                            egui::pos2(rect.right() - 2.0, rect.top() + 2.0),
                            egui::Align2::RIGHT_TOP,
                            col_axis_label.as_ref(),
                            egui::FontId::proportional(11.0),
                            ui.visuals().weak_text_color(),
                        );
                        // Row axis label (From) - bottom left corner
                        ui.painter().text(
                            egui::pos2(rect.left() + 2.0, rect.bottom() - 2.0),
                            egui::Align2::LEFT_BOTTOM,
                            row_axis_label.as_ref(),
                            egui::FontId::proportional(11.0),
                            ui.visuals().weak_text_color(),
                        );
                    });

                    // Column headers (transform numbers with colors)
                    for col in 0..num_transforms {
                        let color = get_transform_color(col);
                        ui.vertical_centered(|ui| {
                            // Color indicator bar
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(32.0, 4.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().rect_filled(rect, 2.0, color);
                            // Transform number
                            ui.label(format!("{}", col + 1));
                        });
                    }
                    ui.end_row();

                    // Data rows
                    for row in 0..num_transforms {
                        // Row header with color indicator
                        let row_color = get_transform_color(row);
                        ui.horizontal(|ui| {
                            // Color indicator
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(4.0, 20.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().rect_filled(rect, 2.0, row_color);
                            ui.label(format!("{}", row + 1));
                        });

                        // Data cells
                        for col in 0..num_transforms {
                            // In "View To" mode: row = src, col = dst
                            // In "View From" mode: row = dst, col = src
                            let (src, dst) = if state.view_to_mode {
                                (row, col)
                            } else {
                                (col, row)
                            };

                            let mut weight = flame.get_xaos(src, dst);
                            let is_diagonal = src == dst;

                            // Background color based on weight
                            let bg_color = weight_to_color(weight);

                            // Create a frame with the background color
                            egui::Frame::new()
                                .fill(bg_color)
                                .corner_radius(2.0)
                                .show(ui, |ui| {
                                    ui.set_min_size(egui::vec2(36.0, 24.0));

                                    // DragValue for editing
                                    let response = ui.add(
                                        egui::DragValue::new(&mut weight)
                                            .range(0.0..=10.0)
                                            .speed(0.01)
                                            .fixed_decimals(2)
                                            .custom_formatter(|v, _| {
                                                if (v - 0.0).abs() < 0.001 {
                                                    "0".to_string()
                                                } else if (v - 1.0).abs() < 0.001 {
                                                    "1".to_string()
                                                } else {
                                                    format!("{:.2}", v)
                                                }
                                            })
                                    );
                                    super::vkb_sync_opts(ui, &response, &format!("{}", weight), "decimal");

                                    // Show tooltip with transition info
                                    let tooltip = if state.view_to_mode {
                                        format!("T{} > T{}", src + 1, dst + 1)
                                    } else {
                                        format!("T{} < T{}", dst + 1, src + 1)
                                    };

                                    // Highlight diagonal cells
                                    if is_diagonal {
                                        ui.painter().rect_stroke(
                                            response.rect,
                                            2.0,
                                            egui::Stroke::new(1.5, Color32::from_rgb(200, 200, 200)),
                                            egui::StrokeKind::Outside,
                                        );
                                    }

                                    let changed = response.changed();
                                    response.on_hover_text(tooltip);

                                    if changed {
                                        if let Ok(update) = config_manager.update_param(
                                            ConfigPath::Xaos { src, dst },
                                            weight.into(),
                                        ) {
                                            max_update = max_update.max(update);
                                        }
                                    }
                                });
                        }
                        ui.end_row();
                    }
                });
        });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // === LINK TRANSFORMS ===
    if ui.button(t!("xaos_editor.link_action"))
        .on_hover_text(t!("xaos_editor.link_action_tooltip"))
        .clicked()
    {
        state.link_dialog_open = !state.link_dialog_open;
        // Initialize with first two different transforms
        state.link_pre = 0;
        state.link_post = if num_transforms > 1 { 1 } else { 0 };
    }

    ui.add_space(4.0);

    // Link dialog
    if state.link_dialog_open && num_transforms >= 2 {
        egui::Frame::new()
            .fill(ui.visuals().extreme_bg_color)
            .stroke(egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
            .inner_margin(8.0)
            .corner_radius(4.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(t!("xaos_editor.select_pre"));
                    egui::ComboBox::from_id_salt("link_pre_combo")
                        .selected_text(format!("T{}", state.link_pre + 1))
                        .show_ui(ui, |ui| {
                            for i in 0..num_transforms {
                                let color = get_transform_color(i);
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(8.0, 14.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(rect, 2.0, color);
                                    ui.selectable_value(&mut state.link_pre, i, format!("T{}", i + 1));
                                });
                            }
                        });

                    ui.add_space(8.0);

                    ui.label(t!("xaos_editor.select_post"));
                    egui::ComboBox::from_id_salt("link_post_combo")
                        .selected_text(format!("T{}", state.link_post + 1))
                        .show_ui(ui, |ui| {
                            for i in 0..num_transforms {
                                let color = get_transform_color(i);
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(8.0, 14.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(rect, 2.0, color);
                                    ui.selectable_value(&mut state.link_post, i, format!("T{}", i + 1));
                                });
                            }
                        });
                });

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    let can_link = state.link_pre != state.link_post;

                    if ui.add_enabled(can_link, egui::Button::new(t!("xaos_editor.link_confirm"))).clicked() {
                        let changes = flame.link_transforms_changes(state.link_pre, state.link_post);
                        if !changes.is_empty() {
                            if let Ok(update) = config_manager.update_batch(
                                changes,
                                format!("Link T{} -> T{}", state.link_pre + 1, state.link_post + 1),
                            ) {
                                max_update = max_update.max(update);
                            }
                        }
                        state.link_dialog_open = false;
                    }

                    if ui.button(t!("xaos_editor.link_cancel")).clicked() {
                        state.link_dialog_open = false;
                    }

                    if !can_link {
                        ui.colored_label(Color32::from_rgb(255, 150, 100), t!("xaos_editor.link_error_same"));
                    }
                });
            });

        ui.add_space(4.0);
    }

    max_update
}

/// Convert xaos weight to a background color
/// 0.0 = dark red, 1.0 = green, >1.0 = blue tint
fn weight_to_color(weight: f32) -> Color32 {
    if weight <= 0.0 {
        // Zero = dark red (blocked)
        Color32::from_rgb(100, 30, 30)
    } else if weight < 1.0 {
        // 0 to 1 = red to yellow gradient
        let t = weight;
        let r = 100 + (155.0 * (1.0 - t * 0.5)) as u8;
        let g = (30.0 + 170.0 * t) as u8;
        let b = 30;
        Color32::from_rgb(r, g, b)
    } else if weight <= 1.001 {
        // 1.0 = green (normal)
        Color32::from_rgb(60, 150, 60)
    } else {
        // >1.0 = blue-green (boosted)
        let t = ((weight - 1.0) / 9.0).min(1.0); // Clamp at 10x
        let r = (60.0 * (1.0 - t)) as u8;
        let g = (150.0 - 50.0 * t) as u8;
        let b = (60 + (140.0 * t) as u8).min(200);
        Color32::from_rgb(r, g, b)
    }
}

//! Xaos Editor Panel
//!
//! Provides a grid-based UI for editing xaos (chaos-weighted transform transitions).
//! Features:
//! - N×N grid showing transition weights between transforms
//! - "View To" / "View From" toggle (like Apophysis)
//! - Double-click to toggle between 0 and 1
//! - Drag to adjust values
//! - Reset all to 1.0 button

use egui::{Color32, Sense, Stroke};
use rust_i18n::t;

use crate::config::{ConfigManager, ConfigPath, UpdateType};
use crate::scene::transforms::Flame;

/// State for the xaos editor panel
#[derive(Default)]
pub struct XaosEditorState {
    /// View mode: true = "View To" (columns show where FROM can go TO)
    /// false = "View From" (rows show where TO came FROM)
    pub view_to_mode: bool,
    /// Currently dragging cell (src, dst)
    pub dragging_cell: Option<(usize, usize)>,
    /// Last mouse Y position for drag calculation
    pub last_drag_y: f32,
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

    // Help text
    ui.label(egui::RichText::new(t!("xaos_editor.help"))
        .small()
        .color(ui.visuals().weak_text_color()));

    ui.add_space(8.0);

    // Action buttons
    ui.horizontal(|ui| {
        // Enable xaos button (if not already enabled)
        if !flame.has_xaos() {
            if ui.button(t!("xaos_editor.enable")).clicked() {
                // Set one weight to trigger xaos initialization
                if let Ok(update) = config_manager.update_param(
                    ConfigPath::Xaos { src: 0, dst: 0 },
                    1.0f32.into(),
                ) {
                    max_update = max_update.max(update);
                }
            }
        }

        // Reset all to 1.0 button
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

    // Grid layout
    let cell_size = 32.0;
    let header_size = 24.0;
    let total_width = header_size + (num_transforms as f32 * cell_size);
    let total_height = header_size + (num_transforms as f32 * cell_size);

    // Scrollable area for large grids
    egui::ScrollArea::both()
        .max_width(ui.available_width())
        .max_height(ui.available_height())
        .show(ui, |ui| {
            let (response, painter) = ui.allocate_painter(
                egui::vec2(total_width, total_height),
                Sense::click_and_drag(),
            );
            let rect = response.rect;

            // Background
            painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);

            // Column headers (destination transforms in "View To" mode)
            let col_label = if state.view_to_mode {
                t!("xaos_editor.to_label")
            } else {
                t!("xaos_editor.from_label")
            };

            // Corner label
            painter.text(
                rect.min + egui::vec2(header_size / 2.0, header_size / 2.0),
                egui::Align2::CENTER_CENTER,
                &col_label,
                egui::FontId::proportional(10.0),
                ui.visuals().weak_text_color(),
            );

            for i in 0..num_transforms {
                let x = rect.min.x + header_size + (i as f32 * cell_size) + cell_size / 2.0;
                let y = rect.min.y + header_size / 2.0;

                // Draw colored indicator
                let color = get_transform_color(i);
                let indicator_rect = egui::Rect::from_center_size(
                    egui::pos2(x, y - 4.0),
                    egui::vec2(cell_size - 4.0, 4.0),
                );
                painter.rect_filled(indicator_rect, 2.0, color);

                // Draw transform number
                painter.text(
                    egui::pos2(x, y + 4.0),
                    egui::Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    egui::FontId::proportional(11.0),
                    ui.visuals().text_color(),
                );
            }

            // Row headers (source transforms in "View To" mode)
            let row_label = if state.view_to_mode {
                t!("xaos_editor.from_label")
            } else {
                t!("xaos_editor.to_label")
            };

            for i in 0..num_transforms {
                let x = rect.min.x + header_size / 2.0;
                let y = rect.min.y + header_size + (i as f32 * cell_size) + cell_size / 2.0;

                // Draw colored indicator
                let color = get_transform_color(i);
                let indicator_rect = egui::Rect::from_center_size(
                    egui::pos2(x - 4.0, y),
                    egui::vec2(4.0, cell_size - 4.0),
                );
                painter.rect_filled(indicator_rect, 2.0, color);

                // Draw transform number
                painter.text(
                    egui::pos2(x + 4.0, y),
                    egui::Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    egui::FontId::proportional(11.0),
                    ui.visuals().text_color(),
                );
            }

            // Draw grid cells
            for row in 0..num_transforms {
                for col in 0..num_transforms {
                    // In "View To" mode: row = src, col = dst
                    // In "View From" mode: row = dst, col = src
                    let (src, dst) = if state.view_to_mode {
                        (row, col)
                    } else {
                        (col, row)
                    };

                    let cell_x = rect.min.x + header_size + (col as f32 * cell_size);
                    let cell_y = rect.min.y + header_size + (row as f32 * cell_size);
                    let cell_rect = egui::Rect::from_min_size(
                        egui::pos2(cell_x, cell_y),
                        egui::vec2(cell_size, cell_size),
                    );

                    let weight = flame.get_xaos(src, dst);

                    // Cell background based on weight
                    let bg_color = weight_to_color(weight);
                    painter.rect_filled(cell_rect.shrink(1.0), 2.0, bg_color);

                    // Cell border
                    let border_color = if src == dst {
                        // Diagonal cells (self-transition) have highlighted border
                        Color32::from_rgb(150, 150, 150)
                    } else {
                        ui.visuals().widgets.noninteractive.bg_stroke.color
                    };
                    painter.rect_stroke(
                        cell_rect.shrink(1.0),
                        2.0,
                        Stroke::new(1.0, border_color),
                        egui::StrokeKind::Inside,
                    );

                    // Weight text
                    let text_color = if weight > 0.5 {
                        Color32::BLACK
                    } else {
                        Color32::WHITE
                    };

                    // Show "0" or "1" for exact values, otherwise show 1 decimal
                    let text = if (weight - 0.0).abs() < 0.001 {
                        "0".to_string()
                    } else if (weight - 1.0).abs() < 0.001 {
                        "1".to_string()
                    } else {
                        format!("{:.1}", weight)
                    };

                    painter.text(
                        cell_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        text,
                        egui::FontId::proportional(11.0),
                        text_color,
                    );
                }
            }

            // Handle interactions
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                // Calculate which cell is under the pointer
                let rel_x = pointer_pos.x - rect.min.x - header_size;
                let rel_y = pointer_pos.y - rect.min.y - header_size;

                if rel_x >= 0.0 && rel_y >= 0.0 {
                    let col = (rel_x / cell_size) as usize;
                    let row = (rel_y / cell_size) as usize;

                    if col < num_transforms && row < num_transforms {
                        let (src, dst) = if state.view_to_mode {
                            (row, col)
                        } else {
                            (col, row)
                        };

                        // Double-click to toggle between 0 and 1
                        if response.double_clicked() {
                            let current = flame.get_xaos(src, dst);
                            let new_value = if current < 0.5 { 1.0 } else { 0.0 };
                            if let Ok(update) = config_manager.update_param(
                                ConfigPath::Xaos { src, dst },
                                new_value.into(),
                            ) {
                                max_update = max_update.max(update);
                            }
                        }
                        // Drag to adjust value
                        else if response.dragged() {
                            let delta = response.drag_delta();
                            // Vertical drag: up = increase, down = decrease
                            let current = flame.get_xaos(src, dst);
                            let new_value = (current - delta.y * 0.01).clamp(0.0, 10.0);
                            if let Ok(update) = config_manager.update_param(
                                ConfigPath::Xaos { src, dst },
                                new_value.into(),
                            ) {
                                max_update = max_update.max(update);
                            }
                        }
                    }
                }
            }

            // Commit on drag release
            if response.drag_stopped() {
                // Force commit the last change
                // The ConfigManager will handle coalescing
            }
        });

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

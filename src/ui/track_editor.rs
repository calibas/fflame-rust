//! Animation track editor UI
//!
//! Provides controls for adding, editing, and deleting animation tracks.
//!
//! Phase 2 adds visual timeline bars for tracks aligned with the scrubber.
//! Phase 3 adds interactions: click to seek, keyframe hover/click.

use egui::{Ui, Color32, Rect, Pos2, Stroke, CornerRadius, Sense, ScrollArea};
use rust_i18n::t;
use crate::animation::{
    Animation, AnimationController, EasingFunction, Interpolation,
    Keyframe, Track, TrackSource,
};
use crate::config::{ConfigPath, EditingTarget, FractalConfig};
use super::animation_panel::TimelineLayout;
use super::target_selector::{TargetSelectorState, render_target_selector};

/// UI state for track editor
#[derive(Default)]
pub struct TrackEditorState {
    /// Selected track type for new track
    pub new_track_type: NewTrackType,
    /// Selected target parameter path for new track
    pub new_track_target: String,
    /// Selected flame (Main or Subflame{i}) for the new track
    pub new_track_flame_target: EditingTarget,
    /// Unified Track Editor panel state
    pub track_editor_panel_open: bool,
    /// Track index being edited in the unified panel (None = adding new track)
    pub editing_track_index: Option<usize>,
    /// Target selector state for the unified panel
    pub target_selector_state: TargetSelectorState,
    /// Signal track parameters for editing
    pub signal_params: SignalParams,
    /// Preview keyframes for Add Track mode (before track is created)
    pub preview_keyframes: Vec<Keyframe>,
    /// Interpolation mode for preview keyframes
    pub preview_interpolation: Interpolation,
}

/// Signal track parameters for track editor
#[derive(Clone)]
pub struct SignalParams {
    pub signal_name: String,
    pub min_output: f64,
    pub max_output: f64,
    pub smoothing: f64,
    pub start_time: f64,
    pub end_time: f64,
    pub fade_in: f64,
    pub fade_in_easing: EasingFunction,
    pub fade_out: f64,
    pub fade_out_easing: EasingFunction,
}

impl Default for SignalParams {
    fn default() -> Self {
        Self {
            signal_name: String::new(),
            min_output: 0.0,
            max_output: 1.0,
            smoothing: 0.0,
            start_time: 0.0,
            end_time: 0.0,
            fade_in: 0.0,
            fade_in_easing: EasingFunction::Linear,
            fade_out: 0.0,
            fade_out_easing: EasingFunction::Linear,
        }
    }
}

/// Response from track editor rendering (Phase 3)
#[derive(Default)]
pub struct TrackEditorResponse {
    /// User clicked on timeline/track area - seek to this time
    pub seek_to_time: Option<f64>,
}

/// Type of track to add
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum NewTrackType {
    #[default]
    Keyframe,
    Signal,
}

/// Convert internal 0-based path string to 1-based display string
///
/// Internal paths use 0-based indices for backward compatibility with saved files,
/// but UI displays 1-based indices to match the rest of the application.
fn path_to_display(path: &str) -> String {
    // Match "Transform.N." pattern and increment the number
    if let Some(rest) = path.strip_prefix("Transform.") {
        if let Some(dot_pos) = rest.find('.') {
            if let Ok(index) = rest[..dot_pos].parse::<usize>() {
                return format!("Transform.{}.{}", index + 1, &rest[dot_pos + 1..]);
            }
        }
    }
    // Return unchanged for non-transform paths
    path.to_string()
}

/// Human-readable prefix for a track's flame target — `"Main"` or
/// `"Subflame 0"`. Subflame indices are displayed 0-based to match
/// the Subflames panel.
fn flame_target_prefix(flame_target: EditingTarget) -> String {
    match flame_target {
        EditingTarget::Main => "Main".to_string(),
        EditingTarget::Subflame { index } => format!("Subflame {}", index),
    }
}

/// Compose the full display string for a track row:
/// `"{flame} / {path}"` where `{path}` is the 1-based-display path.
fn target_display(flame_target: EditingTarget, path: &str) -> String {
    format!("{} / {}", flame_target_prefix(flame_target), path_to_display(path))
}

/// Resolve the flame referenced by `flame_target` inside `config`.
/// Returns `None` when the target's subflame index is out of range —
/// i.e. the track is broken because the subflame was deleted.
fn resolve_flame<'a>(config: &'a FractalConfig, flame_target: EditingTarget) -> Option<&'a crate::scene::transforms::Flame> {
    match flame_target {
        EditingTarget::Main => Some(&config.flame),
        EditingTarget::Subflame { index } => config.flame.subflames.get(index),
    }
}

/// Check whether a track is broken — i.e. its `bound` IDs don't
/// resolve in the current config. Tracks with no `bound` (View, Color,
/// Tone, Rendering — anything that doesn't index a tracked list) are
/// never broken.
///
/// The check is keyed off `Track.bound`, which is populated by
/// `Animation::bind_to_config` after load and kept current by the
/// `rebind_targets` hook. Compare to the pre-PR-2 implementation,
/// which parsed `target` and checked the literal index — that worked
/// but couldn't distinguish "deleted" from "shifted by another delete"
/// and would silently retarget rather than mark broken.
pub fn is_track_broken(config: &FractalConfig, track: &crate::animation::Track) -> bool {
    use crate::animation::TargetBinding;

    // Subflame ID must still resolve.
    if let Some(flame_id) = track.bound.flame {
        if !config.flame.subflames.iter().any(|s| s.id == flame_id) {
            return true;
        }
    }

    let Some(target_binding) = track.bound.target else {
        // No target binding either means the path doesn't index a
        // tracked list (View, Color, etc.) or the track was never
        // bound. Either way, nothing to be broken about.
        return false;
    };

    // Find the flame to scan based on the (already-verified-resolvable)
    // flame_target. If the *path* refers to FractalConfig-level effects,
    // the flame parameter is irrelevant.
    let flame = match track.flame_target {
        EditingTarget::Main => &config.flame,
        EditingTarget::Subflame { index } => match config.flame.subflames.get(index) {
            Some(sub) => sub,
            None => return true, // subflame gone (also caught by flame_id check above)
        },
    };

    let found = match target_binding {
        TargetBinding::Transform(id) => flame.transforms.iter().any(|t| t.id == id),
        TargetBinding::Linked(id) => flame.linked_transforms.iter().any(|t| t.id == id),
        TargetBinding::Final(id) => flame.final_transforms.iter().any(|t| t.id == id),
        TargetBinding::ColorEffect(id) => config.color_effects.iter().any(|e| e.id == id),
        TargetBinding::DensityEffect(id) => config.density_effects.iter().any(|e| e.id == id),
    };
    !found
}

/// Count how many tracks in the animation are broken given the
/// current config. Surfaced in the Animation panel header so the
/// user sees a "(N broken)" hint at a glance.
pub fn count_broken_tracks(animation: &Animation, config: &FractalConfig) -> usize {
    animation
        .tracks
        .iter()
        .filter(|t| is_track_broken(config, t))
        .count()
}

/// Render the track editor section with visual timeline bars aligned with the scrubber.
///
/// Returns a response containing any seek requests from clicking on tracks.
pub fn render_track_editor(
    ui: &mut Ui,
    controller: &mut AnimationController,
    state: &mut TrackEditorState,
    config: &FractalConfig,
    timeline_layout: Option<TimelineLayout>,
) -> TrackEditorResponse {
    let has_animation = controller.animation.is_some();
    let mut response = TrackEditorResponse::default();

    // Track list header with Add Track button.
    // If any tracks are broken, append a "(N broken)" hint so the
    // user notices at a glance — they can then open each row to
    // see the warning icon.
    let (track_count, broken_count) = controller
        .animation
        .as_ref()
        .map(|a| (a.tracks.len(), count_broken_tracks(a, config)))
        .unwrap_or((0, 0));

    ui.horizontal(|ui| {
        ui.strong(t!("track_editor.tracks_header", count = track_count));
        if broken_count > 0 {
            ui.colored_label(
                egui::Color32::from_rgb(220, 140, 60),
                format!("⚠ {} broken", broken_count),
            )
            .on_hover_text(
                "Tracks targeting parameters that don't exist in the current config \
                 (e.g. transform was deleted, subflame is missing). They still load \
                 but won't apply until rebound.",
            );
        }
        ui.separator();
        let add_button = egui::Button::new(t!("track_editor.add_track"))
            .fill(egui::Color32::from_rgb(60, 120, 60));
        if ui.add_enabled(has_animation, add_button).clicked() {
            open_add_track_panel(state);
        }
    });

    ui.separator();

    // Render tracks as visual timeline bars
    if let (Some(ref mut animation), Some(layout)) = (&mut controller.animation, timeline_layout) {
        response = render_tracks_visual(ui, animation, state, config, layout);
    }

    response
}

/// Render tracks as visual timeline bars (Phase 2+3)
///
/// Layout per track:
/// ```text
/// Label        |--●====●========●---|     [Edit] [Delete]
///              ^  ^    ^        ^   ^
///              |  |    keyframe |   |
///              |  first_kf      |   last_kf
///              timeline_start   timeline_end
/// ```
///
/// Phase 3 interactions:
/// - Click on bar area to seek to that time
/// - Hover on keyframe dot to see value tooltip
/// - Click on keyframe dot to open editor for that track
fn render_tracks_visual(
    ui: &mut Ui,
    animation: &mut Animation,
    state: &mut TrackEditorState,
    config: &FractalConfig,
    layout: TimelineLayout,
) -> TrackEditorResponse {
    let mut track_to_delete: Option<usize> = None;
    let mut response = TrackEditorResponse::default();

    // Constants for visual rendering
    const TRACK_HEIGHT: f32 = 24.0;
    const LABEL_WIDTH: f32 = 100.0;
    const BUTTON_WIDTH: f32 = 80.0;
    const KEYFRAME_RADIUS: f32 = 5.0;
    const KEYFRAME_HIT_RADIUS: f32 = 8.0; // Larger hit area for easier clicking
    const BAR_HEIGHT: f32 = 8.0;

    // Colors
    let bar_color = Color32::from_rgb(80, 120, 180);
    let bar_bg_color = Color32::from_gray(60);
    let bar_hover_color = Color32::from_gray(75); // Subtle hover effect
    let keyframe_color = Color32::from_rgb(255, 200, 100);
    let keyframe_hover_color = Color32::from_rgb(255, 255, 150);
    let position_line_color = Color32::from_rgb(255, 80, 80);

    // Calculate position line X
    let position_x = layout.position_x();

    // Track all track rects for position line drawing
    let mut all_track_rects: Vec<Rect> = Vec::new();

    // Store bar area info for later position line calculation
    let mut bar_area_info: Option<(f32, f32, f32)> = None; // (bar_left, bar_right, bar_scale)

    for (track_index, track) in animation.tracks.iter().enumerate() {
        let path = track.target.clone();
        {
            // Get time range for this track
            let (first_time, last_time) = match &track.source {
                TrackSource::Keyframes { keyframes } => {
                    if keyframes.is_empty() {
                        (0.0, layout.duration)
                    } else {
                        let first = keyframes.first().map(|k| k.time).unwrap_or(0.0);
                        let last = keyframes.last().map(|k| k.time).unwrap_or(layout.duration);
                        (first, last)
                    }
                }
                TrackSource::Signal { .. } => {
                    // Signals span full duration
                    (0.0, layout.duration)
                }
            };

            // Allocate full row with click sensing
            let (rect, row_response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), TRACK_HEIGHT),
                Sense::click(),
            );

            if ui.is_rect_visible(rect) {
                let painter = ui.painter();

                // Calculate bar area (between label and buttons)
                let bar_left = rect.left() + LABEL_WIDTH;
                let bar_right = rect.right() - BUTTON_WIDTH;
                let bar_scale = if (layout.bar_right - layout.bar_left).abs() > 0.001 {
                    (bar_right - bar_left) / (layout.bar_right - layout.bar_left)
                } else {
                    1.0
                };

                // Store bar area info for position line
                if bar_area_info.is_none() {
                    bar_area_info = Some((bar_left, bar_right, bar_scale));
                }

                // Background bar (full timeline extent) - with hover highlight
                let bg_rect = Rect::from_min_max(
                    Pos2::new(bar_left, rect.center().y - BAR_HEIGHT / 2.0),
                    Pos2::new(bar_right, rect.center().y + BAR_HEIGHT / 2.0),
                );
                let bg_color = if row_response.hovered() { bar_hover_color } else { bar_bg_color };
                painter.rect_filled(bg_rect, CornerRadius::same(2), bg_color);

                // Track bar (from first to last keyframe)
                let adjusted_bar_left = bar_left + (layout.time_to_x(first_time) - layout.bar_left) * bar_scale;
                let adjusted_bar_right = bar_left + (layout.time_to_x(last_time) - layout.bar_left) * bar_scale;

                let track_rect = Rect::from_min_max(
                    Pos2::new(adjusted_bar_left.max(bar_left).min(bar_right), rect.center().y - BAR_HEIGHT / 2.0),
                    Pos2::new(adjusted_bar_right.max(bar_left).min(bar_right), rect.center().y + BAR_HEIGHT / 2.0),
                );
                painter.rect_filled(track_rect, CornerRadius::same(2), bar_color);

                // Keyframe dots with hover/click interaction
                let mut clicked_keyframe: Option<usize> = None;
                let pointer_pos = ui.ctx().pointer_hover_pos();

                if let TrackSource::Keyframes { keyframes } = &track.source {
                    for (kf_idx, keyframe) in keyframes.iter().enumerate() {
                        let kf_x = bar_left + (layout.time_to_x(keyframe.time) - layout.bar_left) * bar_scale;
                        if kf_x >= bar_left && kf_x <= bar_right {
                            let kf_pos = Pos2::new(kf_x, rect.center().y);

                            // Check if mouse is hovering over this keyframe
                            let is_hovered = pointer_pos.map_or(false, |pos| {
                                (pos - kf_pos).length() <= KEYFRAME_HIT_RADIUS
                            });

                            // Draw keyframe dot (highlighted if hovered)
                            let color = if is_hovered { keyframe_hover_color } else { keyframe_color };
                            painter.circle_filled(kf_pos, KEYFRAME_RADIUS, color);
                            // Add subtle outline on hover
                            if is_hovered {
                                painter.circle_stroke(kf_pos, KEYFRAME_RADIUS + 1.0, Stroke::new(1.5, Color32::WHITE));
                            }

                            // Show tooltip on hover using manual tooltip window
                            if is_hovered {
                                let value_str = keyframe.value.as_f64()
                                    .map(|v| format!("{:.3}", v))
                                    .unwrap_or_else(|| format!("{}", keyframe.value));

                                // Create a tooltip area near the pointer.
                                // `Tooltip::always_open` with `PopupAnchor::Pointer`
                                // matches the legacy `show_tooltip` behavior — body
                                // shows as long as this call runs, positioned at
                                // the cursor.
                                let tooltip_id = egui::Id::new(format!("kf_tooltip_{}_{}", path, kf_idx));
                                egui::Tooltip::always_open(
                                    ui.ctx().clone(),
                                    egui::LayerId::new(egui::Order::Tooltip, tooltip_id),
                                    tooltip_id,
                                    egui::PopupAnchor::Pointer,
                                )
                                .gap(12.0)
                                .show(|ui| {
                                    ui.label(format!(
                                        "{}: {} @ {:.2}s",
                                        path, value_str, keyframe.time
                                    ));
                                });

                                // Check for click on keyframe
                                if row_response.clicked() {
                                    clicked_keyframe = Some(kf_idx);
                                }
                            }
                        }
                    }
                }

                // Handle keyframe click - open Track Editor panel
                if let Some(_kf_idx) = clicked_keyframe {
                    open_edit_track_panel(state, track_index, track);
                }
                // Handle bar area click (not on keyframe) - seek to that time
                else if row_response.clicked() {
                    if let Some(pos) = pointer_pos {
                        if pos.x >= bar_left && pos.x <= bar_right {
                            // Convert click position to time
                            let t = (pos.x - bar_left) / (bar_right - bar_left);
                            let time = t as f64 * layout.duration;
                            response.seek_to_time = Some(time);
                        }
                    }
                }

                // Show pointer cursor when hovering over the bar area
                if row_response.hovered() {
                    if let Some(pos) = pointer_pos {
                        if pos.x >= bar_left && pos.x <= bar_right {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }
                }

                // Draw label on the left with tooltip for full name
                let label_rect = Rect::from_min_max(
                    rect.left_top(),
                    Pos2::new(rect.left() + LABEL_WIDTH - 4.0, rect.bottom()),
                );
                let flame_target = track.flame_target;
                let is_broken = is_track_broken(config, track);
                // Convert to 1-based display + prepend flame prefix
                let full_display = target_display(flame_target, &path);
                // Truncate label if too long (show more characters)
                let display_name = if full_display.len() > 14 {
                    format!("{}...", &full_display[..14])
                } else {
                    full_display.clone()
                };
                let label_color = if is_broken {
                    Color32::from_rgb(220, 140, 60)
                } else {
                    ui.visuals().text_color()
                };
                let label_text = if is_broken {
                    format!("⚠ {}", display_name)
                } else {
                    display_name
                };
                painter.text(
                    label_rect.right_center(),
                    egui::Align2::RIGHT_CENTER,
                    &label_text,
                    egui::FontId::proportional(12.0),
                    label_color,
                );
                // Add tooltip with full track name on hover
                let label_response = ui.interact(label_rect, egui::Id::new(format!("track_label_{}", track_index)), Sense::hover());
                if label_response.hovered() {
                    if is_broken {
                        label_response.on_hover_text(format!(
                            "{}\n\n⚠ Broken: target parameter doesn't exist in the current config.",
                            full_display,
                        ));
                    } else {
                        label_response.on_hover_text(&full_display);
                    }
                }

                all_track_rects.push(bg_rect);
            }

            // Buttons on the right (rendered in UI layer for interaction)
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(Rect::from_min_max(
                    Pos2::new(rect.right() - BUTTON_WIDTH, rect.top()),
                    rect.right_bottom(),
                )),
                |ui| {
                    ui.horizontal_centered(|ui| {
                        if ui.small_button(t!("track_editor.edit")).clicked() {
                            open_edit_track_panel(state, track_index, track);
                        }
                        if ui.small_button("🗑").on_hover_text(t!("track_editor.delete_track")).clicked() {
                            track_to_delete = Some(track_index);
                        }
                    });
                },
            );
        } // end of ui.is_rect_visible block
    }

    // Draw position line through all tracks
    if !all_track_rects.is_empty() {
        if let Some((bar_left, bar_right, bar_scale)) = bar_area_info {
            let painter = ui.painter();
            let first_rect = all_track_rects.first().unwrap();
            let last_rect = all_track_rects.last().unwrap();

            // Calculate position line X in the bar area
            let pos_x = bar_left + (position_x - layout.bar_left) * bar_scale;

            if pos_x >= bar_left && pos_x <= bar_right {
                painter.line_segment(
                    [
                        Pos2::new(pos_x, first_rect.top() - 4.0),
                        Pos2::new(pos_x, last_rect.bottom() + 4.0),
                    ],
                    Stroke::new(2.0, position_line_color),
                );
            }
        }
    }

    // Delete track if requested
    if let Some(index) = track_to_delete {
        animation.remove_track(index);
    }

    response
}

// ============================================================================
// UNIFIED TRACK EDITOR PANEL (Phase 7)
// ============================================================================

/// Render the unified Track Editor panel as a window
///
/// This panel combines Add Track and Edit Track functionality:
/// - Hierarchical target selector
/// - Track type selection (Keyframes, Signal)
/// - Type-specific parameter subpanels
/// - Auto-creates/updates track when target and type are valid
pub fn render_track_editor_panel(
    ctx: &egui::Context,
    controller: &mut AnimationController,
    state: &mut TrackEditorState,
    config: &FractalConfig,
    current_time: f64,
    signal_names: &[String],
) {
    if !state.track_editor_panel_open {
        return;
    }

    let is_editing = state.editing_track_index.is_some();
    let title = if is_editing {
        t!("track_editor.edit_track_title")
    } else {
        t!("track_editor.add_track_title")
    };

    let mut open = state.track_editor_panel_open;

    // Highlighted frame to make dialog stand out from docked panels
    let highlight_frame = egui::Frame::window(&ctx.global_style())
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 149, 237))); // Cornflower blue

    // Default position: keep clear of the compact mode menu button
    // (top-right hamburger area, ~80px tall including safe-area padding).
    let default_pos = {
        let screen = ctx.content_rect();
        egui::pos2(screen.center().x - 175.0, screen.min.y + 150.0)
    };

    egui::Window::new(title)
        .open(&mut open)
        .resizable(true)
        .default_pos(default_pos)
        .default_width(350.0)
        .default_height(450.0)
        .frame(highlight_frame)
        .show(ctx, |ui| {
            render_track_editor_panel_content(ui, controller, state, config, current_time, signal_names);
        });

    // Close if either X button clicked (open=false) or explicitly closed by code
    state.track_editor_panel_open = open && state.track_editor_panel_open;
}

/// Render the content of the Track Editor panel
fn render_track_editor_panel_content(
    ui: &mut Ui,
    controller: &mut AnimationController,
    state: &mut TrackEditorState,
    config: &FractalConfig,
    current_time: f64,
    signal_names: &[String],
) {
    let Some(ref mut animation) = controller.animation else {
        ui.label(t!("track_editor.no_animation"));
        return;
    };

    let is_editing = state.editing_track_index.is_some();
    let duration = animation.duration;

    // =========================================================================
    // TYPE SELECTOR (Add mode only)
    // =========================================================================
    if !is_editing {
        ui.horizontal(|ui| {
            ui.label(t!("track_editor.type"));
            egui::ComboBox::from_id_salt("track_type_selector")
                .selected_text(match state.new_track_type {
                    NewTrackType::Keyframe => t!("track_editor.type_keyframe"),
                    NewTrackType::Signal => t!("track_editor.type_signal"),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.new_track_type, NewTrackType::Keyframe, t!("track_editor.type_keyframe").as_ref());
                    ui.selectable_value(&mut state.new_track_type, NewTrackType::Signal, t!("track_editor.type_signal").as_ref());
                });
        });

        ui.separator();
    }

    // =========================================================================
    // TARGET SELECTOR (Hierarchical or read-only)
    // =========================================================================
    ui.label(t!("track_editor.target"));

    if is_editing {
        // Edit mode: Show target as read-only label, with flame prefix
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.strong(target_display(state.new_track_flame_target, &state.new_track_target));
        });
    } else {
        // Add mode: Show editable target selector
        // Show current selection
        if !state.new_track_target.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.strong(target_display(state.new_track_flame_target, &state.new_track_target));
                if ui.small_button("🗑").clicked() {
                    state.new_track_target.clear();
                    state.new_track_flame_target = EditingTarget::Main;
                }
            });
        }

        // Hierarchical target selector
        egui::CollapsingHeader::new(if state.new_track_target.is_empty() {
            t!("track_editor.select_target")
        } else {
            t!("track_editor.change_target")
        })
        .default_open(state.new_track_target.is_empty())
        .show(ui, |ui| {
            let current = if state.new_track_target.is_empty() {
                None
            } else {
                Some((state.new_track_flame_target, state.new_track_target.as_str()))
            };
            if let Some((flame_target, path)) = render_target_selector(
                ui,
                &mut state.target_selector_state,
                config,
                current,
            ) {
                state.new_track_target = path.to_string_key();
                state.new_track_flame_target = flame_target;
                // Auto-initialize values based on track type (Add mode only)
                if !is_editing {
                    match state.new_track_type {
                        NewTrackType::Keyframe => {
                            initialize_preview_keyframes(state, config, flame_target, duration);
                        }
                        NewTrackType::Signal => {
                            // Signal tracks don't need initialization from config
                        }
                    }
                }
            }
        });
    }

    ui.separator();

    // =========================================================================
    // TYPE-SPECIFIC SUBPANELS
    // =========================================================================
    match state.new_track_type {
        NewTrackType::Keyframe => {
            render_keyframe_subpanel(ui, animation, state, current_time, duration, config);
        }
        NewTrackType::Signal => {
            render_signal_subpanel(ui, state, signal_names, duration);
        }
    }

    // In Edit mode, apply changes immediately after each frame
    // This makes oscillator/circular parameter changes and type switches take effect instantly
    if is_editing {
        update_or_create_track(animation, state, duration, config);
    }

    ui.separator();

    // =========================================================================
    // ACTION BUTTONS (Add mode only)
    // =========================================================================
    if !is_editing {
        let can_create = !state.new_track_target.is_empty();

        let add_button = egui::Button::new(t!("track_editor.add_track"));
        let add_button = if can_create {
            add_button.fill(egui::Color32::from_rgb(60, 120, 60))
        } else {
            add_button
        };
        if ui.add_enabled(can_create, add_button).clicked() {
            // Create the track and close panel
            update_or_create_track(animation, state, duration, config);
            close_track_editor_panel(state);
        }
    }
}

/// Render keyframe-specific options subpanel
fn render_keyframe_subpanel(
    ui: &mut Ui,
    animation: &mut Animation,
    state: &mut TrackEditorState,
    current_time: f64,
    duration: f64,
    config: &FractalConfig,
) {
    ui.label(t!("track_editor.keyframes_section"));

    // If editing an existing keyframe track, show the keyframes
    if let Some(track_index) = state.editing_track_index {
        if let Some(track) = animation.get_track_mut(track_index) {
            if let TrackSource::Keyframes { ref mut keyframes } = track.source {
                // Interpolation selector
                ui.horizontal(|ui| {
                    ui.label(t!("track_editor.interpolation"));
                    egui::ComboBox::from_id_salt("keyframe_interpolation")
                        .selected_text(format!("{:?}", track.interpolation))
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut track.interpolation, Interpolation::Step, t!("track_editor.interpolation_step").as_ref());
                            ui.selectable_value(&mut track.interpolation, Interpolation::Linear, t!("track_editor.interpolation_linear").as_ref());
                            ui.selectable_value(&mut track.interpolation, Interpolation::Smooth, t!("track_editor.interpolation_smooth").as_ref());
                            ui.selectable_value(&mut track.interpolation, Interpolation::Sinusoidal, t!("track_editor.interpolation_sinusoidal").as_ref());
                        });
                });

                // Keyframe list (scrollable)
                ui.label(format!("{}: {}", t!("track_editor.keyframe_count"), keyframes.len()));

                ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        let mut to_delete: Option<usize> = None;
                        let kf_count = keyframes.len();

                        for (i, kf) in keyframes.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                // Time
                                ui.add(super::VkbDragValue::new(&mut kf.time)
                                    .range(0.0..=duration)
                                    .speed(0.01)
                                    .suffix("s")
                                    .min_decimals(2));

                                // Value
                                let mut value = kf.value.as_f64().unwrap_or(0.0);
                                if ui.add(super::VkbDragValue::new(&mut value).speed(0.01).min_decimals(3)).changed() {
                                    kf.value = serde_json::json!(value);
                                }

                                // Easing
                                egui::ComboBox::from_id_salt(format!("kf_ease_{}", i))
                                    .selected_text(format!("{:?}", kf.easing))
                                    .width(120.0)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut kf.easing, EasingFunction::Linear, t!("track_editor.easing_linear").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseIn, t!("track_editor.easing_easein").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseOut, t!("track_editor.easing_easeout").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseInOut, t!("track_editor.easing_easeinout").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseInCubic, t!("track_editor.easing_easeincubic").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseOutCubic, t!("track_editor.easing_easeoutcubic").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseInOutCubic, t!("track_editor.easing_easeinoutcubic").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseInSine, t!("track_editor.easing_easeinsine").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseOutSine, t!("track_editor.easing_easeoutsine").as_ref());
                                        ui.selectable_value(&mut kf.easing, EasingFunction::EaseInOutSine, t!("track_editor.easing_easeinoutsine").as_ref());
                                    });

                                // Delete (if more than 1)
                                if kf_count > 1 && ui.small_button("🗑").clicked() {
                                    to_delete = Some(i);
                                }
                            });
                        }

                        if let Some(i) = to_delete {
                            keyframes.remove(i);
                        }
                    });

                // Add keyframe buttons
                ui.horizontal(|ui| {
                    if ui.button(t!("track_editor.add_at_current")).clicked() {
                        let current_value = ConfigPath::from_string_key(&state.new_track_target)
                            .and_then(|p| get_current_value(config, state.new_track_flame_target, &p))
                            .unwrap_or(0.0);
                        keyframes.push(Keyframe {
                            time: current_time.clamp(0.0, duration),
                            value: serde_json::json!(current_value),
                            easing: EasingFunction::Linear,
                        });
                        keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
                    }

                    if ui.button(t!("track_editor.add_keyframe")).clicked() {
                        let last_time = keyframes.last().map(|k| k.time).unwrap_or(0.0);
                        let last_value = keyframes.last().and_then(|k| k.value.as_f64()).unwrap_or(0.0);
                        keyframes.push(Keyframe {
                            time: (last_time + 1.0).min(duration),
                            value: serde_json::json!(last_value),
                            easing: EasingFunction::Linear,
                        });
                    }

                    if ui.button(t!("track_editor.sort_by_time")).clicked() {
                        keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
                    }
                });

                return;
            }
        }
    }

    // Add mode: show preview keyframes editor
    if !state.preview_keyframes.is_empty() {
        // Interpolation selector for preview
        ui.horizontal(|ui| {
            ui.label(t!("track_editor.interpolation"));
            egui::ComboBox::from_id_salt("preview_interpolation")
                .selected_text(format!("{:?}", state.preview_interpolation))
                .width(100.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.preview_interpolation, Interpolation::Step, t!("track_editor.interpolation_step").as_ref());
                    ui.selectable_value(&mut state.preview_interpolation, Interpolation::Linear, t!("track_editor.interpolation_linear").as_ref());
                    ui.selectable_value(&mut state.preview_interpolation, Interpolation::Smooth, t!("track_editor.interpolation_smooth").as_ref());
                    ui.selectable_value(&mut state.preview_interpolation, Interpolation::Sinusoidal, t!("track_editor.interpolation_sinusoidal").as_ref());
                });
        });

        // Preview keyframe list
        ui.label(format!("{}: {}", t!("track_editor.keyframe_count"), state.preview_keyframes.len()));

        ScrollArea::vertical()
            .max_height(150.0)
            .show(ui, |ui| {
                let mut to_delete: Option<usize> = None;
                let kf_count = state.preview_keyframes.len();

                for (i, kf) in state.preview_keyframes.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        // Time
                        ui.add(super::VkbDragValue::new(&mut kf.time)
                            .range(0.0..=duration)
                            .speed(0.01)
                            .suffix("s")
                            .min_decimals(2));

                        // Value
                        let mut value = kf.value.as_f64().unwrap_or(0.0);
                        if ui.add(super::VkbDragValue::new(&mut value).speed(0.01).min_decimals(3)).changed() {
                            kf.value = serde_json::json!(value);
                        }

                        // Easing
                        egui::ComboBox::from_id_salt(format!("preview_kf_ease_{}", i))
                            .selected_text(format!("{:?}", kf.easing))
                            .width(120.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut kf.easing, EasingFunction::Linear, t!("track_editor.easing_linear").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseIn, t!("track_editor.easing_easein").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseOut, t!("track_editor.easing_easeout").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseInOut, t!("track_editor.easing_easeinout").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseInCubic, t!("track_editor.easing_easeincubic").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseOutCubic, t!("track_editor.easing_easeoutcubic").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseInOutCubic, t!("track_editor.easing_easeinoutcubic").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseInSine, t!("track_editor.easing_easeinsine").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseOutSine, t!("track_editor.easing_easeoutsine").as_ref());
                                ui.selectable_value(&mut kf.easing, EasingFunction::EaseInOutSine, t!("track_editor.easing_easeinoutsine").as_ref());
                            });

                        // Delete (if more than 1)
                        if kf_count > 1 && ui.small_button("🗑").clicked() {
                            to_delete = Some(i);
                        }
                    });
                }

                if let Some(i) = to_delete {
                    state.preview_keyframes.remove(i);
                }
            });

        // Add keyframe buttons
        ui.horizontal(|ui| {
            if ui.button(t!("track_editor.add_at_current")).clicked() {
                let current_value = ConfigPath::from_string_key(&state.new_track_target)
                    .and_then(|p| get_current_value(config, state.new_track_flame_target, &p))
                    .unwrap_or(0.0);
                state.preview_keyframes.push(Keyframe {
                    time: current_time.clamp(0.0, duration),
                    value: serde_json::json!(current_value),
                    easing: EasingFunction::Linear,
                });
                state.preview_keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
            }

            if ui.button(t!("track_editor.add_keyframe")).clicked() {
                let last_time = state.preview_keyframes.last().map(|k| k.time).unwrap_or(0.0);
                let last_value = state.preview_keyframes.last().and_then(|k| k.value.as_f64()).unwrap_or(0.0);
                state.preview_keyframes.push(Keyframe {
                    time: (last_time + 1.0).min(duration),
                    value: serde_json::json!(last_value),
                    easing: EasingFunction::Linear,
                });
            }

            if ui.button(t!("track_editor.sort_by_time")).clicked() {
                state.preview_keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
            }
        });
    } else {
        // No target selected yet
        ui.label(t!("track_editor.select_target_first"));
    }
}

/// Render signal track-specific options subpanel
fn render_signal_subpanel(ui: &mut Ui, state: &mut TrackEditorState, signal_names: &[String], duration: f64) {
    ui.label(t!("track_editor.signal_section"));

    ui.horizontal(|ui| {
        ui.label(t!("track_editor.signal_name"));

        if signal_names.is_empty() {
            ui.label(t!("track_editor.no_signals_available"));
        } else {
            let selected_text = if state.signal_params.signal_name.is_empty() {
                t!("track_editor.select_signal").to_string()
            } else {
                state.signal_params.signal_name.clone()
            };

            egui::ComboBox::from_id_salt("signal_name_selector")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    for name in signal_names {
                        if ui.selectable_label(
                            state.signal_params.signal_name == *name,
                            name,
                        ).clicked() {
                            state.signal_params.signal_name = name.clone();
                        }
                    }
                });
        }
    });

    ui.horizontal(|ui| {
        ui.label(t!("track_editor.signal_min_output"));
        ui.add(super::VkbDragValue::new(&mut state.signal_params.min_output).speed(0.01));
    });

    ui.horizontal(|ui| {
        ui.label(t!("track_editor.signal_max_output"));
        ui.add(super::VkbDragValue::new(&mut state.signal_params.max_output).speed(0.01));
    });

    ui.horizontal(|ui| {
        ui.label(t!("track_editor.signal_smoothing"));
        ui.add(super::VkbDragValue::new(&mut state.signal_params.smoothing).speed(0.01).range(0.0..=0.99));
    });

    ui.separator();

    // Timing controls
    ui.horizontal(|ui| {
        ui.label(t!("track_editor.signal_start_time"));
        ui.add(super::VkbDragValue::new(&mut state.signal_params.start_time)
            .speed(0.1)
            .range(0.0..=duration)
            .suffix("s")
            .min_decimals(1));
    });

    ui.horizontal(|ui| {
        ui.label(t!("track_editor.signal_end_time"));
        ui.add(super::VkbDragValue::new(&mut state.signal_params.end_time)
            .speed(0.1)
            .range(0.0..=duration)
            .suffix("s")
            .min_decimals(1));
        if state.signal_params.end_time == 0.0 {
            ui.weak("(end)");
        }
    });

    // Fade controls
    ui.horizontal(|ui| {
        ui.label(t!("track_editor.signal_fade_in"));
        ui.add(super::VkbDragValue::new(&mut state.signal_params.fade_in)
            .speed(0.1)
            .range(0.0..=30.0)
            .suffix("s")
            .min_decimals(1));
        render_easing_combo(ui, "fade_in_easing", &mut state.signal_params.fade_in_easing);
    });

    ui.horizontal(|ui| {
        ui.label(t!("track_editor.signal_fade_out"));
        ui.add(super::VkbDragValue::new(&mut state.signal_params.fade_out)
            .speed(0.1)
            .range(0.0..=30.0)
            .suffix("s")
            .min_decimals(1));
        render_easing_combo(ui, "fade_out_easing", &mut state.signal_params.fade_out_easing);
    });
}

/// Render an easing function combo box
fn render_easing_combo(ui: &mut Ui, id: &str, easing: &mut EasingFunction) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(format!("{:?}", easing))
        .width(120.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(easing, EasingFunction::Linear, "Linear");
            ui.selectable_value(easing, EasingFunction::EaseIn, "Ease In");
            ui.selectable_value(easing, EasingFunction::EaseOut, "Ease Out");
            ui.selectable_value(easing, EasingFunction::EaseInOut, "Ease In/Out");
            ui.selectable_value(easing, EasingFunction::EaseInCubic, "Ease In Cubic");
            ui.selectable_value(easing, EasingFunction::EaseOutCubic, "Ease Out Cubic");
            ui.selectable_value(easing, EasingFunction::EaseInOutCubic, "Ease In/Out Cubic");
            ui.selectable_value(easing, EasingFunction::EaseInSine, "Ease In Sine");
            ui.selectable_value(easing, EasingFunction::EaseOutSine, "Ease Out Sine");
            ui.selectable_value(easing, EasingFunction::EaseInOutSine, "Ease In/Out Sine");
        });
}

/// Create or update a track based on current state
/// Returns the index of the created/updated track (for switching to edit mode)
fn update_or_create_track(
    animation: &mut Animation,
    state: &mut TrackEditorState,
    duration: f64,
    config: &FractalConfig,
) -> Option<usize> {
    match state.new_track_type {
        NewTrackType::Keyframe => {
            if let Some(track_index) = state.editing_track_index {
                // Update existing track
                if let Some(track) = animation.get_track_mut(track_index) {
                    track.target = state.new_track_target.clone();
                    track.flame_target = state.new_track_flame_target;
                    // Re-bind: target may have changed to a different item.
                    track.bind(config);
                    // Keep existing keyframes, just update target
                }
                Some(track_index)
            } else {
                // Create new track using preview keyframes (auto-filled when target selected)
                let keyframes = if state.preview_keyframes.is_empty() {
                    // Fallback if no preview keyframes
                    let initial_value = ConfigPath::from_string_key(&state.new_track_target)
                        .and_then(|p| get_current_value(config, state.new_track_flame_target, &p))
                        .unwrap_or(0.0);
                    vec![
                        Keyframe {
                            time: 0.0,
                            value: serde_json::json!(initial_value),
                            easing: EasingFunction::Linear,
                        },
                        Keyframe {
                            time: duration,
                            value: serde_json::json!(initial_value),
                            easing: EasingFunction::Linear,
                        },
                    ]
                } else {
                    state.preview_keyframes.clone()
                };
                let mut track = Track::new(
                    state.new_track_target.clone(),
                    TrackSource::Keyframes { keyframes },
                )
                .with_flame_target(state.new_track_flame_target);
                track.interpolation = state.preview_interpolation;
                // Bind right away so the rebind hook can keep the
                // track pointing at the right item after add/delete/
                // reorder, and `is_track_broken` reports correctly.
                track.bind(config);
                let new_index = animation.add_track(track);
                Some(new_index)
            }
        }
        NewTrackType::Signal => {
            if let Some(track_index) = state.editing_track_index {
                // Update existing track
                if let Some(track) = animation.get_track_mut(track_index) {
                    track.target = state.new_track_target.clone();
                    track.flame_target = state.new_track_flame_target;
                    track.source = TrackSource::Signal {
                        signal_name: state.signal_params.signal_name.clone(),
                        min_output: state.signal_params.min_output,
                        max_output: state.signal_params.max_output,
                        smoothing: state.signal_params.smoothing,
                        start_time: state.signal_params.start_time,
                        end_time: state.signal_params.end_time,
                        fade_in: state.signal_params.fade_in,
                        fade_in_easing: state.signal_params.fade_in_easing,
                        fade_out: state.signal_params.fade_out,
                        fade_out_easing: state.signal_params.fade_out_easing,
                    };
                    track.bind(config);
                }
                Some(track_index)
            } else {
                // Create new track
                let mut track = Track::new(
                    state.new_track_target.clone(),
                    TrackSource::Signal {
                        signal_name: state.signal_params.signal_name.clone(),
                        min_output: state.signal_params.min_output,
                        max_output: state.signal_params.max_output,
                        smoothing: state.signal_params.smoothing,
                        start_time: state.signal_params.start_time,
                        end_time: state.signal_params.end_time,
                        fade_in: state.signal_params.fade_in,
                        fade_in_easing: state.signal_params.fade_in_easing,
                        fade_out: state.signal_params.fade_out,
                        fade_out_easing: state.signal_params.fade_out_easing,
                    },
                )
                .with_flame_target(state.new_track_flame_target);
                track.bind(config);
                let new_index = animation.add_track(track);
                Some(new_index)
            }
        }
    }
}

/// Close the track editor panel and reset state
fn close_track_editor_panel(state: &mut TrackEditorState) {
    state.track_editor_panel_open = false;
    state.editing_track_index = None;
    state.new_track_target.clear();
    state.new_track_flame_target = EditingTarget::Main;
    state.target_selector_state = TargetSelectorState::default();
    state.preview_keyframes.clear();
    state.preview_interpolation = Interpolation::Linear;
}

/// Open the track editor panel to add a new track
pub fn open_add_track_panel(state: &mut TrackEditorState) {
    state.track_editor_panel_open = true;
    state.editing_track_index = None;
    state.new_track_target.clear();
    state.new_track_flame_target = EditingTarget::Main;
    state.new_track_type = NewTrackType::Keyframe;
    state.signal_params = SignalParams::default();
    state.target_selector_state = TargetSelectorState::default();
    state.preview_keyframes.clear();
    state.preview_interpolation = Interpolation::Linear;
}

/// Open the track editor panel to edit an existing track by index
pub fn open_edit_track_panel(state: &mut TrackEditorState, track_index: usize, track: &Track) {
    state.track_editor_panel_open = true;
    state.editing_track_index = Some(track_index);
    state.new_track_target = track.target.clone();
    state.new_track_flame_target = track.flame_target;
    state.new_track_type = match &track.source {
        TrackSource::Keyframes { .. } => NewTrackType::Keyframe,
        TrackSource::Signal {
            signal_name, min_output, max_output, smoothing,
            start_time, end_time, fade_in, fade_in_easing, fade_out, fade_out_easing,
        } => {
            state.signal_params = SignalParams {
                signal_name: signal_name.clone(),
                min_output: *min_output,
                max_output: *max_output,
                smoothing: *smoothing,
                start_time: *start_time,
                end_time: *end_time,
                fade_in: *fade_in,
                fade_in_easing: *fade_in_easing,
                fade_out: *fade_out,
                fade_out_easing: *fade_out_easing,
            };
            NewTrackType::Signal
        }
    };
    state.target_selector_state = TargetSelectorState::default();
}

/// Read one affine coefficient from a transform — shared across all
/// pool sites so the match arms only differ in how they look the
/// transform up.
fn read_affine(t: &crate::scene::transforms::Transform, param: crate::config::AffineParam) -> f64 {
    use crate::config::AffineParam;
    match param {
        AffineParam::A => t.a as f64,
        AffineParam::B => t.b as f64,
        AffineParam::C => t.c as f64,
        AffineParam::D => t.d as f64,
        AffineParam::E => t.e as f64,
        AffineParam::F => t.f as f64,
        AffineParam::G => t.g as f64,
    }
}

/// Same as `read_affine` but for the post-affine half.
fn read_post_affine(t: &crate::scene::transforms::Transform, param: crate::config::AffineParam) -> f64 {
    use crate::config::AffineParam;
    match param {
        AffineParam::A => t.post_a as f64,
        AffineParam::B => t.post_b as f64,
        AffineParam::C => t.post_c as f64,
        AffineParam::D => t.post_d as f64,
        AffineParam::E => t.post_e as f64,
        AffineParam::F => t.post_f as f64,
        AffineParam::G => t.post_g as f64,
    }
}

/// Get the current value for a ConfigPath from the fractal config,
/// routed through `flame_target` for per-flame paths. Returns None if
/// the path doesn't map to a numeric value or the target flame
/// doesn't exist.
pub fn get_current_value(
    config: &FractalConfig,
    flame_target: EditingTarget,
    path: &ConfigPath,
) -> Option<f64> {
    // Per-flame paths need the resolved flame; resolve up front so
    // the per-flame arms below can index into it directly.
    let flame = resolve_flame(config, flame_target)?;

    match path {
        // View parameters
        ConfigPath::Zoom => Some(config.zoom as f64),
        ConfigPath::PanX => Some(config.pan_x as f64),
        ConfigPath::PanY => Some(config.pan_y as f64),
        ConfigPath::Rotation => Some(config.rotation as f64),
        ConfigPath::CameraRotationX => Some(config.camera_rotation_x as f64),
        ConfigPath::CameraRotationY => Some(config.camera_rotation_y as f64),
        ConfigPath::CameraZ => Some(config.camera_z as f64),

        // Tone mapping
        ConfigPath::Exposure => Some(config.exposure as f64),
        ConfigPath::Gamma => Some(config.gamma as f64),
        ConfigPath::GammaThreshold => Some(config.gamma_threshold as f64),
        ConfigPath::Brightness => Some(config.brightness as f64),
        ConfigPath::Vibrancy => Some(config.vibrancy as f64),
        ConfigPath::Saturation => Some(config.saturation as f64),
        ConfigPath::HueShift => Some(config.hue_shift as f64),
        ConfigPath::DensityScale => Some(config.density_scale as f64),
        ConfigPath::LevelsLow => Some(config.levels_low as f64),
        ConfigPath::LevelsHigh => Some(config.levels_high as f64),
        ConfigPath::LevelsGamma => Some(config.levels_gamma as f64),

        // Color
        ConfigPath::PaletteRotation => Some(config.palette_rotation as f64),
        ConfigPath::PaletteSqueeze => Some(config.palette_squeeze as f64),
        ConfigPath::SpeedFactor => Some(config.speed_factor as f64),
        ConfigPath::BackgroundColorR => Some(config.background_color[0] as f64),
        ConfigPath::BackgroundColorG => Some(config.background_color[1] as f64),
        ConfigPath::BackgroundColorB => Some(config.background_color[2] as f64),

        // Rendering
        ConfigPath::HistogramColorScale => Some(config.histogram_color_scale as f64),
        ConfigPath::BlendFactor => Some(config.blend_factor as f64),
        ConfigPath::PerspectiveStrength => Some(config.flame.perspective_strength as f64),

        // Transform parameters
        ConfigPath::TransformWeight { index } => {
            flame.transforms.get(*index).map(|t| t.weight as f64)
        }
        ConfigPath::TransformColor { index } => {
            flame.transforms.get(*index).map(|t| t.color as f64)
        }
        ConfigPath::TransformColorSpeed { index } => {
            flame.transforms.get(*index).map(|t| t.color_speed as f64)
        }
        ConfigPath::TransformOpacity { index } => {
            flame.transforms.get(*index).map(|t| t.opacity as f64)
        }
        ConfigPath::TransformAffine { index, param } => {
            flame.transforms.get(*index).map(|t| read_affine(t, *param))
        }
        ConfigPath::TransformPostAffineEnabled { index } => {
            flame.transforms.get(*index).map(|t| if t.post_affine_enabled { 1.0 } else { 0.0 })
        }
        ConfigPath::TransformPostAffine { index, param } => {
            flame.transforms.get(*index).map(|t| read_post_affine(t, *param))
        }
        ConfigPath::TransformOriginX { index } => {
            flame.transforms.get(*index).map(|t| t.e as f64)
        }
        ConfigPath::TransformOriginY { index } => {
            flame.transforms.get(*index).map(|t| -t.f as f64)
        }
        ConfigPath::TransformRotation { index } => {
            flame.transforms.get(*index).map(|t| t.rotation() as f64)
        }
        ConfigPath::TransformScale { index } => {
            flame.transforms.get(*index).map(|t| t.scale() as f64)
        }
        ConfigPath::TransformPostAffineOriginX { index } => {
            flame.transforms.get(*index).map(|t| t.post_origin_x() as f64)
        }
        ConfigPath::TransformPostAffineOriginY { index } => {
            flame.transforms.get(*index).map(|t| t.post_origin_y() as f64)
        }
        ConfigPath::TransformPostAffineRotation { index } => {
            flame.transforms.get(*index).map(|t| t.post_rotation() as f64)
        }
        ConfigPath::TransformPostAffineScale { index } => {
            flame.transforms.get(*index).map(|t| t.post_scale() as f64)
        }
        ConfigPath::TransformVariation { index, variation } => {
            flame.transforms.get(*index).and_then(|t| {
                t.variations.get(variation).map(|&v| v as f64)
            })
        }
        ConfigPath::TransformVariationParam { index, variation, param } => {
            flame.transforms.get(*index).map(|t| {
                t.get_variation_param_or_default(
                    variation, param, &crate::variations::global_registry()
                ) as f64
            })
        }

        // Linked pool — animatable per-pool-member parameters.
        ConfigPath::LinkedTransformAffine { index, param } => {
            flame.linked_transforms.get(*index).map(|t| read_affine(t, *param))
        }
        ConfigPath::LinkedTransformPostAffineEnabled { index } => {
            flame.linked_transforms.get(*index).map(|t| if t.post_affine_enabled { 1.0 } else { 0.0 })
        }
        ConfigPath::LinkedTransformPostAffine { index, param } => {
            flame.linked_transforms.get(*index).map(|t| read_post_affine(t, *param))
        }
        ConfigPath::LinkedTransformVariation { index, variation } => {
            flame.linked_transforms.get(*index).and_then(|t| {
                t.variations.get(variation).map(|&v| v as f64)
            })
        }
        ConfigPath::LinkedTransformVariationParam { index, variation, param } => {
            flame.linked_transforms.get(*index).map(|t| {
                t.get_variation_param_or_default(
                    variation, param, &crate::variations::global_registry()
                ) as f64
            })
        }
        ConfigPath::LinkedTransformOriginX { index } => {
            flame.linked_transforms.get(*index).map(|t| t.origin_x() as f64)
        }
        ConfigPath::LinkedTransformOriginY { index } => {
            flame.linked_transforms.get(*index).map(|t| t.origin_y() as f64)
        }
        ConfigPath::LinkedTransformRotation { index } => {
            flame.linked_transforms.get(*index).map(|t| t.rotation() as f64)
        }
        ConfigPath::LinkedTransformScale { index } => {
            flame.linked_transforms.get(*index).map(|t| t.scale() as f64)
        }
        ConfigPath::LinkedTransformPostAffineOriginX { index } => {
            flame.linked_transforms.get(*index).map(|t| t.post_origin_x() as f64)
        }
        ConfigPath::LinkedTransformPostAffineOriginY { index } => {
            flame.linked_transforms.get(*index).map(|t| t.post_origin_y() as f64)
        }
        ConfigPath::LinkedTransformPostAffineRotation { index } => {
            flame.linked_transforms.get(*index).map(|t| t.post_rotation() as f64)
        }
        ConfigPath::LinkedTransformPostAffineScale { index } => {
            flame.linked_transforms.get(*index).map(|t| t.post_scale() as f64)
        }

        // Final pool — animatable per-pool-member parameters.
        ConfigPath::FinalTransformAffine { index, param } => {
            flame.final_transforms.get(*index).map(|t| read_affine(t, *param))
        }
        ConfigPath::FinalTransformPostAffineEnabled { index } => {
            flame.final_transforms.get(*index).map(|t| if t.post_affine_enabled { 1.0 } else { 0.0 })
        }
        ConfigPath::FinalTransformPostAffine { index, param } => {
            flame.final_transforms.get(*index).map(|t| read_post_affine(t, *param))
        }
        ConfigPath::FinalTransformVariation { index, variation } => {
            flame.final_transforms.get(*index).and_then(|t| {
                t.variations.get(variation).map(|&v| v as f64)
            })
        }
        ConfigPath::FinalTransformVariationParam { index, variation, param } => {
            flame.final_transforms.get(*index).map(|t| {
                t.get_variation_param_or_default(
                    variation, param, &crate::variations::global_registry()
                ) as f64
            })
        }
        ConfigPath::FinalTransformOriginX { index } => {
            flame.final_transforms.get(*index).map(|t| t.origin_x() as f64)
        }
        ConfigPath::FinalTransformOriginY { index } => {
            flame.final_transforms.get(*index).map(|t| t.origin_y() as f64)
        }
        ConfigPath::FinalTransformRotation { index } => {
            flame.final_transforms.get(*index).map(|t| t.rotation() as f64)
        }
        ConfigPath::FinalTransformScale { index } => {
            flame.final_transforms.get(*index).map(|t| t.scale() as f64)
        }
        ConfigPath::FinalTransformPostAffineOriginX { index } => {
            flame.final_transforms.get(*index).map(|t| t.post_origin_x() as f64)
        }
        ConfigPath::FinalTransformPostAffineOriginY { index } => {
            flame.final_transforms.get(*index).map(|t| t.post_origin_y() as f64)
        }
        ConfigPath::FinalTransformPostAffineRotation { index } => {
            flame.final_transforms.get(*index).map(|t| t.post_rotation() as f64)
        }
        ConfigPath::FinalTransformPostAffineScale { index } => {
            flame.final_transforms.get(*index).map(|t| t.post_scale() as f64)
        }

        // Legacy `FinalTransform*` (no index) variants were removed in
        // Phase 9. Animation tracks saved against the legacy string
        // form route through the migration shim in
        // `ConfigPath::from_string_key`, which maps them to indexed
        // variants at index 0 — they hit the indexed match arms above.

        // Non-numeric or complex types
        _ => None,
    }
}

/// Determine the auto-fill end value for a parameter
/// Most parameters just return the same value (start = end)
/// Special parameters like rotation get +2π for a full cycle
pub fn get_auto_fill_end_value(path: &ConfigPath, start_value: f64) -> f64 {
    use std::f64::consts::TAU; // 2π

    match path {
        // Rotation parameters: add full rotation
        ConfigPath::TransformRotation { .. } |
        ConfigPath::TransformPostAffineRotation { .. } |
        ConfigPath::LinkedTransformRotation { .. } |
        ConfigPath::LinkedTransformPostAffineRotation { .. } |
        ConfigPath::FinalTransformRotation { .. } |
        ConfigPath::FinalTransformPostAffineRotation { .. } |
        ConfigPath::Rotation |
        ConfigPath::CameraRotationX |
        ConfigPath::CameraRotationY => start_value + TAU,

        // Color index: add 1.0 for full palette cycle
        ConfigPath::TransformColor { .. } => start_value + 1.0,

        // Palette rotation: add 1.0 for full rotation
        ConfigPath::PaletteRotation => start_value + 1.0,

        // All other parameters: same as start (user adjusts manually)
        _ => start_value,
    }
}

/// Initialize preview keyframes based on the selected target
pub fn initialize_preview_keyframes(
    state: &mut TrackEditorState,
    config: &FractalConfig,
    flame_target: EditingTarget,
    duration: f64,
) {
    // Parse the target string to ConfigPath
    let path = match ConfigPath::from_string_key(&state.new_track_target) {
        Some(p) => p,
        None => {
            // Can't parse path, use default keyframes
            state.preview_keyframes = vec![
                Keyframe { time: 0.0, value: serde_json::json!(0.0), easing: EasingFunction::Linear },
                Keyframe { time: duration, value: serde_json::json!(1.0), easing: EasingFunction::Linear },
            ];
            return;
        }
    };

    // Get current value (routed via flame_target so per-flame paths
    // pick up the right Flame slice for subflame targets).
    let start_value = get_current_value(config, flame_target, &path).unwrap_or(0.0);
    let end_value = get_auto_fill_end_value(&path, start_value);

    state.preview_keyframes = vec![
        Keyframe { time: 0.0, value: serde_json::json!(start_value), easing: EasingFunction::Linear },
        Keyframe { time: duration, value: serde_json::json!(end_value), easing: EasingFunction::Linear },
    ];
    state.preview_interpolation = Interpolation::Linear;
}


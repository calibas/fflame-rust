//! TabViewer implementation for rendering docked panels

use egui_dock::{egui, TabViewer};
use rust_i18n::t;
use super::workspace::PanelType;
use crate::config::manager::EditingTarget;

/// Resolve which `Flame` slice the editor panels should operate on,
/// given the active editing target. Triangle Editor and Transforms
/// panels each get a `&mut Flame`; for `Main` that's the parent
/// flame itself, for `Subflame { index }` it's
/// `parent.subflames[index]`. Falls back to the parent flame if the
/// target index is out of bounds (defensive — UI shouldn't have set
/// such a target).
fn active_target_flame_mut<'a>(
    parent: &'a mut crate::scene::transforms::Flame,
    target: EditingTarget,
) -> &'a mut crate::scene::transforms::Flame {
    match target {
        EditingTarget::Subflame { index } if index < parent.subflames.len() => {
            &mut parent.subflames[index]
        }
        _ => parent,
    }
}

/// Result of processing touch events each frame.
pub enum TouchGesture {
    /// Single finger drag (translation delta)
    Pan(egui::Vec2),
    /// Two-finger pinch (zoom_delta, translation_delta, midpoint)
    Pinch { zoom_delta: f32, translation: egui::Vec2, midpoint: egui::Pos2 },
}

/// Tracks active touch points for manual gesture detection.
/// Needed because egui's built-in `multi_touch()` fails on web when winit
/// assigns different `TouchDeviceId` per finger, and egui-winit's touch→mouse
/// emulation breaks after multi-touch ends without proper End events.
#[derive(Default)]
pub struct TouchTracker {
    /// Active touches: (id, current_position)
    active: std::collections::HashMap<u64, egui::Pos2>,
    /// Previous frame's distance between fingers (for zoom delta)
    prev_distance: Option<f32>,
    /// Previous frame's midpoint (for two-finger translation)
    prev_midpoint: Option<egui::Pos2>,
    /// Previous frame's single-finger position (for one-finger pan)
    prev_single_pos: Option<egui::Pos2>,
}

impl TouchTracker {
    /// Process touch events and return the detected gesture, if any.
    pub fn update(&mut self, events: &[egui::Event]) -> Option<TouchGesture> {
        if events.is_empty() {
            return None;
        }

        // Collect IDs present in this event batch
        let batch_ids: std::collections::HashSet<u64> = events.iter()
            .filter_map(|e| if let egui::Event::Touch { id, .. } = e { Some(id.0) } else { None })
            .collect();

        // If a new finger starts, purge any stale touches not in this batch.
        // This handles fingers that left the viewport without an End event.
        let has_start = events.iter().any(|e|
            matches!(e, egui::Event::Touch { phase: egui::TouchPhase::Start, .. })
        );
        if has_start {
            self.active.retain(|id, _| batch_ids.contains(id));
            // Reset position tracking so the first frame of a new touch
            // doesn't compute a delta from a previous finger's position
            self.prev_single_pos = None;
            self.prev_midpoint = None;
            self.prev_distance = None;
        }

        for event in events {
            if let egui::Event::Touch { device_id: _, id, phase, pos, .. } = event {
                let touch_id = id.0;
                match phase {
                    egui::TouchPhase::Start => {
                        self.active.insert(touch_id, *pos);
                    }
                    egui::TouchPhase::Move => {
                        self.active.insert(touch_id, *pos);
                    }
                    egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                        self.active.remove(&touch_id);
                    }
                }
            }
        }

        match self.active.len() {
            0 => {
                self.prev_distance = None;
                self.prev_midpoint = None;
                self.prev_single_pos = None;
                None
            }
            1 => {
                self.prev_distance = None;
                self.prev_midpoint = None;

                let pos = *self.active.values().next().unwrap();
                let delta = if let Some(prev) = self.prev_single_pos {
                    pos - prev
                } else {
                    egui::Vec2::ZERO
                };
                self.prev_single_pos = Some(pos);

                if delta != egui::Vec2::ZERO {
                    Some(TouchGesture::Pan(delta))
                } else {
                    None
                }
            }
            _ => {
                self.prev_single_pos = None;

                let points: Vec<egui::Pos2> = self.active.values().copied().collect();
                let p1 = points[0];
                let p2 = points[1];
                let distance = p1.distance(p2);
                let midpoint = egui::pos2((p1.x + p2.x) * 0.5, (p1.y + p2.y) * 0.5);

                let zoom_delta = if let Some(prev) = self.prev_distance {
                    if prev > 0.0 { distance / prev } else { 1.0 }
                } else {
                    1.0
                };

                let translation = if let Some(prev_mid) = self.prev_midpoint {
                    midpoint - prev_mid
                } else {
                    egui::Vec2::ZERO
                };

                self.prev_distance = Some(distance);
                self.prev_midpoint = Some(midpoint);

                Some(TouchGesture::Pinch { zoom_delta, translation, midpoint })
            }
        }
    }

    /// Returns true if a specific touch ID is being tracked
    pub fn is_tracking(&self, id: u64) -> bool {
        self.active.contains_key(&id)
    }

    /// Returns true if any fingers are currently active
    pub fn is_touch_active(&self) -> bool {
        !self.active.is_empty()
    }
}

/// Context needed by panels to render
///
/// Holds references to all UI state from EguiLayer that panels might need.
/// This avoids passing 20+ parameters to each panel.
pub struct PanelContext<'a> {
    // Core state
    pub config_manager: &'a mut crate::config::ConfigManager,
    /// Free-fly camera mode signal from App. When true the viewport
    /// panel re-routes primary mouse drag from "pan" to "look around"
    /// (writes the delta into `fly_mouse_drag` instead of calling
    /// pan_fractal_view).
    pub fly_mode_active: bool,
    /// Set by the viewport panel when a mouse drag in fly mode produces
    /// pixel-space delta. Consumed by App.
    pub fly_mouse_drag: &'a mut Option<(f32, f32)>,
    /// Set by the View panel's fly-mode toggle button. Consumed by App.
    pub fly_mode_toggle_requested: &'a mut bool,
    pub flame: &'a mut crate::scene::transforms::Flame,
    /// Last-fetched server variation catalog, if one has ever been
    /// fetched. `None` offline-and-never-fetched; the Variations panel
    /// simply omits its catalog section rather than showing an error.
    pub variation_catalog: Option<&'a crate::storage::variation_catalog::CachedCatalog>,

    // Libraries
    pub preset_library: &'a crate::scene::presets::PresetLibrary,
    pub palette_library: &'a mut crate::scene::palette::PaletteLibrary,

    // Renderer (optional, might not exist during init)
    pub flame_renderer: Option<&'a crate::renderer::compute_kernel::FlameRenderer>,

    // Animation controller
    pub animation_controller: &'a mut crate::animation::AnimationController,

    // Action flags
    pub add_transform: &'a mut bool,
    pub delete_transform: &'a mut Option<usize>,
    pub clone_transform: &'a mut Option<usize>,

    // Linked-pool actions
    pub add_linked_transform: &'a mut bool,
    pub delete_linked_transform: &'a mut Option<usize>,
    pub clone_linked_transform: &'a mut Option<usize>,

    // Final-pool actions
    pub add_final_transform: &'a mut bool,
    pub delete_final_transform: &'a mut Option<usize>,
    pub clone_final_transform: &'a mut Option<usize>,

    // Per-normal attachment edit (Linked/Final toggle or reorder).
    pub attachment_edit: &'a mut Option<crate::ui::response::AttachmentEdit>,
    pub undo_requested: &'a mut bool,
    pub redo_requested: &'a mut bool,
    pub open_palette_editor: &'a mut bool,
    pub open_palette_library: &'a mut bool,
    pub open_triangle_editor: &'a mut bool,
    pub open_preset_library: &'a mut bool,
    pub open_random_generator: &'a mut bool,

    // UI state
    pub paused: &'a mut bool,
    /// Subflames panel checkbox: when true and a subflame is being
    /// edited, the viewport renders that subflame's IFS in
    /// isolation; otherwise the parent flame is rendered (default).
    /// App reads this in `gpu_updates` to pick the render source.
    pub view_subflame_in_isolation: &'a mut bool,
    /// Subflames panel: user clicked the "Load from file" button on a
    /// subflame row. Carries the target index; consumed by App.
    pub load_subflame_into: &'a mut Option<usize>,
    pub png_export_with_background: &'a mut bool,
    pub png_export_transparent: &'a mut bool,
    pub export_width: &'a mut u32,
    pub export_height: &'a mut u32,
    pub use_custom_export_size: &'a mut bool,
    pub png_export_premultiplied: &'a mut bool,
    pub png_export_supersample: &'a mut bool,
    /// GPU `max_texture_dimension_2d` — per-axis ceiling for export size.
    pub max_export_dimension: u32,
    pub palette_editor: &'a mut crate::ui::palette_editor::PaletteEditor,
    pub palette_export_json: &'a mut Option<crate::scene::palette::Palette>,
    pub palette_save_file: &'a mut Option<crate::scene::palette::Palette>,
    pub palette_save_to_library: &'a mut Option<crate::scene::palette::Palette>,
    pub palette_delete_from_library: &'a mut Option<String>,
    pub palette_import_json: &'a mut Option<String>,
    pub palette_load_file: &'a mut bool,

    // Performance metrics
    pub metrics: &'a crate::util::PerformanceMetrics,
    pub window_size: winit::dpi::PhysicalSize<u32>,
    /// Reference to the main window. Used as the parent for any
    /// native file dialogs opened from inside a panel so they stay
    /// on top of the app — without this, the OS may place the
    /// dialog behind the main window and lock the UI.
    pub window: &'a winit::window::Window,

    // Fractal texture for display
    pub fractal_texture_id: Option<egui::TextureId>,
    pub fractal_viewport_size: &'a mut Option<(u32, u32)>,
    /// Tab bar height from previous frame, used to inflate fractal texture size
    pub viewport_tab_bar_height: f32,

    // Config dialog state
    pub config_json_buffer: &'a mut String,
    pub config_export_json: &'a mut Option<String>,
    pub config_import_json: &'a mut Option<String>,
    pub config_save_file: &'a mut bool,
    pub config_load_file: &'a mut bool,
    pub flame_xml_import_file: &'a mut bool,
    pub flame_xml_export_file: &'a mut bool,
    pub open_config_dialog: &'a mut bool,

    // Selected preset config (from any browser)
    pub selected_preset_config: &'a mut Option<crate::config::FractalConfig>,

    // API flame ID loaded from Online tab
    pub loaded_api_flame_id: &'a mut Option<String>,
    // Visibility of flame loaded from Online tab
    pub loaded_api_flame_is_public: &'a mut Option<bool>,
    // Owner user ID of flame loaded from Online tab
    pub loaded_api_flame_user_id: &'a mut Option<String>,
    // Animation count of flame loaded from Online tab
    pub loaded_api_flame_animation_count: &'a mut u32,
    // List of animations linked to the loaded flame
    pub loaded_api_flame_animations: &'a mut Vec<crate::api::types::AnimationSummary>,

    // API notification from browser panel (e.g., delete result)
    pub api_notification: &'a mut Option<(String, bool)>,

    // Login dialog state
    pub login_dialog_state: &'a mut super::login_dialog::LoginDialogState,

    // Save Online dialog state
    pub save_online_dialog_state: &'a mut super::save_online_dialog::SaveOnlineDialogState,

    // Sign out requested (from account panel or 401 detection)
    pub sign_out_requested: &'a mut bool,

    // Cloud palette state (for Palette Library cloud section)
    pub cloud_palette_state: &'a mut super::CloudPaletteState,

    // File browser open request (shared by FractalBrowser)
    pub file_browser_open_requested: &'a mut bool,

    // Animation export settings
    pub animation_export_settings: &'a mut super::animation_panel::AnimationExportSettings,
    pub animation_export_requested: &'a mut Option<super::animation_panel::AnimationExportSettings>,

    // Whether ANY export is running. Disables export buttons; live progress is
    // shown by the global overlay (`export_status::render_export_overlay`), not
    // by the panels.
    pub export_active: bool,

    // Export Animation panel state (Phase 5)
    pub export_panel_state: &'a mut super::animation_panel::ExportPanelState,

    // Track editor state
    pub track_editor_state: &'a mut super::track_editor::TrackEditorState,

    // Animation seek changed flag (timeline was scrubbed)
    pub animation_seek_changed: &'a mut bool,

    // Animation scrubber drag stopped or discrete seek action (frame step) - reset accumulation
    pub animation_seek_drag_stopped: &'a mut bool,

    // PathMap mode: hovered pixel coordinates and cached path info
    pub hovered_pixel: &'a mut Option<(u32, u32)>,
    pub path_click_info: &'a Option<super::PathClickInfo>,
    pub close_path_overlay: &'a mut bool,

    // Path editor state
    pub path_editor_state: &'a mut super::path_editor::PathEditorState,
    pub path_filters_changed: &'a mut Option<Vec<crate::gpu::buffers::GpuPathFilter>>,

    // Random generator panel state
    pub random_generator_panel: &'a mut Option<super::random_generator::RandomGeneratorPanel>,
    pub generated_flame: &'a mut Option<crate::scene::randomize::RandomFlame>,
    pub generated_batch: &'a mut Option<Vec<crate::config::FractalConfig>>,
    pub scripts_panel: &'a mut Option<super::scripts_panel::ScriptsPanel>,
    pub script_generated: &'a mut Option<crate::config::FractalConfig>,
    pub script_animation: &'a mut Option<crate::animation::Animation>,

    // Fractal browser panel state (unified presets/batch/files)
    pub fractal_browser_panel: &'a mut Option<super::fractal_browser::FractalBrowserPanel>,

    // Histogram for density visualization (levels now in ConfigManager)
    pub density_histogram: &'a crate::renderer::DensityHistogram,

    // Xaos editor state
    pub xaos_editor_state: &'a mut super::xaos_editor::XaosEditorState,

    // Signal panel state
    pub audio_manager: &'a mut crate::audio::AudioManager,
    pub audio_player: &'a mut crate::audio::AudioPlayer,
    pub audio_capture: &'a mut crate::audio::AudioCapture,
    pub signal_panel_state: &'a mut super::signal_panel::SignalPanelState,
    pub signal_manager: &'a mut crate::signal::SignalManager,
    pub current_time: f64,
    pub load_audio_file: &'a mut bool,
    pub load_signal_file: &'a mut bool,
    pub save_signal_file: &'a mut Option<String>,

    // API animation state (for Save/Update Online buttons in animation panel)
    pub api_flame_id: &'a Option<String>,
    pub api_animation_id: &'a Option<String>,
    pub flame_animations: &'a [crate::api::types::AnimationSummary],
    pub is_signed_in: bool,

    // API animation save action (from animation panel)
    pub api_animation_save_action: &'a mut super::response::ApiAnimationSaveAction,

    // Open Save Online dialog (from animation panel)
    pub open_save_online_dialog: &'a mut bool,
    pub load_api_animation_id: &'a mut Option<String>,
    pub clear_variation_cache_requested: &'a mut bool,
    /// Downloaded variations the Variations panel asked to re-fetch at
    /// the catalog's version. Consumed by App.
    pub variation_update_requested: &'a mut Vec<String>,
    /// Online-library state the Scripts panel reads, and the slot it
    /// writes a request into. Owned by App.
    pub script_cloud: &'a crate::app::script_cloud::ScriptCloudState,
    /// Last-fetched effect catalog, if there is one. `None` offline and
    /// never-fetched; the panel simply omits its catalog section.
    pub effect_catalog: Option<&'a crate::storage::effect_catalog::CachedEffectCatalog>,
    pub script_cloud_request: &'a mut Option<crate::app::script_cloud::ScriptCloudRequest>,
    /// Whether anyone is signed in — the panel shows its online section
    /// only then, rather than offering actions that cannot work.
    pub signed_in: bool,

    // Compact mode (cached from system settings)
    pub compact_mode: bool,
}

/// Viewer for rendering each panel type
pub struct PanelViewer<'a> {
    pub context: PanelContext<'a>,
    pub touch_tracker: &'a mut TouchTracker,
}

impl<'a> TabViewer for PanelViewer<'a> {
    type Tab = PanelType;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.to_string().into()
    }

    fn is_closeable(&self, tab: &Self::Tab) -> bool {
        !matches!(tab, PanelType::FractalViewport)
    }

    fn tab_style_override(&self, tab: &Self::Tab, global_style: &egui_dock::TabStyle) -> Option<egui_dock::TabStyle> {
        if matches!(tab, PanelType::FractalViewport) {
            let mut style = global_style.clone();
            // Remove body border and inner margin so fractal fills the entire area
            style.tab_body.stroke = egui::Stroke::NONE;
            style.tab_body.inner_margin = egui::Margin::ZERO;
            Some(style)
        } else {
            None
        }
    }

    fn scroll_bars(&self, tab: &Self::Tab) -> [bool; 2] {
        if matches!(tab, PanelType::FractalViewport) {
            [false, false]
        } else if self.context.compact_mode {
            // In compact mode, disable egui_dock's vertical ScrollArea.
            // We wrap panel content in our own ScrollArea with AlwaysVisible
            // to prevent scrollbar oscillation (egui bug #1165).
            [true, false]
        } else {
            [true, true]
        }
    }

    fn clear_background(&self, tab: &Self::Tab) -> bool {
        !matches!(tab, PanelType::FractalViewport)
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        if self.context.compact_mode && !matches!(tab, PanelType::FractalViewport) {
            egui::ScrollArea::vertical()
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    self.render_panel(ui, tab);
                });
        } else {
            self.render_panel(ui, tab);
        }
    }

}

/// Pan the fractal view by a screen-pixel drag delta.
///
/// Free function so it can be called from outside `PanelViewer` —
/// the tab-bar cover Area in `ui::mod` forwards its drag input here,
/// using the leaf's full rect as `panel_size` so the drag scale stays
/// consistent whether the user is dragging in the body or in the
/// cover.
pub fn pan_fractal_view(
    config_manager: &mut crate::config::ConfigManager,
    drag_delta: egui::Vec2,
    panel_size: egui::Vec2,
) {
    let config = config_manager.active_config();

    if config.render_mode == crate::scene::transforms::RenderMode::Escape {
        escape_pan_view(config_manager, drag_delta, panel_size);
        return;
    }

    // Convert screen pixel delta to fractal space.
    // Use the smaller dimension for both axes so drag speed is consistent
    // regardless of landscape vs portrait orientation.
    let ref_size = panel_size.x.min(panel_size.y);
    let scale = 4.0 / (config.zoom * ref_size);
    let dx = -drag_delta.x * scale;
    let dy = -drag_delta.y * scale;

    // Screen space → pan frame (rotation-aware in 2D, identity in 3D)
    let (fractal_dx, fractal_dy) = config.screen_delta_to_pan_frame(dx, dy);

    let new_pan_x = config.pan_x + fractal_dx;
    let new_pan_y = config.pan_y + fractal_dy;

    let _ = config_manager.update_param(
        crate::config::ConfigPath::Pan,
        (new_pan_x, new_pan_y).into(),
    );
}

/// Zoom the fractal view via mouse-wheel scroll.
///
/// Free function so the tab-bar cover Area can forward its scroll
/// input here, passing the leaf's full rect/size so zoom-toward-cursor
/// stays anchored correctly whether the cursor is in the body or the
/// cover.
///
/// `zoom_to_cursor = false` (fly mode) always zooms to center: the
/// cursor-anchored pan adjustment fights the fly camera — the view
/// should stay locked to where the camera points, not drift toward
/// wherever the mouse happens to rest.
pub fn zoom_fractal_view(
    config_manager: &mut crate::config::ConfigManager,
    scroll_delta: f32,
    mouse_pos: Option<egui::Pos2>,
    panel_rect: egui::Rect,
    panel_size: egui::Vec2,
    zoom_to_cursor: bool,
) {
    let config = config_manager.active_config();

    if config.render_mode == crate::scene::transforms::RenderMode::Escape {
        escape_zoom_view(config_manager, scroll_delta, mouse_pos, panel_rect, panel_size, zoom_to_cursor);
        return;
    }

    // Use power-based zoom for smooth scrolling (matches original code)
    let zoom_factor = if scroll_delta.abs() > 0.1 {
        1.1f32.powf(scroll_delta * 0.03)
    } else {
        1.0
    };

    if zoom_factor != 1.0 {
        // Zoom in toward cursor, zoom out from center
        if zoom_factor > 1.0 {
            // Zooming in - zoom toward mouse cursor position
            if let Some(mouse_pos) = mouse_pos.filter(|_| zoom_to_cursor) {
                // Convert mouse position from panel space to fractal space
                // Panel center
                let center_x = panel_rect.center().x;
                let center_y = panel_rect.center().y;

                // Mouse offset from center in panel pixels
                let mouse_offset_x = mouse_pos.x - center_x;
                let mouse_offset_y = mouse_pos.y - center_y;

                // Convert to fractal space (account for current zoom, scale, and rotation)
                let scale = f32::min(panel_size.x, panel_size.y) * 0.25;

                // Screen space → pan frame (rotation-aware in 2D,
                // identity in 3D). Same offset serves both zoom
                // levels — the conversion doesn't depend on zoom.
                let (rotated_offset_x, rotated_offset_y) =
                    config.screen_delta_to_pan_frame(mouse_offset_x, mouse_offset_y);

                let fractal_offset_x = rotated_offset_x / (scale * config.zoom);
                let fractal_offset_y = rotated_offset_y / (scale * config.zoom);

                // Calculate the point in fractal space that the mouse is pointing at
                let point_x = config.pan_x + fractal_offset_x;
                let point_y = config.pan_y + fractal_offset_y;

                // Apply zoom and adjust pan so that point stays under the cursor
                let new_zoom = (config.zoom * zoom_factor).clamp(0.01, 1000.0);
                let new_fractal_offset_x = rotated_offset_x / (scale * new_zoom);
                let new_fractal_offset_y = rotated_offset_y / (scale * new_zoom);
                let new_pan_x = point_x - new_fractal_offset_x;
                let new_pan_y = point_y - new_fractal_offset_y;

                // Update zoom and pan atomically
                let _ = config_manager.update_batch(
                    vec![
                        (crate::config::ConfigPath::Zoom, new_zoom.into()),
                        (crate::config::ConfigPath::Pan, (new_pan_x, new_pan_y).into()),
                    ],
                    "history.action.wheel_zoom".to_string(),
                );
            } else {
                // No mouse position, zoom to center
                let new_zoom = (config.zoom * zoom_factor).clamp(0.01, 1000.0);
                let _ = config_manager.update_param(
                    crate::config::ConfigPath::Zoom,
                    new_zoom.into(),
                );
            }
        } else {
            // Zooming out - always zoom from center
            let new_zoom = (config.zoom * zoom_factor).clamp(0.01, 1000.0);
            let _ = config_manager.update_param(
                crate::config::ConfigPath::Zoom,
                new_zoom.into(),
            );
        }
    }
}

/// Escape-mode complex-plane geometry shared by pan and zoom below.
///
/// The escape shader maps the viewport as: vertical span `4 / 2^zoom`
/// across `height` pixels (horizontal follows aspect with the SAME
/// per-pixel scale), screen y flipped (Im grows up), then the view
/// rotation. So one pixel is `span_y / height` complex units in every
/// direction, and a screen offset becomes a world offset via y-flip +
/// rotation. Done in f64 from the exact-decimal center strings — the
/// phase-1 precision ceiling (f64 formatting round-trips shortest, so
/// writing back never loses what f64 held).
fn escape_screen_to_world(
    esc: &crate::config::escape::EscapeConfig,
    dx_px: f64,
    dy_px: f64,
    panel_size: egui::Vec2,
) -> (f64, f64) {
    let height = f64::from(panel_size.y.max(1.0));
    let per_pixel = (4.0 / esc.zoom_factor()) / height;
    let (dx, dy) = (dx_px * per_pixel, -dy_px * per_pixel);
    let (cos_r, sin_r) = (f64::from(esc.rotation).cos(), f64::from(esc.rotation).sin());
    (dx * cos_r - dy * sin_r, dx * sin_r + dy * cos_r)
}

/// Pan the escape view: the image follows the cursor, so the center
/// moves opposite the drag. One batch → one undo point per coalesced
/// gesture, same as flame pan.
fn escape_pan_view(
    config_manager: &mut crate::config::ConfigManager,
    drag_delta: egui::Vec2,
    panel_size: egui::Vec2,
) {
    let esc = config_manager.active_config().escape.clone();
    let (cx, cy) = esc.center_f64();
    let (wx, wy) = escape_screen_to_world(&esc, f64::from(drag_delta.x), f64::from(drag_delta.y), panel_size);
    let _ = config_manager.update_batch(
        vec![
            (crate::config::ConfigPath::EscapeCenterRe, crate::config::ConfigValue::String(format!("{}", cx - wx))),
            (crate::config::ConfigPath::EscapeCenterIm, crate::config::ConfigValue::String(format!("{}", cy - wy))),
        ],
        "history.param.escape_center_re".to_string(),
    );
}

/// Wheel zoom for the escape view: zoom-in anchors to the cursor
/// (the point under it stays put), zoom-out recedes from center —
/// the same feel as the flame viewport.
fn escape_zoom_view(
    config_manager: &mut crate::config::ConfigManager,
    scroll_delta: f32,
    mouse_pos: Option<egui::Pos2>,
    panel_rect: egui::Rect,
    panel_size: egui::Vec2,
    zoom_to_cursor: bool,
) {
    let esc = config_manager.active_config().escape.clone();

    let zoom_factor = if scroll_delta.abs() > 0.1 {
        f64::from(1.1f32).powf(f64::from(scroll_delta) * 0.03)
    } else {
        return;
    };
    // f32 travel ceiling for the ConfigValue::Float leg; the stored
    // field is f64 and phase 4 lifts the range with perturbation.
    let new_zoom_log2 = (esc.zoom_log2 + zoom_factor.log2()).clamp(-8.0, 300.0);

    let mut updates = vec![(
        crate::config::ConfigPath::EscapeZoomLog2,
        crate::config::ConfigValue::Float(new_zoom_log2 as f32),
    )];

    if zoom_factor > 1.0 {
        if let Some(mouse_pos) = mouse_pos.filter(|_| zoom_to_cursor) {
            // Keep the point under the cursor fixed: with the offset o
            // (screen → world) and scale ratio k = old/new span,
            // center' = center + o·(1 − 1/k) — computed here as the
            // difference of the offset at the two spans.
            let off_x = f64::from(mouse_pos.x - panel_rect.center().x);
            let off_y = f64::from(mouse_pos.y - panel_rect.center().y);
            let (cx, cy) = esc.center_f64();
            let (wx_old, wy_old) = escape_screen_to_world(&esc, off_x, off_y, panel_size);
            let shrink = esc.zoom_factor() / f64::exp2(new_zoom_log2);
            let (wx_new, wy_new) = (wx_old * shrink, wy_old * shrink);
            updates.push((
                crate::config::ConfigPath::EscapeCenterRe,
                crate::config::ConfigValue::String(format!("{}", cx + (wx_old - wx_new))),
            ));
            updates.push((
                crate::config::ConfigPath::EscapeCenterIm,
                crate::config::ConfigValue::String(format!("{}", cy + (wy_old - wy_new))),
            ));
        }
    }

    let _ = config_manager.update_batch(updates, "history.action.wheel_zoom".to_string());
}

impl<'a> PanelViewer<'a> {
    fn render_panel(&mut self, ui: &mut egui::Ui, tab: &mut PanelType) {
        // Escape mode hides the flame-only editing panels rather than
        // teaching them a second vocabulary (plan §3). Shared-tail
        // panels (Colors, Palette, Effects, History, Animation,
        // Export, ...) keep working — they edit state escape mode
        // actually consumes.
        if self.context.config_manager.active_config().render_mode
            == crate::scene::transforms::RenderMode::Escape
            && matches!(
                tab,
                PanelType::Transforms
                    | PanelType::TriangleEditor
                    | PanelType::View
                    | PanelType::XaosEditor
                    | PanelType::Variations
                    | PanelType::Subflames
                    | PanelType::SolidLighting
                    | PanelType::PathEditor
            )
        {
            ui.label(t!("escape_panel.flame_only_hint"));
            return;
        }
        match tab {
            PanelType::FractalViewport => {
                self.render_fractal_viewport(ui);
            }
            PanelType::Transforms => {
                self.render_transforms_panel(ui);
            }
            PanelType::TriangleEditor => {
                self.render_triangle_editor_panel(ui);
            }
            PanelType::Colors => {
                self.render_colors_panel(ui);
            }
            PanelType::PaletteEditor => {
                self.render_palette_editor_panel(ui);
            }
            PanelType::PaletteLibrary => {
                self.render_palette_library_panel(ui);
            }
            PanelType::View => {
                self.render_view_panel(ui);
            }
            PanelType::Rendering => {
                self.render_rendering_panel(ui);
            }
            PanelType::SolidLighting => {
                self.render_solid_lighting_panel(ui);
            }
            PanelType::History => {
                self.render_history_panel(ui);
            }
            PanelType::Animation => {
                self.render_animation_panel(ui);
            }
            PanelType::Performance => {
                self.render_performance_panel(ui);
            }
            PanelType::Help => {
                self.render_help_panel(ui);
            }
            PanelType::KeyboardShortcuts => {
                self.render_keyboard_shortcuts_panel(ui);
            }
            PanelType::ConfigDialog => {
                self.render_config_dialog_panel(ui);
            }
            PanelType::FractalBrowser => {
                self.render_fractal_browser_panel(ui);
            }
            PanelType::PathEditor => {
                self.render_path_editor_panel(ui);
            }
            PanelType::Export => {
                self.render_export_panel(ui);
            }
            PanelType::RandomGenerator => {
                self.render_random_generator_panel(ui);
            }
            PanelType::Effects => {
                self.render_effects_panel(ui);
            }
            PanelType::XaosEditor => {
                self.render_xaos_editor_panel(ui);
            }
            PanelType::Signal => {
                self.render_signal_panel(ui);
            }
            PanelType::LoginDialog => {
                self.render_login_dialog(ui);
            }
            PanelType::SaveOnlineDialog => {
                self.render_save_online_dialog(ui);
            }
            PanelType::Variations => {
                let response = super::variations::render_variations_panel(
                    ui,
                    self.context.flame,
                    self.context.variation_catalog,
                );
                if response.clear_cache_requested {
                    *self.context.clear_variation_cache_requested = true;
                }
                if !response.update_requested.is_empty() {
                    *self.context.variation_update_requested = response.update_requested;
                }
            }
            PanelType::Scripts => {
                self.render_scripts_panel(ui);
            }
            PanelType::Escape => {
                super::escape_panel::render_escape_content(ui, self.context.config_manager);
            }
            PanelType::Subflames => {
                super::subflames::render_subflames_content(
                    ui,
                    self.context.config_manager,
                    self.context.view_subflame_in_isolation,
                    self.context.load_subflame_into,
                );
            }
        }
    }
    /// Render Transforms panel (transform list, affine, variations).
    /// Passes the target flame slice based on the active editing
    /// target: Main → App.flame (parent), Subflame{i} →
    /// App.flame.subflames[i]. ConfigManager handles the mirroring
    /// for writes via `target_flame_mut` routed by editing_target.
    fn render_transforms_panel(&mut self, ui: &mut egui::Ui) {
        let target_flame = active_target_flame_mut(
            self.context.flame,
            self.context.config_manager.editing_target(),
        );
        let _ = super::transforms::render_transforms_content(
            ui,
            self.context.config_manager,
            target_flame,
            super::transforms::PoolActions {
                add_normal: self.context.add_transform,
                delete_normal: self.context.delete_transform,
                clone_normal: self.context.clone_transform,
                add_linked: self.context.add_linked_transform,
                delete_linked: self.context.delete_linked_transform,
                clone_linked: self.context.clone_linked_transform,
                add_final: self.context.add_final_transform,
                delete_final: self.context.delete_final_transform,
                clone_final: self.context.clone_final_transform,
                attachment_edit: self.context.attachment_edit,
            },
            self.context.open_triangle_editor,
        );
    }

    /// Render Triangle Editor panel (visual triangle editing). Sees
    /// the active editing target's flame, same as Transforms.
    fn render_triangle_editor_panel(&mut self, ui: &mut egui::Ui) {
        let target_flame = active_target_flame_mut(
            self.context.flame,
            self.context.config_manager.editing_target(),
        );
        let _ = super::triangle_editor::render_triangle_editor_content(
            ui,
            self.context.config_manager,
            target_flame,
        );
    }

    /// Render Colors panel (color mode, palette, tone mapping)
    fn render_colors_panel(&mut self, ui: &mut egui::Ui) {
        let _ = super::tone_mapping::render_colors_content(
            ui,
            self.context.config_manager,
            self.context.palette_library,
            self.context.open_palette_editor,
            self.context.open_palette_library,
            self.context.density_histogram,
        );
    }

    /// Render Palette Editor panel (palette editing)
    fn render_palette_editor_panel(&mut self, ui: &mut egui::Ui) {
        // Capture palette_library reference for the closure
        let palette_library = &self.context.palette_library;
        // A generating script may still call set_palette / random_palette.
        let palettes: Vec<crate::scene::palette::Palette> =
            palette_library.iter().cloned().collect();
        super::palette_editor::render_palette_editor_content(
            ui,
            self.context.palette_editor,
            self.context.config_manager,
            self.context.palette_export_json,
            self.context.palette_save_file,
            self.context.palette_save_to_library,
            self.context.palette_delete_from_library,
            self.context.palette_import_json,
            self.context.palette_load_file,
            self.context.open_palette_library,
            &palettes,
            |name| palette_library.has_custom_palette_named(name),
        );
    }

    /// Render Palette Library panel (browse and manage palette packs)
    fn render_palette_library_panel(&mut self, ui: &mut egui::Ui) {
        super::palette_library::render_palette_library(
            ui,
            self.context.palette_library,
            self.context.config_manager,
            self.context.open_palette_editor,
        );

        let (online_mode, auth_pair) = {
            let settings = self.context.config_manager.system_settings();
            let online = settings.online_mode;
            let auth = if settings.is_signed_in() {
                let token = settings.auth_token.clone().unwrap_or_default();
                Some((crate::api::API_BASE_URL.to_string(), token))
            } else {
                None
            };
            (online, auth)
        };
        if online_mode {
            let auth = auth_pair.as_ref().map(|(b, t)| (b.as_str(), t.as_str()));
            super::palette_library::render_cloud_palettes_section(
                ui,
                self.context.cloud_palette_state,
                self.context.config_manager,
                auth,
            );
        }
    }

    /// Render the View panel (zoom, pan, rotation)
    fn render_view_panel(&mut self, ui: &mut egui::Ui) {
        super::view::render_view_content(
            ui,
            self.context.config_manager,
            self.context.flame,
            self.context.fly_mode_active,
            self.context.fly_mode_toggle_requested,
        );
    }

    /// Render the Rendering panel (iterations, accumulation)
    fn render_rendering_panel(&mut self, ui: &mut egui::Ui) {
        super::settings::render_settings_content(
            ui,
            self.context.flame_renderer,
            self.context.paused,
            self.context.config_manager,
        );
    }

    /// Render the Solid Rendering & Lighting panel
    fn render_solid_lighting_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                super::solid_panel::render_solid_panel_content(
                    ui,
                    self.context.config_manager,
                );
            });
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

    /// Render Animation panel (playback controls, timeline)
    fn render_animation_panel(&mut self, ui: &mut egui::Ui) {
        // Ensure animation always exists (animation is always present with 0 tracks by default)
        if self.context.animation_controller.animation.is_none() {
            let new_anim = crate::animation::Animation::new("New Animation".to_string(), 10.0);
            self.context.animation_controller.load(new_anim);
        }

        let mut response = super::animation_panel::render_animation_content(
            ui,
            self.context.animation_controller,
            self.context.animation_export_settings,
        );

        // Handle timeline scrubbing (from render_animation_content)
        if response.seek_changed {
            *self.context.animation_seek_changed = true;
        }
        if response.seek_drag_stopped {
            *self.context.animation_seek_drag_stopped = true;
        }

        // Track editor section with visual bars aligned to timeline
        ui.separator();
        let track_response = super::track_editor::render_track_editor(
            ui,
            self.context.animation_controller,
            self.context.track_editor_state,
            self.context.config_manager.active_config(),
            response.timeline_layout,
        );

        // Handle seek from clicking on track bars (Phase 3) - discrete action, needs reset
        if let Some(time) = track_response.seek_to_time {
            self.context.animation_controller.seek(time);
            *self.context.animation_seek_changed = true;
            *self.context.animation_seek_drag_stopped = true;
        }

        // Live export progress is shown by the global overlay
        // (`export_status::render_export_overlay`), not inline here.

        // File controls (after tracks section)
        ui.separator();
        let api_context = super::animation_panel::AnimationApiContext {
            api_flame_id: self.context.api_flame_id,
            api_animation_id: self.context.api_animation_id,
            flame_animations: self.context.flame_animations,
            is_signed_in: self.context.is_signed_in,
        };
        super::animation_panel::render_file_controls(
            ui,
            self.context.animation_controller,
            &mut response,
            Some(&api_context),
            #[cfg(not(target_arch = "wasm32"))]
            self.context.window,
        );

        // Open Save Online dialog from animation panel
        if response.open_save_online_dialog {
            *self.context.open_save_online_dialog = true;
        }

        // Load an animation from the API (clicked in the animations list)
        if let Some(animation_id) = response.load_api_animation_id {
            *self.context.load_api_animation_id = Some(animation_id);
        }

        // Handle file control responses (must be after render_file_controls)
        // Handle open export panel request (Phase 5)
        if response.open_export_panel {
            self.context.export_panel_state.is_open = true;
            // Auto-fill export settings from current config
            self.context.animation_export_settings.iterations_per_thread =
                self.context.config_manager.system_settings().iterations_per_thread;
            self.context.animation_export_settings.max_iterations =
                self.context.config_manager.active_config().max_iterations;
        }

        // Handle animation load response
        if let Some(animation) = response.load_animation.take() {
            // If animation has embedded config, load it via selected_preset_config
            // This ensures proper GPU sync and undo/redo handling
            if let Some(config) = animation.base_config.clone() {
                log::info!("Animation '{}' has embedded config, loading it", animation.name);
                *self.context.selected_preset_config = Some(config);
            }
            let generators = animation.generators.clone();
            let duration = animation.duration;
            self.context.animation_controller.load(animation);
            // The animation's embedded config (if any) is stashed in
            // selected_preset_config above to be applied later by
            // handle_preset_selection. We bind now against whatever
            // active_config currently is — but the load_config_with_undo
            // hook will re-bind once the preset is actually applied,
            // so the final state is correct either way.
            if let Some(anim) = self.context.animation_controller.animation.as_mut() {
                anim.bind_to_config(self.context.config_manager.active_config());
            }
            self.context.signal_panel_state.restore_generators(
                generators, self.context.signal_manager, duration,
            );
        }

        // Handle animation load trigger (WASM only - uses native file picker)
        #[cfg(target_arch = "wasm32")]
        if response.trigger_animation_load {
            let ctx = ui.ctx().clone();
            crate::app::trigger_browser_file_picker(".anim,.json", ctx, "pending_animation_load_raw");
        }

        // Handle animation save response
        if response.save_animation {
            // Sync generators from panel state → animation before saving
            if let Some(ref mut animation) = self.context.animation_controller.animation {
                animation.generators = self.context.signal_panel_state.generators.clone();
            }

            #[cfg(not(target_arch = "wasm32"))]
            if let Some(ref animation) = self.context.animation_controller.animation {
                // Clone animation and embed current config
                let mut animation_with_config = animation.clone();
                animation_with_config.set_base_config(self.context.config_manager.active_config().clone());

                if let Some(path) = rfd::FileDialog::new()
                    .set_parent(self.context.window)
                    .add_filter("Animation", &["anim", "json"])
                    .set_file_name(&format!("{}.anim", animation_with_config.name))
                    .save_file()
                {
                    match animation_with_config.to_json() {
                        Ok(json) => {
                            if let Err(e) = std::fs::write(&path, json) {
                                log::error!("Failed to save animation: {}", e);
                            } else {
                                log::info!("Saved animation with embedded config to {:?}", path);
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to serialize animation: {}", e);
                        }
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            if let Some(ref animation) = self.context.animation_controller.animation {
                // Clone animation and embed current config
                let mut animation_with_config = animation.clone();
                animation_with_config.set_base_config(self.context.config_manager.active_config().clone());

                match animation_with_config.to_json() {
                    Ok(json) => {
                        let filename = format!("{}.anim", animation_with_config.name.to_lowercase().replace(' ', "_"));
                        if let Err(e) = crate::app::trigger_browser_download(json.as_bytes(), &filename, "application/json") {
                            log::error!("Failed to trigger animation download: {}", e);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to serialize animation: {}", e);
                    }
                }
            }
        }

        // Handle animation export request
        if let Some(settings) = response.export_animation.take() {
            *self.context.animation_export_requested = Some(settings);
        }
    }

    /// Render the Performance panel (stats and version info)
    fn render_performance_panel(&mut self, ui: &mut egui::Ui) {
        super::performance::render_performance_content(
            ui,
            self.context.metrics,
            self.context.window_size,
            self.context.flame_renderer,
        );
    }

    /// Render Help panel (intro and links)
    fn render_help_panel(&mut self, ui: &mut egui::Ui) {
        super::help::render_help_panel_content(
            ui,
            self.context.config_manager,
            self.context.open_preset_library,
            self.context.open_random_generator,
        );
    }

    /// Render Keyboard Shortcuts panel
    fn render_keyboard_shortcuts_panel(&mut self, ui: &mut egui::Ui) {
        super::help::render_keyboard_shortcuts_content(ui);
    }

    /// Render Config Dialog panel (import/export configuration)
    fn render_config_dialog_panel(&mut self, ui: &mut egui::Ui) {
        super::config_dialog::render_config_dialog_content(
            ui,
            self.context.config_json_buffer,
            self.context.config_export_json,
            self.context.config_import_json,
            self.context.config_save_file,
            self.context.config_load_file,
            self.context.flame_xml_import_file,
            self.context.flame_xml_export_file,
        );
    }

    /// Render Fractal Viewport (main fractal rendering area)
    fn render_fractal_viewport(&mut self, ui: &mut egui::Ui) {
        // Set panel background to match fractal background color
        // This allows the fractal texture to have transparent edges that blend naturally
        let bg_color = self.context.config_manager.active_config().background_color;
        let bg_color32 = egui::Color32::from_rgb(
            (bg_color[0] * 255.0) as u8,
            (bg_color[1] * 255.0) as u8,
            (bg_color[2] * 255.0) as u8,
        );

        // Override the panel's background color
        ui.visuals_mut().panel_fill = bg_color32;

        if let Some(texture_id) = self.context.fractal_texture_id {
            // Get the actual panel size and report it for texture sizing
            let available_size = ui.available_size();
            let tab_bar_h = self.context.viewport_tab_bar_height;
            // Inflate texture height to include the tab bar area so the fractal
            // covers the entire node seamlessly (the tab bar cover draws the top slice)
            let total_height = available_size.y + tab_bar_h;
            let width = available_size.x.max(1.0) as u32;
            let height = total_height.max(1.0) as u32;

            // Report the inflated size so the GPU renders a taller texture
            *self.context.fractal_viewport_size = Some((width, height));

            // Display the fractal texture with UV offset to skip the top portion
            // (which is drawn separately over the tab bar by the cover Area)
            let uv_top = if total_height > 0.0 { tab_bar_h / total_height } else { 0.0 };
            let image = egui::Image::new(egui::load::SizedTexture::new(texture_id, available_size))
                .uv(egui::Rect::from_min_max(
                    egui::pos2(0.0, uv_top),
                    egui::pos2(1.0, 1.0),
                ))
                .fit_to_exact_size(available_size)
                .maintain_aspect_ratio(false) // Fill entire panel
                .sense(egui::Sense::click_and_drag()); // Enable drag interaction

            let response = ui.add(image);

            // Handle pinch-to-zoom on touchscreens (check before drag to avoid conflicts)
            // Uses custom TouchTracker because egui's multi_touch() doesn't work on web
            // (winit assigns different TouchDeviceId per finger, so egui never sees 2 fingers)
            // Gate new touches on response.hovered() which is layer-aware — returns false
            // when a floating panel is on top of the viewport.
            let accept_new_touches = response.hovered();
            let touch_gesture = ui.input(|i| {
                let touch_events: Vec<egui::Event> = i.events.iter()
                    .filter(|e| match e {
                        egui::Event::Touch { phase: egui::TouchPhase::Start, .. } =>
                            accept_new_touches,
                        // Accept Move/End/Cancel only for touches we're already tracking
                        egui::Event::Touch { id, .. } =>
                            self.touch_tracker.is_tracking(id.0),
                        _ => false,
                    })
                    .cloned()
                    .collect();
                self.touch_tracker.update(&touch_events)
            });
            let touch_active = self.touch_tracker.is_touch_active();

            match touch_gesture {
                Some(TouchGesture::Pan(delta)) => {
                    self.handle_fractal_drag(delta, available_size, false);
                }
                Some(TouchGesture::Pinch { zoom_delta, translation, midpoint }) => {
                    self.handle_fractal_pinch_zoom(zoom_delta, translation, midpoint, response.rect, available_size);
                }
                None => {}
            }

            // Handle mouse drag: pans, or looks (pitch/yaw) while Alt is
            // held — like fly-mode mouse-look. Skipped during touch to
            // avoid double-handling.
            if !touch_active && response.dragged_by(egui::PointerButton::Primary) {
                let drag_delta = response.drag_delta();
                let alt = ui.input(|i| i.modifiers.alt);
                self.handle_fractal_drag(drag_delta, available_size, alt);
            }

            // Handle mouse wheel for zooming
            if response.hovered() {
                let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll_delta.abs() > 0.1 {
                    self.handle_fractal_scroll(scroll_delta, response.hover_pos(), response.rect, available_size);
                }
            }

            // Handle right-click (or drag with right button held) to query path at pixel (PathMap mode)
            // Use down() to detect when button is held, allowing continuous updates while dragging
            let secondary_held = ui.input(|i| i.pointer.secondary_down());
            if secondary_held && response.hovered() {
                if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    // Convert from panel coordinates to texture coordinates
                    let local_x = pointer_pos.x - response.rect.min.x;
                    let local_y = pointer_pos.y - response.rect.min.y;
                    let pixel_x = (local_x / available_size.x * width as f32) as u32;
                    let pixel_y = (local_y / available_size.y * height as f32) as u32;
                    *self.context.hovered_pixel = Some((pixel_x.min(width - 1), pixel_y.min(height - 1)));
                }
            }

            // Display path info overlay when available (PathMap mode only)
            let is_path_map_mode = self.context.config_manager.active_config().color_mode
                == crate::scene::palette::ColorMode::PathMap;
            if is_path_map_mode {
                if let Some(click_info) = self.context.path_click_info {
                    self.render_path_overlay(ui, &response, click_info);
                }
            }
        } else {
            // Fallback if texture not available yet
            ui.centered_and_justified(|ui| {
                ui.label(t!("viewport.initializing"));
            });
        }
    }

    /// Handle a fractal drag. It becomes a camera look-around (pitch/
    /// yaw) instead of a pan when fly mode is active OR `look` is set
    /// (viewport Alt+drag) — recorded into `fly_mouse_drag` for the App
    /// to consume after the UI render, identical to fly-mode mouse-look.
    /// Otherwise the drag pans.
    fn handle_fractal_drag(&mut self, drag_delta: egui::Vec2, panel_size: egui::Vec2, look: bool) {
        if self.context.fly_mode_active || look {
            let prev = self.context.fly_mouse_drag.unwrap_or((0.0, 0.0));
            *self.context.fly_mouse_drag = Some((prev.0 + drag_delta.x, prev.1 + drag_delta.y));
        } else {
            pan_fractal_view(self.context.config_manager, drag_delta, panel_size);
        }
    }

    /// Handle fractal zooming via mouse wheel
    fn handle_fractal_scroll(
        &mut self,
        scroll_delta: f32,
        mouse_pos: Option<egui::Pos2>,
        panel_rect: egui::Rect,
        panel_size: egui::Vec2,
    ) {
        zoom_fractal_view(
            self.context.config_manager,
            scroll_delta,
            mouse_pos,
            panel_rect,
            panel_size,
            // In fly mode, zoom to center — cursor-anchored pan
            // adjustments fight the camera.
            !self.context.fly_mode_active,
        );
    }

    fn handle_fractal_pinch_zoom(
        &mut self,
        zoom_delta: f32,
        translation: egui::Vec2,
        pinch_center: egui::Pos2,
        panel_rect: egui::Rect,
        panel_size: egui::Vec2,
    ) {
        let config = self.context.config_manager.active_config();

        // Escape mode: pinch = zoom anchored at the finger midpoint
        // plus the two-finger translation as a pan, expressed in the
        // escape view's own center/zoom_log2 vocabulary.
        if config.render_mode == crate::scene::transforms::RenderMode::Escape {
            let esc = config.escape.clone();
            let mut updates = Vec::new();
            let (mut cx, mut cy) = esc.center_f64();
            if zoom_delta != 1.0 {
                let new_zoom_log2 =
                    (esc.zoom_log2 + f64::from(zoom_delta).log2()).clamp(-8.0, 300.0);
                let off_x = f64::from(pinch_center.x - panel_rect.center().x);
                let off_y = f64::from(pinch_center.y - panel_rect.center().y);
                let (wx_old, wy_old) = escape_screen_to_world(&esc, off_x, off_y, panel_size);
                let shrink = esc.zoom_factor() / f64::exp2(new_zoom_log2);
                cx += wx_old * (1.0 - shrink);
                cy += wy_old * (1.0 - shrink);
                updates.push((
                    crate::config::ConfigPath::EscapeZoomLog2,
                    crate::config::ConfigValue::Float(new_zoom_log2 as f32),
                ));
            }
            if translation != egui::Vec2::ZERO {
                let (wx, wy) = escape_screen_to_world(
                    &esc,
                    f64::from(translation.x),
                    f64::from(translation.y),
                    panel_size,
                );
                cx -= wx;
                cy -= wy;
            }
            if zoom_delta != 1.0 || translation != egui::Vec2::ZERO {
                updates.push((
                    crate::config::ConfigPath::EscapeCenterRe,
                    crate::config::ConfigValue::String(format!("{}", cx)),
                ));
                updates.push((
                    crate::config::ConfigPath::EscapeCenterIm,
                    crate::config::ConfigValue::String(format!("{}", cy)),
                ));
                let _ = self.context.config_manager.update_batch(
                    updates,
                    "history.action.wheel_zoom".to_string(),
                );
            }
            return;
        }

        let new_zoom = (config.zoom * zoom_delta).clamp(0.01, 1000.0);

        // Start with current pan, then apply zoom-toward-center adjustment
        let mut new_pan_x = config.pan_x;
        let mut new_pan_y = config.pan_y;

        if zoom_delta != 1.0 {
            // Zoom toward the midpoint between the two fingers
            let center_x = panel_rect.center().x;
            let center_y = panel_rect.center().y;
            let offset_x = pinch_center.x - center_x;
            let offset_y = pinch_center.y - center_y;

            let scale = f32::min(panel_size.x, panel_size.y) * 0.25;
            // Screen space → pan frame (rotation-aware in 2D, identity in 3D)
            let (rot_x, rot_y) = config.screen_delta_to_pan_frame(offset_x, offset_y);

            let point_x = config.pan_x + rot_x / (scale * config.zoom);
            let point_y = config.pan_y + rot_y / (scale * config.zoom);

            new_pan_x = point_x - rot_x / (scale * new_zoom);
            new_pan_y = point_y - rot_y / (scale * new_zoom);
        }

        // Apply two-finger translation on top of the zoom pan adjustment
        if translation != egui::Vec2::ZERO {
            let ref_size = panel_size.x.min(panel_size.y);
            let drag_scale = 4.0 / (new_zoom * ref_size);
            let dx = -translation.x * drag_scale;
            let dy = -translation.y * drag_scale;

            let (pan_dx, pan_dy) = config.screen_delta_to_pan_frame(dx, dy);
            new_pan_x += pan_dx;
            new_pan_y += pan_dy;
        }

        // Single batch update: zoom + combined pan = one history entry
        let _ = self.context.config_manager.update_batch(
            vec![
                (crate::config::ConfigPath::Zoom, new_zoom.into()),
                (crate::config::ConfigPath::Pan, (new_pan_x, new_pan_y).into()),
            ],
            "history.action.pinch_zoom".to_string()
        );
    }

    /// Render path overlay showing pixel info, coordinates, path, and color preview
    fn render_path_overlay(
        &mut self,
        ui: &mut egui::Ui,
        _image_response: &egui::Response,
        click_info: &super::PathClickInfo,
    ) {
        // Get transform names for display
        let flame = &self.context.config_manager.active_config().flame;
        let transform_count = flame.transforms.len();

        // Build path string
        let path_vec = click_info.path_entry.to_vec();

        // Format path: show transform indices and names
        let path_str: Vec<String> = path_vec.iter().map(|&idx| {
            let idx = idx as usize;
            if idx < transform_count {
                format!("T{}", idx)
            } else {
                format!("?{}", idx)
            }
        }).collect();

        // Create overlay window anchored to top-left of viewport
        egui::Area::new(egui::Id::new("path_overlay"))
            .fixed_pos(ui.min_rect().min + egui::vec2(10.0, 10.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 220))
                    .inner_margin(egui::Margin::same(10))
                    .corner_radius(egui::CornerRadius::same(6))
                    .show(ui, |ui| {
                        ui.set_max_width(420.0);

                        // Header row with close button
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(t!("path_overlay.title")).strong().color(egui::Color32::WHITE));

                            // Push close button to the right
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("X").clicked() {
                                    *self.context.close_path_overlay = true;
                                }
                            });
                        });

                        ui.add_space(6.0);

                        // Two-column layout: info on left, color preview on right
                        ui.horizontal(|ui| {
                            // Left column: coordinates and path info
                            ui.vertical(|ui| {
                                ui.set_min_width(280.0);

                                // Pixel coordinates section
                                ui.label(egui::RichText::new(t!("path_overlay.coordinates")).strong().color(egui::Color32::LIGHT_GRAY));
                                ui.add_space(2.0);

                                // View space (pixel) coordinates
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.pixel")).color(egui::Color32::GRAY));
                                    ui.label(egui::RichText::new(format!("({}, {})",
                                        click_info.found_pixel.0, click_info.found_pixel.1))
                                        .color(egui::Color32::WHITE));
                                    if click_info.search_distance > 0.0 {
                                        ui.label(egui::RichText::new(t!("path_overlay.pixel_offset", distance = format!("{:.1}", click_info.search_distance)))
                                            .small()
                                            .color(egui::Color32::YELLOW));
                                    }
                                });

                                // Fractal space coordinates
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.fractal")).color(egui::Color32::GRAY));
                                    ui.label(egui::RichText::new(format!("({:.6}, {:.6})",
                                        click_info.fractal_coords.0, click_info.fractal_coords.1))
                                        .color(egui::Color32::LIGHT_GREEN));
                                });

                                // IFS starting point
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.ifs_start")).color(egui::Color32::GRAY));
                                    ui.label(egui::RichText::new(format!("({:.4}, {:.4})",
                                        click_info.path_entry.initial_x, click_info.path_entry.initial_y))
                                        .color(egui::Color32::LIGHT_BLUE));
                                });

                                ui.add_space(6.0);

                                // Path section
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.path_label")).strong().color(egui::Color32::LIGHT_GRAY));
                                    ui.label(egui::RichText::new(t!("path_overlay.path_iterations", count = click_info.path_entry.iteration_count))
                                        .small()
                                        .color(egui::Color32::GRAY));
                                });
                                ui.add_space(2.0);

                                // Wrap path in a scrollable area if it's long
                                if !path_str.is_empty() {
                                    egui::ScrollArea::horizontal().max_width(260.0).show(ui, |ui| {
                                        ui.horizontal_wrapped(|ui| {
                                            for (i, name) in path_str.iter().enumerate() {
                                                if i > 0 {
                                                    ui.label(egui::RichText::new(">").color(egui::Color32::DARK_GRAY));
                                                }
                                                ui.label(egui::RichText::new(name).color(egui::Color32::from_rgb(100, 180, 255)));
                                            }
                                        });
                                    });
                                } else {
                                    ui.label(egui::RichText::new(t!("path_overlay.path_empty")).color(egui::Color32::GRAY));
                                }

                                ui.add_space(6.0);

                                // Hash debug info (shows Prefix Distinct calculation)
                                use crate::renderer::PathEntry;
                                let prefix = click_info.path_entry.get_prefix();
                                let iter_count = click_info.path_entry.iteration_count;
                                // Mix iteration_count into value before hashing (matches GPU)
                                let mixed = prefix ^ (iter_count.wrapping_mul(0x9E3779B9));
                                let hash = PathEntry::scramble_hash(mixed);
                                let hue = click_info.path_entry.compute_prefix_distinct_hue();
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.debug_prefix_distinct")).strong().color(egui::Color32::LIGHT_GRAY));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.debug_path0", value = format!("{:08X}", prefix)))
                                        .small()
                                        .color(egui::Color32::YELLOW));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.debug_mixed", value = format!("{:08X}", mixed)))
                                        .small()
                                        .color(egui::Color32::YELLOW));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.debug_hash", value = format!("{:08X}", hash)))
                                        .small()
                                        .color(egui::Color32::YELLOW));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(t!("path_overlay.debug_hue", value = format!("{:.6}", hue)))
                                        .small()
                                        .color(egui::Color32::YELLOW));
                                });
                            });

                            ui.add_space(12.0);

                            // Right column: 9x9 color preview (clickable)
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(t!("path_overlay.preview_title")).strong().color(egui::Color32::LIGHT_GRAY));
                                ui.add_space(4.0);

                                // Render color grid
                                let (preview_w, preview_h) = click_info.preview_size;
                                let pixel_size = 12.0;
                                let total_size = egui::vec2(
                                    preview_w as f32 * pixel_size,
                                    preview_h as f32 * pixel_size,
                                );

                                // Make clickable to allow selecting other pixels
                                let (rect, response) = ui.allocate_exact_size(total_size, egui::Sense::click());
                                let painter = ui.painter();

                                // Handle click on preview to select a different pixel
                                if response.clicked() {
                                    if let Some(click_pos) = response.interact_pointer_pos() {
                                        // Calculate which cell was clicked
                                        let local_x = click_pos.x - rect.min.x;
                                        let local_y = click_pos.y - rect.min.y;
                                        let cell_x = (local_x / pixel_size) as i32;
                                        let cell_y = (local_y / pixel_size) as i32;

                                        // Calculate offset from center
                                        let center_x = preview_w as i32 / 2;
                                        let center_y = preview_h as i32 / 2;
                                        let offset_x = cell_x - center_x;
                                        let offset_y = cell_y - center_y;

                                        // Calculate new target pixel
                                        let (found_x, found_y) = click_info.found_pixel;
                                        let new_x = (found_x as i32 + offset_x).max(0) as u32;
                                        let new_y = (found_y as i32 + offset_y).max(0) as u32;

                                        // Update hovered_pixel to trigger re-query
                                        *self.context.hovered_pixel = Some((new_x, new_y));
                                    }
                                }

                                // Draw border around preview
                                painter.add(egui::epaint::RectShape::stroke(
                                    rect.expand(1.0),
                                    egui::CornerRadius::same(2),
                                    egui::Stroke::new(1.0, egui::Color32::GRAY),
                                    egui::epaint::StrokeKind::Outside,
                                ));

                                // Draw each pixel
                                for py in 0..preview_h {
                                    for px in 0..preview_w {
                                        let idx = (py * preview_w + px) as usize;
                                        if idx < click_info.color_preview.len() {
                                            let rgba = click_info.color_preview[idx];
                                            let color = egui::Color32::from_rgba_unmultiplied(
                                                rgba[0], rgba[1], rgba[2], rgba[3]
                                            );

                                            let pixel_rect = egui::Rect::from_min_size(
                                                rect.min + egui::vec2(px as f32 * pixel_size, py as f32 * pixel_size),
                                                egui::vec2(pixel_size, pixel_size),
                                            );

                                            painter.rect_filled(pixel_rect, 0.0, color);

                                            // Highlight center pixel with black and white outline (not filled)
                                            let center_x = preview_w / 2;
                                            let center_y = preview_h / 2;
                                            if px == center_x && py == center_y {
                                                // Outer black stroke
                                                painter.add(egui::epaint::RectShape::stroke(
                                                    pixel_rect.shrink(0.5),
                                                    egui::CornerRadius::ZERO,
                                                    egui::Stroke::new(2.0, egui::Color32::BLACK),
                                                    egui::epaint::StrokeKind::Inside,
                                                ));
                                                // Inner white stroke
                                                painter.add(egui::epaint::RectShape::stroke(
                                                    pixel_rect.shrink(2.5),
                                                    egui::CornerRadius::ZERO,
                                                    egui::Stroke::new(1.0, egui::Color32::WHITE),
                                                    egui::epaint::StrokeKind::Inside,
                                                ));
                                            }
                                        }
                                    }
                                }
                            });
                        });

                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(t!("path_overlay.close_hint"))
                            .small()
                            .color(egui::Color32::DARK_GRAY));
                    });
            });
    }

    /// Render Path Editor panel (manage path filters)
    fn render_path_editor_panel(&mut self, ui: &mut egui::Ui) {
        let num_transforms = self.context.flame.transforms.len();
        let response = super::path_editor::render_path_editor_content(
            ui,
            self.context.path_editor_state,
            num_transforms,
        );

        // Handle filter changes
        if let Some(filters) = response.filters_changed {
            *self.context.path_filters_changed = Some(filters);
        }
    }

    /// Render Export panel (PNG export options)
    fn render_export_panel(&mut self, ui: &mut egui::Ui) {
        super::export_panel::render_export_content(
            ui,
            self.context.png_export_with_background,
            self.context.png_export_transparent,
            self.context.export_width,
            self.context.export_height,
            self.context.use_custom_export_size,
            self.context.png_export_premultiplied,
            self.context.png_export_supersample,
            self.context.config_manager,
            *self.context.fractal_viewport_size,
            self.context.export_active,
            self.context.max_export_dimension,
        );
    }

    /// Render Random Generator panel (generate random flames with settings)
    fn render_random_generator_panel(&mut self, ui: &mut egui::Ui) {
        // Initialize panel if not already created
        if self.context.random_generator_panel.is_none() {
            *self.context.random_generator_panel = Some(super::random_generator::RandomGeneratorPanel::new());
        }

        if let Some(panel) = self.context.random_generator_panel.as_mut() {
            let response = panel.render(ui);

            // Handle generate single request
            if response.generate_single {
                let bundle = crate::scene::randomize::generate_random_flame_bundle(&panel.settings);
                *self.context.generated_flame = Some(bundle);
            }

            // Handle generate batch request - create configs with palettes, open File Browser
            if response.generate_batch {
                let flames = crate::scene::randomize::generate_batch(&panel.settings);
                log::info!("Generated batch of {} flames", flames.len());

                // Convert flames to FractalConfigs with palettes
                let use_random_palette = panel.settings.random_palette;
                let palette_count = self.context.palette_library.len();

                let configs: Vec<crate::config::FractalConfig> = flames
                    .into_iter()
                    .enumerate()
                    .map(|(i, rf)| {
                        let mut config = crate::config::FractalConfig::default();
                        config.flame = rf.flame;
                        config.flame.name = format!("Random {}", i + 1);
                        // Scene render settings (config-level since v3).
                        config.render_mode = rf.render_mode;
                        config.perspective_strength = rf.perspective_strength;

                        // Assign palette - configs must be self-contained
                        if use_random_palette && palette_count > 0 {
                            // Pick a random palette from the library
                            let idx = rand::random_range(0..palette_count);
                            if let Some(palette) = self.context.palette_library.get(idx) {
                                config.palette = palette.clone();
                            }
                        } else {
                            // Use current palette from config manager
                            config.palette = self.context.config_manager.active_config().palette.clone();
                        }

                        config
                    })
                    .collect();

                *self.context.generated_batch = Some(configs);
            }
        }
    }

    /// Render the Scripts panel (generator / modifier flame scripts)
    fn render_scripts_panel(&mut self, ui: &mut egui::Ui) {
        if self.context.scripts_panel.is_none() {
            *self.context.scripts_panel = Some(super::scripts_panel::ScriptsPanel::new());
        }
        // Scripts read the live config: modifiers start from it, and
        // generators inherit its palette.
        let current = self.context.config_manager.active_config().clone();
        let palettes: Vec<crate::scene::palette::Palette> =
            self.context.palette_library.iter().cloned().collect();
        if let Some(panel) = self.context.scripts_panel.as_mut() {
            let response = panel.render(
                ui,
                &current,
                palettes,
                self.context.script_cloud,
                self.context.signed_in,
                self.context.script_cloud_request,
                self.context.window,
            );
            if let Some(config) = response.generated {
                *self.context.script_generated = Some(config);
            }
            if let Some(animation) = response.animation {
                *self.context.script_animation = Some(animation);
            }
            if let Some(batch) = response.batch {
                *self.context.generated_batch = Some(batch);
            }
        }
    }

    /// Render Fractal Browser panel (unified presets/batch/files)
    fn render_fractal_browser_panel(&mut self, ui: &mut egui::Ui) {
        // Initialize panel if not already created
        if self.context.fractal_browser_panel.is_none() {
            *self.context.fractal_browser_panel = Some(super::fractal_browser::FractalBrowserPanel::new());
        }

        if let Some(panel) = self.context.fractal_browser_panel.as_mut() {
            let settings = self.context.config_manager.system_settings();
            let online_mode = settings.online_mode;
            let auth: Option<(&str, &str)> = if settings.is_signed_in() {
                let token = settings.auth_token.as_deref().unwrap_or("");
                Some((crate::api::API_BASE_URL, token))
            } else {
                None
            };
            let response = panel.render(ui, online_mode, auth);

            // Handle file open request from Files tab
            if panel.take_open_file_request() {
                *self.context.file_browser_open_requested = true;
            }

            // Handle selection (load the config)
            if let Some(config) = response.selected {
                *self.context.selected_preset_config = Some(config);

                // Pass API flame ID and visibility through (for Online tab loads)
                *self.context.loaded_api_flame_id = response.api_flame_id;
                *self.context.loaded_api_flame_is_public = response.api_flame_is_public;
                *self.context.loaded_api_flame_user_id = response.api_flame_user_id;
                *self.context.loaded_api_flame_animation_count = response.api_flame_animation_count;
                *self.context.loaded_api_flame_animations = response.api_flame_animations;
            }

            // Pass API notifications through (e.g., delete result)
            if let Some(notification) = response.api_notification {
                *self.context.api_notification = Some(notification);
            }

            // Handle session expiry (401 from API)
            if response.session_expired {
                *self.context.sign_out_requested = true;
            }
        }
    }

    /// Render Effects panel (post-processing color and density effects)
    fn render_effects_panel(&mut self, ui: &mut egui::Ui) {
        super::effects_panel::render_effects_panel(
            ui,
            self.context.config_manager,
            self.context.animation_controller,
            self.context.effect_catalog,
        );
    }

    /// Render Xaos Editor panel (chaos-weighted transform transitions)
    fn render_xaos_editor_panel(&mut self, ui: &mut egui::Ui) {
        let _ = super::xaos_editor::render_xaos_editor_content(
            ui,
            self.context.config_manager,
            self.context.flame,
            self.context.xaos_editor_state,
        );
    }

    /// Render Save Online Dialog panel (name input)
    fn render_save_online_dialog(&mut self, ui: &mut egui::Ui) {
        super::save_online_dialog::render_save_online_dialog(
            ui,
            self.context.save_online_dialog_state,
        );
    }

    /// Render Login Dialog panel (sign in / register / account info)
    fn render_login_dialog(&mut self, ui: &mut egui::Ui) {
        let mut sign_out = false;
        if let Some(success) = super::login_dialog::render_login_dialog(
            ui,
            self.context.login_dialog_state,
            self.context.config_manager,
            &mut sign_out,
        ) {
            // Login/register succeeded — show notification
            *self.context.api_notification = Some((
                format!("Signed in as {}", success.email),
                false,
            ));
        }
        if sign_out {
            *self.context.sign_out_requested = true;
        }
    }

    /// Render Signal panel (signals, audio, generators)
    fn render_signal_panel(&mut self, ui: &mut egui::Ui) {
        let animation_duration = self.context.animation_controller.animation
            .as_ref()
            .map(|a| a.duration)
            .unwrap_or(10.0);
        super::signal_panel::render_signal_panel(
            ui,
            self.context.audio_manager,
            self.context.audio_player,
            self.context.audio_capture,
            self.context.signal_panel_state,
            self.context.signal_manager,
            self.context.current_time,
            animation_duration,
            self.context.load_audio_file,
            self.context.load_signal_file,
            self.context.save_signal_file,
        );
    }
}

use crate::scene::transforms::{Flame, RenderMode};
use crate::variations::{VariationCategory, VariationPhase, global_registry};
use crate::config::{ConfigManager, ConfigPath, UpdateType, AffineParam, TransformRef};
use super::variation_params::render_variation_params;
use super::transform_colors::{normal_color, linked_color, final_color};
use egui::Color32;
use rust_i18n::t;

/// Which phase section a variation instance is displayed/run in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PhaseBucket {
    Pre,
    Main,
    Post,
}

impl PhaseBucket {
    /// The JWildfire `fx_priority` value used when assigning a variation to
    /// this bucket from the UI (canonical ±1 / 0; the dispatch buckets by
    /// sign so ±1 is sufficient).
    fn priority(self) -> i32 {
        match self {
            PhaseBucket::Pre => -1,
            PhaseBucket::Main => 0,
            PhaseBucket::Post => 1,
        }
    }

    fn label(self) -> std::borrow::Cow<'static, str> {
        match self {
            PhaseBucket::Pre => t!("variations.phase_pre"),
            PhaseBucket::Main => t!("variations.phase_main"),
            PhaseBucket::Post => t!("variations.phase_post"),
        }
    }
}

/// Effective phase info for a variation instance on a transform:
/// `(is_movable, bucket)`. `is_movable` is true only for `Any`-phase
/// variations (the ones that honour `fx_priority`); for locked
/// `Pre`/`Normal`/`Post` defs the bucket comes from their fixed phase.
/// For `Any` vars the bucket is read from the stored override (default
/// main/0 when unset).
fn variation_phase_info(
    transform: &crate::scene::transforms::Transform,
    variation_name: &str,
) -> (bool, PhaseBucket) {
    let registry = global_registry();
    let def_phase = registry
        .get(variation_name)
        .map(|i| i.phase.clone())
        .unwrap_or(VariationPhase::Normal);
    let natural = def_phase.natural_priority();
    let eff = transform
        .variation_priorities
        .get(variation_name)
        .copied()
        .unwrap_or(natural);
    let bucket = if eff < 0 {
        PhaseBucket::Pre
    } else if eff > 0 {
        PhaseBucket::Post
    } else {
        PhaseBucket::Main
    };
    (def_phase == VariationPhase::Any, bucket)
}

// Per-index transform colors are shared with the Triangle Editor — see
// `super::transform_colors` (normal_color / linked_color / final_color).

/// Render weight control (always visible)
fn render_weight_control(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    index: usize,
    transform: &mut crate::scene::transforms::Transform,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    let mut temp_weight = transform.weight;
    let response = ui.add(super::VkbSlider::new(&mut temp_weight, 0.0..=1024.0)
        .logarithmic(true)
        .text(t!("transform.weight")))
      .on_hover_text(t!("tooltips.transform_weight"));
    if response.changed() {
        if let Ok(update_type) = config_manager.update_param(
            ConfigPath::TransformWeight { index },
            temp_weight.into()
        ) {
            transform.weight = config_manager.active_flame().transforms[index].weight;
            max_update = max_update.max(update_type);
        }
    }
    if response.drag_stopped() {
        let _ = config_manager.force_commit_preview(&ConfigPath::TransformWeight { index });
    }

    max_update
}

/// Render color controls (palette position + preview)
fn render_color_controls(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    index: usize,
    transform: &mut crate::scene::transforms::Transform,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    ui.horizontal(|ui| {
        // Palette position slider (0.0 to 1.0)
        let mut temp_color = transform.color;
        let response_color = ui.add(
            super::VkbSlider::new(&mut temp_color, 0.0..=1.0)
                .text(t!("transform.color"))
                
        ).on_hover_text(t!("tooltips.transform_color"));
        if response_color.changed() {
            if let Ok(update_type) = config_manager.update_param(
                ConfigPath::TransformColor { index },
                temp_color.into()
            ) {
                transform.color = config_manager.active_flame().transforms[index].color;
                max_update = max_update.max(update_type);
            }
        }
        if response_color.drag_stopped() {
            let _ = config_manager.force_commit_preview(&ConfigPath::TransformColor { index });
        }

        // Show color preview at current palette position
        let palette = &config_manager.active_config().palette;
        let actual_color = palette.sample_color(transform.color);
        let color_swatch = egui::Color32::from_rgb(
            (actual_color[0] * 255.0) as u8,
            (actual_color[1] * 255.0) as u8,
            (actual_color[2] * 255.0) as u8,
        );
        let (rect, _response) = ui.allocate_exact_size(egui::vec2(20.0, 18.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, color_swatch);
    });

    // Color Speed (Symmetry) — sits directly under Color.
    let mut temp_speed = transform.color_speed;
    let response_speed = ui.add(super::VkbSlider::new(&mut temp_speed, -1.0..=1.0).text(t!("transform.color_speed")))
        .on_hover_text(t!("tooltips.color_speed"));
    if response_speed.changed() {
        if let Ok(update_type) = config_manager.update_param(
            ConfigPath::TransformColorSpeed { index },
            temp_speed.into()
        ) {
            transform.color_speed = config_manager.active_flame().transforms[index].color_speed;
            max_update = max_update.max(update_type);
        }
    }
    if response_speed.drag_stopped() {
        let _ = config_manager.force_commit_preview(&ConfigPath::TransformColorSpeed { index });
    }

    max_update
}

/// Render affine matrix controls (in Advanced section)
/// Render the 6-or-7 affine drag controls for a transform in any pool.
/// `xref` identifies which pool + index the transform lives in; the
/// caller passes an up-to-date `&mut Transform` clone to display.
/// On change the right ConfigPath variant is emitted via the
/// `TransformRef` path-builder methods.
fn render_affine_controls(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    xref: TransformRef,
    transform: &mut crate::scene::transforms::Transform,
    render_mode: RenderMode,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    // One affine parameter (label, getter for read-back from flame, mut ref to local).
    // Read-back via xref.get(...) lets the same fn drive Normal/Linked/Final pool members.
    macro_rules! affine_row {
        ($ui:expr, $param:expr, $label_key:expr, $tooltip_key:expr, $local_field:ident, $struct_field:ident) => {
            $ui.label(t!($label_key)).on_hover_text(t!($tooltip_key));
            let response = $ui.add(super::VkbDragValue::new(&mut transform.$struct_field).speed(0.01))
                .on_hover_text(t!($tooltip_key));
            let path = xref.affine_path($param);
            if response.changed() {
                if let Ok(update_type) = config_manager.update_param(path.clone(), transform.$struct_field.into()) {
                    if let Some(t) = xref.get(&config_manager.active_config().flame) {
                        transform.$struct_field = t.$struct_field;
                    }
                    max_update = max_update.max(update_type);
                }
            }
            if response.drag_stopped() {
                let _ = config_manager.force_commit_preview(&path);
            }
        };
    }

    ui.horizontal(|ui| {
        affine_row!(ui, AffineParam::A, "transform.affine_a", "tooltips.affine_a", temp_a, a);
        affine_row!(ui, AffineParam::B, "transform.affine_b", "tooltips.affine_b", temp_b, b);
    });
    ui.horizontal(|ui| {
        affine_row!(ui, AffineParam::C, "transform.affine_c", "tooltips.affine_c", temp_c, c);
        affine_row!(ui, AffineParam::D, "transform.affine_d", "tooltips.affine_d", temp_d, d);
    });
    ui.horizontal(|ui| {
        affine_row!(ui, AffineParam::E, "transform.affine_e", "tooltips.affine_e", temp_e, e);
        affine_row!(ui, AffineParam::F, "transform.affine_f", "tooltips.affine_f", temp_f, f);
    });
    if matches!(render_mode, RenderMode::ThreeD) {
        ui.horizontal(|ui| {
            affine_row!(ui, AffineParam::G, "transform.affine_g", "tooltips.affine_g", temp_g, g);
        });
    }

    max_update
}

/// Render post-affine controls: enable checkbox + matrix.
/// `xref` identifies which pool + index — works for Normal / Linked /
/// Final pool members.
fn render_post_affine_controls(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    xref: TransformRef,
    transform: &mut crate::scene::transforms::Transform,
    render_mode: RenderMode,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    // Enable checkbox
    let mut temp_enabled = transform.post_affine_enabled;
    if ui.checkbox(&mut temp_enabled, t!("transform.post_affine_enabled"))
        .on_hover_text(t!("tooltips.post_affine_enabled"))
        .changed()
    {
        let path = xref.post_affine_enabled_path();
        if let Ok(update_type) = config_manager.update_param(path, temp_enabled.into()) {
            if let Some(t) = xref.get(&config_manager.active_config().flame) {
                transform.post_affine_enabled = t.post_affine_enabled;
            }
            max_update = max_update.max(update_type);
        }
    }

    // Show matrix controls only when enabled
    if transform.post_affine_enabled {
        ui.label(t!("transform.post_affine_matrix"));

        // Same pattern as render_affine_controls but for post_* fields and
        // post_affine_path for ConfigPath construction.
        macro_rules! post_affine_row {
            ($ui:expr, $param:expr, $label_key:expr, $tooltip_key:expr, $struct_field:ident) => {
                $ui.label(t!($label_key)).on_hover_text(t!($tooltip_key));
                let response = $ui.add(super::VkbDragValue::new(&mut transform.$struct_field).speed(0.01))
                    .on_hover_text(t!($tooltip_key));
                let path = xref.post_affine_path($param);
                if response.changed() {
                    if let Ok(update_type) = config_manager.update_param(path.clone(), transform.$struct_field.into()) {
                        if let Some(t) = xref.get(&config_manager.active_config().flame) {
                            transform.$struct_field = t.$struct_field;
                        }
                        max_update = max_update.max(update_type);
                    }
                }
                if response.drag_stopped() {
                    let _ = config_manager.force_commit_preview(&path);
                }
            };
        }

        ui.horizontal(|ui| {
            post_affine_row!(ui, AffineParam::A, "transform.affine_a", "tooltips.affine_a", post_a);
            post_affine_row!(ui, AffineParam::B, "transform.affine_b", "tooltips.affine_b", post_b);
        });
        ui.horizontal(|ui| {
            post_affine_row!(ui, AffineParam::C, "transform.affine_c", "tooltips.affine_c", post_c);
            post_affine_row!(ui, AffineParam::D, "transform.affine_d", "tooltips.affine_d", post_d);
        });
        ui.horizontal(|ui| {
            post_affine_row!(ui, AffineParam::E, "transform.affine_e", "tooltips.affine_e", post_e);
            post_affine_row!(ui, AffineParam::F, "transform.affine_f", "tooltips.affine_f", post_f);
        });
        if matches!(render_mode, RenderMode::ThreeD) {
            ui.horizontal(|ui| {
                post_affine_row!(ui, AffineParam::G, "transform.affine_g", "tooltips.affine_g", post_g);
            });
        }
    }

    max_update
}

/// Which JWildfire plane affine a section is editing. Used both to
/// pick the `Transform` array field and to construct the matching
/// `ConfigPath` variant for the undo/redo + GPU-sync path.
#[derive(Copy, Clone)]
enum JwfPlane {
    YzPre,
    ZxPre,
    YzPost,
    ZxPost,
}

impl JwfPlane {
    fn label_key(self) -> &'static str {
        match self {
            JwfPlane::YzPre => "transform.yz_affine_section",
            JwfPlane::ZxPre => "transform.zx_affine_section",
            JwfPlane::YzPost => "transform.yz_post_affine_section",
            JwfPlane::ZxPost => "transform.zx_post_affine_section",
        }
    }

    fn tooltip_key(self) -> &'static str {
        match self {
            JwfPlane::YzPre => "tooltips.yz_affine",
            JwfPlane::ZxPre => "tooltips.zx_affine",
            JwfPlane::YzPost => "tooltips.yz_post_affine",
            JwfPlane::ZxPost => "tooltips.zx_post_affine",
        }
    }

    fn id_salt(self) -> &'static str {
        match self {
            JwfPlane::YzPre => "yz_affine",
            JwfPlane::ZxPre => "zx_affine",
            JwfPlane::YzPost => "yz_post_affine",
            JwfPlane::ZxPost => "zx_post_affine",
        }
    }

    /// Build the ConfigPath for this plane and position on whichever
    /// pool the transform lives in (Normal / Linked / Final). Routes
    /// through `TransformRef`'s per-pool path helpers added to delta.rs.
    fn path(self, xref: TransformRef, position: u8) -> ConfigPath {
        match self {
            JwfPlane::YzPre => xref.yz_coefs_path(position),
            JwfPlane::ZxPre => xref.zx_coefs_path(position),
            JwfPlane::YzPost => xref.yz_post_coefs_path(position),
            JwfPlane::ZxPost => xref.zx_post_coefs_path(position),
        }
    }

    fn coefs_mut(self, t: &mut crate::scene::transforms::Transform) -> &mut [f32; 6] {
        match self {
            JwfPlane::YzPre => &mut t.yz_coefs,
            JwfPlane::ZxPre => &mut t.zx_coefs,
            JwfPlane::YzPost => &mut t.yz_post_coefs,
            JwfPlane::ZxPost => &mut t.zx_post_coefs,
        }
    }

    fn coefs<'a>(self, t: &'a crate::scene::transforms::Transform) -> &'a [f32; 6] {
        match self {
            JwfPlane::YzPre => &t.yz_coefs,
            JwfPlane::ZxPre => &t.zx_coefs,
            JwfPlane::YzPost => &t.yz_post_coefs,
            JwfPlane::ZxPost => &t.zx_post_coefs,
        }
    }
}

/// Render one JWildfire-extension plane affine section. Six positional
/// inputs `[a, c, b, d, e, f]` in JWildfire's XML write order, laid
/// out as three 2-wide rows (same shape as the XY affine), plus a
/// "Reset to identity" button. The `plane.path(xref, pos)` call
/// builds the matching ConfigPath variant (Normal / Linked / Final
/// pool, picked from `xref`) so undo/redo + GPU sync work identically
/// to the XY affine.
fn render_jwf_plane_section(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    xref: TransformRef,
    transform: &mut crate::scene::transforms::Transform,
    index: usize,
    plane: JwfPlane,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    // id_salt must distinguish pool + index so the same xform index
    // in Normal/Linked/Final doesn't collide their collapsing state.
    egui::CollapsingHeader::new(t!(plane.label_key()))
        .id_salt(format!("{}_{}_{}", plane.id_salt(), xref.pool_kind(), index))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(t!(plane.tooltip_key()));

            // Reset button — restores identity. The GPU's
            // `plane_flags` recompute on upload, so resetting drops
            // this plane back to the flat path automatically; same
            // for the on-disk XML/JSON which skip-writes identity.
            if ui.button(t!("transform.plane_reset")).clicked() {
                let identity = crate::scene::transforms::IDENTITY_PLANE_COEFS;
                for position in 0u8..6 {
                    let path = plane.path(xref, position);
                    if let Ok(u) = config_manager.update_param(
                        path,
                        identity[position as usize].into(),
                    ) {
                        max_update = max_update.max(u);
                    }
                }
                if let Some(t) = xref.get(&config_manager.active_config().flame) {
                    *plane.coefs_mut(transform) = *plane.coefs(t);
                }
            }

            // Render a single coefficient cell. Closure rather than a
            // helper function so the borrows on `transform` and
            // `config_manager` stay scoped to the call site.
            let mut render_cell = |ui: &mut egui::Ui,
                                   transform: &mut crate::scene::transforms::Transform,
                                   config_manager: &mut ConfigManager,
                                   pos: u8,
                                   label: &str| {
                ui.label(label);
                let coefs = plane.coefs_mut(transform);
                let response = ui.add(super::VkbDragValue::new(&mut coefs[pos as usize]).speed(0.01));
                let path = plane.path(xref, pos);
                if response.changed() {
                    let value = coefs[pos as usize];
                    if let Ok(u) = config_manager.update_param(path.clone(), value.into()) {
                        if let Some(t) = xref.get(&config_manager.active_config().flame) {
                            *plane.coefs_mut(transform) = *plane.coefs(t);
                        }
                        max_update = max_update.max(u);
                    }
                }
                if response.drag_stopped() {
                    let _ = config_manager.force_commit_preview(&path);
                }
            };

            // Same `a b / c d / e f` two-wide layout as the XY affine.
            ui.horizontal(|ui| {
                render_cell(ui, transform, config_manager, 0, "a");
                render_cell(ui, transform, config_manager, 1, "c");
            });
            ui.horizontal(|ui| {
                render_cell(ui, transform, config_manager, 2, "b");
                render_cell(ui, transform, config_manager, 3, "d");
            });
            ui.horizontal(|ui| {
                render_cell(ui, transform, config_manager, 4, "e");
                render_cell(ui, transform, config_manager, 5, "f");
            });
        });

    max_update
}

/// Render the four JWildfire-extension plane affine sections (YZ pre,
/// ZX pre, YZ post, ZX post) under the transform's "Advanced" group.
/// Works for all three pools (Normal / Linked / Final) — the
/// `TransformRef`-based ConfigPath helpers route to the right
/// variants on each pool.
///
/// Post-affine sections (`YZ Post` / `ZX Post`) are hidden when the
/// transform's XY post-affine isn't enabled — `apply_post_affine`
/// only runs when `post_enabled=1`, so the post-plane fields would
/// be inert anyway. 3D-mode gated overall: in 2D these affines have
/// no effect.
fn render_jwf_plane_sections(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    xref: TransformRef,
    transform: &mut crate::scene::transforms::Transform,
    render_mode: RenderMode,
) -> UpdateType {
    if !matches!(render_mode, RenderMode::ThreeD) {
        return UpdateType::None;
    }
    // `id_salt` needs a stable suffix per panel so multiple
    // transforms' sections don't collide. The pool kind plus the
    // pool-local index gives that.
    let index = xref.index();

    let mut max_update = UpdateType::None;
    let planes: &[JwfPlane] = if transform.post_affine_enabled {
        &[JwfPlane::YzPre, JwfPlane::ZxPre, JwfPlane::YzPost, JwfPlane::ZxPost]
    } else {
        // Post-affine disabled → the post planes can't contribute
        // either (apply_post_affine isn't called when post_enabled=0).
        // Hide their sections to keep the panel focused.
        &[JwfPlane::YzPre, JwfPlane::ZxPre]
    };
    for plane in planes {
        max_update = max_update.max(render_jwf_plane_section(
            ui, config_manager, xref, transform, index, *plane,
        ));
    }
    max_update
}

/// Render advanced settings (opacity, direct color, solo toggle).
/// Color Speed lives under Color now (see `render_color_controls`).
fn render_advanced_settings(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    index: usize,
    transform: &mut crate::scene::transforms::Transform,
    solo_transform: Option<usize>,
) -> UpdateType {
    let mut max_update = UpdateType::None;

    // Solo toggle - only this transform gets weight, all others = 0
    let is_solo = solo_transform == Some(index);
    let mut solo_checked = is_solo;

    if ui.checkbox(&mut solo_checked, t!("transform.solo"))
        .on_hover_text(t!("tooltips.transform_solo"))
        .changed()
    {
        // -1 = None (no solo), 0+ = Some(index)
        let new_value = if solo_checked { index as i32 } else { -1 };
        if let Ok(update_type) = config_manager.update_param(
            ConfigPath::SoloTransform,
            new_value.into()
        ) {
            max_update = max_update.max(update_type);
        }
    }

    // Show "(muted)" indicator if another transform is solo'd
    if solo_transform.is_some() && !is_solo {
        ui.label(egui::RichText::new("(muted)").weak().italics());
    }

    ui.add_space(4.0);

    // Opacity slider
    let mut temp_opacity = transform.opacity;
    let response_opacity = ui.add(super::VkbSlider::new(&mut temp_opacity, 0.0..=1.0).text(t!("transform.opacity")))
        .on_hover_text(t!("tooltips.opacity"));
    if response_opacity.changed() {
        if let Ok(update_type) = config_manager.update_param(
            ConfigPath::TransformOpacity { index },
            temp_opacity.into()
        ) {
            transform.opacity = config_manager.active_flame().transforms[index].opacity;
            max_update = max_update.max(update_type);
        }
    }
    if response_opacity.drag_stopped() {
        let _ = config_manager.force_commit_preview(&ConfigPath::TransformOpacity { index });
    }

    // Direct Color slider (Apophysis pluginColor — blend strength for DC variations)
    let mut temp_dc = transform.direct_color;
    let response_dc = ui.add(super::VkbSlider::new(&mut temp_dc, 0.0..=1.0).text(t!("transform.direct_color")))
        .on_hover_text(t!("tooltips.direct_color"));
    if response_dc.changed() {
        if let Ok(update_type) = config_manager.update_param(
            ConfigPath::TransformDirectColor { index },
            temp_dc.into()
        ) {
            transform.direct_color = config_manager.active_flame().transforms[index].direct_color;
            max_update = max_update.max(update_type);
        }
    }
    if response_dc.drag_stopped() {
        let _ = config_manager.force_commit_preview(&ConfigPath::TransformDirectColor { index });
    }

    max_update
}

/// Render a single enabled variation with weight slider and delete button.
/// `xref` identifies which pool member owns this variation.
fn render_enabled_variation(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    xref: TransformRef,
    variation_name: &str,
    current_weight: f32,
    movable: bool,
    current_bucket: PhaseBucket,
    // Position of this variation within its phase section, for the
    // reorder controls (enable Up unless first, Down unless last).
    section_pos: usize,
    section_len: usize,
) -> (UpdateType, bool, Option<isize>) {
    let mut max_update = UpdateType::None;
    let mut delete_requested = false;
    // Reorder request: -1 = move up, +1 = move down (within the section).
    // Applied by the caller, which knows the section's variation names.
    let mut reorder: Option<isize> = None;

    let registry = global_registry();
    let var_info = registry.get(variation_name);
    let display_name = var_info
        .map(|v| v.display_name.as_str())
        .unwrap_or(variation_name);

    ui.horizontal(|ui| {
        // Weight slider
        let mut value = current_weight;
        let response = ui.add(
            super::VkbSlider::new(&mut value, -5.0..=5.0)
                .text(egui::RichText::new(display_name).color(Color32::LIGHT_GRAY))
                .drag_value_speed(0.1)
                .clamping(egui::SliderClamping::Never)
        );

        if response.changed() {
            value = value.clamp(f32::MIN, f32::MAX);
            let path = xref.variation_path(variation_name.to_string());
            if let Ok(update_type) = config_manager.update_param(path, value.into()) {
                max_update = max_update.max(update_type);
            }
        }

        if response.drag_stopped() {
            let path = xref.variation_path(variation_name.to_string());
            let _ = config_manager.force_commit_preview(&path);
        }

        // Variation Settings — a gear button opening phase selection (for
        // `Any` variations), reorder controls (for every variation), and
        // Remove.
        ui.menu_button("⚙", |ui| {
            // Phase (Pre / Main / Post) — only for movable (`Any`) variations.
            if movable {
                ui.label(t!("variations.phase_label"));
                for bucket in [PhaseBucket::Pre, PhaseBucket::Main, PhaseBucket::Post] {
                    if ui
                        .selectable_label(bucket == current_bucket, bucket.label())
                        .clicked()
                    {
                        if bucket != current_bucket {
                            let path = xref.variation_priority_path(variation_name.to_string());
                            if let Ok(update_type) =
                                config_manager.update_param(path, bucket.priority().into())
                            {
                                max_update = max_update.max(update_type);
                            }
                        }
                        ui.close();
                    }
                }
                ui.separator();
            }
            // Reorder within the phase section (affects dispatch order).
            ui.label(t!("variations.reorder"));
            ui.add_enabled_ui(section_pos > 0, |ui| {
                if ui.button(t!("variations.move_up")).clicked() {
                    reorder = Some(-1);
                    ui.close();
                }
            });
            ui.add_enabled_ui(section_pos + 1 < section_len, |ui| {
                if ui.button(t!("variations.move_down")).clicked() {
                    reorder = Some(1);
                    ui.close();
                }
            });

            // Remove this variation (moved here from the standalone button).
            ui.separator();
            if ui.button(egui::RichText::new(t!("variations.remove")).color(Color32::LIGHT_RED))
                .on_hover_text(t!("tooltips.remove_variation"))
                .clicked()
            {
                delete_requested = true;
                ui.close();
            }
        })
        .response
        .on_hover_text(t!("variations.settings_tooltip"));
    });

    // Show parameters if variation has them
    if let Some(var_info) = var_info {
        if !var_info.parameters.is_empty() {
            egui::CollapsingHeader::new(t!("variations.parameters", name = display_name))
                .id_salt(format!("params_{}_{}_{}", xref.pool_kind(), xref.index(), variation_name))
                .default_open(false)
                .show(ui, |ui| {
                    let param_update = render_variation_params(
                        ui,
                        config_manager,
                        xref,
                        variation_name,
                        &var_info.parameters,
                    );
                    max_update = max_update.max(param_update);
                });
        }
    }

    (max_update, delete_requested, reorder)
}

/// Render the variations section for a transform.
/// `xref` identifies which pool member's variations are being shown.
fn render_variations_section(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    xref: TransformRef,
    transform: &crate::scene::transforms::Transform,
    render_mode: RenderMode,
    add_variation_popup_id: egui::Id,
) -> (UpdateType, Option<String>, Option<String>) {
    let mut max_update = UpdateType::None;
    let mut variation_to_delete: Option<String> = None;
    let mut variation_to_add: Option<String> = None;
    // Pending reorder: (variation, partner-to-swap-with). Applied after the
    // render loop so it doesn't alias the `config_manager` borrow.
    let mut reorder_swap: Option<(String, String)> = None;

    // The transform's effective variation order — drives within-section
    // display order (last applied at the bottom) and the reorder swaps.
    let registry = global_registry();
    let order = transform.ordered_variation_names(&registry);
    let order_pos = |name: &str| order.iter().position(|n| n == name).unwrap_or(usize::MAX);

    // Bucket each enabled variation into its phase section (Pre / Main /
    // Post). Locked variations sit in their fixed phase; `Any` variations
    // follow their fx_priority override (default main).
    let mut sections: [Vec<(String, f32, bool, PhaseBucket)>; 3] =
        [Vec::new(), Vec::new(), Vec::new()];
    for (name, weight) in transform.variations.iter() {
        let (movable, bucket) = variation_phase_info(transform, name);
        let slot = match bucket {
            PhaseBucket::Pre => 0,
            PhaseBucket::Main => 1,
            PhaseBucket::Post => 2,
        };
        sections[slot].push((name.clone(), *weight, movable, bucket));
    }
    // Order each section by the transform's variation order (not
    // alphabetically), so the last-applied variation sits at the bottom.
    for section in &mut sections {
        section.sort_by_key(|(name, ..)| order_pos(name));
    }

    if sections.iter().all(|s| s.is_empty()) {
        ui.label(t!("transform.variations"));
        ui.label(egui::RichText::new(t!("transform.no_variations")).italics().weak());
    } else {
        // Render each non-empty section under its own header.
        let headers = [
            "transform.variations_pre",
            "transform.variations_main",
            "transform.variations_post",
        ];
        for (slot, section) in sections.iter().enumerate() {
            if section.is_empty() {
                continue;
            }
            ui.label(t!(headers[slot]));
            let section_len = section.len();
            for (pos, (name, weight, movable, bucket)) in section.iter().enumerate() {
                let (update, delete, reorder) = render_enabled_variation(
                    ui,
                    config_manager,
                    xref,
                    name,
                    *weight,
                    *movable,
                    *bucket,
                    pos,
                    section_len,
                );
                max_update = max_update.max(update);
                if delete {
                    variation_to_delete = Some(name.clone());
                }
                if let Some(dir) = reorder {
                    // Swap with the adjacent variation in the same section.
                    let partner = (pos as isize + dir) as usize;
                    if partner < section_len {
                        reorder_swap = Some((name.clone(), section[partner].0.clone()));
                    }
                }
            }
        }
    }

    // Apply a pending reorder by swapping the two names in the full
    // effective order and writing it back as the new variation_order.
    if let Some((a, b)) = reorder_swap {
        let mut new_order = order.clone();
        if let (Some(ia), Some(ib)) = (
            new_order.iter().position(|n| n == &a),
            new_order.iter().position(|n| n == &b),
        ) {
            new_order.swap(ia, ib);
            if let Ok(update_type) =
                config_manager.update_param(xref.variation_order_path(), new_order.into())
            {
                max_update = max_update.max(update_type);
            }
        }
    }

    ui.add_space(4.0);

    // Add Variation button
    let add_btn = ui.add(
        egui::Button::new(t!("variations.add")).fill(Color32::from_rgb(0, 68, 0)),
    );
    let focus_search_id = add_variation_popup_id.with("focus_search");
    if add_btn.clicked() {
        egui::Popup::toggle_id(ui.ctx(), add_variation_popup_id);
        // Focus the search box when the popup opens so typing filters
        // immediately (consumed once inside the popup below).
        ui.data_mut(|d| d.insert_temp(focus_search_id, true));
    }

    // Variation picker popup. `from_response` defaults to "always
    // open"; switch to memory-backed open state so the toggle/close
    // calls above actually control visibility.
    egui::Popup::from_response(&add_btn)
        .id(add_variation_popup_id)
        .open_memory(None)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
        ui.set_min_width(250.0);
        ui.set_max_height(300.0);

        // Search filter
        let search_id = ui.id().with("search");
        let mut search_text = ui.data_mut(|d| d.get_temp::<String>(search_id).unwrap_or_default());
        // One-shot focus request set when the Add button opened the popup.
        let focus_search = ui.data_mut(|d| {
            let v = d.get_temp::<bool>(focus_search_id).unwrap_or(false);
            if v { d.remove::<bool>(focus_search_id); }
            v
        });
        ui.horizontal(|ui| {
            ui.label(t!("variations.search"));
            let r = ui.text_edit_singleline(&mut search_text);
            super::vkb_sync(ui, &r, &search_text);
            if focus_search {
                r.request_focus();
            }
        });
        ui.data_mut(|d| d.insert_temp(search_id, search_text.clone()));

        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            let registry = global_registry();
            let search_lower = search_text.to_lowercase();

            // Collect categories to show based on render mode. Plugin
            // variations are surfaced in both 2D and 3D — their wgsl_2d
            // body is expected to be a sensible 2D implementation
            // (subflame_wf is the current sole plugin and has one). The
            // strictly-3D categories (Depth3D, Rotation3D, Full3D)
            // remain 3D-only since their 2D bodies are stubs.
            let categories: Vec<VariationCategory> = if matches!(render_mode, RenderMode::ThreeD) {
                vec![
                    VariationCategory::Basic2D,
                    VariationCategory::Advanced2D,
                    VariationCategory::Depth3D,
                    VariationCategory::Rotation3D,
                    VariationCategory::Full3D,
                    VariationCategory::Plugin,
                ]
            } else {
                vec![
                    VariationCategory::Basic2D,
                    VariationCategory::Advanced2D,
                    VariationCategory::Plugin,
                ]
            };

            for category in categories {
                let variations = registry.by_category(category);
                let filtered: Vec<_> = variations
                    .iter()
                    .filter(|v| {
                        // Filter by search and exclude already-enabled variations
                        let matches_search = search_text.is_empty()
                            || v.name.to_lowercase().contains(&search_lower)
                            || v.display_name.to_lowercase().contains(&search_lower);
                        let not_enabled = !transform.variations.contains_key(&v.name);
                        matches_search && not_enabled
                    })
                    .collect();

                if !filtered.is_empty() {
                    ui.label(egui::RichText::new(format!("{:?}", category)).strong());
                    for var_info in filtered {
                        if ui.selectable_label(false, &var_info.display_name).clicked() {
                            variation_to_add = Some(var_info.name.clone());
                            egui::Popup::close_id(ui.ctx(), add_variation_popup_id);
                        }
                    }
                    ui.add_space(4.0);
                }
            }
        });
    });

    (max_update, variation_to_delete, variation_to_add)
}

/// Bundled mutable references for the transform pool actions used by the
/// Transforms panel. Keeps the call site short and allows new pools to
/// be added without growing the parameter list.
pub struct PoolActions<'a> {
    pub add_normal: &'a mut bool,
    pub delete_normal: &'a mut Option<usize>,
    pub clone_normal: &'a mut Option<usize>,
    pub add_linked: &'a mut bool,
    pub delete_linked: &'a mut Option<usize>,
    pub clone_linked: &'a mut Option<usize>,
    pub add_final: &'a mut bool,
    pub delete_final: &'a mut Option<usize>,
    pub clone_final: &'a mut Option<usize>,
    pub attachment_edit: &'a mut Option<crate::ui::response::AttachmentEdit>,
}

/// Render the Transforms panel content (transform list, affine, variations)
///
/// This is the panel version without the Window wrapper.
pub fn render_transforms_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    flame: &mut Flame,
    pool_actions: PoolActions,
    open_triangle_editor: &mut bool,
) -> UpdateType {
    let mut max_update = UpdateType::None;
    let PoolActions {
        add_normal: add_transform,
        delete_normal: delete_transform,
        clone_normal: clone_transform,
        add_linked,
        delete_linked,
        clone_linked,
        add_final,
        delete_final,
        clone_final,
        attachment_edit,
    } = pool_actions;

    // --- Section collapse/expand policy ---
    // Sections start collapsed on program start and whenever a new fractal
    // is loaded (tracked via ConfigManager::load_generation), but a
    // transform added via the "Add" button opens expanded.
    let pending_open_id = egui::Id::new("transforms_panel_pending_open");
    let load_gen = config_manager.load_generation();
    let collapse_all = ui.ctx().data_mut(|d| {
        let gen_id = egui::Id::new("transforms_panel_load_gen");
        let last: Option<u64> = d.get_temp(gen_id);
        if last != Some(load_gen) {
            d.insert_temp(gen_id, load_gen);
            true
        } else {
            false
        }
    });
    // Take any "open this newly-added member" request (consumed once). A
    // fresh fractal load overrides a stale request from the previous one.
    let pending_open: Option<(u8, usize)> = ui.ctx().data_mut(|d| {
        let v = d.get_temp::<(u8, usize)>(pending_open_id);
        if v.is_some() {
            d.remove::<(u8, usize)>(pending_open_id);
        }
        v
    });
    let pending_open = if collapse_all { None } else { pending_open };
    // pool: 0 = normal, 1 = linked, 2 = final
    let open_override = |pool: u8, idx: usize| -> Option<bool> {
        if collapse_all {
            Some(false)
        } else if pending_open == Some((pool, idx)) {
            Some(true)
        } else {
            None
        }
    };

    ui.heading(format!("Transforms ({})", flame.transforms.len()));

    // Add transform button
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new(t!("transform.add")).fill(Color32::from_rgb(0, 68, 0))).clicked() {
            *add_transform = true;
            // The new transform is appended at the current end; open it.
            let new_idx = flame.transforms.len();
            ui.ctx().data_mut(|d| d.insert_temp(pending_open_id, (0u8, new_idx)));
        }
    });

    ui.separator();

    let render_mode = flame.render_mode;
    let num_normals = flame.transforms.len();
    let num_linked = flame.linked_transforms.len();
    let num_finals = flame.final_transforms.len();
    let solo_transform = config_manager.active_flame().solo_transform;
    // Captured-by-render snapshot of the per-normal attachments — needed to
    // render checkboxes/reorder buttons in the Advanced section while the
    // borrow on `flame.transforms` is held by the iter_mut loop.
    let normal_attachments_snapshot: Vec<(Vec<usize>, Vec<usize>)> = flame
        .transforms
        .iter()
        .map(|t| (t.linked_attachments.clone(), t.final_attachments.clone()))
        .collect();

    // ---------- NORMAL POOL ----------
    let mut normal_delete = None;
    let mut normal_clone = None;
    for (i, transform) in flame.transforms.iter_mut().enumerate() {
        let (linked_att, final_att) = &normal_attachments_snapshot[i];
        ui.push_id(("normal", i), |ui| {
            let block = render_pool_member_block(
                ui,
                config_manager,
                TransformRef::Normal(i),
                transform,
                render_mode,
                PoolMemberOptions {
                    show_weight: true,
                    show_color_top: true,
                    show_color_dynamics: true,
                    show_solo: true,
                    show_edit_triangle: true,
                    can_delete: num_normals > 1,
                    header_text: format!("Transform {}", i + 1),
                    header_color: Some(normal_color(i)),
                    default_open: false,
                    open_override: open_override(0, i),
                    attachments: Some(NormalAttachmentsView {
                        linked: linked_att,
                        final_: final_att,
                        num_linked,
                        num_finals,
                        out: attachment_edit,
                    }),
                },
                solo_transform,
                open_triangle_editor,
            );
            max_update = max_update.max(block.update);
            if block.delete_requested { normal_delete = Some(i); }
            if block.clone_requested { normal_clone = Some(i); }
        });
    }
    if let Some(idx) = normal_delete { *delete_transform = Some(idx); }
    if let Some(idx) = normal_clone { *clone_transform = Some(idx); }

    // ---------- LINKED POOL ----------
    ui.add_space(8.0);
    ui.heading(format!("Linked Transforms ({})", num_linked));
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new(t!("transform.add_linked")).fill(Color32::from_rgb(0, 68, 0)))
            .on_hover_text(t!("tooltips.transform_add_linked"))
            .clicked()
        {
            *add_linked = true;
            let new_idx = flame.linked_transforms.len();
            ui.ctx().data_mut(|d| d.insert_temp(pending_open_id, (1u8, new_idx)));
        }
    });
    ui.separator();

    let mut linked_delete = None;
    let mut linked_clone = None;
    for (i, transform) in flame.linked_transforms.iter_mut().enumerate() {
        ui.push_id(("linked", i), |ui| {
            let block = render_pool_member_block(
                ui,
                config_manager,
                TransformRef::Linked(i),
                transform,
                render_mode,
                PoolMemberOptions {
                    // Linked transforms run sequentially as part of dynamics
                    // and inherit color/opacity from the normal that triggered
                    // them, so weight + color controls don't apply.
                    show_weight: false,
                    show_color_top: false,
                    show_color_dynamics: false,
                    show_solo: false,
                    show_edit_triangle: true,
                    can_delete: true,
                    header_text: format!("Linked {}", i + 1),
                    header_color: Some(linked_color(i)),
                    default_open: false,
                    open_override: open_override(1, i),
                    attachments: None,
                },
                solo_transform,
                open_triangle_editor,
            );
            max_update = max_update.max(block.update);
            if block.delete_requested { linked_delete = Some(i); }
            if block.clone_requested { linked_clone = Some(i); }
        });
    }
    if let Some(idx) = linked_delete { *delete_linked = Some(idx); }
    if let Some(idx) = linked_clone { *clone_linked = Some(idx); }

    // ---------- FINAL POOL ----------
    ui.add_space(8.0);
    ui.heading(format!("Final Transforms ({})", num_finals));
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new(t!("transform.add_final")).fill(Color32::from_rgb(0, 68, 0)))
            .on_hover_text(t!("tooltips.transform_add_final"))
            .clicked()
        {
            *add_final = true;
            let new_idx = flame.final_transforms.len();
            ui.ctx().data_mut(|d| d.insert_temp(pending_open_id, (2u8, new_idx)));
        }
    });
    ui.separator();

    let mut final_delete = None;
    let mut final_clone = None;
    for (i, transform) in flame.final_transforms.iter_mut().enumerate() {
        ui.push_id(("final", i), |ui| {
            let block = render_pool_member_block(
                ui,
                config_manager,
                TransformRef::Final(i),
                transform,
                render_mode,
                PoolMemberOptions {
                    show_weight: false,
                    show_color_top: false,
                    show_color_dynamics: false,
                    show_solo: false,
                    show_edit_triangle: true,
                    can_delete: true,
                    header_text: format!("Final {}", i + 1),
                    header_color: Some(final_color(i)),
                    default_open: false,
                    open_override: open_override(2, i),
                    attachments: None,
                },
                solo_transform,
                open_triangle_editor,
            );
            max_update = max_update.max(block.update);
            if block.delete_requested { final_delete = Some(i); }
            if block.clone_requested { final_clone = Some(i); }
        });
    }
    if let Some(idx) = final_delete { *delete_final = Some(idx); }
    if let Some(idx) = final_clone { *clone_final = Some(idx); }

    max_update
}

/// Per-pool customization knobs for `render_pool_member_block`.
struct PoolMemberOptions<'a> {
    show_weight: bool,
    show_color_top: bool,
    show_color_dynamics: bool,
    show_solo: bool,
    show_edit_triangle: bool,
    can_delete: bool,
    header_text: String,
    header_color: Option<Color32>,
    default_open: bool,
    /// Force this member's collapsing section open/closed this frame,
    /// overriding the persisted egui state. `Some(false)` collapses on
    /// fractal load/program start; `Some(true)` expands a freshly-added
    /// transform; `None` leaves the user's persisted state alone.
    open_override: Option<bool>,
    /// Only meaningful for Normal-pool members — when set, the Advanced
    /// section gains "Linked XForms" and "Final XForms" subsections that
    /// drive the per-normal attachment lists.
    attachments: Option<NormalAttachmentsView<'a>>,
}

/// Borrowed view of a normal transform's attachment lists, plus the
/// out-param channel for any user-driven edits.
struct NormalAttachmentsView<'a> {
    linked: &'a [usize],
    final_: &'a [usize],
    num_linked: usize,
    num_finals: usize,
    out: &'a mut Option<crate::ui::response::AttachmentEdit>,
}

/// Output of one pool-member render: aggregated UpdateType and any
/// row-level button presses that the caller must dispatch.
struct PoolMemberBlock {
    update: UpdateType,
    delete_requested: bool,
    clone_requested: bool,
}

/// Render a single pool member (Normal / Linked / Final) using the
/// shared affine, post-affine, and variations helpers. The pool kind
/// comes from `xref` and decides which ConfigPath variants are used;
/// `opts` controls which extra widgets are shown alongside.
fn render_pool_member_block(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    xref: TransformRef,
    transform: &mut crate::scene::transforms::Transform,
    render_mode: RenderMode,
    mut opts: PoolMemberOptions,
    solo_transform: Option<usize>,
    open_triangle_editor: &mut bool,
) -> PoolMemberBlock {
    let mut update = UpdateType::None;
    let mut delete_requested = false;
    let mut clone_requested = false;

    let id = ui.make_persistent_id(format!("{}_header_{}", xref.pool_kind(), xref.index()));
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, opts.default_open);
    // Collapse-on-load / expand-new-transform override (see PoolMemberOptions).
    if let Some(open) = opts.open_override {
        state.set_open(open);
    }

    let header_response = ui.horizontal(|ui| {
        let _icon_response = state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);
        let header_text = egui::RichText::new(opts.header_text.clone()).strong().size(14.0);
        // Clickable label: clicking the "Transform N" name toggles the section
        // (not just the little triangle icon). Sense::click() is required —
        // a plain ui.label() never reports .clicked().
        let text_response = ui
            .add(egui::Label::new(header_text).sense(egui::Sense::click()))
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if let Some(color) = opts.header_color {
            let (circle_rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
            ui.painter().circle_filled(circle_rect.center(), 5.0, color);
        }
        text_response
    });

    if header_response.inner.clicked() {
        state.toggle(ui);
    }

    // When collapsed, show a compact variation summary under the name
    // (e.g. "pre_blur, bubble") — smaller, indented, light grey.
    if !state.is_open() {
        let registry = crate::variations::global_registry();
        let names = transform.ordered_variation_names(&registry);
        if !names.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(35.0);
                ui.label(
                    egui::RichText::new(names.join(", "))
                        .size(11.0)
                        .color(egui::Color32::from_gray(160)),
                );
            });
        }
    }

    state.show_body_indented(&header_response.response, ui, |ui| {
        egui::Frame::new()
            .inner_margin(egui::Margin { left: 0, right: 5, top: 5, bottom: 5 })
            .show(ui, |ui| {
                // Edit-Triangle / Clone / Delete row
                ui.horizontal(|ui| {
                    if opts.show_edit_triangle {
                        if ui.button(t!("transform.edit_triangle"))
                            .on_hover_text(t!("tooltips.transform_edit_triangle"))
                            .clicked()
                        {
                            ui.ctx().data_mut(|d| {
                                d.insert_persisted(egui::Id::new("triangle_editor_selected_transform"), xref);
                            });
                            *open_triangle_editor = true;
                        }
                    }

                    if ui.button(t!("transform.clone"))
                        .on_hover_text(t!("tooltips.transform_clone"))
                        .clicked()
                    {
                        clone_requested = true;
                    }

                    if opts.can_delete {
                        if ui.button(egui::RichText::new(t!("transform.delete")).color(Color32::LIGHT_RED))
                            .on_hover_text(t!("tooltips.transform_delete"))
                            .clicked()
                        {
                            delete_requested = true;
                        }
                    }
                });

                // Top-level (always-visible) extras: weight + color.
                // These fields only fire ConfigPath variants for Normal-pool
                // transforms today (see Phase 5d), so gate on TransformRef::Normal.
                if let TransformRef::Normal(i) = xref {
                    if opts.show_weight {
                        update = update.max(render_weight_control(ui, config_manager, i, transform));
                    }
                    if opts.show_color_top {
                        update = update.max(render_color_controls(ui, config_manager, i, transform));
                    }
                }

                // Advanced section: affine + post-affine (always for all pools)
                // plus pool-specific color dynamics / solo for the Normal pool.
                egui::CollapsingHeader::new(t!("transform.advanced"))
                    .id_salt(format!("advanced_{}_{}", xref.pool_kind(), xref.index()))
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.label(t!("transform.affine_matrix"));
                        update = update.max(render_affine_controls(ui, config_manager, xref, transform, render_mode));
                        ui.add_space(4.0);
                        update = update.max(render_post_affine_controls(ui, config_manager, xref, transform, render_mode));
                        ui.add_space(4.0);

                        // JWildfire-extension YZ/ZX plane affines and
                        // their post-affine siblings. Self-gated:
                        // 3D-mode-only, Normal pool only.
                        update = update.max(render_jwf_plane_sections(ui, config_manager, xref, transform, render_mode));
                        ui.add_space(4.0);

                        // color_speed / opacity / direct_color / solo are only
                        // wired through ConfigPath::Transform* variants for the
                        // Normal pool today. Linked/Final pools edit those fields
                        // directly via ConfigPath variants added in Phase 5d.
                        if let TransformRef::Normal(i) = xref {
                            if opts.show_color_dynamics {
                                let solo = if opts.show_solo { solo_transform } else { None };
                                update = update.max(render_advanced_settings(ui, config_manager, i, transform, solo));
                            }

                            // Attachment subsections — Linked XForms / Final XForms.
                            if let Some(view) = opts.attachments.as_mut() {
                                ui.add_space(6.0);
                                render_attachment_subsection(
                                    ui, i, "transform.linked_attachments",
                                    crate::ui::response::AttachmentKind::Linked,
                                    view.linked, view.num_linked, view.out,
                                );
                                ui.add_space(4.0);
                                render_attachment_subsection(
                                    ui, i, "transform.final_attachments",
                                    crate::ui::response::AttachmentKind::Final,
                                    view.final_, view.num_finals, view.out,
                                );
                            }
                        }
                    });

                ui.add_space(4.0);

                // Variations section
                let popup_id = ui.id().with("add_var_popup");
                let (var_update, var_to_delete, var_to_add) = {
                    let mut tmp_update = UpdateType::None;
                    let mut tmp_del = None;
                    let mut tmp_add = None;
                    egui::Frame::new()
                        .fill(ui.visuals().extreme_bg_color)
                        .stroke(egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color))
                        .corner_radius(4.0)
                        .inner_margin(6.0)
                        .show(ui, |ui| {
                            let (u, d, a) = render_variations_section(
                                ui,
                                config_manager,
                                xref,
                                transform,
                                render_mode,
                                popup_id,
                            );
                            tmp_update = u;
                            tmp_del = d;
                            tmp_add = a;
                        });
                    (tmp_update, tmp_del, tmp_add)
                };
                update = update.max(var_update);

                if let Some(var_name) = var_to_delete {
                    // Whole-transform snapshot so undo restores the removed
                    // variation's params/priorities/order (which
                    // remove_variation scrubs), not just its weight.
                    if let Ok(u) = config_manager.remove_variation(xref, &var_name) {
                        update = update.max(u);
                    }
                }
                if let Some(var_name) = var_to_add {
                    let path = xref.variation_path(var_name);
                    if let Ok(u) = config_manager.update_param(path, 1.0f32.into()) {
                        update = update.max(u);
                    }
                }
            });
    });

    PoolMemberBlock { update, delete_requested, clone_requested }
}

/// Render the per-normal "Linked XForms" or "Final XForms" subsection: one
/// row per pool member with a checkbox (toggle attach) and ↑/↓ buttons that
/// reorder the attachment within this normal's execution list.
///
/// `attachments` is the ordered list of pool indices already attached to
/// this normal; `pool_size` is the size of the linked/final pool. On user
/// action the resulting `AttachmentEdit` is written to `*out` (last-writer
/// wins within a frame, which matches the one-click-at-a-time UI).
fn render_attachment_subsection(
    ui: &mut egui::Ui,
    normal_index: usize,
    label_key: &str,
    kind: crate::ui::response::AttachmentKind,
    attachments: &[usize],
    pool_size: usize,
    out: &mut Option<crate::ui::response::AttachmentEdit>,
) {
    use crate::ui::response::{AttachmentEdit, AttachmentOp};

    ui.label(t!(label_key));
    if pool_size == 0 {
        ui.label(egui::RichText::new(t!("transform.no_attachments_pool_empty"))
            .italics()
            .weak());
        return;
    }

    let pool_kind_tag = match kind {
        crate::ui::response::AttachmentKind::Linked => "linked",
        crate::ui::response::AttachmentKind::Final => "final",
    };

    // Walk the pool order (0..pool_size) so unattached items remain visible
    // and can be toggled on. For attached items we also draw reorder buttons.
    for pool_idx in 0..pool_size {
        let attached_pos = attachments.iter().position(|&a| a == pool_idx);
        let is_attached = attached_pos.is_some();

        ui.push_id((pool_kind_tag, normal_index, pool_idx), |ui| {
            ui.horizontal(|ui| {
                let mut checked = is_attached;
                let label = format!("{} {}", match kind {
                    crate::ui::response::AttachmentKind::Linked => "Linked",
                    crate::ui::response::AttachmentKind::Final => "Final",
                }, pool_idx + 1);
                if ui.checkbox(&mut checked, label).changed() {
                    *out = Some(AttachmentEdit {
                        normal_index,
                        kind,
                        op: AttachmentOp::Toggle(pool_idx),
                    });
                }
                if let Some(pos) = attached_pos {
                    // Show execution position within this normal's chain.
                    ui.label(egui::RichText::new(format!("#{}", pos + 1)).weak().small());
                    let can_up = pos > 0;
                    let can_down = pos + 1 < attachments.len();
                    if ui.add_enabled(can_up, egui::Button::new("↑").small()).clicked() {
                        *out = Some(AttachmentEdit {
                            normal_index,
                            kind,
                            op: AttachmentOp::MoveUp(pool_idx),
                        });
                    }
                    if ui.add_enabled(can_down, egui::Button::new("↓").small()).clicked() {
                        *out = Some(AttachmentEdit {
                            normal_index,
                            kind,
                            op: AttachmentOp::MoveDown(pool_idx),
                        });
                    }
                }
            });
        });
    }
}

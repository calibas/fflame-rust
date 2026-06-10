use crate::scene::transforms::{Flame, PostSymmetryType, RenderMode};
use crate::config::{ConfigManager, ConfigPath, FractalConfig};
use rust_i18n::t;

/// Render view controls content (for docking panels)
pub fn render_view_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    flame: &Flame,
    fly_mode_active: bool,
    fly_mode_toggle_requested: &mut bool,
) {
    use crate::config::slider::LazyUndoUi;

    // Clone config to avoid borrow conflicts
    let config = config_manager.active_config().clone();

    ui.label(t!("view.zoom")).on_hover_text(t!("view.tooltip_zoom"));
    ui.horizontal(|ui| {
        if ui.button(t!("view.zoom_in").as_ref()).on_hover_text(t!("view.tooltip_zoom")).clicked() {
            let new_zoom = config.zoom * 1.5;
            let _ = config_manager.update_param(
                ConfigPath::Zoom,
                new_zoom.into());
        }
        if ui.button(t!("view.zoom_out").as_ref()).on_hover_text(t!("view.tooltip_zoom")).clicked() {
            let new_zoom = config.zoom / 1.5;
            let _ = config_manager.update_param(
                ConfigPath::Zoom,
                new_zoom.into()
            );
        }
    });
    ui.horizontal(|ui| {
        ui.label(t!("view.zoom_value")).on_hover_text(t!("view.tooltip_zoom"));
        let _ = ui.lazy_drag(config_manager, ConfigPath::Zoom, 0.01, "");
    });

    ui.separator();

    ui.label(t!("view.pan")).on_hover_text(t!("view.tooltip_pan"));
    ui.horizontal(|ui| {
        ui.label(t!("view.pan_x")).on_hover_text(t!("view.tooltip_pan"));
        let mut pan_x = config.pan_x;
        let response_x = ui.add(
            egui::DragValue::new(&mut pan_x)
                .speed(0.001 / config.zoom)
                .custom_formatter(|v, _| format!("{:.7}", v))
        ).on_hover_text(t!("view.tooltip_pan"));
        super::vkb_sync_opts(ui, &response_x, &format!("{}", pan_x), "decimal");
        if response_x.changed() {
            let _ = config_manager.update_param(
                ConfigPath::Pan,
                (pan_x, config.pan_y).into()
            );
        }

        ui.label(t!("view.pan_y")).on_hover_text(t!("view.tooltip_pan"));
        let mut pan_y = config.pan_y;
        let response_y = ui.add(
            egui::DragValue::new(&mut pan_y)
                .speed(0.001 / config.zoom)
                .custom_formatter(|v, _| format!("{:.7}", v))
        ).on_hover_text(t!("view.tooltip_pan"));
        super::vkb_sync_opts(ui, &response_y, &format!("{}", pan_y), "decimal");
        if response_y.changed() {
            let _ = config_manager.update_param(
                ConfigPath::Pan,
                (config.pan_x, pan_y).into()
            );
        }
    });

    // Pan step size depends on zoom
    let pan_step = 0.1 / config.zoom;

    ui.separator();
    ui.label(t!("view.arrow_controls"));

    // Arrow buttons pan in screen space; conversion to the pan frame
    // is rotation-aware in 2D and identity in 3D (see
    // FractalConfig::screen_delta_to_pan_frame).
    ui.horizontal(|ui| {
        ui.add_space(30.0);
        if ui.button("  ^  ").clicked() {
            let (dx, dy) = config.screen_delta_to_pan_frame(0.0, -pan_step);
            let _ = config_manager.update_param(
                ConfigPath::Pan,
                (config.pan_x + dx, config.pan_y + dy).into()
            );
        }
    });
    ui.horizontal(|ui| {
        if ui.button("  <  ").clicked() {
            let (dx, dy) = config.screen_delta_to_pan_frame(-pan_step, 0.0);
            let _ = config_manager.update_param(
                ConfigPath::Pan,
                (config.pan_x + dx, config.pan_y + dy).into()
            );
        }
        if ui.button("  v  ").clicked() {
            let (dx, dy) = config.screen_delta_to_pan_frame(0.0, pan_step);
            let _ = config_manager.update_param(
                ConfigPath::Pan,
                (config.pan_x + dx, config.pan_y + dy).into()
            );
        }
        if ui.button("  >  ").clicked() {
            let (dx, dy) = config.screen_delta_to_pan_frame(pan_step, 0.0);
            let _ = config_manager.update_param(
                ConfigPath::Pan,
                (config.pan_x + dx, config.pan_y + dy).into()
            );
        }
    });

    ui.separator();

    let mut degrees = config.rotation.to_degrees();
    let response = ui.add(
        super::VkbSlider::new(&mut degrees, -180.0..=180.0)
            .text(t!("view.rotation").as_ref())
            .suffix("°")
    ).on_hover_text(t!("view.tooltip_rotation"));
    if response.changed() {
        let new_rotation = degrees.to_radians();
        let _ = config_manager.update_param(
            ConfigPath::Rotation,
            new_rotation.into()
        );
    }

    ui.separator();

    // 3D Rendering Controls
    ui.label(t!("view.render_mode")).on_hover_text(t!("view.tooltip_render_mode"));
    ui.horizontal(|ui| {
        let was_2d = matches!(config.flame.render_mode, crate::scene::transforms::RenderMode::TwoD);
        if ui.selectable_label(was_2d, t!("view.mode_2d").as_ref())
            .on_hover_text(t!("view.tooltip_mode_2d"))
            .clicked()
        {
            if let Err(e) = config_manager.update_param(
                ConfigPath::RenderMode,
                crate::scene::transforms::RenderMode::TwoD.into()
            ) {
                log::error!("Failed to update render mode: {}", e);
            }
        }
        if ui.selectable_label(!was_2d, t!("view.mode_3d").as_ref())
            .on_hover_text(t!("view.tooltip_mode_3d"))
            .clicked()
        {
            if let Err(e) = config_manager.update_param(
                ConfigPath::RenderMode,
                crate::scene::transforms::RenderMode::ThreeD.into()
            ) {
                log::error!("Failed to update render mode: {}", e);
            }
        }
    });

    // Show perspective control only in 3D mode
    if matches!(config.flame.render_mode, crate::scene::transforms::RenderMode::ThreeD) {
        let mut perspective = config.flame.perspective_strength;
        let response = ui.add(
            super::VkbSlider::new(&mut perspective, 0.0..=10.0)
                .text(t!("view.perspective").as_ref())
                .step_by(0.01)
        ).on_hover_text(t!("view.tooltip_perspective"));
        let changed = response.changed();
        if changed {
            if let Err(e) = config_manager.update_param(
                ConfigPath::PerspectiveStrength,
                perspective.into()
            ) {
                log::error!("Failed to update perspective strength: {}", e);
            }
        }

    }

    ui.separator();

    // 3D Camera rotation controls (only visible in 3D mode)
    if matches!(flame.render_mode, RenderMode::ThreeD) {
        ui.separator();
        ui.label(t!("view.camera_3d"));

        let mut degrees_x = config.camera_rotation_x.to_degrees();
        let response = ui.add(
            super::VkbSlider::new(&mut degrees_x, -180.0..=180.0)
                .text(t!("view.camera_pitch").as_ref())
                .suffix("°")
        ).on_hover_text(t!("view.tooltip_camera_pitch"));
        if response.changed() {
            let new_camera_x = degrees_x.to_radians();
            let _ = config_manager.update_param(
                ConfigPath::CameraRotationX,
                new_camera_x.into()
            );
        }

        let mut degrees_y = config.camera_rotation_y.to_degrees();
        let response = ui.add(
            super::VkbSlider::new(&mut degrees_y, -180.0..=180.0)
                .text(t!("view.camera_yaw").as_ref())
                .suffix("°")
        ).on_hover_text(t!("view.tooltip_camera_yaw"));
        if response.changed() {
            let new_camera_y = degrees_y.to_radians();
            let _ = config_manager.update_param(
                ConfigPath::CameraRotationY,
                new_camera_y.into()
            );
        }

        // Bank — JWildfire/Apophysis Y-axis rotation, the 4th camera
        // angle. Tilts the view horizontally (perspective skew).
        // Round-trips through XML as `cam_roll` per JWildfire's
        // rename quirk; see docs/projects/jwf-features.md.
        let mut degrees_bank = config.camera_bank.to_degrees();
        let response = ui.add(
            super::VkbSlider::new(&mut degrees_bank, -180.0..=180.0)
                .text(t!("view.camera_bank").as_ref())
                .suffix("°")
        ).on_hover_text(t!("view.tooltip_camera_bank"));
        if response.changed() {
            let new_bank = degrees_bank.to_radians();
            let _ = config_manager.update_param(
                ConfigPath::CameraBank,
                new_bank.into()
            );
        }

        // Camera world-space position. Round-trips through JWildfire's
        // `cam_pos_x/y/z` triple. Default (0, 0, 0). Will be driven by
        // WASD / mouse-look in stage 2; for now, numeric sliders only.
        ui.horizontal(|ui| {
            ui.label(t!("view.camera_x")).on_hover_text(t!("view.tooltip_camera_x"));
            let _ = ui.lazy_drag(config_manager, ConfigPath::CameraX, 0.01, "");
        });
        ui.horizontal(|ui| {
            ui.label(t!("view.camera_y")).on_hover_text(t!("view.tooltip_camera_y"));
            let _ = ui.lazy_drag(config_manager, ConfigPath::CameraY, 0.01, "");
        });
        ui.horizontal(|ui| {
            ui.label(t!("view.camera_z")).on_hover_text(t!("view.tooltip_camera_z"));
            let _ = ui.lazy_drag(config_manager, ConfigPath::CameraZ, 0.01, "");
        });

        // Free-fly camera mode toggle. When on:
        //   - Primary mouse drag in the fractal viewport rotates the
        //     view (pitch + yaw) instead of panning the fractal.
        //   - WASD moves the camera along its forward / right axes.
        //   - Q / E move it along world-down / world-up.
        //   - Shift acts as a sprint multiplier.
        // F2 toggles the same state from any focus. Sensitivity and
        // speed live in Settings → Preferences. See
        // `docs/projects/free-camera-movement.md` for the full plan.
        let fly_label = if fly_mode_active {
            t!("view.fly_mode_on")
        } else {
            t!("view.fly_mode_off")
        };
        if ui.button(fly_label.as_ref())
            .on_hover_text(t!("view.tooltip_fly_mode"))
            .clicked()
        {
            *fly_mode_toggle_requested = true;
        }

        // Fly-mode tuning. Folded behind a collapsing header so it
        // doesn't crowd the main View panel; the defaults are good
        // for typical mice and most users will never touch these.
        egui::CollapsingHeader::new(t!("view.fly_mode_settings").as_ref())
            .default_open(false)
            .show(ui, |ui| {
                // Mouse-look model. FreeLook = screen-relative
                // (space-sim); Fps = world-up anchored (horizon
                // stays level). Q/E rise axis follows the choice.
                let mode = config_manager.system_settings().fly_camera_mode;
                ui.horizontal(|ui| {
                    ui.label(t!("view.fly_camera_mode"));
                    if ui.selectable_label(
                        mode == crate::storage::FlyCameraMode::FreeLook,
                        t!("view.fly_camera_mode_free_look").as_ref(),
                    )
                    .on_hover_text(t!("view.tooltip_fly_camera_mode_free_look"))
                    .clicked()
                        && mode != crate::storage::FlyCameraMode::FreeLook
                    {
                        let _ = config_manager.update_system_setting(
                            ConfigPath::SystemFlyCameraMode,
                            "free_look".into(),
                        );
                    }
                    if ui.selectable_label(
                        mode == crate::storage::FlyCameraMode::Fps,
                        t!("view.fly_camera_mode_fps").as_ref(),
                    )
                    .on_hover_text(t!("view.tooltip_fly_camera_mode_fps"))
                    .clicked()
                        && mode != crate::storage::FlyCameraMode::Fps
                    {
                        let _ = config_manager.update_system_setting(
                            ConfigPath::SystemFlyCameraMode,
                            "fps".into(),
                        );
                    }
                });

                let mut sensitivity = config_manager.system_settings().fly_mouse_sensitivity;
                ui.horizontal(|ui| {
                    ui.label(t!("view.fly_sensitivity"));
                    if ui.add(super::VkbSlider::new(&mut sensitivity, 0.0005..=0.05)
                            .logarithmic(true)
                            ).changed() {
                        let _ = config_manager.update_system_setting(
                            ConfigPath::SystemFlyMouseSensitivity,
                            sensitivity.into(),
                        );
                    }
                });

                let mut speed = config_manager.system_settings().fly_move_speed;
                ui.horizontal(|ui| {
                    ui.label(t!("view.fly_move_speed"));
                    if ui.add(super::VkbSlider::new(&mut speed, 0.05..=20.0)
                            .logarithmic(true)
                            ).changed() {
                        let _ = config_manager.update_system_setting(
                            ConfigPath::SystemFlyMoveSpeed,
                            speed.into(),
                        );
                    }
                });

                let mut sprint = config_manager.system_settings().fly_sprint_multiplier;
                ui.horizontal(|ui| {
                    ui.label(t!("view.fly_sprint_multiplier"));
                    if ui.add(super::VkbSlider::new(&mut sprint, 1.0..=20.0)
                            ).changed() {
                        let _ = config_manager.update_system_setting(
                            ConfigPath::SystemFlySprintMultiplier,
                            sprint.into(),
                        );
                    }
                });

                let mut invert_y = config_manager.system_settings().fly_invert_y;
                if ui.checkbox(&mut invert_y, t!("view.fly_invert_y").as_ref()).changed() {
                    let _ = config_manager.update_system_setting(
                        ConfigPath::SystemFlyInvertY,
                        invert_y.into(),
                    );
                }
            });

        // Preserve Z — JWildfire's `preserve_z` flag. Defaults to
        // off (Apo/JWF default) so flames with variations that scale
        // Z by >1 (e.g. spherical at high weight) don't explode and
        // poison the camera transform via `0·∞ = NaN`.
        let mut preserve_z = config.flame.preserve_z;
        let response = ui.checkbox(&mut preserve_z, t!("view.preserve_z").as_ref())
            .on_hover_text(t!("view.tooltip_preserve_z"));
        if response.changed() {
            let _ = config_manager.update_param(
                ConfigPath::PreserveZ,
                preserve_z.into(),
            );
        }

        // Depth Effects (collapsible, hidden by default)
        ui.separator();
        egui::CollapsingHeader::new(t!("view.depth_effects").as_ref())
            .default_open(false)
            .show(ui, |ui| {
                // Depth of Field controls
                ui.label(t!("view.depth_of_field"));

                let mut dof_focus = config.dof_focus_distance;
                let response = ui.add(
                    super::VkbSlider::new(&mut dof_focus, -5.0..=5.0)
                        .text(t!("view.dof_focus_distance").as_ref())
                ).on_hover_text(t!("view.tooltip_dof_focus_distance"));
                if response.changed() {
                    let _ = config_manager.update_param(
                        ConfigPath::DofFocusDistance,
                        dof_focus.into()
                    );
                }

                let mut dof_blur = config.dof_blur_strength;
                let response = ui.add(
                    super::VkbSlider::new(&mut dof_blur, 0.0..=1.0)
                        .text(t!("view.dof_blur_strength").as_ref())
                        .step_by(0.001)
                ).on_hover_text(t!("view.tooltip_dof_blur_strength"));
                if response.changed() {
                    let _ = config_manager.update_param(
                        ConfigPath::DofBlurStrength,
                        dof_blur.into()
                    );
                }

                // Depth Fog controls
                ui.add_space(8.0);
                ui.label(t!("view.fog_section").as_ref());

                let mut fog_strength = config.fog_strength;
                let response = ui.add(
                    super::VkbSlider::new(&mut fog_strength, 0.0..=5.0)
                        .text(t!("view.fog_strength").as_ref())
                ).on_hover_text(t!("view.tooltip_fog_strength"));
                if response.changed() {
                    let _ = config_manager.update_param(
                        ConfigPath::FogStrength,
                        fog_strength.into()
                    );
                }

                let mut fog_start = config.fog_start;
                let response = ui.add(
                    super::VkbSlider::new(&mut fog_start, -5.0..=5.0)
                        .text(t!("view.fog_start").as_ref())
                ).on_hover_text(t!("view.tooltip_fog_start"));
                if response.changed() {
                    let _ = config_manager.update_param(
                        ConfigPath::FogStart,
                        fog_start.into()
                    );
                }

                // Density weighting by depth
                ui.add_space(8.0);
                ui.label(t!("view.depth_density_section").as_ref());

                // Depth-density compensation — only meaningful with
                // perspective > 0 (the weight collapses to 1 in
                // orthographic).
                let mut depth_comp = config.flame.depth_density_compensation;
                let response = ui.add(
                    super::VkbSlider::new(&mut depth_comp, 0.0..=1.0)
                        .text(t!("view.depth_density_compensation").as_ref())
                        .step_by(0.01)
                ).on_hover_text(t!("view.tooltip_depth_density_compensation"));
                if response.changed() {
                    let _ = config_manager.update_param(
                        ConfigPath::DepthDensityCompensation,
                        depth_comp.into()
                    );
                }

                // Far density fade: thins far samples' density with a
                // Gaussian falloff past the start depth (unlike fog,
                // which recolors at full density). Our own extension.
                ui.add_space(8.0);
                ui.label(t!("view.far_density_fade_section").as_ref());

                let mut far_fade = config.flame.far_density_fade;
                let response = ui.add(
                    super::VkbSlider::new(&mut far_fade, 0.0..=5.0)
                        .text(t!("view.far_density_fade").as_ref())
                        .step_by(0.01)
                ).on_hover_text(t!("view.tooltip_far_density_fade"));
                if response.changed() {
                    let _ = config_manager.update_param(
                        ConfigPath::FarDensityFade,
                        far_fade.into()
                    );
                }

                let mut far_fade_start = config.flame.far_density_fade_start;
                let response = ui.add(
                    super::VkbSlider::new(&mut far_fade_start, -5.0..=5.0)
                        .text(t!("view.far_density_fade_start").as_ref())
                ).on_hover_text(t!("view.tooltip_far_density_fade_start"));
                if response.changed() {
                    let _ = config_manager.update_param(
                        ConfigPath::FarDensityFadeStart,
                        far_fade_start.into()
                    );
                }
            });
    }

    ui.separator();

    render_post_symmetry_section(ui, config_manager, &config);

    ui.separator();

    if ui.button(t!("view.reset").as_ref()).clicked() {
        let _ = config_manager.update_batch(
            vec![
                (ConfigPath::Zoom, 1.0.into()),
                (ConfigPath::Pan, (0.0, 0.0).into()),
                (ConfigPath::Rotation, 0.0.into()),
                (ConfigPath::CameraRotationX, 0.0.into()),
                (ConfigPath::CameraRotationY, 0.0.into()),
                (ConfigPath::CameraBank, 0.0.into()),
                (ConfigPath::CameraX, 0.0.into()),
                (ConfigPath::CameraY, 0.0.into()),
                (ConfigPath::CameraZ, 0.0.into()),
                (ConfigPath::DofFocusDistance, crate::config::DEFAULT_DOF_FOCUS_DISTANCE.into()),
                (ConfigPath::DofBlurStrength, crate::config::DEFAULT_DOF_BLUR_STRENGTH.into()),
                (ConfigPath::FogStrength, crate::config::DEFAULT_FOG_STRENGTH.into()),
                (ConfigPath::FogStart, crate::config::DEFAULT_FOG_START.into()),
            ],
            "history.action.reset_view".to_string()
        );
    }
}

/// Post-symmetry section. Type dropdown + the per-mode controls
/// gated by which axis-vs-Point is active. JWildfire-compat: the
/// values round-trip through `flame.post_symmetry` and the GPU
/// HAS_POST_SYMMETRY gate updates automatically when the type
/// changes (forces a shader rebuild via ShaderConstants).
fn render_post_symmetry_section(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    config: &FractalConfig,
) {
    use crate::config::slider::LazyUndoUi;
    let _ = config_manager;

    let ps = &config.flame.post_symmetry;
    let current_ty = ps.ty;

    ui.label(t!("view.post_symmetry")).on_hover_text(t!("view.tooltip_post_symmetry"));

    // Type dropdown.
    let type_label = match current_ty {
        PostSymmetryType::None => t!("view.post_symmetry_none"),
        PostSymmetryType::XAxis => t!("view.post_symmetry_x_axis"),
        PostSymmetryType::YAxis => t!("view.post_symmetry_y_axis"),
        PostSymmetryType::Point => t!("view.post_symmetry_point"),
    };
    egui::ComboBox::from_label(t!("view.post_symmetry_type").as_ref())
        .selected_text(type_label)
        .show_ui(ui, |ui| {
            for (ty, lbl) in [
                (PostSymmetryType::None, t!("view.post_symmetry_none")),
                (PostSymmetryType::XAxis, t!("view.post_symmetry_x_axis")),
                (PostSymmetryType::YAxis, t!("view.post_symmetry_y_axis")),
                (PostSymmetryType::Point, t!("view.post_symmetry_point")),
            ] {
                if ui.selectable_label(current_ty == ty, lbl).clicked() && current_ty != ty {
                    let _ = config_manager.update_param(
                        ConfigPath::PostSymmetryType,
                        (ty.as_u32() as i32).into(),
                    );
                }
            }
        });

    if current_ty == PostSymmetryType::None {
        return;
    }

    // Center always shown (relevant to every non-None mode).
    ui.horizontal(|ui| {
        ui.label(t!("view.post_symmetry_center"));
        let _ = ui.lazy_drag(config_manager, ConfigPath::PostSymmetryCenterX, 0.01, "X");
        let _ = ui.lazy_drag(config_manager, ConfigPath::PostSymmetryCenterY, 0.01, "Y");
    });

    match current_ty {
        PostSymmetryType::XAxis | PostSymmetryType::YAxis => {
            // Axis modes: distance pans the mirror along the axis,
            // rotation pre-rotates around the center.
            ui.horizontal(|ui| {
                ui.label(t!("view.post_symmetry_distance"))
                    .on_hover_text(t!("view.tooltip_post_symmetry_distance"));
                let _ = ui.lazy_drag(config_manager, ConfigPath::PostSymmetryDistance, 0.01, "");
            });
            ui.horizontal(|ui| {
                ui.label(t!("view.post_symmetry_rotation"))
                    .on_hover_text(t!("view.tooltip_post_symmetry_rotation"));
                let _ = ui.lazy_drag(config_manager, ConfigPath::PostSymmetryRotation, 0.5, "°");
            });
        }
        PostSymmetryType::Point => {
            // Point mode: just order. Distance/rotation are ignored.
            ui.horizontal(|ui| {
                ui.label(t!("view.post_symmetry_order"))
                    .on_hover_text(t!("view.tooltip_post_symmetry_order"));
                let mut order = ps.order as i32;
                let response = ui.add(egui::Slider::new(&mut order, 1..=32));
                if response.changed() {
                    let _ = config_manager.update_param(
                        ConfigPath::PostSymmetryOrder,
                        order.into(),
                    );
                }
            });
        }
        PostSymmetryType::None => unreachable!(),
    }
}

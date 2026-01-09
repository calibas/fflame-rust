//! Random Generator panel
//!
//! Panel for generating random fractal flames with configurable parameters.

use egui;
use rust_i18n::t;

use crate::scene::randomize::{RandomGeneratorSettings, SymmetryType};
use crate::variations::global_registry;

/// State for the Random Generator panel
pub struct RandomGeneratorPanel {
    /// Current generation settings
    pub settings: RandomGeneratorSettings,
    /// Symmetry order for rotational/dihedral (UI state)
    symmetry_order: u8,
}

impl Default for RandomGeneratorPanel {
    fn default() -> Self {
        Self {
            settings: RandomGeneratorSettings::default(),
            symmetry_order: 3,
        }
    }
}

/// Response from the random generator panel
#[derive(Default)]
pub struct RandomGeneratorResponse {
    /// Generate a single random flame
    pub generate_single: bool,
    /// Generate a batch of random flames
    pub generate_batch: bool,
}

impl RandomGeneratorPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render the panel content
    pub fn render(&mut self, ui: &mut egui::Ui) -> RandomGeneratorResponse {
        let mut response = RandomGeneratorResponse::default();

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Transforms section
            egui::CollapsingHeader::new(t!("random_generator.transforms"))
                .default_open(true)
                .show(ui, |ui| {
                    self.render_transforms_section(ui);
                });

            // Variations section
            egui::CollapsingHeader::new(t!("random_generator.variations"))
                .default_open(true)
                .show(ui, |ui| {
                    self.render_variations_section(ui);
                });

            // Affine section
            egui::CollapsingHeader::new(t!("random_generator.affine"))
                .default_open(false)
                .show(ui, |ui| {
                    self.render_affine_section(ui);
                });

            // Color & Weight section
            egui::CollapsingHeader::new(t!("random_generator.color_weight"))
                .default_open(false)
                .show(ui, |ui| {
                    self.render_color_weight_section(ui);
                });

            // Symmetry section
            egui::CollapsingHeader::new(t!("random_generator.symmetry"))
                .default_open(true)
                .show(ui, |ui| {
                    self.render_symmetry_section(ui);
                });

            // 3D section
            egui::CollapsingHeader::new(t!("random_generator.3d_options"))
                .default_open(false)
                .show(ui, |ui| {
                    self.render_3d_section(ui);
                });

            ui.separator();

            // Generate buttons
            ui.horizontal(|ui| {
                if ui.button(t!("random_generator.generate_single").as_ref()).clicked() {
                    response.generate_single = true;
                }
                if ui.button(t!("random_generator.generate_batch").as_ref()).clicked() {
                    response.generate_batch = true;
                }
            });

            // Batch options
            ui.horizontal(|ui| {
                ui.label(t!("random_generator.batch_count"));
                ui.add(egui::DragValue::new(&mut self.settings.batch_count)
                    .range(1..=100)
                    .speed(1));
            });

            // Seed control
            ui.horizontal(|ui| {
                ui.label(t!("random_generator.seed"));
                let mut use_seed = self.settings.seed.is_some();
                if ui.checkbox(&mut use_seed, "").changed() {
                    self.settings.seed = if use_seed { Some(12345) } else { None };
                }
                if let Some(ref mut seed) = self.settings.seed {
                    ui.add(egui::DragValue::new(seed).speed(100));
                    if ui.button("🎲").on_hover_text(t!("random_generator.randomize_seed")).clicked() {
                        *seed = rand::random();
                    }
                }
            });
        });

        response
    }

    fn render_transforms_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(t!("random_generator.count"));
            ui.add(egui::DragValue::new(&mut self.settings.transform_count_min)
                .range(1..=10)
                .speed(0.1));
            ui.label(t!("random_generator.to"));
            ui.add(egui::DragValue::new(&mut self.settings.transform_count_max)
                .range(1..=10)
                .speed(0.1));
        });

        // Ensure min <= max
        if self.settings.transform_count_min > self.settings.transform_count_max {
            self.settings.transform_count_max = self.settings.transform_count_min;
        }

        ui.checkbox(
            &mut self.settings.include_final_transform,
            t!("random_generator.include_final_transform"),
        );
    }

    fn render_variations_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(t!("random_generator.per_transform"));
            ui.add(egui::DragValue::new(&mut self.settings.variations_per_transform_min)
                .range(1..=10)
                .speed(0.1));
            ui.label(t!("random_generator.to"));
            ui.add(egui::DragValue::new(&mut self.settings.variations_per_transform_max)
                .range(1..=10)
                .speed(0.1));
        });

        // Ensure min <= max
        if self.settings.variations_per_transform_min > self.settings.variations_per_transform_max {
            self.settings.variations_per_transform_max = self.settings.variations_per_transform_min;
        }

        ui.horizontal(|ui| {
            ui.label(t!("random_generator.weight_range"));
            ui.add(egui::DragValue::new(&mut self.settings.variation_weight_min)
                .range(0.0..=2.0)
                .speed(0.01)
                .fixed_decimals(2));
            ui.label(t!("random_generator.to"));
            ui.add(egui::DragValue::new(&mut self.settings.variation_weight_max)
                .range(0.0..=2.0)
                .speed(0.01)
                .fixed_decimals(2));
        });

        ui.checkbox(
            &mut self.settings.always_include_linear,
            t!("random_generator.always_include_linear"),
        );

        // Variation selection
        ui.separator();
        ui.label(t!("random_generator.enabled_variations"));

        // Quick selection buttons
        ui.horizontal(|ui| {
            if ui.button(t!("random_generator.select_all").as_ref()).clicked() {
                let registry = global_registry();
                self.settings.enabled_variations = registry.names()
                    .iter()
                    .cloned()
                    .collect();
            }
            if ui.button(t!("random_generator.select_none").as_ref()).clicked() {
                self.settings.enabled_variations.clear();
            }
            if ui.button(t!("random_generator.select_2d").as_ref()).clicked() {
                let registry = global_registry();
                use crate::variations::VariationCategory;
                self.settings.enabled_variations = registry.names()
                    .iter()
                    .filter(|name| {
                        registry.get(name)
                            .map(|info| !matches!(info.category,
                                VariationCategory::Depth3D |
                                VariationCategory::Rotation3D |
                                VariationCategory::Full3D))
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect();
            }
        });

        // Variation checkboxes in a grid
        let registry = global_registry();
        let all_variations: Vec<_> = registry.names().to_vec();

        egui::Grid::new("variation_grid")
            .num_columns(3)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                for (i, name) in all_variations.iter().enumerate() {
                    let mut enabled = self.settings.enabled_variations.contains(name);
                    if ui.checkbox(&mut enabled, name.as_str()).changed() {
                        if enabled {
                            self.settings.enabled_variations.insert(name.clone());
                        } else {
                            self.settings.enabled_variations.remove(name);
                        }
                    }
                    if (i + 1) % 3 == 0 {
                        ui.end_row();
                    }
                }
            });
    }

    fn render_affine_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(t!("random_generator.scale"));
            ui.add(egui::DragValue::new(&mut self.settings.scale_min)
                .range(0.1..=3.0)
                .speed(0.01)
                .fixed_decimals(2));
            ui.label(t!("random_generator.to"));
            ui.add(egui::DragValue::new(&mut self.settings.scale_max)
                .range(0.1..=3.0)
                .speed(0.01)
                .fixed_decimals(2));
        });

        ui.horizontal(|ui| {
            ui.label(t!("random_generator.shear"));
            ui.add(egui::DragValue::new(&mut self.settings.shear_min)
                .range(-2.0..=2.0)
                .speed(0.01)
                .fixed_decimals(2));
            ui.label(t!("random_generator.to"));
            ui.add(egui::DragValue::new(&mut self.settings.shear_max)
                .range(-2.0..=2.0)
                .speed(0.01)
                .fixed_decimals(2));
        });

        ui.horizontal(|ui| {
            ui.label(t!("random_generator.translate"));
            ui.add(egui::DragValue::new(&mut self.settings.translate_min)
                .range(-3.0..=3.0)
                .speed(0.01)
                .fixed_decimals(2));
            ui.label(t!("random_generator.to"));
            ui.add(egui::DragValue::new(&mut self.settings.translate_max)
                .range(-3.0..=3.0)
                .speed(0.01)
                .fixed_decimals(2));
        });

        ui.checkbox(
            &mut self.settings.allow_negative_scale,
            t!("random_generator.allow_negative_scale"),
        );
    }

    fn render_color_weight_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(t!("random_generator.transform_weight"));
            ui.add(egui::DragValue::new(&mut self.settings.weight_min)
                .range(0.1..=3.0)
                .speed(0.01)
                .fixed_decimals(2));
            ui.label(t!("random_generator.to"));
            ui.add(egui::DragValue::new(&mut self.settings.weight_max)
                .range(0.1..=3.0)
                .speed(0.01)
                .fixed_decimals(2));
        });

        ui.checkbox(
            &mut self.settings.distribute_colors_evenly,
            t!("random_generator.distribute_colors_evenly"),
        );

        ui.checkbox(
            &mut self.settings.random_palette,
            t!("random_generator.random_palette"),
        );
    }

    fn render_symmetry_section(&mut self, ui: &mut egui::Ui) {
        // Symmetry type selection
        let current_type = match self.settings.symmetry {
            SymmetryType::None => 0,
            SymmetryType::BilateralHorizontal => 1,
            SymmetryType::BilateralVertical => 2,
            SymmetryType::Rotational(_) => 3,
            SymmetryType::Dihedral(_) => 4,
        };

        let symmetry_names = [
            t!("random_generator.symmetry_none"),
            t!("random_generator.symmetry_bilateral_h"),
            t!("random_generator.symmetry_bilateral_v"),
            t!("random_generator.symmetry_rotational"),
            t!("random_generator.symmetry_dihedral"),
        ];

        let mut selected = current_type;
        egui::ComboBox::from_label(t!("random_generator.symmetry_type"))
            .selected_text(symmetry_names[selected].as_ref())
            .show_ui(ui, |ui| {
                for (i, name) in symmetry_names.iter().enumerate() {
                    ui.selectable_value(&mut selected, i, name.as_ref());
                }
            });

        // Update symmetry type if changed
        if selected != current_type {
            self.settings.symmetry = match selected {
                0 => SymmetryType::None,
                1 => SymmetryType::BilateralHorizontal,
                2 => SymmetryType::BilateralVertical,
                3 => SymmetryType::Rotational(self.symmetry_order),
                4 => SymmetryType::Dihedral(self.symmetry_order),
                _ => SymmetryType::None,
            };
        }

        // Show order control for rotational/dihedral
        if matches!(self.settings.symmetry, SymmetryType::Rotational(_) | SymmetryType::Dihedral(_)) {
            ui.horizontal(|ui| {
                ui.label(t!("random_generator.symmetry_order"));
                if ui.add(egui::DragValue::new(&mut self.symmetry_order)
                    .range(2..=12)
                    .speed(0.1))
                    .changed()
                {
                    // Update the symmetry with new order
                    self.settings.symmetry = match self.settings.symmetry {
                        SymmetryType::Rotational(_) => SymmetryType::Rotational(self.symmetry_order),
                        SymmetryType::Dihedral(_) => SymmetryType::Dihedral(self.symmetry_order),
                        other => other,
                    };
                }
            });

            // Show how many transforms will be added
            let count = self.settings.symmetry.transform_count();
            ui.label(t!("random_generator.symmetry_transforms_added", count = count));
        }
    }

    fn render_3d_section(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(
            &mut self.settings.enable_3d,
            t!("random_generator.enable_3d"),
        );

        if self.settings.enable_3d {
            ui.checkbox(
                &mut self.settings.include_3d_variations,
                t!("random_generator.include_3d_variations"),
            );

            ui.horizontal(|ui| {
                ui.label(t!("random_generator.perspective"));
                ui.add(egui::DragValue::new(&mut self.settings.perspective_min)
                    .range(0.0..=10.0)
                    .speed(0.1)
                    .fixed_decimals(1));
                ui.label(t!("random_generator.to"));
                ui.add(egui::DragValue::new(&mut self.settings.perspective_max)
                    .range(0.0..=10.0)
                    .speed(0.1)
                    .fixed_decimals(1));
            });
        }
    }
}

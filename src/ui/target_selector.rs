//! Hierarchical target selector for animation tracks
//!
//! Provides a tree-based UI for selecting animatable parameters (ConfigPath).
//! Supports collapsible categories, search filtering, and dynamic content
//! based on the current flame configuration.

use egui::{CollapsingHeader, ScrollArea, TextEdit, Ui};
use crate::config::delta::{AffineParam, ConfigPath};
use crate::config::FractalConfig;
use crate::effects::global_effect_registry;
use crate::scene::transforms::Flame;
use crate::variations::global_registry;

/// State for the target selector widget
#[derive(Default)]
pub struct TargetSelectorState {
    /// Search filter text
    pub search_filter: String,
    /// Currently expanded categories (for persistence)
    pub expanded: std::collections::HashSet<String>,
}

impl TargetSelectorState {
    pub fn new() -> Self {
        Self {
            search_filter: String::new(),
            expanded: std::collections::HashSet::new(),
        }
    }
}

/// Category of animatable parameters
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TargetCategory {
    View,
    Color,
    ToneMapping,
    Rendering,
    Effects,
    Xaos,
    Transform(usize),
    FinalTransform,
}

impl TargetCategory {
    fn label(&self) -> String {
        match self {
            TargetCategory::View => "View".to_string(),
            TargetCategory::Color => "Color".to_string(),
            TargetCategory::ToneMapping => "Tone Mapping".to_string(),
            TargetCategory::Rendering => "Rendering".to_string(),
            TargetCategory::Effects => "Effects".to_string(),
            TargetCategory::Xaos => "Xaos".to_string(),
            TargetCategory::Transform(i) => format!("Transform {}", i + 1),
            TargetCategory::FinalTransform => "Final Transform".to_string(),
        }
    }

    fn id(&self) -> String {
        match self {
            TargetCategory::View => "view".to_string(),
            TargetCategory::Color => "color".to_string(),
            TargetCategory::ToneMapping => "tonemapping".to_string(),
            TargetCategory::Rendering => "rendering".to_string(),
            TargetCategory::Effects => "effects".to_string(),
            TargetCategory::Xaos => "xaos".to_string(),
            TargetCategory::Transform(i) => format!("transform_{}", i),
            TargetCategory::FinalTransform => "final_transform".to_string(),
        }
    }
}

/// A selectable target item
struct TargetItem {
    path: ConfigPath,
    label: String,
}

impl TargetItem {
    fn new(path: ConfigPath, label: &str) -> Self {
        Self {
            path,
            label: label.to_string(),
        }
    }
}

/// Render the hierarchical target selector
///
/// Returns `Some(ConfigPath)` if a target was selected, `None` otherwise.
pub fn render_target_selector(
    ui: &mut Ui,
    state: &mut TargetSelectorState,
    flame: &Flame,
    config: &FractalConfig,
    current_selection: Option<&str>,
) -> Option<ConfigPath> {
    let mut selected: Option<ConfigPath> = None;

    // Search filter
    ui.horizontal(|ui| {
        ui.label("🔍");
        ui.add(
            TextEdit::singleline(&mut state.search_filter)
                .hint_text("Search parameters...")
                .desired_width(ui.available_width()),
        );
    });

    ui.add_space(4.0);

    // Scrollable content
    ScrollArea::vertical()
        .max_height(300.0)
        .show(ui, |ui| {
            let filter = state.search_filter.to_lowercase();
            let has_filter = !filter.is_empty();

            // View category
            if let Some(path) = render_category(
                ui,
                state,
                TargetCategory::View,
                &get_view_items(),
                &filter,
                has_filter,
                current_selection,
            ) {
                selected = Some(path);
            }

            // Color category
            if let Some(path) = render_category(
                ui,
                state,
                TargetCategory::Color,
                &get_color_items(),
                &filter,
                has_filter,
                current_selection,
            ) {
                selected = Some(path);
            }

            // Tone Mapping category
            if let Some(path) = render_category(
                ui,
                state,
                TargetCategory::ToneMapping,
                &get_tonemapping_items(),
                &filter,
                has_filter,
                current_selection,
            ) {
                selected = Some(path);
            }

            // Rendering category
            if let Some(path) = render_category(
                ui,
                state,
                TargetCategory::Rendering,
                &get_rendering_items(),
                &filter,
                has_filter,
                current_selection,
            ) {
                selected = Some(path);
            }

            // Effects category (dynamic based on current effects)
            let effects_items = get_effects_items(config);
            if !effects_items.is_empty() {
                if let Some(path) = render_category(
                    ui,
                    state,
                    TargetCategory::Effects,
                    &effects_items,
                    &filter,
                    has_filter,
                    current_selection,
                ) {
                    selected = Some(path);
                }
            }

            // Xaos category (only shown if flame has xaos enabled or multiple transforms)
            let xaos_items = get_xaos_items(flame);
            if !xaos_items.is_empty() {
                if let Some(path) = render_category(
                    ui,
                    state,
                    TargetCategory::Xaos,
                    &xaos_items,
                    &filter,
                    has_filter,
                    current_selection,
                ) {
                    selected = Some(path);
                }
            }

            // Transform categories (one per transform)
            for i in 0..flame.transforms.len() {
                let items = get_transform_items(i, &flame.transforms[i]);
                if let Some(path) = render_category(
                    ui,
                    state,
                    TargetCategory::Transform(i),
                    &items,
                    &filter,
                    has_filter,
                    current_selection,
                ) {
                    selected = Some(path);
                }
            }

            // Final Transform category (if enabled or for potential enabling)
            let final_items = get_final_transform_items(flame.final_transform.as_ref());
            if let Some(path) = render_category(
                ui,
                state,
                TargetCategory::FinalTransform,
                &final_items,
                &filter,
                has_filter,
                current_selection,
            ) {
                selected = Some(path);
            }
        });

    selected
}

/// Render a single category with its items
fn render_category(
    ui: &mut Ui,
    state: &mut TargetSelectorState,
    category: TargetCategory,
    items: &[TargetItem],
    filter: &str,
    has_filter: bool,
    current_selection: Option<&str>,
) -> Option<ConfigPath> {
    let mut selected: Option<ConfigPath> = None;

    // Filter items if search is active
    let filtered_items: Vec<&TargetItem> = if has_filter {
        items
            .iter()
            .filter(|item| item.label.to_lowercase().contains(filter))
            .collect()
    } else {
        items.iter().collect()
    };

    // Skip empty categories when filtering
    if has_filter && filtered_items.is_empty() {
        return None;
    }

    let category_id = category.id();
    let is_expanded = state.expanded.contains(&category_id) || has_filter;

    let header = CollapsingHeader::new(category.label())
        .id_salt(&category_id)
        .default_open(is_expanded)
        .show(ui, |ui| {
            for item in filtered_items {
                let key = item.path.to_string_key();
                let is_selected = current_selection.map_or(false, |s| s == key);

                let response = ui.selectable_label(is_selected, &item.label);
                if response.clicked() {
                    selected = Some(item.path.clone());
                }
            }
        });

    // Track expansion state
    if header.header_response.clicked() {
        if state.expanded.contains(&category_id) {
            state.expanded.remove(&category_id);
        } else {
            state.expanded.insert(category_id);
        }
    }

    selected
}

/// Get view parameter items
fn get_view_items() -> Vec<TargetItem> {
    vec![
        TargetItem::new(ConfigPath::Zoom, "Zoom"),
        TargetItem::new(ConfigPath::PanX, "Pan X"),
        TargetItem::new(ConfigPath::PanY, "Pan Y"),
        TargetItem::new(ConfigPath::Rotation, "Rotation"),
        TargetItem::new(ConfigPath::CameraRotationX, "Camera Pitch"),
        TargetItem::new(ConfigPath::CameraRotationY, "Camera Yaw"),
        TargetItem::new(ConfigPath::CameraZ, "Camera Z"),
        TargetItem::new(ConfigPath::DofFocusDistance, "DOF Focus Distance"),
        TargetItem::new(ConfigPath::DofBlurStrength, "DOF Blur Strength"),
        TargetItem::new(ConfigPath::FogStrength, "Fog Density"),
        TargetItem::new(ConfigPath::FogStart, "Fog Start"),
    ]
}

/// Get color parameter items
fn get_color_items() -> Vec<TargetItem> {
    vec![
        TargetItem::new(ConfigPath::PaletteRotation, "Palette Rotation"),
        TargetItem::new(ConfigPath::PaletteSqueeze, "Palette Squeeze"),
        TargetItem::new(ConfigPath::SpeedFactor, "Speed Blend Factor"),
        TargetItem::new(ConfigPath::BackgroundColorR, "Background Red"),
        TargetItem::new(ConfigPath::BackgroundColorG, "Background Green"),
        TargetItem::new(ConfigPath::BackgroundColorB, "Background Blue"),
    ]
}

/// Get tone mapping parameter items
fn get_tonemapping_items() -> Vec<TargetItem> {
    vec![
        TargetItem::new(ConfigPath::Exposure, "Exposure"),
        TargetItem::new(ConfigPath::Gamma, "Gamma"),
        TargetItem::new(ConfigPath::GammaThreshold, "Gamma Threshold"),
        TargetItem::new(ConfigPath::Brightness, "Brightness"),
        TargetItem::new(ConfigPath::Vibrancy, "Vibrancy"),
        TargetItem::new(ConfigPath::Saturation, "Saturation"),
        TargetItem::new(ConfigPath::HueShift, "Hue Shift"),
        TargetItem::new(ConfigPath::DensityScale, "Density Scale"),
        TargetItem::new(ConfigPath::LevelsLow, "Levels Low"),
        TargetItem::new(ConfigPath::LevelsHigh, "Levels High"),
        TargetItem::new(ConfigPath::LevelsGamma, "Levels Midtones"),
    ]
}

/// Get rendering parameter items
fn get_rendering_items() -> Vec<TargetItem> {
    vec![
        TargetItem::new(ConfigPath::BlendFactor, "Blend Factor"),
        TargetItem::new(ConfigPath::HistogramColorScale, "Histogram Color Scale"),
        TargetItem::new(ConfigPath::PerspectiveStrength, "Perspective Strength"),
        TargetItem::new(ConfigPath::SoloTransform, "Solo Transform"),
    ]
}

/// Get xaos parameter items (dynamic based on number of transforms)
fn get_xaos_items(flame: &Flame) -> Vec<TargetItem> {
    let mut items = Vec::new();
    let num_transforms = flame.transforms.len();

    // Only show xaos items if there are transforms
    if num_transforms == 0 {
        return items;
    }

    // Add an item for each xaos cell (src → dst transition weight)
    for src in 0..num_transforms {
        for dst in 0..num_transforms {
            items.push(TargetItem::new(
                ConfigPath::Xaos { src, dst },
                &format!("T{} > T{}", src + 1, dst + 1),
            ));
        }
    }

    items
}

/// Get effects parameter items (dynamic based on current effects in config)
fn get_effects_items(config: &FractalConfig) -> Vec<TargetItem> {
    let mut items = Vec::new();
    let registry = global_effect_registry();

    // Density Effects
    for (index, effect) in config.density_effects.iter().enumerate() {
        let effect_name = if let Some(info) = registry.get(&effect.effect_type) {
            info.translated_name()
        } else {
            capitalize_first(&effect.effect_type)
        };

        // Enabled toggle
        items.push(TargetItem::new(
            ConfigPath::DensityEffectEnabled { index },
            &format!("{} (Density) → Enabled", effect_name),
        ));

        // Parameters
        if let Some(info) = registry.get(&effect.effect_type) {
            for param in &info.parameters {
                items.push(TargetItem::new(
                    ConfigPath::DensityEffectParam {
                        index,
                        param: param.name.clone(),
                    },
                    &format!("{} (Density) → {}", effect_name, info.translated_param_name(&param.name)),
                ));
            }
        }
    }

    // Color Effects
    for (index, effect) in config.color_effects.iter().enumerate() {
        let effect_name = if let Some(info) = registry.get(&effect.effect_type) {
            info.translated_name()
        } else {
            capitalize_first(&effect.effect_type)
        };

        // Enabled toggle
        items.push(TargetItem::new(
            ConfigPath::ColorEffectEnabled { index },
            &format!("{} (Color) → Enabled", effect_name),
        ));

        // Parameters
        if let Some(info) = registry.get(&effect.effect_type) {
            for param in &info.parameters {
                items.push(TargetItem::new(
                    ConfigPath::ColorEffectParam {
                        index,
                        param: param.name.clone(),
                    },
                    &format!("{} (Color) → {}", effect_name, info.translated_param_name(&param.name)),
                ));
            }
        }
    }

    items
}

/// Get transform parameter items (dynamic based on active variations)
fn get_transform_items(index: usize, transform: &crate::scene::transforms::Transform) -> Vec<TargetItem> {
    let mut items = Vec::new();

    // Properties
    items.push(TargetItem::new(
        ConfigPath::TransformWeight { index },
        "Weight",
    ));
    items.push(TargetItem::new(
        ConfigPath::TransformColor { index },
        "Color",
    ));
    items.push(TargetItem::new(
        ConfigPath::TransformColorSpeed { index },
        "Color Speed",
    ));
    items.push(TargetItem::new(
        ConfigPath::TransformOpacity { index },
        "Opacity",
    ));

    // High-level transforms
    items.push(TargetItem::new(
        ConfigPath::TransformOriginX { index },
        "Origin X",
    ));
    items.push(TargetItem::new(
        ConfigPath::TransformOriginY { index },
        "Origin Y",
    ));
    items.push(TargetItem::new(
        ConfigPath::TransformRotation { index },
        "Rotation",
    ));
    items.push(TargetItem::new(
        ConfigPath::TransformScale { index },
        "Scale",
    ));

    // Affine parameters
    for param in [
        AffineParam::A,
        AffineParam::B,
        AffineParam::C,
        AffineParam::D,
        AffineParam::E,
        AffineParam::F,
        AffineParam::G,
    ] {
        items.push(TargetItem::new(
            ConfigPath::TransformAffine { index, param },
            &format!("Affine {}", param.to_char()),
        ));
    }

    // Active variations and their parameters
    let registry = global_registry();
    for (var_name, weight) in &transform.variations {
        if *weight != 0.0 {
            // Variation weight
            items.push(TargetItem::new(
                ConfigPath::TransformVariation {
                    index,
                    variation: var_name.clone(),
                },
                &format!("{}", capitalize_first(var_name)),
            ));

            // Variation parameters (if any)
            if let Some(info) = registry.get(var_name) {
                for param in &info.parameters {
                    items.push(TargetItem::new(
                        ConfigPath::TransformVariationParam {
                            index,
                            variation: var_name.clone(),
                            param: param.name.clone(),
                        },
                        &format!("{} → {}", capitalize_first(var_name), &param.display_name),
                    ));
                }
            }
        }
    }

    items
}

/// Get final transform parameter items
fn get_final_transform_items(final_transform: Option<&crate::scene::transforms::Transform>) -> Vec<TargetItem> {
    let mut items = Vec::new();

    // Enabled toggle
    items.push(TargetItem::new(
        ConfigPath::FinalTransformEnabled,
        "Enabled",
    ));

    // High-level transforms
    items.push(TargetItem::new(ConfigPath::FinalTransformOriginX, "Origin X"));
    items.push(TargetItem::new(ConfigPath::FinalTransformOriginY, "Origin Y"));
    items.push(TargetItem::new(ConfigPath::FinalTransformRotation, "Rotation"));
    items.push(TargetItem::new(ConfigPath::FinalTransformScale, "Scale"));

    // Affine parameters
    for param in [
        AffineParam::A,
        AffineParam::B,
        AffineParam::C,
        AffineParam::D,
        AffineParam::E,
        AffineParam::F,
        AffineParam::G,
    ] {
        items.push(TargetItem::new(
            ConfigPath::FinalTransformAffine { param },
            &format!("Affine {}", param.to_char()),
        ));
    }

    // Active variations (if final transform exists)
    if let Some(transform) = final_transform {
        let registry = global_registry();
        for (var_name, weight) in &transform.variations {
            if *weight != 0.0 {
                items.push(TargetItem::new(
                    ConfigPath::FinalTransformVariation {
                        variation: var_name.clone(),
                    },
                    &format!("{}", capitalize_first(var_name)),
                ));

                // Variation parameters
                if let Some(info) = registry.get(var_name) {
                    for param in &info.parameters {
                        items.push(TargetItem::new(
                            ConfigPath::FinalTransformVariationParam {
                                variation: var_name.clone(),
                                param: param.name.clone(),
                            },
                            &format!("{} → {}", capitalize_first(var_name), &param.display_name),
                        ));
                    }
                }
            }
        }
    }

    items
}

/// Capitalize first letter of a string
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Get display name for a ConfigPath
pub fn config_path_display_name(path: &ConfigPath) -> String {
    // Use the Display implementation which provides good names
    format!("{}", path)
}

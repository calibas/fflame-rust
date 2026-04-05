//! Path Editor panel for managing path filters
//!
//! Allows users to add, edit, and delete path filters that block specific
//! transform sequences during fractal iteration.

use crate::gpu::buffers::GpuPathFilter;
use rust_i18n::t;

/// State for a single filter being edited
#[derive(Clone, Debug)]
pub struct FilterEntry {
    /// Pattern as a comma-separated string of transform indices (e.g., "0,1,2")
    pub pattern_text: String,
    /// For exact depth filters: the iteration depth at which to match
    /// 0 = suffix filter (matches at any depth)
    pub depth: u32,
    /// Whether this filter is enabled
    pub enabled: bool,
}

impl Default for FilterEntry {
    fn default() -> Self {
        Self {
            pattern_text: String::new(),
            depth: 0,
            enabled: true,
        }
    }
}

impl FilterEntry {
    /// Parse the pattern text into a vector of transform indices
    fn parse_pattern(&self) -> Option<Vec<u32>> {
        if self.pattern_text.trim().is_empty() {
            return None;
        }

        let mut pattern = Vec::new();
        for part in self.pattern_text.split(',') {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }
            match trimmed.parse::<u32>() {
                Ok(idx) if idx < 16 => pattern.push(idx),
                _ => return None, // Invalid index
            }
        }

        if pattern.is_empty() || pattern.len() > 8 {
            return None;
        }

        Some(pattern)
    }

    /// Convert to GpuPathFilter if valid
    pub fn to_gpu_filter(&self) -> Option<GpuPathFilter> {
        if !self.enabled {
            return None;
        }

        let pattern = self.parse_pattern()?;

        if self.depth == 0 {
            Some(GpuPathFilter::suffix(&pattern))
        } else {
            // For exact depth, pattern length must not exceed depth
            if pattern.len() as u32 > self.depth {
                return None;
            }
            Some(GpuPathFilter::at_depth(&pattern, self.depth))
        }
    }

    /// Check if the current input is valid
    pub fn is_valid(&self) -> bool {
        if !self.enabled {
            return true; // Disabled entries are always "valid"
        }

        if let Some(pattern) = self.parse_pattern() {
            if self.depth > 0 {
                // For exact depth, pattern length must not exceed depth
                pattern.len() as u32 <= self.depth
            } else {
                true
            }
        } else {
            false
        }
    }

    /// Get a human-readable filter type description
    pub fn filter_type_label(&self) -> String {
        if self.depth == 0 {
            t!("path_editor.suffix").to_string()
        } else {
            t!("path_editor.at_depth").to_string()
        }
    }
}

/// State for the Path Editor panel
#[derive(Default)]
pub struct PathEditorState {
    /// List of filter entries
    pub filters: Vec<FilterEntry>,
    /// New filter being added
    pub new_filter: FilterEntry,
    /// Whether filters have changed since last apply
    pub dirty: bool,
}

impl PathEditorState {
    /// Create a new empty state
    pub fn new() -> Self {
        Self::default()
    }

    /// Load filters from the renderer's current state
    pub fn load_from_renderer(&mut self, gpu_filters: &[GpuPathFilter]) {
        self.filters.clear();
        for filter in gpu_filters {
            if filter.length == 0 {
                continue; // Skip empty filters
            }

            // Unpack the pattern
            let mut pattern_parts = Vec::new();
            for i in 0..filter.length {
                let idx = (filter.pattern >> (i * 4)) & 0xF;
                pattern_parts.push(idx.to_string());
            }

            self.filters.push(FilterEntry {
                pattern_text: pattern_parts.join(","),
                depth: filter.depth,
                enabled: true,
            });
        }
        self.dirty = false;
    }

    /// Convert all enabled filters to GPU format
    pub fn to_gpu_filters(&self) -> Vec<GpuPathFilter> {
        self.filters
            .iter()
            .filter_map(|f| f.to_gpu_filter())
            .collect()
    }

    /// Add the new filter to the list
    pub fn add_new_filter(&mut self) {
        if self.new_filter.is_valid() && !self.new_filter.pattern_text.trim().is_empty() {
            self.filters.push(self.new_filter.clone());
            self.new_filter = FilterEntry::default();
            self.dirty = true;
        }
    }

    /// Remove a filter by index
    pub fn remove_filter(&mut self, index: usize) {
        if index < self.filters.len() {
            self.filters.remove(index);
            self.dirty = true;
        }
    }
}

/// Response from the path editor panel
#[derive(Default)]
pub struct PathEditorResponse {
    /// New set of filters to apply (if changed)
    pub filters_changed: Option<Vec<GpuPathFilter>>,
}

/// Render the path editor content
pub fn render_path_editor_content(
    ui: &mut egui::Ui,
    state: &mut PathEditorState,
    num_transforms: usize,
) -> PathEditorResponse {
    let mut response = PathEditorResponse::default();

    // Experimental feature warning
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(t!("path_editor.experimental")).strong().color(egui::Color32::from_rgb(255, 180, 0)));
    });
    ui.label(egui::RichText::new(t!("path_editor.experimental_hint").as_ref()).small().italics());
    ui.add_space(8.0);

    ui.heading(t!("path_editor.heading"));
    ui.add_space(4.0);

    ui.label(t!("path_editor.description"));
    ui.label(egui::RichText::new(t!("path_editor.suffix_hint").as_ref()).small().color(ui.visuals().weak_text_color()));
    ui.label(egui::RichText::new(t!("path_editor.at_depth_hint").as_ref()).small().color(ui.visuals().weak_text_color()));

    ui.add_space(8.0);
    ui.separator();

    // Existing filters
    ui.add_space(4.0);
    ui.label(egui::RichText::new(t!("path_editor.active_filters").as_ref()).strong());

    if state.filters.is_empty() {
        ui.label(egui::RichText::new(t!("path_editor.no_filters").as_ref()).italics().color(ui.visuals().weak_text_color()));
    } else {
        let mut to_remove = None;

        egui::ScrollArea::vertical()
            .max_height(200.0)
            .show(ui, |ui| {
                for (i, filter) in state.filters.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        // Enable/disable checkbox
                        if ui.checkbox(&mut filter.enabled, "").changed() {
                            state.dirty = true;
                        }

                        // Filter type indicator
                        let type_label = filter.filter_type_label();
                        ui.label(egui::RichText::new(format!("[{}]", type_label))
                            .small()
                            .color(if filter.depth == 0 {
                                egui::Color32::from_rgb(100, 180, 255)
                            } else {
                                egui::Color32::from_rgb(255, 180, 100)
                            }));

                        // Pattern display/edit
                        let pattern_response = ui.add(
                            egui::TextEdit::singleline(&mut filter.pattern_text)
                                .desired_width(100.0)
                                .hint_text("0,1,2")
                        );
                        super::vkb_sync(ui, &pattern_response, &filter.pattern_text);
                        if pattern_response.changed() {
                            state.dirty = true;
                        }

                        // Depth for exact-depth filters
                        if filter.depth > 0 {
                            ui.label("@");
                            let mut depth_i32 = filter.depth as i32;
                            if ui.add(super::VkbDragValue::new(&mut depth_i32).range(1..=32).speed(0.1)).changed() {
                                filter.depth = depth_i32.max(1) as u32;
                                state.dirty = true;
                            }
                        }

                        // Validation indicator
                        if !filter.is_valid() {
                            ui.label(egui::RichText::new("⚠").color(egui::Color32::YELLOW));
                        }

                        // Delete button
                        if ui.small_button("🗑").clicked() {
                            to_remove = Some(i);
                        }
                    });
                }
            });

        if let Some(i) = to_remove {
            state.remove_filter(i);
        }
    }

    ui.add_space(8.0);
    ui.separator();

    // Add new filter section
    ui.add_space(4.0);
    ui.label(egui::RichText::new(t!("path_editor.add_new_filter").as_ref()).strong());

    ui.horizontal(|ui| {
        ui.label(t!("path_editor.type"));
        if ui.selectable_label(state.new_filter.depth == 0, t!("path_editor.suffix").as_ref()).clicked() {
            state.new_filter.depth = 0;
        }
        if ui.selectable_label(state.new_filter.depth > 0, t!("path_editor.at_depth").as_ref()).clicked() {
            if state.new_filter.depth == 0 {
                state.new_filter.depth = 2; // Default depth
            }
        }
    });

    ui.horizontal(|ui| {
        ui.label(t!("path_editor.pattern"));
        let r = ui.add(
            egui::TextEdit::singleline(&mut state.new_filter.pattern_text)
                .desired_width(150.0)
                .hint_text(t!("path_editor.pattern_hint").as_ref())
        );
        super::vkb_sync(ui, &r, &state.new_filter.pattern_text);
    });

    if state.new_filter.depth > 0 {
        ui.horizontal(|ui| {
            ui.label(t!("path_editor.depth"));
            let mut depth_i32 = state.new_filter.depth as i32;
            ui.add(super::VkbDragValue::new(&mut depth_i32).range(1..=32).speed(0.1));
            state.new_filter.depth = depth_i32.max(1) as u32;
        });
    }

    // Help text for pattern format
    ui.add_space(4.0);
    ui.label(egui::RichText::new(t!("path_editor.transform_indices_hint",
        max = num_transforms.saturating_sub(1).min(15)
    ).as_ref()).small().color(ui.visuals().weak_text_color()));

    // Validation message
    if !state.new_filter.pattern_text.is_empty() && !state.new_filter.is_valid() {
        ui.label(egui::RichText::new(t!("path_editor.invalid_pattern").as_ref()).color(egui::Color32::YELLOW));
    }

    ui.add_space(4.0);

    // Add button
    ui.horizontal(|ui| {
        let can_add = state.new_filter.is_valid() && !state.new_filter.pattern_text.trim().is_empty();
        ui.add_enabled_ui(can_add, |ui| {
            if ui.button(t!("path_editor.add_filter")).clicked() {
                state.add_new_filter();
            }
        });
    });

    ui.add_space(8.0);
    ui.separator();

    // Apply/Clear buttons
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        // Apply button (only enabled if dirty)
        ui.add_enabled_ui(state.dirty, |ui| {
            if ui.button(t!("path_editor.apply_filters")).clicked() {
                response.filters_changed = Some(state.to_gpu_filters());
                state.dirty = false;
            }
        });

        // Clear all button
        ui.add_enabled_ui(!state.filters.is_empty(), |ui| {
            if ui.button(t!("path_editor.clear_all")).clicked() {
                state.filters.clear();
                state.dirty = true;
            }
        });
    });

    // Status line
    ui.add_space(4.0);
    let active_count = state.filters.iter().filter(|f| f.enabled && f.is_valid()).count();
    let status_text = if state.dirty {
        t!("path_editor.status_unsaved", count = active_count).to_string()
    } else {
        t!("path_editor.status", count = active_count).to_string()
    };
    ui.label(egui::RichText::new(status_text).small().color(ui.visuals().weak_text_color()));

    response
}

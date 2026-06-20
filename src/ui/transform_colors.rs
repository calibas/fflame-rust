//! Per-index transform colors shared by the Transforms panel (header
//! dots) and the Triangle Editor (overlay strokes), so the two views
//! always agree.
//!
//! Each pool has its own family so the pools stay visually distinct:
//!   - Normal: the canonical bright 8-color wheel.
//!   - Linked: cool blue/purple shades.
//!   - Final:  warm tan/gold shades.
//!
//! Index 0 of the Linked/Final palettes is the original single pool color
//! the Triangle Editor used before per-index coloring, so existing
//! single-linked / single-final fractals look unchanged.

use egui::Color32;

/// Distinct bright color per normal-transform index.
pub fn normal_color(index: usize) -> Color32 {
    const COLORS: [Color32; 8] = [
        Color32::from_rgb(255, 100, 100), // Red
        Color32::from_rgb(100, 255, 100), // Green
        Color32::from_rgb(100, 100, 255), // Blue
        Color32::from_rgb(255, 255, 100), // Yellow
        Color32::from_rgb(255, 100, 255), // Magenta
        Color32::from_rgb(100, 255, 255), // Cyan
        Color32::from_rgb(255, 150, 100), // Orange
        Color32::from_rgb(150, 100, 255), // Purple
    ];
    COLORS[index % COLORS.len()]
}

/// Cool blue/purple shade per linked-transform index.
pub fn linked_color(index: usize) -> Color32 {
    const COLORS: [Color32; 8] = [
        Color32::from_rgb(180, 180, 220), // original linked pool tone
        Color32::from_rgb(150, 165, 235),
        Color32::from_rgb(190, 160, 230),
        Color32::from_rgb(150, 195, 220),
        Color32::from_rgb(205, 180, 235),
        Color32::from_rgb(140, 185, 225),
        Color32::from_rgb(175, 150, 215),
        Color32::from_rgb(200, 200, 240),
    ];
    COLORS[index % COLORS.len()]
}

/// Warm tan/gold shade per final-transform index.
pub fn final_color(index: usize) -> Color32 {
    const COLORS: [Color32; 8] = [
        Color32::from_rgb(220, 200, 160), // original final pool tone
        Color32::from_rgb(225, 180, 120),
        Color32::from_rgb(235, 210, 150),
        Color32::from_rgb(210, 185, 140),
        Color32::from_rgb(235, 195, 120),
        Color32::from_rgb(215, 205, 170),
        Color32::from_rgb(230, 175, 110),
        Color32::from_rgb(225, 195, 145),
    ];
    COLORS[index % COLORS.len()]
}

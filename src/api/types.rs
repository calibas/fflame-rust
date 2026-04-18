//! API request/response types matching the Fractal Flame API OpenAPI schema.
//!
//! These types serialize to the exact JSON format the API expects.
//! Enums use lowercase serde renames to match the API convention.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Auth
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegisterRequest {
    pub display_name: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub token_type: String,
    pub expires_in: u64,
}

// ============================================================================
// User
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct ApiUser {
    pub id: String,
    pub auth_provider_id: String,
    pub display_name: String,
    pub email: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================================
// Enums (with API-compatible serde renames)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiRenderMode {
    #[serde(rename = "2d")]
    TwoD,
    #[serde(rename = "3d")]
    ThreeD,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiColorMode {
    Palette,
    Speed,
    PathMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiToneMapMode {
    Linear,
    Logarithmic,
    Density,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiPathMapStyle {
    Prefix,
    Suffix,
    PrefixDistinct,
    SuffixDistinct,
    Depth,
    OriginRadial,
    OriginHorizontal,
    OriginVertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiPathCaptureMode {
    FirstHit,
    FirstAfterBurnIn,
    LastHit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiPathTrackingMode {
    First,
    Recent,
}

/// Visibility for flames and palettes (private/unlisted/public).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiVisibility {
    Private,
    Unlisted,
    Public,
}

/// Backward-compatible alias.
pub type ApiPaletteVisibility = ApiVisibility;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiEffectStage {
    Density,
    Color,
}

// ============================================================================
// Transforms
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTransformInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub d: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub e: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub f: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub g: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_speed: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variations: Option<HashMap<String, f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variation_params: Option<HashMap<String, f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_affine_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_a: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_b: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_c: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_d: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_e: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_f: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_g: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransformResponse {
    pub id: String,
    pub sort_order: i32,
    pub is_final_transform: bool,
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
    pub g: f32,
    pub weight: f32,
    pub color: f32,
    pub color_speed: f32,
    pub opacity: f32,
    pub variations: HashMap<String, f32>,
    pub variation_params: HashMap<String, f32>,
    pub post_affine_enabled: bool,
    pub post_a: f32,
    pub post_b: f32,
    pub post_c: f32,
    pub post_d: f32,
    pub post_e: f32,
    pub post_f: f32,
    pub post_g: f32,
}

// ============================================================================
// Effects
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct CreateEffectInput {
    pub effect_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigEffectResponse {
    pub effect_id: String,
    pub effect_name: String,
    pub sort_order: i32,
    pub params: Option<serde_json::Value>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, Deserialize)]
pub struct Effect {
    pub id: String,
    pub name: String,
    pub effect_stage: ApiEffectStage,
    pub description: Option<String>,
}

// ============================================================================
// Variations (read-only from API)
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct ApiVariation {
    pub id: String,
    pub name: String,
    pub category: Option<String>,
    pub description: Option<String>,
}

// ============================================================================
// Flames
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct CreateFlameRequest {
    pub name: String,
    pub transforms: Vec<CreateTransformInput>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<ApiVisibility>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_transform: Option<CreateTransformInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_mode: Option<ApiRenderMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perspective_strength: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xaos: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solo_transform: Option<i32>,

    // View
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pan_x: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pan_y: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera_rotation_x: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera_rotation_y: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera_z: Option<f32>,

    // 3D effects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dof_focus_distance: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dof_blur_strength: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fog_strength: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fog_start: Option<f32>,

    // Rendering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub density_scale: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_factor: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub histogram_color_scale: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blend_factor: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_dynamic_blend: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_iterations_per_pixel: Option<i32>,

    // Color
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_mode: Option<ApiColorMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_map_style: Option<ApiPathMapStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_capture_mode: Option<ApiPathCaptureMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_tracking_mode: Option<ApiPathTrackingMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette_rotation: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette_size: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette_squeeze: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<Vec<f32>>,

    // Tone mapping
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tonemap_mode: Option<ApiToneMapMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tonemap_curve: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_curve: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exposure: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamma: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamma_threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brightness: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vibrancy: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saturation: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hue_shift: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha_blend_low: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha_blend_high: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levels_low: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levels_high: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levels_gamma: Option<f32>,

    // Effects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub density_effects: Option<Vec<CreateEffectInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_effects: Option<Vec<CreateEffectInput>>,

    // Reproducibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deterministic_rng: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FlameResponse {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub render_mode: ApiRenderMode,
    pub perspective_strength: f32,
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub rotation: f32,
    pub camera_rotation_x: f32,
    pub camera_rotation_y: f32,
    pub camera_z: f32,
    pub dof_focus_distance: f32,
    pub dof_blur_strength: f32,
    pub fog_strength: f32,
    pub fog_start: f32,
    pub density_scale: f32,
    pub speed_factor: f32,
    pub max_iterations: u64,
    pub histogram_color_scale: f32,
    pub blend_factor: f32,
    pub use_dynamic_blend: bool,
    pub target_iterations_per_pixel: i32,
    pub color_mode: ApiColorMode,
    pub path_map_style: ApiPathMapStyle,
    pub path_capture_mode: ApiPathCaptureMode,
    pub path_tracking_mode: ApiPathTrackingMode,
    pub palette_id: Option<String>,
    pub palette_rotation: f32,
    pub palette_size: i32,
    pub palette_squeeze: f32,
    pub background_color: Vec<f32>,
    pub tonemap_mode: ApiToneMapMode,
    pub tonemap_curve: Option<serde_json::Value>,
    pub use_curve: bool,
    pub exposure: f32,
    pub gamma: f32,
    pub gamma_threshold: f32,
    pub brightness: f32,
    pub vibrancy: f32,
    pub saturation: f32,
    pub hue_shift: f32,
    pub alpha_blend_low: f32,
    pub alpha_blend_high: f32,
    pub levels_low: f32,
    pub levels_high: f32,
    pub levels_gamma: f32,
    pub deterministic_rng: bool,
    pub solo_transform: Option<i32>,
    pub xaos: Option<serde_json::Value>,
    pub final_transform: Option<TransformResponse>,
    pub transforms: Vec<TransformResponse>,
    pub density_effects: Vec<ConfigEffectResponse>,
    pub color_effects: Vec<ConfigEffectResponse>,
    pub transform_count: i32,
    pub variation_names: Vec<String>,
    pub has_3d: bool,
    #[serde(default)]
    pub animation_count: u32,
    #[serde(default)]
    pub animations: Vec<AnimationSummary>,
    #[serde(default)]
    pub visibility: Option<ApiVisibility>,
    pub created_at: String,
    pub updated_at: String,
}

/// Summary info for an animation attached to a flame.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnimationSummary {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlameListItem {
    pub id: String,
    pub name: String,
    pub render_mode: ApiRenderMode,
    pub transform_count: i32,
    pub variation_names: Vec<String>,
    pub has_3d: bool,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================================
// Search
// ============================================================================

/// Parameters for searching flames via GET /api/search/flames
#[derive(Debug, Clone, Default, Serialize)]
pub struct SearchFlamesParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_mode: Option<ApiRenderMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_3d: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_transforms: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_transforms: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<u32>,
}

impl SearchFlamesParams {
    /// Build query string from non-None fields.
    pub fn to_query_string(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref mode) = self.render_mode {
            let s = serde_json::to_value(mode)
                .ok()
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default();
            parts.push(format!("render_mode={}", s));
        }
        if let Some(has_3d) = self.has_3d {
            parts.push(format!("has_3d={}", has_3d));
        }
        if let Some(ref variation) = self.variation {
            parts.push(format!("variation={}", variation));
        }
        if let Some(min) = self.min_transforms {
            parts.push(format!("min_transforms={}", min));
        }
        if let Some(max) = self.max_transforms {
            parts.push(format!("max_transforms={}", max));
        }
        if let Some(ref name) = self.name {
            parts.push(format!("name={}", name));
        }
        if let Some(page) = self.page {
            parts.push(format!("page={}", page));
        }
        if let Some(per_page) = self.per_page {
            parts.push(format!("per_page={}", per_page));
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("?{}", parts.join("&"))
        }
    }
}

// ============================================================================
// Palettes
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct CreatePaletteRequest {
    pub visibility: ApiPaletteVisibility,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stops: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_data: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdatePaletteRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stops: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_data: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaletteResponse {
    pub id: String,
    pub visibility: ApiPaletteVisibility,
    pub owner_id: String,
    pub name: Option<String>,
    #[serde(default)]
    pub data_hash: Option<String>,
    pub stops: Option<serde_json::Value>,
    pub color_data: Option<Vec<u32>>,
    pub color_count: Option<i32>,
    pub dominant_hue: Option<f32>,
    pub avg_color_r: Option<f32>,
    pub avg_color_g: Option<f32>,
    pub avg_color_b: Option<f32>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================================
// Animations
// ============================================================================

/// Loop mode for animations (matches API enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiLoopMode {
    Once,
    Loop,
    PingPong,
}

impl From<crate::animation::LoopMode> for ApiLoopMode {
    fn from(mode: crate::animation::LoopMode) -> Self {
        match mode {
            crate::animation::LoopMode::Once => ApiLoopMode::Once,
            crate::animation::LoopMode::Loop => ApiLoopMode::Loop,
            crate::animation::LoopMode::PingPong => ApiLoopMode::PingPong,
        }
    }
}

impl From<ApiLoopMode> for crate::animation::LoopMode {
    fn from(mode: ApiLoopMode) -> Self {
        match mode {
            ApiLoopMode::Once => crate::animation::LoopMode::Once,
            ApiLoopMode::Loop => crate::animation::LoopMode::Loop,
            ApiLoopMode::PingPong => crate::animation::LoopMode::PingPong,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateAnimationRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flame_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_mode: Option<ApiLoopMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracks: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generators: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_config: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<ApiVisibility>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnimationResponse {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub duration: f64,
    pub loop_mode: ApiLoopMode,
    pub visibility: ApiVisibility,
    pub tracks: Option<serde_json::Value>,
    pub generators: Option<serde_json::Value>,
    pub base_config: Option<serde_json::Value>,
    pub flame_id: Option<String>,
    /// Full flame data embedded by the server (same shape as FlameResponse).
    /// Authoritative source for the flame config — no separate flame request needed.
    pub flame: Option<FlameResponse>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationListItem {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub duration: f64,
    pub loop_mode: ApiLoopMode,
    pub visibility: ApiVisibility,
    pub flame_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================================
// Variations
// ============================================================================

/// Phase of a variation as serialized by the API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ApiVariationPhase {
    Pre,
    Normal,
    Post,
}

/// Parameter type as serialized by the API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ApiParamType {
    Float,
    UnlimitedFloat,
    Integer,
    UnlimitedInteger,
    Boolean,
    Angle,
    Enum { choices: Vec<String> },
}

/// Single parameter of an API variation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiVariationParameter {
    pub name: String,
    pub display_name: String,
    pub param_type: ApiParamType,
    pub default_value: f32,
    #[serde(default)]
    pub min_value: Option<f32>,
    #[serde(default)]
    pub max_value: Option<f32>,
}

/// Full variation definition fetched from the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariationDownload {
    pub id: String,
    pub name: String,
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub category: String,
    pub version: u32,
    pub phase: ApiVariationPhase,
    #[serde(default)]
    pub needs_rng: bool,
    #[serde(default)]
    pub parameters: Vec<ApiVariationParameter>,
    pub shader_2d: String,
    #[serde(default)]
    pub shader_3d: Option<String>,
}

/// Summary of a variation in a list response (no shader code).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariationListItem {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub category: String,
    pub version: u32,
    #[serde(default)]
    pub description: Option<String>,
}

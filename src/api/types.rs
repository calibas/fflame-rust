//! API request/response types matching the Fractal Flame API OpenAPI schema.
//!
//! These types serialize to the exact JSON format the API expects.
//! Enums use lowercase serde renames to match the API convention.

use serde::{Deserialize, Serialize};

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

/// Visibility for flames and palettes (private/unlisted/public).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiVisibility {
    Private,
    Unlisted,
    Public,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiEffectStage {
    Density,
    Color,
}

// ============================================================================
// Transforms
// ============================================================================

/// Wire shape for a single **root-flame** transform. The full transform
/// state lives opaquely in `data` (the same JSON `Transform` serializes to
/// in a `.fflame`); the server stores it verbatim in the `transforms`
/// table's `data` JSONB column. Only `kind`, `sort_order`, and
/// `variation_names` are promoted to columns — `kind`/`sort_order` to
/// reconstruct the pools, `variation_names` (GIN-indexed) to power
/// per-transform variation search.
///
/// Subflame transforms are NOT sent this way; they ride inside the config
/// blob's recursive `subflames` tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTransformWire {
    /// Which pool this transform belongs to: `"normal"`, `"linked"`, or
    /// `"final"`. Used to bucket transforms back into the three pools on
    /// read.
    pub kind: String,
    /// Position within its pool (array order on save).
    pub sort_order: i32,
    /// Non-zero-weight variation names, client-filtered. Search/index only;
    /// the authoritative variation map lives in `data`.
    pub variation_names: Vec<String>,
    /// Opaque per-transform state: affines, post-affines, weight, color,
    /// opacity, direct_color, variations, variation_params, 3D coefs,
    /// linked/final attachments, variation order/priorities, etc.
    pub data: serde_json::Value,
}

// ============================================================================
// Effects
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectListItem {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    /// Pipeline position: `density` runs before tonemap, `color` after.
    /// Served as `category` to match the variation payload — it is the
    /// same column as the server's `effect_stage`, because an effect's
    /// category *is* its stage.
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Markdown stripped, for a client with no markdown renderer. Unlike
    /// scripts, an effect has no client-side source to re-derive prose
    /// from, so this has to travel.
    #[serde(default)]
    pub description_plain: Option<String>,
    #[serde(default)]
    pub version: u32,
    /// Whether a client may fetch and register this at runtime.
    ///
    /// False for every row until the server's shaders are seeded. That
    /// is the honest statement of the interim — the alternative is a
    /// client discovering it by fetching one and finding nothing to
    /// compile.
    #[serde(default)]
    pub downloadable: bool,
}

/// Full detail for `GET /api/effects/{name}` — carries the WGSL.
///
/// `Serialize` as well as `Deserialize`: the cache stores the payload
/// verbatim, so a field this build does not yet read still survives a
/// round trip rather than being silently dropped on write.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectDownload {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub description_plain: Option<String>,
    #[serde(default)]
    pub version: u32,
    /// Served verbatim, same wire shape as a variation's parameters —
    /// see [`ApiEffectParameter`].
    #[serde(default)]
    pub parameters: Vec<ApiEffectParameter>,
    /// The WGSL. **Nullable**: null until the server's shader seed
    /// migration lands, which is exactly when `downloadable` flips true.
    /// Registration refuses a null one rather than registering an effect
    /// that would render nothing.
    #[serde(default)]
    pub shader: Option<String>,
    /// Whether the shader needs the `// INCLUDE_BLEND_MODES` splice.
    ///
    /// Advisory: the client splices on the marker actually present, so
    /// this is cross-checked rather than trusted. A shader that calls
    /// into the shared library without the marker is refused — it would
    /// otherwise fail to compile naming a function nobody wrote.
    #[serde(default)]
    pub requires_blend_modes: bool,
    #[serde(default)]
    pub downloadable: bool,
}

/// One parameter of a downloaded effect.
///
/// Deliberately the *same type* as a variation's, not a copy of its
/// shape. The API serves both verbatim from one format, and two structs
/// that must stay identical are two structs that will not.
pub type ApiEffectParameter = ApiVariationParameter;

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

/// Request body for `POST /api/flames` (and `PUT`). The flame config is an
/// **opaque blob** — the same JSON a `.fflame` file holds, minus the root
/// flame's transforms (split into `transforms` below) and minus the palette
/// (sent inline via `palette`). New config fields need zero API/DB work; the
/// server stores `config` verbatim and extracts only `render_mode` (at
/// `config.flame.render_mode`) into a typed column for catalog queries.
#[derive(Debug, Clone, Serialize)]
pub struct CreateFlameRequest {
    /// Flame name. Single field — the old `flame_name`/cloud-title split is
    /// gone; `Flame::name` round-trips through this.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<ApiVisibility>,
    /// Inline content-addressable palette. `None` leaves the flame with no
    /// palette; `Some` embeds content — the server computes the SHA-256 hash
    /// and stores `(hash, name)` on the flame row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette: Option<ApiPalette>,
    /// Opaque config blob: the whole `FractalConfig` minus root transforms
    /// and palette, including the full recursive subflame tree (each subflame
    /// keeps its own inline transform pools). Carries `version`.
    pub config: serde_json::Value,
    /// The root flame's transforms only, flattened across the normal /
    /// linked / final pools. Subflame transforms ride inside `config`.
    pub transforms: Vec<ApiTransformWire>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FlameResponse {
    pub id: String,
    pub user_id: String,
    pub name: String,
    #[serde(default)]
    pub visibility: Option<ApiVisibility>,
    /// Inline palette payload (server-computed hash + content + derived
    /// metadata). `None` when the flame has no palette.
    #[serde(default)]
    pub palette: Option<ApiPalette>,
    /// Opaque config blob, mirrored back as stored. Reconstructed into a
    /// `FractalConfig` (with root transforms re-injected) on the client.
    pub config: serde_json::Value,
    /// Root flame transforms (all three pools, flat).
    #[serde(default)]
    pub transforms: Vec<ApiTransformWire>,
    // Server-derived display metadata (`render_mode`, `transform_count`,
    // `variation_names`, `has_3d`) is no longer carried on the single-flame
    // response — `render_mode` lives in the config blob's flame, and the rest
    // are recoverable from `transforms` / the blob when needed. The list
    // endpoint (`FlameListItem`) still surfaces them for the browser.
    #[serde(default)]
    pub animation_count: u32,
    #[serde(default)]
    pub animations: Vec<AnimationSummary>,
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

/// Content-addressable palette payload. Used inline on flame
/// requests/responses and as the body for `POST /api/palettes`, the
/// public `GET /api/palettes/{hash}` endpoint, and library entry reads.
///
/// On requests the client sends content (`color_data` and/or `stops`)
/// plus an optional flame-specific `name`. The server computes the
/// SHA-256 hash and identifies the palette by it — the client never
/// needs to send `hash` (the server ignores it when content is present).
///
/// On responses the server populates `hash` plus the derived metadata
/// fields (`avg_color_*`, `dominant_hue`, `color_count`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiPalette {
    /// Server-computed SHA-256 hex. Set on responses; clients omit it
    /// on requests (server ignores any client-supplied value).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// Flame-specific display name. Travels with the flame, independent
    /// of any library nickname the caller may have set separately.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// RGB color data, one u32 per entry (0xRRGGBB). At least one of
    /// `color_data` or `stops` must be present on a request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_data: Option<Vec<u32>>,
    /// Editor stop list (opaque JSON, preserved as-sent by the server).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stops: Option<serde_json::Value>,
    /// Derived: average channel values. Response only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avg_color_r: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avg_color_g: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avg_color_b: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dominant_hue: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_count: Option<i32>,
}

/// Entry from `GET /api/users/me/palettes` — the caller's bookmarked
/// library. Carries the palette content alongside the caller's personal
/// `nickname` and `added_at` timestamp.
#[derive(Debug, Clone, Deserialize)]
pub struct LibraryPaletteEntry {
    pub hash: String,
    /// Caller's personal label. `None` when no nickname was set.
    #[serde(default)]
    pub nickname: Option<String>,
    pub added_at: String,
    #[serde(default)]
    pub color_data: Option<Vec<u32>>,
    #[serde(default)]
    pub stops: Option<serde_json::Value>,
    #[serde(default)]
    pub avg_color_r: Option<f32>,
    #[serde(default)]
    pub avg_color_g: Option<f32>,
    #[serde(default)]
    pub avg_color_b: Option<f32>,
    #[serde(default)]
    pub dominant_hue: Option<f32>,
    #[serde(default)]
    pub color_count: Option<i32>,
}

/// Body for `PUT /api/users/me/palettes/{hash}`. Omitted `nickname`
/// leaves the existing label unchanged.
#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateLibraryNicknameRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
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
// Scripts
// ============================================================================

/// A script as the server stores it — detail payload, source included.
///
/// `source` is authoritative and everything else is derived from it by
/// the collect pass. The server has no Rhai engine, so the derived
/// fields are whatever the uploader sent; the client **re-derives on
/// load** rather than trusting them, and a mismatch means the stored
/// metadata is stale, not that the script is wrong.
///
/// There is no `description_plain` here, unlike variations and effects.
/// A script's prose is derived from its own source, which the client
/// always has, so a stored stripped copy would be a derivation of a
/// derivation; `script::strip_markdown` does it locally instead.
#[derive(Debug, Clone, Deserialize)]
pub struct ScriptResponse {
    pub id: String,
    /// Owner. Distinct from `authors`, which is credit.
    pub user_id: String,
    /// The owner's display name — case-insensitively unique server-side,
    /// which makes `owner_display_name/name` a stable human-readable key
    /// and means an import never has to invent a disambiguator or show a
    /// UUID fragment.
    #[serde(default)]
    pub owner_display_name: String,
    /// Stem-like key, unique per user.
    pub name: String,
    /// The SCRIPT's declared name, from `script("…")`. Not the owner's —
    /// that is `owner_display_name`.
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub kind: Option<String>,
    /// Credit, NOT ownership. Empty is the normal case for an original
    /// script; a ported one credits whoever wrote the original, who may
    /// never have used this app.
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub source: String,
    /// Send this back on PUT. Bumps on EVERY write — unlike `version` on
    /// variations and effects, which deliberately does not bump for
    /// prose edits so cached copies stay valid. One field serves both
    /// purposes here only because the source IS the content: no edit
    /// leaves a cached copy valid.
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub visibility: Option<ApiVisibility>,
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// One row of a listing or a search — everything but the source.
#[derive(Debug, Clone, Deserialize)]
pub struct ScriptListItem {
    pub id: String,
    pub user_id: String,
    #[serde(default)]
    pub owner_display_name: String,
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub visibility: Option<ApiVisibility>,
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Create / update payload.
///
/// `display_name` and `kind` are **required** by the server, which is
/// why [`crate::api::sync::script_to_create_request`] returns a Result:
/// a script that fails to collect declares neither, and inventing a
/// kind would write a value into the server's search index that the
/// source contradicts. Such a script still saves locally, where there
/// is no schema to satisfy.
#[derive(Debug, Clone, Serialize)]
pub struct ScriptCreateRequest {
    pub name: String,
    pub display_name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub source: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<ApiVisibility>,
    /// The version the client last read. Required on update, absent on
    /// create.
    ///
    /// Travels in the body rather than an `If-Match` header: the API is
    /// JSON end to end with no ETag machinery, so a header would be the
    /// only one of its kind, and there is no intermediary cache to gain
    /// from the idiom.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
}

/// The body of a `409 Conflict`: somebody else wrote first.
///
/// Deliberately not the whole script — a client that wants the current
/// content can GET it, and a source can be 256 KB. What is here is
/// enough to choose between refetch-and-merge and warn-the-user, which
/// is the choice the panel has to offer instead of "something went
/// wrong".
#[derive(Debug, Clone, Deserialize)]
pub struct ScriptConflict {
    pub id: String,
    /// The version currently stored. Yours was older.
    pub current_version: u32,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl ScriptConflict {
    /// Recover the conflict body from a failed update.
    ///
    /// Both transports put the raw response body into
    /// `FetchError::Http::message`, so no new error variant is needed —
    /// but 403 and 404 are converted to their own variants *before* the
    /// body is read, and only the fall-through path keeps it. A 409 is
    /// on the fall-through path, which is the only reason this works.
    pub fn from_error(e: &crate::resources::FetchError) -> Option<Self> {
        match e {
            crate::resources::FetchError::Http { status: 409, message } => {
                serde_json::from_str(message).ok()
            }
            _ => None,
        }
    }
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
    /// Free-form help / tooltip prose. Single English locale by
    /// policy. `None` / absent renders the control without a tooltip.
    /// Edits to this field do not bump the variation `version` (see
    /// VARIATIONS_WIRE_FORMAT.md §8).
    #[serde(default)]
    pub description: Option<String>,
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
    /// Whether this variation needs `xform_id` for reads from the
    /// per-transform `transforms[xform_id]` storage buffer (affine, weight,
    /// color, etc.). When true, the generated WGSL signature includes
    /// `xform_id: u32`. Old API responses using `needs_affine` are accepted
    /// via serde alias; new payloads use `needs_transform`.
    #[serde(default, alias = "needs_affine")]
    pub needs_transform: bool,
    /// Whether this variation writes the iteration-local color register `vc`
    /// (Apophysis direct-color variations). When true, the WGSL signature
    /// gains `vc: ptr<function, f32>`. Old API responses default to false.
    #[serde(default)]
    pub writes_color: bool,
    /// Capability flags, superseding the three legacy bools above when
    /// present. Names come from `Feature::to_api_str`; see the generated
    /// contract for the full set.
    ///
    /// Declared ahead of the server sending it: `serde(default)` makes
    /// the field forward-compatible, so the client is ready the day the
    /// migration lands rather than a coordination round later. An
    /// unknown string is **ignored with a warning** — a newer server
    /// must be able to serve an older client.
    #[serde(default)]
    pub features: Vec<String>,

    /// Per-(thread, xform, variation) f32 state slots. Was hardcoded to
    /// 0 on this side, which silently mis-rendered any stateful
    /// server-hosted variation.
    #[serde(default)]
    pub state_count: usize,

    /// Optional WGSL seeding state beyond the default zero-fill.
    #[serde(default)]
    pub shader_state_init: Option<String>,

    /// Foreign-app names that resolve to this variation on `.flame`
    /// import (e.g. `linear3D` for `linear`).
    ///
    /// §3 of the wire-format doc has listed this as though it were
    /// already here; it was not. Sixth instance of that class.
    #[serde(default)]
    pub aliases: Vec<String>,

    /// Per-call emission cap (`Feature::PlotEmits`). Its own field
    /// rather than a `features` entry because it carries a payload; the
    /// engine clamps to 16.
    #[serde(default)]
    pub plot_emits: u8,

    /// Attribution and the markdown-stripped description. Presentation
    /// only — deliberately NOT loaded into `VariationInfo`, so the
    /// in-memory footprint stays flat however much prose ships.
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub description_plain: Option<String>,

    #[serde(default)]
    pub parameters: Vec<ApiVariationParameter>,
    /// The 2D body. **Optional only for `only_3d`** — see
    /// `VariationRegistry::register_from_api`, which refuses `None` for
    /// any other category rather than letting it become a silent skip.
    ///
    /// An `only_3d` variation is filtered out of the active set in 2D
    /// builds *before* any source lookup
    /// (`ShaderBuilder::active_with_local_indices`), so a 2D body would
    /// never be read. Requiring a vestigial one would mean dead data a
    /// curator writes, a reviewer reads, and nothing can validate.
    #[serde(default)]
    pub shader_2d: Option<String>,
    #[serde(default)]
    pub shader_3d: Option<String>,
    /// Number of init-derived parameters this variation produces.
    /// Old API responses without this field default to 0.
    #[serde(default)]
    pub init_param_count: usize,
    /// Optional WGSL init function, run once per param change on the GPU.
    /// Old API responses without this field default to None.
    #[serde(default)]
    pub shader_init: Option<String>,
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

    // Declared ahead of the server sending them, as with
    // `VariationDownload` — every one is `serde(default)`, so an older
    // payload simply leaves them empty.
    /// Markdown stripped, for clients that do not render markdown. This
    /// one is not a nicety: the panel shows no description at all today,
    /// and built-in descriptions live in Rust doc comments that are
    /// invisible at runtime. The catalog is the ONLY way prose reaches
    /// the app for a variation it already ships.
    #[serde(default)]
    pub description_plain: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    /// False when the variation is built-in-only — engine-integral, so
    /// there is nothing to fetch. See WIRE_FORMAT §3.1.
    #[serde(default = "default_true")]
    pub downloadable: bool,
    /// Whether a 3D body exists. A variation without one is dropped
    /// entirely from 3D flames, so this is a browse-time decision.
    #[serde(default = "default_true")]
    pub has_shader_3d: bool,
}

fn default_true() -> bool {
    true
}

//! API request/response types matching the Fractal Flame API OpenAPI schema.
//!
//! These types serialize to the exact JSON format the API expects.
//! Enums use lowercase serde renames to match the API convention.

use serde::{Deserialize, Serialize};

/// A wire enum spelled as a string, which reads a value this build does
/// not know as `Unknown(value)` instead of failing the whole response,
/// and writes it back as read (docs/archive/projects/api-updates.md, step 1). A
/// newer server's new value -- a render mode, a phase, a loop mode --
/// then costs an old client that one item, not the page it was on.
macro_rules! wire_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $wire:literal, )+
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        $vis enum $name {
            $( $(#[$vmeta])* $variant, )+
            /// A value this build does not know, kept as it was read.
            Unknown(String),
        }

        impl $name {
            /// The wire spelling.
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $wire, )+
                    Self::Unknown(s) => s,
                }
            }

            /// From the wire spelling; anything else is `Unknown`.
            pub fn from_wire(s: &str) -> Self {
                match s {
                    $( $wire => Self::$variant, )+
                    other => Self::Unknown(other.to_string()),
                }
            }

            /// Whether this build knows the value.
            pub fn is_known(&self) -> bool {
                !matches!(self, Self::Unknown(_))
            }
        }

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                Ok(Self::from_wire(&String::deserialize(d)?))
            }
        }
    };
}

/// A list from the API, read item by item: an item this build cannot
/// read -- a newer server's shape -- is skipped and logged, and the rest
/// of the list is kept. Before this, one such item failed the whole page
/// (an escape flame in a 0.5 client's list: docs/archive/projects/api-updates.md,
/// hazard 2).
#[derive(Debug, Clone)]
pub struct Lenient<T>(pub Vec<T>);

impl<T> Lenient<T> {
    pub fn into_inner(self) -> Vec<T> {
        self.0
    }
}

impl<'de, T: serde::de::DeserializeOwned> Deserialize<'de> for Lenient<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // A page's `items` (the catalog API's envelope, `{items, next}`),
        // or a bare array (a bounded sub-list such as a fractal's tags).
        let items = match serde_json::Value::deserialize(d)? {
            serde_json::Value::Array(items) => items,
            serde_json::Value::Object(mut page) => match page.remove("items") {
                Some(serde_json::Value::Array(items)) => items,
                _ => return Err(serde::de::Error::custom("a page without items")),
            },
            _ => return Err(serde::de::Error::custom("not a list")),
        };
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            match serde_json::from_value(item) {
                Ok(v) => out.push(v),
                Err(e) => log::warn!("skipped a list item this version cannot read: {e}"),
            }
        }
        Ok(Lenient(out))
    }
}

/// One page of a list (the catalog API's envelope): its items, read as
/// [`Lenient`] reads them, and the cursor of the next page, `None` at the
/// end. `deleted`: asked with `updated_since`, the ids deleted since.
#[derive(Debug, Clone, Deserialize)]
#[serde(bound(deserialize = "T: serde::de::DeserializeOwned"))]
pub struct Page<T> {
    pub items: Lenient<T>,
    #[serde(default)]
    pub next: Option<String>,
    #[serde(default)]
    pub deleted: Option<Vec<String>>,
}

/// An item's owner (absent on built-ins).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Owner {
    pub id: String,
    pub display_name: String,
}

/// What makes an item curated (docs/projects/curated-content.md): an
/// admin's static copy under a canonical name. Absent on everything else.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Curation {
    /// The canonical name; absent on a fractal frozen only as a curated
    /// animation's.
    #[serde(default)]
    pub name: Option<String>,
    /// Whether the name means this version now.
    #[serde(default)]
    pub current: bool,
    #[serde(default)]
    pub curated_at: Option<String>,
    /// The content's hash: what a config pins (`crate::curated`).
    #[serde(default)]
    pub content_hash: String,
    /// Whose work it is.
    #[serde(default)]
    pub credit: Option<String>,
}

/// One thing to resolve: a canonical name, a content hash, or both.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResolveItem {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

/// What a canonical name or a hash resolves to (`GET /curated/{kind}/{name}`,
/// `POST /curated/resolve`): where to fetch it and its version.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Resolution {
    pub kind: String,
    #[serde(default)]
    pub name: Option<String>,
    /// `flame`, `palette`, `script`, `animation`, or a built-in
    /// `variation` / `effect`.
    pub resource: String,
    #[serde(default)]
    pub id: Option<String>,
    pub content_hash: String,
    #[serde(default)]
    pub current: bool,
    pub href: String,
}

/// A JSON null read as the type's default: a field the server may send as
/// null (a variation's `category`) where this client keeps a value.
fn null_as_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

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

wire_enum! {
pub enum ApiRenderMode {
    TwoD = "2d",
    ThreeD = "3d",
    /// Escape-time fractals. Stored as FLAMES -- same endpoints, ids
    /// and access control -- and told apart by this value alone, so a
    /// consumer groups by `render_mode` rather than looking for a
    /// second resource type.
    ///
    /// Two consequences the server confirmed by measurement:
    /// `transform_count` is 0 and `variation_names` is empty for these,
    /// legitimately (the formula IS the fractal), and `has_3d` is
    /// false, so a `has_3d=false` filter returns escape fractals
    /// alongside 2D flames.
    Escape = "escape",
    /// Neighbour-coupled simulations. Stored as FLAMES on the same
    /// endpoints and told apart by this value alone, exactly as escape
    /// is, with the same two consequences: `transform_count` is 0 and
    /// `variation_names` is empty for these, legitimately.
    ///
    /// The server stores it from migration 102 (2026-11-11), and lists it
    /// only to a request naming reader 7 or more.
    Simulation = "simulation",
}
}

wire_enum! {
/// Visibility for flames and palettes (private/unlisted/public).
pub enum ApiVisibility {
    Private = "private",
    Unlisted = "unlisted",
    Public = "public",
}
}

wire_enum! {
pub enum ApiEffectStage {
    Density = "density",
    Color = "color",
}
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
/// server stores `config` verbatim and extracts only `render_mode` (at the
/// blob's ROOT, `config.pointer("/render_mode")`) into a typed column for
/// catalog queries.
///
/// The root is where config v3 put it, and where current clients write it;
/// they do not write `flame.render_mode` at all. This comment said `flame`
/// until 2026-08-29, when the API measured all three shapes: root-only is
/// catalogued correctly, inner-only is a 400 (`config.render_mode_missing`),
/// and a pre-v3 blob carrying both catalogues by the ROOT. So no flame was
/// ever mis-catalogued by the stale wording -- but it was wrong.
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
    #[serde(default)]
    pub owner: Option<Owner>,
    pub name: String,
    /// The write token: send it back on PUT/PATCH; a stale one is a 409.
    #[serde(default)]
    pub revision: Option<i64>,
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
    // `transform_count`, `variation_names` and `has_3d` are not carried on
    // the single-flame response — they are recoverable from `transforms` /
    // the blob when needed, and the list endpoint (`FlameListItem`) still
    // surfaces them for the browser.
    //
    // `render_mode` IS carried, as a top-level mirror of the blob's value
    // (openapi: "so list + detail responses have a consistent shape"). This
    // struct deliberately does not bind it: the config blob is the source
    // this client reads, and a second copy of the same fact is a second
    // thing that can disagree. Serde ignores the extra field.
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
    /// `builtin` (the app's shipped content, seeded by the sync) or
    /// `user`.
    #[serde(default)]
    pub origin: Option<String>,
    /// Curated: the canonical name and the version's hash
    /// (docs/projects/curated-content.md).
    #[serde(default)]
    pub curated: Option<Curation>,
    #[serde(default)]
    pub owner: Option<Owner>,
    pub name: String,
    /// Markdown.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub visibility: Option<ApiVisibility>,
    /// Its thumbnail, a path on the API, or none: used as given.
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    pub render_mode: ApiRenderMode,
    /// The catalog's facts (api-catalog-spec.md): `flame`, `escape` or
    /// `simulation`, and 2 or 3; absent on an item the server has not
    /// summarised yet.
    #[serde(default)]
    pub engine: Option<String>,
    #[serde(default)]
    pub dimension: Option<i16>,
    pub transform_count: i32,
    pub variation_names: Vec<String>,
    pub has_3d: bool,
    #[serde(default)]
    pub favorite_count: i32,
    pub created_at: String,
    pub updated_at: String,
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

/// A palette (`GET /users/me/palettes`, `GET /palettes/{id}`): an item
/// with its own id that points at content by hash, so new colours keep
/// its name and id. The colours are `content`, as `/palette-data/{hash}`
/// serves them.
#[derive(Debug, Clone, Deserialize)]
pub struct PaletteItem {
    pub id: String,
    /// `builtin` (the app's shipped content, seeded by the sync) or
    /// `user`.
    #[serde(default)]
    pub origin: Option<String>,
    /// Curated: the canonical name and the version's hash
    /// (docs/projects/curated-content.md).
    #[serde(default)]
    pub curated: Option<Curation>,
    #[serde(default)]
    pub owner: Option<Owner>,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub visibility: Option<ApiVisibility>,
    pub content_hash: String,
    #[serde(default)]
    pub content: ApiPalette,
    #[serde(default)]
    pub revision: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// `PATCH /R/{id}` for a fractal, a palette or a script: the name or the
/// visibility, what is present replacing what is stored.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ItemPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<ApiVisibility>,
}

/// `POST /palettes`: a palette of the user's, published (its colours as
/// `encode_palette_data` writes them).
#[derive(Debug, Clone, Serialize)]
pub struct CreatePaletteRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub visibility: ApiVisibility,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_data: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stops: Option<serde_json::Value>,
}

// ============================================================================
// Animations
// ============================================================================

wire_enum! {
/// Loop mode for animations (matches API enum).
pub enum ApiLoopMode {
    Once = "once",
    Loop = "loop",
    PingPong = "ping_pong",
}
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
            // A newer server's mode: play it as a loop, the default.
            ApiLoopMode::Unknown(_) => crate::animation::LoopMode::Loop,
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
    /// The animation's motion blur, when it has one (the server stores it
    /// from migration 105; an older server ignores the field).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motion_blur: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<ApiVisibility>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnimationResponse {
    pub id: String,
    #[serde(default)]
    pub owner: Option<Owner>,
    #[serde(default)]
    pub revision: Option<i64>,
    pub name: String,
    pub duration: f64,
    pub loop_mode: ApiLoopMode,
    pub visibility: ApiVisibility,
    pub tracks: Option<serde_json::Value>,
    pub generators: Option<serde_json::Value>,
    pub base_config: Option<serde_json::Value>,
    /// Absent from an older server, or when the animation has none.
    #[serde(default)]
    pub motion_blur: Option<serde_json::Value>,
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
    /// `builtin` (the app's shipped content, seeded by the sync) or
    /// `user`.
    #[serde(default)]
    pub origin: Option<String>,
    /// Curated: the canonical name and the version's hash
    /// (docs/projects/curated-content.md).
    #[serde(default)]
    pub curated: Option<Curation>,
    #[serde(default)]
    pub owner: Option<Owner>,
    pub name: String,
    /// Markdown.
    #[serde(default)]
    pub description: Option<String>,
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
    /// Curated: the canonical name and the version's hash.
    #[serde(default)]
    pub curated: Option<Curation>,
    /// Owner (absent on a built-in). Distinct from `authors`, which is
    /// credit. Its display name is case-insensitively unique server-side,
    /// which makes `owner/name` a stable human-readable key and means an
    /// import never has to invent a disambiguator or show a UUID fragment.
    #[serde(default)]
    pub owner: Option<Owner>,
    /// Stem-like key, unique per user.
    pub name: String,
    /// The SCRIPT's declared name, from `script("…")`. Not the owner's —
    /// that is `owner`.
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
    /// leaves a cached copy valid. The wire calls it `revision`, as every
    /// item's write token.
    #[serde(default, alias = "revision")]
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
    /// `builtin` (the app's shipped content, seeded by the sync) or
    /// `user`.
    #[serde(default)]
    pub origin: Option<String>,
    /// Curated: the canonical name and the version's hash
    /// (docs/projects/curated-content.md).
    #[serde(default)]
    pub curated: Option<Curation>,
    #[serde(default)]
    pub owner: Option<Owner>,
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, alias = "revision")]
    pub version: u32,
    #[serde(default)]
    pub visibility: Option<ApiVisibility>,
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl ScriptResponse {
    /// The owner's display name, empty for a built-in.
    pub fn owner_name(&self) -> &str {
        self.owner.as_ref().map_or("", |o| o.display_name.as_str())
    }
}

impl ScriptListItem {
    /// The owner's display name, empty for a built-in.
    pub fn owner_name(&self) -> &str {
        self.owner.as_ref().map_or("", |o| o.display_name.as_str())
    }
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
    /// from the idiom. The wire calls it `revision`.
    #[serde(rename = "revision", skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    /// A formula-library entry's `INFO.id`, the id the server stores it
    /// under (`api::entries`); absent for a CPU script, which the server
    /// gives one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
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
    /// The script's id: not in the body (the contract's 409 names only the
    /// revision); [`ScriptConflict::from_error`] fills it in.
    #[serde(default)]
    pub id: String,
    /// The version currently stored (the wire's `current_revision`).
    /// Yours was older.
    #[serde(alias = "current_revision")]
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
    pub fn from_error(e: &crate::resources::FetchError, id: &str) -> Option<Self> {
        match e {
            crate::resources::FetchError::Http { status: 409, message } => {
                let mut conflict: Self = serde_json::from_str(message).ok()?;
                conflict.id = id.to_string();
                Some(conflict)
            }
            _ => None,
        }
    }
}

// ============================================================================
// Variations
// ============================================================================

wire_enum! {
/// Phase of a variation as serialized by the API. `any` (a variation the
/// transform may place in any phase, JWildfire's `fx_priority`) is what
/// most shipped variations are; the server has stored it since migration
/// 071.
pub enum ApiVariationPhase {
    Pre = "pre",
    Normal = "normal",
    Post = "post",
    Any = "any",
}
}

/// Parameter type as serialized by the API: a string (`"float"`), or
/// `{"enum": {"choices": [...]}}`. A type this build does not know reads
/// as `Unknown`, holding the value as it was read (and written back so),
/// and the parameter is offered as a free float: every parameter is one
/// float slot on the GPU.
#[derive(Debug, Clone, PartialEq)]
pub enum ApiParamType {
    Float,
    UnlimitedFloat,
    Integer,
    UnlimitedInteger,
    Boolean,
    Angle,
    Enum { choices: Vec<String> },
    Unknown(serde_json::Value),
}

/// The known spellings, as serde writes them.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum KnownParamType {
    Float,
    UnlimitedFloat,
    Integer,
    UnlimitedInteger,
    Boolean,
    Angle,
    Enum { choices: Vec<String> },
}

impl Serialize for ApiParamType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let known = match self {
            Self::Float => KnownParamType::Float,
            Self::UnlimitedFloat => KnownParamType::UnlimitedFloat,
            Self::Integer => KnownParamType::Integer,
            Self::UnlimitedInteger => KnownParamType::UnlimitedInteger,
            Self::Boolean => KnownParamType::Boolean,
            Self::Angle => KnownParamType::Angle,
            Self::Enum { choices } => KnownParamType::Enum { choices: choices.clone() },
            Self::Unknown(v) => return v.serialize(s),
        };
        known.serialize(s)
    }
}

impl<'de> Deserialize<'de> for ApiParamType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        Ok(match serde_json::from_value::<KnownParamType>(value.clone()) {
            Ok(KnownParamType::Float) => Self::Float,
            Ok(KnownParamType::UnlimitedFloat) => Self::UnlimitedFloat,
            Ok(KnownParamType::Integer) => Self::Integer,
            Ok(KnownParamType::UnlimitedInteger) => Self::UnlimitedInteger,
            Ok(KnownParamType::Boolean) => Self::Boolean,
            Ok(KnownParamType::Angle) => Self::Angle,
            Ok(KnownParamType::Enum { choices }) => Self::Enum { choices },
            Err(_) => Self::Unknown(value),
        })
    }
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
    #[serde(default, deserialize_with = "null_as_default")]
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A list is the catalog API's page; a bare array still reads (a
    /// bounded sub-list), and an item this build cannot read is skipped.
    #[test]
    fn a_list_is_a_page() {
        let page = serde_json::json!({"items": [{"id": "a", "name": "A"}, {"id": 5}], "next": "bzoy"});
        let read: Lenient<AnimationSummary> = serde_json::from_value(page.clone()).unwrap();
        assert_eq!(read.0.len(), 1, "the unreadable one skipped");
        let paged: Page<AnimationSummary> = serde_json::from_value(page).unwrap();
        assert_eq!(paged.next.as_deref(), Some("bzoy"));
        let bare: Lenient<AnimationSummary> = serde_json::from_value(serde_json::json!([{"id": "a", "name": "A"}])).unwrap();
        assert_eq!(bare.0.len(), 1);
        assert!(serde_json::from_value::<Lenient<AnimationSummary>>(serde_json::json!({"error": "x"})).is_err());
    }

    /// The contract's 409 names the revision, not the script: the caller's
    /// id is filled in.
    #[test]
    fn a_script_conflict_from_the_contracts_409() {
        let body = r#"{"error":"conflict","message":"Changed since you read it","current_revision":7,"updated_at":"2026-11-16T00:00:00Z"}"#;
        let e = crate::resources::FetchError::Http { status: 409, message: body.into() };
        let c = ScriptConflict::from_error(&e, "abc").expect("a conflict");
        assert_eq!((c.id.as_str(), c.current_version), ("abc", 7));
        let other = crate::resources::FetchError::Http { status: 400, message: body.into() };
        assert!(ScriptConflict::from_error(&other, "abc").is_none());
    }

    /// Items name their owner as an object; a script's write token is
    /// `revision` on the wire, both ways.
    #[test]
    fn owners_and_revisions() {
        let script: ScriptListItem = serde_json::from_value(serde_json::json!({
            "id": "s", "name": "n", "owner": {"id": "u", "display_name": "Ann"}, "revision": 4
        }))
        .unwrap();
        assert_eq!((script.owner_name(), script.version), ("Ann", 4));
        let builtin: ScriptListItem = serde_json::from_value(serde_json::json!({"id": "s", "name": "n", "origin": "builtin"})).unwrap();
        assert_eq!(builtin.owner_name(), "");
        let req = ScriptCreateRequest {
            name: "n".into(),
            display_name: "N".into(),
            kind: "generator".into(),
            authors: Vec::new(),
            description: None,
            source: "fn main() {}".into(),
            flags: Vec::new(),
            visibility: None,
            version: Some(4),
            id: None,
        };
        let sent = serde_json::to_value(&req).unwrap();
        assert_eq!(sent["revision"], 4);
        assert!(sent.get("version").is_none());
    }

    /// A palette is an item pointing at content; a variation's category
    /// may come as null.
    #[test]
    fn palettes_and_null_categories() {
        let p: PaletteItem = serde_json::from_value(serde_json::json!({
            "id": "p1", "name": "Sunset", "content_hash": "h", "revision": 2, "visibility": "private",
            "content": {"hash": "h", "color_data": [1, 2, 3], "stops": null}
        }))
        .unwrap();
        assert_eq!((p.name.as_str(), p.content.hash.as_deref()), ("Sunset", Some("h")));
        let v: VariationListItem = serde_json::from_value(serde_json::json!({
            "id": "linear", "name": "linear", "display_name": "Linear", "category": null, "version": 1
        }))
        .unwrap();
        assert_eq!(v.category, "");
    }
}

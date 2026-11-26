//! API integration module for the Fractal Flame API.
//!
//! Feature-gated behind `--features api`. Provides:
//! - HTTP client with authenticated requests (client.rs)
//! - API request/response types matching the OpenAPI schema (types.rs)
//! - FractalConfig ↔ API format conversion (sync.rs)
//! - Auth state management (auth.rs)
//! - ApiState coordinator for managing API operations

pub mod auth;
pub mod client;
pub mod entries;
pub mod health;
pub mod job;
pub mod sync;
pub mod types;
#[cfg(test)]
mod future_tests;
#[cfg(test)]
mod integration_tests;

use crate::config::FractalConfig;
use crate::resources::{FetchError, FetchResult, LoadState};

/// Whose items a list asks for: the curated ones (the app's browsing,
/// docs/projects/curated-content.md), every public one (the API's own
/// browsing, kept for its tests and tools), or the caller's own (every
/// visibility; signed in).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Scope {
    #[default]
    Curated,
    Public,
    Mine,
}

impl Scope {
    /// The list's path for resource `r` (`flames`, `palettes`, `scripts`,
    /// `animations`).
    fn path(self, r: &str) -> String {
        match self {
            Scope::Curated | Scope::Public => format!("/api/v1/{r}"),
            Scope::Mine => format!("/api/v1/users/me/{r}"),
        }
    }
}

/// One page's query words (docs/projects/catalog-api.md, *Query words*):
/// a search, a sort, a kind, the cursor of the page to read, and any of a
/// resource's own filters (`render_mode`, ...).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Browse {
    pub q: String,
    /// One of the resource's sorts; `None` for its default.
    pub sort: Option<String>,
    pub kind: Option<String>,
    /// The `next` of the page before; `None` for the first.
    pub cursor: Option<String>,
    /// 1-100; 0 for the server's default (50).
    pub per_page: u32,
    pub filters: Vec<(String, String)>,
}

impl Browse {
    /// `?q=...&sort=...`, the empty words left out.
    pub fn query(&self) -> String {
        let mut parts = Vec::new();
        let q = self.q.trim();
        if !q.is_empty() {
            parts.push(format!("q={}", urlencode(q)));
        }
        if let Some(s) = &self.sort {
            parts.push(format!("sort={}", urlencode(s)));
        }
        if let Some(k) = &self.kind {
            parts.push(format!("kind={}", urlencode(k)));
        }
        for (k, v) in &self.filters {
            parts.push(format!("{}={}", urlencode(k), urlencode(v)));
        }
        if self.per_page > 0 {
            parts.push(format!("per_page={}", self.per_page.min(100)));
        }
        if let Some(c) = &self.cursor {
            parts.push(format!("cursor={}", urlencode(c)));
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("?{}", parts.join("&"))
        }
    }
}

/// Result of loading a flame from the API, including metadata for ownership tracking.
pub struct FlameLoadResult {
    pub config: FractalConfig,
    pub is_public: Option<bool>,
    pub user_id: String,
    pub animation_count: u32,
    pub animations: Vec<types::AnimationSummary>,
}

/// The visibility a flame **Update** sends, from the Save Online
/// dialog's "Make public" box and what the flame was when the dialog
/// opened (`was_public`: `Some(true)` public, `Some(false)` private or
/// unlisted, `None` unknown).
///
/// `update_flame` reads `None` as "keep what the server has", which is
/// right for an unchecked box on a flame that was not public (an
/// unlisted flame stays unlisted) but wrong for one that was: the
/// dialog opens with the box checked for a public flame, so unchecking
/// it is the user asking to make it private. Sending `None` there left
/// the flame public while the app recorded it as private.
pub fn flame_update_visibility(make_public: bool, was_public: Option<bool>) -> Option<ApiVisibility> {
    if make_public {
        Some(ApiVisibility::Public)
    } else if was_public == Some(true) {
        Some(ApiVisibility::Private)
    } else {
        None
    }
}

/// Make the server's copy of an animation's flame its `base_config`.
///
/// The flame embedded in the animation response is authoritative: an
/// animation is tied to its fractal online by `fractal_id`, and the flame
/// may have been updated since the animation stored a `base_config`
/// (one is stored only if the animation already carried one). The
/// stored copy was preferred whenever it existed, so such an animation
/// loaded onto the flame as it was, not as it is. Without an embedded
/// flame the animation's own `base_config` stands.
pub fn adopt_server_flame(
    animation: &mut crate::animation::Animation,
    server_flame: Option<&FractalConfig>,
) {
    if let Some(config) = server_flame {
        animation.base_config = Some(config.clone());
    }
}

/// The API's base URL, chosen by the build profile (`build.rs`): the
/// deployed server (`https://fractalsforall.com`) for `--profile dist`,
/// what ships; a local one (`http://localhost:3000`) for `--release` and
/// debug builds, for testing against an API run on this machine. Set
/// `FFLAME_API_BASE_URL` when building to choose another.
pub const API_BASE_URL: &str = env!("FFLAME_API_BASE_URL");

use auth::AuthState;
use client::build_url;
use types::*;

/// API server connectivity state (orthogonal to auth status).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiConnectivity {
    /// Haven't checked yet (app just started)
    Unknown,
    /// Server reachable (got any HTTP response)
    Online,
    /// Server unreachable (network error, timeout, DNS failure)
    Unreachable,
}

/// Result of a health check call to GET /api/v1/users/me.
#[derive(Debug, Clone)]
pub enum HealthCheckOutcome {
    /// 200 — token/cookie valid, user authenticated (carries email and user id)
    Authenticated { email: Option<String>, user_id: String },
    /// 401 — token expired or cookie invalid
    TokenExpired,
    /// Other HTTP error (5xx, etc.) — server reachable but unhappy
    ServerError(String),
    /// Network error (timeout, DNS, connection refused)
    NetworkError(String),
}

/// Percent-encode a query-string value.
///
/// Written rather than pulled in: the need is a few free-text query
/// values (the list words', a name in a path), and the unreserved set from RFC 3986 is short
/// enough to state.
pub(crate) fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Perform an API health check by calling GET /api/v1/users/me.
///
/// One failed attempt is retried once, whether it failed as a 401 or
/// as a transport error — see the comment at the retry. What a single
/// outcome MEANS for connectivity is not decided here: that is
/// [`health::HealthPolicy`], which needs two network errors in a row
/// before it calls the server unreachable.
pub async fn check_api_health(base_url: &str, token: &str) -> HealthCheckOutcome {
    let url = build_url(base_url, "/api/v1/users/me");
    let classify =|r: Result<ApiUser, FetchError>| match r {
        Ok(user) => HealthCheckOutcome::Authenticated { email: user.email, user_id: user.id },
        Err(FetchError::Unauthorized) => HealthCheckOutcome::TokenExpired,
        Err(FetchError::Network(msg)) => HealthCheckOutcome::NetworkError(msg),
        Err(other) => HealthCheckOutcome::ServerError(other.to_string()),
    };

    match client::api_get::<ApiUser>(&url, token).await {
        // Both retried once, for the same reason: the first request
        // after an idle stretch may land on a pooled connection the
        // server has already closed. That shows up as a 401 on some
        // stacks and as a transport error on others — a browser reports
        // it as `NetworkError when attempting to fetch resource` — and
        // in either form the second attempt, on a fresh connection, is
        // the one that tells the truth.
        Err(FetchError::Unauthorized) => {
            log::info!("Health check got 401, retrying once...");
            classify(client::api_get::<ApiUser>(&url, token).await)
        }
        Err(FetchError::Network(msg)) => {
            log::info!("Health check network error ({msg}), retrying once...");
            classify(client::api_get::<ApiUser>(&url, token).await)
        }
        other => classify(other),
    }
}

/// Central API state coordinator.
///
/// Manages auth and cached API data (flame list, etc.).
/// API base URL is the hard-coded `API_BASE_URL` constant.
pub struct ApiState {
    /// Authentication state (token, user info)
    pub auth: AuthState,
    /// Cached list of user's flames
    pub flames: Vec<FlameListItem>,
    /// Load state for the flames list
    pub flames_load_state: LoadState,
}

impl Default for ApiState {
    fn default() -> Self {
        Self {
            auth: AuthState::new(),
            flames: Vec::new(),
            flames_load_state: LoadState::NotLoaded,
        }
    }
}

impl ApiState {
    pub fn new(_base_url: &str) -> Self {
        Self {
            ..Default::default()
        }
    }

    /// Set the auth token (called by JS wrapper).
    /// After setting, call `validate_token()` to fetch user info.
    pub fn set_token(&mut self, token: &str) {
        self.auth.set_token(token.to_string());
    }

    /// Clear auth state (sign out).
    pub fn clear_auth(&mut self) {
        self.auth.clear();
        self.flames.clear();
        self.flames_load_state = LoadState::NotLoaded;
    }

    /// Validate auth by fetching user info from /api/v1/users/me.
    /// On desktop, uses the stored Bearer token. On WASM, uses cookies.
    pub async fn validate_token(&mut self) -> FetchResult<()> {
        let token = self.require_token()?;

        let url = build_url(API_BASE_URL, "/api/v1/users/me");
        match client::api_get::<ApiUser>(&url, &token).await {
            Ok(user) => {
                self.auth.set_user(user);
                Ok(())
            }
            Err(e) => {
                self.auth.set_error(e.to_string());
                Err(e)
            }
        }
    }

    /// List the current user's flames.
    pub async fn list_my_flames(
        &mut self,
        page: u32,
        per_page: u32,
    ) -> FetchResult<Vec<FlameListItem>> {
        let token = self.require_token()?;
        let url = build_url(
            API_BASE_URL,
            &format!("/api/v1/users/me/fractals?page={}&per_page={}", page, per_page),
        );

        self.flames_load_state = LoadState::Loading;
        match client::api_get::<types::Lenient<FlameListItem>>(&url, &token).await.map(types::Lenient::into_inner) {
            Ok(flames) => {
                self.flames = flames.clone();
                self.flames_load_state = LoadState::Loaded;
                Ok(flames)
            }
            Err(e) => {
                self.flames_load_state = LoadState::Failed(e.to_string());
                Err(e)
            }
        }
    }

    /// Save a FractalConfig to the API as a new flame.
    /// Returns the server-assigned flame ID.
    ///
    /// Single POST: the palette travels inline on the flame body and is
    /// deduplicated server-side by content hash. Optionally uploads a
    /// thumbnail afterwards.
    pub async fn save_flame(
        &self,
        config: &FractalConfig,
        name: Option<&str>,
        visibility: Option<ApiVisibility>,
        thumbnail_jpg: Option<&[u8]>,
    ) -> FetchResult<String> {
        let token = self.require_token()?;

        let mut flame_req = sync::config_to_create_request(config, name)?;
        flame_req.visibility = visibility;
        let flame_url = build_url(API_BASE_URL, "/api/v1/fractals");
        let flame_resp: FlameResponse =
            client::api_post(&flame_url, &flame_req, &token).await?;
        let flame_id = flame_resp.id;

        if let Some(jpg_data) = thumbnail_jpg {
            self.upload_thumbnail(&flame_id, jpg_data, 512, 512).await?;
        }

        Ok(flame_id)
    }

    /// Upload a JPEG thumbnail for a flame.
    ///
    /// PUT /api/v1/fractals/{id}/thumbnail?width={w}&height={h}
    /// Body: raw JPEG bytes, Content-Type: image/jpeg
    /// Returns 204 No Content on success.
    pub async fn upload_thumbnail(
        &self,
        flame_id: &str,
        jpg_data: &[u8],
        width: u32,
        height: u32,
    ) -> FetchResult<()> {
        let token = self.require_token()?;
        let url = build_url(
            API_BASE_URL,
            &format!(
                "/api/v1/fractals/{}/thumbnail?width={}&height={}",
                flame_id, width, height
            ),
        );
        client::api_put_binary(&url, jpg_data, "image/jpeg", &token).await
    }

    /// Update an existing flame on the API.
    ///
    /// Single PUT with the inline palette payload. The server
    /// recomputes the content hash and stores it on the flame row;
    /// there's no separate palette round trip. When `visibility` is
    /// `None`, we fetch the current flame first to preserve the
    /// existing setting.
    ///
    /// If `thumbnail_jpg` is `Some`, uploads a new thumbnail.
    pub async fn update_flame(
        &self,
        flame_id: &str,
        config: &FractalConfig,
        name: Option<&str>,
        visibility: Option<ApiVisibility>,
        thumbnail_jpg: Option<&[u8]>,
    ) -> FetchResult<FlameResponse> {
        let token = self.require_token()?;

        let effective_visibility = match visibility {
            Some(v) => Some(v),
            None => {
                let current_url = build_url(API_BASE_URL, &format!("/api/v1/fractals/{}", flame_id));
                let current: FlameResponse = client::api_get(&current_url, &token).await?;
                current.visibility
            }
        };

        let mut req = sync::config_to_create_request(config, name)?;
        req.visibility = effective_visibility;

        let url = build_url(API_BASE_URL, &format!("/api/v1/fractals/{}", flame_id));
        let resp: FlameResponse = client::api_put(&url, &req, &token).await?;

        if let Some(jpg_data) = thumbnail_jpg {
            self.upload_thumbnail(flame_id, jpg_data, 512, 512).await?;
        }

        Ok(resp)
    }

    /// Load a flame from the API and convert to FractalConfig.
    pub async fn load_flame(&self, flame_id: &str) -> FetchResult<FractalConfig> {
        let result = self.load_flame_with_visibility(flame_id).await?;
        Ok(result.config)
    }

    /// Load a flame and also return its visibility (true = public).
    ///
    /// A public flame needs no sign-in; a private one is its owner's (the
    /// token, when there is one, says who asks).
    pub async fn load_flame_with_visibility(&self, flame_id: &str) -> FetchResult<FlameLoadResult> {
        let url = build_url(API_BASE_URL, &format!("/api/v1/fractals/{}", flame_id));
        let resp: FlameResponse = self.public_get(&url).await?;

        let is_public = resp.visibility.as_ref().map(|v| matches!(v, ApiVisibility::Public));
        let user_id = resp.owner.as_ref().map(|o| o.id.clone()).unwrap_or_default();
        let animation_count = resp.animation_count;
        let animations = resp.animations.clone();
        let config = sync::flame_response_to_config(&resp)?;
        Ok(FlameLoadResult { config, is_public, user_id, animation_count, animations })
    }

    /// Delete a flame from the API.
    pub async fn delete_flame(&self, flame_id: &str) -> FetchResult<()> {
        let token = self.require_token()?;
        let url = build_url(API_BASE_URL, &format!("/api/v1/fractals/{}", flame_id));
        client::api_delete(&url, &token).await
    }

    /// One page of fractals: the public ones, or yours (`sort` =
    /// `recent`, `updated`, `name`, `popular`, `views`; `render_mode` and
    /// the catalog filters as `filters`).
    pub async fn browse_flames(&self, scope: Scope, words: &Browse) -> FetchResult<types::Page<FlameListItem>> {
        self.browse(scope, "fractals", words).await
    }

    /// One page of palettes, each with its colours (`sort` = `recent`,
    /// `updated`, `name`, `popular`).
    pub async fn browse_palettes(&self, scope: Scope, words: &Browse) -> FetchResult<types::Page<PaletteItem>> {
        self.browse(scope, "palettes", words).await
    }

    /// One page of Rhai scripts of a kind (`sort` = `recent`, `updated`,
    /// `name`). Running one is running somebody's code: see
    /// [`Self::search_public_scripts`].
    pub async fn browse_scripts(&self, scope: Scope, words: &Browse) -> FetchResult<types::Page<types::ScriptListItem>> {
        self.browse(scope, "scripts", words).await
    }

    /// One page of animations (`sort` = `recent`, `updated`, `name`,
    /// `popular`, `views`).
    pub async fn browse_animations(&self, scope: Scope, words: &Browse) -> FetchResult<types::Page<types::AnimationListItem>> {
        self.browse(scope, "animations", words).await
    }

    async fn browse<T: serde::de::DeserializeOwned>(&self, scope: Scope, resource: &str, words: &Browse) -> FetchResult<types::Page<T>> {
        let mut words = words.clone();
        if scope == Scope::Curated {
            words.filters.push(("curated".to_string(), "true".to_string()));
        }
        let url = build_url(API_BASE_URL, &format!("{}{}", scope.path(resource), words.query()));
        match scope {
            Scope::Curated | Scope::Public => self.public_get(&url).await,
            Scope::Mine => client::api_get(&url, &self.require_token()?).await,
        }
    }

    /// One palette, by id (its colours included).
    pub async fn get_palette(&self, id: &str) -> FetchResult<PaletteItem> {
        let url = build_url(API_BASE_URL, &format!("/api/v1/palettes/{}", urlencode(id)));
        self.public_get(&url).await
    }

    /// Whether `resource` (`flames`, `animations`, `palettes`, `scripts`)
    /// has an item `id` the caller can read.
    pub async fn exists(&self, resource: &str, id: &str) -> FetchResult<()> {
        let url = build_url(API_BASE_URL, &format!("/api/v1/{resource}/{}", urlencode(id)));
        self.public_get::<serde_json::Value>(&url).await.map(|_| ())
    }

    /// Many at once (`POST /curated/resolve`, at most 500): each item a
    /// name, a hash, or both; one answer an item, in order, `None` where it
    /// resolves to nothing. A hash alone asks whether that content is
    /// curated -- what the client trusts without asking.
    pub async fn resolve_curated_many(&self, items: &[types::ResolveItem]) -> FetchResult<Vec<Option<types::Resolution>>> {
        #[derive(serde::Serialize)]
        struct Request<'a> {
            items: &'a [types::ResolveItem],
        }
        #[derive(serde::Deserialize)]
        struct Response {
            items: Vec<Option<types::Resolution>>,
        }
        let url = build_url(API_BASE_URL, "/api/v1/curated/resolve");
        let resp: Response = client::api_post_unauth(&url, &Request { items }).await?;
        Ok(resp.items)
    }

    /// What a canonical name means now (`GET /curated/{kind}/{name}`).
    pub async fn resolve_curated(&self, kind: &str, name: &str) -> FetchResult<types::Resolution> {
        let url = build_url(API_BASE_URL, &format!("/api/v1/curated/{}/{}", urlencode(kind), urlencode(name)));
        self.public_get(&url).await
    }

    /// A thumbnail's bytes, by the `thumbnail_url` an item gave (a path
    /// on the API, used as given: it changes when the image does).
    ///
    /// A path is the API's (the local server's too, `build_url`), asked
    /// with the token -- a private fractal's thumbnail is its owner's; a
    /// whole URL elsewhere (a CDN, one day) is asked without it: the
    /// sign-in goes to the API alone.
    pub async fn fetch_thumbnail(&self, thumbnail_url: &str) -> FetchResult<Vec<u8>> {
        if thumbnail_url.starts_with('/') {
            let url = build_url(API_BASE_URL, thumbnail_url);
            #[cfg(not(target_arch = "wasm32"))]
            return client::api_get_bytes_with(&url, self.auth.get_token()).await;
            #[cfg(target_arch = "wasm32")]
            return client::api_get_bytes(&url).await;
        }
        #[cfg(not(target_arch = "wasm32"))]
        return client::api_get_bytes_with(thumbnail_url, None).await;
        #[cfg(target_arch = "wasm32")]
        return client::api_get_bytes(thumbnail_url).await;
    }

    // --- Palettes ---

    /// Publish a palette of the user's (`POST /palettes`), private or
    /// public: a palette item of its own, its colours by hash.
    pub async fn create_palette(&self, palette: &crate::scene::palette::Palette, visibility: ApiVisibility) -> FetchResult<PaletteItem> {
        let token = self.require_token()?;
        let (stops, color_data) = sync::encode_palette_data(palette);
        let req = types::CreatePaletteRequest { name: palette.name.clone(), description: None, visibility, color_data, stops };
        let url = build_url(API_BASE_URL, "/api/v1/palettes");
        client::api_post(&url, &req, &token).await
    }

    /// Rename an item of the caller's, or change who sees it: `resource`
    /// is `flames`, `palettes` or `scripts`.
    pub async fn patch_item(&self, resource: &str, id: &str, patch: &types::ItemPatch) -> FetchResult<serde_json::Value> {
        let token = self.require_token()?;
        let url = build_url(API_BASE_URL, &format!("/api/v1/{resource}/{id}"));
        client::api_patch(&url, patch, &token).await
    }

    /// Delete one of the caller's palettes. Its content stays: fractals
    /// may use it.
    pub async fn delete_palette(&self, id: &str) -> FetchResult<()> {
        let token = self.require_token()?;
        let url = build_url(API_BASE_URL, &format!("/api/v1/palettes/{id}"));
        client::api_delete(&url, &token).await
    }

    // --- Animation operations ---

    /// Save an animation to the API as a new entry.
    /// Returns the server-assigned animation ID.
    pub async fn save_animation(
        &self,
        animation: &crate::animation::Animation,
        name: Option<&str>,
        flame_id: Option<&str>,
        visibility: Option<ApiVisibility>,
    ) -> FetchResult<String> {
        let token = self.require_token()?;
        let req = sync::animation_to_create_request(animation, name, flame_id, visibility);
        let url = build_url(API_BASE_URL, "/api/v1/animations");
        let resp: AnimationResponse = client::api_post(&url, &req, &token).await?;
        Ok(resp.id)
    }

    /// Update an existing animation on the API.
    pub async fn update_animation(
        &self,
        animation_id: &str,
        animation: &crate::animation::Animation,
        name: Option<&str>,
        flame_id: Option<&str>,
        visibility: Option<ApiVisibility>,
    ) -> FetchResult<AnimationResponse> {
        let token = self.require_token()?;
        let req = sync::animation_to_create_request(animation, name, flame_id, visibility);
        let url = build_url(API_BASE_URL, &format!("/api/v1/animations/{}", animation_id));
        client::api_put(&url, &req, &token).await
    }

    /// Load an animation from the API with its embedded flame config.
    ///
    /// Returns (Animation, Option<FractalConfig>, Option<flame_id>).
    /// The embedded flame from the animation response is authoritative —
    /// no separate flame endpoint request needed.
    pub async fn load_animation_full(
        &self,
        animation_id: &str,
    ) -> FetchResult<(crate::animation::Animation, Option<FractalConfig>, Option<String>)> {
        // Public: a curated or someone's public animation loads for a guest
        // too; the token goes along when there is one (a private one of
        // the user's own).
        let url = build_url(API_BASE_URL, &format!("/api/v1/animations/{}", urlencode(animation_id)));
        let resp: AnimationResponse = self.public_get(&url).await?;
        let flame_id = resp.flame_id.clone();

        // Convert embedded flame to FractalConfig (palette travels inline).
        let flame_config = resp
            .flame
            .as_ref()
            .map(sync::flame_response_to_config)
            .transpose()?;

        let mut animation = sync::animation_response_to_animation(&resp);
        adopt_server_flame(&mut animation, flame_config.as_ref());

        Ok((animation, flame_config, flame_id))
    }

    /// Delete an animation from the API.
    pub async fn delete_animation(&self, animation_id: &str) -> FetchResult<()> {
        let token = self.require_token()?;
        let url = build_url(API_BASE_URL, &format!("/api/v1/animations/{}", animation_id));
        client::api_delete(&url, &token).await
    }

    /// List animations for a specific flame.
    pub async fn list_flame_animations(
        &self,
        flame_id: &str,
    ) -> FetchResult<Vec<AnimationListItem>> {
        let token = self.require_token()?;
        let url = build_url(
            API_BASE_URL,
            &format!("/api/v1/fractals/{}/animations?per_page=100", flame_id),
        );
        client::api_get::<types::Lenient<AnimationListItem>>(&url, &token).await.map(types::Lenient::into_inner)
    }

    // --- Helpers ---

    /// A GET the server answers without a sign-in -- the catalogs, a
    /// download, a public script -- with the token when there is one (it
    /// may also see the caller's private rows), unauthenticated when
    /// not: signed out, or a guest. On WASM the cookie decides either way.
    async fn public_get<T: serde::de::DeserializeOwned>(&self, url: &str) -> FetchResult<T> {
        #[cfg(target_arch = "wasm32")]
        return client::api_get(url, "").await;

        #[cfg(not(target_arch = "wasm32"))]
        match self.auth.get_token() {
            Some(token) => client::api_get(url, token).await,
            None => client::api_get_unauth(url).await,
        }
    }

    fn require_token(&self) -> FetchResult<String> {
        // On WASM, auth is handled via cookies — no token needed.
        // Return empty string so the client signature is satisfied (it ignores the token).
        #[cfg(target_arch = "wasm32")]
        return Ok(String::new());

        #[cfg(not(target_arch = "wasm32"))]
        self.auth
            .get_token()
            .map(|t| t.to_string())
            .ok_or(FetchError::Unauthorized)
    }

    /// Fetch the full definition of a variation by name.
    /// Variations are public — no authentication required.
    // ========================================================================
    // Scripts
    // ========================================================================

    /// Create a script. Returns the server's copy — including the id,
    /// and the version to hold for the next update.
    ///
    /// `req` from `sync::script_to_create_request` (a CPU script) or
    /// `entries::create_request` (a formula-library entry, under its own
    /// id: a 409 when that id is published already).
    pub async fn create_script(&self, req: &types::ScriptCreateRequest) -> FetchResult<types::ScriptResponse> {
        let token = self.require_token()?;
        let url = build_url(API_BASE_URL, "/api/v1/scripts");
        client::api_post(&url, req, &token).await
    }

    /// Fetch one script, source included.
    pub async fn load_script(&self, id: &str) -> FetchResult<types::ScriptResponse> {
        let url = build_url(API_BASE_URL, &format!("/api/v1/scripts/{id}"));
        self.public_get(&url).await
    }

    /// Update a script, optimistically.
    ///
    /// `version` is the one the client last read; the server refuses
    /// with 409 if somebody wrote first. Not pessimism — a desktop app
    /// and a browser tab share no store, which is exactly the case
    /// last-write-wins loses data in, silently and with nothing to
    /// notice it by.
    ///
    /// On refusal, [`types::ScriptConflict::from_error`] recovers the
    /// current version so the caller can offer a choice rather than an
    /// apology.
    ///
    /// `req` carries the `version` (built as for [`Self::create_script`],
    /// with `Some(version)`).
    pub async fn update_script(&self, id: &str, req: &types::ScriptCreateRequest) -> FetchResult<types::ScriptResponse> {
        let token = self.require_token()?;
        let url = build_url(API_BASE_URL, &format!("/api/v1/scripts/{id}"));
        client::api_put(&url, req, &token).await
    }

    pub async fn delete_script(&self, id: &str) -> FetchResult<()> {
        let token = self.require_token()?;
        let url = build_url(API_BASE_URL, &format!("/api/v1/scripts/{id}"));
        client::api_delete(&url, &token).await
    }

    // ========================================================================
    // Effects
    // ========================================================================

    /// The effect catalog — everything the server knows, without shaders.
    pub async fn list_effects(&self) -> FetchResult<Vec<types::EffectListItem>> {
        self.every_page("/api/v1/effects").await
    }

    /// Every page of a public list, following `next` (100 to a page, the
    /// most the server serves).
    async fn every_page<T: serde::de::DeserializeOwned>(&self, path: &str) -> FetchResult<Vec<T>> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        // A bound well past any catalog: a server that never says the end
        // must not keep a client asking for ever.
        for _ in 0..100 {
            let query = match &cursor {
                Some(c) => format!("?per_page=100&cursor={}", urlencode(c)),
                None => "?per_page=100".to_string(),
            };
            let url = build_url(API_BASE_URL, &format!("{path}{query}"));
            let page: types::Page<T> = self.public_get(&url).await?;
            out.extend(page.items.into_inner());
            match page.next {
                Some(next) => cursor = Some(next),
                None => return Ok(out),
            }
        }
        Ok(out)
    }

    /// One effect, with its WGSL.
    ///
    /// `shader` may still be null — the server serves the row before its
    /// shader is seeded, and says so through `downloadable`. Registration
    /// refuses a null one rather than adding an effect that renders
    /// nothing.
    pub async fn fetch_effect(&self, name: &str) -> FetchResult<types::EffectDownload> {
        let url = build_url(API_BASE_URL, &format!("/api/v1/effects/{}", urlencode(name)));
        self.public_get(&url).await
    }

    pub async fn fetch_variation(&self, name: &str) -> FetchResult<types::VariationDownload> {
        // Publicly readable: a guest's request goes unsigned (never an
        // empty `Bearer`), a signed-in one carries the token.
        let url = build_url(API_BASE_URL, &format!("/api/v1/variations/{}", urlencode(name)));
        self.public_get(&url).await
    }

    /// List all available variations on the server (summary form, no shader code).
    pub async fn list_variations(&self) -> FetchResult<Vec<types::VariationListItem>> {
        self.every_page("/api/v1/variations").await
    }
}

/// The newest app release, as `GET /api/v1/app/latest` gives it
/// (docs/archive/projects/api-updates.md, rule 7).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AppLatest {
    pub version: String,
    #[serde(default)]
    pub url: Option<String>,
}

/// Ask the server for the newest release. `None` on any failure -- the
/// endpoint is newer than this client, and an update notice is never worth
/// an error.
pub async fn latest_app_version(base_url: &str) -> Option<AppLatest> {
    let url = build_url(base_url, "/api/v1/app/latest");
    client::api_get_unauth::<AppLatest>(&url).await.ok()
}

/// Whether release `latest` is newer than `current`, comparing dotted
/// numbers (`0.6.0` < `0.6.1` < `0.10.0`); anything after a `-` or `+`
/// is ignored, and a version that does not parse is never newer.
pub fn is_newer_release(latest: &str, current: &str) -> bool {
    fn parts(v: &str) -> Option<Vec<u64>> {
        let core = v.trim().trim_start_matches('v').split(['-', '+']).next()?;
        core.split('.').map(|p| p.parse().ok()).collect()
    }
    match (parts(latest), parts(current)) {
        (Some(mut a), Some(mut b)) => {
            let n = a.len().max(b.len());
            a.resize(n, 0);
            b.resize(n, 0);
            a > b
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--profile dist` (what ships) talks to the deployed server; a
    /// `--release` or debug build to one on this machine (`build.rs`).
    #[test]
    fn the_api_follows_the_build_profile() {
        if env!("FFLAME_API_BASE_URL_CHOSEN_BY") == "override" {
            return;
        }
        match env!("BUILD_PROFILE") {
            "release" | "debug" => assert_eq!(API_BASE_URL, "http://localhost:3000"),
            _ => assert_eq!(API_BASE_URL, "https://fractalsforall.com"),
        }
    }

    #[test]
    fn release_versions_compare_as_numbers() {
        assert!(is_newer_release("0.6.1", "0.6.0"));
        assert!(is_newer_release("0.10.0", "0.9.9"));
        assert!(is_newer_release("1.0", "0.9.9"));
        assert!(!is_newer_release("0.6.0", "0.6.0"));
        assert!(!is_newer_release("0.6.0-beta", "0.6.0"));
        assert!(!is_newer_release("0.5.9", "0.6.0"));
        assert!(!is_newer_release("soon", "0.6.0"));
    }

    /// A page's words: the empty ones left out, the free text encoded,
    /// the cursor last; your own list under `users/me`.
    #[test]
    fn browse_words_make_the_query() {
        assert_eq!(Browse::default().query(), "");
        let words = Browse {
            q: " fire & ice ".into(),
            sort: Some("popular".into()),
            kind: Some("formula".into()),
            cursor: Some("abc=".into()),
            per_page: 500,
            filters: vec![("render_mode".into(), "2d".into())],
        };
        assert_eq!(words.query(), "?q=fire+%26+ice&sort=popular&kind=formula&render_mode=2d&per_page=100&cursor=abc%3D");
        assert_eq!(Scope::Public.path("fractals"), "/api/v1/fractals");
        assert_eq!(Scope::Curated.path("animations"), "/api/v1/animations");
        assert_eq!(Scope::Mine.path("palettes"), "/api/v1/users/me/palettes");
        assert_eq!(Scope::default(), Scope::Curated, "the app browses the curated collection");
    }
    use crate::scene::transforms::RenderMode;

    /// Simulations save online like every other config (the server stores
    /// them from migration 102): no client-side refusal any more. Signed
    /// out, a save gets as far as the sign-in check, and the payload names
    /// the mode.
    #[test]
    fn simulation_configs_go_online_like_any_other() {
        let mut config = FractalConfig::default();
        config.render_mode = RenderMode::Simulation;
        let api = ApiState::default();
        let save = pollster::block_on(api.save_flame(&config, Some("x"), None, None));
        assert!(!matches!(save, Err(FetchError::Unsupported(_))), "no refusal: {save:?}");
        let update = pollster::block_on(api.update_flame("id", &config, None, None, None));
        assert!(!matches!(update, Err(FetchError::Unsupported(_))), "no refusal: {update:?}");
        let req = sync::config_to_create_request(&config, Some("x")).expect("payload");
        assert_eq!(serde_json::to_value(&req).unwrap()["config"]["render_mode"], "simulation");
    }

    /// What a flame Update sends for "Make public". The case that was
    /// wrong: a public flame with the box unchecked sent nothing, and
    /// `update_flame` read that as "keep public".
    #[test]
    fn unchecking_make_public_on_a_public_flame_makes_it_private() {
        use types::ApiVisibility::{Private, Public};
        assert_eq!(flame_update_visibility(false, Some(true)), Some(Private));
        // Checked is public whatever it was.
        for was in [Some(true), Some(false), None] {
            assert_eq!(flame_update_visibility(true, was), Some(Public), "{was:?}");
        }
        // Unchecked on a flame that was not public (private or
        // unlisted), or whose visibility is unknown: keep the server's.
        assert_eq!(flame_update_visibility(false, Some(false)), None);
        assert_eq!(flame_update_visibility(false, None), None);
    }

    /// The server's flame replaces an animation's stored base config;
    /// without one, the stored config stands.
    #[test]
    fn the_server_flame_is_the_animations_base_config() {
        let mut stored = FractalConfig::default();
        stored.flame.name = "stored".into();
        let mut server = FractalConfig::default();
        server.flame.name = "server".into();

        let mut anim = crate::animation::Animation::with_config("A".into(), 5.0, stored.clone());
        adopt_server_flame(&mut anim, Some(&server));
        assert_eq!(anim.base_config.as_ref().unwrap().flame.name, "server");

        let mut anim = crate::animation::Animation::with_config("A".into(), 5.0, stored);
        adopt_server_flame(&mut anim, None);
        assert_eq!(anim.base_config.as_ref().unwrap().flame.name, "stored");

        let mut anim = crate::animation::Animation::new("A".into(), 5.0);
        adopt_server_flame(&mut anim, Some(&server));
        assert_eq!(anim.base_config.as_ref().unwrap().flame.name, "server");
    }
}

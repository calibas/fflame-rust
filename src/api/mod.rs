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
pub mod health;
pub mod sync;
pub mod types;

use crate::config::FractalConfig;
use crate::resources::{FetchError, FetchResult, LoadState};

/// Result of loading a flame from the API, including metadata for ownership tracking.
pub struct FlameLoadResult {
    pub config: FractalConfig,
    pub is_public: Option<bool>,
    pub user_id: String,
    pub animation_count: u32,
    pub animations: Vec<types::AnimationSummary>,
}

/// Hard-coded API base URL. Change this for release builds.
pub const API_BASE_URL: &str = "https://fractalsforall.com";
// pub const API_BASE_URL: &str = "http://localhost:3000";

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

/// Result of a health check call to GET /api/users/me.
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
/// Written rather than pulled in: the only need is one parameter, and
/// the unreserved set from RFC 3986 is short enough to state.
///
/// NOTE: `SearchFlamesParams::to_query_string` interpolates its values
/// raw, so a flame name containing `&` or `#` still corrupts that URL.
/// Left alone here because it is a live path with its own behaviour to
/// re-check, not because it is correct.
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

/// Perform an API health check by calling GET /api/users/me.
///
/// One failed attempt is retried once, whether it failed as a 401 or
/// as a transport error — see the comment at the retry. What a single
/// outcome MEANS for connectivity is not decided here: that is
/// [`health::HealthPolicy`], which needs two network errors in a row
/// before it calls the server unreachable.
pub async fn check_api_health(base_url: &str, token: &str) -> HealthCheckOutcome {
    let url = build_url(base_url, "/api/users/me");
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

    /// Refuse configs the server cannot store yet, BEFORE anything is
    /// sent — and before the token check, so the refusal is the same
    /// signed in or out.
    ///
    /// The server casts `render_mode` into a Postgres enum with no
    /// "escape" value; uploading one would be a 500 on their side
    /// instead of a sentence on ours. Nothing forces this coordination
    /// (verified: the engine contract carries no render-mode
    /// vocabulary), so this guard is the only seatbelt. Remove it when
    /// the API adds the value.
    fn refuse_unsupported_online(config: &FractalConfig) -> FetchResult<()> {
        if config.render_mode == crate::scene::transforms::RenderMode::Escape {
            return Err(FetchError::Unsupported(
                "Escape-time fractals can't be saved online yet — the server \
                 doesn't know this render mode. Save locally for now."
                    .to_string(),
            ));
        }
        Ok(())
    }

    /// Clear auth state (sign out).
    pub fn clear_auth(&mut self) {
        self.auth.clear();
        self.flames.clear();
        self.flames_load_state = LoadState::NotLoaded;
    }

    /// Validate auth by fetching user info from /api/users/me.
    /// On desktop, uses the stored Bearer token. On WASM, uses cookies.
    pub async fn validate_token(&mut self) -> FetchResult<()> {
        let token = self.require_token()?;

        let url = build_url(API_BASE_URL, "/api/users/me");
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
            &format!("/api/flames?page={}&per_page={}", page, per_page),
        );

        self.flames_load_state = LoadState::Loading;
        match client::api_get::<Vec<FlameListItem>>(&url, &token).await {
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
        Self::refuse_unsupported_online(config)?;
        let token = self.require_token()?;

        let mut flame_req = sync::config_to_create_request(config, name)?;
        flame_req.visibility = visibility;
        let flame_url = build_url(API_BASE_URL, "/api/flames");
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
    /// PUT /api/flames/{id}/thumbnail?width={w}&height={h}
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
                "/api/flames/{}/thumbnail?width={}&height={}",
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
        Self::refuse_unsupported_online(config)?;
        let token = self.require_token()?;

        let effective_visibility = match visibility {
            Some(v) => Some(v),
            None => {
                let current_url = build_url(API_BASE_URL, &format!("/api/flames/{}", flame_id));
                let current: FlameResponse = client::api_get(&current_url, &token).await?;
                current.visibility
            }
        };

        let mut req = sync::config_to_create_request(config, name)?;
        req.visibility = effective_visibility;

        let url = build_url(API_BASE_URL, &format!("/api/flames/{}", flame_id));
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
    pub async fn load_flame_with_visibility(&self, flame_id: &str) -> FetchResult<FlameLoadResult> {
        let token = self.require_token()?;
        let url = build_url(API_BASE_URL, &format!("/api/flames/{}", flame_id));
        let resp: FlameResponse = client::api_get(&url, &token).await?;

        let is_public = resp.visibility.as_ref().map(|v| matches!(v, ApiVisibility::Public));
        let user_id = resp.user_id.clone();
        let animation_count = resp.animation_count;
        let animations = resp.animations.clone();
        let config = sync::flame_response_to_config(&resp)?;
        Ok(FlameLoadResult { config, is_public, user_id, animation_count, animations })
    }

    /// Delete a flame from the API.
    pub async fn delete_flame(&self, flame_id: &str) -> FetchResult<()> {
        let token = self.require_token()?;
        let url = build_url(API_BASE_URL, &format!("/api/flames/{}", flame_id));
        client::api_delete(&url, &token).await
    }

    /// Search flames with filters.
    pub async fn search_flames(
        &self,
        params: &SearchFlamesParams,
    ) -> FetchResult<Vec<FlameListItem>> {
        let token = self.require_token()?;
        let url = build_url(
            API_BASE_URL,
            &format!("/api/search/flames{}", params.to_query_string()),
        );
        client::api_get(&url, &token).await
    }

    /// Search public flames without auth (for gallery browsing).
    pub async fn search_public_flames(
        &self,
        params: &SearchFlamesParams,
    ) -> FetchResult<Vec<FlameListItem>> {
        let url = build_url(
            API_BASE_URL,
            &format!("/api/search/flames{}", params.to_query_string()),
        );
        client::api_get_unauth(&url).await
    }

    // --- Palette library operations ---

    /// List the caller's bookmarked palette library entries (the
    /// `/api/users/me/palettes` endpoint). Each entry includes the
    /// content hash, optional nickname, and full palette content.
    pub async fn list_library_palettes(
        &self,
        page: u32,
        per_page: u32,
    ) -> FetchResult<Vec<LibraryPaletteEntry>> {
        let token = self.require_token()?;
        let url = build_url(
            API_BASE_URL,
            &format!("/api/users/me/palettes?page={}&per_page={}", page, per_page),
        );
        client::api_get(&url, &token).await
    }

    /// Fetch a single palette by content hash. Public — no auth required.
    pub async fn get_palette(&self, hash: &str) -> FetchResult<ApiPalette> {
        let url = build_url(API_BASE_URL, &format!("/api/palettes/{}", hash));
        client::api_get_unauth(&url).await
    }

    /// Set or update the caller's personal nickname for a library entry.
    /// 404 if the content hash isn't yet in `palette_data`.
    pub async fn set_library_nickname(
        &self,
        hash: &str,
        nickname: Option<&str>,
    ) -> FetchResult<()> {
        let token = self.require_token()?;
        let url = build_url(
            API_BASE_URL,
            &format!("/api/users/me/palettes/{}", hash),
        );
        let req = UpdateLibraryNicknameRequest {
            nickname: nickname.map(|s| s.to_string()),
        };
        let _: serde_json::Value = client::api_put(&url, &req, &token).await?;
        Ok(())
    }

    /// Remove the caller's library entry for the given content hash.
    /// Never deletes the underlying palette content (palettes are shared
    /// and reference-counted by flames).
    pub async fn remove_library_palette(&self, hash: &str) -> FetchResult<()> {
        let token = self.require_token()?;
        let url = build_url(
            API_BASE_URL,
            &format!("/api/users/me/palettes/{}", hash),
        );
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
        let url = build_url(API_BASE_URL, "/api/animations");
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
        let url = build_url(API_BASE_URL, &format!("/api/animations/{}", animation_id));
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
        let token = self.require_token()?;
        let url = build_url(API_BASE_URL, &format!("/api/animations/{}", animation_id));
        let resp: AnimationResponse = client::api_get(&url, &token).await?;
        let flame_id = resp.flame_id.clone();

        // Convert embedded flame to FractalConfig (palette travels inline).
        let flame_config = resp
            .flame
            .as_ref()
            .map(sync::flame_response_to_config)
            .transpose()?;

        let mut animation = sync::animation_response_to_animation(&resp);

        // Use the embedded flame as the animation's base_config (authoritative source)
        if animation.base_config.is_none() {
            if let Some(ref config) = flame_config {
                animation.base_config = Some(config.clone());
            }
        }

        Ok((animation, flame_config, flame_id))
    }

    /// Delete an animation from the API.
    pub async fn delete_animation(&self, animation_id: &str) -> FetchResult<()> {
        let token = self.require_token()?;
        let url = build_url(API_BASE_URL, &format!("/api/animations/{}", animation_id));
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
            &format!("/api/flames/{}/animations", flame_id),
        );
        client::api_get(&url, &token).await
    }

    // --- Helpers ---

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

    /// The caller's own scripts.
    pub async fn list_my_scripts(
        &self,
        page: u32,
        per_page: u32,
    ) -> FetchResult<Vec<types::ScriptListItem>> {
        let token = self.require_token()?;
        let url = build_url(
            API_BASE_URL,
            &format!("/api/scripts?page={page}&per_page={per_page}"),
        );
        client::api_get(&url, &token).await
    }

    /// Create a script. Returns the server's copy — including the id,
    /// and the version to hold for the next update.
    pub async fn create_script(
        &self,
        name: &str,
        source: &str,
        visibility: Option<ApiVisibility>,
    ) -> FetchResult<types::ScriptResponse> {
        let token = self.require_token()?;
        let req = sync::script_to_create_request(name, source, visibility, None)
            .map_err(FetchError::Parse)?;
        let url = build_url(API_BASE_URL, "/api/scripts");
        client::api_post(&url, &req, &token).await
    }

    /// Fetch one script, source included.
    pub async fn load_script(&self, id: &str) -> FetchResult<types::ScriptResponse> {
        let token = self.require_token()?;
        let url = build_url(API_BASE_URL, &format!("/api/scripts/{id}"));
        client::api_get(&url, &token).await
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
    pub async fn update_script(
        &self,
        id: &str,
        name: &str,
        source: &str,
        visibility: Option<ApiVisibility>,
        version: u32,
    ) -> FetchResult<types::ScriptResponse> {
        let token = self.require_token()?;
        let req = sync::script_to_create_request(name, source, visibility, Some(version))
            .map_err(FetchError::Parse)?;
        let url = build_url(API_BASE_URL, &format!("/api/scripts/{id}"));
        client::api_put(&url, &req, &token).await
    }

    pub async fn delete_script(&self, id: &str) -> FetchResult<()> {
        let token = self.require_token()?;
        let url = build_url(API_BASE_URL, &format!("/api/scripts/{id}"));
        client::api_delete(&url, &token).await
    }

    /// Public script search.
    ///
    /// This is the point where a user can run code somebody else wrote.
    /// Whatever surfaces these results owes them visible provenance, and
    /// anything loaded from here must be run through
    /// `ScriptHost::with_untrusted` — which is what makes running one
    /// survivable rather than merely allowed.
    pub async fn search_public_scripts(
        &self,
        query: &str,
        page: u32,
        per_page: u32,
    ) -> FetchResult<Vec<types::ScriptListItem>> {
        let token = self.require_token()?;
        let url = build_url(
            API_BASE_URL,
            &format!(
                "/api/search/scripts?q={}&page={page}&per_page={per_page}",
                urlencode(query)
            ),
        );
        client::api_get(&url, &token).await
    }

    // ========================================================================
    // Effects
    // ========================================================================

    /// The effect catalog — everything the server knows, without shaders.
    pub async fn list_effects(&self) -> FetchResult<Vec<types::EffectListItem>> {
        let token = self.require_token()?;
        let url = build_url(API_BASE_URL, "/api/effects");
        client::api_get(&url, &token).await
    }

    /// One effect, with its WGSL.
    ///
    /// `shader` may still be null — the server serves the row before its
    /// shader is seeded, and says so through `downloadable`. Registration
    /// refuses a null one rather than adding an effect that renders
    /// nothing.
    pub async fn fetch_effect(&self, name: &str) -> FetchResult<types::EffectDownload> {
        let token = self.require_token()?;
        let url = build_url(API_BASE_URL, &format!("/api/effects/{name}"));
        client::api_get(&url, &token).await
    }

    pub async fn fetch_variation(&self, name: &str) -> FetchResult<types::VariationDownload> {
        let url = build_url(API_BASE_URL, &format!("/api/variations/{}", name));
        // Use empty token — variations are publicly readable
        client::api_get::<types::VariationDownload>(&url, "").await
    }

    /// List all available variations on the server (summary form, no shader code).
    pub async fn list_variations(&self) -> FetchResult<Vec<types::VariationListItem>> {
        let url = build_url(API_BASE_URL, "/api/variations");
        client::api_get::<Vec<types::VariationListItem>>(&url, "").await
    }
}

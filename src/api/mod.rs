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
pub mod sync;
pub mod types;

use crate::config::FractalConfig;
use crate::resources::{FetchError, FetchResult, LoadState};

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
    /// 200 — token/cookie valid, user authenticated (carries email if available)
    Authenticated(Option<String>),
    /// 401 — token expired or cookie invalid
    TokenExpired,
    /// Other HTTP error (5xx, etc.) — server reachable but unhappy
    ServerError(String),
    /// Network error (timeout, DNS, connection refused)
    NetworkError(String),
}

/// Perform an API health check by calling GET /api/users/me.
///
/// On 401, retries once before reporting TokenExpired. This avoids
/// false logouts caused by stale pooled connections after sleep/wake
/// — a dead TCP socket can produce spurious 401s on the first attempt.
pub async fn check_api_health(base_url: &str, token: &str) -> HealthCheckOutcome {
    let url = build_url(base_url, "/api/users/me");
    match client::api_get::<ApiUser>(&url, token).await {
        Ok(user) => HealthCheckOutcome::Authenticated(user.email),
        Err(FetchError::Unauthorized) => {
            // Retry once — stale connections after sleep can produce spurious 401s
            log::info!("Health check got 401, retrying once...");
            match client::api_get::<ApiUser>(&url, token).await {
                Ok(user) => HealthCheckOutcome::Authenticated(user.email),
                Err(FetchError::Unauthorized) => HealthCheckOutcome::TokenExpired,
                Err(FetchError::Network(msg)) => HealthCheckOutcome::NetworkError(msg),
                Err(other) => HealthCheckOutcome::ServerError(other.to_string()),
            }
        }
        Err(FetchError::Network(msg)) => HealthCheckOutcome::NetworkError(msg),
        Err(other) => HealthCheckOutcome::ServerError(other.to_string()),
    }
}

/// Central API state coordinator.
///
/// Manages auth, base URL, and cached API data (flame list, etc.).
pub struct ApiState {
    /// Authentication state (token, user info)
    pub auth: AuthState,
    /// Base URL for the API (e.g., "https://api.fflame.app")
    pub base_url: String,
    /// Cached list of user's flames
    pub flames: Vec<FlameListItem>,
    /// Load state for the flames list
    pub flames_load_state: LoadState,
}

impl Default for ApiState {
    fn default() -> Self {
        Self {
            auth: AuthState::new(),
            base_url: String::new(),
            flames: Vec::new(),
            flames_load_state: LoadState::NotLoaded,
        }
    }
}

impl ApiState {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
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

    /// Validate auth by fetching user info from /api/users/me.
    /// On desktop, uses the stored Bearer token. On WASM, uses cookies.
    pub async fn validate_token(&mut self) -> FetchResult<()> {
        let token = self.require_token()?;

        let url = build_url(&self.base_url, "/api/users/me");
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
            &self.base_url,
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
    /// Order: create flame first, then palette, then update the flame
    /// to link the palette. Optionally upload thumbnail and set visibility.
    pub async fn save_flame(
        &self,
        config: &FractalConfig,
        name: Option<&str>,
        visibility: Option<ApiVisibility>,
        thumbnail_jpg: Option<&[u8]>,
    ) -> FetchResult<String> {
        let token = self.require_token()?;

        // 1. Create the flame (without palette)
        let mut flame_req = sync::config_to_create_request(config, name);
        flame_req.visibility = visibility;
        let flame_url = build_url(&self.base_url, "/api/flames");
        let flame_resp: FlameResponse =
            client::api_post(&flame_url, &flame_req, &token).await?;
        let flame_id = flame_resp.id;

        // 2. Create the palette (same visibility as flame)
        let palette_req = sync::palette_to_create_request(&config.palette, visibility);
        let palette_url = build_url(&self.base_url, "/api/palettes");
        let palette_resp: PaletteResponse =
            client::api_post(&palette_url, &palette_req, &token).await?;

        // 3. Update the flame to link the palette
        let update_url = build_url(&self.base_url, &format!("/api/flames/{}", flame_id));
        let mut update_req = sync::config_to_create_request(config, name);
        update_req.palette_id = Some(palette_resp.id);
        update_req.visibility = visibility;
        let _: FlameResponse = client::api_put(&update_url, &update_req, &token).await?;

        // 4. Upload thumbnail if provided
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
            &self.base_url,
            &format!(
                "/api/flames/{}/thumbnail?width={}&height={}",
                flame_id, width, height
            ),
        );
        client::api_put_binary(&url, jpg_data, "image/jpeg", &token).await
    }

    /// Update an existing flame on the API.
    pub async fn update_flame(
        &self,
        flame_id: &str,
        config: &FractalConfig,
        name: Option<&str>,
    ) -> FetchResult<FlameResponse> {
        let token = self.require_token()?;
        let req = sync::config_to_create_request(config, name);
        let url = build_url(&self.base_url, &format!("/api/flames/{}", flame_id));
        client::api_put(&url, &req, &token).await
    }

    /// Load a flame from the API and convert to FractalConfig.
    pub async fn load_flame(&self, flame_id: &str) -> FetchResult<FractalConfig> {
        let token = self.require_token()?;
        let url = build_url(&self.base_url, &format!("/api/flames/{}", flame_id));
        let resp: FlameResponse = client::api_get(&url, &token).await?;

        // Load palette if referenced
        let palette_resp = if let Some(ref palette_id) = resp.palette_id {
            let palette_url = build_url(&self.base_url, &format!("/api/palettes/{}", palette_id));
            client::api_get::<PaletteResponse>(&palette_url, &token)
                .await
                .ok()
        } else {
            None
        };

        Ok(sync::flame_response_to_config(
            &resp,
            palette_resp.as_ref(),
        ))
    }

    /// Delete a flame from the API.
    pub async fn delete_flame(&self, flame_id: &str) -> FetchResult<()> {
        let token = self.require_token()?;
        let url = build_url(&self.base_url, &format!("/api/flames/{}", flame_id));
        client::api_delete(&url, &token).await
    }

    /// Search flames with filters.
    pub async fn search_flames(
        &self,
        params: &SearchFlamesParams,
    ) -> FetchResult<Vec<FlameListItem>> {
        let token = self.require_token()?;
        let url = build_url(
            &self.base_url,
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
            &self.base_url,
            &format!("/api/search/flames{}", params.to_query_string()),
        );
        client::api_get_unauth(&url).await
    }

    // --- Palette operations ---

    /// List palettes, optionally filtered by visibility.
    pub async fn list_palettes(
        &self,
        visibility: Option<ApiPaletteVisibility>,
        page: u32,
        per_page: u32,
    ) -> FetchResult<Vec<PaletteResponse>> {
        let token = self.require_token()?;
        let visibility_param = visibility
            .map(|v| {
                let v = serde_json::to_value(v)
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default();
                format!("&visibility={}", v)
            })
            .unwrap_or_default();
        let url = build_url(
            &self.base_url,
            &format!("/api/palettes?page={}&per_page={}{}", page, per_page, visibility_param),
        );
        client::api_get(&url, &token).await
    }

    /// Get a single palette by ID.
    pub async fn get_palette(&self, palette_id: &str) -> FetchResult<PaletteResponse> {
        let token = self.require_token()?;
        let url = build_url(&self.base_url, &format!("/api/palettes/{}", palette_id));
        client::api_get(&url, &token).await
    }

    /// Update an existing palette.
    pub async fn update_palette(
        &self,
        palette_id: &str,
        req: &UpdatePaletteRequest,
    ) -> FetchResult<PaletteResponse> {
        let token = self.require_token()?;
        let url = build_url(&self.base_url, &format!("/api/palettes/{}", palette_id));
        client::api_put(&url, req, &token).await
    }

    /// Delete a palette.
    pub async fn delete_palette(&self, palette_id: &str) -> FetchResult<()> {
        let token = self.require_token()?;
        let url = build_url(&self.base_url, &format!("/api/palettes/{}", palette_id));
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
        let url = build_url(&self.base_url, "/api/animations");
        let resp: AnimationResponse = client::api_post(&url, &req, &token).await?;
        Ok(resp.id)
    }

    /// Update an existing animation on the API.
    pub async fn update_animation(
        &self,
        animation_id: &str,
        animation: &crate::animation::Animation,
        name: Option<&str>,
    ) -> FetchResult<AnimationResponse> {
        let token = self.require_token()?;
        let req = sync::animation_to_create_request(animation, name, None, None);
        let url = build_url(&self.base_url, &format!("/api/animations/{}", animation_id));
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
        let url = build_url(&self.base_url, &format!("/api/animations/{}", animation_id));
        let resp: AnimationResponse = client::api_get(&url, &token).await?;
        let flame_id = resp.flame_id.clone();

        // Convert embedded flame to FractalConfig (fetch palette if referenced)
        let flame_config = if let Some(ref flame_resp) = resp.flame {
            let palette_resp = if let Some(ref palette_id) = flame_resp.palette_id {
                let palette_url = build_url(&self.base_url, &format!("/api/palettes/{}", palette_id));
                client::api_get::<PaletteResponse>(&palette_url, &token)
                    .await
                    .ok()
            } else {
                None
            };
            Some(sync::flame_response_to_config(flame_resp, palette_resp.as_ref()))
        } else {
            None
        };

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
        let url = build_url(&self.base_url, &format!("/api/animations/{}", animation_id));
        client::api_delete(&url, &token).await
    }

    /// List animations for a specific flame.
    pub async fn list_flame_animations(
        &self,
        flame_id: &str,
    ) -> FetchResult<Vec<AnimationListItem>> {
        let token = self.require_token()?;
        let url = build_url(
            &self.base_url,
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
}

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

    /// Validate the current token by fetching user info.
    pub async fn validate_token(&mut self) -> FetchResult<()> {
        let token = self
            .auth
            .get_token()
            .ok_or_else(|| FetchError::Network("No auth token set".to_string()))?
            .to_string();

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
    pub async fn save_flame(
        &self,
        config: &FractalConfig,
        name: Option<&str>,
    ) -> FetchResult<String> {
        let token = self.require_token()?;

        // First, save the palette
        let palette_req = sync::palette_to_create_request(&config.palette);
        let palette_url = build_url(&self.base_url, "/api/palettes");
        let palette_resp: PaletteResponse =
            client::api_post(&palette_url, &palette_req, &token).await?;

        // Then save the flame with the palette ID
        let mut flame_req = sync::config_to_create_request(config, name);
        flame_req.palette_id = Some(palette_resp.id);

        let url = build_url(&self.base_url, "/api/flames");
        let resp: FlameResponse = client::api_post(&url, &flame_req, &token).await?;
        Ok(resp.id)
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

    // --- Helpers ---

    fn require_token(&self) -> FetchResult<String> {
        self.auth
            .get_token()
            .map(|t| t.to_string())
            .ok_or(FetchError::Unauthorized)
    }
}

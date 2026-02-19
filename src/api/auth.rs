//! Authentication state and token management.
//!
//! Tokens are provided by the JS wrapper (WASM) or stored in the filesystem (desktop).
//! The app never shows a login form — authentication happens on the website.

use super::types::ApiUser;

/// Current authentication status.
#[derive(Debug, Clone, PartialEq)]
pub enum AuthStatus {
    /// Not authenticated
    SignedOut,
    /// Authenticated with valid token
    SignedIn,
    /// Checking token validity
    Loading,
    /// Authentication failed
    Error(String),
}

/// Authentication state for API access.
#[derive(Debug, Clone)]
pub struct AuthState {
    /// JWT bearer token (None when signed out)
    pub token: Option<String>,
    /// Current user info (fetched after token is set)
    pub user: Option<ApiUser>,
    /// Current auth status
    pub status: AuthStatus,
}

impl Default for AuthState {
    fn default() -> Self {
        Self {
            token: None,
            user: None,
            status: AuthStatus::SignedOut,
        }
    }
}

impl AuthState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the auth token (called by JS wrapper or after login).
    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
        self.status = AuthStatus::Loading; // Will be set to SignedIn after /users/me succeeds
    }

    /// Set user info after successful /users/me call.
    pub fn set_user(&mut self, user: ApiUser) {
        self.user = Some(user);
        self.status = AuthStatus::SignedIn;
    }

    /// Mark auth as failed.
    pub fn set_error(&mut self, message: String) {
        self.status = AuthStatus::Error(message);
        self.token = None;
        self.user = None;
    }

    /// Clear all auth state (sign out).
    pub fn clear(&mut self) {
        self.token = None;
        self.user = None;
        self.status = AuthStatus::SignedOut;
    }

    /// Check if the user is signed in.
    pub fn is_signed_in(&self) -> bool {
        self.status == AuthStatus::SignedIn && self.token.is_some()
    }

    /// Get the token if signed in.
    pub fn get_token(&self) -> Option<&str> {
        self.token.as_deref()
    }
}

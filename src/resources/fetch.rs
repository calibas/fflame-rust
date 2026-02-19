//! Platform-specific HTTP fetch implementation
//!
//! Desktop: Uses ureq or reqwest for native HTTP
//! WASM: Uses browser's fetch API via web-sys

use super::FetchError;

/// Result type for fetch operations
pub type FetchResult<T> = Result<T, FetchError>;

/// State of an async fetch operation
#[derive(Debug, Clone)]
pub enum FetchState<T> {
    /// Fetch not started
    Idle,
    /// Fetch in progress
    Loading,
    /// Fetch completed successfully
    Complete(T),
    /// Fetch failed
    Failed(FetchError),
}

impl<T> FetchState<T> {
    pub fn is_loading(&self) -> bool {
        matches!(self, FetchState::Loading)
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, FetchState::Complete(_))
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, FetchState::Failed(_))
    }
}

/// Handle for tracking an in-flight fetch request
pub struct FetchHandle<T> {
    state: FetchState<T>,
}

impl<T> FetchHandle<T> {
    pub fn new() -> Self {
        Self {
            state: FetchState::Idle,
        }
    }

    pub fn state(&self) -> &FetchState<T> {
        &self.state
    }

    pub fn set_loading(&mut self) {
        self.state = FetchState::Loading;
    }

    pub fn set_complete(&mut self, value: T) {
        self.state = FetchState::Complete(value);
    }

    pub fn set_failed(&mut self, error: FetchError) {
        self.state = FetchState::Failed(error);
    }

    pub fn take_result(&mut self) -> Option<FetchResult<T>> {
        match std::mem::replace(&mut self.state, FetchState::Idle) {
            FetchState::Complete(value) => Some(Ok(value)),
            FetchState::Failed(error) => Some(Err(error)),
            other => {
                self.state = other;
                None
            }
        }
    }
}

impl<T> Default for FetchHandle<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Desktop implementation (native HTTP)
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;

    /// Fetch text content from a URL synchronously (blocking)
    ///
    /// For desktop, we use a simple blocking approach since we're typically
    /// running in a background thread or during initialization.
    pub fn fetch_text_sync(url: &str) -> FetchResult<String> {
        // Use ureq for simple synchronous HTTP requests
        // For now, use a basic implementation with std::process or reqwest
        // We'll use a simple approach with the `ureq` crate or fall back to filesystem

        // Check if this is a file:// URL or relative path
        if url.starts_with("file://") || !url.contains("://") {
            // Local file - use filesystem
            let path = if url.starts_with("file://") {
                &url[7..]
            } else {
                url
            };
            std::fs::read_to_string(path)
                .map_err(|e| FetchError::Network(format!("Failed to read file: {}", e)))
        } else {
            // HTTP URL - for now, return an error until we add a proper HTTP client
            // In Phase 2, we'll add ureq or reqwest as a dependency
            Err(FetchError::Network(format!(
                "HTTP fetch not yet implemented for desktop. URL: {}",
                url
            )))
        }
    }

    /// Fetch text content from a URL asynchronously
    ///
    /// Spawns a background thread to perform the fetch.
    /// Use the returned handle to check status and get results.
    pub fn fetch_text_async(url: String, handle: std::sync::Arc<std::sync::Mutex<FetchHandle<String>>>) {
        // Mark as loading
        if let Ok(mut h) = handle.lock() {
            h.set_loading();
        }

        // Spawn background thread
        std::thread::spawn(move || {
            let result = fetch_text_sync(&url);
            if let Ok(mut h) = handle.lock() {
                match result {
                    Ok(text) => h.set_complete(text),
                    Err(e) => h.set_failed(e),
                }
            }
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

// ============================================================================
// WASM implementation (browser fetch API)
// ============================================================================

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Request, RequestInit, RequestMode, Response};

    /// Fetch text content from a URL asynchronously (WASM)
    ///
    /// Uses the browser's fetch API. The result is delivered via callback.
    pub async fn fetch_text(url: &str) -> FetchResult<String> {
        let mut opts = RequestInit::new();
        opts.method("GET");
        opts.mode(RequestMode::Cors);

        let request = Request::new_with_str_and_init(url, &opts)
            .map_err(|e| FetchError::Network(format!("Failed to create request: {:?}", e)))?;

        let window = web_sys::window()
            .ok_or_else(|| FetchError::Network("No window object".to_string()))?;

        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| FetchError::Network(format!("Fetch failed: {:?}", e)))?;

        let resp: Response = resp_value
            .dyn_into()
            .map_err(|_| FetchError::Network("Response is not a Response object".to_string()))?;

        let status = resp.status();
        if status == 401 {
            return Err(FetchError::Unauthorized);
        }
        if status == 403 {
            return Err(FetchError::Forbidden);
        }
        if status == 404 {
            return Err(FetchError::NotFound(url.to_string()));
        }
        if status < 200 || status >= 300 {
            return Err(FetchError::Http {
                status,
                message: resp.status_text(),
            });
        }

        let text = JsFuture::from(
            resp.text()
                .map_err(|e| FetchError::Network(format!("Failed to get text: {:?}", e)))?,
        )
        .await
        .map_err(|e| FetchError::Parse(format!("Failed to read text: {:?}", e)))?;

        text.as_string()
            .ok_or_else(|| FetchError::Parse("Response is not a string".to_string()))
    }

    /// Spawn an async fetch and update the handle when complete
    pub fn fetch_text_spawn(
        url: String,
        handle: std::rc::Rc<std::cell::RefCell<FetchHandle<String>>>,
    ) {
        // Mark as loading
        handle.borrow_mut().set_loading();

        // Spawn the async fetch
        wasm_bindgen_futures::spawn_local(async move {
            let result = fetch_text(&url).await;
            match result {
                Ok(text) => handle.borrow_mut().set_complete(text),
                Err(e) => handle.borrow_mut().set_failed(e),
            }
        });
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*;

// ============================================================================
// Unified API
// ============================================================================

/// Fetch text content from a URL
///
/// This is the main entry point for fetching resources.
/// - On desktop: Performs synchronous fetch (blocking)
/// - On WASM: Returns a future that must be awaited
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_text(url: &str) -> FetchResult<String> {
    native::fetch_text_sync(url)
}

#[cfg(target_arch = "wasm32")]
pub use wasm::fetch_text;

/// Fetch and parse JSON from a URL (desktop - synchronous)
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_json<T: serde::de::DeserializeOwned>(url: &str) -> FetchResult<T> {
    let text = fetch_text(url)?;
    serde_json::from_str(&text).map_err(FetchError::from)
}

/// Fetch and parse JSON from a URL (WASM - async)
#[cfg(target_arch = "wasm32")]
pub async fn fetch_json<T: serde::de::DeserializeOwned>(url: &str) -> FetchResult<T> {
    let text = fetch_text(url).await?;
    serde_json::from_str(&text).map_err(FetchError::from)
}

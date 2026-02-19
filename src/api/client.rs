//! HTTP client for the Fractal Flame API.
//!
//! Extends the existing fetch system with POST/PUT/DELETE methods and
//! Authorization headers. WASM uses browser Fetch API, desktop uses
//! blocking HTTP (to be implemented).

use crate::resources::{FetchError, FetchResult};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Build the full URL from base URL and path.
pub fn build_url(base_url: &str, path: &str) -> String {
    let base = base_url.trim_end_matches('/');
    format!("{}{}", base, path)
}

// ============================================================================
// WASM implementation
// ============================================================================

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Headers, Request, RequestInit, RequestMode, Response};

    /// Make an authenticated API request (WASM).
    ///
    /// Supports GET, POST, PUT, DELETE with optional JSON body and Bearer token.
    pub async fn api_request_raw(
        method: &str,
        url: &str,
        body: Option<String>,
        token: Option<&str>,
    ) -> FetchResult<String> {
        let mut opts = RequestInit::new();
        opts.method(method);
        opts.mode(RequestMode::Cors);

        // Set headers
        let headers = Headers::new()
            .map_err(|e| FetchError::Network(format!("Failed to create headers: {:?}", e)))?;

        if body.is_some() {
            headers
                .set("Content-Type", "application/json")
                .map_err(|e| FetchError::Network(format!("Failed to set content-type: {:?}", e)))?;
        }

        if let Some(token) = token {
            headers
                .set("Authorization", &format!("Bearer {}", token))
                .map_err(|e| FetchError::Network(format!("Failed to set auth header: {:?}", e)))?;
        }

        opts.headers(&headers);

        // Set body for POST/PUT
        if let Some(ref json_body) = body {
            opts.body(Some(&JsValue::from_str(json_body)));
        }

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

        // Handle auth errors
        if status == 401 {
            return Err(FetchError::Unauthorized);
        }
        if status == 403 {
            return Err(FetchError::Forbidden);
        }
        if status == 404 {
            return Err(FetchError::NotFound(url.to_string()));
        }

        // Handle 204 No Content (for DELETE)
        if status == 204 {
            return Ok(String::new());
        }

        if status < 200 || status >= 300 {
            // Try to read error body for better messages
            let error_body = JsFuture::from(
                resp.text()
                    .map_err(|_| FetchError::Http {
                        status,
                        message: resp.status_text(),
                    })?,
            )
            .await
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| resp.status_text());

            return Err(FetchError::Http {
                status,
                message: error_body,
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

    /// Make a typed API request that deserializes the JSON response.
    pub async fn api_request<T: DeserializeOwned>(
        method: &str,
        url: &str,
        body: Option<String>,
        token: Option<&str>,
    ) -> FetchResult<T> {
        let text = api_request_raw(method, url, body, token).await?;
        serde_json::from_str(&text).map_err(FetchError::from)
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::{api_request, api_request_raw};

// ============================================================================
// Desktop stub (to be implemented)
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;

    pub async fn api_request_raw(
        _method: &str,
        url: &str,
        _body: Option<String>,
        _token: Option<&str>,
    ) -> FetchResult<String> {
        Err(FetchError::Network(format!(
            "HTTP API requests not yet implemented for desktop. URL: {}",
            url
        )))
    }

    pub async fn api_request<T: DeserializeOwned>(
        _method: &str,
        url: &str,
        _body: Option<String>,
        _token: Option<&str>,
    ) -> FetchResult<T> {
        Err(FetchError::Network(format!(
            "HTTP API requests not yet implemented for desktop. URL: {}",
            url
        )))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::{api_request, api_request_raw};

// ============================================================================
// Convenience wrappers
// ============================================================================

/// GET request with auth token.
pub async fn api_get<T: DeserializeOwned>(url: &str, token: &str) -> FetchResult<T> {
    api_request("GET", url, None, Some(token)).await
}

/// POST request with JSON body and auth token.
pub async fn api_post<T: DeserializeOwned, B: Serialize>(
    url: &str,
    body: &B,
    token: &str,
) -> FetchResult<T> {
    let json = serde_json::to_string(body).map_err(|e| FetchError::Parse(e.to_string()))?;
    api_request("POST", url, Some(json), Some(token)).await
}

/// POST request without auth (for login/register).
pub async fn api_post_unauth<T: DeserializeOwned, B: Serialize>(
    url: &str,
    body: &B,
) -> FetchResult<T> {
    let json = serde_json::to_string(body).map_err(|e| FetchError::Parse(e.to_string()))?;
    api_request("POST", url, Some(json), None).await
}

/// PUT request with JSON body and auth token.
pub async fn api_put<T: DeserializeOwned, B: Serialize>(
    url: &str,
    body: &B,
    token: &str,
) -> FetchResult<T> {
    let json = serde_json::to_string(body).map_err(|e| FetchError::Parse(e.to_string()))?;
    api_request("PUT", url, Some(json), Some(token)).await
}

/// DELETE request with auth token. Returns Ok(()) on 204.
pub async fn api_delete(url: &str, token: &str) -> FetchResult<()> {
    api_request_raw("DELETE", url, None, Some(token)).await?;
    Ok(())
}

/// WASM API for JavaScript control
///
/// Provides wasm_bindgen exports for:
/// - Loading configs from JSON or URL parameters
/// - Programmatic render control
/// - PNG export to browser
/// - Query iteration progress
///
/// This enables:
/// - URL parameter config loading (e.g., ?config=base64json)
/// - Automated testing via Selenium/Puppeteer
/// - Browser-based export functionality
/// - Programmatic fractal generation

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::config::FractalConfig;

/// Global WASM API instance
///
/// This is accessed via `window.fractalFlameApi` in JavaScript
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct WasmApi {
    config: Option<FractalConfig>,
    current_iterations: u64,
    target_iterations: u64,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WasmApi {
    /// Create new WASM API instance
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmApi {
        WasmApi {
            config: None,
            current_iterations: 0,
            target_iterations: 0,
        }
    }

    /// Load config from JSON string
    ///
    /// JavaScript usage:
    /// ```js
    /// const api = new WasmApi();
    /// api.load_config_json('{"flame": {...}, "max_iterations": 1000000}');
    /// ```
    #[wasm_bindgen]
    pub fn load_config_json(&mut self, json: &str) -> Result<(), JsValue> {
        let config: FractalConfig = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse config: {}", e)))?;

        self.config = Some(config);
        self.current_iterations = 0;
        self.target_iterations = self.config.as_ref().unwrap().max_iterations;

        Ok(())
    }

    /// Load config from URL parameters
    ///
    /// Supports multiple formats:
    /// - `?config=<base64_json>` - Base64-encoded FractalConfig JSON
    /// - `?preset=<name>` - Load built-in preset
    ///
    /// JavaScript usage:
    /// ```js
    /// const api = new WasmApi();
    /// api.load_config_from_url(window.location.href);
    /// ```
    #[wasm_bindgen]
    pub fn load_config_from_url(&mut self, url_str: &str) -> Result<(), JsValue> {
        // Parse URL manually to extract query params
        let parts: Vec<&str> = url_str.split('?').collect();
        if parts.len() < 2 {
            return Err(JsValue::from_str("No query parameters in URL"));
        }

        let query = parts[1];
        let params: Vec<(&str, &str)> = query
            .split('&')
            .filter_map(|param| {
                let kv: Vec<&str> = param.split('=').collect();
                if kv.len() == 2 {
                    Some((kv[0], kv[1]))
                } else {
                    None
                }
            })
            .collect();

        // Try ?config=<base64_json>
        if let Some((_, config_b64)) = params.iter().find(|(k, _)| *k == "config") {
            // Decode base64
            let json_bytes = base64_decode(config_b64)
                .map_err(|e| JsValue::from_str(&format!("Invalid base64: {}", e)))?;

            let json = String::from_utf8(json_bytes)
                .map_err(|e| JsValue::from_str(&format!("Invalid UTF-8: {}", e)))?;

            return self.load_config_json(&json);
        }

        // Try ?preset=<name>
        if let Some((_, preset_name)) = params.iter().find(|(k, _)| *k == "preset") {
            return self.load_preset(preset_name);
        }

        Err(JsValue::from_str("No config parameter found in URL"))
    }

    /// Load built-in preset by name
    ///
    /// JavaScript usage:
    /// ```js
    /// api.load_preset("Bubble");
    /// ```
    #[wasm_bindgen]
    pub fn load_preset(&mut self, name: &str) -> Result<(), JsValue> {
        use crate::scene::presets::PresetLibrary;

        let library = PresetLibrary::new();
        let config = library.presets()
            .iter()
            .find(|p| p.flame.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| JsValue::from_str(&format!("Preset '{}' not found", name)))?
            .clone();

        self.config = Some(config);
        self.current_iterations = 0;
        self.target_iterations = self.config.as_ref().unwrap().max_iterations;

        Ok(())
    }

    /// Get current loaded config as JSON
    ///
    /// JavaScript usage:
    /// ```js
    /// const configJson = api.get_config_json();
    /// console.log(JSON.parse(configJson));
    /// ```
    #[wasm_bindgen]
    pub fn get_config_json(&self) -> Result<String, JsValue> {
        let config = self.config.as_ref()
            .ok_or_else(|| JsValue::from_str("No config loaded"))?;

        serde_json::to_string_pretty(config)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize config: {}", e)))
    }

    /// Get current iteration count
    #[wasm_bindgen]
    pub fn get_current_iterations(&self) -> f64 {
        self.current_iterations as f64
    }

    /// Get target iteration count
    #[wasm_bindgen]
    pub fn get_target_iterations(&self) -> f64 {
        self.target_iterations as f64
    }

    /// Get render progress (0.0 to 1.0)
    #[wasm_bindgen]
    pub fn get_progress(&self) -> f64 {
        if self.target_iterations == 0 {
            0.0
        } else {
            (self.current_iterations as f64) / (self.target_iterations as f64)
        }
    }

    /// Check if config is loaded
    #[wasm_bindgen]
    pub fn has_config(&self) -> bool {
        self.config.is_some()
    }

    /// Get list of available preset names
    ///
    /// Returns JSON array of preset names
    #[wasm_bindgen]
    pub fn get_preset_names(&self) -> String {
        use crate::scene::presets::PresetLibrary;

        let library = PresetLibrary::new();
        let names: Vec<&str> = library.presets()
            .iter()
            .map(|p| p.flame.name.as_str())
            .collect();

        serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string())
    }

    /// Set target iterations (useful for testing specific iteration counts)
    #[wasm_bindgen]
    pub fn set_target_iterations(&mut self, iterations: f64) {
        self.target_iterations = iterations as u64;
    }

    /// Export PNG from current config
    ///
    /// Returns PNG data as Uint8Array that can be downloaded or displayed
    ///
    /// JavaScript usage:
    /// ```js
    /// const api = new WasmApi();
    /// api.load_preset("Bubble");
    /// const pngBytes = await api.export_png(800, 600, 256);
    ///
    /// // Download
    /// const blob = new Blob([pngBytes], { type: 'image/png' });
    /// const url = URL.createObjectURL(blob);
    /// const a = document.createElement('a');
    /// a.href = url;
    /// a.download = 'fractal.png';
    /// a.click();
    /// ```
    #[wasm_bindgen]
    pub async fn export_png(
        &self,
        width: u32,
        height: u32,
        iterations_per_thread: u32,
    ) -> Result<Vec<u8>, JsValue> {
        use crate::app::export::export_headless_wasm;

        let config = self.config.as_ref()
            .ok_or_else(|| JsValue::from_str("No config loaded"))?;

        let png_data = export_headless_wasm(config, width, height, iterations_per_thread)
            .await
            .map_err(|e| JsValue::from_str(&e))?;

        Ok(png_data)
    }
}

/// Helper function to decode base64
#[cfg(target_arch = "wasm32")]
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    // Simple base64 decode without external deps
    // For URL parameters - decode using percent decoding first if needed
    let cleaned = input.replace("%3D", "=").replace("%2B", "+").replace("%2F", "/");

    // Decode base64 manually (simple implementation for URL safe base64)
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = Vec::new();
    let bytes = cleaned.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'=' {
            break;
        }

        let b1 = CHARSET.iter().position(|&c| c == bytes[i]).ok_or("Invalid base64")?;
        i += 1;
        if i >= bytes.len() {
            break;
        }

        let b2 = CHARSET.iter().position(|&c| c == bytes[i]).ok_or("Invalid base64")?;
        i += 1;

        result.push((b1 << 2 | b2 >> 4) as u8);

        if i >= bytes.len() || bytes[i] == b'=' {
            break;
        }
        let b3 = CHARSET.iter().position(|&c| c == bytes[i]).ok_or("Invalid base64")?;
        i += 1;

        result.push(((b2 & 0xF) << 4 | b3 >> 2) as u8);

        if i >= bytes.len() || bytes[i] == b'=' {
            break;
        }
        let b4 = CHARSET.iter().position(|&c| c == bytes[i]).ok_or("Invalid base64")?;
        i += 1;

        result.push(((b3 & 0x3) << 6 | b4) as u8);
    }

    Ok(result)
}

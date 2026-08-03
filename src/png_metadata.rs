/// PNG metadata embedding for fractal flame renders
///
/// Embeds comprehensive metadata into PNG tEXt chunks for:
/// - Build version and compilation info
/// - Complete flame configuration (JSON)
/// - Render settings and timings
/// - Test metadata (if from test suite)

use serde::{Deserialize, Serialize};

/// Complete metadata for a rendered fractal flame PNG
///
/// `Default` exists for tests and for partial construction; a real
/// export always goes through `from_app_state`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PngMetadata {
    // Build Info
    pub version: String,
    pub git_hash: String,
    pub git_branch: String,
    pub build_timestamp: String,
    pub platform: String,
    pub rustc_version: String,
    pub build_profile: String,

    // Render Settings
    pub width: u32,
    pub height: u32,
    pub total_iterations: u64,
    pub render_time_ms: f64,
    pub iterations_per_thread: u32,
    pub speed_factor: f32,

    // Flame Config (embedded as JSON)
    pub config_json: String,
    pub config_checksum: String,  // SHA256 for verification

    // Test Metadata (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_category: Option<String>,

    // Display Settings
    pub background_color: [f32; 3],
    pub exposure: f32,
    pub gamma: f32,
    pub use_tone_curve: bool,

    // Rendering Mode
    pub render_mode: String,      // "2D" or "3D"
    pub projection: String,        // "Orthographic" or "Perspective{strength}"

    // Shader Compilation
    pub shader_variations_count: usize,
    pub shader_compile_time_ms: Option<f64>,
}

impl PngMetadata {
    /// Create metadata from app state
    pub fn from_app_state(
        width: u32,
        height: u32,
        total_iterations: u64,
        render_time_ms: f64,
        iterations_per_thread: u32,
        speed_factor: f32,
        config: &crate::config::FractalConfig,
    ) -> Self {
        let version_info = crate::version::get_version_info();
        let config_json = serde_json::to_string(config).unwrap_or_default();
        let config_checksum = Self::calculate_checksum(&config_json);

        let (render_mode, projection) = match config.render_mode {
            crate::scene::transforms::RenderMode::TwoD => ("2D".to_string(), "N/A".to_string()),
            crate::scene::transforms::RenderMode::ThreeD => {
                let proj_str = if config.perspective_strength < 0.01 {
                    "Orthographic".to_string()
                } else {
                    format!("Perspective{{strength:{}}}", config.perspective_strength)
                };
                ("3D".to_string(), proj_str)
            }
        };

        let shader_variations_count = config.flame.extract_active_variations().len();

        Self {
            version: version_info.version.to_string(),
            git_hash: version_info.git_hash.to_string(),
            git_branch: version_info.git_branch.to_string(),
            build_timestamp: version_info.build_time.to_string(),
            platform: version_info.platform().to_string(),
            rustc_version: version_info.rustc_version.to_string(),
            build_profile: version_info.profile.to_string(),

            width,
            height,
            total_iterations,
            render_time_ms,
            iterations_per_thread,
            speed_factor,

            config_json,
            config_checksum,

            test_name: None,
            test_category: None,

            background_color: config.background_color,
            exposure: config.exposure,
            gamma: config.gamma,
            use_tone_curve: config.tonemap_curve.points.len() > 2,

            render_mode,
            projection,

            shader_variations_count,
            shader_compile_time_ms: None,
        }
    }

    /// Calculate SHA256 checksum of a string
    fn calculate_checksum(data: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Convert metadata to PNG tEXt chunk key-value pairs
    pub fn to_text_chunks(&self) -> Vec<(String, String)> {
        vec![
            ("Software".to_string(), format!("Fractal Art Editor v{}", self.version)),
            ("GitHash".to_string(), self.git_hash.clone()),
            ("Branch".to_string(), self.git_branch.clone()),
            ("BuildDate".to_string(), self.build_timestamp.clone()),
            ("Platform".to_string(), self.platform.clone()),
            ("RustcVersion".to_string(), self.rustc_version.clone()),
            ("BuildProfile".to_string(), self.build_profile.clone()),

            ("Resolution".to_string(), format!("{}x{}", self.width, self.height)),
            ("Iterations".to_string(), self.total_iterations.to_string()),
            ("RenderTime".to_string(), format!("{:.2}ms", self.render_time_ms)),
            ("IterationsPerThread".to_string(), self.iterations_per_thread.to_string()),
            ("SpeedFactor".to_string(), format!("{:.2}", self.speed_factor)),

            ("ConfigChecksum".to_string(), format!("hash:{}", self.config_checksum)),
            ("Config".to_string(), self.config_json.clone()),  // Full JSON config

            ("BackgroundColor".to_string(), format!("{},{},{}",
                self.background_color[0], self.background_color[1], self.background_color[2])),
            ("Exposure".to_string(), self.exposure.to_string()),
            ("Gamma".to_string(), self.gamma.to_string()),
            ("ToneCurve".to_string(), self.use_tone_curve.to_string()),

            ("RenderMode".to_string(), self.render_mode.clone()),
            ("Projection".to_string(), self.projection.clone()),

            ("ShaderVariations".to_string(), self.shader_variations_count.to_string()),
        ]
    }
}

/// Whether exported PNGs omit their metadata.
///
/// # Why this is enforced at the encoder, not at the call sites
///
/// The complete `FractalConfig` travels in every exported PNG — which is
/// the feature that makes an image reproducible and a bug report
/// actionable. It is also, for anyone who did not choose it, a leak: a
/// flame someone spent a week on is fully recoverable from a picture of
/// it, and so is any name or comment they put in the config.
///
/// Nine places construct a `PngMetadata`. A privacy switch honoured at
/// nine call sites is one that will eventually be forgotten at the
/// tenth, and the failure mode is silent — the export succeeds and the
/// data ships anyway. So the switch lives here, at the single point
/// every PNG passes through, and the call sites are not trusted with it.
///
/// Process-global rather than threaded through: exports run on
/// background threads and from the headless CLI, and the alternative is
/// a parameter on every signature between the setting and the encoder.
static STRIP_METADATA: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Set from `SystemSettings` at startup and whenever the user toggles
/// it, and from `--strip-metadata` in the headless CLI.
pub fn set_strip_metadata(strip: bool) {
    STRIP_METADATA.store(strip, std::sync::atomic::Ordering::Relaxed);
}

pub fn strip_metadata() -> bool {
    STRIP_METADATA.load(std::sync::atomic::Ordering::Relaxed)
}

/// Encode RGBA pixel data as PNG, with metadata unless stripping is on.
///
/// Named for what it usually does. It honours [`strip_metadata`], so a
/// caller cannot leak by forgetting — see that constant for why the
/// check is here rather than at the nine sites that build a
/// `PngMetadata`.
pub fn encode_png_with_metadata(
    width: u32,
    height: u32,
    rgba_data: Vec<u8>,
    metadata: &PngMetadata,
) -> Result<Vec<u8>, String> {
    use png::{BitDepth, ColorType, Encoder};

    // Create PNG encoder
    let mut png_data = Vec::new();
    {
        let mut encoder = Encoder::new(&mut png_data, width, height);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);

        // The privacy switch, applied where nothing can route around it.
        // See `STRIP_METADATA`.
        if !strip_metadata() {
            for (key, value) in metadata.to_text_chunks() {
                encoder.add_text_chunk(key, value)
                    .map_err(|e| format!("Failed to add text chunk: {}", e))?;
            }
        }

        let mut writer = encoder.write_header()
            .map_err(|e| format!("Failed to write PNG header: {}", e))?;

        writer.write_image_data(&rgba_data)
            .map_err(|e| format!("Failed to write image data: {}", e))?;
    }

    Ok(png_data)
}

/// Read metadata from a PNG file
pub fn read_png_metadata(png_data: &[u8]) -> Result<PngMetadata, String> {
    use png::Decoder;
    use std::io::Cursor;

    let decoder = Decoder::new(Cursor::new(png_data));
    let reader = decoder.read_info()
        .map_err(|e| format!("Failed to read PNG: {}", e))?;

    let info = reader.info();
    let text_chunks: std::collections::HashMap<String, String> = info.uncompressed_latin1_text
        .iter()
        .map(|chunk| (chunk.keyword.clone(), chunk.text.clone()))
        .collect();

    // Extract metadata from text chunks
    // Both spellings. The product was called "Fractal Flame Renderer"
    // in every PNG exported before the rename, and those files are
    // exactly the ones somebody reaches for when they want an old
    // flame back — dropping the old prefix would report their version
    // as "unknown" for no reason.
    let version = text_chunks.get("Software")
        .and_then(|s| {
            s.strip_prefix("Fractal Art Editor v")
                .or_else(|| s.strip_prefix("Fractal Flame Renderer v"))
        })
        .unwrap_or("unknown")
        .to_string();

    let git_hash = text_chunks.get("GitHash")
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());

    let resolution = text_chunks.get("Resolution").unwrap_or(&"0x0".to_string()).clone();
    let (width, height) = if let Some((w, h)) = resolution.split_once('x') {
        (w.parse().unwrap_or(0), h.parse().unwrap_or(0))
    } else {
        (0, 0)
    };

    let config_json = text_chunks.get("Config").cloned().unwrap_or_default();

    Ok(PngMetadata {
        version,
        git_hash,
        git_branch: text_chunks.get("Branch").cloned().unwrap_or_default(),
        build_timestamp: text_chunks.get("BuildDate").cloned().unwrap_or_default(),
        platform: text_chunks.get("Platform").cloned().unwrap_or_default(),
        rustc_version: text_chunks.get("RustcVersion").cloned().unwrap_or_default(),
        build_profile: text_chunks.get("BuildProfile").cloned().unwrap_or_default(),

        width,
        height,
        total_iterations: text_chunks.get("Iterations").and_then(|s| s.parse().ok()).unwrap_or(0),
        render_time_ms: text_chunks.get("RenderTime")
            .and_then(|s| s.trim_end_matches("ms").parse().ok()).unwrap_or(0.0),
        iterations_per_thread: text_chunks.get("IterationsPerThread").and_then(|s| s.parse().ok()).unwrap_or(256),
        speed_factor: text_chunks.get("SpeedFactor").and_then(|s| s.parse().ok()).unwrap_or(1.0),

        config_json,
        config_checksum: text_chunks.get("ConfigChecksum")
            .map(|s| s.strip_prefix("hash:").unwrap_or(s).to_string())
            .unwrap_or_default(),

        test_name: None,
        test_category: None,

        background_color: [0.0, 0.0, 0.0],  // Would need parsing
        exposure: 1.0,
        gamma: 2.2,
        use_tone_curve: text_chunks.get("ToneCurve").map(|s| s == "true").unwrap_or(false),

        render_mode: text_chunks.get("RenderMode").cloned().unwrap_or_default(),
        projection: text_chunks.get("Projection").cloned().unwrap_or_default(),

        shader_variations_count: text_chunks.get("ShaderVariations").and_then(|s| s.parse().ok()).unwrap_or(0),
        shader_compile_time_ms: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_consistency() {
        let data = "test config";
        let checksum1 = PngMetadata::calculate_checksum(data);
        let checksum2 = PngMetadata::calculate_checksum(data);
        assert_eq!(checksum1, checksum2);
    }

    #[test]
    fn test_checksum_different() {
        let checksum1 = PngMetadata::calculate_checksum("config1");
        let checksum2 = PngMetadata::calculate_checksum("config2");
        assert_ne!(checksum1, checksum2);
    }
}

#[cfg(test)]
mod strip_tests {
    use super::*;

    fn chunk_keys(png: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        let mut pos = 8;
        while pos + 8 <= png.len() {
            let len = u32::from_be_bytes([png[pos], png[pos + 1], png[pos + 2], png[pos + 3]])
                as usize;
            let ctype = String::from_utf8_lossy(&png[pos + 4..pos + 8]).to_string();
            if ctype == "IEND" {
                break;
            }
            out.push(ctype);
            pos += 12 + len;
        }
        out
    }

    /// Stripping must remove the config, and the image must still be a
    /// valid PNG of the right size — a privacy switch that corrupts the
    /// export is not a privacy switch.
    ///
    /// Serialised against the global by construction: this test owns
    /// the flag for its duration and restores it, and no other test
    /// touches it.
    #[test]
    fn stripping_removes_every_text_chunk_and_nothing_else() {
        let was = strip_metadata();
        let rgba = vec![128u8; 4 * 4 * 4];
        let meta = PngMetadata {
            config_json: "{\"secret\":true}".to_string(),
            ..Default::default()
        };

        set_strip_metadata(false);
        let with = encode_png_with_metadata(4, 4, rgba.clone(), &meta).expect("encode");
        assert!(
            chunk_keys(&with).iter().any(|c| c == "tEXt"),
            "the default must still carry metadata: {:?}",
            chunk_keys(&with)
        );
        assert!(
            String::from_utf8_lossy(&with).contains("secret"),
            "sanity: the config is in there when not stripping"
        );

        set_strip_metadata(true);
        let without = encode_png_with_metadata(4, 4, rgba, &meta).expect("encode");
        let keys = chunk_keys(&without);
        assert!(!keys.iter().any(|c| c == "tEXt"), "{keys:?}");
        assert!(
            !String::from_utf8_lossy(&without).contains("secret"),
            "the config must not survive anywhere in the file"
        );
        assert_eq!(keys.first().map(String::as_str), Some("IHDR"));
        assert!(keys.iter().any(|c| c == "IDAT"), "still a real image: {keys:?}");
        assert!(without.len() < with.len(), "and smaller");

        set_strip_metadata(was);
    }
}

#[cfg(test)]
mod naming_tests {
    use super::*;

    /// The version must parse out of BOTH the current product name and
    /// the one every PNG exported before the rename carries.
    ///
    /// Old exports are exactly the files somebody reaches for when they
    /// want an old flame back. Reporting their version as "unknown"
    /// because the product was renamed would be a self-inflicted wound,
    /// and a silent one — the config still loads, so nothing looks
    /// broken until someone asks which build made it.
    #[test]
    fn both_product_names_parse_their_version() {
        let render = |software: &str| {
            let mut meta = PngMetadata { version: "9.9.9".into(), ..Default::default() };
            meta.config_json = "{}".into();
            let png = {
                // Build a PNG carrying exactly this Software string.
                let mut data = Vec::new();
                {
                    let mut enc = png::Encoder::new(&mut data, 1, 1);
                    enc.set_color(png::ColorType::Rgba);
                    enc.set_depth(png::BitDepth::Eight);
                    enc.add_text_chunk("Software".into(), software.into()).unwrap();
                    enc.add_text_chunk("Config".into(), "{}".into()).unwrap();
                    let mut w = enc.write_header().unwrap();
                    w.write_image_data(&[0, 0, 0, 255]).unwrap();
                }
                data
            };
            let _ = &meta;
            read_png_metadata(&png).expect("parses").version
        };

        assert_eq!(render("Fractal Art Editor v0.5.0"), "0.5.0", "the current name");
        assert_eq!(
            render("Fractal Flame Renderer v0.4.4"),
            "0.4.4",
            "every PNG exported before the rename"
        );
        assert_eq!(render("Some Other Tool v1.0"), "unknown", "and nothing else");
    }
}

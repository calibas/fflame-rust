//! The config's rules: what a value may be and what changing it implies,
//! for every [`ConfigPath`] -- the program's one setter. [`ConfigManager`](super::ConfigManager)
//! wraps it with history, preview and [`UpdateType`]; the video export, the
//! cameras and every edit made on a config outside the manager (a mode's
//! look, a fly-mode step) call it directly, so a value means the same
//! wherever it is written ([architecture-review.md](../../docs/projects/architecture-review.md)
//! section 2.1).
//!
//! [`UpdateType`]: super::UpdateType

use super::delta::{AffineParam, ConfigPath, ConfigValue};
use super::fractal_config::FractalConfig;
use super::manager::{custom_source_from_json, ConfigError, EditingTarget};
use crate::scene::transforms::{Flame, Transform};

/// Apply one value to `config` at `path`, against the flame `target`
/// names (the main flame or a subflame): the value's clamps and side
/// effects, as an edit in the app applies them -- no history, no preview.
pub fn apply(config: &mut FractalConfig, target: EditingTarget, path: &ConfigPath, value: ConfigValue) -> Result<(), ConfigError> {
        match path {
            // View
            ConfigPath::Zoom => {
                config.zoom = value.try_into()?;
            }
            ConfigPath::Pan => {
                let (x, y): (f64, f64) = value.try_into()?;
                config.pan_x = x;
                config.pan_y = y;
            }
            ConfigPath::PanX => {
                config.pan_x = value.try_into()?;
            }
            ConfigPath::PanY => {
                config.pan_y = value.try_into()?;
            }
            ConfigPath::Rotation => {
                config.rotation = value.try_into()?;
            }
            ConfigPath::CameraRotationX => {
                config.camera_rotation_x = value.try_into()?;
            }
            ConfigPath::CameraRotationY => {
                config.camera_rotation_y = value.try_into()?;
            }
            ConfigPath::CameraBank => {
                config.camera_bank = value.try_into()?;
            }
            ConfigPath::CameraX => {
                config.camera_x = value.try_into()?;
            }
            ConfigPath::CameraY => {
                config.camera_y = value.try_into()?;
            }
            ConfigPath::CameraZ => {
                config.camera_z = value.try_into()?;
            }
            ConfigPath::DofFocusDistance => {
                config.dof_focus_distance = value.try_into()?;
            }
            ConfigPath::DofBlurStrength => {
                config.dof_blur_strength = value.try_into()?;
            }
            ConfigPath::FogStrength => {
                config.fog_strength = value.try_into()?;
            }
            ConfigPath::FogStart => {
                config.fog_start = value.try_into()?;
            }
            ConfigPath::FilterRadius => {
                config.filter_radius = value.try_into()?;
            }
            ConfigPath::FilterBlurEdges => {
                config.filter_blur_edges = value.try_into()?;
            }

            // Tone mapping
            ConfigPath::Exposure => {
                config.exposure = value.try_into()?;
            }
            ConfigPath::Gamma => {
                config.gamma = value.try_into()?;
            }
            ConfigPath::GammaThreshold => {
                config.gamma_threshold = value.try_into()?;
            }
            ConfigPath::Brightness => {
                config.brightness = value.try_into()?;
            }
            ConfigPath::Vibrancy => {
                config.vibrancy = value.try_into()?;
            }
            ConfigPath::WhiteLevel => {
                config.white_level = value.try_into()?;
            }
            ConfigPath::Saturation => {
                config.saturation = value.try_into()?;
            }
            ConfigPath::HueShift => {
                config.hue_shift = value.try_into()?;
            }
            ConfigPath::AlphaBlendLow => {
                config.alpha_blend_low = value.try_into()?;
            }
            ConfigPath::AlphaBlendHigh => {
                config.alpha_blend_high = value.try_into()?;
            }
            ConfigPath::DensityScale => {
                config.density_scale = value.try_into()?;
            }
            ConfigPath::TonemapMode => {
                config.tonemap_mode = value.try_into()?;
            }
            ConfigPath::HighlightMode => {
                config.highlight_mode = value.try_into()?;
            }
            ConfigPath::TonemapCurve => {
                config.tonemap_curve = value.try_into()?;
            }
            ConfigPath::UseCurve => {
                config.use_curve = value.try_into()?;
            }
            // Levels controls
            ConfigPath::LevelsEnabled => {
                config.levels_enabled = value.try_into()?;
            }
            ConfigPath::AutoExposure => {
                config.auto_exposure = value.try_into()?;
            }
            ConfigPath::CylinderTargeting => {
                config.cylinder_targeting = value.try_into()?;
            }
            ConfigPath::CylinderAlways => {
                config.cylinder_always = value.try_into()?;
            }
            ConfigPath::CylinderTrim => {
                config.cylinder_trim = value.try_into()?;
            }
            ConfigPath::CylinderTrimLevels => {
                config.cylinder_trim_levels = value.try_into()?;
            }
            ConfigPath::WordRemovals => {
                config.word_removals = value.try_into()?;
            }
            ConfigPath::LevelsLow => {
                config.levels_low = value.try_into()?;
            }
            ConfigPath::LevelsHigh => {
                config.levels_high = value.try_into()?;
            }
            ConfigPath::LevelsGamma => {
                config.levels_gamma = value.try_into()?;
            }

            // Color
            ConfigPath::ColorMode => {
                config.color_mode = value.try_into()?;
            }
            ConfigPath::PathMapStyle => {
                config.path_map_style = value.try_into()?;
            }
            ConfigPath::PathMapLevel => {
                config.path_map_level = value.try_into()?;
            }
            ConfigPath::PaletteIndex => {
                // PaletteIndex is deprecated - ignore updates
                log::warn!("Attempted to set deprecated PaletteIndex - use Palette instead");
            }
            ConfigPath::Palette => {
                if let ConfigValue::Palette(mut palette) = value {
                    // Safety: Never allow built-in flag to be true in config.palette
                    // Built-ins should only exist in the library
                    if palette.built_in {
                        log::warn!("Attempted to set built-in palette in config.palette - forcing built_in=false");
                        palette.built_in = false;
                    }

                    // Update palette data
                    config.palette = palette;
                }
            }
            ConfigPath::PaletteRotation => {
                config.palette_rotation = value.try_into()?;
            }
            ConfigPath::PaletteSize => {
                let v: f32 = value.try_into()?;
                config.palette_size = (v as u32).clamp(256, 4096);
            }
            ConfigPath::PaletteSqueeze => {
                let v: f32 = value.try_into()?;
                config.palette_squeeze = v.clamp(0.1, 16.0);
            }
            ConfigPath::PaletteSqueezeMode => {
                config.palette_squeeze_mode = value.try_into()?;
            }
            ConfigPath::PaletteSqueezeFalloff => {
                let v: f32 = value.try_into()?;
                config.palette_squeeze_falloff = v.clamp(0.05, 0.99);
            }
            ConfigPath::PaletteLogStrength => {
                let v: f32 = value.try_into()?;
                config.palette_log_strength = v.clamp(-10.0, 10.0);
            }
            ConfigPath::PaletteReverse => {
                config.palette_reverse = value.try_into()?;
            }
            ConfigPath::SpeedFactor => {
                config.speed_factor = value.try_into()?;
            }
            ConfigPath::BackgroundColor => {
                let c: [f32; 3] = value.try_into()?;
                config.background_color = [
                    c[0].clamp(0.0, 1.0),
                    c[1].clamp(0.0, 1.0),
                    c[2].clamp(0.0, 1.0),
                ];
            }
            ConfigPath::BackgroundColorR => {
                let v: f32 = value.try_into()?;
                config.background_color[0] = v.clamp(0.0, 1.0);
            }
            ConfigPath::BackgroundColorG => {
                let v: f32 = value.try_into()?;
                config.background_color[1] = v.clamp(0.0, 1.0);
            }
            ConfigPath::BackgroundColorB => {
                let v: f32 = value.try_into()?;
                config.background_color[2] = v.clamp(0.0, 1.0);
            }

            // Rendering settings
            ConfigPath::BlendFactor => {
                config.blend_factor = value.try_into()?;
            }
            ConfigPath::UseDynamicBlend => {
                config.use_dynamic_blend = value.try_into()?;
            }
            ConfigPath::MaxIterations => {
                config.max_iterations = value.try_into()?;
            }
            ConfigPath::DeterministicRng => {
                config.deterministic_rng = value.try_into()?;
            }

            // Escape-time
            ConfigPath::EscapeFormula => {
                config.escape.formula = value.try_into()?;
            }
            ConfigPath::EscapeCustomFormula => {
                let s: String = value.try_into()?;
                config.escape.custom_formula = custom_source_from_json(&s)?;
            }
            ConfigPath::EscapeCustomColoring => {
                let s: String = value.try_into()?;
                config.escape.custom_coloring = custom_source_from_json(&s)?;
            }
            ConfigPath::FlameCustomVariations => {
                let s: String = value.try_into()?;
                config.flame.custom_variations =
                    if s.is_empty() { Vec::new() } else { serde_json::from_str(&s).map_err(|_| ConfigError::TypeMismatch)? };
            }
            ConfigPath::CustomEffects => {
                let s: String = value.try_into()?;
                config.custom_effects =
                    if s.is_empty() { Vec::new() } else { serde_json::from_str(&s).map_err(|_| ConfigError::TypeMismatch)? };
            }
            ConfigPath::EscapeJulia => {
                config.escape.julia = value.try_into()?;
            }
            ConfigPath::EscapeJuliaRe => {
                config.escape.julia_re = value.try_into()?;
            }
            ConfigPath::EscapeJuliaIm => {
                config.escape.julia_im = value.try_into()?;
            }
            ConfigPath::EscapeCenterRe => {
                config.escape.center_re = value.try_into()?;
            }
            ConfigPath::EscapeCenterIm => {
                config.escape.center_im = value.try_into()?;
            }
            ConfigPath::EscapeCamTargetX => {
                config.escape.cam_target_x = value.try_into()?;
            }
            ConfigPath::EscapeCamTargetY => {
                config.escape.cam_target_y = value.try_into()?;
            }
            ConfigPath::EscapeCamTargetZ => {
                config.escape.cam_target_z = value.try_into()?;
            }
            ConfigPath::EscapeCamPitch => {
                // A terrain's to the poles and no further: past them the
                // elevation would mean the same view with the yaw
                // reversed, and a slider that wraps a view is a slider
                // that jumps. A solid's has no poles: an orbit carries on
                // over the top, the camera upside down, and the angle
                // wraps at a half turn either way.
                let v: f32 = value.try_into()?;
                config.escape.cam_pitch = if config.escape.terrain_active() {
                    v.clamp(-1.5708, 1.5708)
                } else {
                    v - std::f32::consts::TAU * (v / std::f32::consts::TAU).round()
                };
            }
            ConfigPath::EscapeCamYaw => {
                config.escape.cam_yaw = value.try_into()?;
            }
            ConfigPath::EscapeCamBank => {
                config.escape.cam_bank = value.try_into()?;
            }
            ConfigPath::EscapeCamFov => {
                let v: f32 = value.try_into()?;
                config.escape.cam_fov = v.clamp(0.05, 3.0);
            }
            ConfigPath::EscapeZoomLog2 | ConfigPath::EscapeZoomLog10 => {
                // An f64, as the config holds it (a track's f32 widens):
                // a script's or Python's zoom keeps its digits.
                let v: f64 = value.try_into()?;
                // Same travel range as the input paths; NaN from a
                // wild animation signal must not poison the view.
                let v = if matches!(path, ConfigPath::EscapeZoomLog10) {
                    crate::config::escape::EscapeConfig::log10_to_log2(v)
                } else {
                    v
                };
                config.escape.zoom_log2 =
                    if v.is_finite() { v.clamp(-8.0, 100_000_000.0) } else { 0.0 };
            }
            ConfigPath::EscapeRotation => {
                config.escape.rotation = value.try_into()?;
            }
            ConfigPath::EscapeMaxIter => {
                config.escape.max_iter = value.try_into()?;
            }

            // Simulation. Every bound here is a real one: an unclamped
            // grid dimension allocates gigabytes, an unclamped dt makes
            // the explicit solver diverge (phase 0 measured the caps per
            // model), and a steps_per_frame of zero would make Run do
            // nothing while looking like it worked.
            ConfigPath::SimModel => {
                let name = String::try_from(value)?;
                // A new version of the same Rhai model (P4: its id in both
                // names) keeps what was set for it: each edit Used is a new
                // version, and Live would otherwise reset the parameters,
                // dt and steps at every keystroke.
                let same_script = crate::config::sim::same_script(&name, &config.sim.model);
                if name != config.sim.model && !same_script {
                    // Parameters belong to the model that declared them,
                    // and dt/steps are per-model working values --
                    // Gray-Scott runs at dt 1.0 where Schnakenberg
                    // diverges above 0.02, so carrying one model's
                    // settings into another is unusable either way.
                    config.sim.model_params.clear();
                    let m = crate::sim::model_or_default(&name);
                    config.sim.dt = m.default_dt;
                    config.sim.steps = m.default_steps;
                }
                config.sim.model = name;
            }
            ConfigPath::SimColoring => {
                config.sim.coloring = String::try_from(value)?;
            }
            ConfigPath::SimCustomColoring => {
                let s: String = value.try_into()?;
                config.sim.custom_coloring = custom_source_from_json(&s)?;
            }
            ConfigPath::SimCustomModel => {
                let s: String = value.try_into()?;
                config.sim.custom_model = custom_source_from_json(&s)?;
            }
            ConfigPath::SimGridMode => {
                let mode = String::try_from(value)?;
                let (w, h) = match config.sim.grid {
                    crate::config::sim::SimGrid::Fixed { width, height } => (width, height),
                    crate::config::sim::SimGrid::Viewport { .. } => (512, 512),
                };
                config.sim.grid = if mode == "fixed" {
                    crate::config::sim::SimGrid::Fixed { width: w, height: h }
                } else {
                    crate::config::sim::SimGrid::Viewport { scale: 1.0 }
                };
            }
            ConfigPath::SimGridWidth => {
                let w: u32 = u32::try_from(value)?;
                let h = match config.sim.grid {
                    crate::config::sim::SimGrid::Fixed { height, .. } => height,
                    crate::config::sim::SimGrid::Viewport { .. } => 512,
                };
                config.sim.grid = crate::config::sim::SimGrid::Fixed {
                    width: w.clamp(16, 8192),
                    height: h,
                };
            }
            ConfigPath::SimGridHeight => {
                let h: u32 = u32::try_from(value)?;
                let w = match config.sim.grid {
                    crate::config::sim::SimGrid::Fixed { width, .. } => width,
                    crate::config::sim::SimGrid::Viewport { .. } => 512,
                };
                config.sim.grid = crate::config::sim::SimGrid::Fixed {
                    width: w,
                    height: h.clamp(16, 8192),
                };
            }
            ConfigPath::SimGridScale => {
                let sc: f32 = f32::try_from(value)?;
                let sc = if sc.is_finite() { sc.clamp(0.125, 4.0) } else { 1.0 };
                config.sim.grid = crate::config::sim::SimGrid::Viewport { scale: sc };
            }
            ConfigPath::SimSeed => {
                config.sim.seed = match value {
                    ConfigValue::UInt64(v) => v,
                    other => u32::try_from(other)? as u64,
                };
            }
            ConfigPath::SimInitKind => {
                let kind = String::try_from(value)?;
                config.sim.init = config.sim.init.with_kind(&kind);
            }
            ConfigPath::SimInitAmplitude => {
                let a: f32 = f32::try_from(value)?;
                config.sim.init = crate::config::sim::SimInit::Noise {
                    amplitude: if a.is_finite() { a.clamp(0.0, 1.0) } else { 0.05 },
                };
            }
            ConfigPath::SimInitRadius => {
                let r = u32::try_from(value)?.clamp(1, 4096);
                config.sim.init = match config.sim.init {
                    crate::config::sim::SimInit::Blob { .. } => {
                        crate::config::sim::SimInit::Blob { radius: r }
                    }
                    crate::config::sim::SimInit::Ring { .. } => {
                        crate::config::sim::SimInit::Ring { radius: r }
                    }
                    crate::config::sim::SimInit::Blobs { count, .. } => {
                        crate::config::sim::SimInit::Blobs { count, radius: r }
                    }
                    other => other,
                };
            }
            ConfigPath::SimInitCount => {
                let c = u32::try_from(value)?.clamp(1, 64);
                if let crate::config::sim::SimInit::Blobs { radius, .. } = config.sim.init {
                    config.sim.init =
                        crate::config::sim::SimInit::Blobs { count: c, radius };
                }
            }
            ConfigPath::SimSteps => {
                config.sim.steps = u32::try_from(value)?.min(10_000_000);
            }
            ConfigPath::SimStepsPerFrame => {
                config.sim.steps_per_frame = u32::try_from(value)?.clamp(1, 4096);
            }
            ConfigPath::SimTerrainEnabled => {
                let v: bool = value.try_into()?;
                config.sim.terrain.enabled = v;
            }
            ConfigPath::SimTerrainLayer => {
                let v: u32 = value.try_into()?;
                config.sim.terrain.layer = v.min(7);
            }
            ConfigPath::SimTerrainChannel => {
                let v: u32 = value.try_into()?;
                config.sim.terrain.channel = v.min(3);
            }
            ConfigPath::SimTerrainSoftness => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.softness = v.clamp(0.0, 8.0);
            }
            ConfigPath::SimTerrainHeight => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.height = v.clamp(-1.0, 1.0);
            }
            ConfigPath::SimTerrainTiling => {
                let v: String = value.try_into()?;
                config.sim.terrain.tiling = crate::config::sim::SimTiling::from_str_or_default(&v);
            }
            ConfigPath::SimTerrainTier => {
                let v: String = value.try_into()?;
                config.sim.terrain.tier = crate::config::escape::RenderTier::from_str_or_default(&v);
            }
            ConfigPath::SimTerrainShadow => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.shadow = v.clamp(0.0, 1.0);
            }
            ConfigPath::SimTerrainShadowSharpness => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.shadow_sharpness = v.clamp(1.0, 128.0);
            }
            ConfigPath::SimTerrainOcclusion => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.occlusion = v.clamp(0.0, 0.2);
            }
            ConfigPath::SimTerrainFar => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.far = v.clamp(0.5, 64.0);
            }
            ConfigPath::SimTerrainHaze => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.haze = v.clamp(0.0, 1.0);
            }
            ConfigPath::SimTerrainCamPitch => {
                let v = finite_or_zero(value.try_into()?);
                // To the poles, as the escape cameras: `solid_frame`
                // holds there, and a fly mode's look passes through them.
                config.sim.terrain.cam_pitch = v.clamp(-1.5708, 1.5708);
            }
            ConfigPath::SimTerrainCamYaw => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.cam_yaw = v;
            }
            ConfigPath::SimTerrainCamBank => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.cam_bank = v;
            }
            ConfigPath::SimTerrainCamFov => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.cam_fov = v.clamp(0.05, 3.0);
            }
            ConfigPath::SimTerrainCamDistance => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.cam_distance = v.clamp(0.01, 100.0);
            }
            ConfigPath::SimTerrainTargetX => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.target_x = v;
            }
            ConfigPath::SimTerrainTargetY => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.target_y = v;
            }
            ConfigPath::SimTerrainTargetLift => {
                let v: f32 = value.try_into()?;
                config.sim.terrain.target_lift = if v.is_finite() { v.clamp(-1.0e3, 1.0e3) } else { 0.0 };
            }
            ConfigPath::SimPathSamples => {
                let v: u32 = value.try_into()?;
                config.sim.terrain.path.samples = v.clamp(1, 65_536);
            }
            ConfigPath::SimPathNoiseThreshold => {
                let v: f32 = value.try_into()?;
                config.sim.terrain.path.noise_threshold = if v.is_finite() { v.clamp(0.0, 64.0) } else { 0.0 };
            }
            ConfigPath::SimPathEnvironmentMap => {
                let v: String = value.try_into()?;
                config.sim.terrain.path.environment_map = v;
            }
            ConfigPath::SimPathEnvironmentRotation => {
                let v: f32 = value.try_into()?;
                config.sim.terrain.path.environment_rotation = if v.is_finite() { v.rem_euclid(360.0) } else { 0.0 };
            }
            ConfigPath::SimPathEnvironmentHorizon => {
                let v: f32 = value.try_into()?;
                config.sim.terrain.path.environment_horizon = if v.is_finite() { v.clamp(-80.0, 80.0) } else { 0.0 };
            }
            ConfigPath::SimPathEnvironmentStars => {
                let v: f32 = value.try_into()?;
                config.sim.terrain.path.environment_stars = if v.is_finite() { v.clamp(0.0, 16.0) } else { 0.0 };
            }
            ConfigPath::SimPathEnvironmentBackdrop => {
                let v: bool = value.try_into()?;
                config.sim.terrain.path.environment_backdrop = v;
            }
            ConfigPath::SimPathBounces => {
                let v: u32 = value.try_into()?;
                config.sim.terrain.path.bounces = v.min(16);
            }
            ConfigPath::SimPathEnvironment => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.path.environment = v.clamp(0.0, 16.0);
            }
            ConfigPath::SimPathGloss => {
                let v = finite_or_zero(value.try_into()?);
                config.solid_shading.gloss = v.clamp(0.0, 1.0);
            }
            ConfigPath::SimPathRoughness => {
                let v = finite_or_zero(value.try_into()?);
                config.solid_shading.roughness = v.clamp(0.02, 1.0);
            }
            ConfigPath::SimPathEmission => {
                let v = finite_or_zero(value.try_into()?);
                config.solid_shading.glow = v.clamp(0.0, 16.0);
            }
            ConfigPath::SimPathAperture => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.path.aperture = v.clamp(0.0, 1.0);
            }
            ConfigPath::SimPathFocus => {
                let v = finite_or_zero(value.try_into()?);
                config.sim.terrain.path.focus = v.clamp(0.0, 64.0);
            }
            ConfigPath::SimPathDenoise => {
                let v: bool = value.try_into()?;
                config.sim.terrain.path.denoise = v;
            }
            ConfigPath::SimPathSky => {
                let v: bool = value.try_into()?;
                config.sim.terrain.path.sky_gradient = v;
            }
            ConfigPath::SimPathStudio => {
                let v: bool = value.try_into()?;
                config.sim.terrain.path.studio = v;
            }
            ConfigPath::SimPathZenith => {
                let v: [f32; 3] = value.try_into()?;
                config.sim.terrain.path.zenith = v.map(|c| c.clamp(0.0, 1.0));
            }
            ConfigPath::SimDt => {
                let d: f32 = f32::try_from(value)?;
                // Stored as REQUESTED, bounded only by the model's own
                // static ceiling. The stability cap depends on the
                // other parameters, and applying it here made dt move
                // when an unrelated slider did -- see the note on
                // `SimModelParam` below. The cap is applied to the
                // value the solver runs at, in `SimRenderer`.
                // Over every layer, not the flat model: a layered
                // config runs at the tightest of them, and reading
                // `sim.model` here let the clamp accept a dt one layer
                // could not survive.
                let ceiling = config.sim.max_dt_ceiling();
                config.sim.dt = if d.is_finite() { d.clamp(1e-4, ceiling) } else { 1.0 };
            }
            ConfigPath::SimBoundary => {
                let n = String::try_from(value)?;
                if let Some(b) = crate::config::sim::SimBoundary::from_name(&n) {
                    config.sim.boundary = b;
                }
            }
            // Per-step rates, so the bounds are what one step can
            // sanely do: a zoom outside [1/2, 2] a step, or a pan of
            // more than 64 cells a step, is a typo, and a NaN from a
            // signal must not reach the resampler.
            ConfigPath::SimWarpZoom => {
                let v: f32 = f32::try_from(value)?;
                config.sim.warp.zoom = if v.is_finite() { v.clamp(0.5, 2.0) } else { 1.0 };
            }
            // The view: a centre anywhere (a tiled field goes on), a zoom
            // from a speck to a cell filling the screen, any turn.
            ConfigPath::SimViewCenterX => {
                let v: f32 = f32::try_from(value)?;
                config.sim.view.center_x = if v.is_finite() { v.clamp(-1.0e6, 1.0e6) } else { 0.5 };
            }
            ConfigPath::SimViewCenterY => {
                let v: f32 = f32::try_from(value)?;
                config.sim.view.center_y = if v.is_finite() { v.clamp(-1.0e6, 1.0e6) } else { 0.5 };
            }
            ConfigPath::SimViewZoom => {
                let v: f32 = f32::try_from(value)?;
                config.sim.view.zoom = if v.is_finite() { v.clamp(1.0e-3, 1.0e4) } else { 1.0 };
            }
            ConfigPath::SimViewRotation => {
                let v: f32 = f32::try_from(value)?;
                config.sim.view.rotation = if v.is_finite() { v } else { 0.0 };
            }
            ConfigPath::SimViewTile => {
                config.sim.view.tile = bool::try_from(value)?;
            }
            ConfigPath::SimWarpRotation => {
                let v: f32 = f32::try_from(value)?;
                config.sim.warp.rotation =
                    if v.is_finite() { v.clamp(-std::f32::consts::PI, std::f32::consts::PI) } else { 0.0 };
            }
            ConfigPath::SimWarpPanX => {
                let v: f32 = f32::try_from(value)?;
                config.sim.warp.pan_x = if v.is_finite() { v.clamp(-64.0, 64.0) } else { 0.0 };
            }
            ConfigPath::SimWarpPanY => {
                let v: f32 = f32::try_from(value)?;
                config.sim.warp.pan_y = if v.is_finite() { v.clamp(-64.0, 64.0) } else { 0.0 };
            }
            ConfigPath::SimWarpFlow => {
                let v: f32 = f32::try_from(value)?;
                config.sim.warp.flow =
                    if v.is_finite() { v.clamp(-std::f32::consts::PI, std::f32::consts::PI) } else { 0.0 };
            }
            ConfigPath::SimWarpFilter => {
                let n = String::try_from(value)?;
                if let Some(f) = crate::config::sim::SimWarpFilter::from_name(&n) {
                    config.sim.warp.filter = f;
                }
            }
            ConfigPath::SimWarpMode => {
                let n = String::try_from(value)?;
                if let Some(m) = crate::config::sim::SimWarpMode::from_name(&n) {
                    config.sim.warp.mode = m;
                }
            }
            ConfigPath::SimWarpCull => {
                config.sim.warp.cull = bool::try_from(value)?;
            }
            ConfigPath::SimWarpLayers => {
                let v: i32 = i32::try_from(value)?;
                config.sim.warp.layers = (v.clamp(0, 15)) as u32;
            }
            ConfigPath::SimMatteChannel => {
                let n = String::try_from(value)?;
                if let Some(c) = crate::config::sim::SimMatteChannel::from_name(&n) {
                    config.sim.matte.channel = c;
                }
            }
            // The cutoff is in the CHANNEL's units, which a model
            // decides -- a sandpile's heights run to 3, a crystal
            // mass past 2 -- so the range is wide and only a
            // non-finite value is refused.
            ConfigPath::SimMatteCutoff => {
                let v: f32 = f32::try_from(value)?;
                config.sim.matte.cutoff = if v.is_finite() { v } else { 0.5 };
            }
            ConfigPath::SimMatteSoftness => {
                let v: f32 = f32::try_from(value)?;
                config.sim.matte.softness =
                    if v.is_finite() { v.max(0.0) } else { 0.0 };
            }
            ConfigPath::SimMatteInvert => {
                config.sim.matte.invert = bool::try_from(value)?;
            }
            ConfigPath::SimMatteEdge => {
                let n = String::try_from(value)?;
                if let Some(e) = crate::config::sim::SimMatteEdge::from_name(&n) {
                    config.sim.matte.edge = e;
                }
            }
            ConfigPath::SimUpscale => {
                let n = String::try_from(value)?;
                if let Some(u) = crate::config::sim::SimUpscale::from_name(&n) {
                    config.sim.upscale = u;
                }
            }
            ConfigPath::SimDownscale => {
                let n = String::try_from(value)?;
                if let Some(d) = crate::config::sim::SimDownscale::from_name(&n) {
                    config.sim.downscale = d;
                }
            }
            ConfigPath::SimFit => {
                let n = String::try_from(value)?;
                if let Some(v) = crate::config::sim::SimFit::from_name(&n) {
                    config.sim.fit = v;
                }
            }
            ConfigPath::SimModelParam { param } => {
                // ONE FIELD. Raising a diffusion rate does lower the
                // stability cap, and an earlier version pulled `dt`
                // down to meet it here -- which made the Time step
                // slider move when the user dragged Mobility, and
                // never move back, because the pull was one-way. A
                // parameter these models default to sitting exactly at
                // the cap (Cahn-Hilliard) turned every nudge into a
                // ratchet.
                //
                // The solver is protected where it is actually run:
                // `SimRenderer` clamps the uniform to
                // `max_dt_for(params)` on every path, so no
                // combination of stored values can diverge, and the
                // effective step stays a pure function of the config
                // -- which is what reproducibility needs. The panel
                // says so when the cap binds.
                let v: f32 = f32::try_from(value)?;
                config.sim.model_params.insert(param.clone(), v);
            }
            ConfigPath::SimColoringParam { param } => {
                let v: f32 = f32::try_from(value)?;
                config.sim.coloring_params.insert(param.clone(), v);
            }
            ConfigPath::SimLayerModel { layer } => {
                let name = String::try_from(value)?;
                if let Some(l) = config.sim.layers.get_mut(*layer) {
                    if l.model != name {
                        // As for SimModel: parameters belong to the model
                        // that declared them -- kept for a new version of
                        // the same Rhai model.
                        if !crate::config::sim::same_script(&name, &l.model) {
                            l.model_params.clear();
                        }
                        l.model = name;
                    }
                }
            }
            ConfigPath::SimLayerCustomModel { layer } => {
                let s: String = value.try_into()?;
                if let Some(l) = config.sim.layers.get_mut(*layer) {
                    l.custom_model = custom_source_from_json(&s)?;
                }
            }
            ConfigPath::SimLayerEnabled { layer } => {
                let v = bool::try_from(value)?;
                if let Some(l) = config.sim.layers.get_mut(*layer) {
                    l.enabled = v;
                }
            }
            ConfigPath::SimLayerParam { layer, param } => {
                let v: f32 = f32::try_from(value)?;
                if let Some(l) = config.sim.layers.get_mut(*layer) {
                    l.model_params.insert(param.clone(), v);
                }
            }
            ConfigPath::SimCouplingFrom { index } => {
                let v: i32 = i32::try_from(value)?;
                if let Some(c) = config.sim.couplings.get_mut(*index) {
                    c.from = v.max(0) as usize;
                }
            }
            ConfigPath::SimCouplingTo { index } => {
                let v: i32 = i32::try_from(value)?;
                if let Some(c) = config.sim.couplings.get_mut(*index) {
                    c.to = v.max(0) as usize;
                }
            }
            ConfigPath::SimCouplingForm { index } => {
                let n = String::try_from(value)?;
                if let (Some(c), Some(f)) = (
                    config.sim.couplings.get_mut(*index),
                    crate::config::sim::SimCouplingForm::from_name(&n),
                ) {
                    c.form = f;
                }
            }
            ConfigPath::SimCouplingStrength { index } => {
                let v: f32 = f32::try_from(value)?;
                if let Some(c) = config.sim.couplings.get_mut(*index) {
                    c.strength = if v.is_finite() { v } else { 0.0 };
                }
            }
            ConfigPath::SimCouplingChannels { index } => {
                let v: i32 = i32::try_from(value)?;
                if let Some(c) = config.sim.couplings.get_mut(*index) {
                    c.channels = v.clamp(0, 15) as u32;
                }
            }
            ConfigPath::SimUseTransforms => {
                config.sim.use_transforms = bool::try_from(value)?;
            }
            ConfigPath::SimColorLayerParam { index, param } => {
                let v: f32 = f32::try_from(value)?;
                if let Some(l) = config.sim.color_layers.get_mut(*index) {
                    l.coloring_params.insert(param.clone(), v);
                }
            }
            ConfigPath::SimColorLayerOpacity { index } => {
                let v: f32 = f32::try_from(value)?;
                if let Some(l) = config.sim.color_layers.get_mut(*index) {
                    l.opacity = if v.is_finite() { v.clamp(0.0, 1.0) } else { 1.0 };
                }
            }
            ConfigPath::SimColorLayerMatteCutoff { index } => {
                let v: f32 = f32::try_from(value)?;
                if let Some(l) = config.sim.color_layers.get_mut(*index) {
                    l.matte.cutoff = if v.is_finite() { v } else { 0.5 };
                }
            }
            ConfigPath::SimColorLayerMatteSoftness { index } => {
                let v: f32 = f32::try_from(value)?;
                if let Some(l) = config.sim.color_layers.get_mut(*index) {
                    l.matte.softness = if v.is_finite() { v.max(0.0) } else { 0.0 };
                }
            }
            ConfigPath::EscapeSupersample => {
                let v: u32 = value.try_into()?;
                config.escape.supersample =
                    v.clamp(1, crate::config::escape::MAX_SUPERSAMPLE);
            }
            ConfigPath::EscapeDownsample => {
                let v: String = value.try_into()?;
                config.escape.downsample =
                    crate::config::escape::DownsampleMode::from_str_or_default(&v);
            }
            ConfigPath::EscapeShadingEnabled => {
                config.escape.shading.enabled = value.try_into()?;
            }
            ConfigPath::EscapeShadingLightAngle => {
                let v: f32 = value.try_into()?;
                config.escape.shading.light_angle = v.rem_euclid(360.0);
            }
            ConfigPath::EscapeShadingHeight => {
                let v: f32 = value.try_into()?;
                config.escape.shading.height = v.clamp(0.0, 100_000.0);
            }
            ConfigPath::EscapeShadingField => {
                let v: String = value.try_into()?;
                config.escape.shading.field =
                    crate::config::escape::shading_field_from_str(&v);
            }
            ConfigPath::EscapeContrastMode => {
                let v: String = value.try_into()?;
                config.escape.contrast.mode =
                    crate::config::escape::contrast_mode_from_str(&v);
            }
            ConfigPath::EscapeContrastClip => {
                let v: f32 = value.try_into()?;
                config.escape.contrast.clip = v.clamp(0.0, 0.25);
            }
            ConfigPath::EscapeContrastStrength => {
                let v: f32 = value.try_into()?;
                config.escape.contrast.strength = v.clamp(0.0, 1.0);
            }
            ConfigPath::EscapeContrastTurns => {
                let v: f32 = value.try_into()?;
                config.escape.contrast.turns = v.clamp(0.05, 64.0);
            }
            ConfigPath::EscapeTransfer => {
                let v: String = value.try_into()?;
                config.escape.palette_map.transfer =
                    crate::config::escape::TransferCurve::from_name(&v);
            }
            ConfigPath::EscapeTransferPivot => {
                let v: f32 = value.try_into()?;
                let (lo, hi) = crate::config::escape::PIVOT_RANGE;
                config.escape.palette_map.pivot = v.clamp(lo, hi);
            }
            ConfigPath::EscapePaletteCurve => {
                let v: String = value.try_into()?;
                config.escape.palette_map.curve =
                    crate::config::escape::PaletteCurve::from_name(&v);
            }
            ConfigPath::EscapePaletteStepped => {
                config.escape.palette_map.stepped = value.try_into()?;
            }
            ConfigPath::EscapeShadingShadowColor => {
                config.escape.shading.shadow_color = value.try_into()?;
            }
            ConfigPath::EscapeShadingShadowStrength => {
                let v: f32 = value.try_into()?;
                config.escape.shading.shadow_strength = v.clamp(0.0, 4.0);
            }
            ConfigPath::EscapeShadingShadowBlend => {
                let v: String = value.try_into()?;
                config.escape.shading.shadow_blend =
                    crate::config::escape::shading_blend_from_str(&v);
            }
            ConfigPath::EscapeShadingHighlightColor => {
                config.escape.shading.highlight_color = value.try_into()?;
            }
            ConfigPath::EscapeShadingHighlightStrength => {
                let v: f32 = value.try_into()?;
                config.escape.shading.highlight_strength = v.clamp(0.0, 4.0);
            }
            ConfigPath::EscapeShadingSoftness => {
                let v: f32 = value.try_into()?;
                config.escape.shading.softness = v.clamp(0.0, 16.0);
            }
            ConfigPath::EscapeTerrainEnabled => {
                config.escape.terrain.enabled = value.try_into()?;
            }
            ConfigPath::EscapeTerrainSource => {
                let v: String = value.try_into()?;
                config.escape.terrain.source = crate::config::escape::TerrainSource::from_str_or_default(&v);
            }
            ConfigPath::EscapeTerrainHeight => {
                let v: f32 = value.try_into()?;
                config.escape.terrain.height = v.clamp(0.0, 1.0);
            }
            ConfigPath::EscapeTerrainDeWidth => {
                let v: f32 = value.try_into()?;
                config.escape.terrain.de_width = v.clamp(1.0e-4, 0.5);
            }
            ConfigPath::EscapeTerrainDetail => {
                let v: f32 = value.try_into()?;
                config.escape.terrain.detail = v.clamp(0.1, 8.0);
            }
            ConfigPath::EscapeTerrainFar => {
                let v: f32 = value.try_into()?;
                config.escape.terrain.far = v.clamp(0.5, 64.0);
            }
            ConfigPath::EscapeTerrainHaze => {
                let v: f32 = value.try_into()?;
                config.escape.terrain.haze = v.clamp(0.0, 1.0);
            }
            ConfigPath::EscapeTerrainTargetLift => {
                // Bounded where the f32 world still resolves a section:
                // a target a thousand view widths up is a dot of ground.
                let v: f32 = value.try_into()?;
                config.escape.terrain.target_lift = if v.is_finite() { v.clamp(-1.0e3, 1.0e3) } else { 0.0 };
            }
            ConfigPath::EscapeTerrainTier => {
                let v: String = value.try_into()?;
                config.escape.terrain.tier = crate::config::escape::RenderTier::from_str_or_default(&v);
            }
            ConfigPath::EscapeSolidTier => {
                let v: String = value.try_into()?;
                config.escape.solid_tier = crate::config::escape::RenderTier::from_str_or_default(&v);
            }
            ConfigPath::EscapePathSamples => {
                let v: u32 = value.try_into()?;
                config.escape.path.samples = v.clamp(1, 65_536);
            }
            ConfigPath::EscapePathNoiseThreshold => {
                let v: f32 = value.try_into()?;
                config.escape.path.noise_threshold = if v.is_finite() { v.clamp(0.0, 64.0) } else { 0.0 };
            }
            ConfigPath::EscapePathBounces => {
                let v: u32 = value.try_into()?;
                config.escape.path.bounces = v.min(16);
            }
            ConfigPath::EscapePathEnvironment => {
                let v: f32 = value.try_into()?;
                config.escape.path.environment = v.clamp(0.0, 16.0);
            }
            ConfigPath::EscapePathGloss => {
                let v: f32 = value.try_into()?;
                config.solid_shading.gloss = v.clamp(0.0, 1.0);
            }
            ConfigPath::EscapePathRoughness => {
                let v: f32 = value.try_into()?;
                config.solid_shading.roughness = v.clamp(0.02, 1.0);
            }
            ConfigPath::EscapePathEmission => {
                let v: f32 = value.try_into()?;
                config.solid_shading.glow = v.clamp(0.0, 16.0);
            }
            ConfigPath::EscapePathAperture => {
                let v: f32 = value.try_into()?;
                config.escape.path.aperture = v.clamp(0.0, 1.0);
            }
            ConfigPath::EscapePathFocus => {
                let v: f32 = value.try_into()?;
                config.escape.path.focus = v.clamp(0.0, 64.0);
            }
            ConfigPath::EscapePathDenoise => {
                let v: bool = value.try_into()?;
                config.escape.path.denoise = v;
            }
            ConfigPath::EscapePathSky => {
                let v: bool = value.try_into()?;
                config.escape.path.sky_gradient = v;
            }
            ConfigPath::EscapePathStudio => {
                let v: bool = value.try_into()?;
                config.escape.path.studio = v;
            }
            ConfigPath::EscapePathEnvironmentMap => {
                let v: String = value.try_into()?;
                config.escape.path.environment_map = v;
            }
            ConfigPath::EscapePathEnvironmentRotation => {
                let v: f32 = value.try_into()?;
                config.escape.path.environment_rotation = if v.is_finite() { v.rem_euclid(360.0) } else { 0.0 };
            }
            ConfigPath::EscapePathEnvironmentHorizon => {
                let v: f32 = value.try_into()?;
                config.escape.path.environment_horizon = if v.is_finite() { v.clamp(-80.0, 80.0) } else { 0.0 };
            }
            ConfigPath::EscapePathEnvironmentStars => {
                let v: f32 = value.try_into()?;
                config.escape.path.environment_stars = if v.is_finite() { v.clamp(0.0, 16.0) } else { 0.0 };
            }
            ConfigPath::EscapePathEnvironmentBackdrop => {
                let v: bool = value.try_into()?;
                config.escape.path.environment_backdrop = v;
            }
            ConfigPath::EscapePathZenith => {
                let v: [f32; 3] = value.try_into()?;
                config.escape.path.zenith = v.map(|c| c.clamp(0.0, 1.0));
            }
            ConfigPath::EscapeTerrainInterior => {
                let v: String = value.try_into()?;
                config.escape.terrain.interior = crate::config::escape::TerrainInterior::from_str_or_default(&v);
            }
            ConfigPath::EscapeTerrainCurve => {
                let v: String = value.try_into()?;
                config.escape.terrain.curve = crate::config::escape::TerrainCurve::from_str_or_default(&v);
            }
            ConfigPath::EscapeTerrainInvert => {
                config.escape.terrain.invert = value.try_into()?;
            }
            ConfigPath::EscapeTerrainLocked => {
                config.escape.terrain.locked = value.try_into()?;
            }
            ConfigPath::EscapeTerrainLockZoom => {
                let v: f64 = value.try_into()?;
                if v.is_finite() {
                    config.escape.terrain.lock_zoom = v;
                }
            }
            ConfigPath::EscapeTerrainLockLo => {
                config.escape.terrain.lock_range[0] = value.try_into()?;
            }
            ConfigPath::EscapeTerrainLockHi => {
                config.escape.terrain.lock_range[1] = value.try_into()?;
            }
            ConfigPath::EscapeTerrainPrefilter => {
                let v: u32 = value.try_into()?;
                config.escape.terrain.prefilter = v.clamp(1, crate::config::escape::MAX_TERRAIN_PREFILTER);
            }
            ConfigPath::EscapeTerrainSmooth => {
                config.escape.terrain.smooth = value.try_into()?;
            }
            ConfigPath::EscapeTerrainSoftness => {
                let v: f32 = value.try_into()?;
                config.escape.terrain.softness = if v.is_finite() { v.clamp(0.0, crate::config::escape::MAX_TERRAIN_SOFTNESS) } else { 0.0 };
            }
            ConfigPath::EscapeTerrainShadow => {
                let v: f32 = value.try_into()?;
                config.escape.terrain.shadow = v.clamp(0.0, 1.0);
            }
            ConfigPath::EscapeTerrainShadowSharpness => {
                let v: f32 = value.try_into()?;
                config.escape.terrain.shadow_sharpness = v.clamp(1.0, 128.0);
            }
            ConfigPath::EscapeTerrainLakeTint => {
                let v: [f32; 3] = value.try_into()?;
                config.escape.terrain.lake_tint = v.map(|c| c.clamp(0.0, 1.0));
            }
            ConfigPath::EscapeTerrainLakeRoughness => {
                let v: f32 = value.try_into()?;
                config.escape.terrain.lake_roughness = v.clamp(0.02, 1.0);
            }
            ConfigPath::EscapeTerrainOcclusion => {
                let v: f32 = value.try_into()?;
                config.escape.terrain.occlusion = v.clamp(0.0, 0.1);
            }
            ConfigPath::EscapeShadingTextureKind => {
                let v: String = value.try_into()?;
                config.escape.shading.texture_kind =
                    crate::config::escape::ShadingTexture::from_str_or_default(&v);
            }
            ConfigPath::EscapeShadingTextureStrength => {
                let v: f32 = value.try_into()?;
                config.escape.shading.texture_strength = v.clamp(0.0, 4.0);
            }
            ConfigPath::EscapeShadingTextureScale => {
                let v: f32 = value.try_into()?;
                config.escape.shading.texture_scale = v.clamp(0.25, 64.0);
            }
            ConfigPath::EscapeShadingTextureWhere => {
                let v: String = value.try_into()?;
                config.escape.shading.texture_where = crate::config::escape::TextureWhere::from_name(&v);
            }
            ConfigPath::EscapeShadingTextureAngle => {
                let v: f32 = value.try_into()?;
                let (lo, hi) = crate::config::escape::TEXTURE_ANGLE_RANGE;
                config.escape.shading.texture_angle = v.clamp(lo, hi);
            }
            ConfigPath::EscapeShadingTextureAngleSoft => {
                let v: f32 = value.try_into()?;
                let (lo, hi) = crate::config::escape::TEXTURE_ANGLE_SOFT_RANGE;
                config.escape.shading.texture_angle_soft = v.clamp(lo, hi);
            }
            ConfigPath::EscapeShadingModel => {
                let v: String = value.try_into()?;
                config.escape.shading.model = crate::config::escape::ReliefModel::from_name(&v);
            }
            ConfigPath::EscapeShadingElevation => {
                let v: f32 = value.try_into()?;
                config.escape.shading.elevation = v.clamp(0.0, 90.0);
            }
            ConfigPath::EscapeShadingAmbient => {
                let v: f32 = value.try_into()?;
                config.escape.shading.ambient = v.clamp(0.0, 1.0);
            }
            ConfigPath::EscapeShadingStencil => {
                let v: String = value.try_into()?;
                config.escape.shading.stencil = crate::config::escape::SlopeStencil::from_name(&v);
            }
            ConfigPath::EscapeShadingHeightCurve => {
                let v: String = value.try_into()?;
                config.escape.shading.height_curve =
                    crate::config::escape::HeightTransfer::from_name(&v);
            }
            ConfigPath::EscapeShadingHeightPre => {
                let v: f32 = value.try_into()?;
                config.escape.shading.height_pre = v.clamp(1e-3, 1e3);
            }
            ConfigPath::EscapeShadingHeightPost => {
                let v: f32 = value.try_into()?;
                config.escape.shading.height_post = v.clamp(1e-3, 1e3);
            }
            ConfigPath::EscapeShadingOffset => {
                let v: f32 = value.try_into()?;
                let (lo, hi) = crate::config::escape::RELIEF_OFFSET_RANGE;
                config.escape.shading.offset = v.clamp(lo, hi);
            }
            ConfigPath::EscapeShadingEmboss => {
                let v: String = value.try_into()?;
                config.escape.shading.emboss = crate::config::escape::EmbossType::from_name(&v);
            }
            ConfigPath::EscapeShadingEmbossSections => {
                let v: u32 = value.try_into()?;
                let (lo, hi) = crate::config::escape::EMBOSS_SECTIONS_RANGE;
                config.escape.shading.emboss_sections = v.clamp(lo, hi);
            }
            ConfigPath::EscapeShadingCreasePattern => {
                let v: String = value.try_into()?;
                config.escape.shading.creases.pattern = crate::config::escape::CreasePattern::from_name(&v);
            }
            ConfigPath::EscapeShadingCreaseDecay => {
                let v: f32 = value.try_into()?;
                let (lo, hi) = crate::config::escape::CREASE_DECAY_RANGE;
                config.escape.shading.creases.decay = v.clamp(lo, hi);
            }
            ConfigPath::EscapeShadingCreaseFlip => {
                config.escape.shading.creases.flip = value.try_into()?;
            }
            ConfigPath::EscapeTextureOverlay => {
                config.escape.texture_overlay.enabled = value.try_into()?;
            }
            ConfigPath::EscapeTextureOverlayMerge => {
                let v: f32 = value.try_into()?;
                config.escape.texture_overlay.merge = v.clamp(0.0, 1.0);
            }
            ConfigPath::EscapeTextureOverlayPower => {
                let v: f32 = value.try_into()?;
                let (lo, hi) = crate::config::escape::OVERLAY_POWER_RANGE;
                config.escape.texture_overlay.power = v.round().clamp(lo, hi);
            }
            ConfigPath::EscapeTextureOverlayRatio => {
                let v: f32 = value.try_into()?;
                let (lo, hi) = crate::config::escape::OVERLAY_RATIO_RANGE;
                config.escape.texture_overlay.ratio = v.clamp(lo, hi);
            }
            ConfigPath::EscapeTextureOverlayFit => {
                let v: String = value.try_into()?;
                config.escape.texture_overlay.fit = crate::config::escape::TextureFit::from_name(&v);
            }
            ConfigPath::EscapeTextureOverlayTile => {
                let v: f32 = value.try_into()?;
                let (lo, hi) = crate::config::escape::OVERLAY_TILE_RANGE;
                config.escape.texture_overlay.tile_scale = v.clamp(lo, hi);
            }
            ConfigPath::EscapeShadingHighlightBlend => {
                let v: String = value.try_into()?;
                config.escape.shading.highlight_blend =
                    crate::config::escape::shading_blend_from_str(&v);
            }
            ConfigPath::EscapeReferencePeriod => {
                let v: u32 = value.try_into()?;
                config.escape.reference_period = if v == 0 { None } else { Some(v) };
            }
            ConfigPath::EscapeBailout => {
                config.escape.bailout = value.try_into()?;
            }
            ConfigPath::EscapeDampingRe => {
                config.escape.damping_re = value.try_into()?;
            }
            ConfigPath::EscapeDampingIm => {
                config.escape.damping_im = value.try_into()?;
            }
            ConfigPath::EscapeBiomorph => {
                let s: String = value.try_into()?;
                config.escape.biomorph = crate::config::escape::biomorph_from_str(&s)
                    .ok_or(ConfigError::TypeMismatch)?;
            }
            ConfigPath::EscapeColoring => {
                config.escape.coloring = value.try_into()?;
            }
            ConfigPath::EscapeFormulaParam { param } => {
                let v: f32 = value.try_into()?;
                // `name.r` / `name.angle` write a complex parameter's parts.
                let writes = config.escape.formula_param_writes(param, v, |p| crate::escape::param_default(&config.escape, crate::escape::ParamOf::Formula, p));
                for (k, x) in writes {
                    config.escape.formula_params.insert(k, x);
                }
            }
            ConfigPath::EscapeColoringParam { param } => {
                let v: f32 = value.try_into()?;
                config.escape.coloring_params.insert(param.clone(), v);
            }
            ConfigPath::EscapeLayerColoring => {
                config.escape.layer.coloring = value.try_into()?;
            }
            ConfigPath::EscapeLayerParam { param } => {
                let v: f32 = value.try_into()?;
                config.escape.layer.params.insert(param.clone(), v);
            }
            ConfigPath::EscapeLayerBlend => {
                let v: String = value.try_into()?;
                config.escape.layer.blend = crate::config::escape::LayerBlend::from_name(&v);
            }
            ConfigPath::EscapeLayerWeight => {
                let v: f32 = value.try_into()?;
                config.escape.layer.weight = v.clamp(0.0, 1.0);
            }
            ConfigPath::EscapeLens => {
                config.escape.lens = value.try_into()?;
            }
            ConfigPath::EscapeLensAmount => {
                let v = finite_or_zero(value.try_into()?);
                let lim = crate::config::escape::LENS_AMOUNT_LIMIT;
                config.escape.lens_amount = v.clamp(-lim, lim);
            }
            ConfigPath::EscapeSolidDetail => {
                let v: f32 = value.try_into()?;
                let (lo, hi) = crate::config::escape::SOLID_DETAIL_RANGE;
                config.escape.solid_detail = if v.is_finite() { v.clamp(lo, hi) } else { hi };
            }
            ConfigPath::EscapeLensParam { param } => {
                let v: f32 = value.try_into()?;
                config.escape.lens_params.insert(param.clone(), v);
            }
            ConfigPath::EscapeTransforms => {
                let s: String = value.try_into()?;
                config.escape.transforms = crate::config::escape::transforms_from_wire(&s).map_err(|_| ConfigError::TypeMismatch)?;
            }
            ConfigPath::EscapeTransformAmount { index } => {
                let v: f32 = value.try_into()?;
                let lim = crate::config::escape::LENS_AMOUNT_LIMIT;
                config.escape.transforms.get_mut(*index).ok_or_else(|| ConfigError::InvalidPath(format!("no transformation {index}")))?.amount = finite_or_zero(v).clamp(-lim, lim);
            }
            ConfigPath::EscapeTransformParam { index, param } => {
                let v: f32 = value.try_into()?;
                config.escape.transforms.get_mut(*index).ok_or_else(|| ConfigError::InvalidPath(format!("no transformation {index}")))?.params.insert(param.clone(), v);
            }

            // Transforms
            ConfigPath::TransformCount => {
                // Can't directly set count - must add/remove transforms
                return Err(ConfigError::ReadOnlyParameter);
            }
            ConfigPath::TransformWeight { index } => {
                let xform = normal_transform_mut(config, target, *index)?;
                xform.weight = value.try_into()?;
            }
            ConfigPath::TransformColor { index } => {
                let xform = normal_transform_mut(config, target, *index)?;
                xform.color = value.try_into()?;
            }
            ConfigPath::TransformColorSpeed { index } => {
                let xform = normal_transform_mut(config, target, *index)?;
                xform.color_speed = value.try_into()?;
            }
            ConfigPath::TransformOpacity { index } => {
                let xform = normal_transform_mut(config, target, *index)?;
                xform.opacity = value.try_into()?;
            }
            ConfigPath::TransformDirectColor { index } => {
                let xform = normal_transform_mut(config, target, *index)?;
                xform.direct_color = value.try_into()?;
            }
            ConfigPath::TransformAffine { index, param } => {
                let xform = normal_transform_mut(config, target, *index)?;
                let new_value: f32 = value.try_into()?;
                match param {
                    AffineParam::A => xform.a = new_value,
                    AffineParam::B => xform.b = new_value,
                    AffineParam::C => xform.c = new_value,
                    AffineParam::D => xform.d = new_value,
                    AffineParam::E => xform.e = new_value,
                    AffineParam::F => xform.f = new_value,
                    AffineParam::G => xform.g = new_value,
                }
            }
            ConfigPath::TransformPostAffineEnabled { index } => {
                let xform = normal_transform_mut(config, target, *index)?;
                xform.post_affine_enabled = value.try_into()?;
            }
            ConfigPath::TransformPostAffine { index, param } => {
                let xform = normal_transform_mut(config, target, *index)?;
                let new_value: f32 = value.try_into()?;
                match param {
                    AffineParam::A => xform.post_a = new_value,
                    AffineParam::B => xform.post_b = new_value,
                    AffineParam::C => xform.post_c = new_value,
                    AffineParam::D => xform.post_d = new_value,
                    AffineParam::E => xform.post_e = new_value,
                    AffineParam::F => xform.post_f = new_value,
                    AffineParam::G => xform.post_g = new_value,
                }
            }
            // JWildfire plane coefs — write the one position into the
            // [f32; 6] array. The GpuTransform builder picks up the
            // change on the next upload and re-computes plane_flags
            // automatically (identity comparison drops back to the
            // flat path when values return to identity).
            ConfigPath::TransformYzCoefs { index, position } => {
                let xform = normal_transform_mut(config, target, *index)?;
                let new_value: f32 = value.try_into()?;
                if let Some(slot) = xform.yz_coefs.get_mut(*position as usize) {
                    *slot = new_value;
                }
            }
            ConfigPath::TransformZxCoefs { index, position } => {
                let xform = normal_transform_mut(config, target, *index)?;
                let new_value: f32 = value.try_into()?;
                if let Some(slot) = xform.zx_coefs.get_mut(*position as usize) {
                    *slot = new_value;
                }
            }
            ConfigPath::TransformYzPostCoefs { index, position } => {
                let xform = normal_transform_mut(config, target, *index)?;
                let new_value: f32 = value.try_into()?;
                if let Some(slot) = xform.yz_post_coefs.get_mut(*position as usize) {
                    *slot = new_value;
                }
            }
            ConfigPath::TransformZxPostCoefs { index, position } => {
                let xform = normal_transform_mut(config, target, *index)?;
                let new_value: f32 = value.try_into()?;
                if let Some(slot) = xform.zx_post_coefs.get_mut(*position as usize) {
                    *slot = new_value;
                }
            }
            ConfigPath::TransformVariation { index, variation } => {
                let xform = normal_transform_mut(config, target, *index)?;
                apply_variation_weight(xform, variation, value)?;
            }
            ConfigPath::TransformVariationPriority { index, variation } => {
                let xform = normal_transform_mut(config, target, *index)?;
                apply_variation_priority(xform, variation, value)?;
            }
            ConfigPath::TransformVariationOrder { index } => {
                let xform = normal_transform_mut(config, target, *index)?;
                xform.variation_order = value.try_into()?;
            }
            ConfigPath::TransformVariationParam { index, variation, param } => {
                let xform = normal_transform_mut(config, target, *index)?;
                apply_variation_param(xform, variation, param, value)?;
            }
            // High-level transform operations (translate, rotate, scale)
            ConfigPath::TransformOriginX { index } => {
                let xform = normal_transform_mut(config, target, *index)?;
                let new_value: f32 = value.try_into()?;
                xform.set_origin_x(new_value);
            }
            ConfigPath::TransformOriginY { index } => {
                let xform = normal_transform_mut(config, target, *index)?;
                let new_value: f32 = value.try_into()?;
                xform.set_origin_y(new_value);
            }
            ConfigPath::TransformRotation { index } => {
                let xform = normal_transform_mut(config, target, *index)?;
                let new_value: f32 = value.try_into()?;
                xform.set_rotation(new_value);
            }
            ConfigPath::TransformScale { index } => {
                let xform = normal_transform_mut(config, target, *index)?;
                let new_value: f32 = value.try_into()?;
                xform.set_scale(new_value);
            }
            // High-level post-affine ops (normal pool)
            ConfigPath::TransformPostAffineOriginX { index } => {
                let xform = normal_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_origin_x(v);
            }
            ConfigPath::TransformPostAffineOriginY { index } => {
                let xform = normal_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_origin_y(v);
            }
            ConfigPath::TransformPostAffineRotation { index } => {
                let xform = normal_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_rotation(v);
            }
            ConfigPath::TransformPostAffineScale { index } => {
                let xform = normal_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_scale(v);
            }

            // High-level pre-affine ops (linked pool)
            ConfigPath::LinkedTransformOriginX { index } => {
                let xform = linked_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_origin_x(v);
            }
            ConfigPath::LinkedTransformOriginY { index } => {
                let xform = linked_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_origin_y(v);
            }
            ConfigPath::LinkedTransformRotation { index } => {
                let xform = linked_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_rotation(v);
            }
            ConfigPath::LinkedTransformScale { index } => {
                let xform = linked_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_scale(v);
            }
            // High-level post-affine ops (linked pool)
            ConfigPath::LinkedTransformPostAffineOriginX { index } => {
                let xform = linked_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_origin_x(v);
            }
            ConfigPath::LinkedTransformPostAffineOriginY { index } => {
                let xform = linked_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_origin_y(v);
            }
            ConfigPath::LinkedTransformPostAffineRotation { index } => {
                let xform = linked_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_rotation(v);
            }
            ConfigPath::LinkedTransformPostAffineScale { index } => {
                let xform = linked_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_scale(v);
            }

            // High-level pre-affine ops (final pool)
            ConfigPath::FinalTransformOriginX { index } => {
                let xform = final_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_origin_x(v);
            }
            ConfigPath::FinalTransformOriginY { index } => {
                let xform = final_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_origin_y(v);
            }
            ConfigPath::FinalTransformRotation { index } => {
                let xform = final_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_rotation(v);
            }
            ConfigPath::FinalTransformScale { index } => {
                let xform = final_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_scale(v);
            }
            // High-level post-affine ops (final pool)
            ConfigPath::FinalTransformPostAffineOriginX { index } => {
                let xform = final_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_origin_x(v);
            }
            ConfigPath::FinalTransformPostAffineOriginY { index } => {
                let xform = final_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_origin_y(v);
            }
            ConfigPath::FinalTransformPostAffineRotation { index } => {
                let xform = final_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_rotation(v);
            }
            ConfigPath::FinalTransformPostAffineScale { index } => {
                let xform = final_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                xform.set_post_scale(v);
            }

            // Legacy `FinalTransform*` (no index) variants were removed
            // in Phase 9. The migration shim in
            // `ConfigPath::from_string_key` maps the legacy string form
            // to indexed variants at index 0; those go through the
            // indexed `FinalTransform*` arms in the Final Transform pool
            // section below.

            // Linked Transform pool — same shape as TransformXxx but
            // sourced from flame.linked_transforms[index].
            ConfigPath::LinkedTransformAffine { index, param } => {
                let xform = linked_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                match param {
                    AffineParam::A => xform.a = v, AffineParam::B => xform.b = v,
                    AffineParam::C => xform.c = v, AffineParam::D => xform.d = v,
                    AffineParam::E => xform.e = v, AffineParam::F => xform.f = v,
                    AffineParam::G => xform.g = v,
                };
            }
            ConfigPath::LinkedTransformPostAffineEnabled { index } => {
                let xform = linked_transform_mut(config, target, *index)?;
                xform.post_affine_enabled = value.try_into()?;
            }
            ConfigPath::LinkedTransformPostAffine { index, param } => {
                let xform = linked_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                match param {
                    AffineParam::A => xform.post_a = v, AffineParam::B => xform.post_b = v,
                    AffineParam::C => xform.post_c = v, AffineParam::D => xform.post_d = v,
                    AffineParam::E => xform.post_e = v, AffineParam::F => xform.post_f = v,
                    AffineParam::G => xform.post_g = v,
                };
            }
            // JWildfire plane coefs on the Linked pool. Same per-slot
            // write pattern as the Normal-pool variants — the
            // GpuTransform builder picks up the change on next upload.
            ConfigPath::LinkedTransformYzCoefs { index, position } => {
                let xform = linked_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.yz_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::LinkedTransformZxCoefs { index, position } => {
                let xform = linked_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.zx_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::LinkedTransformYzPostCoefs { index, position } => {
                let xform = linked_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.yz_post_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::LinkedTransformZxPostCoefs { index, position } => {
                let xform = linked_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.zx_post_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::LinkedTransformVariation { index, variation } => {
                let xform = linked_transform_mut(config, target, *index)?;
                apply_variation_weight(xform, variation, value)?;
            }
            ConfigPath::LinkedTransformVariationPriority { index, variation } => {
                let xform = linked_transform_mut(config, target, *index)?;
                apply_variation_priority(xform, variation, value)?;
            }
            ConfigPath::LinkedTransformVariationOrder { index } => {
                let xform = linked_transform_mut(config, target, *index)?;
                xform.variation_order = value.try_into()?;
            }
            ConfigPath::LinkedTransformVariationParam { index, variation, param } => {
                let xform = linked_transform_mut(config, target, *index)?;
                apply_variation_param(xform, variation, param, value)?;
            }

            // Final Transform pool — sourced from flame.final_transforms[index].
            ConfigPath::FinalTransformAffine { index, param } => {
                let xform = final_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                match param {
                    AffineParam::A => xform.a = v, AffineParam::B => xform.b = v,
                    AffineParam::C => xform.c = v, AffineParam::D => xform.d = v,
                    AffineParam::E => xform.e = v, AffineParam::F => xform.f = v,
                    AffineParam::G => xform.g = v,
                };
            }
            ConfigPath::FinalTransformPostAffineEnabled { index } => {
                let xform = final_transform_mut(config, target, *index)?;
                xform.post_affine_enabled = value.try_into()?;
            }
            ConfigPath::FinalTransformPostAffine { index, param } => {
                let xform = final_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                match param {
                    AffineParam::A => xform.post_a = v, AffineParam::B => xform.post_b = v,
                    AffineParam::C => xform.post_c = v, AffineParam::D => xform.post_d = v,
                    AffineParam::E => xform.post_e = v, AffineParam::F => xform.post_f = v,
                    AffineParam::G => xform.post_g = v,
                };
            }
            // JWildfire plane coefs on the Final pool.
            ConfigPath::FinalTransformYzCoefs { index, position } => {
                let xform = final_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.yz_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::FinalTransformZxCoefs { index, position } => {
                let xform = final_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.zx_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::FinalTransformYzPostCoefs { index, position } => {
                let xform = final_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.yz_post_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::FinalTransformZxPostCoefs { index, position } => {
                let xform = final_transform_mut(config, target, *index)?;
                let v: f32 = value.try_into()?;
                if let Some(slot) = xform.zx_post_coefs.get_mut(*position as usize) { *slot = v; }
            }
            ConfigPath::FinalTransformVariation { index, variation } => {
                let xform = final_transform_mut(config, target, *index)?;
                apply_variation_weight(xform, variation, value)?;
            }
            ConfigPath::FinalTransformVariationPriority { index, variation } => {
                let xform = final_transform_mut(config, target, *index)?;
                apply_variation_priority(xform, variation, value)?;
            }
            ConfigPath::FinalTransformVariationOrder { index } => {
                let xform = final_transform_mut(config, target, *index)?;
                xform.variation_order = value.try_into()?;
            }
            ConfigPath::FinalTransformVariationParam { index, variation, param } => {
                let xform = final_transform_mut(config, target, *index)?;
                apply_variation_param(xform, variation, param, value)?;
            }

            // Scene-level render state — config-level since v3 (write to the
            // config, not the resolved flame target).
            ConfigPath::RenderMode => {
                config.render_mode = value.try_into()?;
            }
            ConfigPath::PerspectiveStrength => {
                config.perspective_strength = value.try_into()?;
            }
            ConfigPath::DepthDensityCompensation => {
                config.depth_density_compensation = value.try_into()?;
            }
            ConfigPath::FarDensityFade => {
                config.far_density_fade = value.try_into()?;
            }
            ConfigPath::SolidStrength => {
                config.solid_strength = value.try_into()?;
            }
            ConfigPath::SurfaceThickness => {
                config.surface_thickness = value.try_into()?;
            }
            ConfigPath::SolidShadowStrength => {
                config.solid_shading.shadow_strength = value.try_into()?;
            }
            ConfigPath::ShadingStrength => {
                config.solid_shading.shading_strength = value.try_into()?;
            }
            ConfigPath::SolidAmbient => {
                config.solid_shading.ambient = value.try_into()?;
            }
            ConfigPath::SolidDiffuse => {
                config.solid_shading.diffuse = value.try_into()?;
            }
            ConfigPath::SolidSpecular => {
                config.solid_shading.specular = value.try_into()?;
            }
            ConfigPath::SolidShininess => {
                config.solid_shading.shininess = value.try_into()?;
            }
            ConfigPath::SolidGloss => {
                let v: f32 = value.try_into()?;
                config.solid_shading.gloss = if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
            }
            ConfigPath::SolidRoughness => {
                let v: f32 = value.try_into()?;
                config.solid_shading.roughness = if v.is_finite() { v.clamp(0.02, 1.0) } else { 0.5 };
            }
            ConfigPath::SolidMetallic => {
                let v: f32 = value.try_into()?;
                config.solid_shading.metallic = if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
            }
            ConfigPath::SolidRayTolerance => {
                let v: f32 = value.try_into()?;
                config.solid_shading.ray_tolerance = if v.is_finite() { v.clamp(1.0, 100.0) } else { 1.0 };
            }
            ConfigPath::SolidReflectance => {
                let v: f32 = value.try_into()?;
                config.solid_shading.reflectance = if v.is_finite() { v.clamp(0.0, 1.0) } else { 1.0 };
            }
            ConfigPath::SolidGlow => {
                let v: f32 = value.try_into()?;
                config.solid_shading.glow = if v.is_finite() { v.clamp(0.0, 16.0) } else { 0.0 };
            }
            ConfigPath::SsaoStrength => {
                config.solid_shading.ssao_strength = value.try_into()?;
            }
            ConfigPath::SsaoRadius => {
                config.solid_shading.ssao_radius = value.try_into()?;
            }
            ConfigPath::NormalSmoothing => {
                let v: f32 = value.try_into()?;
                config.solid_shading.normal_smoothing = (v.round().max(0.0) as u32).min(3);
            }
            ConfigPath::GapFill => {
                let v: f32 = value.try_into()?;
                config.solid_shading.gap_fill = (v.round().max(0.0) as u32).min(3);
            }
            ConfigPath::SolidLightEnabled { index } => {
                let l = config.solid_shading.lights.get_mut(*index).ok_or(ConfigError::InvalidIndex)?;
                l.enabled = value.try_into()?;
            }
            ConfigPath::SolidLightsFollowCamera => {
                config.solid_shading.lights_follow_camera = value.try_into()?;
            }
            ConfigPath::SolidLightParam { index, param } => {
                let l = config.solid_shading.lights.get_mut(*index).ok_or(ConfigError::InvalidIndex)?;
                if let Some(axis) = ["x", "y", "z"].iter().position(|a| a == param) {
                    // f64 from a Double (the panel, Place at camera), f32
                    // from a Float (a track's keyframe).
                    l.position[axis] = match value {
                        ConfigValue::Double(d) => d,
                        other => {
                            let v: f32 = other.try_into()?;
                            v as f64
                        }
                    };
                    return Ok(());
                }
                let v: f32 = value.try_into()?;
                crate::config::fractal_config::set_light_param(l, param, v).ok_or(ConfigError::InvalidIndex)?;
            }
            ConfigPath::FarDensityFadeStart => {
                config.far_density_fade_start = value.try_into()?;
            }

            // Effects
            ConfigPath::DensityEffectEnabled { index } => {
                let effect = config
                    .density_effects
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                effect.enabled = value.try_into()?;
            }
            ConfigPath::DensityEffectParam { index, param } => {
                let effect = config
                    .density_effects
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                effect.set_param(param, value.try_into()?);
            }
            ConfigPath::ColorEffectEnabled { index } => {
                let effect = config
                    .color_effects
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                effect.enabled = value.try_into()?;
            }
            // Its parameters stay: a parameter the new effect lacks is
            // ignored, one it adds reads its default.
            ConfigPath::ColorEffectType { index } => {
                let effect = config.color_effects.get_mut(*index).ok_or(ConfigError::InvalidIndex)?;
                effect.effect_type = value.try_into()?;
            }
            ConfigPath::DensityEffectType { index } => {
                let effect = config.density_effects.get_mut(*index).ok_or(ConfigError::InvalidIndex)?;
                effect.effect_type = value.try_into()?;
            }
            ConfigPath::ColorEffectParam { index, param } => {
                let effect = config
                    .color_effects
                    .get_mut(*index)
                    .ok_or(ConfigError::InvalidIndex)?;
                effect.set_param(param, value.try_into()?);
            }

            // Add/Remove effect operations
            ConfigPath::AddColorEffect { effect_type } => {
                use crate::effects::EffectInstance;
                config.color_effects.push(EffectInstance::new(effect_type));
            }
            ConfigPath::RemoveColorEffect { index } => {
                if *index >= config.color_effects.len() {
                    return Err(ConfigError::InvalidIndex);
                }
                config.color_effects.remove(*index);
            }
            ConfigPath::AddDensityEffect { effect_type } => {
                use crate::effects::EffectInstance;
                config.density_effects.push(EffectInstance::new(effect_type));
            }
            ConfigPath::RemoveDensityEffect { index } => {
                if *index >= config.density_effects.len() {
                    return Err(ConfigError::InvalidIndex);
                }
                config.density_effects.remove(*index);
            }

            // Xaos (chaos-weighted transform transitions) — per-flame
            ConfigPath::Xaos { src, dst } => {
                let weight: f32 = value.try_into()?;
                let src = *src;
                let dst = *dst;
                active_flame_mut(config, target)?.set_xaos(src, dst, weight);
            }

            // Solo transform (-1 = None, 0+ = Some(index)) — per-flame
            ConfigPath::SoloTransform => {
                let idx: i32 = value.try_into()?;
                active_flame_mut(config, target)?.solo_transform = if idx < 0 {
                    None
                } else {
                    Some(idx as usize)
                };
            }

            // Post-symmetry — per-flame plot-time symmetry. Type
            // round-trips through u32; unknown values clamp to None.
            ConfigPath::PostSymmetryType => {
                use crate::scene::transforms::PostSymmetryType;
                let raw: i32 = value.try_into()?;
                active_flame_mut(config, target)?.post_symmetry.ty = match raw {
                    1 => PostSymmetryType::XAxis,
                    2 => PostSymmetryType::YAxis,
                    3 => PostSymmetryType::Point,
                    _ => PostSymmetryType::None,
                };
            }
            ConfigPath::PostSymmetryOrder => {
                let raw: i32 = value.try_into()?;
                active_flame_mut(config, target)?.post_symmetry.order = raw.max(1).min(32) as u32;
            }
            ConfigPath::PostSymmetryCenterX => {
                let v: f32 = value.try_into()?;
                active_flame_mut(config, target)?.post_symmetry.center_x = v;
            }
            ConfigPath::PostSymmetryCenterY => {
                let v: f32 = value.try_into()?;
                active_flame_mut(config, target)?.post_symmetry.center_y = v;
            }
            ConfigPath::PostSymmetryDistance => {
                let v: f32 = value.try_into()?;
                active_flame_mut(config, target)?.post_symmetry.distance = v;
            }
            ConfigPath::PostSymmetryRotation => {
                let v: f32 = value.try_into()?;
                active_flame_mut(config, target)?.post_symmetry.rotation_deg = v;
            }
            ConfigPath::PreserveZ => {
                config.preserve_z = value.try_into()?;
            }
            ConfigPath::ResetOrbits => {
                config.reset_orbits = value.try_into()?;
            }
            // Within what an export can be: the GPU's texture limit is
            // the export panel's to enforce, against the pixel budget.
            ConfigPath::ImageWidth => {
                let v: u32 = value.try_into()?;
                config.image_size.0 = v.clamp(crate::config::MIN_IMAGE_DIM, crate::config::MAX_IMAGE_DIM);
            }
            ConfigPath::ImageHeight => {
                let v: u32 = value.try_into()?;
                config.image_size.1 = v.clamp(crate::config::MIN_IMAGE_DIM, crate::config::MAX_IMAGE_DIM);
            }

            // System Settings - These should NOT be called via apply_value (they're not in FractalConfig)
            // Use config_manager.update_system_setting() instead
            ConfigPath::SystemIterationsPerThread
            | ConfigPath::SystemVsyncEnabled
            | ConfigPath::SystemTargetFps
            | ConfigPath::SystemFlyMouseSensitivity
            | ConfigPath::SystemFlyMoveSpeed
            | ConfigPath::SystemFlySprintMultiplier
            | ConfigPath::SystemFlyInvertY
            | ConfigPath::SystemFlyCameraMode
            | ConfigPath::SystemFlyCollisions
            | ConfigPath::SystemFlyProximity
            | ConfigPath::SystemPngStripMetadata
            | ConfigPath::SystemLanguage
            | ConfigPath::SystemBurnIn
            | ConfigPath::SystemFrameGovernor
            | ConfigPath::SystemManualWorkgroups
            | ConfigPath::SystemOrbitCacheMb
            | ConfigPath::SystemShowHelpOnStartup => {
                panic!("System settings should not be modified via apply_value(). Use config_manager.update_system_setting() instead.");
            }
        }

        Ok(())
}

/// The flame `target` names in `config`.
fn target_flame_mut(config: &mut FractalConfig, target: EditingTarget) -> Option<&mut Flame> {
    match target {
        EditingTarget::Main => Some(&mut config.flame),
        EditingTarget::Subflame { index } => config.flame.subflames.get_mut(index),
    }
}

/// The flame being edited, or `InvalidIndex` for a subflame that is not there.
fn active_flame_mut(config: &mut FractalConfig, target: EditingTarget) -> Result<&mut Flame, ConfigError> {
    target_flame_mut(config, target).ok_or(ConfigError::InvalidIndex)
}

fn normal_transform_mut(config: &mut FractalConfig, target: EditingTarget, index: usize) -> Result<&mut Transform, ConfigError> {
    active_flame_mut(config, target)?.transforms.get_mut(index).ok_or(ConfigError::InvalidIndex)
}

fn linked_transform_mut(config: &mut FractalConfig, target: EditingTarget, index: usize) -> Result<&mut Transform, ConfigError> {
    active_flame_mut(config, target)?.linked_transforms.get_mut(index).ok_or(ConfigError::InvalidIndex)
}

fn final_transform_mut(config: &mut FractalConfig, target: EditingTarget, index: usize) -> Result<&mut Transform, ConfigError> {
    active_flame_mut(config, target)?.final_transforms.get_mut(index).ok_or(ConfigError::InvalidIndex)
}

/// Apply a variation-weight change to a single transform, regardless of
/// which pool it lives in. NaN is the sentinel for "remove this variation"
/// (sent by the trash button); 0.0 is kept so the slider stays visible.
fn apply_variation_weight(
    xform: &mut crate::scene::transforms::Transform,
    variation: &str,
    value: ConfigValue,
) -> Result<(), ConfigError> {
    let weight: f32 = value.try_into()?;
    if weight.is_nan() {
        // `remove_variation` also drops the name from `variation_order`.
        xform.remove_variation(variation);
    } else {
        // `set_variation` records the name in `variation_order` on first
        // add, so UI-built flames dispatch in add order.
        xform.set_variation(variation, weight);
    }
    Ok(())
}

/// Apply a variation fx_priority (phase) change to a single transform,
/// regardless of pool. Stored sparsely: a priority equal to the
/// variation def's natural-phase priority removes the override (the
/// `Any` default is main/0), any other value inserts it. See
/// `Transform::variation_priorities`.
fn apply_variation_priority(
    xform: &mut crate::scene::transforms::Transform,
    variation: &str,
    value: ConfigValue,
) -> Result<(), ConfigError> {
    let prio: i32 = value.try_into()?;
    let natural = crate::variations::global_registry()
        .get(variation)
        .map(|i| i.phase.natural_priority())
        .unwrap_or(0);
    if prio == natural {
        xform.variation_priorities.remove(variation);
    } else {
        xform.variation_priorities.insert(variation.to_string(), prio);
    }
    Ok(())
}

/// Apply a variation-parameter change to a single transform, regardless
/// of which pool it lives in.
fn apply_variation_param(
    xform: &mut crate::scene::transforms::Transform,
    variation: &str,
    param: &str,
    value: ConfigValue,
) -> Result<(), ConfigError> {
    let new_value: f32 = value.try_into()?;
    let key = format!("{}.{}", variation, param);
    xform.variation_params.insert(key, new_value);
    Ok(())
}

/// A non-finite value as 0, before its clamp: a track driven by a wild
/// signal must not poison a camera, a terrain or a lens with NaN.
fn finite_or_zero(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}

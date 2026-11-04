//! Entering a render mode: what a mode change, or a choice that takes
//! the config into another engine, implies for the config -- one undo
//! step through [`ConfigManager::choose`].
//!
//! The rules live here, in the config, rather than in the UI, so every
//! surface that switches a mode applies them alike. The four modes
//! (`RenderMode::ALL`) are peers, and this is the one place that moves
//! between them (the `nothing_outside_this_module_writes_the_render_mode`
//! scan holds it). What a mode means for the UI -- the panel it brings
//! forward, the engines kept resident, its labels -- is `ui::render_mode`.

use crate::config::delta::{ConfigPath, ConfigValue, UpdateType};
use crate::config::manager::{ConfigError, ConfigManager, EditingTarget};
use crate::config::FractalConfig;
use crate::scene::transforms::RenderMode;

/// Whether a config draws a lit 3D surface of the escape or the
/// simulation engine: a solid formula, or a terrain. A 3D flame is not
/// one -- its look is the flame's own.
pub fn surface_3d(config: &FractalConfig) -> bool {
    match config.render_mode {
        #[cfg(feature = "engine-escape")]
        RenderMode::Escape => crate::escape::ifs::formula_is_solid(&config.escape.formula) || config.escape.terrain_active(),
        #[cfg(feature = "engine-sim")]
        RenderMode::Simulation => cfg!(feature = "terrain") && config.sim.terrain_active(),
        _ => false,
    }
}

/// The 3D look (docs/archive/projects/3d-look-and-tools.md, P2), for a config
/// that `after` enters a lit 3D view `before` did not draw: the tone map
/// Linear at exposure 2 and gamma 1 -- the escape image is linear light
/// and the tone map encodes sRGB itself, so gamma 1 is the one correct
/// encode -- saturation 1 and no hue shift, none of which a flame's values
/// suit; and, when the lighting rig is untouched (a flame's default, no
/// lighting at all), the rig taken over ([`rig_taken_over`]). Nothing
/// when it enters no such view, or was in one already.
pub fn look_3d(before: &FractalConfig, after: &FractalConfig) -> Vec<(ConfigPath, ConfigValue)> {
    use crate::config::defaults::{DEFAULT_HUE_SHIFT, DEFAULT_SATURATION, ESCAPE_EXPOSURE, ESCAPE_GAMMA};
    if surface_3d(before) || !surface_3d(after) {
        return Vec::new();
    }
    let mut changes = vec![
        (ConfigPath::TonemapMode, crate::scene::tonemap::ToneMapMode::Linear.into()),
        (ConfigPath::Exposure, ESCAPE_EXPOSURE.into()),
        (ConfigPath::Gamma, ESCAPE_GAMMA.into()),
        (ConfigPath::Saturation, DEFAULT_SATURATION.into()),
        (ConfigPath::HueShift, DEFAULT_HUE_SHIFT.into()),
    ];
    if after.solid_shading.rig_untouched() {
        changes.extend(rig_taken_over(after));
    }
    // A new solid's lights are the fractal's, whatever an older file left.
    if after.solid_shading.lights_follow_camera {
        changes.push((ConfigPath::SolidLightsFollowCamera, false.into()));
    }
    changes
}

/// What an untouched lighting rig becomes when a lit 3D view takes it
/// over -- on entry (the 3D look) and at the Lighting panel's
/// first change of it: full strength and no ambient (3d-look-and-tools
/// P2), the rest as stored, and light 0 where the untouched rig's key
/// light was, so taking the rig over does not swing it: on a terrain the
/// sun (`DEFAULT_SUN`; light 0 ships at mode D's camera key light), on a
/// solid whose lights are the fractal's the world's key
/// (`ifs::default_world_key`).
pub fn rig_taken_over(config: &FractalConfig) -> Vec<(ConfigPath, ConfigValue)> {
    #[allow(unused_mut)]
    let mut changes = vec![(ConfigPath::ShadingStrength, 1.0f32.into()), (ConfigPath::SolidAmbient, 0.0f32.into())];
    #[allow(unused_variables)]
    let light = |param: &str| ConfigPath::SolidLightParam { index: 0, param: param.to_string() };
    #[cfg(feature = "engine-escape")]
    if crate::camera::gesture::view_kind(config) == crate::camera::gesture::ViewKind::Solid && !config.solid_shading.lights_follow_camera {
        let (azimuth, elevation) = crate::escape::ifs::default_world_key();
        changes.push((light("azimuth"), azimuth.into()));
        changes.push((light("elevation"), elevation.into()));
    }
    #[cfg(feature = "terrain")]
    {
        use crate::camera::gesture::{view_kind, ViewKind};
        if matches!(view_kind(config), ViewKind::EscapeTerrain | ViewKind::SimTerrain) {
            let (azimuth, elevation) = crate::escape::terrain::DEFAULT_SUN;
            changes.push((light("azimuth"), azimuth.into()));
            changes.push((light("elevation"), elevation.into()));
        }
    }
    #[cfg(not(any(feature = "terrain", feature = "engine-escape")))]
    let _ = config;
    changes
}

/// A solid's lights fixed in the fractal's space where they are now (the
/// View panel's Fly Mode section, for a solid saved while lights turned
/// with the camera): each directional light's angles turned from the
/// camera's space into the fractal's at this camera, and
/// `lights_follow_camera` off -- the picture as it stands, and from then on
/// the lights stay put as the camera moves. An untouched rig is taken
/// over first: its key light is light 0 as it ships.
#[cfg(feature = "engine-escape")]
pub fn lights_to_world(config: &FractalConfig) -> Vec<(ConfigPath, ConfigValue)> {
    let mut changes = Vec::new();
    if config.solid_shading.rig_untouched() {
        changes.extend(rig_taken_over(config));
    }
    for (index, l) in config.solid_shading.lights.iter().enumerate() {
        if l.kind != crate::config::LightKind::Directional {
            continue;
        }
        let (azimuth, elevation) = crate::escape::ifs::camera_light_to_world(&config.escape, l.azimuth, l.elevation);
        changes.push((ConfigPath::SolidLightParam { index, param: "azimuth".to_string() }, azimuth.into()));
        changes.push((ConfigPath::SolidLightParam { index, param: "elevation".to_string() }, elevation.into()));
    }
    changes.push((ConfigPath::SolidLightsFollowCamera, false.into()));
    changes
}

/// What else `changes` imply when they take `before` into `mode`: the
/// mode itself if it is another, and then the 3D look ([`look_3d`]) when
/// the result is a lit 3D view `before` did not draw, or else the tone
/// map's rescue (`entry_tone_mapping`: a flame's Log-calibrated mapping
/// renders a non-flame engine's unit-range image black). In `mode`
/// already, only the look the changes themselves bring.
///
/// For a caller that cannot go through [`ConfigManager::choose`] -- a
/// structural snapshot applies these to its `after` detached.
pub fn entering_changes(
    before: &FractalConfig,
    changes: &[(ConfigPath, ConfigValue)],
    mode: RenderMode,
) -> Vec<(ConfigPath, ConfigValue)> {
    let mut after = before.clone();
    for (path, value) in changes {
        let _ = ConfigManager::apply_value_detached(&mut after, EditingTarget::Main, path, value.clone());
    }
    implied(before, after, mode)
}

/// [`entering_changes`] for a choice made as a whole-config snapshot (a
/// layered simulation preset, a texture): what entering `mode` implies,
/// applied to `after` in place, so the snapshot is still one undo step.
pub fn enter_snapshot(before: &FractalConfig, after: &mut FractalConfig, mode: RenderMode) {
    for (path, value) in implied(before, after.clone(), mode) {
        let _ = ConfigManager::apply_value_detached(after, EditingTarget::Main, &path, value);
    }
}

/// What entering `mode` adds to `after`, which is `before` with a choice
/// made in it.
fn implied(before: &FractalConfig, mut after: FractalConfig, mode: RenderMode) -> Vec<(ConfigPath, ConfigValue)> {
    let entering = after.render_mode != mode;
    let mut out = Vec::new();
    if entering {
        out.push((ConfigPath::RenderMode, mode.into()));
        let _ = ConfigManager::apply_value_detached(&mut after, EditingTarget::Main, &ConfigPath::RenderMode, mode.into());
    }
    let look = look_3d(before, &after);
    if !look.is_empty() {
        out.extend(look);
    } else if entering {
        // Escape output has its own calibration; a simulation keeps the
        // flame defaults (`entry_tone_mapping`, shared with the script
        // handles).
        if let Some((exposure, gamma)) = crate::config::defaults::entry_tone_mapping(before, mode) {
            out.push((ConfigPath::TonemapMode, crate::scene::tonemap::ToneMapMode::Linear.into()));
            out.push((ConfigPath::Exposure, exposure.into()));
            out.push((ConfigPath::Gamma, gamma.into()));
        }
    }
    out
}

impl ConfigManager {
    /// `changes` made in `mode`, as one undo step: choosing something for
    /// an engine takes the config into its mode (one-workspace plan,
    /// decision 4) -- a formula picked, a Rhai entry put to use, a
    /// simulation model, a texture -- with what entering implies
    /// ([`entering_changes`]). In the mode already, the changes with the
    /// look they bring.
    pub fn choose(
        &mut self,
        mut changes: Vec<(ConfigPath, ConfigValue)>,
        description: &str,
        mode: RenderMode,
    ) -> Result<UpdateType, ConfigError> {
        let extra = entering_changes(self.active_config(), &changes, mode);
        changes.extend(extra);
        if changes.is_empty() {
            return Ok(UpdateType::None);
        }
        self.update_batch(changes, description.to_string())
    }

    /// Change the render mode, as one undo step; to the mode already
    /// shown, nothing (no history entry).
    pub fn switch_mode(&mut self, mode: RenderMode) -> Result<(), ConfigError> {
        self.choose(Vec::new(), "history.param.render_mode", mode).map(|_| ())
    }
}

//! Entering a render mode: what a mode change, or a choice that takes
//! the config into another engine, does to the config -- one undo step
//! through [`ConfigManager::choose`].
//!
//! **Each engine owns its settings.** The palette, the tone map, the
//! background, the effects and the lighting rig work differently for a
//! flame, an escape-time picture and a simulation, so nothing of them is
//! carried from one engine to another (the user, 2026-11-04: "There's
//! really no reason coloring/tonemapping/lighting or any other settings
//! should be copied between modes."). The config's own fields hold the
//! shown engine's; the others wait in `FractalConfig::engine_settings`,
//! in the config's own JSON. Leaving an engine puts its settings there;
//! entering one takes its own back -- or, the first time, its defaults,
//! never a copy. 2D and 3D flames are one engine and share them.
//!
//! The stash is config state like any other, so the old objections to
//! restoring on the way out do not hold: undo and redo move it with the
//! mode (the switch is one whole-config step), a saved file keeps each
//! engine's settings, and a loaded fractal brings its own. What the
//! flame alone reads (its camera, its iteration settings) is the flame's
//! already; the picture's frame (`image_size`) and `deterministic_rng`
//! stay shared. Every field of `FractalConfig` is in one of these
//! classes (`every_config_field_is_classified`).
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
        RenderMode::Escape => crate::escape::ifs::formula_is_solid(&config.escape.formula) || config.escape.terrain_active(),
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
    if surface_3d(before) || !surface_3d(after) {
        return Vec::new();
    }
    look_3d_changes(after)
}

/// The 3D look's changes for `after`, whatever it came from.
fn look_3d_changes(after: &FractalConfig) -> Vec<(ConfigPath, ConfigValue)> {
    use crate::config::defaults::{DEFAULT_HUE_SHIFT, DEFAULT_SATURATION, ESCAPE_EXPOSURE, ESCAPE_GAMMA};
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
    let mut changes = vec![(ConfigPath::ShadingStrength, 1.0f32.into()), (ConfigPath::SolidAmbient, 0.0f32.into())];
    let light = |param: &str| ConfigPath::SolidLightParam { index: 0, param: param.to_string() };
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
    changes
}

/// A solid's lights fixed in the fractal's space where they are now (the
/// View panel's Fly Mode section, for a solid saved while lights turned
/// with the camera): each directional light's angles turned from the
/// camera's space into the fractal's at this camera, and
/// `lights_follow_camera` off -- the picture as it stands, and from then on
/// the lights stay put as the camera moves. An untouched rig is taken
/// over first: its key light is light 0 as it ships.
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

// ---------------------------------------------------------------- engines

/// The three engines. 2D and 3D flames are one: the same chaos game,
/// one palette and tone map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    Flame,
    Escape,
    Simulation,
}

impl Engine {
    pub fn of(mode: RenderMode) -> Self {
        match mode {
            RenderMode::TwoD | RenderMode::ThreeD => Engine::Flame,
            RenderMode::Escape => Engine::Escape,
            RenderMode::Simulation => Engine::Simulation,
        }
    }

    /// Its key in `FractalConfig::engine_settings`.
    pub fn key(self) -> &'static str {
        match self {
            Engine::Flame => "flame",
            Engine::Escape => "escape",
            Engine::Simulation => "simulation",
        }
    }
}

/// A class of `FractalConfig` fields, listed once: their names, and a
/// copy between two configs (all of them, or one by name).
macro_rules! field_class {
    ($(#[$doc:meta])* $list:ident, $copy:ident: $($field:ident),* $(,)?) => {
        $(#[$doc])*
        pub const $list: &[&str] = &[$(stringify!($field)),*];

        fn $copy(from: &FractalConfig, to: &mut FractalConfig, only: Option<&str>) {
            $(if only.is_none_or(|name| name == stringify!($field)) {
                to.$field = from.$field.clone();
            })*
        }
    };
}

field_class!(
    /// The settings each engine keeps its own of: the palette and its
    /// settings, the background, the tone map, the effects (and the Rhai
    /// effects they carry), the lighting rig.
    ENGINE_OWNED, copy_owned:
    palette, palette_rotation, palette_size, palette_squeeze, palette_squeeze_mode,
    palette_squeeze_falloff, palette_log_strength, palette_reverse, background_color,
    tonemap_mode, highlight_mode, tonemap_curve, use_curve, exposure, gamma, gamma_threshold,
    brightness, vibrancy, white_level, saturation, hue_shift, alpha_blend_low, alpha_blend_high,
    levels_enabled, levels_low, levels_high, levels_gamma, auto_exposure,
    density_effects, color_effects, custom_effects, solid_shading,
);

field_class!(
    /// What the flame alone reads -- its camera, projection, depth,
    /// filter, iteration, colour-mode and Focused Rendering settings. The
    /// flame's already, so a switch leaves them where they are; a flame
    /// seeded into an empty config brings its own ([`seed_flame`]).
    FLAME_ONLY, copy_flame_only:
    preserve_z, reset_orbits, zoom, pan_x, pan_y, rotation, camera_rotation_x,
    camera_rotation_y, camera_bank, camera_x, camera_y, camera_z, perspective_strength,
    dof_focus_distance, dof_blur_strength, fog_strength, fog_start, depth_density_compensation,
    far_density_fade, far_density_fade_start, solid_strength, surface_thickness, filter_radius,
    filter_blur_edges, density_scale, speed_factor, max_iterations, blend_factor,
    use_dynamic_blend, color_mode, path_map_style, path_map_level, importance,
    cylinder_targeting, cylinder_always, cylinder_trim, cylinder_trim_levels, word_removals,
);

/// What every engine shares: the picture's frame and its reproducibility.
pub const SHARED: &[&str] = &["image_size", "deterministic_rng"];

/// The engines' own blocks, the mode, and the stash.
pub const STRUCTURE: &[&str] = &["flame", "render_mode", "escape", "sim", "engine_settings", "unread"];

/// A config's engine-owned settings, as its JSON (a field at its default
/// is absent, as in a file).
fn owned_json(config: &FractalConfig) -> serde_json::Map<String, serde_json::Value> {
    match serde_json::to_value(config) {
        Ok(serde_json::Value::Object(o)) => o.into_iter().filter(|(k, _)| ENGINE_OWNED.contains(&k.as_str())).collect(),
        _ => serde_json::Map::new(),
    }
}

/// `config`'s engine-owned settings set from `owned` (as [`owned_json`]
/// wrote them), read through `FractalConfig`'s own serde, so an absent
/// field takes the default a file's would. False if they do not read.
fn restore_owned(config: &mut FractalConfig, owned: &serde_json::Map<String, serde_json::Value>) -> bool {
    let Ok(serde_json::Value::Object(mut carrier)) = serde_json::to_value(FractalConfig::default()) else {
        return false;
    };
    for k in ENGINE_OWNED {
        carrier.remove(*k);
    }
    carrier.extend(owned.iter().map(|(k, v)| (k.clone(), v.clone())));
    match serde_json::from_value::<FractalConfig>(serde_json::Value::Object(carrier)) {
        Ok(read) => {
            copy_owned(&read, config, None);
            true
        }
        Err(e) => {
            log::warn!("an engine's stashed settings did not read ({e}); it starts from its defaults");
            false
        }
    }
}

/// An engine's settings the first time a config enters it: the defaults,
/// with the tone map the engine's picture needs -- an escape-time or
/// simulation image is unit-range linear light, which a flame's Log
/// mapping renders black; escape-time has its own calibration.
fn first_entry(config: &mut FractalConfig, engine: Engine) {
    use crate::config::defaults::{DEFAULT_EXPOSURE, DEFAULT_GAMMA, ESCAPE_EXPOSURE, ESCAPE_GAMMA};
    let mut defaults = FractalConfig::default();
    match engine {
        Engine::Flame => {}
        Engine::Escape => {
            defaults.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
            defaults.exposure = ESCAPE_EXPOSURE;
            defaults.gamma = ESCAPE_GAMMA;
        }
        Engine::Simulation => {
            defaults.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
            defaults.exposure = DEFAULT_EXPOSURE;
            defaults.gamma = DEFAULT_GAMMA;
        }
    }
    copy_owned(&defaults, config, None);
}

/// Whether `config` draws its flame: a flame mode, a formula drawing the
/// flame (the Flame Attractor, `IfsDef::needs_flame`), or a simulation
/// whose layers the flame's transforms warp.
pub fn needs_flame(config: &FractalConfig) -> bool {
    match config.render_mode {
        RenderMode::TwoD | RenderMode::ThreeD => true,
        RenderMode::Escape => crate::escape::ifs::get_ifs(&config.escape.formula).is_some_and(|d| d.needs_flame),
        RenderMode::Simulation => config.sim.use_transforms,
    }
}

/// Whether entering `mode` (or choosing in it) must give `after` a flame:
/// it draws one, and has none.
fn wants_seed(after: &FractalConfig, mode: RenderMode) -> bool {
    after.flame.transforms.is_empty() && needs_flame(&FractalConfig { render_mode: mode, ..after.clone() })
}

/// An empty flame, where one is drawn, given the app's own first flame
/// (the first preset, the one it starts on): its transforms, its camera
/// and iteration settings, and -- when `owned`, the flame engine entered
/// for the first time -- its palette and tone map, so it looks as it was
/// made to. A config made in another engine (a simulation, a script's
/// escape picture) has an empty flame, and a flame mode, the Flame
/// Attractor or a warp by its transforms drew nothing from it; every
/// editing path that assumes a transform then had none to edit.
pub fn seed_flame(config: &mut FractalConfig, owned: bool) {
    let Some(preset) = crate::resources::presets::load_embedded_presets().ok().and_then(|p| p.into_iter().next()) else {
        log::warn!("no preset to seed an empty flame with");
        return;
    };
    config.flame = preset.flame.clone();
    copy_flame_only(&preset, config, None);
    if owned {
        copy_owned(&preset, config, None);
    }
}

/// `after` -- `before` with a choice made in it -- entered into `mode`:
/// the mode; when that is another engine, the one left stashed and the
/// one entered restored (or its defaults), what the choice itself set
/// kept over them; an empty flame seeded where one is drawn
/// ([`seed_flame`]); and the 3D look when the result is a lit 3D view the
/// engine was not showing ([`look_3d`]).
pub fn enter(before: &FractalConfig, after: &mut FractalConfig, mode: RenderMode) {
    let (from, to) = (Engine::of(before.render_mode), Engine::of(mode));
    let mut restored = false;
    let seed = wants_seed(after, mode);
    if from != to {
        // What the choice set of the engine-owned settings wins over the
        // entered engine's: a preset's palette, say.
        let set_by_choice: Vec<&str> = {
            let (was, now) = (owned_json(before), owned_json(after));
            ENGINE_OWNED.iter().copied().filter(|k| was.get(*k) != now.get(*k)).collect()
        };
        let chosen = after.clone();
        after.engine_settings.insert(from.key().to_string(), owned_json(before));
        if let Some(own) = after.engine_settings.remove(to.key()) {
            restored = restore_owned(after, &own);
        }
        if !restored {
            first_entry(after, to);
        }
        if seed {
            seed_flame(after, to == Engine::Flame && !restored);
        }
        for field in set_by_choice {
            copy_owned(&chosen, after, Some(field));
        }
    } else if seed {
        seed_flame(after, false);
    }
    after.render_mode = mode;
    // The 3D look on entering a lit view the engine was not showing: in
    // the engine already, as `before` drew it; an engine restored, as it
    // was left (its block is as it was); a first entry, from nothing.
    let was_lit = if from == to {
        surface_3d(before)
    } else if restored {
        surface_3d(&FractalConfig { render_mode: mode, ..before.clone() })
    } else {
        false
    };
    if !was_lit && surface_3d(after) {
        for (path, value) in look_3d_changes(after) {
            let _ = crate::config::apply(after, EditingTarget::Main, &path, value);
        }
    }
}

/// [`enter`] for a choice made as a whole-config snapshot (a layered
/// simulation preset, a texture): `after` entered into `mode` in place,
/// so the snapshot is still one undo step.
pub fn enter_snapshot(before: &FractalConfig, after: &mut FractalConfig, mode: RenderMode) {
    enter(before, after, mode);
}

impl ConfigManager {
    /// `changes` made in `mode`, as one undo step: choosing something for
    /// an engine takes the config into its mode (one-workspace plan,
    /// decision 4) -- a formula picked, a Rhai entry put to use, a
    /// simulation model, a texture -- with what entering implies
    /// ([`enter`]). In the mode already, the changes with the 3D look
    /// they bring.
    pub fn choose(
        &mut self,
        mut changes: Vec<(ConfigPath, ConfigValue)>,
        description: &str,
        mode: RenderMode,
    ) -> Result<UpdateType, ConfigError> {
        let before = self.active_config().clone();
        // The mode shown, chosen again with nothing else: no step at all
        // (a menu row for the active mode costs no undo entry), a flame to
        // seed included -- seeding comes with a real change.
        if changes.is_empty() && before.render_mode == mode {
            return Ok(UpdateType::None);
        }
        let mut after = before.clone();
        for (path, value) in &changes {
            let _ = crate::config::apply(&mut after, EditingTarget::Main, path, value.clone());
        }
        if before.render_mode == mode && !wants_seed(&after, mode) {
            changes.extend(look_3d(&before, &after));
            if changes.is_empty() {
                return Ok(UpdateType::None);
            }
            return self.update_batch(changes, description.to_string());
        }
        // Another mode, or a flame to seed: a whole-config step, so the
        // engines' settings and the flame move with it through undo and
        // redo.
        enter(&before, &mut after, mode);
        self.apply_config_edit(after, description.to_string())?;
        Ok(UpdateType::IterationReset)
    }

    /// Change the render mode, as one undo step; to the mode already
    /// shown, nothing (no history entry).
    pub fn switch_mode(&mut self, mode: RenderMode) -> Result<(), ConfigError> {
        self.choose(Vec::new(), "history.param.render_mode", mode).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::defaults::{ESCAPE_EXPOSURE, ESCAPE_GAMMA};
    use crate::scene::tonemap::ToneMapMode;

    /// A config in `mode` with a one-transform flame, so no switch seeds
    /// one (`a_flame_is_seeded_where_one_is_drawn` covers that).
    fn in_mode(mode: RenderMode) -> FractalConfig {
        FractalConfig { render_mode: mode, ..crate::resources::presets::create_default_preset() }
    }

    /// A config in `mode` whose flame is empty, as a simulation's or a
    /// script-built escape picture's is.
    fn empty_flame_in(mode: RenderMode) -> FractalConfig {
        FractalConfig { render_mode: mode, ..FractalConfig::default() }
    }

    /// `c` with its engine's settings off their defaults, every kind of
    /// them: the tone map, the palette, the background, the effects, the
    /// lighting.
    fn styled(mut c: FractalConfig, n: f32) -> FractalConfig {
        c.exposure = 1.0 + n;
        c.vibrancy = 0.1 * n;
        c.saturation = 1.0 + 0.1 * n;
        c.background_color = [0.1 * n, 0.0, 0.0];
        c.palette_rotation = 0.05 * n;
        c.color_effects = vec![crate::effects::EffectInstance::new("vignette")];
        c.solid_shading.ambient = 0.1 * n;
        c
    }

    /// What a first entry into `mode`'s engine gives.
    fn first_entry_of(mode: RenderMode) -> serde_json::Map<String, serde_json::Value> {
        let mut c = in_mode(mode);
        first_entry(&mut c, Engine::of(mode));
        owned_json(&c)
    }

    /// The user's case: each engine keeps its own palette, tone map,
    /// effects and lighting, and gets them back on returning -- nothing of
    /// one engine is copied into another.
    #[test]
    fn each_engine_keeps_its_own_settings() {
        let mut m = ConfigManager::new(styled(in_mode(RenderMode::TwoD), 3.0));
        let flame = owned_json(m.active_config());
        m.switch_mode(RenderMode::Escape).expect("to escape");
        let c = m.active_config();
        assert_eq!((c.tonemap_mode, c.exposure, c.gamma), (ToneMapMode::Linear, ESCAPE_EXPOSURE, ESCAPE_GAMMA), "escape's own tone map");
        assert!(c.color_effects.is_empty(), "the flame's effects were copied");
        assert_eq!(owned_json(c), first_entry_of(RenderMode::Escape), "a first entry is the engine's defaults");
        m.update_param(ConfigPath::Exposure, 5.0f32.into()).expect("exposure");
        let escape = owned_json(m.active_config());
        m.switch_mode(RenderMode::Simulation).expect("to sim");
        assert_eq!(owned_json(m.active_config()), first_entry_of(RenderMode::Simulation), "nor the escape's into the simulation");
        m.switch_mode(RenderMode::TwoD).expect("back to the flame");
        assert_eq!(owned_json(m.active_config()), flame, "the flame's own back");
        m.switch_mode(RenderMode::Escape).expect("back to escape");
        assert_eq!(owned_json(m.active_config()), escape, "and the escape's");
        let keys: Vec<&String> = m.active_config().engine_settings.keys().collect();
        assert_eq!(keys, ["flame", "simulation"], "the engines not shown wait in the stash");
    }

    /// 2D and 3D flames are one engine: a switch between them moves no
    /// setting and stashes nothing.
    #[test]
    fn two_d_and_three_d_share_the_flames_settings() {
        let mut m = ConfigManager::new(styled(in_mode(RenderMode::TwoD), 2.0));
        let flame = owned_json(m.active_config());
        m.switch_mode(RenderMode::ThreeD).expect("to 3d");
        assert_eq!(owned_json(m.active_config()), flame);
        assert!(m.active_config().engine_settings.is_empty());
    }

    /// A switch is one undo step: undo takes the mode and every engine's
    /// settings back, redo brings them forward again.
    #[test]
    fn a_switch_undoes_and_redoes_whole() {
        let mut m = ConfigManager::new(styled(in_mode(RenderMode::TwoD), 1.0));
        let flame = owned_json(m.active_config());
        let before = m.history_len();
        m.switch_mode(RenderMode::Escape).expect("switch");
        assert_eq!(m.history_len() - before, 1, "one undo step");
        m.undo().expect("undo");
        let c = m.active_config();
        assert_eq!((c.render_mode, owned_json(c)), (RenderMode::TwoD, flame.clone()));
        assert!(c.engine_settings.is_empty(), "the stash went back too");
        m.redo().expect("redo");
        let c = m.active_config();
        assert_eq!((c.render_mode, owned_json(c)), (RenderMode::Escape, first_entry_of(RenderMode::Escape)));
        assert_eq!(c.engine_settings.get("flame"), Some(&flame));
    }

    /// The stash is part of the file: a config saved in one engine and
    /// loaded gives the other engines their own settings back.
    #[test]
    fn the_engines_settings_travel_with_the_file() {
        let mut m = ConfigManager::new(styled(in_mode(RenderMode::TwoD), 4.0));
        let flame = owned_json(m.active_config());
        m.switch_mode(RenderMode::Simulation).expect("switch");
        let loaded = FractalConfig::from_json(&m.active_config().to_json().expect("save")).expect("load");
        let mut m = ConfigManager::new(loaded);
        m.switch_mode(RenderMode::TwoD).expect("back");
        assert_eq!(owned_json(m.active_config()), flame);
    }

    /// Every pair of engines: a first entry is the entered engine's
    /// defaults, whatever the engine left was wearing.
    #[test]
    fn a_first_entry_copies_nothing() {
        for from in RenderMode::ALL {
            for to in RenderMode::ALL {
                if Engine::of(*from) == Engine::of(*to) {
                    continue;
                }
                let mut m = ConfigManager::new(styled(in_mode(*from), 5.0));
                m.switch_mode(*to).expect("switch");
                assert_eq!(owned_json(m.active_config()), first_entry_of(*to), "{from:?} -> {to:?}");
            }
        }
    }

    /// A choice that takes the config into another engine keeps what the
    /// choice itself set (a preset's exposure), the entered engine's own
    /// for the rest.
    #[test]
    fn a_choice_across_engines_keeps_what_it_set() {
        let mut m = ConfigManager::new(styled(in_mode(RenderMode::TwoD), 3.0));
        m.choose(
            vec![(ConfigPath::Exposure, 7.0f32.into()), (ConfigPath::EscapeFormula, ConfigValue::String("burning_ship".into()))],
            "history.action.escape_preset",
            RenderMode::Escape,
        )
        .expect("choose");
        let c = m.active_config();
        assert_eq!((c.render_mode, c.escape.formula.as_str(), c.exposure), (RenderMode::Escape, "burning_ship", 7.0));
        assert_eq!(c.vibrancy, FractalConfig::default().vibrancy, "the rest is escape's own");
        assert!(c.color_effects.is_empty());
    }

    /// The 3D look is for a lit view the engine was not showing: a solid
    /// entered for the first time gets it; a solid left and returned to
    /// keeps the look it was given since.
    #[test]
    fn a_lit_view_gets_the_3d_look_once() {
        let mut flame = in_mode(RenderMode::TwoD);
        flame.escape.formula = "mandelbulb_solid".to_string();
        let mut m = ConfigManager::new(flame);
        m.switch_mode(RenderMode::Escape).expect("first entry");
        let c = m.active_config();
        assert_eq!((c.tonemap_mode, c.exposure, c.solid_shading.shading_strength), (ToneMapMode::Linear, ESCAPE_EXPOSURE, 1.0), "the 3D look, rig taken over");
        m.update_param(ConfigPath::Exposure, 3.0f32.into()).expect("tuned");
        m.switch_mode(RenderMode::TwoD).expect("away");
        m.switch_mode(RenderMode::Escape).expect("back");
        assert_eq!(m.active_config().exposure, 3.0, "a returned solid keeps its own look");
    }

    /// Every field of `FractalConfig` is in exactly one class: owned by
    /// each engine, shared, the flame's alone, or structure. A new field
    /// must say which, or it would be silently copied between engines (or
    /// silently not). A source scan, as the render mode's writer scan is.
    #[test]
    fn every_config_field_is_classified() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/config/fractal_config.rs");
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("no source tree; skipping");
            return;
        };
        let body = &text[text.find("pub struct FractalConfig {").expect("the struct")..];
        let body = &body[..body.find("\n}").expect("its end")];
        let fields: Vec<&str> = body
            .lines()
            .filter_map(|l| l.trim().strip_prefix("pub ").and_then(|r| r.split(':').next()))
            .filter(|f| f.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
            .collect();
        assert!(fields.len() > 60, "the scan found {} fields", fields.len());
        let classes = [ENGINE_OWNED, SHARED, FLAME_ONLY, STRUCTURE];
        for f in &fields {
            let n = classes.iter().filter(|c| c.contains(f)).count();
            assert_eq!(n, 1, "`{f}` is in {n} classes of config::mode, not one");
        }
        for class in classes {
            for f in class {
                assert!(fields.contains(f), "`{f}` is classified but is no field of FractalConfig");
            }
        }
    }
    /// Where the config draws its flame and has none, it is given the
    /// app's first flame in the same undo step: a flame mode entered from
    /// a simulation (with the preset's look, the flame engine's first
    /// entry), the Flame Attractor chosen in escape-time (escape keeping
    /// its own look). A flame that has transforms is never replaced.
    #[test]
    fn a_flame_is_seeded_where_one_is_drawn() {
        let preset = crate::resources::presets::load_embedded_presets().expect("presets").remove(0);
        assert!(!preset.flame.transforms.is_empty());

        let mut m = ConfigManager::new(empty_flame_in(RenderMode::Simulation));
        m.switch_mode(RenderMode::TwoD).expect("to the flame");
        let c = m.active_config();
        assert_eq!(c.flame.transforms.len(), preset.flame.transforms.len(), "seeded");
        assert_eq!((c.zoom, owned_json(c)), (preset.zoom, owned_json(&preset)), "the preset's view and look");
        m.undo().expect("undo");
        assert!(m.active_config().flame.transforms.is_empty(), "one undo step");
        assert_eq!(m.active_config().render_mode, RenderMode::Simulation);

        // A flame with transforms stays.
        let mut m = ConfigManager::new(in_mode(RenderMode::Simulation));
        let flame = serde_json::to_value(&m.active_config().flame).expect("flame");
        m.switch_mode(RenderMode::TwoD).expect("to the flame");
        assert_eq!(serde_json::to_value(&m.active_config().flame).expect("flame"), flame);

        // The Flame Attractor, chosen in escape-time.
        {
            let mut escape = empty_flame_in(RenderMode::Escape);
            escape.exposure = 3.0;
            let mut m = ConfigManager::new(escape);
            m.choose(vec![(ConfigPath::EscapeFormula, ConfigValue::String("ifs_flame".into()))], "history.param.escape_formula", RenderMode::Escape)
                .expect("choose");
            let c = m.active_config();
            assert_eq!((c.escape.formula.as_str(), c.flame.transforms.len()), ("ifs_flame", preset.flame.transforms.len()));
            assert_eq!(c.exposure, 3.0, "escape keeps its own look");
        }

        // A simulation warped by the flame's transforms.
        {
            let mut m = ConfigManager::new(empty_flame_in(RenderMode::Simulation));
            m.choose(vec![(ConfigPath::SimUseTransforms, true.into())], "history.param.sim_use_transforms", RenderMode::Simulation)
                .expect("choose");
            assert_eq!(m.active_config().flame.transforms.len(), preset.flame.transforms.len());
        }
    }
}

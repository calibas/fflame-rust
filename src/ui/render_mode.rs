//! What a render mode means for the UI: the panel it brings forward,
//! the engines kept resident, its labels and tips.
//!
//! What entering a mode does to the CONFIG -- the mode itself, the 3D
//! look, the tone map -- is `config::mode`, with the switch itself a
//! `ConfigManager` operation (`choose`, `switch_mode`), so every surface
//! applies the same rules. The wrappers below keep the panels' calls.
//! Before one place wrote the mode, five sites did without sharing code,
//! and only two reset the tone mapping (docs/archive/projects/ui-render-modes.md
//! section 3.1).

use crate::config::delta::{ConfigPath, ConfigValue, UpdateType};
use crate::config::manager::{ConfigError, ConfigManager};
use crate::scene::transforms::RenderMode;

pub use crate::config::mode::{enter_snapshot, rig_taken_over};
#[cfg(feature = "engine-escape")]
pub use crate::config::mode::lights_to_world;

/// `changes` as one undo step, with the 3D look when they take the
/// config into a lit 3D view: the formula picker choosing a solid, a
/// terrain switched on, a Rhai solid put to use.
pub fn apply_with_look(
    config_manager: &mut ConfigManager,
    changes: Vec<(ConfigPath, ConfigValue)>,
    description: &str,
) -> Result<UpdateType, ConfigError> {
    let mode = config_manager.active_config().render_mode;
    config_manager.choose(changes, description, mode)
}

/// `changes` made in `mode`, as one undo step (`ConfigManager::choose`).
pub fn choose(
    config_manager: &mut ConfigManager,
    changes: Vec<(ConfigPath, ConfigValue)>,
    description: &str,
    mode: RenderMode,
) -> Result<UpdateType, ConfigError> {
    config_manager.choose(changes, description, mode)
}

/// Change the render mode (`ConfigManager::switch_mode`).
pub fn switch_render_mode(
    config_manager: &mut ConfigManager,
    mode: RenderMode,
) -> Result<(), ConfigError> {
    config_manager.switch_mode(mode)
}

/// The panel that edits a mode's picture, which a change into the mode
/// brings to the front (one-workspace plan): one workspace holds all
/// three, as tabs, and a mode change closes and swaps nothing.
pub fn main_panel_for(mode: RenderMode) -> super::workspace::PanelType {
    use super::workspace::PanelType;
    match mode {
        RenderMode::TwoD | RenderMode::ThreeD => PanelType::Transforms,
        RenderMode::Escape => PanelType::Escape,
        RenderMode::Simulation => PanelType::Simulation,
    }
}

/// Does this mode need the escape engine's GPU state resident?
///
/// Both non-flame renderers are created lazily and, before this, were
/// dropped only on device loss or before a synchronous high-res
/// export -- so leaving a mode left everything allocated. That was
/// tolerable while switching was buried inside two panel buttons; a
/// Mode menu invites it constantly (ui-render-modes plan, section 2,
/// decision 4).
pub fn keeps_escape_engine(mode: RenderMode) -> bool {
    matches!(mode, RenderMode::Escape)
}

/// The same for the simulation's grid.
///
/// Note what this costs, which the escape case does not: the escape
/// renderer rebuilds itself deterministically from the config, so
/// returning costs only time, but the simulation's grid IS its state.
/// Freeing it means returning restarts from the seed at step 0.
pub fn keeps_sim_engine(mode: RenderMode) -> bool {
    matches!(mode, RenderMode::Simulation)
}

/// Does the spacebar drive the simulation's transport in this mode?
///
/// Everywhere else it keeps the video-editor convention and plays or
/// pauses the animation. In Simulation the transport is the control
/// you reach for constantly -- the grid is always mid-run -- and the
/// animation is the rarer thing.
///
/// Typing safety needs no guard here: the app only sees a key that
/// egui did not consume, and egui consumes keyboard input whenever any
/// widget has focus, so a space typed into one of the panel's numeric
/// fields never reaches the shortcut.
pub fn space_runs_the_simulation(mode: RenderMode) -> bool {
    matches!(mode, RenderMode::Simulation)
}

/// The i18n key describing what a mode renders, for a menu hover.
pub fn mode_tip_key(mode: RenderMode) -> &'static str {
    match mode {
        RenderMode::TwoD => "mode.two_d_tip",
        RenderMode::ThreeD => "mode.three_d_tip",
        RenderMode::Escape => "mode.escape_tip",
        RenderMode::Simulation => "mode.simulation_tip",
    }
}

/// The i18n key naming a mode, for a picker or a menu row.
pub fn mode_label_key(mode: RenderMode) -> &'static str {
    match mode {
        RenderMode::TwoD => "mode.two_d",
        RenderMode::ThreeD => "mode.three_d",
        RenderMode::Escape => "mode.escape",
        RenderMode::Simulation => "mode.simulation",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::config::mode::{look_3d, surface_3d};
    use crate::config::FractalConfig;
    use crate::scene::tonemap::ToneMapMode;

    /// A solid's lights on entry (2026-10-19): the 3D look leaves them the
    /// fractal's whatever an older file left; an untouched rig taken over
    /// keeps its key light where it was -- the world's key when the lights
    /// are the fractal's, light 0 as it ships when they follow the camera.
    #[cfg(feature = "engine-escape")]
    #[test]
    fn a_solid_enters_with_its_lights_in_the_fractal() {
        let mut solid = FractalConfig::default();
        solid.render_mode = RenderMode::Escape;
        solid.escape.formula = "mandelbulb_solid".to_string();
        let mut plane = solid.clone();
        plane.escape.formula = "mandelbrot".to_string();
        let mut old = solid.clone();
        old.solid_shading.lights_follow_camera = true;
        assert!(look_3d(&plane, &old).contains(&(ConfigPath::SolidLightsFollowCamera, false.into())), "entering: the fractal's");
        assert!(!look_3d(&plane, &solid).iter().any(|(p, _)| *p == ConfigPath::SolidLightsFollowCamera), "already the fractal's");
        let light = |param: &str| ConfigPath::SolidLightParam { index: 0, param: param.to_string() };
        let (azimuth, elevation) = crate::escape::ifs::default_world_key();
        let taken = rig_taken_over(&solid);
        assert!(taken.contains(&(light("azimuth"), azimuth.into())) && taken.contains(&(light("elevation"), elevation.into())));
        assert!(!rig_taken_over(&old).iter().any(|(p, _)| matches!(p, ConfigPath::SolidLightParam { .. })), "the camera's key stays");
    }

    fn manager_in(mode: RenderMode) -> ConfigManager {
        let mut config = FractalConfig::default();
        config.render_mode = mode;
        ConfigManager::new(config)
    }

    /// Entering a lit 3D view gives it the 3D look (3d-look-and-tools P2),
    /// one undo step: by the mode switch with a solid formula, by the
    /// formula picker, by a terrain switched on. A touched rig is the
    /// user's and stays; a view already 3D keeps what it has.
    #[cfg(all(feature = "engine-escape", feature = "terrain"))]
    #[test]
    fn entering_a_3d_view_gives_it_the_3d_look() {
        use crate::config::ConfigValue;
        let looks_3d = |c: &FractalConfig, rig: bool| {
            c.tonemap_mode == ToneMapMode::Linear
                && c.exposure == 2.0
                && c.gamma == 1.0
                && c.saturation == 1.0
                && c.hue_shift == 0.0
                && (!rig || (c.solid_shading.shading_strength == 1.0 && c.solid_shading.ambient == 0.0))
        };
        // From a flame, its saturation a flame's, into Escape on a solid.
        let mut flame = FractalConfig::default();
        flame.saturation = 1.8;
        flame.escape.formula = "mandelbulb_solid".to_string();
        let mut m = ConfigManager::new(flame);
        switch_render_mode(&mut m, RenderMode::Escape).expect("switch");
        assert!(looks_3d(m.active_config(), true), "the mode switch into a solid");
        m.undo().expect("undo");
        assert_eq!(m.active_config().render_mode, RenderMode::TwoD);
        assert_eq!(m.active_config().saturation, 1.8, "one undo step");

        // The formula picker, from a plane.
        let mut plane = FractalConfig::default();
        plane.render_mode = RenderMode::Escape;
        plane.exposure = 3.0;
        let mut m = ConfigManager::new(plane.clone());
        apply_with_look(&mut m, vec![(ConfigPath::EscapeFormula, ConfigValue::String("kifs_solid".into()))], "history.param.escape_formula").unwrap();
        assert!(looks_3d(m.active_config(), true), "the picker");
        // ... and from it to another solid: nothing.
        let mut c = m.active_config().clone();
        c.exposure = 3.0;
        let mut m = ConfigManager::new(c);
        apply_with_look(&mut m, vec![(ConfigPath::EscapeFormula, ConfigValue::String("mandelbox_solid".into()))], "history.param.escape_formula").unwrap();
        assert_eq!(m.active_config().exposure, 3.0, "a solid to a solid keeps its look");

        // A terrain switched on; a touched rig kept.
        let mut touched = plane;
        touched.solid_shading.shading_strength = 0.5;
        touched.solid_shading.ambient = 0.3;
        let mut m = ConfigManager::new(touched);
        apply_with_look(&mut m, vec![(ConfigPath::EscapeTerrainEnabled, true.into())], "history.param.escape_terrain_enabled").unwrap();
        assert!(looks_3d(m.active_config(), false), "the terrain");
        assert_eq!((m.active_config().solid_shading.shading_strength, m.active_config().solid_shading.ambient), (0.5, 0.3), "a touched rig stays");
        // An untouched rig on a terrain: light 0 takes the default sun's
        // place, so the sun does not swing.
        let mut m = ConfigManager::new({
            let mut c = FractalConfig::default();
            c.render_mode = RenderMode::Escape;
            c
        });
        apply_with_look(&mut m, vec![(ConfigPath::EscapeTerrainEnabled, true.into())], "history.param.escape_terrain_enabled").unwrap();
        let c = m.active_config();
        assert!(looks_3d(c, true), "an untouched terrain");
        assert_eq!((c.solid_shading.lights[0].azimuth, c.solid_shading.lights[0].elevation), crate::escape::terrain::DEFAULT_SUN);
        assert!(c.solid_shading.lights[0].enabled);
        // A plane is no 3D view: the picker choosing another plane changes nothing else.
        let mut m = ConfigManager::new({
            let mut c = FractalConfig::default();
            c.render_mode = RenderMode::Escape;
            c.exposure = 3.0;
            c
        });
        apply_with_look(&mut m, vec![(ConfigPath::EscapeFormula, ConfigValue::String("burning_ship".into()))], "history.param.escape_formula").unwrap();
        assert_eq!(m.active_config().exposure, 3.0);
    }

    /// Every ordered pair of modes: the switch lands where it was told.
    #[test]
    fn every_mode_switch_lands_in_the_right_mode() {
        for from in RenderMode::ALL {
            for to in RenderMode::ALL {
                let mut m = manager_in(*from);
                switch_render_mode(&mut m, *to).expect("switch");
                assert_eq!(
                    m.active_config().render_mode,
                    *to,
                    "{from:?} -> {to:?} did not land"
                );
            }
        }
    }

    /// A mode change is its own undo entry. Coalescing used to merge
    /// consecutive switches into one, so two changes of mind cost one
    /// undo and landed you two modes back.
    #[test]
    fn each_mode_switch_is_its_own_undo_entry() {
        let mut m = manager_in(RenderMode::TwoD);
        let before = m.history_len();
        switch_render_mode(&mut m, RenderMode::ThreeD).expect("to 3d");
        switch_render_mode(&mut m, RenderMode::TwoD).expect("back to 2d");
        assert_eq!(
            m.history_len() - before,
            2,
            "two switches must be two undo entries"
        );
    }

    /// Switching to the mode you are already in does nothing at all --
    /// a menu row for the active mode must not cost an undo entry.
    #[test]
    fn switching_to_the_current_mode_is_a_no_op() {
        for mode in RenderMode::ALL {
            let mut m = manager_in(*mode);
            let before = m.history_len();
            switch_render_mode(&mut m, *mode).expect("same");
            assert_eq!(m.history_len(), before, "{mode:?} -> {mode:?} touched history");
        }
    }

    /// `switch_render_mode` is the ONLY thing that writes the mode.
    ///
    /// The rescue that keeps a non-flame image from rendering black is
    /// applied there, so a second writer would be a mode change that
    /// silently skips it -- which is exactly what the View menu and
    /// the View panel used to be. A source scan rather than a type
    /// trick, because `ConfigPath` is an ordinary enum anyone can
    /// name; if the tree is not present (a packaged build) it passes.
    #[test]
    fn nothing_outside_this_module_writes_the_render_mode() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        if !root.is_dir() {
            eprintln!("no source tree; skipping");
            return;
        }
        // Whole-config load paths legitimately carry a mode in from a
        // file or a browser row; they are not switches, and the app
        // follows them through `load_generation`.
        const LOADS_A_WHOLE_CONFIG: &[&str] = &["ui_handlers.rs", "panel_viewer.rs", "api"];
        let mut offenders: Vec<String> = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "rs") {
                    continue;
                }
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                // `config/mode.rs` writes it; `delta.rs`, `manager.rs` and
                // `apply.rs` (the setter) define it.
                if name == "render_mode.rs" || name == "mode.rs" || name == "delta.rs" || name == "manager.rs" || name == "apply.rs" {
                    continue;
                }
                let rel = path.strip_prefix(&root).unwrap_or(&path).to_string_lossy().to_string();
                if LOADS_A_WHOLE_CONFIG.iter().any(|allow| rel.contains(allow)) {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else { continue };
                for (n, line) in text.lines().enumerate() {
                    let line = line.trim_start();
                    if line.starts_with("//") || line.starts_with("///") {
                        continue;
                    }
                    if line.contains("ConfigPath::RenderMode") {
                        offenders.push(format!("{rel}:{}", n + 1));
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "these write the render mode directly instead of going through \
             ConfigManager::choose, so they skip the engines' settings swap: {offenders:?}"
        );
    }

    /// Each engine is resident in its own mode and no other.
    #[test]
    fn each_engine_is_resident_in_its_own_mode_alone() {
        for m in RenderMode::ALL {
            assert_eq!(keeps_escape_engine(*m), *m == RenderMode::Escape, "escape in {m:?}");
            assert_eq!(keeps_sim_engine(*m), *m == RenderMode::Simulation, "sim in {m:?}");
        }
        // And no mode keeps both, which is the property that makes
        // freeing on switch worth doing at all.
        for m in RenderMode::ALL {
            assert!(
                !(keeps_escape_engine(*m) && keeps_sim_engine(*m)),
                "{m:?} would hold both engines"
            );
        }
    }

    /// The spacebar drives the simulation in Simulation mode and the
    /// animation everywhere else. Tabled over every mode so a fifth
    /// one cannot be added without an answer.
    #[test]
    fn the_spacebar_drives_the_simulation_in_simulation_mode_alone() {
        for m in RenderMode::ALL {
            assert_eq!(
                space_runs_the_simulation(*m),
                *m == RenderMode::Simulation,
                "{m:?}"
            );
        }
    }

    /// Every mode names itself, and no two share a key.
    #[test]
    fn every_mode_has_its_own_label_key() {
        let keys: Vec<&str> = RenderMode::ALL.iter().map(|m| mode_label_key(*m)).collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), keys.len(), "duplicate label keys: {keys:?}");
    }

    /// Both keys of every mode resolve to real text. `t!` hands back
    /// the key itself when it is missing, which is how a typo shows.
    #[test]
    fn every_mode_label_and_tip_resolves() {
        use rust_i18n::t;
        for m in RenderMode::ALL {
            for key in [mode_label_key(*m), mode_tip_key(*m)] {
                let text = t!(key);
                assert_ne!(text.as_ref(), key, "{m:?}: missing locale key {key}");
            }
        }
    }

    /// Each mode brings its own editor forward: the flame modes the
    /// Transforms panel, the other two their own -- and each acts in its
    /// mode, or a mode change would open onto a panel saying it does not.
    #[test]
    fn each_mode_brings_its_own_panel_forward() {
        use super::super::workspace::PanelType;
        assert_eq!(main_panel_for(RenderMode::TwoD), PanelType::Transforms);
        assert_eq!(main_panel_for(RenderMode::ThreeD), PanelType::Transforms);
        assert_eq!(main_panel_for(RenderMode::Escape), PanelType::Escape);
        assert_eq!(main_panel_for(RenderMode::Simulation), PanelType::Simulation);
        for m in RenderMode::ALL {
            assert!(
                super::super::visibility::panel(main_panel_for(*m), *m, super::super::visibility::Solid::No).is_active(),
                "{m:?} brings forward a panel inactive in it"
            );
        }
    }

    /// Choosing something for another engine takes the config into its
    /// mode in the same undo step, with the tone map's rescue from a
    /// Log-mapped flame; in the mode already, the changes alone.
    #[test]
    fn choosing_enters_the_mode_in_one_undo_step() {
        use crate::config::ConfigValue;
        let mut m = manager_in(RenderMode::TwoD);
        m.update_param(ConfigPath::TonemapMode, ToneMapMode::Logarithmic.into()).expect("tonemap");
        let before = m.history_len();
        choose(&mut m, vec![(ConfigPath::EscapeFormula, ConfigValue::String("burning_ship".into()))], "history.param.escape_formula", RenderMode::Escape)
            .expect("choose");
        let c = m.active_config();
        assert_eq!((c.render_mode, c.escape.formula.as_str(), c.tonemap_mode), (RenderMode::Escape, "burning_ship", ToneMapMode::Linear));
        assert_eq!(m.history_len() - before, 1, "one undo step");
        m.undo().expect("undo");
        let c = m.active_config();
        assert_eq!((c.render_mode, c.tonemap_mode), (RenderMode::TwoD, ToneMapMode::Logarithmic), "one undo reverses it all");
        assert_ne!(c.escape.formula, "burning_ship");

        // In the mode already: the change, nothing else.
        let mut m = manager_in(RenderMode::Escape);
        m.update_param(ConfigPath::Exposure, 3.0f32.into()).expect("exposure");
        choose(&mut m, vec![(ConfigPath::EscapeFormula, ConfigValue::String("burning_ship".into()))], "history.param.escape_formula", RenderMode::Escape)
            .expect("choose");
        assert_eq!((m.active_config().escape.formula.as_str(), m.active_config().exposure), ("burning_ship", 3.0));
    }
}

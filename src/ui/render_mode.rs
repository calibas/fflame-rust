//! Switching the render mode, and what the mode implies for the UI.
//!
//! The four modes (`RenderMode::ALL`) are peers, and this is the one
//! place that moves between them. Before this module the mode was
//! written from five sites that did not share code -- the View menu,
//! the compact menu, the View panel, and enter/leave buttons inside
//! the Escape and Simulation panels -- and only the last two reset the
//! tone mapping, so leaving Escape by the menu left a flame wearing
//! escape-calibrated exposure. See `docs/archive/projects/ui-render-modes.md`
//! section 3.1.

use crate::config::delta::ConfigPath;
use crate::config::manager::{ConfigError, ConfigManager};
use crate::scene::transforms::RenderMode;

/// Change the render mode.
///
/// Both non-flame engines write a unit-range image, and a flame's
/// Log-calibrated exposure renders that black. Entering EITHER from a
/// flame mode resets the tonemap once, batched into a single undo
/// entry with the mode change; switching between the two non-flame
/// modes does not, because the values are already right.
///
/// The reset is one-way on purpose, and the reason is worth keeping
/// because "just restore it on the way out" looks obviously right and
/// is not.
///
/// What would have to be restored is session state: the config holds
/// exactly one tone-mapping triple, and two mode families take turns
/// owning it. A saved escape or simulation config carries its Linear
/// mapping legitimately -- that is part of how it looks -- so the
/// previous flame's values belong nowhere on disk. Remembering them
/// anywhere else makes a side channel that undo cannot see, and the
/// mode change is then recorded in two places that drift apart: redo
/// re-applies the batch without going through here, so the memory and
/// the history disagree after any undo tour. There are sharper edges
/// too. The reset is conditional on Logarithmic, so a naive memory
/// would not refresh when it does not fire and would later restore
/// over a mapping the user chose deliberately. And loading a different
/// fractal while in Escape would hand ITS tone mapping to the config
/// that arrives.
///
/// Against all that: the tone-map MODE selector is already disabled in
/// the non-flame modes, so a round trip cannot lose a chosen mapping.
/// It loses exposure and gamma, two sliders, and it is visible the
/// instant it happens because the picture changes.
///
/// If it is ever fixed, the shape is a per-mode stash mirroring
/// `Workspace::stashed`: recorded on every entry rather than only when
/// the reset fires, stamped with the config's load generation so a
/// load invalidates it, and applied inside the same batch as the mode
/// change so one undo still reverses the whole thing.
pub fn switch_render_mode(
    config_manager: &mut ConfigManager,
    mode: RenderMode,
) -> Result<(), ConfigError> {
    let config = config_manager.active_config();
    if config.render_mode == mode {
        return Ok(());
    }
    let entering_non_flame = is_non_flame(mode) && !is_non_flame(config.render_mode);
    let default_tonemap = entering_non_flame
        && config.tonemap_mode == crate::scene::tonemap::ToneMapMode::Logarithmic;
    if default_tonemap {
        config_manager
            .update_batch(
                vec![
                    (ConfigPath::RenderMode, mode.into()),
                    (
                        ConfigPath::TonemapMode,
                        crate::scene::tonemap::ToneMapMode::Linear.into(),
                    ),
                    (
                        ConfigPath::Exposure,
                        crate::config::defaults::DEFAULT_EXPOSURE.into(),
                    ),
                    (
                        ConfigPath::Gamma,
                        crate::config::defaults::DEFAULT_GAMMA.into(),
                    ),
                ],
                "history.param.render_mode".to_string(),
            )
            .map(|_| ())
    } else {
        config_manager
            .update_param(ConfigPath::RenderMode, mode.into())
            .map(|_| ())
    }
}

/// The two engines that are not the chaos game.
pub fn is_non_flame(mode: RenderMode) -> bool {
    mode.is_non_flame()
}

/// Whether the fly camera means anything in this mode.
///
/// Its own function because the menu bar used to ask `!render_mode_2d`,
/// which is true in Escape and Simulation as well -- so the button
/// stayed live in both, and only a runtime check in
/// `app::fly_camera` stopped it doing anything.
pub fn fly_mode_available(mode: RenderMode) -> bool {
    matches!(mode, RenderMode::ThreeD)
}

/// The workspace a mode wants, and the panel that edits it.
///
/// `None` for the flame modes: they share the Standard workspace, and
/// a caller leaving a non-flame mode reads that as "go back to
/// Standard". Written as a table rather than nested ifs because a
/// third mode made the branching the part most likely to gain a hole.
/// The canonical copy -- `app::follow_loaded_render_mode` used to keep
/// its own.
pub fn layout_for(
    mode: RenderMode,
) -> Option<(super::workspace::WorkspaceLayout, super::workspace::PanelType)> {
    use super::workspace::{PanelType, WorkspaceLayout};
    match mode {
        RenderMode::Escape => Some((WorkspaceLayout::EscapeTime, PanelType::Escape)),
        RenderMode::Simulation => Some((WorkspaceLayout::Simulation, PanelType::Simulation)),
        RenderMode::TwoD | RenderMode::ThreeD => None,
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
    use crate::config::FractalConfig;
    use crate::scene::tonemap::ToneMapMode;

    fn manager_in(mode: RenderMode) -> ConfigManager {
        let mut config = FractalConfig::default();
        config.render_mode = mode;
        ConfigManager::new(config)
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

    /// Entering a non-flame mode from a flame mode rescues the tone
    /// mapping; every other pair leaves it alone.
    #[test]
    fn only_entering_a_non_flame_mode_resets_the_tone_mapping() {
        for from in RenderMode::ALL {
            for to in RenderMode::ALL {
                if from == to {
                    continue;
                }
                let mut m = manager_in(*from);
                // Start Logarithmic, as a flame preset would.
                m.update_param(ConfigPath::TonemapMode, ToneMapMode::Logarithmic.into())
                    .expect("tonemap");
                switch_render_mode(&mut m, *to).expect("switch");
                let want_reset = is_non_flame(*to) && !is_non_flame(*from);
                let got = m.active_config().tonemap_mode;
                if want_reset {
                    assert_eq!(got, ToneMapMode::Linear, "{from:?} -> {to:?} should reset");
                } else {
                    assert_eq!(
                        got,
                        ToneMapMode::Logarithmic,
                        "{from:?} -> {to:?} should not touch the tone mapping"
                    );
                }
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
                if name == "render_mode.rs" || name == "delta.rs" || name == "manager.rs" {
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
            "these write the render mode directly instead of calling \
             switch_render_mode, so they skip the tone-map rescue: {offenders:?}"
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

    /// Fly mode belongs to 3D alone.
    #[test]
    fn fly_mode_is_offered_in_three_d_alone() {
        for mode in RenderMode::ALL {
            assert_eq!(
                fly_mode_available(*mode),
                *mode == RenderMode::ThreeD,
                "{mode:?}"
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

    /// Each non-flame mode brings its own workspace and its own
    /// editor; the flame modes share Standard.
    #[test]
    fn only_the_non_flame_modes_bring_a_workspace() {
        use super::super::workspace::{PanelType, WorkspaceLayout};
        assert_eq!(layout_for(RenderMode::TwoD), None);
        assert_eq!(layout_for(RenderMode::ThreeD), None);
        assert_eq!(
            layout_for(RenderMode::Escape),
            Some((WorkspaceLayout::EscapeTime, PanelType::Escape))
        );
        assert_eq!(
            layout_for(RenderMode::Simulation),
            Some((WorkspaceLayout::Simulation, PanelType::Simulation))
        );
        // And each mode's own editor is available in it -- a workspace
        // that opens a panel the visibility policy greys would be a
        // contradiction.
        for m in RenderMode::ALL {
            if let Some((_, panel)) = layout_for(*m) {
                assert!(
                    super::super::visibility::panel(panel, *m, super::super::visibility::Solid::No)
                        .is_show(),
                    "{m:?} opens a panel it also greys"
                );
            }
        }
    }
}

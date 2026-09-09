//! Switching the render mode, and what the mode implies for the UI.
//!
//! The four modes (`RenderMode::ALL`) are peers, and this is the one
//! place that moves between them. Before this module the mode was
//! written from five sites that did not share code -- the View menu,
//! the compact menu, the View panel, and enter/leave buttons inside
//! the Escape and Simulation panels -- and only the last two reset the
//! tone mapping, so leaving Escape by the menu left a flame wearing
//! escape-calibrated exposure. See `docs/projects/ui-render-modes.md`
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
/// The reset is deliberately one-way: leaving does not restore what
/// you had. That is a known wart, filed in the plan's section 5.
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
    matches!(mode, RenderMode::Escape | RenderMode::Simulation)
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
}

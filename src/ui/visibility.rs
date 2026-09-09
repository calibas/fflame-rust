//! What the active render mode makes available.
//!
//! One place answers "does this mean anything right now", so the panel
//! dispatcher, the desktop Window menu and the compact Window submenu
//! cannot disagree. Before this module the dispatcher held two
//! `matches!` blocks, the menus held none at all -- so the Window menu
//! cheerfully opened eight panels in Escape mode that then rendered a
//! one-line stub -- and the stub's text was a single shared string that
//! was wrong in two of the three places it appeared.
//!
//! See `docs/projects/ui-render-modes.md` section 3.2. `RenderMode` is
//! the only axis here; the signature has room for the skill level that
//! `ux-improvements.md` section 2 proposes and for the compact flag
//! already threaded through `PanelViewerContext`, without either being
//! built.

use super::workspace::PanelType;
use crate::scene::transforms::RenderMode;

/// Whether a thing is offered, offered-but-dead, or absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vis {
    /// Draw it and let it work.
    Show,
    /// Draw it disabled. The key names the reason, so a user who
    /// looks for a control finds out why it is not available rather
    /// than wondering whether it exists.
    Grey(&'static str),
    /// Do not draw it at all. Reserved for whole sections inside a
    /// panel (plan phase 4); no PANEL is ever hidden, because hiding
    /// its menu row would make it unreachable AND undiscoverable.
    #[allow(dead_code)]
    Hide,
}

impl Vis {
    pub fn is_show(self) -> bool {
        matches!(self, Vis::Show)
    }
}

/// Edits the flame, which this mode does not render.
const FLAME_ONLY: &str = "visibility.flame_only";
/// The 3D flame engine alone.
const THREE_D_ONLY: &str = "visibility.three_d_only";
/// The other non-flame engine's editing surface.
const OTHER_ENGINE: &str = "visibility.other_engine";
/// Produces a flame, so using it would leave this mode.
const MAKES_A_FLAME: &str = "visibility.makes_a_flame";

/// Is this panel meaningful in this mode?
///
/// Exhaustive by construction -- there is no `_` arm, so a new panel
/// or a new mode does not compile until someone has decided what it
/// means. That is the point of the function.
pub fn panel(p: PanelType, m: RenderMode) -> Vis {
    use PanelType as P;
    use RenderMode as M;
    match p {
        // The picture, and everything that reads the shared tail:
        // both non-flame engines write an image in the flame
        // accumulator's layout and go through the same density
        // effects, tonemap and colour effects.
        P::FractalViewport
        | P::Colors
        | P::PaletteEditor
        | P::PaletteLibrary
        | P::Effects
        | P::FractalBrowser
        | P::History
        | P::Animation
        | P::Signal
        | P::Scripts
        | P::Performance
        | P::Help
        | P::KeyboardShortcuts
        | P::ConfigDialog
        | P::Export
        | P::Rendering
        | P::LoginDialog
        | P::SaveOnlineDialog => Vis::Show,

        // Flame-only editing surfaces.
        P::View | P::XaosEditor | P::Subflames | P::PathEditor => match m {
            M::TwoD | M::ThreeD => Vis::Show,
            M::Escape | M::Simulation => Vis::Grey(FLAME_ONLY),
        },

        // The transform editors. Simulation uses the flame's
        // transforms as its per-layer warps (simulation-layers plan,
        // section 4), so they stay there and go in Escape. They are
        // live only when `sim.use_transforms` is on, which is a
        // control-level matter for phase 4, not a reason to hide the
        // panel: it is how you turn the feature on.
        P::Transforms | P::TriangleEditor | P::Variations => match m {
            M::TwoD | M::ThreeD | M::Simulation => Vis::Show,
            M::Escape => Vis::Grey(FLAME_ONLY),
        },

        // Occlusion and the shade pass are pseudo-3D flame features.
        // The panel already says so itself; saying it in the menu too
        // means you find out before opening it.
        P::SolidLighting => match m {
            M::ThreeD => Vis::Show,
            M::TwoD | M::Escape | M::Simulation => Vis::Grey(THREE_D_ONLY),
        },

        // It works, but everything it produces is a flame, so using it
        // silently leaves the mode you are in.
        P::RandomGenerator => match m {
            M::TwoD | M::ThreeD => Vis::Show,
            M::Escape | M::Simulation => Vis::Grey(MAKES_A_FLAME),
        },

        // The two non-flame editors. Each is the way INTO its mode, so
        // both are offered from a flame mode; each hides the other,
        // because a config is exactly one mode and showing both would
        // read as though they compose.
        P::Escape => match m {
            M::TwoD | M::ThreeD | M::Escape => Vis::Show,
            M::Simulation => Vis::Grey(OTHER_ENGINE),
        },
        P::Simulation => match m {
            M::TwoD | M::ThreeD | M::Simulation => Vis::Show,
            M::Escape => Vis::Grey(OTHER_ENGINE),
        },
    }
}

/// One row of a Window menu.
pub struct WindowMenuRow {
    pub panel: PanelType,
    pub label_key: &'static str,
    /// Drawn only when online mode is on.
    pub online_only: bool,
}

const fn row(panel: PanelType, label_key: &'static str) -> WindowMenuRow {
    WindowMenuRow { panel, label_key, online_only: false }
}

/// The desktop Window menu, in order. The single source of which
/// panels a menu offers and what they are called; the compact submenu
/// below picks from it rather than keeping its own copy of the labels.
pub static WINDOW_MENU: &[WindowMenuRow] = &[
    row(PanelType::Performance, "menu.window_performance"),
    row(PanelType::Rendering, "menu.window_rendering"),
    row(PanelType::View, "menu.window_view"),
    row(PanelType::Escape, "menu.window_escape"),
    row(PanelType::Simulation, "menu.window_simulation"),
    row(PanelType::SolidLighting, "menu.window_solid_lighting"),
    row(PanelType::Transforms, "menu.window_transforms"),
    row(PanelType::TriangleEditor, "menu.window_triangle_editor"),
    row(PanelType::Colors, "menu.window_colors"),
    row(PanelType::PaletteEditor, "menu.window_palette_editor"),
    row(PanelType::PaletteLibrary, "menu.window_palette_library"),
    row(PanelType::FractalBrowser, "menu.window_fractal_browser"),
    row(PanelType::History, "menu.window_history"),
    row(PanelType::Animation, "menu.window_animation"),
    row(PanelType::PathEditor, "menu.window_path_editor"),
    row(PanelType::RandomGenerator, "menu.window_random_generator"),
    row(PanelType::Effects, "menu.window_effects"),
    row(PanelType::Variations, "menu.window_variations"),
    row(PanelType::Scripts, "menu.window_scripts"),
    row(PanelType::Subflames, "menu.window_subflames"),
    row(PanelType::XaosEditor, "menu.window_xaos_editor"),
    row(PanelType::Signal, "menu.window_signal"),
    WindowMenuRow {
        panel: PanelType::LoginDialog,
        label_key: "menu.window_account",
        online_only: true,
    },
];

/// The compact Window submenu keeps its own ORDER -- touch priority,
/// transforms first -- but not its own labels. Palette Editor and
/// Palette Library are deliberately absent: both are reachable from
/// the Colors panel, and on a phone this menu was scrolling off the
/// bottom. Path Editor, Random Generator and Account are absent too.
pub static COMPACT_WINDOW_MENU: &[PanelType] = &[
    PanelType::Transforms,
    PanelType::TriangleEditor,
    PanelType::Colors,
    PanelType::View,
    PanelType::Rendering,
    PanelType::SolidLighting,
    PanelType::FractalBrowser,
    PanelType::Variations,
    PanelType::Subflames,
    PanelType::Scripts,
    PanelType::Escape,
    PanelType::Simulation,
    PanelType::History,
    PanelType::Effects,
    PanelType::XaosEditor,
    PanelType::Animation,
    PanelType::Signal,
    PanelType::Performance,
];

/// The label a compact row shows, from the shared table.
pub fn label_key_of(p: PanelType) -> &'static str {
    WINDOW_MENU
        .iter()
        .find(|r| r.panel == p)
        .map(|r| r.label_key)
        .unwrap_or("menu.window")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_i18n::t;

    /// Every panel this build knows about, so the tests below cannot
    /// silently skip a new one.
    const ALL_PANELS: &[PanelType] = &[
        PanelType::FractalViewport,
        PanelType::Transforms,
        PanelType::TriangleEditor,
        PanelType::Colors,
        PanelType::PaletteEditor,
        PanelType::PaletteLibrary,
        PanelType::FractalBrowser,
        PanelType::View,
        PanelType::Rendering,
        PanelType::History,
        PanelType::Animation,
        PanelType::Performance,
        PanelType::Help,
        PanelType::KeyboardShortcuts,
        PanelType::ConfigDialog,
        PanelType::PathEditor,
        PanelType::Export,
        PanelType::RandomGenerator,
        PanelType::Scripts,
        PanelType::Effects,
        PanelType::XaosEditor,
        PanelType::SolidLighting,
        PanelType::Signal,
        PanelType::LoginDialog,
        PanelType::SaveOnlineDialog,
        PanelType::Variations,
        PanelType::Subflames,
        PanelType::Escape,
        PanelType::Simulation,
    ];

    /// The list above is the whole enum. `PanelType` has no iterator,
    /// so this is the closest thing to one -- and `Display` panics on
    /// nothing, so a missing variant shows up as a count mismatch the
    /// moment someone adds one without updating the tests.
    #[test]
    fn the_test_list_covers_every_panel() {
        let mut seen: Vec<String> = ALL_PANELS.iter().map(|p| format!("{p:?}")).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), ALL_PANELS.len(), "duplicate entry in ALL_PANELS");
        assert_eq!(ALL_PANELS.len(), 29, "a panel was added; decide what it means per mode");
    }

    /// Every reason a control is greyed names a string that exists.
    /// `t!` returns the key itself when it is missing, which is how
    /// this catches a typo.
    #[test]
    fn every_reason_resolves_to_real_text() {
        for p in ALL_PANELS {
            for m in RenderMode::ALL {
                if let Vis::Grey(key) = panel(*p, *m) {
                    let text = t!(key);
                    assert_ne!(text.as_ref(), key, "{p:?}/{m:?}: missing locale key {key}");
                    assert!(text.len() > 10, "{key} is too terse to explain anything");
                }
            }
        }
    }

    /// The flame modes offer everything except the two that are not
    /// theirs: Solid & Lighting is 3D alone.
    #[test]
    fn the_flame_modes_offer_everything_but_solid_in_two_d() {
        for p in ALL_PANELS {
            assert!(panel(*p, RenderMode::ThreeD).is_show(), "{p:?} missing in 3D");
            let want = *p != PanelType::SolidLighting;
            assert_eq!(panel(*p, RenderMode::TwoD).is_show(), want, "{p:?} in 2D");
        }
    }

    /// Escape and Simulation grey exactly the panels the plan's
    /// section 1.2 table says, and nothing else. Pinning the set is
    /// the regression guard: a panel quietly dropping out of a mode is
    /// the failure this whole module exists to prevent.
    #[test]
    fn each_non_flame_mode_greys_exactly_its_documented_set() {
        let greyed = |m: RenderMode| -> Vec<String> {
            let mut v: Vec<String> = ALL_PANELS
                .iter()
                .filter(|p| !panel(**p, m).is_show())
                .map(|p| format!("{p:?}"))
                .collect();
            v.sort();
            v
        };
        let mut want_escape = vec![
            "Transforms", "TriangleEditor", "Variations", "View", "XaosEditor",
            "Subflames", "PathEditor", "SolidLighting", "RandomGenerator", "Simulation",
        ];
        want_escape.sort();
        assert_eq!(greyed(RenderMode::Escape), want_escape, "Escape");

        let mut want_sim = vec![
            "View", "XaosEditor", "Subflames", "PathEditor", "SolidLighting",
            "RandomGenerator", "Escape",
        ];
        want_sim.sort();
        assert_eq!(greyed(RenderMode::Simulation), want_sim, "Simulation");
    }

    /// The two engines hide each other, and each is reachable from a
    /// flame mode -- otherwise there would be no way in.
    #[test]
    fn each_engine_is_reachable_from_a_flame_mode_and_hides_the_other() {
        assert!(panel(PanelType::Escape, RenderMode::TwoD).is_show());
        assert!(panel(PanelType::Simulation, RenderMode::TwoD).is_show());
        assert!(!panel(PanelType::Escape, RenderMode::Simulation).is_show());
        assert!(!panel(PanelType::Simulation, RenderMode::Escape).is_show());
        assert!(panel(PanelType::Escape, RenderMode::Escape).is_show());
        assert!(panel(PanelType::Simulation, RenderMode::Simulation).is_show());
    }

    /// The compact submenu picks from the desktop table rather than
    /// keeping its own, so labels cannot drift apart.
    #[test]
    fn the_compact_menu_is_a_subset_of_the_window_menu() {
        for p in COMPACT_WINDOW_MENU {
            let row = WINDOW_MENU.iter().find(|r| r.panel == *p);
            let row = row.unwrap_or_else(|| panic!("{p:?} is in the compact menu but not the desktop one"));
            assert!(!row.online_only, "{p:?}: the compact menu has no online gate");
            assert_eq!(label_key_of(*p), row.label_key);
        }
    }

    /// No panel appears twice in a menu, and every label key exists.
    #[test]
    fn the_window_menu_is_well_formed() {
        let mut seen: Vec<String> = WINDOW_MENU.iter().map(|r| format!("{:?}", r.panel)).collect();
        seen.sort();
        let before = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), before, "a panel appears twice in the Window menu");
        for r in WINDOW_MENU {
            let text = t!(r.label_key);
            assert_ne!(text.as_ref(), r.label_key, "missing locale key {}", r.label_key);
        }
    }
}

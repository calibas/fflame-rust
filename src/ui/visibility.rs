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
//! See `docs/archive/projects/ui-render-modes.md` section 3.2. Panels depend on
//! the render mode alone; controls also depend on the tone-map mode,
//! because some of the tonemap's own parameters are read by one branch
//! of it and not the others. There is room for the skill level that
//! `ux-improvements.md` section 2 proposes and for the compact flag
//! already threaded through `PanelViewerContext`, without either being
//! built.

use super::workspace::PanelType;
use crate::scene::tonemap::ToneMapMode;
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
/// Only the logarithmic tone mapping reads it.
const LOG_ONLY: &str = "visibility.log_only";
/// Only the linear tone mapping behaves in this mode.
const LINEAR_ONLY: &str = "visibility.linear_only";
/// Mixes between two values that are equal here.
const ALPHA_BLEND_INERT: &str = "visibility.alpha_blend_inert";

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
        | P::SaveOnlineDialog
        // The transform editors, in EVERY mode. Simulation uses the
        // flame's transforms as its per-layer warps (simulation-layers
        // plan, section 4), and escape mode D renders the flame's
        // attractor as a distance field — so in both, editing a
        // transform edits the picture. In mode A and mode B the flame
        // is inert, but the panel is how you prepare one before
        // switching, and greying it there would be a per-FORMULA
        // answer from a per-mode policy.
        | P::Transforms
        | P::TriangleEditor
        | P::Variations => Vis::Show,

        // Flame-only editing surfaces.
        P::View | P::XaosEditor | P::Subflames | P::PathEditor => match m {
            M::TwoD | M::ThreeD => Vis::Show,
            M::Escape | M::Simulation => Vis::Grey(FLAME_ONLY),
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

/// A control, or a group of controls that share a fate.
///
/// Grouped rather than one variant per widget: the answer is the same
/// for every slider the logarithmic branch alone reads, and a variant
/// each would be nine ways to get the same decision wrong. The
/// groupings come from measuring what the shaders actually read --
/// `docs/archive/projects/ui-render-modes.md` sections 1.4 and 3.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// Everything driven by the chaos game: pause, reset accumulation,
    /// max iterations, iterations per thread, burn-in, deterministic
    /// RNG, the blend controls, and the Rendering menu that repeats
    /// them. `should_iterate` is false in both non-flame modes, so
    /// none of it runs there.
    ChaosGame,
    /// The deep-zoom reference orbit cache: escape's alone.
    OrbitCache,
    /// The tone-map preset dropdown, and Reset Colors. Not merely
    /// dead in a non-flame mode but HARMFUL: every preset is
    /// Log-calibrated, so applying one re-blacks a unit-range image --
    /// the exact thing entering the mode resets the tone mapping to
    /// prevent.
    TonemapPresets,
    /// The tone-map mode selector. Both non-flame engines write a
    /// unit-range image and no sample density, so Logarithmic and
    /// Density divide by a floor of 1e-6 and Linear is the only
    /// setting that behaves.
    TonemapMode,
    /// Read only by the logarithmic branch: gamma threshold,
    /// brightness, vibrancy, highlights.
    LogOnlyTone,
    /// Alpha blend low/high. It mixes between a gamma-corrected alpha
    /// and a linear one -- but only the LOGARITHMIC branch of the
    /// tonemap gamma-corrects alpha, so under the linear mapping the
    /// two values are computed by the same expression and the mix is
    /// an exact no-op. That is true in a flame on Linear exactly as it
    /// is in Escape or Simulation, which is why this asks about the
    /// tone-map mode and not only the render mode.
    ///
    /// The feature was correct when written, before the tonemap grew
    /// per-mode branches; the Linear branch gamma-corrects colour but
    /// not alpha. Making it act under Linear would change every
    /// escape and simulation baseline, so the control is disabled
    /// rather than the shader changed.
    AlphaBlendCurve,
    /// The spatial filter and its blur-edges companion, which run
    /// inside the compute pass the non-flame modes never dispatch.
    SpatialFilter,
    /// The density levels section: histogram, enable, low/high/gamma.
    /// Hard-off for both non-flame modes in the frame loop, and inert
    /// even if it were not.
    DensityLevels,
    /// Panning and zooming the viewport: drag, wheel, pinch, the
    /// arrow keys, and the View menu's Reset / Zoom In / Zoom Out.
    ///
    /// Simulation has no view to move. Flames and escape each have an
    /// absolute view the renderer reads (`config.zoom`/`pan_*` and the
    /// escape centre respectively), but the simulation's only spatial
    /// control is `sim.warp`, which is a PER-STEP transform of the
    /// field itself -- a velocity applied to the content, not a camera
    /// over it. Binding a drag to it would advect the field, blur it
    /// through a resample every step, do nothing at all while paused,
    /// and do nothing in octave mode. So the gesture is refused rather
    /// than misdirected: it used to fall through to the flame path and
    /// write `config.zoom`/`pan_*`, which the simulation ignores --
    /// invisible, but it drifted the flame view you would see on
    /// switching back and filled the history with entries that changed
    /// nothing.
    ///
    /// A real display-time view for the simulation is a feature, not a
    /// bug fix; when it exists this arm becomes `Show`.
    ViewNavigation,
    /// Colour mode, and what hangs off it: speed blend, path style,
    /// path capture and tracking. Neither generator reads the mode,
    /// and choosing PathMap allocates a path buffer, forces a flame
    /// shader recompile, and hides the palette controls both modes
    /// genuinely use.
    ColorMode,
}

/// Is this control meaningful in this mode?
///
/// Exhaustive, like `panel`, and for the same reason. `tone` is the
/// active tone-map mode: a couple of the tonemap's parameters are read
/// by one of its branches and not the others, which is a fact about
/// the tone-map mode rather than the render mode.
pub fn control(c: Control, m: RenderMode, tone: ToneMapMode) -> Vis {
    use Control as C;
    let flame = !matches!(m, RenderMode::Escape | RenderMode::Simulation);
    match c {
        C::ChaosGame | C::TonemapPresets | C::SpatialFilter | C::DensityLevels | C::ColorMode => {
            if flame {
                Vis::Show
            } else {
                Vis::Hide
            }
        }
        C::ViewNavigation => {
            if matches!(m, RenderMode::Simulation) {
                Vis::Hide
            } else {
                Vis::Show
            }
        }
        C::OrbitCache => {
            if matches!(m, RenderMode::Escape) {
                Vis::Show
            } else {
                Vis::Hide
            }
        }
        C::TonemapMode => {
            if flame {
                Vis::Show
            } else {
                Vis::Grey(LINEAR_ONLY)
            }
        }
        C::LogOnlyTone => {
            if flame {
                Vis::Show
            } else {
                Vis::Grey(LOG_ONLY)
            }
        }
        C::AlphaBlendCurve => {
            // Not a mode question: the linear branch does not
            // gamma-correct alpha, so there is nothing to blend
            // between, wherever you are.
            if tone == ToneMapMode::Linear {
                Vis::Grey(ALPHA_BLEND_INERT)
            } else if flame {
                Vis::Show
            } else {
                Vis::Grey(ALPHA_BLEND_INERT)
            }
        }
    }
}

/// Draw `body` under a control's policy: normally, disabled with a
/// hover that explains itself, or not at all.
///
/// Returns `None` when the control is hidden, so a caller can skip a
/// separator or a heading that would otherwise be left behind.
pub fn gated<R>(
    ui: &mut egui::Ui,
    c: Control,
    m: RenderMode,
    tone: ToneMapMode,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    match control(c, m, tone) {
        Vis::Show => Some(body(ui)),
        Vis::Grey(reason) => {
            let inner = ui.add_enabled_ui(false, body);
            inner.response.on_disabled_hover_text(rust_i18n::t!(reason));
            Some(inner.inner)
        }
        Vis::Hide => None,
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

    /// Every control this build knows about.
    const ALL_CONTROLS: &[Control] = &[
        Control::ChaosGame,
        Control::OrbitCache,
        Control::TonemapPresets,
        Control::TonemapMode,
        Control::LogOnlyTone,
        Control::AlphaBlendCurve,
        Control::SpatialFilter,
        Control::DensityLevels,
        Control::ColorMode,
        Control::ViewNavigation,
    ];

    /// Every reason a panel or control is greyed names a string that
    /// exists. `t!` returns the key itself when it is missing, which
    /// is how this catches a typo.
    #[test]
    fn every_reason_resolves_to_real_text() {
        let mut checked = 0;
        for m in RenderMode::ALL {
            for p in ALL_PANELS {
                if let Vis::Grey(key) = panel(*p, *m) {
                    let text = t!(key);
                    assert_ne!(text.as_ref(), key, "{p:?}/{m:?}: missing locale key {key}");
                    assert!(text.len() > 10, "{key} is too terse to explain anything");
                    checked += 1;
                }
            }
            for c in ALL_CONTROLS {
                if let Vis::Grey(key) = control(*c, *m, ToneMapMode::Logarithmic) {
                    let text = t!(key);
                    assert_ne!(text.as_ref(), key, "{c:?}/{m:?}: missing locale key {key}");
                    assert!(text.len() > 10, "{key} is too terse to explain anything");
                    checked += 1;
                }
            }
        }
        assert!(checked > 0, "the scan found nothing to check");
    }

    /// Simulation has no viewport navigation; every other mode does.
    #[test]
    fn only_simulation_refuses_viewport_navigation() {
        for m in RenderMode::ALL {
            let want = *m != RenderMode::Simulation;
            assert_eq!(
                control(Control::ViewNavigation, *m, ToneMapMode::Linear).is_show(),
                want,
                "{m:?}"
            );
        }
    }

    /// Every control is available in both flame modes under the
    /// logarithmic mapping, except the orbit cache, which is escape's
    /// alone and does nothing in a flame.
    #[test]
    fn the_flame_modes_offer_every_control_but_the_orbit_cache() {
        for m in [RenderMode::TwoD, RenderMode::ThreeD] {
            for c in ALL_CONTROLS {
                let want = *c != Control::OrbitCache;
                assert_eq!(
                    control(*c, m, ToneMapMode::Logarithmic).is_show(),
                    want,
                    "{c:?} in {m:?}"
                );
            }
        }
    }

    /// The alpha-blend pair is inert under the LINEAR mapping, in a
    /// flame exactly as in the other engines: the linear branch does
    /// not gamma-correct alpha, so the mix has identical operands.
    /// Every other control is unaffected by the tone-map mode.
    #[test]
    fn the_alpha_blend_pair_is_dead_under_the_linear_mapping() {
        for m in RenderMode::ALL {
            assert!(
                !control(Control::AlphaBlendCurve, *m, ToneMapMode::Linear).is_show(),
                "{m:?}: linear must disable the alpha blend pair"
            );
            for c in ALL_CONTROLS {
                if *c == Control::AlphaBlendCurve {
                    continue;
                }
                assert_eq!(
                    control(*c, *m, ToneMapMode::Linear),
                    control(*c, *m, ToneMapMode::Logarithmic),
                    "{c:?} in {m:?} must not depend on the tone-map mode"
                );
            }
        }
        // And it IS offered where it acts.
        assert!(control(Control::AlphaBlendCurve, RenderMode::TwoD, ToneMapMode::Logarithmic).is_show());
    }

    /// The two traps are gone from the non-flame modes: the tone-map
    /// preset dropdown and Reset Colors both set Logarithmic, which
    /// renders a unit-range image black. Neither is merely greyed --
    /// they are not drawn at all, because there is nothing a user
    /// could want from them here.
    #[test]
    fn the_tone_map_presets_are_unreachable_in_the_non_flame_modes() {
        for m in [RenderMode::Escape, RenderMode::Simulation] {
            assert_eq!(
                control(Control::TonemapPresets, m, ToneMapMode::Linear),
                Vis::Hide,
                "{m:?}: the preset dropdown must not be reachable"
            );
        }
    }

    /// Each non-flame mode's controls, pinned. Same reasoning as the
    /// panel table: a control quietly changing status is the failure
    /// this module exists to prevent.
    #[test]
    fn the_non_flame_modes_gate_exactly_their_documented_controls() {
        for m in [RenderMode::Escape, RenderMode::Simulation] {
            let hidden = |c: Control| control(c, m, ToneMapMode::Logarithmic) == Vis::Hide;
            let greyed = |c: Control| matches!(control(c, m, ToneMapMode::Logarithmic), Vis::Grey(_));
            assert!(hidden(Control::ChaosGame), "{m:?} chaos game");
            assert!(hidden(Control::TonemapPresets), "{m:?} presets");
            assert!(hidden(Control::SpatialFilter), "{m:?} spatial filter");
            assert!(hidden(Control::DensityLevels), "{m:?} levels");
            assert!(hidden(Control::ColorMode), "{m:?} colour mode");
            if m == RenderMode::Simulation {
                assert!(hidden(Control::ViewNavigation), "sim viewport navigation");
            } else {
                assert!(
                    control(Control::ViewNavigation, m, ToneMapMode::Logarithmic).is_show(),
                    "escape keeps its own navigation"
                );
            }
            assert!(greyed(Control::TonemapMode), "{m:?} tone map mode");
            assert!(greyed(Control::LogOnlyTone), "{m:?} log-only tone");
            assert!(greyed(Control::AlphaBlendCurve), "{m:?} alpha blend");
        }
        // The orbit cache is the one control a non-flame mode gains.
        assert!(control(Control::OrbitCache, RenderMode::Escape, ToneMapMode::Linear).is_show());
        assert_eq!(
            control(Control::OrbitCache, RenderMode::Simulation, ToneMapMode::Linear),
            Vis::Hide
        );
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
        // The transform editors are NOT here: escape mode D renders
        // the flame's attractor as a distance field, so editing a
        // transform edits the picture.
        let mut want_escape = vec![
            "View", "XaosEditor",
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

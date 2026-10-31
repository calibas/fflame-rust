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
//! Every panel is drawn in every mode (`docs/projects/one-workspace.md`):
//! [`panel`] says whether it acts on the picture, and one that does not
//! wears a banner saying so ([`inactive_banner`]) above its controls,
//! which stay editable. Controls depend on the tone-map mode too,
//! because some of the tonemap's own parameters are read by one branch
//! of it and not the others; a dead control is greyed or hidden
//! ([`control`]; `docs/archive/projects/ui-render-modes.md` section 3.2).

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
    /// Do not draw it at all. Reserved for whole sections and controls
    /// inside a panel; a PANEL is never hidden (nor greyed: [`panel`]).
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
/// Lights a surface, and nothing loaded renders one. Not "3D flame
/// only": `Solid::of` also answers yes for escape solids and terrains
/// and for simulation terrains, and the old wording sent the user to
/// the wrong mode.
const NEEDS_SURFACE: &str = "visibility.needs_surface";
/// The Escape panel: it sets up an escape-time fractal.
const ESCAPE_ENGINE: &str = "visibility.escape_engine";
/// The Simulation panel: it sets up a simulation.
const SIM_ENGINE: &str = "visibility.sim_engine";
/// The Texture panel: only an escape-time fractal uses a texture so far.
const ESCAPE_TEXTURES: &str = "visibility.escape_textures";
/// The Hybrid Designer writes escape-time formulas.
const ESCAPE_HYBRIDS: &str = "visibility.escape_hybrids";
/// The Rendering menu: it configures the chaos game.
pub const CHAOS_GAME: &str = "visibility.chaos_game";
/// Only the logarithmic tone mapping reads it.
const LOG_ONLY: &str = "visibility.log_only";
/// Only the linear tone mapping behaves in this mode.
const LINEAR_ONLY: &str = "visibility.linear_only";
/// Mixes between two values that are equal here.
const ALPHA_BLEND_INERT: &str = "visibility.alpha_blend_inert";
/// Focused Rendering plans in the plane.
const TWO_D_ONLY: &str = "visibility.two_d_only";
const LIGHT_FLAME: &str = "visibility.light_flame";
/// A simulation's grid scale is its render scale.
const SIM_GRID_SCALE: &str = "visibility.sim_grid_scale";
const POINT_LIGHT_TERRAIN: &str = "visibility.point_light_terrain";
/// A 3D flame's lights turn with its camera, always.
const LIGHT_FRAME_FLAME: &str = "visibility.light_frame_flame";

/// Whether a view offers a kind of light (lights-in-the-fractal, L6): a
/// directional light everywhere; a point where the world holds still --
/// mode D's solids (a terrain's world is the view's, moving with every
/// pan and zoom); a headlamp in every escape and simulation 3D view. The
/// 3D flame's shade pass lights in screen space, directional alone.
pub fn light_kind(kind: crate::config::LightKind, view: crate::camera::gesture::ViewKind) -> Vis {
    use crate::camera::gesture::ViewKind;
    use crate::config::LightKind;
    match (kind, view) {
        (LightKind::Directional, _) => Vis::Show,
        (LightKind::Point, ViewKind::Solid) => Vis::Show,
        (LightKind::Headlamp, ViewKind::Solid | ViewKind::EscapeTerrain | ViewKind::SimTerrain) => Vis::Show,
        (LightKind::Point, ViewKind::EscapeTerrain | ViewKind::SimTerrain) => Vis::Grey(POINT_LIGHT_TERRAIN),
        _ => Vis::Grey(LIGHT_FLAME),
    }
}

/// Whether a view's directional lights may follow the camera
/// (`SolidShadingSettings::lights_follow_camera`): a solid's choose; a 3D
/// flame's always do (its shade pass lights the projected picture); a
/// terrain's never do, so it has nothing to choose.
pub fn light_frame(view: crate::camera::gesture::ViewKind) -> Vis {
    use crate::camera::gesture::ViewKind;
    match view {
        ViewKind::Solid => Vis::Show,
        ViewKind::Flame3d => Vis::Grey(LIGHT_FRAME_FLAME),
        _ => Vis::Hide,
    }
}

/// Whether the loaded config renders a SOLID SURFACE.
///
/// Not a render mode and deliberately not derivable from one (D2 in
/// `ifs-distance-rendering.md`): escape mode is not three-dimensional,
/// one FORMULA in it is, so gating on the mode would offer lighting
/// over a Mandelbrot and withhold it from the thing it steers. A 3D
/// flame is the other case, and it is the same question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Solid {
    Yes,
    No,
}

impl Solid {
    /// What a config says. The flame side is the render mode; the
    /// escape side is the formula.
    pub fn of(config: &crate::config::FractalConfig) -> Solid {
        let yes = match config.render_mode {
            RenderMode::ThreeD => true,
            // A solid formula, or the 3D terrain view of a plane's
            // picture: both are lit surfaces.
            RenderMode::Escape => {
                crate::escape::ifs::formula_is_solid(&config.escape.formula)
                    || config.escape.terrain_active()
            }
            // A simulation drawn as a 3D terrain is lit by the same
            // lights.
            RenderMode::Simulation => cfg!(feature = "terrain") && config.sim.terrain_active(),
            RenderMode::TwoD => false,
        };
        if yes { Solid::Yes } else { Solid::No }
    }
}

/// Whether a panel acts on the picture in a mode.
///
/// Every panel is drawn in every mode, its controls live (one-workspace
/// plan, decision 3): one that does not act says so at its top -- why,
/// and the modes it is for, each a button into it ([`inactive_banner`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelUse {
    Active,
    /// `reason` is a `visibility.*` locale key; `enter`, the modes the
    /// panel acts in that a button can switch to (none when no mode
    /// switch would make it act, as for lighting over a plane).
    Inactive { reason: &'static str, enter: &'static [RenderMode] },
}

impl PanelUse {
    /// The tests' question; the app matches on the banner's reason.
    #[cfg(test)]
    pub fn is_active(self) -> bool {
        matches!(self, PanelUse::Active)
    }
}

const FLAMES: &[RenderMode] = &[RenderMode::TwoD, RenderMode::ThreeD];
const ESCAPE: &[RenderMode] = &[RenderMode::Escape];

/// Does this panel act on the picture in this mode?
///
/// Exhaustive by construction -- there is no `_` arm, so a new panel
/// or a new mode does not compile until someone has decided what it
/// means. That is the point of the function.
///
/// `solid` is the one thing the mode does not settle: lighting belongs
/// to whatever renders a surface, and in escape mode that is a
/// property of the formula.
pub fn panel(p: PanelType, m: RenderMode, solid: Solid) -> PanelUse {
    use PanelType as P;
    use RenderMode as M;
    let inactive = |reason, enter| PanelUse::Inactive { reason, enter };
    match p {
        // The picture, and everything that reads the shared tail:
        // both non-flame engines write an image in the flame
        // accumulator's layout and go through the same density
        // effects, tonemap and colour effects.
        // The View panel: every mode's camera, and how its picture is
        // drawn (camera-unification C5).
        P::FractalViewport
        | P::View
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
        // switching, and a banner there would be a per-FORMULA answer
        // from a per-mode policy.
        | P::Transforms
        | P::TriangleEditor
        | P::Variations
        // A generated flame loads with its own mode, from any mode.
        | P::RandomGenerator
        // Formulas, colourings and solids for Escape (using one enters
        // it), variations for the flame: something for every mode.
        | P::FormulaEditor => PanelUse::Active,

        // Flame-only editing surfaces.
        P::XaosEditor | P::Subflames | P::Paths => match m {
            M::TwoD | M::ThreeD => PanelUse::Active,
            M::Escape | M::Simulation => inactive(FLAME_ONLY, FLAMES),
        },

        // Lighting belongs to whatever renders a SURFACE, and two
        // engines do: the 3D flame pipeline, and escape mode's solid
        // distance marcher. Both read these same settings, so both
        // offer the same panel -- a marcher lights itself rather than
        // going through the shade pass, but the vocabulary is one.
        //
        // Written against `solid` rather than the mode, because the mode
        // is not what decides it -- `Solid::of` already answers for the
        // 3D flame pipeline and for the escape marcher. A 2D flame is a
        // switch away from a surface; a plane in the other engines is a
        // solid or a terrain away, which the reason names.
        P::SolidLighting => match (solid, m) {
            (Solid::Yes, _) => PanelUse::Active,
            (Solid::No, M::TwoD) => inactive(NEEDS_SURFACE, &[M::ThreeD]),
            (Solid::No, M::ThreeD | M::Escape | M::Simulation) => inactive(NEEDS_SURFACE, &[]),
        },

        // Each engine's editor: drawn everywhere, so a formula or a
        // model can be looked at and chosen from any mode -- choosing
        // enters the mode (`render_mode::choose`).
        P::Escape => match m {
            M::Escape => PanelUse::Active,
            M::TwoD | M::ThreeD | M::Simulation => inactive(ESCAPE_ENGINE, ESCAPE),
        },
        P::Simulation => match m {
            M::Simulation => PanelUse::Active,
            M::TwoD | M::ThreeD | M::Escape => inactive(SIM_ENGINE, &[M::Simulation]),
        },

        // Textures go into an escape-time fractal (sim-textures plan:
        // escape first); Simulation makes them, with Save as texture.
        // Picking one enters Escape.
        P::Textures => match m {
            M::Escape => PanelUse::Active,
            M::TwoD | M::ThreeD | M::Simulation => inactive(ESCAPE_TEXTURES, ESCAPE),
        },

        // Its hybrids are escape-time formulas; Use enters Escape.
        P::HybridDesigner => match m {
            M::Escape => PanelUse::Active,
            M::TwoD | M::ThreeD | M::Simulation => inactive(ESCAPE_HYBRIDS, ESCAPE),
        },
    }
}

/// The top of a panel that does not act on the picture in `mode`
/// (one-workspace plan, decision 3): a framed note that says so, why
/// (`reason`), and a button into each mode of `enter`. Returns the mode
/// a button asked for.
pub fn inactive_banner(ui: &mut egui::Ui, mode: RenderMode, reason: &'static str, enter: &[RenderMode]) -> Option<RenderMode> {
    use super::render_mode::mode_label_key;
    let warn = ui.visuals().warn_fg_color;
    let mut picked = None;
    egui::Frame::group(ui.style())
        .stroke(egui::Stroke::new(1.0, warn))
        .fill(warn.gamma_multiply(0.08))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let heading = rust_i18n::t!("visibility.inactive_heading", mode = rust_i18n::t!(mode_label_key(mode)));
            ui.label(egui::RichText::new(heading).strong().color(warn));
            ui.label(rust_i18n::t!(reason));
            if !enter.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    for m in enter {
                        if ui.button(rust_i18n::t!("visibility.switch_to", mode = rust_i18n::t!(mode_label_key(*m)))).clicked() {
                            picked = Some(*m);
                        }
                    }
                });
            }
        });
    ui.add_space(4.0);
    picked
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
    /// Every mode has a view now. A simulation's is `sim.view`, a camera
    /// the colour pass applies over the field (camera-unification C6) --
    /// not `sim.warp`, which is a PER-STEP transform of the field itself,
    /// a velocity applied to the content: binding a drag to that advected
    /// the field, so until the view existed the gesture was refused.
    ViewNavigation,
    /// The deep-zoom section: auto exposure and cylinder targeting.
    /// Both are properties of the CHAOS GAME -- one counts the plot
    /// attempts that land in frame, the other forces a prefix of
    /// transforms -- so neither means anything in a mode that has no
    /// chaos game to instrument.
    DeepZoom,
    /// Colour mode, and what hangs off it: speed blend, path style,
    /// path capture and tracking. Neither generator reads the mode,
    /// and choosing PathMap allocates a path buffer, forces a flame
    /// shader recompile, and hides the palette controls both modes
    /// genuinely use.
    ColorMode,
    /// Focused Rendering itself, and PathMap, which draws through it:
    /// its planners enumerate discs in the PLANE, so a 3D flame gets
    /// neither (the renderer reports `TargetingState::NotPlanar`).
    FocusedRendering,
    /// The viewport's render scale and upscaler (`renderer::upscale`).
    /// Not a simulation's: its grid is the content, a bound grid's cells
    /// follow the viewport's size, and its grid scale is the render scale
    /// (docs/archive/projects/quality-governor.md).
    RenderScale,
    /// The video export's upscaler (`ExportRung`): a flame or escape-time
    /// frame rendered below the video's size is drawn up by it. A
    /// simulation's frame is not: its bound grid scales instead, and its
    /// own resolve draws the cells.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    ExportUpscaler,
    /// The video export's cap on escape-time supersampling.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    ExportEscapeAntialiasing,
    /// The video export's note on what its render scale does to a
    /// simulation's grid.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    ExportSimGrid,
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
        C::ChaosGame
        | C::TonemapPresets
        | C::SpatialFilter
        | C::DensityLevels
        | C::DeepZoom
        | C::ColorMode => {
            if flame {
                Vis::Show
            } else {
                Vis::Hide
            }
        }
        C::FocusedRendering => match m {
            RenderMode::TwoD => Vis::Show,
            RenderMode::ThreeD => Vis::Grey(TWO_D_ONLY),
            RenderMode::Escape | RenderMode::Simulation => Vis::Hide,
        },
        C::ViewNavigation => Vis::Show,
        C::ExportUpscaler => {
            if matches!(m, RenderMode::Simulation) {
                Vis::Hide
            } else {
                Vis::Show
            }
        }
        C::ExportEscapeAntialiasing => {
            if matches!(m, RenderMode::Escape) {
                Vis::Show
            } else {
                Vis::Hide
            }
        }
        C::ExportSimGrid => {
            if matches!(m, RenderMode::Simulation) {
                Vis::Show
            } else {
                Vis::Hide
            }
        }
        C::RenderScale => {
            if matches!(m, RenderMode::Simulation) {
                Vis::Grey(SIM_GRID_SCALE)
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

/// Is fly mode offered for the camera the viewport shows?
///
/// A question about the CAMERA, not the render mode: escape mode is 3D
/// for one formula and a terrain, a simulation for its terrain. Every 3D
/// camera flies (`camera::fly`, camera-unification C4); a 2D view has no
/// eye to fly.
pub fn fly_mode(kind: crate::camera::gesture::ViewKind) -> Vis {
    use crate::camera::gesture::ViewKind as K;
    match kind {
        K::Flame3d | K::Solid | K::EscapeTerrain | K::SimTerrain => Vis::Show,
        K::Flame2d | K::EscapePlane | K::Sim2d => Vis::Hide,
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
    // Every lit 3D view's lighting, shadows, material and surface
    // (3d-look-and-tools P3; it was the View panel's Lighting & Material
    // section from camera-unification C5 until then).
    row(PanelType::SolidLighting, "menu.window_solid_lighting"),
    row(PanelType::Escape, "menu.window_escape"),
    row(PanelType::Simulation, "menu.window_simulation"),
    row(PanelType::Textures, "menu.window_textures"),
    row(PanelType::FormulaEditor, "menu.window_formula_editor"),
    row(PanelType::HybridDesigner, "menu.window_hybrid_designer"),
    row(PanelType::Transforms, "menu.window_transforms"),
    row(PanelType::TriangleEditor, "menu.window_triangle_editor"),
    row(PanelType::Colors, "menu.window_colors"),
    row(PanelType::PaletteEditor, "menu.window_palette_editor"),
    row(PanelType::PaletteLibrary, "menu.window_palette_library"),
    row(PanelType::FractalBrowser, "menu.window_fractal_browser"),
    row(PanelType::History, "menu.window_history"),
    row(PanelType::Animation, "menu.window_animation"),
    row(PanelType::Paths, "menu.window_paths"),
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
/// bottom. Paths, Random Generator and Account are absent too.
pub static COMPACT_WINDOW_MENU: &[PanelType] = &[
    PanelType::Transforms,
    PanelType::TriangleEditor,
    PanelType::Colors,
    PanelType::View,
    PanelType::SolidLighting,
    PanelType::Rendering,
    PanelType::FractalBrowser,
    PanelType::Variations,
    PanelType::Subflames,
    PanelType::Scripts,
    PanelType::Escape,
    PanelType::Simulation,
    PanelType::Textures,
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

    /// Each view's kinds of light: a point where the world holds still
    /// (a solid), a headlamp in every escape and simulation 3D view, and
    /// a flame's shade pass directional alone.
    #[test]
    fn each_view_offers_its_kinds_of_light() {
        use crate::camera::gesture::ViewKind;
        use crate::config::LightKind;
        for view in [ViewKind::Flame2d, ViewKind::Flame3d, ViewKind::EscapePlane, ViewKind::Solid, ViewKind::EscapeTerrain, ViewKind::Sim2d, ViewKind::SimTerrain] {
            assert!(light_kind(LightKind::Directional, view).is_show(), "{view:?}");
        }
        assert!(light_kind(LightKind::Point, ViewKind::Solid).is_show());
        assert!(!light_kind(LightKind::Point, ViewKind::EscapeTerrain).is_show());
        assert!(!light_kind(LightKind::Point, ViewKind::Flame3d).is_show());
        for view in [ViewKind::Solid, ViewKind::EscapeTerrain, ViewKind::SimTerrain] {
            assert!(light_kind(LightKind::Headlamp, view).is_show(), "{view:?}");
        }
        assert!(!light_kind(LightKind::Headlamp, ViewKind::Flame3d).is_show());
    }
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
        PanelType::Paths,
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
        PanelType::Textures,
        PanelType::FormulaEditor,
        PanelType::HybridDesigner,
    ];

    /// The list above is the whole enum. `PanelType` has no iterator,
    /// so this is the closest thing to one -- and `Display` panics on
    /// nothing, so a missing variant shows up as a count mismatch the
    /// moment someone adds one without updating the tests.
    /// Lighting follows the CONFIG, not the mode (D2).
    ///
    /// Escape mode is not three-dimensional — one formula in it is —
    /// so a rule written against the mode would offer the lighting
    /// panel over a Mandelbrot, where it steers nothing, and withhold
    /// it from the solid marcher, which reads these very settings.
    #[test]
    fn lighting_follows_the_formula_in_escape_mode() {
        use crate::scene::transforms::RenderMode as M;

        let solid_of = |formula: &str| {
            let mut c = crate::config::FractalConfig::default();
            c.render_mode = M::Escape;
            c.escape.formula = formula.to_string();
            Solid::of(&c)
        };
        assert_eq!(solid_of("ifs_flame_3d"), Solid::Yes);
        assert_eq!(solid_of("ifs_flame"), Solid::No, "the planar walk renders no surface");
        assert_eq!(solid_of("mandelbrot"), Solid::No);

        assert!(panel(PanelType::SolidLighting, M::Escape, Solid::Yes).is_active());
        assert!(!panel(PanelType::SolidLighting, M::Escape, Solid::No).is_active());
        // The flame side is the same question, answered by the same flag.
        assert!(panel(PanelType::SolidLighting, M::ThreeD, Solid::Yes).is_active());
        // TwoD never reports Solid::Yes, so the flag alone deciding is
        // not a licence for the 2D mode to show it -- `Solid::of` is.
        assert_eq!(Solid::of(&crate::config::FractalConfig::default()), Solid::No);
        // An escape plane drawn as a 3D terrain is lit like a solid.
        let mut terrain = crate::config::FractalConfig::default();
        terrain.render_mode = RenderMode::Escape;
        assert_eq!(Solid::of(&terrain), Solid::No);
        terrain.escape.terrain.enabled = true;
        assert_eq!(Solid::of(&terrain), if cfg!(feature = "terrain") { Solid::Yes } else { Solid::No });
        // And a simulation's.
        let mut sim = crate::config::FractalConfig::default();
        sim.render_mode = RenderMode::Simulation;
        assert_eq!(Solid::of(&sim), Solid::No);
        sim.sim.terrain.enabled = true;
        assert_eq!(Solid::of(&sim), if cfg!(feature = "terrain") { Solid::Yes } else { Solid::No });

        // A 3D flame is the other thing that renders a surface, and
        // `Solid::of` has to say so or the two answers disagree.
        let mut flame3d = crate::config::FractalConfig::default();
        flame3d.render_mode = M::ThreeD;
        assert_eq!(Solid::of(&flame3d), Solid::Yes);
    }

    #[test]
    fn the_test_list_covers_every_panel() {
        let mut seen: Vec<String> = ALL_PANELS.iter().map(|p| format!("{p:?}")).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), ALL_PANELS.len(), "duplicate entry in ALL_PANELS");
        assert_eq!(ALL_PANELS.len(), 32, "a panel was added; decide what it means per mode");
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
        Control::DeepZoom,
        Control::ColorMode,
        Control::ViewNavigation,
        Control::FocusedRendering,
        Control::RenderScale,
        Control::ExportUpscaler,
        Control::ExportEscapeAntialiasing,
        Control::ExportSimGrid,
    ];

    /// Every reason a panel or control is greyed names a string that
    /// exists. `t!` returns the key itself when it is missing, which
    /// is how this catches a typo.
    #[test]
    fn every_reason_resolves_to_real_text() {
        let mut checked = 0;
        for key in ["visibility.inactive_heading", "visibility.switch_to", CHAOS_GAME] {
            assert_ne!(t!(key).as_ref(), key, "missing locale key {key}");
        }
        for m in RenderMode::ALL {
            for p in ALL_PANELS {
                if let PanelUse::Inactive { reason: key, .. } = panel(*p, *m, Solid::No) {
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

    /// Every mode navigates its view: a simulation's too, since it has
    /// one (`sim.view`).
    #[test]
    fn every_mode_navigates_its_view() {
        for m in RenderMode::ALL {
            assert!(control(Control::ViewNavigation, *m, ToneMapMode::Linear).is_show(), "{m:?}");
        }
    }

    /// Every control is available in both flame modes under the
    /// logarithmic mapping, except the orbit cache and the video export's
    /// antialiasing cap, which are escape's alone and do nothing in a
    /// flame, the export's note on a simulation's grid, and Focused
    /// Rendering in 3D, whose planners work in the plane.
    #[test]
    fn the_flame_modes_offer_every_control_but_the_orbit_cache() {
        let elsewhere = [Control::OrbitCache, Control::ExportEscapeAntialiasing, Control::ExportSimGrid];
        for m in [RenderMode::TwoD, RenderMode::ThreeD] {
            for c in ALL_CONTROLS {
                let want = !elsewhere.contains(c) && !(*c == Control::FocusedRendering && m == RenderMode::ThreeD);
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
            assert!(hidden(Control::DeepZoom), "{m:?} deep zoom");
            assert!(hidden(Control::ColorMode), "{m:?} colour mode");
            assert!(hidden(Control::FocusedRendering), "{m:?} focused rendering");
            assert!(
                control(Control::ViewNavigation, m, ToneMapMode::Logarithmic).is_show(),
                "{m:?} has its own view to navigate"
            );
            assert!(greyed(Control::TonemapMode), "{m:?} tone map mode");
            assert!(greyed(Control::LogOnlyTone), "{m:?} log-only tone");
            assert!(greyed(Control::AlphaBlendCurve), "{m:?} alpha blend");
        }
        // The controls a non-flame mode gains: escape-time's orbit cache
        // and the video export's antialiasing cap, a simulation's note on
        // its grid. A simulation's export frame is not drawn up.
        let shown = |c: Control, m: RenderMode| control(c, m, ToneMapMode::Linear).is_show();
        assert!(shown(Control::OrbitCache, RenderMode::Escape));
        assert!(!shown(Control::OrbitCache, RenderMode::Simulation));
        assert!(shown(Control::ExportEscapeAntialiasing, RenderMode::Escape));
        assert!(!shown(Control::ExportEscapeAntialiasing, RenderMode::Simulation));
        assert!(shown(Control::ExportSimGrid, RenderMode::Simulation));
        assert!(!shown(Control::ExportSimGrid, RenderMode::Escape));
        assert!(shown(Control::ExportUpscaler, RenderMode::Escape));
        assert!(!shown(Control::ExportUpscaler, RenderMode::Simulation));
    }

    /// The panels each mode does not use, pinned: a panel quietly
    /// changing status is the failure this module exists to prevent.
    /// (Every panel is DRAWN in every mode; these wear the banner.)
    #[test]
    fn each_mode_leaves_exactly_its_documented_panels_unused() {
        let unused = |m: RenderMode, solid: Solid| -> Vec<String> {
            let mut v: Vec<String> = ALL_PANELS
                .iter()
                .filter(|p| !panel(**p, m, solid).is_active())
                .map(|p| format!("{p:?}"))
                .collect();
            v.sort();
            v
        };
        fn sorted(mut v: Vec<&str>) -> Vec<String> {
            v.sort();
            v.into_iter().map(String::from).collect()
        }
        let engines = ["Escape", "Simulation", "Textures", "HybridDesigner"];
        let mut two_d = engines.to_vec();
        two_d.push("SolidLighting");
        assert_eq!(unused(RenderMode::TwoD, Solid::No), sorted(two_d), "2D");
        assert_eq!(unused(RenderMode::ThreeD, Solid::Yes), sorted(engines.to_vec()), "3D");
        // The transform editors are NOT here: escape mode D renders
        // the flame's attractor as a distance field, so editing a
        // transform edits the picture.
        let flame_only = ["XaosEditor", "Subflames", "Paths"];
        let mut escape = vec!["Simulation"];
        escape.extend(flame_only);
        assert_eq!(unused(RenderMode::Escape, Solid::Yes), sorted(escape), "Escape");
        let mut sim = vec!["Escape", "Textures", "HybridDesigner"];
        sim.extend(flame_only);
        assert_eq!(unused(RenderMode::Simulation, Solid::Yes), sorted(sim), "Simulation");
    }

    /// An unused panel's buttons lead where it is used: every mode it
    /// offers makes it act. And each mode's editor acts in its mode --
    /// a mode change brings it forward (`render_mode::main_panel_for`).
    #[test]
    fn an_unused_panel_leads_where_it_acts() {
        for m in RenderMode::ALL {
            for p in ALL_PANELS {
                for solid in [Solid::No, Solid::Yes] {
                    if let PanelUse::Inactive { enter, .. } = panel(*p, *m, solid) {
                        for to in enter {
                            assert_ne!(to, m, "{p:?} in {m:?} offers its own mode");
                            // Entering 3D from 2D makes a surface.
                            let solid_there = if *to == RenderMode::ThreeD { Solid::Yes } else { solid };
                            assert!(panel(*p, *to, solid_there).is_active(), "{p:?}: {m:?} leads to {to:?}, where it does not act");
                        }
                    }
                }
            }
            let main = super::super::render_mode::main_panel_for(*m);
            assert!(panel(main, *m, Solid::No).is_active(), "{m:?}'s editor");
        }
        // The engines' editors lead into their engines from every mode.
        for m in [RenderMode::TwoD, RenderMode::ThreeD, RenderMode::Simulation] {
            assert_eq!(panel(PanelType::Escape, m, Solid::No), PanelUse::Inactive { reason: ESCAPE_ENGINE, enter: ESCAPE });
        }
    }

    /// Fly mode is every 3D camera's and no 2D view's.
    #[test]
    fn every_3d_camera_flies() {
        use crate::camera::gesture::ViewKind as K;
        for k in [K::Flame2d, K::Flame3d, K::EscapePlane, K::Solid, K::EscapeTerrain, K::Sim2d, K::SimTerrain] {
            assert_eq!(fly_mode(k).is_show(), k.is_3d(), "{k:?}");
        }
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

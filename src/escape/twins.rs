//! The built-in formulas' twins: each a Rhai formula that renders as its
//! built-in (plan, phase 4 step 4), which the Escape panel's Edit as
//! Formula puts in the built-in's place and opens in the Formula Editor
//! as the user's own to change.
//!
//! A twin keeps its built-in's parameters by name (the config's values
//! carry over), reads the panel's Bailout through its `bailout`
//! parameter and the Julia toggle through its context, and declares its
//! degree at infinity so the smooth colourings divide by the same power.
//! What a built-in has besides its step -- Mann damping, the biomorph
//! test, its native deep-zoom tiers and their series approximation --
//! the twin has not: it deep-zooms by derived equations
//! (`lang::delta`), every family, a real power by its difference form.
//! Phoenix, Manowar and Spider carry a value besides z (the previous z,
//! the moving c), which the derived equations follow (phase 4, step 5);
//! Littlewood's greedy digit is a branch on the iterate, so its twin
//! deep-zooms where the built-in does not. The convergent ones (Magnet,
//! Nova, Novaretti, Newton) test their settling, `|z - prev|^2` under
//! the built-ins' 1e-12, which sets `converged` as theirs does. Origami's
//! builds its lines in `prepare` (an array, in f32 as the built-in's) and
//! gives its paper's creases (`fn creases`).
//!
//! The sources are `assets/formulas/twins/*.rhai`; the GPU tests hold
//! each to its built-in (`escape::app_repro_test`).

/// Each twin: the built-in's registry name and the Rhai source.
pub const TWINS: &[(&str, &str)] = &[
    ("mandelbrot", include_str!("../../assets/formulas/twins/mandelbrot.rhai")),
    ("multibrot", include_str!("../../assets/formulas/twins/multibrot.rhai")),
    ("tricorn", include_str!("../../assets/formulas/twins/tricorn.rhai")),
    ("burning_ship", include_str!("../../assets/formulas/twins/burning_ship.rhai")),
    ("lambda", include_str!("../../assets/formulas/twins/lambda.rhai")),
    ("cactus", include_str!("../../assets/formulas/twins/cactus.rhai")),
    // Carrying a value besides z (step 5): the previous z, the moving c.
    ("phoenix", include_str!("../../assets/formulas/twins/phoenix.rhai")),
    ("manowar", include_str!("../../assets/formulas/twins/manowar.rhai")),
    ("spider", include_str!("../../assets/formulas/twins/spider.rhai")),
    // The greedy digit map: its choice a branch on the iterate.
    ("littlewood", include_str!("../../assets/formulas/twins/littlewood.rhai")),
    // Escaping on Re z or |Im z|, not |z| (order of work, item 2, batch 1).
    ("exponential", include_str!("../../assets/formulas/twins/exponential.rhai")),
    ("trig", include_str!("../../assets/formulas/twins/trig.rhai")),
    ("lambda_sine", include_str!("../../assets/formulas/twins/lambda_sine.rhai")),
    ("collatz", include_str!("../../assets/formulas/twins/collatz.rhai")),
    ("tetration", include_str!("../../assets/formulas/twins/tetration.rhai")),
    // Powers by parameters, a pole (item 2, batch 2).
    ("feather", include_str!("../../assets/formulas/twins/feather.rhai")),
    ("mcmullen", include_str!("../../assets/formulas/twins/mcmullen.rhai")),
    // Branches on the iterate, a fold, three that never escape (item 2,
    // batch 3).
    ("barnsley", include_str!("../../assets/formulas/twins/barnsley.rhai")),
    ("kaliset", include_str!("../../assets/formulas/twins/kaliset.rhai")),
    ("ducks", include_str!("../../assets/formulas/twins/ducks.rhai")),
    ("lattes", include_str!("../../assets/formulas/twins/lattes.rhai")),
    // Convergent: an orbit ends as it settles, or escapes (item 2,
    // batch 4); Novaretti's only settles.
    ("magnet", include_str!("../../assets/formulas/twins/magnet.rhai")),
    ("nova", include_str!("../../assets/formulas/twins/nova.rhai")),
    ("novaretti", include_str!("../../assets/formulas/twins/novaretti.rhai")),
    ("newton", include_str!("../../assets/formulas/twins/newton.rhai")),
    // Folded paper: its lines a table built per image, a fold a pass,
    // its creases (item 3).
    ("origami", include_str!("../../assets/formulas/twins/origami.rhai")),
];

/// The built-in colourings' twins (P7 step 2, docs/archive/projects/rhai-plugins.md):
/// each a Rhai colouring that renders as its built-in, which the Escape
/// panel's Edit as Rhai puts in the built-in's place. A twin keeps its
/// built-in's parameters by name; WGSL's `round` (halves to even) it
/// spells out, the dialect's rounding halves away from zero. Added in
/// batches, as the dialect learns what the rest read; the sources are
/// `assets/formulas/twins/colorings/*.rhai`.
pub const COLORING_TWINS: &[(&str, &str)] = &[
    // Batch 1: what a colouring could say already.
    ("escape_count", include_str!("../../assets/formulas/twins/colorings/escape_count.rhai")),
    ("orbit_trap", include_str!("../../assets/formulas/twins/colorings/orbit_trap.rhai")),
    ("sphere_average", include_str!("../../assets/formulas/twins/colorings/sphere_average.rhai")),
    ("decomposition", include_str!("../../assets/formulas/twins/colorings/decomposition.rhai")),
    ("basic", include_str!("../../assets/formulas/twins/colorings/basic.rhai")),
    ("gaussian_integer", include_str!("../../assets/formulas/twins/colorings/gaussian_integer.rhai")),
    ("itinerary", include_str!("../../assets/formulas/twins/colorings/itinerary.rhai")),
    // Batch 2: what the loop knows -- the escape fraction, the degree,
    // the orbit's ending, the step's previous iterate and c.
    ("smooth", include_str!("../../assets/formulas/twins/colorings/smooth.rhai")),
    ("orbit_average", include_str!("../../assets/formulas/twins/colorings/orbit_average.rhai")),
    ("stripe_average", include_str!("../../assets/formulas/twins/colorings/stripe_average.rhai")),
    ("magnitude_average", include_str!("../../assets/formulas/twins/colorings/magnitude_average.rhai")),
    ("root_basin", include_str!("../../assets/formulas/twins/colorings/root_basin.rhai")),
    ("triangle_inequality", include_str!("../../assets/formulas/twins/colorings/triangle_inequality.rhai")),
    ("period", include_str!("../../assets/formulas/twins/colorings/period.rhai")),
    ("position_average", include_str!("../../assets/formulas/twins/colorings/position_average.rhai")),
    ("curvature_average", include_str!("../../assets/formulas/twins/colorings/curvature_average.rhai")),
    ("velocity", include_str!("../../assets/formulas/twins/colorings/velocity.rhai")),
    ("threads", include_str!("../../assets/formulas/twins/colorings/threads.rhai")),
    ("exponential_smoothing", include_str!("../../assets/formulas/twins/colorings/exponential_smoothing.rhai")),
    // Batch 3: what `INFO` says beside the program -- a level clamped
    // rather than wrapped, and what picking it sets.
    ("position_map", include_str!("../../assets/formulas/twins/colorings/position_map.rhai")),
    ("external_rays", include_str!("../../assets/formulas/twins/colorings/external_rays.rhai")),
    // Batch 4: the derivative, and the pixel it is measured in.
    ("distance_estimate", include_str!("../../assets/formulas/twins/colorings/distance_estimate.rhai")),
    ("normal_map", include_str!("../../assets/formulas/twins/colorings/normal_map.rhai")),
    // Batch 5: a colour as the result (`fn color`), the palette in the
    // colouring.
    ("rainbow_fringe", include_str!("../../assets/formulas/twins/colorings/rainbow_fringe.rhai")),
    ("infinite_waves", include_str!("../../assets/formulas/twins/colorings/infinite_waves.rhai")),
    ("voc_traps", include_str!("../../assets/formulas/twins/colorings/voc_traps.rhai")),
    ("direct_traps", include_str!("../../assets/formulas/twins/colorings/direct_traps.rhai")),
    // Batch 6: the config's texture, read in the loop, and the colour's
    // coverage.
    ("image_trap", include_str!("../../assets/formulas/twins/colorings/image_trap.rhai")),
];

/// The built-in mode D solids' twins (P7 step 4,
/// docs/archive/projects/rhai-solids-fields.md): each a Rhai solid that renders as
/// its built-in, which the Escape panel's Edit as Rhai puts in the
/// built-in's place. A twin keeps its built-in's parameters by name, its
/// arithmetic step for step, and the colourings' readings (`dr`, the
/// `address` its built-in accumulates); its `prepare` gives the built-in's
/// ball. The sources are `assets/formulas/twins/solids/*.rhai`; the GPU
/// tests hold each to its built-in (`escape::ifs`).
pub const SOLID_TWINS: &[(&str, &str)] = &[
    ("mandelbox_solid", include_str!("../../assets/formulas/twins/solids/mandelbox.rhai")),
    ("mandelbulb_solid", include_str!("../../assets/formulas/twins/solids/mandelbulb.rhai")),
    ("quaternion_julia_solid", include_str!("../../assets/formulas/twins/solids/quaternion_julia.rhai")),
    ("kifs_solid", include_str!("../../assets/formulas/twins/solids/kifs.rhai")),
];

/// The built-in mode B fields' twins and their colourings' (P7 step 4,
/// docs/archive/projects/rhai-solids-fields.md): each a Rhai field or field
/// colouring that renders as its built-in, which the Escape panel's Edit
/// as Rhai puts in its place. Sources in `assets/formulas/twins/fields/`.
pub const FIELD_TWINS: &[(&str, &str)] = &[
    ("weierstrass", include_str!("../../assets/formulas/twins/fields/weierstrass.rhai")),
    ("markus_lyapunov", include_str!("../../assets/formulas/twins/fields/markus_lyapunov.rhai")),
    ("standard_map_ftle", include_str!("../../assets/formulas/twins/fields/standard_map_ftle.rhai")),
];

pub const FIELD_COLORING_TWINS: &[(&str, &str)] = &[
    ("field_value", include_str!("../../assets/formulas/twins/fields/value.rhai")),
    ("field_diverging", include_str!("../../assets/formulas/twins/fields/diverging.rhai")),
    ("field_hillshade", include_str!("../../assets/formulas/twins/fields/hillshade.rhai")),
];

/// The built-in field `name`'s twin as the config carries it.
pub fn field_custom_source(name: &str) -> Option<crate::config::escape::CustomSource> {
    carried(name, FIELD_TWINS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)?)
}

/// The built-in field colouring `name`'s twin as the config carries it.
pub fn field_coloring_custom_source(name: &str) -> Option<crate::config::escape::CustomSource> {
    carried(name, FIELD_COLORING_TWINS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)?)
}

/// The twin of the built-in solid `name`, when it has one.
pub fn solid_source(name: &str) -> Option<&'static str> {
    SOLID_TWINS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// The solid's twin as the config carries it.
pub fn solid_custom_source(name: &str) -> Option<crate::config::escape::CustomSource> {
    carried(name, solid_source(name)?)
}

/// The twin of the built-in formula `name`, when it has one.
pub fn source(name: &str) -> Option<&'static str> {
    TWINS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// The twin as the config carries it.
pub fn custom_source(name: &str) -> Option<crate::config::escape::CustomSource> {
    carried(name, source(name)?)
}

/// The twin of the built-in colouring `name`, when it has one.
pub fn coloring_source(name: &str) -> Option<&'static str> {
    COLORING_TWINS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// The colouring's twin as the config carries it.
pub fn coloring_custom_source(name: &str) -> Option<crate::config::escape::CustomSource> {
    carried(name, coloring_source(name)?)
}

fn carried(name: &str, text: &str) -> Option<crate::config::escape::CustomSource> {
    Some(crate::config::escape::CustomSource {
        dialect: crate::config::escape::CustomDialect::Rhai,
        entry: super::lang::rhai::title(text).unwrap_or_else(|| name.to_string()),
        source: text.to_string(),
        variations: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::lang::builtins::C64;
    use super::super::lang::interp::{perturb_holds, Env, ParamValues};
    use super::super::lang::{delta, perturb, rhai};
    use super::*;

    /// Each colouring twin compiles as the app compiles it (its WGSL
    /// validated), with its built-in's name in the panel, its parameters
    /// exactly -- names, captions, hints, kinds, defaults, ranges,
    /// choices, a log slider -- its features as the built-in's (an
    /// orbit accumulator, the interior coloured, period detection, a
    /// bounded value, the derivative, a colour, the palette or the texture
    /// in the loop, the coverage, UF's loop order), and what picking it
    /// sets: the recommended bailout
    /// and the values a fresh pick takes.
    #[test]
    fn every_colouring_twin_compiles_as_its_builtin() {
        use super::super::ColoringFeature as F;
        for (name, _) in COLORING_TWINS {
            let builtin = super::super::get_coloring(name);
            assert_eq!(builtin.name, *name);
            let src = coloring_custom_source(name).expect("a twin");
            let key = super::super::custom::name_of(super::super::lang::EntryKind::Coloring, &src);
            let twin = super::super::custom::compile_coloring(&src, &key)
                .unwrap_or_else(|d| panic!("{name}: {:?}", d.iter().map(|x| x.render(&src.source)).collect::<Vec<_>>()));
            assert_eq!(twin.display_name, builtin.display_name, "{name}: its title");
            assert_eq!(twin.parameters, builtin.parameters, "{name}: its parameters");
            for f in [
                F::NeedsOrbitAccum,
                F::ColorsInterior,
                F::NeedsPeriod,
                F::Bounded,
                F::NeedsDerivative,
                F::DirectColor,
                F::PaletteInLoop,
                F::TextureInLoop,
                F::DirectAlpha,
                F::SkipsEscapingIterate,
            ] {
                assert_eq!(twin.features.contains(&f), builtin.features.contains(&f), "{name}: {f:?}");
            }
            // What picking it sets (`INFO.bailout`, `INFO.pick`).
            assert_eq!(twin.recommended_bailout, builtin.recommended_bailout, "{name}: its recommended bailout");
            assert_eq!(twin.pick_params, builtin.pick_params, "{name}: what a fresh pick sets");
        }
    }

    /// **Each solid twin compiles as its built-in** (docs/projects/rhai-
    /// solids-fields.md): registered as a mode D solid with the built-in's
    /// title and every parameter it has -- names, captions, hints, kinds,
    /// defaults, ranges, choices -- the template's and Colour Depth
    /// included, in any order (a config carries them by name), and none
    /// else; its default colouring the built-in's; and `prepare`'s ball
    /// the built-in's, at the defaults and with each parameter moved.
    #[test]
    fn every_solid_twin_compiles_as_its_builtin() {
        for (name, _) in SOLID_TWINS {
            let builtin = super::super::ifs::get_ifs(name).expect("a built-in solid");
            let src = solid_custom_source(name).expect("a twin");
            let key = super::super::custom::name_of(super::super::lang::EntryKind::Solid, &src);
            let twin = super::super::custom::compile_solid(&src, &key)
                .unwrap_or_else(|d| panic!("{name}: {:?}", d.iter().map(|x| x.render(&src.source)).collect::<Vec<_>>()));
            assert_eq!(twin.display_name, builtin.display_name, "{name}: its title");
            assert_eq!(twin.default_coloring, builtin.default_coloring, "{name}: its default colouring");
            assert_eq!(twin.march_inside, builtin.march_inside, "{name}: its march");
            for b in builtin.parameters {
                let t = twin.parameters.iter().find(|x| x.name == b.name).unwrap_or_else(|| panic!("{name}: no `{}`", b.name));
                assert_eq!(t, b, "{name}.{}", b.name);
            }
            assert_eq!(twin.parameters.len(), builtin.parameters.len(), "{name}: no other parameters ({:?})", twin.parameters.iter().map(|p| &p.name).collect::<Vec<_>>());
            // The ball, as `standalone_radius` gives the built-in's: to f32,
            // as `prepare`'s values are handed on (and the shader reads
            // either ball in f32).
            let mut esc = crate::config::escape::EscapeConfig::default();
            let ball = |esc: &crate::config::escape::EscapeConfig| {
                (super::super::ifs::standalone_radius(builtin, esc), super::super::ifs::standalone_radius(twin, esc))
            };
            let same = |b: f64, t: f64| b as f32 == t as f32;
            let (b, t) = ball(&esc);
            assert!(same(b, t), "{name}: the ball at the defaults: {b} against {t}");
            for p in builtin.parameters.iter().skip(7) {
                let (lo, hi) = p.slot_range();
                for v in [lo, hi, 0.5 * (lo + hi) + 0.137] {
                    esc.formula_params.clear();
                    esc.formula_params.insert(p.name.to_string(), v);
                    if p.name == "mode" {
                        esc.formula_params.insert("cx".into(), 1.3);
                        esc.formula_params.insert("cz".into(), -0.7);
                    }
                    let (b, t) = ball(&esc);
                    assert!(same(b, t), "{name}: the ball at {} = {v}: {b} against {t}", p.name);
                }
            }
        }
    }

    /// **Each field twin and field colouring twin compiles as its built-in**
    /// (docs/archive/projects/rhai-solids-fields.md): registered with the
    /// built-in's title and parameters exactly, a field's gradient flag
    /// and default colouring the built-in's.
    #[test]
    fn every_field_twin_compiles_as_its_builtin() {
        use super::super::lang::EntryKind;
        for (name, _) in FIELD_TWINS {
            let builtin = super::super::fields::FIELDS.iter().find(|f| f.name == *name).expect("a built-in field");
            let src = field_custom_source(name).expect("a twin");
            let key = super::super::custom::name_of(EntryKind::Field, &src);
            let twin = super::super::custom::compile_field(&src, &key)
                .unwrap_or_else(|d| panic!("{name}: {:?}", d.iter().map(|x| x.render(&src.source)).collect::<Vec<_>>()));
            assert_eq!(twin.display_name, builtin.display_name, "{name}: its title");
            assert_eq!(twin.has_gradient, builtin.has_gradient, "{name}: its gradient");
            assert_eq!(twin.default_coloring, builtin.default_coloring, "{name}: its default colouring");
            assert_eq!(twin.parameters, builtin.parameters, "{name}: its parameters");
            assert!(twin.own_state, "{name}: its own state");
        }
        for (name, _) in FIELD_COLORING_TWINS {
            let builtin = super::super::fields::FIELD_COLORINGS.iter().find(|c| c.name == *name).expect("a built-in field colouring");
            let src = field_coloring_custom_source(name).expect("a twin");
            let key = super::super::custom::name_of(EntryKind::FieldColoring, &src);
            let twin = super::super::custom::compile_field_coloring(&src, &key)
                .unwrap_or_else(|d| panic!("{name}: {:?}", d.iter().map(|x| x.render(&src.source)).collect::<Vec<_>>()));
            assert_eq!(twin.display_name, builtin.display_name, "{name}: its title");
            assert_eq!(twin.parameters, builtin.parameters, "{name}: its parameters");
        }
    }

    /// The built-in Image Trap reads the config's texture as the formula
    /// language reads one (batch 6): its `image_trap_texture` is the
    /// language's `uf_texture`, function for function, so its twin and it
    /// agree -- and both agree across platforms, the filtering done by hand.
    #[test]
    fn the_image_trap_reads_the_texture_as_the_language_does() {
        let body = |src: &str, name: &str| {
            let at = src.find(&format!("fn {name}(")).unwrap_or_else(|| panic!("no fn {name}"));
            let end = src[at..].find("
}
").expect("its end");
            src[at + 3 + name.len()..at + end].to_string()
        };
        let builtin = super::super::get_coloring("image_trap").wgsl_accum;
        let lang = super::super::lang::wgsl::texture_wgsl("", "esc_texture");
        assert_eq!(body(builtin, "image_trap_texture"), body(&lang, "uf_texture"));
    }

    /// Each twin compiles, and keeps its built-in's parameters (names,
    /// defaults, ranges, choices) besides its `bailout`, and its degree.
    #[test]
    fn every_twin_compiles_with_its_builtins_parameters() {
        for (name, text) in TWINS {
            let p = rhai::program(text).unwrap_or_else(|d| panic!("{name}: {:?}", d.iter().map(|x| x.render(text)).collect::<Vec<_>>()));
            let builtin = super::super::get_formula(name);
            assert_eq!(builtin.name, *name);
            for b in builtin.parameters {
                let t = p.params.iter().find(|x| x.name == b.name).unwrap_or_else(|| panic!("{name}: no `{}`", b.name));
                assert_eq!(t.default.0 as f32, b.slot_default(), "{name}.{}", b.name);
                match b.options() {
                    None => assert_eq!((t.min.map(|v| v as f32), t.max.map(|v| v as f32)), (Some(b.slot_range().0), Some(b.slot_range().1)), "{name}.{}", b.name),
                    Some(o) => assert_eq!(t.choices, o.iter().map(str::to_string).collect::<Vec<_>>(), "{name}.{}", b.name),
                }
            }
            // A built-in that never escapes has no bailout, nor its twin.
            let escapes = !builtin.has_feature(super::super::FormulaFeature::NonEscaping);
            assert_eq!(p.params.iter().any(|x| x.name == "bailout"), escapes, "{name}: the panel's bailout");
            assert_eq!(p.params.len(), builtin.parameters.len() + usize::from(escapes), "{name}: no other parameters");
            // The degree, at the defaults and at a power.
            let mut slots: Vec<f32> = builtin.parameters.iter().map(|b| b.slot_default()).collect();
            let deg = |slots: &[f32]| {
                let params = ParamValues::from_slots(&p, slots);
                // Undeclared, 2: the smooth colourings' default, as the
                // built-ins' without a polynomial degree.
                super::super::lang::interp::degree_of(&p, &params, &Env::at(C64::ZERO, 1)).unwrap_or(2.0) as f32
            };
            // Through the smooth colourings' rule: 1 or less keeps 2.
            let used = |d: f32| if d > 1.0001 && d <= 1.0e6 { d } else { 2.0 };
            assert_eq!(used(deg(&slots)), super::super::escape_degree_of(builtin, &slots), "{name}: degree");
            if let Some(first) = slots.first_mut().filter(|_| builtin.parameters[0].name == "power") {
                *first = 5.0;
                assert_eq!(used(deg(&slots)), super::super::escape_degree_of(builtin, &slots), "{name}: degree at power 5");
            }
            assert!(custom_source(name).is_some_and(|s| s.entry == p.name));
        }
        // A field has none (mode B; the order of work, item 2).
        assert!(source("weierstrass").is_none());
    }

    /// Origami's twin builds the built-in's line table bit for bit (item
    /// 3, step 3e): its `prepare` computes in f32 an operation at a time
    /// (`to_f32`), as `formulas::origami_derived_lines` does in Rust --
    /// each line's endpoints fold through every earlier line, and in f64
    /// the tables parted by whole units past the first few lines.
    #[test]
    fn origamis_lines_are_the_builtins() {
        let p = rhai::program(source("origami").unwrap()).unwrap();
        for (seed, spread) in [(7.0f32, 2.0f32), (8.0, 2.0), (123.0, 1.3), (511.0, 0.37)] {
            let mut params = ParamValues::defaults(&p);
            params.values[0].0 = f64::from(seed);
            params.values[1].0 = f64::from(spread);
            let ours = super::super::lang::interp::globals(&p, &params, &Env::at(C64::ZERO, 32));
            let theirs = crate::escape::formulas::origami_derived_lines(&[seed, spread]);
            assert_eq!(ours, theirs, "seed {seed}, spread {spread}");
        }
    }

    /// Every twin deep-zooms by derived equations on both planes: a power
    /// given as a parameter by its difference form (step 6), the Tricorn
    /// at power 2 by its exact square.
    #[test]
    fn the_twins_derive_their_deep_zoom() {
        let holds = |name: &str, set: &[(usize, f64)], julia: bool| -> bool {
            let p = rhai::program(source(name).unwrap()).unwrap();
            let Some(d) = delta::with_derived_equations(&p) else { return false };
            if perturb::perturbation(&d).is_err() {
                return false;
            }
            let mut params = ParamValues::defaults(&d);
            for &(i, v) in set {
                params.values[i].0 = v;
            }
            let mut env = Env::at(C64::ZERO, 100);
            env.julia = julia;
            perturb_holds(&d, &params, &env)
        };
        for julia in [false, true] {
            assert!(holds("mandelbrot", &[], julia));
            assert!(holds("lambda", &[], julia));
            assert!(holds("cactus", &[], julia));
            assert!(holds("tricorn", &[], julia), "power 2");
            assert!(holds("tricorn", &[(0, 3.0)], julia), "power 3: a real power");
            for v in 0..6 {
                assert!(holds("burning_ship", &[(0, v as f64)], julia), "variant {v}");
            }
            assert!(holds("multibrot", &[], julia));
            assert!(holds("multibrot", &[(0, 2.5)], julia), "a power between");
            // Carrying the previous z, or the moving c (step 5).
            assert!(holds("phoenix", &[], julia));
            assert!(holds("manowar", &[], julia));
            assert!(holds("spider", &[], julia));
            // Branches on the iterate, a fold, never escaping (batch 3).
            for v in 0..3 {
                assert!(holds("barnsley", &[(0, v as f64)], julia), "barnsley variant {v}");
            }
            for v in 0..2 {
                assert!(holds("kaliset", &[(0, v as f64)], julia), "kaliset {v}");
            }
            for v in 0..5 {
                assert!(holds("ducks", &[(0, v as f64)], julia), "ducks variant {v}");
            }
            for v in 0..3 {
                assert!(holds("lattes", &[(0, v as f64)], julia), "lattes variant {v}");
            }
            // Convergent (item 2, batch 4): Newton by every function and
            // scheme.
            for v in 0..2 {
                assert!(holds("magnet", &[(0, v as f64)], julia), "magnet variant {v}");
            }
            assert!(holds("nova", &[], julia), "nova");
            assert!(holds("novaretti", &[], julia), "novaretti");
            for func in 0..6 {
                for scheme in 0..3 {
                    assert!(holds("newton", &[(1, scheme as f64), (4, func as f64)], julia), "newton function {func} scheme {scheme}");
                }
            }
            // Powers by parameters, a pole (item 2, batch 2).
            assert!(holds("feather", &[], julia), "feather");
            assert!(holds("mcmullen", &[], julia), "mcmullen");
            // Escaping on Re z or |Im z| (item 2, batch 1).
            for name in ["exponential", "lambda_sine", "collatz", "tetration"] {
                assert!(holds(name, &[], julia), "{name}");
            }
            for v in 0..2 {
                assert!(holds("trig", &[(0, v as f64)], julia), "trig variant {v}");
            }
            // The greedy digit, a branch on the iterate, by every set.
            for d in 0..3 {
                assert!(holds("littlewood", &[(0, d as f64)], julia), "littlewood digit set {d}");
            }
        }
    }
}

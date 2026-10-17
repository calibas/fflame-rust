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
//! the built-ins' 1e-12, which sets `converged` as theirs does.
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
];

/// The twin of the built-in formula `name`, when it has one.
pub fn source(name: &str) -> Option<&'static str> {
    TWINS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// The twin as the config carries it.
pub fn custom_source(name: &str) -> Option<crate::config::escape::CustomSource> {
    let text = source(name)?;
    Some(crate::config::escape::CustomSource {
        dialect: crate::config::escape::CustomDialect::Rhai,
        entry: super::lang::rhai::title(text).unwrap_or_else(|| name.to_string()),
        source: text.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::lang::builtins::C64;
    use super::super::lang::interp::{perturb_holds, Env, ParamValues};
    use super::super::lang::{delta, perturb, rhai};
    use super::*;

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
                assert_eq!(t.default.0 as f32, b.default, "{name}.{}", b.name);
                if b.choices.is_empty() {
                    assert_eq!((t.min.map(|v| v as f32), t.max.map(|v| v as f32)), (Some(b.min), Some(b.max)), "{name}.{}", b.name);
                } else {
                    let want: Vec<String> = b.choices.iter().map(|c| c.to_string()).collect();
                    assert_eq!(t.choices, want, "{name}.{}", b.name);
                }
            }
            // A built-in that never escapes has no bailout, nor its twin.
            let escapes = !builtin.has_feature(super::super::FormulaFeature::NonEscaping);
            assert_eq!(p.params.iter().any(|x| x.name == "bailout"), escapes, "{name}: the panel's bailout");
            assert_eq!(p.params.len(), builtin.parameters.len() + usize::from(escapes), "{name}: no other parameters");
            // The degree, at the defaults and at a power.
            let mut slots: Vec<f32> = builtin.parameters.iter().map(|b| b.default).collect();
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

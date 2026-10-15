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
//! (`lang::delta`), the families the derivation covers (all but a
//! Multibrot's or a multicorn's real power).
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
            assert!(p.params.iter().any(|x| x.name == "bailout"), "{name}: the panel's bailout");
            assert_eq!(p.params.len(), builtin.parameters.len() + 1, "{name}: no other parameters");
            // The degree, at the defaults and at a power.
            let mut slots: Vec<f32> = builtin.parameters.iter().map(|b| b.default).collect();
            let deg = |slots: &[f32]| {
                let params = ParamValues::from_slots(&p, slots);
                super::super::lang::interp::degree_of(&p, &params, &Env::at(C64::ZERO, 1)).unwrap() as f32
            };
            assert_eq!(deg(&slots), super::super::escape_degree_of(builtin, &slots), "{name}: degree");
            if let Some(first) = slots.first_mut().filter(|_| builtin.parameters[0].name == "power") {
                *first = 5.0;
                assert_eq!(deg(&slots), 5.0, "{name}: degree at power 5");
            }
            assert!(custom_source(name).is_some_and(|s| s.entry == p.name));
        }
        assert!(source("phoenix").is_none());
    }

    /// The families the derivation covers deep-zoom by derived equations
    /// on both planes: the Mandelbrot, the Tricorn at power 2, every
    /// Burning Ship variant, Lambda, Cactus. A power given as a parameter
    /// does not.
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
            assert!(!holds("tricorn", &[(0, 3.0)], julia), "power 3: a real power");
            for v in 0..6 {
                assert!(holds("burning_ship", &[(0, v as f64)], julia), "variant {v}");
            }
            assert!(!holds("multibrot", &[], julia));
        }
    }
}

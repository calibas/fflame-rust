//! Escape-time fractal rendering (Mandelbrot and kin).
//!
//! The parallel system to the flame pipeline described in
//! `docs/projects/escape-time-fractals.md`. Where the flame renderer
//! runs a chaos game into a histogram, this mode evaluates a per-pixel
//! iteration directly — one compute pass, no accumulation loop — and
//! writes an `Rgba32Float` image shaped exactly like the flame
//! accumulator (`rgb` = mean color, `a` = density), so the existing
//! tonemap → effects → readback tail consumes it unchanged.
//!
//! Architecture mirrors the variation system deliberately:
//! * [`FormulaDef`] / [`ColoringDef`] are `static` definitions with
//!   inline WGSL and self-describing parameters (the panel UI will
//!   auto-generate sliders from them, the way variation params do).
//! * Registries are ordered slices; **append-only**, name-addressed.
//! * The assembler splices exactly one formula and one coloring into a
//!   small template — per-combination shaders, cached by the renderer.
//!
//! Unlike variations there is no index-mapping problem: a pipeline
//! holds ONE formula and ONE coloring, so WGSL function names are
//! fixed (`formula_step`, `coloring_map`) rather than prefixed.

pub mod assembler;
pub mod colorings;
pub mod formulas;
pub mod renderer;

pub use renderer::EscapeRenderer;

/// A parameter a formula or coloring exposes. Same shape as variation
/// parameters, minus the type zoo — everything is a float slot in v1
/// (integers ride as floats the way variation `Integer` params do).
pub struct EscapeParamDef {
    /// Key inside `EscapeConfig::formula_params` / `coloring_params`.
    pub name: &'static str,
    pub display_name: &'static str,
    pub default: f32,
    pub min: f32,
    pub max: f32,
    pub tooltip: &'static str,
}

/// A formula: the iterated step `z ← f(z, c)`.
///
/// The WGSL must define
/// `fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32>`
/// and may read its parameters via `fparam(slot)` — slots are assigned
/// in `parameters` order. Helper functions must be name-prefixed
/// (`fn myformula_helper`) since colorings share the module namespace.
pub struct FormulaDef {
    /// Registry name — the string `EscapeConfig::formula` stores.
    pub name: &'static str,
    pub display_name: &'static str,
    pub parameters: &'static [EscapeParamDef],
    pub wgsl: &'static str,
}

/// A coloring: maps the per-pixel orbit summary to a palette position.
///
/// The WGSL must define
/// `fn coloring_map(z: vec2<f32>, n: u32, escaped: bool) -> f32`
/// returning a palette coordinate (wrapped into [0,1) by the caller),
/// reading parameters via `cparam(slot)`. Interior pixels (`!escaped`)
/// are currently painted black by the template before `coloring_map`
/// is consulted; interior colorings arrive with the phase-2 set.
pub struct ColoringDef {
    /// Registry name — the string `EscapeConfig::coloring` stores.
    pub name: &'static str,
    pub display_name: &'static str,
    pub parameters: &'static [EscapeParamDef],
    pub wgsl: &'static str,
}

/// Ordered formula registry. **Append-only** — UI ordering and any
/// future stable-ID scheme both read this order.
pub static FORMULAS: &[&FormulaDef] = &[&formulas::MANDELBROT];

/// Ordered coloring registry. **Append-only.**
pub static COLORINGS: &[&ColoringDef] = &[&colorings::ESCAPE_COUNT, &colorings::SMOOTH];

/// Look up a formula by name. An unknown name renders the default
/// formula with a warning rather than failing the load — the same
/// forward-compatibility posture `EscapeConfig::formula` documents
/// (a file from a newer build should degrade, not error).
pub fn get_formula(name: &str) -> &'static FormulaDef {
    FORMULAS
        .iter()
        .find(|f| f.name == name)
        .copied()
        .unwrap_or_else(|| {
            log::warn!("Unknown escape formula '{name}', rendering '{}'", FORMULAS[0].name);
            FORMULAS[0]
        })
}

/// Look up a coloring by name, with the same unknown-name fallback.
pub fn get_coloring(name: &str) -> &'static ColoringDef {
    COLORINGS
        .iter()
        .find(|c| c.name == name)
        .copied()
        .unwrap_or_else(|| {
            log::warn!("Unknown escape coloring '{name}', using '{}'", COLORINGS[0].name);
            COLORINGS[0]
        })
}

/// Pack a keyed param map into slot order for one def, filling absent
/// keys with defaults. Slot `i` is `parameters[i]` — the contract the
/// WGSL `fparam`/`cparam` helpers index against.
pub fn pack_params(
    defs: &[EscapeParamDef],
    values: &std::collections::BTreeMap<String, f32>,
    out: &mut [f32],
) {
    for (i, def) in defs.iter().enumerate() {
        if i >= out.len() {
            // More params than GPU slots — a def bug, not a user state.
            log::error!("Escape param '{}' exceeds the {}-slot budget", def.name, out.len());
            break;
        }
        out[i] = values.get(def.name).copied().unwrap_or(def.default);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_names_are_unique_and_lookup_works() {
        let mut seen = std::collections::HashSet::new();
        for f in FORMULAS {
            assert!(seen.insert(f.name), "duplicate formula name {}", f.name);
            assert_eq!(get_formula(f.name).name, f.name);
        }
        seen.clear();
        for c in COLORINGS {
            assert!(seen.insert(c.name), "duplicate coloring name {}", c.name);
            assert_eq!(get_coloring(c.name).name, c.name);
        }
    }

    #[test]
    fn unknown_names_fall_back_instead_of_failing() {
        // A file from a newer build must degrade, not error.
        assert_eq!(get_formula("from_the_future").name, FORMULAS[0].name);
        assert_eq!(get_coloring("from_the_future").name, COLORINGS[0].name);
    }

    #[test]
    fn config_defaults_name_real_registry_entries() {
        // EscapeConfig::default() says "mandelbrot"/"smooth"; if either
        // string drifts from the registry the default config would
        // silently render the fallback.
        let esc = crate::config::escape::EscapeConfig::default();
        assert!(FORMULAS.iter().any(|f| f.name == esc.formula));
        assert!(COLORINGS.iter().any(|c| c.name == esc.coloring));
    }

    #[test]
    fn pack_params_fills_defaults_and_overrides_in_slot_order() {
        let mut values = std::collections::BTreeMap::new();
        values.insert("scale".to_string(), 0.25f32);
        let mut out = [0.0f32; 4];
        pack_params(colorings::SMOOTH.parameters, &values, &mut out);
        assert_eq!(out[0], 0.25); // overridden
    }
}

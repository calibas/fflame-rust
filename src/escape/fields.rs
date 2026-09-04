//! Mode B — field evaluation (plan §1, phase 3).
//!
//! No classification, no escape: each pixel evaluates a finite
//! analytic sum or a finite-orbit statistic and colors the resulting
//! scalar field. Cheapest of the three fragment modes; f32 is
//! trivially sufficient (the sums converge absolutely, the statistics
//! are averages).
//!
//! Same registry discipline as formulas/colorings: `static` defs with
//! inline WGSL, **append-only** ordered slices, name-addressed.
//! `EscapeConfig::formula` / `::coloring` name entries here exactly as
//! they name mode-A entries — the renderer routes on which registry
//! resolves the formula name.
//!
//! WGSL contract (template `FIELD_TEMPLATE` in assembler.rs):
//! - a field def defines
//!   `fn field_init(p: vec2<f32>) -> FieldState` and
//!   `fn field_step(n: u32, p: vec2<f32>, s: FieldState) -> FieldStep`
//!   where the template provides
//!   `struct FieldState { a: vec4<f32>, b: vec4<f32> }` (eight floats
//!   of per-pixel state — the FTLE tangent matrix needs more than a
//!   vec4) and
//!   `struct FieldStep { state: FieldState, value: f32, grad: vec2<f32> }`.
//!   The template accumulates `sum += value`, `grad += step.grad`.
//!   Params via `fparam(slot)`; `params.max_iter` (the TERM COUNT in
//!   this mode) is in scope for statistics that normalize in-loop.
//! - a field coloring defines
//!   `fn field_color(sum: f32, grad: vec2<f32>, n_terms: u32) -> FieldShade`
//!   with `struct FieldShade { t: f32, lum: f32 }` — `t` becomes the
//!   palette position (wrapped), `lum` multiplies the sampled color
//!   (hillshading). Params via `cparam(slot)`.

use super::EscapeParamDef;

/// A mode-B field definition.
pub struct FieldDef {
    /// Registry name — what `EscapeConfig::formula` stores.
    pub name: &'static str,
    pub display_name: &'static str,
    /// Whether `field_step` returns a real analytic gradient (the
    /// hillshade coloring is meaningful). Statistics fields return
    /// zero gradients and hillshade degrades to flat lighting.
    pub has_gradient: bool,
    /// Coloring used when `EscapeConfig::coloring` names a mode-A
    /// coloring (the usual state right after switching formulas).
    pub default_coloring: &'static str,
    pub parameters: &'static [EscapeParamDef],
    /// Named starting points, first = the default applied on a switch
    /// (see [`super::EscapePreset`]). A field's natural view and term
    /// count are as particular as any formula's.
    pub presets: &'static [super::EscapePreset],
    pub wgsl: &'static str,
}

/// A mode-B coloring: scalar field (+ gradient) → palette position
/// and luminance.
pub struct FieldColoringDef {
    pub name: &'static str,
    pub display_name: &'static str,
    pub parameters: &'static [EscapeParamDef],
    pub wgsl: &'static str,
}

// ====================================================================
// Fields
// ====================================================================

/// Weierstrass / Besicovitch–Ursell lacunary surface (plan §4 phase 3,
/// *[ETI]*): F(x,y) = Σₙ aⁿ·g(bⁿx + φ)·g(bⁿy + φ) — the 2D
/// Besicovitch–Ursell generalization with a generator choice
/// (cos / sin / triangle-wave ≙ a Takagi/blancmange surface). The
/// analytic gradient falls out of the sum term-by-term
/// (∂/∂x = aⁿbⁿ·g′(bⁿx+φ)·g(bⁿy+φ)), so hillshade normals are free.
/// Bagula's published `gg[x_,y_]` passes x twice — a 1D field in
/// disguise; we implement the evident intent (the true 2D product),
/// per the plan's note.
pub static WEIERSTRASS: FieldDef = FieldDef {
    name: "weierstrass",
    display_name: "Weierstrass Field",
    has_gradient: true,
    default_coloring: "field_hillshade",
    presets: super::presets::WEIERSTRASS,
    parameters: &[
        EscapeParamDef {
            name: "a",
            display_name: "Amplitude Ratio",
            default: 0.55,
            min: 0.05,
            max: 0.95,
            tooltip: "Per-octave amplitude factor a. Roughness rises toward 1 \
                      (the sum stays convergent below 1).",
            choices: &[],
        },
        EscapeParamDef {
            name: "b",
            display_name: "Frequency Ratio",
            default: 2.0,
            min: 1.2,
            max: 8.0,
            tooltip: "Per-octave frequency factor b (the lacunary sequence bⁿ). \
                      Classic Weierstrass needs ab ≥ 1 for nowhere-differentiability.",
            choices: &[],
        },
        EscapeParamDef {
            name: "generator",
            display_name: "Generator",
            default: 0.0,
            min: 0.0,
            max: 2.0,
            tooltip: "0 = cosine, 1 = sine, 2 = triangle wave (Takagi/blancmange).",
            choices: &["Cosine", "Sine", "Triangle (Takagi)"],
        },
        EscapeParamDef {
            name: "phase",
            display_name: "Phase",
            default: 0.0,
            min: 0.0,
            max: 6.28318,
            tooltip: "Phase offset added inside every octave's generator.",
            choices: &[],
        },
    ],
    wgsl: r#"
// Generator pair: g(t) and g'(t), selected by fparam(2). `t` arrives
// already reduced to [-pi, pi] by esc_reduce.
fn weier_g(t: f32, gen: i32) -> f32 {
    if (gen == 0) {
        return cos(t);
    }
    if (gen == 1) {
        return sin(t);
    }
    // Triangle wave, period 2*pi, range [-1, 1].
    let f = fract(t * 0.15915494);
    return 1.0 - 4.0 * abs(f - 0.5);
}

fn weier_dg(t: f32, gen: i32) -> f32 {
    if (gen == 0) {
        return -sin(t);
    }
    if (gen == 1) {
        return cos(t);
    }
    // d/dt of the triangle wave: +-4/(2*pi), sign by half-period.
    let f = fract(t * 0.15915494);
    return select(0.63661977, -0.63661977, f > 0.5);
}

fn field_init(p: vec2<f32>) -> FieldState {
    // Running octave scales: a^n in .x, b^n in .y.
    return FieldState(vec4<f32>(1.0, 1.0, 0.0, 0.0), vec4<f32>(0.0));
}

fn field_step(n: u32, p: vec2<f32>, s: FieldState) -> FieldStep {
    let gen = i32(fparam(2u));
    let phase = fparam(3u);
    let an = s.a.x;
    let bn = s.a.y;
    // Reduced once here and shared by g and dg, so the cost is two
    // reductions per octave rather than four.
    let tx = esc_reduce(bn * p.x + phase);
    let ty = esc_reduce(bn * p.y + phase);
    let gx = weier_g(tx, gen);
    let gy = weier_g(ty, gen);
    let value = an * gx * gy;
    let grad = vec2<f32>(
        an * bn * weier_dg(tx, gen) * gy,
        an * bn * gx * weier_dg(ty, gen),
    );
    let next = FieldState(
        vec4<f32>(an * fparam(0u), bn * fparam(1u), 0.0, 0.0),
        vec4<f32>(0.0),
    );
    return FieldStep(next, value, grad);
}
"#,
};

/// Markus–Lyapunov (plan §4 phase 3; unparked per *[ETI]* — "cheaper
/// than Magnet"). The logistic map x ← r·x·(1−x) with r switching
/// between A = pixel.x and B = pixel.y along a periodic letter
/// sequence; the field is the Lyapunov exponent
/// λ = (1/N)·Σ log|r·(1−2x)| after a transient. λ < 0 = stable
/// (the classic Zircon-Zity spikes), λ > 0 = chaos — a signed scalar,
/// mapped through the diverging coloring by default.
pub static MARKUS_LYAPUNOV: FieldDef = FieldDef {
    name: "markus_lyapunov",
    display_name: "Markus–Lyapunov",
    has_gradient: false,
    default_coloring: "field_diverging",
    presets: super::presets::MARKUS_LYAPUNOV,
    parameters: &[
        EscapeParamDef {
            name: "seq_bits",
            display_name: "Sequence Bits",
            default: 2.0,
            min: 0.0,
            max: 1023.0,
            tooltip: "The A/B forcing sequence as a bit pattern (bit n of the \
                      integer: 0 = A, 1 = B), read LSB-first over the sequence \
                      length. 2 with length 2 = the classic \"AB\".",
            choices: &[],
        },
        EscapeParamDef {
            name: "seq_len",
            display_name: "Sequence Length",
            default: 2.0,
            min: 1.0,
            max: 10.0,
            tooltip: "How many bits of the pattern form the repeating sequence.",
            choices: &[],
        },
        EscapeParamDef {
            name: "warmup",
            display_name: "Warmup",
            default: 50.0,
            min: 0.0,
            max: 500.0,
            tooltip: "Transient iterations discarded before the exponent \
                      accumulates (lets the orbit settle onto its attractor).",
            choices: &[],
        },
    ],
    wgsl: r#"
fn lyap_r(n: u32, p: vec2<f32>) -> f32 {
    let len = max(u32(fparam(1u)), 1u);
    let bits = u32(fparam(0u));
    let use_b = ((bits >> (n % len)) & 1u) != 0u;
    return select(p.x, p.y, use_b);
}

fn field_init(p: vec2<f32>) -> FieldState {
    return FieldState(vec4<f32>(0.5, 0.0, 0.0, 0.0), vec4<f32>(0.0));
}

fn field_step(n: u32, p: vec2<f32>, s: FieldState) -> FieldStep {
    let r = lyap_r(n, p);
    let x = s.a.x;
    let x_next = clamp(r * x * (1.0 - x), 0.0, 1.0);
    let warmup = u32(fparam(2u));
    var value = 0.0;
    if (n >= warmup && params.max_iter > warmup) {
        // Normalized in-loop: the sum IS the mean exponent when the
        // loop ends, whatever the term count.
        let d = abs(r * (1.0 - 2.0 * x_next));
        value = log(max(d, 1e-20)) / f32(params.max_iter - warmup);
    }
    return FieldStep(
        FieldState(vec4<f32>(x_next, 0.0, 0.0, 0.0), vec4<f32>(0.0)),
        value,
        vec2<f32>(0.0, 0.0),
    );
}
"#,
};

/// Finite-time Lyapunov exponent of the Chirikov standard map (plan
/// §4 phase 3 — "FTLE / standard-map stability as the
/// generalization"). Pixel = (θ₀, I₀); iterate I ← I + K·sin θ,
/// θ ← θ + I, carry the tangent 2×2 through the Jacobian
/// [[1+K·cosθ, 1], [K·cosθ, 1]], renormalizing each step; the field
/// is the mean log growth — near 0 on KAM islands, positive in the
/// chaotic sea.
pub static STANDARD_MAP_FTLE: FieldDef = FieldDef {
    name: "standard_map_ftle",
    display_name: "Standard Map FTLE",
    has_gradient: false,
    default_coloring: "field_diverging",
    presets: super::presets::STANDARD_MAP_FTLE,
    parameters: &[EscapeParamDef {
        name: "k",
        display_name: "Kick Strength",
        default: 1.0,
        min: 0.0,
        max: 10.0,
        tooltip: "Chirikov K. Islands dominate below ~0.97; the chaotic sea \
                  takes over above it.",
        choices: &[],
    }],
    wgsl: r#"
fn field_init(p: vec2<f32>) -> FieldState {
    // a = (theta, I); b = tangent matrix (m00, m01, m10, m11) = identity.
    return FieldState(
        vec4<f32>(p.x, p.y, 0.0, 0.0),
        vec4<f32>(1.0, 0.0, 0.0, 1.0),
    );
}

fn field_step(n: u32, p: vec2<f32>, s: FieldState) -> FieldStep {
    let k = fparam(0u);
    let theta = s.a.x;
    let i_old = s.a.y;
    // theta accumulates without bound through `t_new`, so it needs
    // the same reduction the Weierstrass octaves do.
    let th = esc_reduce(theta);
    let i_new = i_old + k * sin(th);
    let t_new = theta + i_new;
    // Jacobian of (theta, I) -> (theta + I + K sin theta, I + K sin theta)
    // at the PRE-step point.
    let kc = k * cos(th);
    let m = s.b;
    let j00 = 1.0 + kc;
    // Row-major tangent update: M <- J * M.
    let n00 = j00 * m.x + 1.0 * m.z;
    let n01 = j00 * m.y + 1.0 * m.w;
    let n10 = kc * m.x + 1.0 * m.z;
    let n11 = kc * m.y + 1.0 * m.w;
    let nrm = max(max(abs(n00), abs(n01)), max(abs(n10), abs(n11)));
    let scale = max(nrm, 1e-20);
    let value = log(scale) / f32(max(params.max_iter, 1u));
    return FieldStep(
        FieldState(
            vec4<f32>(t_new, i_new, 0.0, 0.0),
            vec4<f32>(n00, n01, n10, n11) / scale,
        ),
        value,
        vec2<f32>(0.0, 0.0),
    );
}
"#,
};

// ====================================================================
// Field colorings
// ====================================================================

/// Palette position directly from the field value.
pub static FIELD_VALUE: FieldColoringDef = FieldColoringDef {
    name: "field_value",
    display_name: "Field Value",
    parameters: &[
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette cycles per unit of field value.",
            choices: &[],
        },
        EscapeParamDef {
            name: "offset",
            display_name: "Offset",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Palette phase offset.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn field_color(sum: f32, grad: vec2<f32>, n_terms: u32) -> FieldShade {
    return FieldShade(sum * cparam(0u) + cparam(1u), 1.0);
}
"#,
};

/// Signed scalar → diverging palette convention (plan coloring
/// catalog): zero lands mid-palette, the sign picks the half, an
/// atan squash keeps extremes inside it without wrapping.
pub static FIELD_DIVERGING: FieldColoringDef = FieldColoringDef {
    name: "field_diverging",
    display_name: "Diverging",
    parameters: &[EscapeParamDef {
        name: "scale",
        display_name: "Scale",
        default: 3.0,
        min: 0.01,
        max: 100.0,
        tooltip: "Sensitivity: how fast the field value saturates toward the \
                  palette's ends.",
        choices: &[],
    }],
    wgsl: r#"
fn field_color(sum: f32, grad: vec2<f32>, n_terms: u32) -> FieldShade {
    // atan squash: (-inf, inf) -> (0, 1) with 0 at exactly 0.5. The
    // 0.999 keeps the wrapped palette's two ends from meeting.
    let t = 0.5 + 0.999 * atan(sum * cparam(0u)) / 3.14159265;
    return FieldShade(t, 1.0);
}
"#,
};

/// Analytic-gradient hillshade (plan: "hillshade normals for free").
/// Height = the field value (drives the palette position); the
/// gradient builds a surface normal, lit by a directional light.
pub static FIELD_HILLSHADE: FieldColoringDef = FieldColoringDef {
    name: "field_hillshade",
    display_name: "Hillshade",
    parameters: &[
        EscapeParamDef {
            name: "azimuth",
            display_name: "Light Azimuth",
            default: 315.0,
            min: 0.0,
            max: 360.0,
            tooltip: "Light direction, degrees clockwise from north (the \
                      cartography convention; 315 = upper-left).",
            choices: &[],
        },
        EscapeParamDef {
            name: "elevation",
            display_name: "Light Elevation",
            default: 45.0,
            min: 5.0,
            max: 90.0,
            tooltip: "Light height above the horizon, degrees.",
            choices: &[],
        },
        EscapeParamDef {
            name: "relief",
            display_name: "Relief",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Vertical exaggeration of the surface before lighting.",
            choices: &[],
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Palette Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette cycles per unit of field value.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn field_color(sum: f32, grad: vec2<f32>, n_terms: u32) -> FieldShade {
    let az = cparam(0u) * 0.01745329;
    let el = cparam(1u) * 0.01745329;
    let relief = cparam(2u);
    let normal = normalize(vec3<f32>(-grad.x * relief, -grad.y * relief, 1.0));
    let light = vec3<f32>(sin(az) * cos(el), cos(az) * cos(el), sin(el));
    let lum = clamp(dot(normal, light), 0.0, 1.0) * 0.9 + 0.1;
    return FieldShade(sum * cparam(3u), lum);
}
"#,
};

// ====================================================================
// Registries
// ====================================================================

/// Ordered field registry. **Append-only** — same contract as
/// [`super::FORMULAS`].
pub static FIELDS: &[&FieldDef] = &[&WEIERSTRASS, &MARKUS_LYAPUNOV, &STANDARD_MAP_FTLE];

/// Ordered field-coloring registry. **Append-only.**
pub static FIELD_COLORINGS: &[&FieldColoringDef] =
    &[&FIELD_VALUE, &FIELD_DIVERGING, &FIELD_HILLSHADE];

/// Look up a FIELD formula by name. `None` = the name belongs to
/// mode A (or is unknown) — the caller routes to the formula
/// registry, which carries the unknown-name fallback.
pub fn get_field(name: &str) -> Option<&'static FieldDef> {
    FIELDS.iter().find(|f| f.name == name).copied()
}

/// Resolve the coloring for a field render. `EscapeConfig::coloring`
/// usually still names a mode-A coloring right after a formula
/// switch — that (or any unknown name) falls back to the FIELD's
/// declared default, so switching to a field never renders the wrong
/// registry's shader.
pub fn get_field_coloring(name: &str, field: &FieldDef) -> &'static FieldColoringDef {
    FIELD_COLORINGS
        .iter()
        .find(|c| c.name == name)
        .copied()
        .unwrap_or_else(|| {
            FIELD_COLORINGS
                .iter()
                .find(|c| c.name == field.default_coloring)
                .copied()
                .unwrap_or(FIELD_COLORINGS[0])
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_registry_names_are_unique_and_disjoint_from_formulas() {
        let mut seen = std::collections::HashSet::new();
        for f in FIELDS {
            assert!(seen.insert(f.name), "duplicate field name {}", f.name);
            // A name in both registries would make routing ambiguous.
            assert!(
                !crate::escape::FORMULAS.iter().any(|m| m.name == f.name),
                "{} shadows a mode-A formula",
                f.name
            );
        }
        seen.clear();
        for c in FIELD_COLORINGS {
            assert!(seen.insert(c.name), "duplicate field coloring {}", c.name);
        }
    }

    #[test]
    fn default_colorings_resolve() {
        for f in FIELDS {
            assert!(
                FIELD_COLORINGS.iter().any(|c| c.name == f.default_coloring),
                "{} declares unknown default coloring {}",
                f.name,
                f.default_coloring
            );
            // A mode-A coloring name (the post-switch state) must fall
            // back to the field's default, not panic or mis-resolve.
            let resolved = get_field_coloring("smooth", f);
            assert_eq!(resolved.name, f.default_coloring);
        }
    }

    #[test]
    fn field_lookup_routes_correctly() {
        assert!(get_field("weierstrass").is_some());
        assert!(get_field("mandelbrot").is_none(), "mode A stays mode A");
        assert!(get_field("nonexistent").is_none());
    }
}

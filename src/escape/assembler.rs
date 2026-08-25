//! WGSL assembly for escape-time pipelines.
//!
//! Far simpler than `shader_builder_v2`: one formula + one coloring
//! spliced into a fixed template, no index maps, no feature flags yet.
//! Marker lines (`//__FORMULA__`, `//__COLORING__`) are replaced
//! whole-line, the same directive-on-its-own-line discipline the
//! effect chain's include splicer uses.
//!
//! ## Output contract
//!
//! The compute pass writes an `Rgba32Float` image shaped like the
//! flame accumulator: `rgb` = color in linear space, `a` = density
//! (1.0 everywhere — every pixel is "hit" exactly once). The tonemap
//! shader's Linear mode then applies exposure and its `pow(1/gamma)`
//! display encode; palette texels are decoded `pow(2.2)` here so the
//! round trip is identity at the default gamma — the same
//! `srgb_to_linear` convention the flame plot uses.

use super::{ColoringDef, FormulaDef};

/// Number of vec4 slots for each param block in the uniform — 16 float
/// params per formula / per coloring. Mirrored in the Rust-side
/// `EscapeParamsGpu`; a def exceeding it logs in `pack_params`.
pub const PARAM_VEC4S: usize = 4;

const TEMPLATE: &str = r#"
// Escape-time compute pass (assembled per formula x coloring).
// See src/escape/assembler.rs for the output contract.

struct EscapeParams {
    center: vec2<f32>,     // view center (f64 cast -- phase-1 ceiling)
    julia_c: vec2<f32>,    // fixed c in Julia mode
    span: vec2<f32>,       // complex-plane extent of the viewport (x, y)
    rot_cs: vec2<f32>,     // (cos, sin) of view rotation
    width: u32,
    height: u32,
    max_iter: u32,
    flags: u32,            // bit 0 = Julia mode
    bailout: f32,          // escape radius SQUARED
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
    fparams: array<vec4<f32>, 4>,  // formula params, slot-ordered
    cparams: array<vec4<f32>, 4>,  // coloring params, slot-ordered
}

@group(0) @binding(0) var<uniform> params: EscapeParams;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var palette_texture: texture_2d<f32>;
@group(0) @binding(3) var palette_sampler: sampler;

fn fparam(i: u32) -> f32 {
    return params.fparams[i / 4u][i % 4u];
}

fn cparam(i: u32) -> f32 {
    return params.cparams[i / 4u][i % 4u];
}

//__FORMULA__

//__COLORING__

@compute @workgroup_size(8, 8, 1)
fn escape_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.width || gid.y >= params.height) {
        return;
    }

    // Pixel center -> complex plane: offset from view center, y flipped
    // (texture y grows down, Im grows up), then view rotation.
    let uv = (vec2<f32>(f32(gid.x), f32(gid.y)) + vec2<f32>(0.5, 0.5))
        / vec2<f32>(f32(params.width), f32(params.height));
    var d = (uv - vec2<f32>(0.5, 0.5)) * params.span;
    d.y = -d.y;
    let rot = params.rot_cs;
    let pixel = params.center + vec2<f32>(
        d.x * rot.x - d.y * rot.y,
        d.x * rot.y + d.y * rot.x,
    );

    // Julia toggle: parameter plane iterates z from the critical point
    // with c = pixel; dynamical plane iterates z from the pixel with c
    // fixed. One flag, not a formula-list entry (plan section 3).
    let is_julia = (params.flags & 1u) != 0u;
    var z = select(vec2<f32>(0.0, 0.0), pixel, is_julia);
    let c = select(pixel, params.julia_c, is_julia);

    var escaped = false;
    var n = 0u;
    for (var i = 0u; i < params.max_iter; i = i + 1u) {
        z = formula_step(z, c);
        if (dot(z, z) > params.bailout) {
            escaped = true;
            n = i + 1u;
            break;
        }
    }

    var rgb = vec3<f32>(0.0, 0.0, 0.0); // interior: black (v1)
    if (escaped) {
        let t = fract(coloring_map(z, n, escaped));
        // textureSampleLevel: explicit LOD, legal in non-uniform
        // control flow (unlike textureSample) -- WASM-safe.
        let srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(t, 0.5), 0.0).rgb;
        rgb = pow(max(srgb, vec3<f32>(0.0)), vec3<f32>(2.2));
    }

    textureStore(out_tex, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(rgb, 1.0));
}
"#;

/// Assemble the WGSL for one (formula, coloring) pair.
pub fn assemble(formula: &FormulaDef, coloring: &ColoringDef) -> String {
    let mut out = Vec::new();
    for line in TEMPLATE.lines() {
        match line.trim() {
            "//__FORMULA__" => {
                out.push(format!("// formula: {}", formula.name));
                out.push(formula.wgsl.to_string());
            }
            "//__COLORING__" => {
                out.push(format!("// coloring: {}", coloring.name));
                out.push(coloring.wgsl.to_string());
            }
            _ => out.push(line.to_string()),
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::escape::{COLORINGS, FORMULAS};
    use egui_wgpu::wgpu::naga;

    #[test]
    fn assembly_splices_both_bodies_and_leaves_no_markers() {
        for f in FORMULAS {
            for c in COLORINGS {
                let src = assemble(f, c);
                assert!(src.contains("fn formula_step"), "{}x{}", f.name, c.name);
                assert!(src.contains("fn coloring_map"), "{}x{}", f.name, c.name);
                assert!(!src.contains("//__FORMULA__"));
                assert!(!src.contains("//__COLORING__"));
            }
        }
    }

    #[test]
    fn every_combination_parses_as_wgsl() {
        // naga front-end parse + validation, no GPU needed — the same
        // guarantee the flame shader dumps get from their tests. This
        // is the compile half of the "formula x coloring compile
        // probe" the plan calls for; the render half needs a device.
        for f in FORMULAS {
            for c in COLORINGS {
                let src = assemble(f, c);
                let module = naga::front::wgsl::parse_str(&src)
                    .unwrap_or_else(|e| panic!("{}x{} failed to parse: {e}", f.name, c.name));
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::all(),
                )
                .validate(&module)
                .unwrap_or_else(|e| panic!("{}x{} failed validation: {e:?}", f.name, c.name));
            }
        }
    }

    #[test]
    fn defs_fit_the_param_slot_budget() {
        for f in FORMULAS {
            assert!(f.parameters.len() <= PARAM_VEC4S * 4, "{}", f.name);
        }
        for c in COLORINGS {
            assert!(c.parameters.len() <= PARAM_VEC4S * 4, "{}", c.name);
        }
    }
}

//! The **camera lens**: a variation applied to the screen offset.
//!
//! A lens warps the VIEW, not the fractal. Every escape kernel derives
//! its sample point from a normalised screen offset; a lens is a map
//! applied to that offset before the view scale, so the formula and
//! its iteration are untouched and only which point each pixel samples
//! changes.
//!
//! ```text
//! pixel -> offset n -> L(n) -> * span -> rotate -> + centre -> the formula
//!                      ^^^^
//! ```
//!
//! That is the INVERSE direction of a flame's final transform, which
//! maps attractor points to the screen. Two things follow, and both
//! are why this is worth having: a lens need not be invertible (a
//! many-to-one map just shows a region more than once, and every pixel
//! still gets one deterministic sample), and a variation's lens
//! picture is not its final-transform picture.
//!
//! # The variation code is not reimplemented here
//!
//! [`ShaderBuilder::build_layer_map_at`] emits the variation
//! functions, whichever helper libraries they need, the packed
//! `get_param`, the buffer declarations and a
//! `flame_map(xform_id, u, seed) -> vec2<f32>`. The simulation's
//! layer warp already relies on it, and its
//! `every_variation_validates_in_the_layer_warp` test asserts that all
//! of them compile. This module only builds the one-transform flame
//! that describes the lens and the small glue that calls it.
//!
//! # Units
//!
//! The lens sees **half-height one**: `n.y` spans `[-1, 1]` and `n.x`
//! spans `[-aspect, aspect]`. That is the scale variations are written
//! for -- the unit disc is where `fisheye`, `spherical` and the rest
//! do something recognisable -- so every call site converts into that
//! convention and back rather than handing over raw pixels or raw
//! world units.
//!
//! See `docs/projects/escape-camera-lens.md`.

use crate::config::escape::EscapeConfig;
use crate::scene::transforms::{Flame, Transform};
use crate::variations::VariationRegistry;

/// How far the amount may be pushed either way.
///
/// Past 1 the lens OVERSHOOTS: the displacement `L(n) - n` is scaled
/// up rather than blended in. Below 0 it runs BACKWARDS, which is the
/// useful half -- `mix(n, L(n), -1)` is `2n - L(n)`, the displacement
/// reflected, which to first order is the lens's inverse. That is how
/// a lens whose bulge goes the wrong way is turned around without
/// needing a second variation that happens to be its inverse.
///
/// Defined in the config module, which is compiled without this
/// engine, and re-exported here.
pub use crate::config::escape::LENS_AMOUNT_LIMIT;

/// The lenses offered first, in this order.
///
/// Hand-picked from the survey sheets as the ones that read as a
/// CAMERA rather than as an effect -- a bulge, a twist, an unwrap --
/// with the three most asked for at the front. Everything else that
/// measured usable follows in the app's ordinary order.
pub const LENS_RECOMMENDED: &[&str] = &[
    "bubble", "hemisphere", "eyefish", "fisheye",
    "spherical", "cylinder", "bent", "horseshoe",
    "swirl", "curl", "escher", "bipolar",
    "elliptic", "polar", "polar2", "log",
    "disc", "blur_zoom", "separation", "wedge",
    "cardioid", "tangent", "rectangles", "waves",
    "splits", "crop",
];

/// Every variation that measured usable as a lens.
///
/// From `variation_probe lens` (see `src/probe/lens.rs`): each
/// variation evaluated over a screen grid and classified, with
/// `clean` and `unbounded` kept and `degenerate` and `broken`
/// dropped. That is 396 of 647 -- the rest are the RNG blur and
/// noise families, which give a different answer per pixel, and the
/// z-only variations, which return nothing in two dimensions.
///
/// A LIST rather than a rule, because no rule available at runtime
/// agrees with the measurement: filtering instead on shader shape --
/// no RNG, no accumulator, no per-thread state -- keeps 20 variations
/// the measurement rejects and drops 74 it accepts. The picker's
/// "show everything" box is the escape hatch, since a variation's
/// parameters are editable and the classification describes its
/// DEFAULTS.
///
/// Sorted, so `lens_is_usable` can binary-search it.
pub const LENS_USABLE: &[&str] = &[
    "CircleTrans1", "acoth", "affine3D", "anamorphcyl",
    "apollonian_gasket", "apollony", "arch", "arcsech",
    "arcsech2", "arcsinh", "arctanh", "arctruchet",
    "atan", "atan2_spirals", "auger", "bCollide",
    "bMod", "bSwirl", "bTransform", "barycentroid",
    "bent", "bent2", "bi_linear", "bipolar",
    "bipolar2", "blob", "blob3D", "blocky",
    "blur_zoom", "bsplit", "bubble", "bubble2",
    "bubbleT3D", "bubble_wf", "butterfly", "butterfly3D",
    "bwraps", "bwraps7", "cardioid", "cell",
    "checkerboard_wf", "chladni", "chladni_disc", "chrysanthemum",
    "chunk", "circleLinear", "circleRand", "circlecrop",
    "circlesplit", "circlize", "circlize2", "circus",
    "clifford_js", "collideoscope", "complex", "conic",
    "corners", "cos", "cos2_bs", "cosh",
    "cosh2_bs", "coshq", "cosine", "cosq",
    "cot", "cot2_bs", "coth", "coth2_bs",
    "cothq", "cotq", "cpow2", "cpow3",
    "crop", "crop3D", "cross", "csc",
    "csc2_bs", "csc_squared", "csch", "csch2_bs",
    "cschq", "cscq", "curl", "curl3D",
    "curl_sp", "curve", "cylinder", "cylinder2",
    "d_spherical", "dc_bubble", "dc_hexes_wf", "dc_linear",
    "dc_ztransl", "deltaA", "devil_warp", "diamond",
    "dinis_surface_wf", "disc", "disc2", "disc3",
    "disc3d", "eCollide", "eMod", "eMotion",
    "ePush", "eRotate", "eScale", "eclipse",
    "edisc", "electron_orbital", "elliptic", "ennepers",
    "ennepers2", "epispiral", "epispiral_wf", "erf",
    "erf3D", "escher", "estiq", "ex",
    "exp", "exp2", "exp2_bs", "exponential",
    "eyefish", "falloff2", "fan", "fan2",
    "fdisc", "fibonacci2", "fisheye", "flatten",
    "flipcircle", "flipy", "flower_db", "flux",
    "foci", "foci_3D", "fourth", "fract_dragon_wf",
    "fract_julia_wf", "fract_mandelbrot_wf", "fract_meteors_wf", "fract_pearls_wf",
    "fract_salamander_wf", "fuchsian_triangle", "funnel", "gamma",
    "gridout", "gridout2", "gridout3D", "handkerchief",
    "heart", "heart_wf", "hecke_group", "helicoid",
    "helix", "hemisphere", "henon", "hexaplay3D",
    "ho", "hofstadter", "hole2", "holesq",
    "horseshoe", "hyperbolic", "hyperbolic_camera", "hyperbolicellipse",
    "hypercrop", "hypershift", "hypertile", "hypertile3D",
    "iconattractor_js", "idisc", "intersection", "invpolar",
    "invsquircular", "invtree_js", "jac_asn", "jac_cn",
    "jac_dn", "jac_sn", "jubiQ", "jubiq4d",
    "kaleidoscope", "layered_spiral", "lazyTravis", "lazyjess",
    "lazysensen", "lazysusan", "linear", "linear3D",
    "linearT", "linearT3D", "log", "log_db",
    "log_tile2", "loonie", "loonie2", "loonie3",
    "loonie_3D", "loq", "lorentz_mobius", "lorenz_js",
    "lozi", "mandelbrot", "mask", "matrix3D",
    "mcarpet", "minkQM", "minkowski", "minkowski_camera",
    "minkowskope", "mobiq", "mobiq4d", "mobius",
    "mobiusN", "mobius_dragon_3D", "mobius_strip", "multi_kaleidoscope",
    "murl", "murl2", "ngon", "npolar",
    "octagon", "octapol", "onion", "onion2",
    "ortho", "ovoid3d", "pRose3D", "pTransform",
    "panorama1", "panorama2", "parplot2d_wf", "perspective",
    "petal", "plane_wf", "plusrecip", "poincare3D",
    "pointgrid3d_wf", "pointgrid_wf", "polar", "polar2",
    "polyhedron", "popcorn2_3D", "post_bwraps", "post_bwraps2",
    "post_circlecrop", "post_colorscale_wf", "post_crop", "post_curl",
    "post_curl3D", "post_heat", "post_rotate_x", "post_rotate_y",
    "post_spherical", "post_spin_z", "post_ztranslate_wf", "power",
    "pre_bwraps", "pre_bwraps2", "pre_circlecrop", "pre_crop",
    "pre_curl", "pre_dcztransl", "pre_disc", "pre_disc3d",
    "pre_rotate_x", "pre_rotate_y", "pre_sinusoidal", "pre_sinusoidal3d",
    "pre_spherical", "pre_spin_z", "pre_wave3D_wf", "pre_zscale",
    "pre_ztranslate", "pressure_wave", "projective", "pulse",
    "pyramid", "q_ode", "quasiconformal", "quaternion",
    "quaternion_camera", "quaternion_julia", "quaternion_linear", "quaternion_rotation",
    "rational3", "rays", "rays1", "rays2",
    "rays3", "rectangles", "rhodonea", "rings",
    "rings2", "ripple", "rippled", "rose_wf",
    "roundspher", "roundspher3D", "scry", "scry2",
    "scry_3D", "sec", "sec2_bs", "secant2",
    "sech", "sech2_bs", "sechq", "secq",
    "separation", "shift", "shredlin", "shredrad",
    "sigmoid", "sin", "sin2_bs", "sinh",
    "sinh2_bs", "sinhq", "sinq", "sintrange",
    "sinusoidal", "sinusoidal3d", "sph3D", "sphere_nja",
    "spherecrop", "spherical", "spherical3D", "spherical3D_wf",
    "sphericalN", "spiral", "spiralwing", "spirograph",
    "spligon", "split", "splits", "splits3D",
    "spray_blur", "squarize", "squircular", "squirrel",
    "stereogram", "stripes", "stripfit", "stwin",
    "svensson_js", "svf", "swirl", "swirl3",
    "swirl3D_wf", "sym_bg7", "sym_ng3", "sym_ng4",
    "sym_ng5", "szubieta", "tan", "tan2_bs",
    "tancos", "tangent", "tangent3D", "tanh",
    "tanh2_bs", "tanhq", "tanq", "target",
    "target_sp", "taurus", "tile_log", "tqmirror",
    "trade", "truchet2", "twoface", "unpolar",
    "vibration2", "vogel", "voron", "w",
    "waves", "waves2", "waves2_3D", "waves2_radial",
    "waves2_wf", "waves2b", "waves3_wf", "waves4_wf",
    "wdisc", "wedge", "wedge_sph", "whitney_umbrella",
    "xerf", "xheart", "yplot2d_wf", "z",
];

/// Whether a variation measured usable as a lens.
pub fn lens_is_usable(name: &str) -> bool {
    LENS_USABLE.binary_search(&name).is_ok()
}

/// The picker's order: the recommended head, then everything else
/// usable in the registry's own order.
pub fn lens_menu(registry: &VariationRegistry) -> Vec<String> {
    let mut out: Vec<String> = LENS_RECOMMENDED
        .iter()
        .filter(|n| registry.get(n).is_some())
        .map(|n| n.to_string())
        .collect();
    for name in registry.names() {
        if lens_is_usable(name) && !LENS_RECOMMENDED.contains(&name.as_str()) {
            out.push(name.clone());
        }
    }
    out
}

/// The bind group the lens flame's buffers land on.
///
/// Not 1: mode D already binds its IFS rows there.
pub const LENS_GROUP: u32 = 2;

/// Whether this config asks for a lens at all.
///
/// An amount of zero is not a lens: the blend is the identity, so the
/// shader is better off without the machinery than with a no-op in it.
/// Either sign is a lens; they bulge opposite ways.
pub fn is_active(escape: &EscapeConfig, registry: &VariationRegistry) -> bool {
    // Zero is the identity in either direction, so it is "no lens".
    // Negative is a lens, and a useful one.
    !escape.lens.is_empty()
        && escape.lens_amount != 0.0
        && registry.get(&escape.lens).is_some()
}

/// The one-transform flame that describes the lens.
///
/// Identity affine, because a lens IS the variation on the screen
/// offset and an affine of its own would be a second, invisible
/// control. The **amount rides in the transform's weight**, which
/// `flame_map` does not read -- it is a chaos-game selection
/// probability and a map never selects -- so it is free carriage for a
/// live value that would otherwise cost a field in a uniform struct
/// redeclared in nine places.
pub fn lens_flame(escape: &EscapeConfig, registry: &VariationRegistry) -> Option<Flame> {
    if !is_active(escape, registry) {
        return None;
    }
    let info = registry.get(&escape.lens)?;

    // The IDENTITY, in this engine's convention. `apply_affine` is
    // `x' = a*x + b*y + e` and `y' = c*x + d*y + f` (shaders/core/
    // affine.wgsl), so the diagonal is a and D -- not a and e. Setting
    // a = 1, e = 1 instead maps the whole plane onto the line y = 0,
    // which renders as a lens that ruins the picture identically for
    // every variation; that is exactly the bug
    // `a_lens_of_linear_is_the_identity` now pins.
    let mut t = Transform::new();
    t.a = 1.0;
    t.b = 0.0;
    t.e = 0.0;
    t.c = 0.0;
    t.d = 1.0;
    t.f = 0.0;
    t.g = 0.0;
    // NEGATED. The slider is a camera control, so positive means the
    // middle BULGES OUT -- and for every radial lens in the
    // recommended set that is the variation run backwards. Measured
    // profiles say so rather than intuition: the magnification at
    // radius r is r / L_t(r), and for `bubble`, `hemisphere` and
    // `eyefish` alike it RISES with r at a positive raw blend (the
    // rim stretched more than the middle, which reads as the centre
    // being pushed in) and FALLS with r at a negative one, which is
    // the bulge. Reported from use, twice, before it was believed.
    //
    // The config stores what the slider says; the shader gets the
    // blend, and the blend is its negation.
    let amount = escape.lens_amount.clamp(-LENS_AMOUNT_LIMIT, LENS_AMOUNT_LIMIT);
    t.weight = -amount;
    t.variations.clear();
    t.set_variation(&escape.lens, 1.0);
    for p in &info.parameters {
        if let Some(v) = escape.lens_params.get(&p.name) {
            t.set_variation_param(&escape.lens, &p.name, *v);
        }
    }

    let mut flame = Flame::new();
    flame.name = "escape lens".to_string();
    flame.transforms.clear();
    flame.transforms.push(t);
    Some(flame)
}

/// The pipeline's identity, for the shader cache key.
///
/// The variation name alone is not enough: a parameter can change the
/// map's SHAPE (an enum choice picks a different branch of the
/// formula), and a stale pipeline would then draw the previous lens.
/// The amount is excluded on purpose -- it rides in a buffer, so
/// dragging that slider must not recompile.
pub fn lens_key(escape: &EscapeConfig, registry: &VariationRegistry) -> String {
    if !is_active(escape, registry) {
        return String::new();
    }
    let mut key = escape.lens.clone();
    for (k, v) in &escape.lens_params {
        key.push_str(&format!(",{k}={v}"));
    }
    key
}

/// The WGSL glue: `esc_lens`, over `build_layer_map_at`'s output.
///
/// The seed is derived from the point rather than passed in, so every
/// call site keeps a one-argument lens and a variation that draws from
/// the RNG still varies across the frame instead of displacing the
/// whole picture by one constant.
pub const LENS_GLUE: &str = r#"
// The camera lens: a variation on the normalised screen offset, in
// the half-height-one convention. `mix` toward the identity is the
// amount, which rides in the transform's weight.
fn esc_lens(n: vec2<f32>) -> vec2<f32> {
    let seed = bitcast<u32>(n.x) * 7919u + bitcast<u32>(n.y) * 104729u;
    let mapped = flame_map(0u, n, seed);
    return mix(n, mapped, transforms[0].weight);
}
"#;

/// Everything the shader needs for this lens, or `None` for no lens.
pub fn lens_source(escape: &EscapeConfig, registry: &VariationRegistry) -> Option<String> {
    let flame = lens_flame(escape, registry)?;
    let builder = crate::shader_builder_v2::ShaderBuilder::new(registry.clone());
    let mut src = builder.build_layer_map_at(&flame, LENS_GROUP);
    // The layer map moves the flame's BINDINGS to our group, but not
    // its NAMES, and the escape shader declares a palette of its own.
    // `build_layer_map` already renames `params` for the same reason;
    // these two are the rest of the overlap, and the escape host is
    // the one that has to give way because the flame header is shared
    // with the simulation.
    //
    // `ff_atan2` is the third: mode D's walk uses the same guarded
    // atan2 the flame does, so both blocks define it. Renaming inside
    // the lens block is safe because every caller of it is in that
    // block too -- the variation bodies that need it come with it.
    for name in ["palette_texture", "palette_sampler", "ff_atan2"] {
        src = src.replace(name, &format!("lens_{name}"));
    }
    src.push_str(LENS_GLUE);
    Some(src)
}

// ---------------------------------------------------------------- GPU

use wgpu::util::DeviceExt;

/// The lens flame's buffers and its bind group.
///
/// The same five bindings the flame header declares and the
/// simulation's layer warp binds -- transforms, the flame's params,
/// the packed variation params, attachments, subflame metadata. Four
/// of them are inert for a lens (a map has no attachments, no
/// subflames and no plot), but the header declares them, so they must
/// be bound or the group is rejected.
pub struct LensGpu {
    /// What this was built for, so a change of lens rebuilds and a
    /// change of amount does not.
    pub key: String,
    transforms: wgpu::Buffer,
    flame_params: wgpu::Buffer,
    variation_params: wgpu::Buffer,
    attachments: wgpu::Buffer,
    subflame_meta: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

/// The layout for [`LENS_GROUP`].
pub fn lens_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let storage_ro = |binding: u32| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Escape Lens Layout"),
        entries: &[
            storage_ro(0),
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            storage_ro(5),
            storage_ro(10),
            storage_ro(12),
        ],
    })
}

impl LensGpu {
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Build the buffers for a lens flame and bind them.
    pub fn build(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        flame: &Flame,
        key: String,
    ) -> Self {
        use crate::gpu::buffers::{pack_gpu_transforms, pack_gpu_variation_params};
        let transforms = pack_gpu_transforms(flame, crate::scene::transforms::RenderMode::TwoD);
        let vparams = pack_gpu_variation_params(flame);
        let make = |label: &str, bytes: &[u8]| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            })
        };
        let transforms = make("Escape Lens Transforms", bytemuck::cast_slice(&transforms[..1]));
        let variation_params =
            make("Escape Lens Variation Params", bytemuck::cast_slice(&vparams[..1]));
        let flame_params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Escape Lens Flame Params"),
            contents: bytemuck::bytes_of(
                &<crate::gpu::buffers::GpuParams as bytemuck::Zeroable>::zeroed(),
            ),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let attachments = vec![
            <crate::gpu::buffers::GpuAttachmentList as bytemuck::Zeroable>::zeroed();
            crate::gpu::buffers::MAX_TRANSFORMS
        ];
        let metas = crate::gpu::buffers::build_subflame_metas(&[]).unwrap_or_else(|_| {
            [<crate::gpu::buffers::SubflameMeta as bytemuck::Zeroable>::zeroed();
                crate::gpu::buffers::MAX_SUBFLAMES]
        });
        let attachments = make("Escape Lens Attachments", bytemuck::cast_slice(&attachments));
        let subflame_meta = make("Escape Lens Subflame Meta", bytemuck::cast_slice(&metas));
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Escape Lens BG"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: transforms.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: flame_params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: variation_params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 10, resource: attachments.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 12, resource: subflame_meta.as_entire_binding() },
            ],
        });
        Self {
            key,
            transforms,
            flame_params,
            variation_params,
            attachments,
            subflame_meta,
            bind_group,
        }
    }

    /// Rewrite the transform row.
    ///
    /// This is how the AMOUNT gets to the shader without a recompile:
    /// it rides in the transform's weight, so dragging that slider is
    /// a buffer write rather than a new pipeline.
    pub fn write(&self, queue: &wgpu::Queue, flame: &Flame) {
        use crate::gpu::buffers::{pack_gpu_transforms, pack_gpu_variation_params};
        let transforms = pack_gpu_transforms(flame, crate::scene::transforms::RenderMode::TwoD);
        let vparams = pack_gpu_variation_params(flame);
        queue.write_buffer(&self.transforms, 0, bytemuck::cast_slice(&transforms[..1]));
        queue.write_buffer(&self.variation_params, 0, bytemuck::cast_slice(&vparams[..1]));
        // Referenced so the buffers outlive the bind group; nothing
        // else writes them.
        let _ = (&self.flame_params, &self.attachments, &self.subflame_meta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(name: &str, amount: f32) -> EscapeConfig {
        let mut e = EscapeConfig::default();
        e.lens = name.to_string();
        e.lens_amount = amount;
        e
    }











    #[test]
    fn no_lens_is_no_source() {
        let r = crate::variations::global_registry();
        assert!(lens_source(&EscapeConfig::default(), &r).is_none());
        // A name that is not a variation, and a zero amount, are both
        // "no lens" rather than a broken one.
        assert!(lens_source(&cfg("not_a_variation", 1.0), &r).is_none());
        assert!(lens_source(&cfg("eyefish", 0.0), &r).is_none());
    }

    #[test]
    fn the_lens_flame_is_the_variation_and_nothing_else() {
        let r = crate::variations::global_registry();
        let f = lens_flame(&cfg("eyefish", 0.6), &r).expect("a lens");
        assert_eq!(f.transforms.len(), 1);
        let t = &f.transforms[0];
        // a and D are the diagonal, e and f the translation.
        assert_eq!([t.a, t.b, t.c, t.d, t.e, t.f], [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        // Negated: the slider is a camera control and the weight is
        // the raw blend. See `lens_flame`.
        assert!((t.weight + 0.6).abs() < 1e-6, "the amount rides in the weight");
        assert_eq!(t.variations.len(), 1);
        assert!(t.variations.contains_key("eyefish"));
    }

    /// A parameter must reach the flame, or the panel would edit a
    /// value the shader never reads -- which is the whole point of
    /// storing them.
    #[test]
    fn parameters_reach_the_lens_flame() {
        let r = crate::variations::global_registry();
        let name = r
            .names()
            .iter()
            .find(|n| r.get(n).is_some_and(|i| !i.parameters.is_empty()))
            .expect("some variation has parameters")
            .to_string();
        let param = r.get(&name).unwrap().parameters[0].name.clone();

        let mut e = cfg(&name, 1.0);
        e.lens_params.insert(param.clone(), 0.375);
        let f = lens_flame(&e, &r).expect("a lens");
        let got = f.transforms[0].get_variation_param_or_default(&name, &param, &r);
        assert!((got - 0.375).abs() < 1e-6, "{name}.{param} read back {got}");
    }

    /// The slider is a camera control: POSITIVE bulges the middle
    /// out, and that is the variation run backwards.
    ///
    /// `mix(n, L(n), w)` is `n + w(L(n) - n)`, so the sign of the
    /// blend is the sign of the displacement and nothing about `L`
    /// enters. Which sign BULGES is a property of the variation, and
    /// the measured profiles agree for every radial lens in the
    /// recommended set: magnification at radius r is `r / L_w(r)`,
    /// which rises with r at a positive raw blend -- the rim
    /// stretched more than the middle, reading as the centre pushed
    /// in -- and falls with r at a negative one.
    ///
    /// So the config's amount is negated on its way to the weight,
    /// and this pins that. It was reported twice from use before it
    /// was believed: first for `eyefish`, then for `bubble`.
    #[test]
    fn a_positive_amount_bulges_and_reaches_the_weight_negated() {
        let r = crate::variations::global_registry();
        let mut out = EscapeConfig::default();
        out.lens = "bubble".to_string();
        out.lens_amount = 1.0;
        let mut inward = out.clone();
        inward.lens_amount = -1.0;

        assert!(is_active(&out, &r) && is_active(&inward, &r));
        assert_eq!(
            lens_flame(&out, &r).unwrap().transforms[0].weight,
            -1.0,
            "a positive amount must reach the shader negated"
        );
        assert_eq!(lens_flame(&inward, &r).unwrap().transforms[0].weight, 1.0);

        // One pipeline serves both: the amount is not in the key, so
        // flipping the sign must not recompile a shader.
        assert_eq!(lens_key(&out, &r), lens_key(&inward, &r));

        // The range reaches its edge, and zero is still no lens.
        let mut far = out.clone();
        far.lens_amount = LENS_AMOUNT_LIMIT;
        assert_eq!(
            lens_flame(&far, &r).unwrap().transforms[0].weight,
            -LENS_AMOUNT_LIMIT
        );
        let mut off = out.clone();
        off.lens_amount = 0.0;
        assert!(!is_active(&off, &r), "zero is the identity either way");
    }

    /// The picker's list: the recommended head in its own order, then
    /// every other variation that measured usable, and nothing that
    /// did not.
    #[test]
    fn the_menu_recommends_first_and_offers_only_what_works() {
        let r = crate::variations::global_registry();
        let menu = lens_menu(&r);

        // Every name in either list is a real variation -- a rename
        // in the registry would otherwise silently shrink the picker.
        for n in LENS_RECOMMENDED {
            assert!(r.get(n).is_some(), "recommended `{n}` is not a variation");
            assert!(lens_is_usable(n), "recommended `{n}` did not measure usable");
        }
        for n in LENS_USABLE {
            assert!(r.get(n).is_some(), "usable `{n}` is not a variation");
        }

        // The head is first, in order.
        let head: Vec<&str> = LENS_RECOMMENDED.iter().copied().collect();
        assert_eq!(&menu[..head.len()], &head[..], "the head is not first");
        assert_eq!(&menu[0], "bubble");
        assert_eq!(&menu[1], "hemisphere");
        assert_eq!(&menu[2], "eyefish");

        // Nothing unusable, and no duplicates.
        for n in &menu {
            assert!(lens_is_usable(n), "`{n}` is offered but did not measure usable");
        }
        let mut sorted = menu.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), menu.len(), "the menu repeats a lens");

        // The measured-bad are really gone: `zcone` and `zscale`
        // return nothing in two dimensions, and `blur`, `noise` and
        // `gaussian_blur` scatter.
        //
        // Note which name is NOT here. `blur_zoom` draws from the RNG
        // and is offered anyway, because it MEASURED clean -- its
        // jitter is small enough to stay a lens, and a zoom blur is a
        // camera effect. RNG is not the criterion; the measurement is.
        for n in ["zcone", "zscale", "blur", "noise", "gaussian_blur"] {
            if r.get(n).is_some() {
                assert!(!menu.iter().any(|m| m == n), "`{n}` should not be offered");
            }
        }

        // The list is sorted, because `lens_is_usable` binary-searches it.
        let mut owned: Vec<&str> = LENS_USABLE.to_vec();
        owned.sort();
        assert_eq!(owned, LENS_USABLE.to_vec(), "LENS_USABLE is not sorted");
    }

    /// The key must move when a parameter does, or a lens edit would
    /// reuse the pipeline compiled for the previous shape.
    #[test]
    fn the_key_follows_the_parameters_but_not_the_amount() {
        let r = crate::variations::global_registry();
        let a = cfg("eyefish", 1.0);
        let mut b = cfg("eyefish", 0.25);
        assert_eq!(lens_key(&a, &r), lens_key(&b, &r), "the amount must not recompile");
        b.lens_params.insert("power".into(), 3.0);
        assert_ne!(lens_key(&a, &r), lens_key(&b, &r), "a parameter must recompile");
        assert_eq!(lens_key(&EscapeConfig::default(), &r), "");
    }

    /// The source must land on the lens group, not the simulation's.
    #[test]
    fn the_lens_binds_above_mode_ds_group() {
        let r = crate::variations::global_registry();
        let src = lens_source(&cfg("eyefish", 1.0), &r).expect("a lens");
        assert!(src.contains("@group(2) @binding("), "not on the lens group");
        assert!(!src.contains("@group(1) @binding("), "collides with mode D");
        assert!(src.contains("fn esc_lens"), "no glue");
        assert!(src.contains("fn flame_map"), "no map");
    }
}

#[cfg(test)]
mod gpu_tests {
    use super::*;
    use crate::config::FractalConfig;
    use crate::scene::transforms::RenderMode;

    const W: u32 = 96;
    const H: u32 = 96;

    fn device() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .expect("no GPU adapter");
        let mut limits = wgpu::Limits::default();
        let al = adapter.limits();
        limits.max_storage_buffer_binding_size = al.max_storage_buffer_binding_size;
        limits.max_buffer_size = al.max_buffer_size;
        limits.max_storage_buffers_per_shader_stage = al.max_storage_buffers_per_shader_stage;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("lens test"),
            required_features: wgpu::Features::CLEAR_TEXTURE,
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        }))
        .expect("device")
    }

    fn config(lens: &str, amount: f32) -> FractalConfig {
        let mut c = FractalConfig::default();
        c.render_mode = RenderMode::Escape;
        c.escape.formula = "mandelbrot".to_string();
        c.escape.coloring = "smooth".to_string();
        c.escape.max_iter = 64;
        c.escape.lens = lens.to_string();
        c.escape.lens_amount = amount;
        c.tonemap_mode = crate::scene::tonemap::ToneMapMode::Linear;
        c.exposure = crate::config::defaults::DEFAULT_EXPOSURE;
        c.gamma = crate::config::defaults::DEFAULT_GAMMA;
        c
    }

    fn render(device: &wgpu::Device, queue: &wgpu::Queue, c: &FractalConfig) -> Vec<u8> {
        let job = crate::renderer::RenderJob::new(c, W, H);
        pollster::block_on(crate::renderer::render(
            device,
            queue,
            job,
            &mut crate::renderer::NoProgress,
        ))
        .expect("render")
        .rgba_data
    }

    fn differing(a: &[u8], b: &[u8]) -> usize {
        a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count()
    }

    /// End to end on a real device: a lens changes the picture, an
    /// amount of zero does not, and the amount is continuous between.
    ///
    /// The zero case is the one worth having. It proves the lens is
    /// genuinely a blend toward the identity rather than an effect
    /// that merely fades, and it proves the group-2 bind group does
    /// not disturb a render that asks for nothing.
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn a_lens_bends_the_view_and_an_amount_of_zero_does_not() {
        let (device, queue) = device();
        let plain = render(&device, &queue, &config("", 1.0));
        let off = render(&device, &queue, &config("eyefish", 0.0));
        assert_eq!(differing(&plain, &off), 0, "amount 0 changed the picture");

        let full = render(&device, &queue, &config("eyefish", 1.0));
        let half = render(&device, &queue, &config("eyefish", 0.5));
        let n = (W * H) as usize;
        let d_full = differing(&plain, &full);
        let d_half = differing(&plain, &half);
        println!("  eyefish: full {d_full}/{n} px, half {d_half}/{n} px");
        assert!(d_full > n / 4, "a full lens barely moved the picture: {d_full}/{n}");
        assert!(d_half > 0 && d_half < d_full, "the amount is not a blend: {d_half} vs {d_full}");
    }


    /// If the map returned zero, a linear lens at amount 0.5 would be
    /// exactly a 2x zoom -- mix(n, 0, 0.5) = n/2. Decisive between
    /// "the map is identity" and "the transforms buffer is empty".
    #[test]

    /// The lens reaches every path that renders headlessly.
    ///
    /// The perturbed kernel is not among them -- it acquires its
    /// reference orbit progressively, so a single headless render of
    /// a deep zoom is entirely unlit and a pixel comparison there
    /// compares two black images. It is gated at the shader instead,
    /// by `every_template_applies_the_lens`.
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn every_headless_path_carries_the_lens() {
        let (device, queue) = device();
        let n = (W * H) as usize;

        let paths: Vec<(&str, fn(&mut FractalConfig))> = vec![
            ("direct", |_c| {}),
            ("field", |c| {
                c.escape.formula = "weierstrass".to_string();
                c.escape.coloring = "field_value".to_string();
            }),
        ];

        for (label, tweak) in paths {
            let mut plain = config("", 1.0);
            tweak(&mut plain);
            let mut lensed = config("eyefish", 1.0);
            tweak(&mut lensed);
            let mut lin = config("linear", 1.0);
            tweak(&mut lin);

            let a = render(&device, &queue, &plain);
            let lit = a.chunks(4).filter(|p| p[0] > 4 || p[1] > 4 || p[2] > 4).count();
            assert!(lit > n / 10, "{label}: nothing rendered, so nothing is proven");

            let d = differing(&a, &render(&device, &queue, &lensed));
            println!("  {label}: lens moved {d}/{n} px of {lit} lit");
            assert!(d > n / 50, "{label}: the lens did not reach this path ({d}/{n})");
            assert_eq!(
                differing(&a, &render(&device, &queue, &lin)),
                0,
                "{label}: linear is not the identity"
            );
        }
    }


    /// A lens PARAMETER reaches the shader.
    ///
    /// This is the end of the chain the panel edits: config -> flame
    /// -> packed variation params -> get_param -> the picture. A
    /// parameter that only reached the config would leave the slider
    /// looking functional and doing nothing.
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn a_lens_parameter_reaches_the_picture() {
        let (device, queue) = device();
        // `curl` takes c1/c2 and is the identity at both zero.
        let mut a = config("curl", 1.0);
        a.escape.lens_params.insert("c1".into(), 0.0);
        a.escape.lens_params.insert("c2".into(), 0.0);
        let mut b = config("curl", 1.0);
        b.escape.lens_params.insert("c1".into(), 0.8);
        b.escape.lens_params.insert("c2".into(), 0.0);

        let flat = render(&device, &queue, &a);
        let curled = render(&device, &queue, &b);
        let n = (W * H) as usize;
        let d = differing(&flat, &curled);
        println!("  curl c1 0 -> 0.8: {d}/{n} px");
        assert!(d > n / 20, "the parameter did not reach the shader: {d}/{n}");

        // And at c1 = c2 = 0 curl IS the identity, so it must match an
        // unlensed render exactly -- which pins the units and the
        // affine convention, not just the wiring. `linear` alongside
        // it is the same assertion with nothing to go wrong in the
        // variation: it is what caught the transform being built with
        // a = e = 1, which is a shear onto the line y = 0 rather than
        // the identity this engine's `apply_affine` means.
        let plain = render(&device, &queue, &config("", 1.0));
        let lin = render(&device, &queue, &config("linear", 1.0));
        assert_eq!(differing(&plain, &lin), 0, "linear as a lens is not the identity");
        assert_eq!(differing(&plain, &flat), 0, "curl at zero is not the identity");
    }
}

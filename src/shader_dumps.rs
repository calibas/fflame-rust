//! Canonical dumps of the generated flame WGSL, tracked as golden files
//! under `tests/shader_dumps/`.
//!
//! The flame shader is assembled at runtime from templates plus the
//! active variations' inline bodies, so shader changes are invisible in
//! review: a diff shows the template or variation edit, never the WGSL
//! it produces. These dumps close that gap. A commit that changes
//! codegen carries the before/after WGSL next to the source change, and
//! an *unintended* change — a template flag that starts firing, a helper
//! that stops being emitted — fails the test instead of shipping quietly.
//!
//! Regenerate after an intended change, then review the diff as part of
//! the commit:
//!
//! ```text
//! UPDATE_SHADER_DUMPS=1 cargo test --lib canonical_shader_dumps
//! ```
//!
//! # What the axes are, and aren't
//!
//! The set covers **in-app vs export**, a real fork in the emitted code:
//! in-app plots with atomic adds straight into the u32 histogram, while
//! large exports write `Sample` structs into a points buffer for a later
//! scatter pass (`output_histogram_direct`). Beyond that it spans 2D/3D,
//! solid rendering, multi-emit, and the path-tracking/xaos flags.
//!
//! It deliberately does **not** split desktop vs WASM, or Vulkan vs
//! Metal. Codegen is a pure function from (flame, flags, constants) to
//! WGSL text — it never sees the platform or the backend, so per-platform
//! sets would be byte-identical copies. `codegen_has_no_platform_forks`
//! keeps that true rather than assumed.

use crate::scene::transforms::{Flame, Transform};
use crate::shader_builder_v2::{ShaderBuilder, ShaderConstants};
use std::collections::HashMap;
use std::path::Path;

const DUMP_DIR: &str = "tests/shader_dumps";

/// One canonical shader, named by the path it represents in the app.
struct Spec {
    file: &'static str,
    /// Variations per transform — the outer slice is the transform list,
    /// so `&[&["linear"], &["linear", "spray_blur"]]` is a two-transform
    /// flame whose second transform carries an emitter.
    transforms: &'static [&'static [&'static str]],
    render_3d: bool,
    path_tracking: bool,
    xaos: bool,
    /// `true` = in-app atomic histogram writes, `false` = the sample-emit
    /// path large exports take.
    direct_histogram: bool,
    solid: bool,
}

/// Baseline spec; each entry below overrides only what it exercises.
const BASE: Spec = Spec {
    file: "",
    transforms: &[&["linear"]],
    render_3d: false,
    path_tracking: false,
    xaos: false,
    direct_histogram: true,
    solid: false,
};

const SPECS: &[Spec] = &[
    // ---- in-app (atomic direct-histogram) ----
    Spec { file: "in-app-2d.wgsl", ..BASE },
    Spec { file: "in-app-3d.wgsl", render_3d: true, ..BASE },
    Spec { file: "in-app-3d-solid.wgsl", render_3d: true, solid: true, ..BASE },
    Spec {
        file: "in-app-2d-multi-emit.wgsl",
        transforms: &[&["linear"], &["linear", "spray_blur"]],
        ..BASE
    },
    Spec {
        file: "in-app-2d-path-xaos.wgsl",
        transforms: &[&["linear"], &["spherical"]],
        path_tracking: true,
        xaos: true,
        ..BASE
    },
    // ---- export (sample-emit into a points buffer) ----
    Spec { file: "export-2d.wgsl", direct_histogram: false, ..BASE },
    Spec { file: "export-3d.wgsl", render_3d: true, direct_histogram: false, ..BASE },
    Spec {
        file: "export-2d-multi-emit.wgsl",
        transforms: &[&["linear"], &["linear", "spray_blur"]],
        direct_histogram: false,
        ..BASE
    },
];

impl Spec {
    /// Build this spec's WGSL exactly as the app would.
    fn generate(&self) -> String {
        let mut flame = Flame::new();
        let mut active: HashMap<String, f32> = HashMap::new();
        for names in self.transforms {
            let mut xf = Transform::new();
            for name in *names {
                // set_variation (not a raw map insert) so variation_order
                // is recorded — that's what fixes dispatch order, and
                // therefore the local index map the shader is built around.
                xf.set_variation(name, 1.0);
                active.insert((*name).to_string(), 1.0);
            }
            flame.transforms.push(xf);
        }

        let constants = ShaderConstants {
            num_transforms: flame.transforms.len() as u32,
            solid_enabled: self.solid,
            probe: false,
            ..ShaderConstants::default()
        };

        let builder = ShaderBuilder::new(crate::variations::global_registry().clone());
        builder.build_from_template(
            &flame,
            &active,
            self.render_3d,
            self.path_tracking,
            self.xaos,
            self.direct_histogram,
            &constants,
        )
    }
}

/// Canonical form: LF endings only.
///
/// Templates and variation bodies are embedded from the working copy, so
/// on a `core.autocrlf` checkout the generated shader arrives as a MIX —
/// CRLF for the parts that came from `shaders/*.wgsl` and `.rs` sources,
/// LF for the parts the builder writes itself. Normalizing both sides
/// makes a dump a property of the code rather than of whoever's checkout
/// generated it.
fn canonical(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// First differing line between stored and freshly generated WGSL.
fn first_difference(stored: &str, fresh: &str) -> String {
    let clip = |s: &str| s.chars().take(72).collect::<String>();
    for (i, (a, b)) in stored.lines().zip(fresh.lines()).enumerate() {
        if a != b {
            return format!(
                "line {}: stored `{}` vs generated `{}`",
                i + 1,
                clip(a.trim()),
                clip(b.trim())
            );
        }
    }
    format!(
        "line counts differ: stored {}, generated {}",
        stored.lines().count(),
        fresh.lines().count()
    )
}

#[test]
fn canonical_shader_dumps() {
    let dir = Path::new(DUMP_DIR);
    let update = std::env::var_os("UPDATE_SHADER_DUMPS").is_some();
    if update {
        std::fs::create_dir_all(dir).expect("create dump dir");
    }

    let mut problems = Vec::new();
    for spec in SPECS {
        let fresh = canonical(&spec.generate());
        let path = dir.join(spec.file);
        if update {
            std::fs::write(&path, fresh.as_bytes()).expect("write dump");
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(stored) => {
                let stored = canonical(&stored);
                if stored != fresh {
                    problems.push(format!("  {} — {}", spec.file, first_difference(&stored, &fresh)));
                }
            }
            Err(e) => problems.push(format!("  {} — unreadable ({e})", spec.file)),
        }
    }

    assert!(
        problems.is_empty(),
        "generated WGSL no longer matches the canonical dumps:\n{}\n\n\
         If the change is intended, regenerate and review the WGSL diff \
         alongside your source change:\n    \
         UPDATE_SHADER_DUMPS=1 cargo test --lib canonical_shader_dumps",
        problems.join("\n")
    );
}

/// One dump set is valid for every platform only while codegen has no
/// platform forks. Guard the assumption: a conditional compiled into the
/// builder would make desktop and WASM diverge silently, and these dumps
/// — which only ever run on the host — would keep passing.
///
/// Scope is the pure text-generation path. Platform-dependent *inputs*
/// (device limits picking an export path, say) are fine and are not
/// scanned: the specs above pin those inputs explicitly.
#[test]
fn codegen_has_no_platform_forks() {
    // Split so the needle doesn't appear literally in a scanned file —
    // this test's own source is not scanned, but the builder's is.
    let needle = concat!("target_", "arch");
    let sources: &[(&str, &str)] = &[
        ("src/shader_builder_v2.rs", include_str!("shader_builder_v2.rs")),
        ("shaders/core/main_template.wgsl", include_str!("../shaders/core/main_template.wgsl")),
        ("shaders/core/header.wgsl", include_str!("../shaders/core/header.wgsl")),
    ];
    for (name, text) in sources {
        assert!(
            !text.contains(needle),
            "{name} gained a platform conditional. Generated WGSL is currently \
             identical on desktop and WASM, which is why tests/shader_dumps/ holds \
             ONE set. If the divergence is real, split the dumps per platform; if \
             not, remove the conditional."
        );
    }
}

//! Generating the probe shader for a batch.
//!
//! Thin by design: everything of substance lives in the ordinary shader
//! builder, and the only thing this adds is the `PROBE` constant and the
//! layout of the histogram buffer the entry point reads and writes.

use super::batch::Batch;
use crate::shader_builder_v2::{ShaderBuilder, ShaderConstants};
use std::collections::HashMap;

/// Entry point name in the generated module.
pub const ENTRY_POINT: &str = "probe_main";

/// Start of the probe block in `main_template.wgsl`. Everything before
/// it must be identical whether the flag is set or not.
///
/// The leading newline is part of the marker on purpose. The gated
/// block's opening tag is followed by a newline, and the template
/// processor strips the tag but not that newline — so with the flag on,
/// it lands in front of this comment. Folding it into the marker is what
/// lets the identity check compare exactly rather than trimming, and
/// trimming is what previously hid a stray blank line all the way into
/// every canonical shader dump.
pub const BLOCK_MARKER: &str = "\n// PROBE-BLOCK-BEGIN";

/// `u32`s of header before the input points: the point count and the
/// transform count. Must match the block in `main_template.wgsl`.
pub const HEADER_WORDS: usize = 2;

/// `u32`s per input point and per output slot. Four rather than three so
/// both regions stay 16-byte aligned, which keeps the offset arithmetic
/// on the shader side a shift.
pub const WORDS_PER_SLOT: usize = 4;

/// Where the outputs start, in `u32`s.
pub fn output_base(point_count: usize) -> usize {
    HEADER_WORDS + point_count * WORDS_PER_SLOT
}

/// Total `u32`s the probe buffer needs.
pub fn buffer_words(point_count: usize, xform_count: usize) -> usize {
    output_base(point_count) + point_count * xform_count * WORDS_PER_SLOT
}

/// Build the WGSL for one batch.
pub fn build(batch: &Batch, render_3d: bool) -> String {
    let flame = super::flame::build_probe_flame(batch);

    let constants = ShaderConstants {
        num_transforms: flame.transforms.len() as u32,
        probe: true,
        ..ShaderConstants::default()
    };

    let builder = ShaderBuilder::new(crate::variations::global_registry().clone());
    builder.build_from_template(
        &flame,
        // Unused by the builder — the local index map comes from the
        // flame's own variation order.
        &HashMap::new(),
        render_3d,
        false, // path tracking: the probe never plots
        false, // xaos: one transform per variation, no selection
        // Direct histogram, because the probe's I/O rides on that
        // binding. On the sample-emit path binding 2 is `samples`.
        true,
        &constants,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::batch::{builtin_targets, plan_batches};

    fn validate(source: &str, what: &str) {
        let module = match wgpu::naga::front::wgsl::parse_str(source) {
            Ok(m) => m,
            Err(e) => panic!("{what}: probe WGSL fails to parse\n{}", e.emit_to_string(source)),
        };

        // Parsing only proves it is well-formed text. Validation is what
        // catches a call whose argument list does not match the
        // generated signature — precisely the drift this design exists
        // to prevent, so it is worth checking rather than assuming.
        let mut validator = wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        );
        if let Err(e) = validator.validate(&module) {
            panic!("{what}: probe WGSL fails validation\n{}", e.emit_to_string(source));
        }
    }

    /// The one that matters: every shipped variation, in the batches the
    /// probe will actually use, in both dimensions.
    ///
    /// A flame of 99 unrelated variations is far outside anything the
    /// app builds — mixed phases, mixed features, every helper library
    /// spliced in at once. If the shader builder has a combination it
    /// cannot handle, this finds it without needing a GPU.
    #[test]
    fn every_batch_compiles_in_both_dimensions() {
        let batches = plan_batches(&builtin_targets());
        assert!(!batches.is_empty(), "no variations to probe");

        for (i, batch) in batches.iter().enumerate() {
            for render_3d in [false, true] {
                let dim = if render_3d { "3d" } else { "2d" };
                let source = build(batch, render_3d);
                assert!(
                    source.contains(&format!("fn {ENTRY_POINT}")),
                    "batch {i} {dim}: PROBE flag did not emit the entry point"
                );
                validate(&source, &format!("batch {i} ({dim}, {} variations)", batch.targets.len()));
            }
        }
    }

    /// The byte-identity contract, the same one `solid_enabled` holds
    /// to. A diagnostic that changes the code it measures measures the
    /// wrong code.
    #[test]
    fn probe_off_is_byte_identical() {
        let batches = plan_batches(&builtin_targets());
        let batch = &batches[0];
        let flame = crate::probe::flame::build_probe_flame(batch);
        let builder = ShaderBuilder::new(crate::variations::global_registry().clone());

        for render_3d in [false, true] {
            let off = builder.build_from_template(
                &flame,
                &HashMap::new(),
                render_3d,
                false,
                false,
                true,
                &ShaderConstants {
                    num_transforms: flame.transforms.len() as u32,
                    probe: false,
                    ..ShaderConstants::default()
                },
            );
            assert!(
                !off.contains(ENTRY_POINT),
                "PROBE=false still emitted the probe entry point"
            );

            let on = build(batch, render_3d);
            // Everything before the probe block must be untouched.
            let head = on
                .split_once(BLOCK_MARKER)
                .map(|(head, _)| head)
                .expect("the probe block should carry its begin marker");
            // Compared exactly, not trimmed. Trailing whitespace is
            // still a difference, and tolerating it here let a stray
            // blank line and a CRLF/LF mismatch through — both of which
            // showed up as two extra lines in every canonical shader
            // dump, which is where they should never have reached.
            assert_eq!(
                off, head,
                "enabling PROBE perturbed the rest of the shader"
            );
        }
    }

    #[test]
    fn the_buffer_layout_leaves_no_overlap() {
        let (points, xforms) = (27, 99);
        let base = output_base(points);
        assert!(
            base >= HEADER_WORDS + points * WORDS_PER_SLOT,
            "outputs would overwrite the inputs the shader still has to read"
        );
        assert_eq!(buffer_words(points, xforms), base + points * xforms * WORDS_PER_SLOT);
    }
}

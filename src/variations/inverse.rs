//! What a variation tells the inverse walks about itself.
//!
//! `docs/projects/ifs-general.md` D1. The analysis used to decide
//! which variations the walks can invert with two `match`es on the
//! variation NAME -- one in `variation_stage` to notice a kernel, one
//! in `transform_map_2d_ordered` (and its solid twin) to build it
//! from the transform's parameters. A seventh kernel meant editing
//! both, in a file that knows nothing about the variation, and the
//! gates that check the six were a hand-written list.
//!
//! Here instead each kernel is an [`InverseDef`] living in the same
//! file as the variation's forward WGSL, and [`INVERSES`] is the
//! append-only list of them. The analysis asks
//! [`VariationRegistry::inverse`](super::VariationRegistry::inverse),
//! and the gates iterate the list, so a seventh kernel gets the six's
//! gates by being registered.
//!
//! **Why a side list rather than a field on `VariationDef`.** The
//! plan asked for `inverse: Option<&'static InverseDef>` on the
//! definition, "absent by default so the other 640 definitions do not
//! move". Rust has no default for a field of a plain struct literal,
//! so that field would mean adding `inverse: None,` to all 647 of
//! them -- 647 lines of noise to reach seven. The list is keyed by
//! `VariationDef::name` and `inverse()` resolves through the registry,
//! so a name with no variation has no inverse and an alias finds the
//! canonical one; what the plan wanted from the field is what this
//! gives, without the bulk edit.

use crate::scene::ifs_analysis::{Kernel, Kernel3};

/// A variation's parameters, by name, at the transform being
/// analysed. The kernel constructors read their own parameters
/// through this rather than taking a `Transform` and a registry,
/// which would make every definition file depend on both.
pub type ParamFn<'a> = &'a dyn Fn(&str) -> f64;

/// Why a variation that HAS an inverse still cannot supply one at
/// these parameters.
///
/// The caller turns this into the transform's own refusal, naming
/// the variation, so the flame panel says which one and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The parameters make the map singular or many-valued in a way
    /// the walk has no branch rule for -- a zero power, a zero
    /// distance, a scale that reaches zero.
    Degenerate,
    /// The variation is in a MODE the walk does not invert, though
    /// another setting of the same variation would be fine.
    Mode,
}

/// The kernel a variation is, in the space it is a kernel in.
///
/// A variation is planar or solid here, never both: the planar
/// kernels are two-dimensional maps and the solid ones move a point
/// in space, and nothing in the catalogue is a kernel in both senses.
pub enum InverseKernel {
    Planar(fn(ParamFn) -> Result<Kernel, Refusal>),
    Solid(fn(ParamFn) -> Result<Kernel3, Refusal>),
}

/// One variation's contribution to the inverse walks.
pub struct InverseDef {
    /// The variation this inverts, spelled as its
    /// [`VariationDef::name`](super::definition::VariationDef::name).
    pub name: &'static str,
    /// The kernel, given the transform's parameters.
    pub kernel: InverseKernel,
}

impl InverseDef {
    /// Whether this kernel lives in the plane.
    pub fn is_planar(&self) -> bool {
        matches!(self.kernel, InverseKernel::Planar(_))
    }
}

/// Every registered inverse. **Append-only**, like the variation
/// registration list, and for the same reason: the gates iterate it,
/// so the order is what a failure report names.
pub static INVERSES: &[&InverseDef] = &[
    &super::defs::INVERSE_JULIA,
    &super::defs::INVERSE_JULIAN,
    &super::defs::INVERSE_SPHERICAL,
    &super::defs::INVERSE_BUBBLE,
    &super::defs::INVERSE_HEMISPHERE,
    &super::defs::INVERSE_DISC,
    &super::defs::INVERSE_BLOB,
    &super::defs::INVERSE_JULIA3D,
    &super::defs::INVERSE_JULIA3DZ,
    &super::defs::INVERSE_QUATERNION_JULIA,
];

/// The inverse registered for `name`, without going through a
/// registry. [`VariationRegistry::inverse`](super::VariationRegistry::inverse)
/// is the call the analysis makes; this is what it looks up in.
pub fn for_name(name: &str) -> Option<&'static InverseDef> {
    INVERSES.iter().copied().find(|d| d.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every registered inverse names a variation that exists, and no
    /// variation is named twice.
    ///
    /// The list is written by hand, so this is the check that a typo
    /// in a name does not silently disqualify a flame that should
    /// have worked -- the analysis would simply never find the
    /// kernel, and the transform would be refused as `NotAffine`
    /// with no hint that the entry was meant to be there.
    #[test]
    fn every_registered_inverse_names_a_variation() {
        let registry = crate::variations::global_registry();
        let mut seen: Vec<&str> = Vec::new();
        for d in INVERSES {
            assert!(
                registry.get(d.name).is_some(),
                "inverse {:?} names no registered variation",
                d.name
            );
            assert!(!seen.contains(&d.name), "inverse {:?} registered twice", d.name);
            seen.push(d.name);
        }
        assert_eq!(seen.len(), 10, "ten inverses: seven planar, three solid");
    }
}

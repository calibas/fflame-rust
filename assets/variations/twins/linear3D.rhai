/// JWildfire / Apophysis 7X `linear3D` — identity transform that also
/// carries Z unconditionally (`Linear3DFunc.java`: `pVarTP.z +=
/// pAmount * pAffineTP.z`, no preserve_z gate). This is what lets Z
/// compound across chaos-game iterations in 3D flames; see
/// `Feature::AlwaysZ`. Body is identical to `linear` — the dispatch
/// metadata is the whole difference.
///
/// # Authors
/// - Scott Draves

// Linear 3D, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Linear 3D", name: "rhai_linear3d", category: "depth_3d", phase: "any" };

fn variation(z, p) {
    z
}

fn variation3(v, p) {
    v
}

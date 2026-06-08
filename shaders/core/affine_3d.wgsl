// Affine transformations for 3D mode
//
// Two paths, gated by `xform.plane_flags`:
//
//   * "Flat" path (plane_flags == 0): standard Apophysis affine —
//     `x' = ax + by + e`, `y' = cx + dy + f`, `z' = z + g`. Matches the
//     pre-extension math byte-for-byte so existing Apo / JWF flames
//     with no `yzCoefs` / `zxCoefs` render unchanged. The dispatch
//     mirrors JWildfire's `TransformationAffineFlatStep`.
//
//   * "Full" path (any plane flag set): JWildfire's
//     `TransformationAffineFullStep` composition — sequential linear
//     parts in fixed XY → YZ → ZX order with each plane's 2×2,
//     then ALL six offsets summed at the end without being rotated
//     by subsequent matrices. This is *not* mathematically true
//     sequential affine composition (in which earlier offsets would
//     propagate through later rotations); JWildfire decouples linear
//     and offset on purpose. See
//     `docs/projects/jwf-features.md` ("zxCoefs / yzCoefs") for the
//     full reasoning. We mirror their math exactly so on-disk values
//     produce the same render in both apps.

// Apply pre-affine transformation (3D)
fn apply_affine(xform: Transform, p: vec3<f32>) -> vec3<f32> {
    // Bits 0 (YZ) and 1 (ZX) — see `PLANE_FLAG_*` consts on the
    // Rust side. Zero → no JWildfire 3D plane affines are active,
    // take the cheap Apophysis path.
    if ((xform.plane_flags & 3u) == 0u) {
        return vec3<f32>(
            xform.a * p.x + xform.b * p.y + xform.e,
            xform.c * p.x + xform.d * p.y + xform.f,
            p.z + xform.g
        );
    }

    // Full path. XY linear is always applied (the standard `coefs`
    // attribute is present even on JWF flames). YZ and ZX are gated
    // by their bits so flames with only ZX skip the YZ math entirely.
    var x = xform.a * p.x + xform.b * p.y;
    var y = xform.c * p.x + xform.d * p.y;
    var z = p.z;

    // YZ step: rotate `(y, z)` within the YZ plane.
    // `yz_coefs` positions are JWildfire's 00/01/10/11/20/21 indexing
    // — see jwf-features.md table for the 12-coefficient mapping.
    if ((xform.plane_flags & 1u) != 0u) {
        let ny = xform.yz_coefs[0] * y + xform.yz_coefs[2] * z;
        let nz = xform.yz_coefs[1] * y + xform.yz_coefs[3] * z;
        y = ny;
        z = nz;
    }

    // ZX step: rotate `(x, z)`. Per JWildfire's source, the "first
    // axis" of the ZX plane is X (despite the plane's name), so
    // `zx_coefs[0]` multiplies x in the x-output, matching XForm.java.
    if ((xform.plane_flags & 2u) != 0u) {
        let nx = xform.zx_coefs[0] * x + xform.zx_coefs[2] * z;
        let nz = xform.zx_coefs[1] * x + xform.zx_coefs[3] * z;
        x = nx;
        z = nz;
    }

    // Offset sum. JWildfire's translation contributions:
    //   x_out = x + xy20 + zx20      = x + xform.e + zx_coefs[4]
    //   y_out = y + xy21 + yz20      = y + xform.f + yz_coefs[4]
    //   z_out = z + yz21 + zx21      = z + yz_coefs[5] + zx_coefs[5]
    // Note: JWildfire has no equivalent of our Apophysis `g` Z offset
    // when running the Full path — the Z translation rides on the
    // plane affines. A flame that mixes a non-zero `g` with active
    // plane affines silently loses `g` here (documented limitation).
    return vec3<f32>(
        x + xform.e + xform.zx_coefs[4],
        y + xform.f + xform.yz_coefs[4],
        z + xform.yz_coefs[5] + xform.zx_coefs[5]
    );
}

// Apply post-affine transformation (3D)
fn apply_post_affine(xform: Transform, p: vec3<f32>) -> vec3<f32> {
    // Bits 2 (YZ post) and 3 (ZX post). Same flat/full pattern as
    // `apply_affine`. The XY post is gated externally by
    // `xform.post_enabled` — when post is disabled this function
    // isn't called at all.
    if ((xform.plane_flags & 12u) == 0u) {
        return vec3<f32>(
            xform.post_a * p.x + xform.post_b * p.y + xform.post_e,
            xform.post_c * p.x + xform.post_d * p.y + xform.post_f,
            p.z + xform.post_g
        );
    }

    var x = xform.post_a * p.x + xform.post_b * p.y;
    var y = xform.post_c * p.x + xform.post_d * p.y;
    var z = p.z;

    if ((xform.plane_flags & 4u) != 0u) {
        let ny = xform.yz_post_coefs[0] * y + xform.yz_post_coefs[2] * z;
        let nz = xform.yz_post_coefs[1] * y + xform.yz_post_coefs[3] * z;
        y = ny;
        z = nz;
    }

    if ((xform.plane_flags & 8u) != 0u) {
        let nx = xform.zx_post_coefs[0] * x + xform.zx_post_coefs[2] * z;
        let nz = xform.zx_post_coefs[1] * x + xform.zx_post_coefs[3] * z;
        x = nx;
        z = nz;
    }

    return vec3<f32>(
        x + xform.post_e + xform.zx_post_coefs[4],
        y + xform.post_f + xform.yz_post_coefs[4],
        z + xform.yz_post_coefs[5] + xform.zx_post_coefs[5]
    );
}

// Rotate around X axis (affects Y and Z)
fn rotate_x(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    // Apophysis: z' = c*z - s*y, y' = s*z + c*y
    return vec3<f32>(
        p.x,
        s * p.z + c * p.y,
        c * p.z - s * p.y
    );
}

// Rotate around Y axis (affects X and Z)
fn rotate_y(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    // Apophysis: x' = c*x - s*z, z' = s*x + c*z
    return vec3<f32>(
        c * p.x - s * p.z,
        p.y,
        s * p.x + c * p.z
    );
}

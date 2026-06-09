// Utility functions shared by 2D and 3D shaders
//
// NOTE: `get_param` is generated per-flame by the shader builder
// (see `build_packed_get_param` in src/shader_builder_v2.rs) and
// injected before this file is included. The injected version uses
// per-variation packed offsets baked from the active variation set,
// so each variation occupies exactly `parameters.len() +
// init_param_count` slots in the variation_params buffer instead of
// a fixed 16.

// Select transform based on cumulative weights (uses params.num_transforms)
fn select_transform(rand_val: f32) -> u32 {
    var cumulative = 0.0;
    var total_weight = 0.0;

    // Calculate total weight
    for (var i = 0u; i < params.num_transforms; i++) {
        total_weight += transforms[i].weight;
    }

    let ttarget = rand_val * total_weight;

    for (var i = 0u; i < params.num_transforms; i++) {
        cumulative += transforms[i].weight;
        if (ttarget <= cumulative) {
            return i;
        }
    }

    return params.num_transforms - 1u;
}

// Select transform using hard-coded NUM_TRANSFORMS constant
// Enables loop unrolling optimization by compiler
fn select_transform_const(rand_val: f32) -> u32 {
    var cumulative = 0.0;
    var total_weight = 0.0;

    // Calculate total weight (loop can be unrolled with known NUM_TRANSFORMS)
    for (var i = 0u; i < NUM_TRANSFORMS; i++) {
        total_weight += transforms[i].weight;
    }

    let ttarget = rand_val * total_weight;

    for (var i = 0u; i < NUM_TRANSFORMS; i++) {
        cumulative += transforms[i].weight;
        if (ttarget <= cumulative) {
            return i;
        }
    }

    return NUM_TRANSFORMS - 1u;
}

// Select transform with xaos (chaos) weighting
// Uses hard-coded NUM_TRANSFORMS for loop unrolling
// prev_xform: Index of the transform that was just applied
// Xaos modifies probability: P(src→dst) = weight[dst] × xaos[src][dst]
fn select_transform_xaos(rand_val: f32, prev_xform: u32) -> u32 {
    var cumulative = 0.0;
    var total_weight = 0.0;

    // Base index into xaos_weights array for this source transform
    let xaos_base = prev_xform * NUM_TRANSFORMS;

    // Calculate total modified weight
    for (var i = 0u; i < NUM_TRANSFORMS; i++) {
        let base_weight = transforms[i].weight;
        let xaos_modifier = xaos_weights[xaos_base + i];
        total_weight += base_weight * xaos_modifier;
    }

    let threshold = rand_val * total_weight;

    // Select based on modified weights
    for (var i = 0u; i < NUM_TRANSFORMS; i++) {
        let base_weight = transforms[i].weight;
        let xaos_modifier = xaos_weights[xaos_base + i];
        cumulative += base_weight * xaos_modifier;
        if (threshold <= cumulative) {
            return i;
        }
    }

    return NUM_TRANSFORMS - 1u;
}

// sRGB → linear decoding. Palette colors (.flame XML hex, .palette JSON)
// are authored against an sRGB monitor; the pipeline math is linear. Use
// the gamma-2.2 approximation so encode (pow 1/2.2 at fragment-shader tail)
// composed with decode is identity on round-tripped values.
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
}

// Convert speed to color using palette lookup
fn speed_to_color(speed: f32) -> vec3<f32> {
    // Normalize speed to [0, 1] range
    // Use logarithmic scale for better visualization
    let normalized_speed = clamp(log(speed * 10.0 + 1.0) / 3.0, 0.0, 1.0);

    // Sample from palette texture (2D texture with height=1, so y=0.5)
    let srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(normalized_speed, 0.5), 0.0).rgb;
    return srgb_to_linear(srgb);
}

// Build the 4-angle Apophysis / JWildfire camera matrix.
//
// Transcribed verbatim from `output/FlameRendererView.java`'s
// `createProjectionMatrix(yaw, pitch, bank, roll)`. Each WGSL
// column holds `(JWF m[0][col], JWF m[1][col], JWF m[2][col])` so
// `camera_matrix * v` in WGSL produces the same result as JWF's
// row-major `m[row][col]` application.
//
// All four angles in radians. The argument convention (which
// parameter slot consumes which user-facing slider input) and
// the input sign tuning both live at the call site in
// `project_3d_to_2d_apophysis` — keeping this function as a
// faithful JWF transcription means future debugging can match
// the Java source character-by-character.
fn build_camera_matrix(yaw: f32, pitch: f32, bank: f32, roll: f32) -> mat3x3<f32> {
    let sy = sin(yaw);
    let cy = cos(yaw);
    let sp = sin(pitch);
    let cp = cos(pitch);
    let sb = sin(bank);
    let cb = cos(bank);
    let sr = sin(roll);
    let cr = cos(roll);

    return mat3x3<f32>(
        // Column 0 — populates JWF m[0][0], m[1][0], m[2][0]
        vec3<f32>(
            -cp*sr*sy - (sp*sb*sr - cb*cr)*cy,
            -cp*cy*sr + (sp*sb*sr - cb*cr)*sy,
             cb*sp*sr + cr*sb
        ),
        // Column 1 — JWF m[0][1], m[1][1], m[2][1]
        vec3<f32>(
             cp*cr*sy + (cr*sp*sb + cb*sr)*cy,
             cp*cr*cy - (cr*sp*sb + cb*sr)*sy,
            -cb*cr*sp + sb*sr
        ),
        // Column 2 — JWF m[0][2], m[1][2], m[2][2]
        vec3<f32>(
            -cp*cy*sb + sp*sy,
             cp*sb*sy + cy*sp,
             cp*cb
        )
    );
}

// Apply Apophysis camera transformation: translate by camera position,
// then rotate. Uses ALL nine matrix elements — the pre-port 2-angle
// matrix had `m[2][0] = 0` always, so this function used to drop the
// `m[2][0]·z_translated` contribution to x entirely. The new 4-angle
// matrix populates `m[2][0] = cos(bank)·sin(pitch)·sin(roll) +
// cos(roll)·sin(bank)`, so the term has to come back.
//
// The translation was Z-only pre-PR ("camera moves up and down");
// now it's the full position vector (camera_x, camera_y, camera_z)
// so the camera can be anywhere in world space. Existing flames have
// camera_x = camera_y = 0 by default so behavior is preserved.
fn camera_transform(p: vec3<f32>, camera_matrix: mat3x3<f32>, camera_pos: vec3<f32>) -> vec3<f32> {
    let p_t = p - camera_pos;
    let x = camera_matrix[0][0] * p_t.x + camera_matrix[1][0] * p_t.y + camera_matrix[2][0] * p_t.z;
    let y = camera_matrix[0][1] * p_t.x + camera_matrix[1][1] * p_t.y + camera_matrix[2][1] * p_t.z;
    let z = camera_matrix[0][2] * p_t.x + camera_matrix[1][2] * p_t.y + camera_matrix[2][2] * p_t.z;
    return vec3<f32>(x, y, z);
}

// Apply Apophysis perspective projection
// Source: Apophysis ControlPoint.pas:606
fn apply_perspective(p: vec3<f32>, persp_strength: f32) -> vec2<f32> {
    if (abs(persp_strength) < 1e-6) {
        // Orthographic: no perspective
        return p.xy;
    }

    // Apophysis formula: zr = 1 - cameraPersp * z
    let zr = 1.0 - persp_strength * p.z;

    // Avoid division by zero
    if (abs(zr) < 1e-6) {
        return p.xy;
    }

    // Perspective divide: x' = x / zr, y' = y / zr
    return p.xy / zr;
}

// Complete Apophysis 3D → 2D projection pipeline. Takes all four
// camera rotation angles (yaw, pitch, bank, roll) plus camera height
// and perspective strength. JWildfire's reference path negates yaw
// at the call site — we mirror that here so on-disk values render
// the same in both apps.
fn project_3d_to_2d_apophysis(
    p: vec3<f32>,
    pitch: f32,
    yaw: f32,
    bank: f32,
    roll: f32,
    camera_pos: vec3<f32>,
    persp_strength: f32
) -> vec2<f32> {
    // Empirical convention mapping — keeps the JWF matrix verbatim
    // and routes our slider inputs to the right slots here. Two
    // adjustments vs. the naive (yaw, pitch, bank, roll) pass-through:
    //
    //   1. **Yaw ↔ roll slot swap.** Our `yaw` parameter goes into
    //      the matrix's `roll` slot, and our `roll` parameter goes
    //      into the matrix's `yaw` slot. Without this, our Yaw
    //      slider produces look-axis behavior and our Rotation
    //      slider produces world-Z behavior — backwards from JWF.
    //      Almost certainly because JWildfire applies their matrix
    //      with a different convention internally (M^T·v or
    //      different basis), but rather than chase the underlying
    //      transform we swap at this call site.
    //
    //   2. **Sign tuning.** Each angle's input is negated or
    //      passed direct to match JWildfire's per-slider direction
    //      empirically. The negation pattern was tuned by testing
    //      each slider against JWildfire's renderer one axis at a
    //      time:
    //
    //          yaw    → roll slot,  direct
    //          pitch  → pitch slot, negated
    //          bank   → bank slot,  negated
    //          roll   → yaw slot,   direct
    //
    // Combined, this gives slider behavior matching JWildfire and
    // our pre-branch app for all four axes.
    let camera_matrix = build_camera_matrix(
        roll,     // matrix's `yaw` slot ← our `roll` input
        -pitch,
        -bank,
        yaw,      // matrix's `roll` slot ← our `yaw` input
    );
    let camera_space = camera_transform(p, camera_matrix, camera_pos);
    return apply_perspective(camera_space, persp_strength);
}

// Convert 3D fractal coords to pixel coords (with Apophysis camera system).
//
// In 3D mode the `rotation` field plays the role of JWildfire's *roll*
// angle (XML `rotate`, degrees → radians on import) and goes *into*
// the camera matrix as the roll argument — NOT applied as a 2D
// post-projection twist the way `world_to_pixel` (2D) does it. This
// matches JWildfire's composition order: roll is inside the 3D
// matrix, so it interacts correctly with pitch and yaw. The 2D
// version still applies `rotation` as a post-projection screen
// rotation since there's no camera matrix there.
fn world_to_pixel_3d(p: vec3<f32>) -> vec2<i32> {
    let p2d = project_3d_to_2d_apophysis(
        p,
        params.camera_rotation_x,  // pitch
        params.camera_rotation_y,  // yaw
        params.camera_bank,        // bank (XML `cam_roll` — JWildfire rename quirk)
        // Roll: negated so positive `rotation` matches the 2D
        // `world_to_pixel` direction (standard math-convention CCW).
        // JWildfire's matrix roll rotates CW for positive input; the
        // negation flips it to CCW so a flame loaded in 2D and
        // switched to 3D keeps the same visual rotation direction.
        // XML round-trip stays exact — `rotate` is still written
        // verbatim from `config.rotation`.
        -params.rotation,
        vec3<f32>(params.camera_x, params.camera_y, params.camera_z),
        params.perspective_strength
    );

    // Pan + zoom only. The screen-space rotation block that used to
    // live here has been removed in 3D mode — `params.rotation`
    // already participated as the matrix's `roll` argument above.
    var transformed = p2d - vec2<f32>(params.pan_x, params.pan_y);
    transformed = transformed * params.zoom;

    // Map from fractal space (typically -2 to 2) to pixel space
    let scale = f32(min(params.width, params.height)) * 0.25;
    let center = vec2<f32>(f32(params.width), f32(params.height)) * 0.5;
    let pixel = center + transformed * scale;
    return vec2<i32>(i32(pixel.x), i32(pixel.y));
}

// Convert 2D fractal coords to pixel coords
fn world_to_pixel(p: vec2<f32>) -> vec2<i32> {
    // Apply view transform: pan, rotation, and zoom
    var transformed = p - vec2<f32>(params.pan_x, params.pan_y);

    // Apply rotation
    let cos_r = cos(params.rotation);
    let sin_r = sin(params.rotation);
    transformed = vec2<f32>(
        transformed.x * cos_r - transformed.y * sin_r,
        transformed.x * sin_r + transformed.y * cos_r
    );

    // Apply zoom
    transformed = transformed * params.zoom;

    // Map from fractal space (typically -2 to 2) to pixel space
    let scale = f32(min(params.width, params.height)) * 0.25;
    let center = vec2<f32>(f32(params.width), f32(params.height)) * 0.5;
    let pixel = center + transformed * scale;
    return vec2<i32>(i32(pixel.x), i32(pixel.y));
}

// Apply post-symmetry to a 3D world-space sample. `k` indexes the
// symmetry copy: 0 = original (returned as-is), 1..K-1 = mirrored or
// rotated copies depending on `params.post_symmetry.kind`.
//
//   1 (XAxis): reflect across y = center_y → shift the copy by
//              (distance, 0) → rotate it around the center by
//              `rotation` radians. K = 2 (one mirror).
//   2 (YAxis): reflect across x = center_x → shift by (0, distance)
//              → rotate around center. K = 2.
//   3 (Point): rotate around (center_x, center_y) by k × 2π / order.
//              K = order.
//
// Z passes through unchanged — post-symmetry is a 2D operation
// applied to XY before the camera transform.
//
// Live behind `HAS_POST_SYMMETRY`: when the flame has no symmetry
// active, this function isn't referenced and the shader builder strips
// it from the compiled module.
fn post_symmetry_copy(p: vec3<f32>, k: u32) -> vec3<f32> {
    let cx = params.post_symmetry.center_x;
    let cy = params.post_symmetry.center_y;
    let kind = params.post_symmetry.kind;

    if (kind == 0u) {
        return p;
    }

    if (kind == 3u) {
        // Point mode: k=0 returns p unchanged (no rotation); k≥1
        // rotates around the center by k × 2π / order. Distance and
        // rotation aren't part of Point mode — only `order` and
        // `center` matter (matches JWildfire).
        if (k == 0u) {
            return p;
        }
        let order = max(params.post_symmetry.order, 1u);
        let theta = f32(k) * 6.28318530717958647692 / f32(order);
        let cs = cos(theta);
        let sn = sin(theta);
        let dx = p.x - cx;
        let dy = p.y - cy;
        return vec3<f32>(dx * cs - dy * sn + cx, dx * sn + dy * cs + cy, p.z);
    }

    // Axis modes (1 = XAxis math, 2 = YAxis math).
    //
    // We use the standard math-class convention: the named axis is
    // the *line of reflection*. XAxis reflects across the X axis
    // (horizontal line y = center_y), flipping Y; YAxis reflects
    // across the Y axis (vertical line x = center_x), flipping X.
    // JWildfire's `.flame` XML uses the opposite naming (their
    // `X_AXIS` is our YAxis); the swap lives in
    // `PostSymmetryType::xml_token`.
    //
    // Distance and rotation produce OPPOSITE effects on the original
    // and the mirror — the original moves +distance / rotates +θ,
    // the mirror moves −distance / rotates −θ. The mirror line stays
    // in place (still y = cy for XAxis, x = cx for YAxis), so the
    // symmetry across the named axis is preserved no matter how
    // distance and rotation are adjusted. Matches JWildfire's
    // "the symmetry is preserved along the axis" behavior.
    //
    // The trick: apply distance pan and rotation to `p` first, then
    // for k=1 reflect at the end. Algebraically, reflecting after
    // applying (+d, +θ) is equivalent to applying (−d, −θ) and then
    // reflecting `p`, so the mirror sees the opposite-direction
    // perturbation for free.
    var x = p.x;
    var y = p.y;

    // Pan along the flip direction (perpendicular to the mirror line).
    // JWildfire stores `distance` as the *total separation* between
    // the original and the mirror; the per-copy pan is `distance / 2`.
    // See JWildfire's `PostAxisSymmetryWFFunc.init` —
    // `_halve_dist = pAmount / 2.0` — used as `+halve_dist` for the
    // original and `-halve_dist` for the mirror, giving total
    // separation `distance`. Pre-fix we panned by the full `distance`,
    // doubling the visible spread vs JWildfire at the same numeric
    // input. We split by 2 here so on-disk values round-trip 1:1
    // with JWildfire (no import/export multiplier needed).
    let half_dist = params.post_symmetry.distance * 0.5;
    if (kind == 1u) {
        // XAxis flips Y → distance pans along Y.
        y = y + half_dist;
    } else {
        // YAxis flips X → distance pans along X.
        x = x + half_dist;
    }

    // Rotation around the center. JWildfire's PostAxisSymmetryWFFunc
    // uses a clockwise rotation matrix (`[cos sin; -sin cos]`); we
    // use the standard math-convention counterclockwise
    // (`[cos -sin; sin cos]`). Negate the angle here so the on-disk
    // `rotation_deg` value round-trips with JWildfire at the same
    // sign — pre-fix, JWildfire's `-15°` looked like our `+15°`.
    // No `/2` factor on rotation (unlike distance) — JWildfire's init
    // does `a = rotation_deg × π/180`, just degrees-to-radians.
    let a = -params.post_symmetry.rotation;
    let cs = cos(a);
    let sn = sin(a);
    let dx = x - cx;
    let dy = y - cy;
    x = dx * cs - dy * sn + cx;
    y = dx * sn + dy * cs + cy;

    // Reflect for k=1. This inverts the prior pan and rotation for
    // the mirror copy in one step (see the algebra note above).
    if (k == 1u) {
        if (kind == 1u) {
            y = 2.0 * cy - y;
        } else {
            x = 2.0 * cx - x;
        }
    }

    return vec3<f32>(x, y, p.z);
}

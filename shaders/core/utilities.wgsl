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

// Build Apophysis camera matrix (ZXY Euler rotation: yaw around Z, then pitch around X)
// Source: Apophysis ControlPoint.pas:467-475
fn build_camera_matrix(pitch: f32, yaw: f32) -> mat3x3<f32> {
    let cy = cos(-yaw);  // Note: -yaw inversion as in Apophysis
    let sy = sin(-yaw);
    let cp = cos(pitch);
    let sp = sin(pitch);

    // Camera matrix from Apophysis formula
    // Returns column-major matrix for WGSL
    return mat3x3<f32>(
        vec3<f32>(cy,           cp * sy,      sp * sy),      // Column 0
        vec3<f32>(-sy,          cp * cy,      sp * cy),      // Column 1
        vec3<f32>(0.0,          -sp,          cp)            // Column 2
    );
}

// Apply Apophysis camera transformation: translate + rotate
fn camera_transform(p: vec3<f32>, camera_matrix: mat3x3<f32>, camera_z: f32) -> vec3<f32> {
    // Step 1: Translate Z coordinate (move origin to camera height)
    let z_translated = p.z - camera_z;

    // Step 2: Rotate using camera matrix (Apophysis formula)
    // Matrix multiplication: camera_matrix * vec3(p.x, p.y, z_translated)
    let x = camera_matrix[0][0] * p.x + camera_matrix[1][0] * p.y;
    let y = camera_matrix[0][1] * p.x + camera_matrix[1][1] * p.y + camera_matrix[2][1] * z_translated;
    let z = camera_matrix[0][2] * p.x + camera_matrix[1][2] * p.y + camera_matrix[2][2] * z_translated;

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

// Complete Apophysis 3D → 2D projection pipeline
fn project_3d_to_2d_apophysis(
    p: vec3<f32>,
    pitch: f32,
    yaw: f32,
    camera_z: f32,
    persp_strength: f32
) -> vec2<f32> {
    // Build camera matrix
    let camera_matrix = build_camera_matrix(pitch, yaw);

    // Transform to camera space (translate + rotate)
    let camera_space = camera_transform(p, camera_matrix, camera_z);

    // Apply perspective projection
    return apply_perspective(camera_space, persp_strength);
}

// Convert 3D fractal coords to pixel coords (with Apophysis camera system)
fn world_to_pixel_3d(p: vec3<f32>) -> vec2<i32> {
    // Apply Apophysis 3D → 2D projection (camera transform + perspective)
    let p2d = project_3d_to_2d_apophysis(
        p,
        params.camera_rotation_x,  // pitch
        params.camera_rotation_y,  // yaw
        params.camera_z,
        params.perspective_strength
    );

    // Apply view transform: pan, rotation, and zoom
    var transformed = p2d - vec2<f32>(params.pan_x, params.pan_y);

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

    // Rotation around the center.
    let cs = cos(params.post_symmetry.rotation);
    let sn = sin(params.post_symmetry.rotation);
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

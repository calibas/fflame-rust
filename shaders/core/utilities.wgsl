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
// Originally a verbatim transcription of `createProjectionMatrix`
// from `output/FlameRendererView.java`. That turned out to be subtly
// wrong: JWF *applies* its matrix transposed (`applyCameraMatrix`
// multiplies point components against matrix COLUMNS, see
// FlameRendererView.java lines 262-266), while our `camera_transform`
// applies rows. Transposing a product reverses the factor order, so
// a verbatim copy applied row-wise reverses the rotation composition.
// The empirical yaw↔roll slot swap at the call site repaired the two
// OUTER factors, but left bank and pitch in reversed relative order —
// bank acted outside pitch (world-axis-ish behavior) instead of
// inside (camera-axis-ish). Symptom: at pitch 90° the Bank slider
// yawed around world-Z in our app while JWF rolls around the look
// axis. At pitch 0 the two orders coincide, which is why this
// survived the original axis-by-axis slider tuning.
//
// This body now constructs the factorization directly:
//
//   M(yaw, pitch, bank, roll) = Rz(−yaw)·Rx(−pitch)·Ry(−bank)·Rz(−roll)
//
// which, with the call-site slot mapping (yaw slot ← −rotation,
// pitch ← −pitch, bank ← −bank, roll slot ← yaw), yields the
// effective world→camera chain
//
//   M_eff = Rz(rotation)·Rx(pitch)·Ry(bank)·Rz(−yaw)
//
// — exactly JWF's effective (transposed) chain
// Rz(camRoll)·Rx(camPitch)·Ry(camBank)·Rz(−camYaw). With bank = 0
// this is algebraically identical to the old transcription, so all
// existing flames without bank render unchanged. Verified numerically
// against the JWF source formula + transposed application.
//
// All four angles in radians. Each WGSL column holds
// `(m[0][col], m[1][col], m[2][col])` of the row-major matrix, same
// storage convention as before.
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
        // Column 0 — m[0][0], m[1][0], m[2][0]
        vec3<f32>(
             cy*cb*cr - sy*cp*sr + sy*sp*sb*cr,
            -sy*cb*cr - cy*cp*sr + cy*sp*sb*cr,
             sp*sr + cp*sb*cr
        ),
        // Column 1 — m[0][1], m[1][1], m[2][1]
        vec3<f32>(
             cy*cb*sr + sy*cp*cr + sy*sp*sb*sr,
            -sy*cb*sr + cy*cp*cr + cy*sp*sb*sr,
            -sp*cr + cp*sb*sr
        ),
        // Column 2 — m[0][2], m[1][2], m[2][2]
        vec3<f32>(
            -cy*sb + sy*sp*cb,
             sy*sb + cy*sp*cb,
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

// Apply Apophysis perspective projection.
// Source: Apophysis ControlPoint.pas:606
//
// The classic Apo formula `zr = 1 - persp · z` is a stylistic effect,
// not a true perspective matrix. For points the camera can see
// (camera-space z < 0 in our convention) `zr > 1` and behavior is
// sensible. For points behind the focal plane, however, `zr` shrinks
// to zero (singularity at z = 1/persp) and then flips sign, which
// mirrors the point onto the opposite side of the screen — the
// "wraparound where the sky lands above the floor" artifact that
// becomes obvious at persp ≥ ~0.5.
//
// We clip points where zr drops below a small positive threshold:
// returning an out-of-frame sentinel makes the downstream bounds
// check drop the sample naturally. This matches JWildfire behavior
// (Apo had the same wraparound bug; we deliberately don't reproduce
// it here). Existing Apo flames at typical persp ≈ 0.1 are unaffected
// because their content never gets close to the z = 10 singularity.
//
// TODO: expose this as a "Behind-camera wraparound (Apo-compatible)"
// toggle in the View panel if anyone needs the original Apo behavior
// back — e.g., a flame author who deliberately leans on the
// wraparound as a stylistic effect. Default would stay clipped.
fn apply_perspective(p: vec3<f32>, persp_strength: f32) -> vec2<f32> {
    if (abs(persp_strength) < 1e-6) {
        // Orthographic: no perspective
        return p.xy;
    }

    // Apophysis formula: zr = 1 - cameraPersp * z
    let zr = 1.0 - persp_strength * p.z;

    // Clip behind-camera and near-focal-plane points. The threshold
    // is small but positive so we drop both the singularity itself
    // and the extreme-magnification halo just in front of it.
    if (zr < 1e-3) {
        return vec2<f32>(1e30, 1e30);
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
    // Convention mapping — routes our slider inputs to the right
    // matrix slots. Two adjustments vs. the naive (yaw, pitch, bank,
    // roll) pass-through:
    //
    //   1. **Yaw ↔ roll slot swap.** Our `yaw` parameter goes into
    //      the matrix's `roll` slot, and our `roll` parameter goes
    //      into the matrix's `yaw` slot. Root cause (confirmed from
    //      JWF source, `FlameRendererView.applyCameraMatrix`): JWF
    //      applies its matrix TRANSPOSED, which reverses the factor
    //      order; the swap re-lands the two outer factors. The
    //      middle factors (bank vs. pitch) couldn't be fixed by any
    //      slot routing — that order is corrected inside
    //      `build_camera_matrix` itself (see its comment).
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
// `params.rotation` (XML `rotate`) is deliberately NOT passed into
// the camera matrix here. The roll factor sits outermost in the
// camera chain (`Rz(R)·Rx(P)·Ry(B)·Rz(−Y)`) and never touches
// camera-space z, so it commutes exactly with the perspective
// divide: rolling inside the matrix ≡ rotating the projected 2D
// point. We exploit that to apply rotation AFTER the pan
// subtraction, with the identical composition (and identical code)
// as the 2D `world_to_pixel`:
//
//     pan → rotate → zoom
//
// This makes Pan X/Y mean the same thing in both render modes —
// a pre-rotation position in the fractal plane, the Apophysis
// convention — so toggling 2D/3D never shifts the view when pan and
// rotation are both set. (JWildfire's 3D path instead pans in
// screen-aligned post-projection coordinates, which is why JWF
// flames jump when perspective crosses zero. We diverge from JWF
// deliberately here; pitch/yaw/bank behavior is unaffected because
// the outermost roll composes after them either way.)
fn world_to_pixel_3d(p: vec3<f32>) -> vec2<i32> {
    return project_3d_full(p).pixel;
}

// Full 3D projection result: the pixel plus the camera-space position it
// was projected from. The camera_space here is ROLL-LESS (params.rotation
// is applied post-projection, see world_to_pixel_3d's doc above) — but the
// outermost roll is a screen-plane rotation that never touches camera-space
// z, so this single camera_space serves every per-sample depth consumer
// (depth-density compensation, DoF, far-density fade, fog, and solid
// rendering's depth buffer). Those blocks previously each rebuilt an
// equivalent matrix per splat; z is roll-invariant so the values are
// identical.
struct Projection3D {
    pixel: vec2<i32>,
    camera_space: vec3<f32>,
}

fn project_3d_full(p: vec3<f32>) -> Projection3D {
    // Same matrix world_to_pixel_3d always built via
    // project_3d_to_2d_apophysis (roll = 0.0 in the matrix; see the slot
    // mapping comments there).
    let camera_matrix = build_camera_matrix(
        0.0,                        // roll applied post-projection below
        -params.camera_rotation_x,  // pitch
        -params.camera_bank,        // bank
         params.camera_rotation_y,  // matrix roll slot ← our yaw
    );
    let camera_space = camera_transform(
        p,
        camera_matrix,
        vec3<f32>(params.camera_x, params.camera_y, params.camera_z)
    );
    let p2d = apply_perspective(camera_space, params.perspective_strength);

    // Pan, rotate, zoom — mirrors `world_to_pixel` (2D) exactly.
    var transformed = p2d - vec2<f32>(params.pan_x, params.pan_y);

    let cos_r = cos(params.rotation);
    let sin_r = sin(params.rotation);
    transformed = vec2<f32>(
        transformed.x * cos_r - transformed.y * sin_r,
        transformed.x * sin_r + transformed.y * cos_r
    );

    transformed = transformed * params.zoom;

    // Map from fractal space (typically -2 to 2) to pixel space
    let scale = f32(min(params.width, params.height)) * 0.25;
    let center = vec2<f32>(f32(params.width), f32(params.height)) * 0.5;
    let pixel = center + transformed * scale;
    return Projection3D(vec2<i32>(i32(pixel.x), i32(pixel.y)), camera_space);
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

// atan2 with IEEE behaviour at the origin, for variation code that can
// reach atan2(0, 0).
//
// Metal compiles shaders with fast-math (wgpu never clears
// `fastMathEnabled`, which defaults to true), and its fast atan2
// returns pi/4 at the origin. Probed on an M2: that is the ONLY input
// where it diverges from IEEE — everything else agrees to 1 ulp. pi/4
// is a plausible finite value, so no bad-value guard downstream can
// tell it from a real angle; it silently relocates the point. It cost
// npolar 73% of apo-misc7's lit pixels, because npolar at its default
// parity reaches (0, 0) on every call.
//
// The branch reproduces IEEE exactly, which is NOT "return 0": the
// result depends on the signs of the two zeros.
//
//   atan2(+0, +0) = +0     atan2(+0, -0) = +pi
//   atan2(-0, +0) = -0     atan2(-0, -0) = -pi
//
// Collapsing all four to 0 changes the render on Vulkan, where atan2
// is already IEEE — measured, it moved the image. Getting this right
// is what makes the guard a bit-exact no-op on platforms that never
// needed it. The sign bit is read via bitcast because `x < 0.0` is
// false for -0.0, and because integer ops are immune to fast-math.
//
// Not substituted globally: one comparison per call costs ~5% across
// the full variation set, and probing all 646 variations found only
// npolar, ho and log_db reaching the origin at default parameters.
// Call this where (0, 0) is reachable; plain atan2 is fine elsewhere.
fn ff_atan2(y: f32, x: f32) -> f32 {
    if (y == 0.0 && x == 0.0) {
        let pi = 3.14159265358979;
        let mag = select(0.0, pi, (bitcast<u32>(x) & 0x80000000u) != 0u);
        return select(mag, -mag, (bitcast<u32>(y) & 0x80000000u) != 0u);
    }
    return atan2(y, x);
}

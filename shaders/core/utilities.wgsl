// Utility functions shared by 2D and 3D shaders

// Get a variation parameter value for a specific transform
// variation_id: Index of the variation (0-99)
// param_slot: Parameter slot within the variation (0-11)
fn get_param(xform_id: u32, variation_id: u32, param_slot: u32) -> f32 {
    let idx = variation_id * 12u + param_slot;
    return variation_params[xform_id].params[idx];
}

// Select transform based on cumulative weights
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

// Convert speed to color using palette lookup
fn speed_to_color(speed: f32) -> vec3<f32> {
    // Normalize speed to [0, 1] range
    // Use logarithmic scale for better visualization
    let normalized_speed = clamp(log(speed * 10.0 + 1.0) / 3.0, 0.0, 1.0);

    // Sample from palette texture (2D texture with height=1, so y=0.5)
    return textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(normalized_speed, 0.5), 0.0).rgb;
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

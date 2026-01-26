// Utility functions for tiled rendering
// Uses full_width/full_height for coordinate calculation

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

// Select transform with xaos-weighted transition probabilities
// xaos_weights[src * num_transforms + dst] modifies P(src→dst)
// When xaos buffer contains all 1.0s, this behaves identically to select_transform
fn select_transform_xaos(rand_val: f32, prev_xform: u32) -> u32 {
    var cumulative = 0.0;
    var total_weight = 0.0;

    // Calculate modified total weight based on xaos from previous transform
    let xaos_base = prev_xform * params.num_transforms;
    for (var i = 0u; i < params.num_transforms; i++) {
        let base_weight = transforms[i].weight;
        let xaos_modifier = xaos_weights[xaos_base + i];
        total_weight += base_weight * xaos_modifier;
    }

    // Guard against zero total (shouldn't happen with valid data)
    if (total_weight <= 0.0) {
        return 0u;
    }

    let threshold = rand_val * total_weight;

    // Select transform based on modified probabilities
    for (var i = 0u; i < params.num_transforms; i++) {
        let base_weight = transforms[i].weight;
        let xaos_modifier = xaos_weights[xaos_base + i];
        cumulative += base_weight * xaos_modifier;
        if (threshold <= cumulative) {
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
fn build_camera_matrix(pitch: f32, yaw: f32) -> mat3x3<f32> {
    let cy = cos(-yaw);
    let sy = sin(-yaw);
    let cp = cos(pitch);
    let sp = sin(pitch);

    return mat3x3<f32>(
        vec3<f32>(cy,           cp * sy,      sp * sy),
        vec3<f32>(-sy,          cp * cy,      sp * cy),
        vec3<f32>(0.0,          -sp,          cp)
    );
}

// Apply Apophysis camera transformation
fn camera_transform(p: vec3<f32>, camera_matrix: mat3x3<f32>, camera_z: f32) -> vec3<f32> {
    let z_translated = p.z - camera_z;
    let x = camera_matrix[0][0] * p.x + camera_matrix[1][0] * p.y;
    let y = camera_matrix[0][1] * p.x + camera_matrix[1][1] * p.y + camera_matrix[2][1] * z_translated;
    let z = camera_matrix[0][2] * p.x + camera_matrix[1][2] * p.y + camera_matrix[2][2] * z_translated;
    return vec3<f32>(x, y, z);
}

// Apply Apophysis perspective projection
fn apply_perspective(p: vec3<f32>, persp_strength: f32) -> vec2<f32> {
    if (abs(persp_strength) < 1e-6) {
        return p.xy;
    }
    let zr = 1.0 - persp_strength * p.z;
    if (abs(zr) < 1e-6) {
        return p.xy;
    }
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
    let camera_matrix = build_camera_matrix(pitch, yaw);
    let camera_space = camera_transform(p, camera_matrix, camera_z);
    return apply_perspective(camera_space, persp_strength);
}

// Convert 3D fractal coords to FULL-RESOLUTION pixel coords (for tiled rendering)
fn world_to_pixel_3d(p: vec3<f32>) -> vec2<i32> {
    let p2d = project_3d_to_2d_apophysis(
        p,
        params.camera_rotation_x,
        params.camera_rotation_y,
        params.camera_z,
        params.perspective_strength
    );

    var transformed = p2d - vec2<f32>(params.pan_x, params.pan_y);

    let cos_r = cos(params.rotation);
    let sin_r = sin(params.rotation);
    transformed = vec2<f32>(
        transformed.x * cos_r - transformed.y * sin_r,
        transformed.x * sin_r + transformed.y * cos_r
    );

    transformed = transformed * params.zoom;

    // Use FULL resolution for pixel calculation (not tile size)
    let scale = f32(min(tile_params.full_width, tile_params.full_height)) * 0.25;
    let center = vec2<f32>(f32(tile_params.full_width), f32(tile_params.full_height)) * 0.5;
    let pixel = center + transformed * scale;
    return vec2<i32>(i32(pixel.x), i32(pixel.y));
}

// Convert 2D fractal coords to FULL-RESOLUTION pixel coords (for tiled rendering)
fn world_to_pixel(p: vec2<f32>) -> vec2<i32> {
    var transformed = p - vec2<f32>(params.pan_x, params.pan_y);

    let cos_r = cos(params.rotation);
    let sin_r = sin(params.rotation);
    transformed = vec2<f32>(
        transformed.x * cos_r - transformed.y * sin_r,
        transformed.x * sin_r + transformed.y * cos_r
    );

    transformed = transformed * params.zoom;

    // Use FULL resolution for pixel calculation (not tile size)
    let scale = f32(min(tile_params.full_width, tile_params.full_height)) * 0.25;
    let center = vec2<f32>(f32(tile_params.full_width), f32(tile_params.full_height)) * 0.5;
    let pixel = center + transformed * scale;
    return vec2<i32>(i32(pixel.x), i32(pixel.y));
}

// Calculate tile index and local pixel offset for a full-resolution pixel
// Returns (tile_index, local_pixel_index) or (-1, 0) if out of bounds
fn pixel_to_tile(px: u32, py: u32) -> vec2<i32> {
    // Which tile does this pixel belong to?
    let tile_x = px / tile_params.tile_size;
    let tile_y = py / tile_params.tile_size;

    // Check if tile is within bounds
    if (tile_x >= tile_params.tiles_x || tile_y >= tile_params.tiles_y) {
        return vec2<i32>(-1, 0);
    }

    let tile_idx = tile_y * tile_params.tiles_x + tile_x;

    // Check if this tile is in the current chunk (for chunked processing)
    if (tile_idx < tile_params.tile_offset || tile_idx >= tile_params.tile_offset + tile_params.num_tiles) {
        return vec2<i32>(-1, 0);
    }

    // Local coordinates within tile
    let local_x = px % tile_params.tile_size;
    let local_y = py % tile_params.tile_size;
    let local_idx = local_y * tile_params.tile_size + local_x;

    // Adjust tile index for chunk offset
    let chunk_tile_idx = tile_idx - tile_params.tile_offset;

    return vec2<i32>(i32(chunk_tile_idx), i32(local_idx));
}

// Utility functions shared by 2D and 3D shaders

// Get a variation parameter value for a specific transform
// variation_id: Index of the variation (0-23)
// param_slot: Parameter slot within the variation (0-7)
fn get_param(xform_id: u32, variation_id: u32, param_slot: u32) -> f32 {
    let idx = variation_id * 8u + param_slot;
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

    // Sample from palette texture
    return textureSampleLevel(palette_texture, palette_sampler, normalized_speed, 0.0).rgb;
}

// Projection functions for 3D → 2D (only used in 3D mode)
fn project_orthographic(p: vec3<f32>) -> vec2<f32> {
    // Simple: just drop Z coordinate
    return p.xy;
}

fn project_perspective(p: vec3<f32>, strength: f32) -> vec2<f32> {
    // Perspective projection: scale by (strength / (strength + z))
    let scale = strength / (strength + p.z);
    return p.xy * scale;
}

fn project_to_2d(p: vec3<f32>) -> vec2<f32> {
    if (params.projection_type == 0u) {
        return project_orthographic(p);
    } else {
        return project_perspective(p, params.perspective_strength);
    }
}

// Convert 3D fractal coords to pixel coords (with camera rotation)
fn world_to_pixel_3d(p: vec3<f32>) -> vec2<i32> {
    // First, apply 3D camera rotation (rotate the 3D point in space)
    var rotated = p;

    // Rotate around Y axis (yaw - left/right orbit)
    if (params.camera_rotation_y != 0.0) {
        let cy = cos(params.camera_rotation_y);
        let sy = sin(params.camera_rotation_y);
        rotated = vec3<f32>(
            rotated.x * cy + rotated.z * sy,
            rotated.y,
            -rotated.x * sy + rotated.z * cy
        );
    }

    // Rotate around X axis (pitch - up/down orbit)
    if (params.camera_rotation_x != 0.0) {
        let cx = cos(params.camera_rotation_x);
        let sx = sin(params.camera_rotation_x);
        rotated = vec3<f32>(
            rotated.x,
            rotated.y * cx - rotated.z * sx,
            rotated.y * sx + rotated.z * cx
        );
    }

    // Then, project 3D → 2D
    let p2d = project_to_2d(rotated);

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

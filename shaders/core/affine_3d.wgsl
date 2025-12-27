// Affine transformations for 3D mode

// Apply affine transformation (3D)
// Standard affine formula: x' = ax + by + e, y' = cx + dy + f, z' = z + g
fn apply_affine(xform: Transform, p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        xform.a * p.x + xform.b * p.y + xform.e,
        xform.c * p.x + xform.d * p.y + xform.f,
        p.z + xform.g  // Z is just offset
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

// Affine transformations for 2D mode

// Apply pre-affine transformation (2D)
// Standard affine formula: x' = ax + by + e, y' = cx + dy + f
fn apply_affine(xform: Transform, p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        xform.a * p.x + xform.b * p.y + xform.e,
        xform.c * p.x + xform.d * p.y + xform.f
    );
}

// Apply post-affine transformation (2D)
// Same simultaneous affine formula, applied after variations
fn apply_post_affine(xform: Transform, p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        xform.post_a * p.x + xform.post_b * p.y + xform.post_e,
        xform.post_c * p.x + xform.post_d * p.y + xform.post_f
    );
}

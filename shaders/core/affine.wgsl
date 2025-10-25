// Affine transformations for 2D mode

// Apply affine transformation (2D)
// Y is negated to match Apophysis coordinate system
fn apply_affine(xform: Transform, p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        xform.a * p.x + xform.b * p.y + xform.e,
        -(xform.c * p.x + xform.d * p.y + xform.f)
    );
}

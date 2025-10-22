// Example plugin variation: blob
// This would be variation index 25 (second plugin variation)
//
// Creates blob-like distortions

fn variation_plugin_25(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    // Blob - creates organic blob shapes
    let r = length(p);
    let theta = atan2(p.y, p.x);
    let phi = atan2(p.z, length(p.xy));

    // Modulate radius with sine waves for blob effect
    let waves = 3.0;
    let amplitude = 0.5;
    let new_r = r * (1.0 + amplitude * sin(waves * theta) * cos(waves * phi));

    // Convert back to Cartesian
    let xy_r = new_r * cos(phi);
    let new_x = xy_r * cos(theta);
    let new_y = xy_r * sin(theta);
    let new_z = new_r * sin(phi);

    return vec3<f32>(new_x, new_y, new_z);
}

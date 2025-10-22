// Example plugin variation: curl_3d
// This would be variation index 24 (first plugin variation)
//
// To use this variation, you would:
// 1. Register it in a variation plugin registry
// 2. Load it when variation index 24 is active in any transform
// 3. It will be automatically injected into the shader

fn variation_plugin_24(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    // Curl 3D - creates swirling patterns in 3D space
    let c1 = 1.0;
    let c2 = 2.0;

    let re = p.x * cos(c2 * p.y) - p.y * sin(c2 * p.y);
    let im = p.x * sin(c2 * p.y) + p.y * cos(c2 * p.y);

    let r = sqrt(re * re + im * im);
    let theta = atan2(im, re);

    let new_x = r * cos(theta + c1);
    let new_y = r * sin(theta + c1);
    let new_z = p.z + sin(r) * 0.5;  // Add some Z variation based on XY distance

    return vec3<f32>(new_x, new_y, new_z);
}

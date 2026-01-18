// Plasma Effect
//
// Classic demoscene plasma effect using summed sinusoids.
// Blends procedural plasma colors with the input image.
// Parameters:
//   params[0] = intensity (0-1): Blend amount with original image
//   params[1] = scale (0.5-10): Frequency of plasma pattern
//   params[2] = speed (0-10): Animation speed
//   params[3] = time: Current time for animation
//   params[4] = blend_mode: 0=Add, 1=Multiply, 2=Overlay, 3=Screen, 4=Color

struct EffectParams {
    params: array<vec4<f32>, 4>,
    width: u32,
    height: u32,
    _padding: vec2<f32>,
}

fn get_param(index: u32) -> f32 {
    return effect_params.params[index / 4u][index % 4u];
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> effect_params: EffectParams;

const PI: f32 = 3.14159265359;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    let x = f32((vertex_index & 1u) << 2u);
    let y = f32((vertex_index & 2u) << 1u);
    output.position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);
    output.uv = vec2<f32>(x * 0.5, y * 0.5);
    return output;
}

fn plasma(uv: vec2<f32>, scale: f32, time: f32) -> f32 {
    let p = uv * scale;

    var v = sin(p.x + time);
    v += sin((p.y + time) * 0.5);
    v += sin((p.x + p.y + time) * 0.5);

    // Circular pattern
    let cx = p.x + 0.5 * sin(time * 0.2);
    let cy = p.y + 0.5 * cos(time * 0.3);
    v += sin(sqrt(cx * cx + cy * cy + 1.0) + time);

    return v * 0.25;  // Normalize to roughly -1 to 1
}

fn plasma_color(v: f32) -> vec3<f32> {
    // Generate psychedelic colors from plasma value
    return vec3<f32>(
        sin(v * PI) * 0.5 + 0.5,
        sin(v * PI + 2.094) * 0.5 + 0.5,  // 2*PI/3
        sin(v * PI + 4.189) * 0.5 + 0.5   // 4*PI/3
    );
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Overlay blend: combines multiply and screen
fn blend_overlay(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return select(
        2.0 * base * blend,
        1.0 - 2.0 * (1.0 - base) * (1.0 - blend),
        base > vec3<f32>(0.5)
    );
}

// Screen blend: inverse multiply
fn blend_screen(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return 1.0 - (1.0 - base) * (1.0 - blend);
}

// Color blend: use blend hue/saturation on base luminance
fn rgb_to_hsl(c: vec3<f32>) -> vec3<f32> {
    let cmax = max(c.r, max(c.g, c.b));
    let cmin = min(c.r, min(c.g, c.b));
    let delta = cmax - cmin;
    let l = (cmax + cmin) * 0.5;

    if (delta < 0.0001) {
        return vec3<f32>(0.0, 0.0, l);
    }

    let s = delta / (1.0 - abs(2.0 * l - 1.0));
    var h: f32;
    if (cmax == c.r) {
        h = ((c.g - c.b) / delta) % 6.0;
    } else if (cmax == c.g) {
        h = (c.b - c.r) / delta + 2.0;
    } else {
        h = (c.r - c.g) / delta + 4.0;
    }
    h /= 6.0;
    if (h < 0.0) { h += 1.0; }

    return vec3<f32>(h, s, l);
}

fn hsl_to_rgb(hsl: vec3<f32>) -> vec3<f32> {
    let h = hsl.x;
    let s = hsl.y;
    let l = hsl.z;

    let c = (1.0 - abs(2.0 * l - 1.0)) * s;
    let x = c * (1.0 - abs((h * 6.0) % 2.0 - 1.0));
    let m = l - c * 0.5;

    var rgb: vec3<f32>;
    let h6 = h * 6.0;
    if (h6 < 1.0) { rgb = vec3<f32>(c, x, 0.0); }
    else if (h6 < 2.0) { rgb = vec3<f32>(x, c, 0.0); }
    else if (h6 < 3.0) { rgb = vec3<f32>(0.0, c, x); }
    else if (h6 < 4.0) { rgb = vec3<f32>(0.0, x, c); }
    else if (h6 < 5.0) { rgb = vec3<f32>(x, 0.0, c); }
    else { rgb = vec3<f32>(c, 0.0, x); }

    return rgb + m;
}

fn blend_color(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    let base_hsl = rgb_to_hsl(base);
    let blend_hsl = rgb_to_hsl(blend);
    // Use blend's hue and saturation, base's luminance
    return hsl_to_rgb(vec3<f32>(blend_hsl.x, blend_hsl.y, base_hsl.z));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let original = textureSample(input_texture, input_sampler, input.uv);

    let intensity = get_param(0u);
    let scale = max(0.5, get_param(1u));
    let speed = get_param(2u);
    let time = get_param(3u) * speed;
    let blend_mode = i32(get_param(4u));

    // Generate plasma
    let v = plasma(input.uv, scale, time);
    let plasma_rgb = plasma_color(v);

    // Apply blend mode
    var blended: vec3<f32>;
    if (blend_mode == 1) {
        // Multiply: darkens image with plasma pattern
        blended = mix(original.rgb, original.rgb * plasma_rgb, intensity);
    } else if (blend_mode == 2) {
        // Overlay: increases contrast with plasma
        blended = mix(original.rgb, blend_overlay(original.rgb, plasma_rgb), intensity);
    } else if (blend_mode == 3) {
        // Screen: lightens image with plasma
        blended = mix(original.rgb, blend_screen(original.rgb, plasma_rgb), intensity);
    } else if (blend_mode == 4) {
        // Color: applies plasma hue to original luminance
        blended = mix(original.rgb, blend_color(original.rgb, plasma_rgb), intensity);
    } else {
        // Default (0): Additive glow
        blended = original.rgb + plasma_rgb * intensity;
    }

    return vec4<f32>(blended, original.a);
}

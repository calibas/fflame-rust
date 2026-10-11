# GPU Shader Techniques for Fractal Flame Rendering

Real-time image reconstruction from sparse point data and psychedelic visual effects require fundamentally different approaches: **reconstruction techniques** focus on coherent image synthesis from incomplete information, while **creative effects** transform complete images into visually striking outputs. For fractal flame renderers outputting individual points, a pipeline combining **Gaussian splatting** during accumulation, **temporal reprojection** with one previous frame, and **variance-guided spatial filtering** (SVGF/À-Trous wavelets) provides the highest quality real-time reconstruction. The psychedelic effects covered—from IQ's cosine palettes to reaction-diffusion patterns—are all achievable in single-pass shaders except feedback effects which require ping-pong buffers.

This reference covers **18 distinct techniques** across both categories, all implemented in WGSL format with source attribution. Each technique includes pass requirements, performance considerations, and tunable parameters for integration into a GPU fractal flame renderer.

---

## Part 1: Image Enhancement and Point Reconstruction

### Bilateral filtering preserves edges while smoothing noise

Bilateral filtering (Tomasi & Manduchi, 1998) uniquely combines **spatial** and **intensity** weighting, ensuring pixels are only averaged when they're both close in screen space AND similar in color value. This preserves the sharp edges characteristic of fractal flame structures while smoothing noisy regions.

**Mathematical foundation**: `BF[I](p) = Σ Gs(||p-q||) × Gr(|I(p)-I(q)|) × I(q) / Wp` where `Gs` is the spatial Gaussian and `Gr` is the range/intensity Gaussian.

```wgsl
struct BilateralUniforms {
    resolution: vec2f,
    sigma_spatial: f32,  // 3-10 pixels typical
    sigma_range: f32,    // 0.1-0.5 normalized intensity
}

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;
@group(0) @binding(2) var<uniform> u: BilateralUniforms;

const KERNEL_RADIUS: i32 = 5;

fn gaussian(x: f32, sigma: f32) -> f32 {
    return exp(-(x * x) / (2.0 * sigma * sigma));
}

fn luminance(c: vec3f) -> f32 {
    return dot(c, vec3f(0.2126, 0.7152, 0.0722));
}

@fragment
fn bilateral_filter(@location(0) uv: vec2f) -> @location(0) vec4f {
    let texel = 1.0 / u.resolution;
    let center = textureSample(input_tex, tex_sampler, uv).rgb;
    let center_lum = luminance(center);
    
    var color_sum = vec3f(0.0);
    var weight_sum = 0.0;
    
    for (var y = -KERNEL_RADIUS; y <= KERNEL_RADIUS; y++) {
        for (var x = -KERNEL_RADIUS; x <= KERNEL_RADIUS; x++) {
            let offset = vec2f(f32(x), f32(y)) * texel;
            let sample_color = textureSample(input_tex, tex_sampler, uv + offset).rgb;
            
            let spatial_dist = length(vec2f(f32(x), f32(y)));
            let spatial_w = gaussian(spatial_dist, u.sigma_spatial);
            
            let range_dist = abs(center_lum - luminance(sample_color));
            let range_w = gaussian(range_dist, u.sigma_range);
            
            let weight = spatial_w * range_w;
            color_sum += sample_color * weight;
            weight_sum += weight;
        }
    }
    
    return vec4f(color_sum / weight_sum, 1.0);
}
```

**Performance**: O(n²) per pixel for kernel size n. Single-pass but expensive for large kernels. Can be approximated with separable 2-pass horizontal/vertical filters or downsampled bilateral grid.

### Gaussian splatting reconstructs continuous images from discrete points

Gaussian splatting treats each point as an elliptical Gaussian blob, "splatting" contributions to surrounding pixels with falloff based on distance. This naturally handles varying point densities and provides smooth anti-aliasing.

For fractal flames, each accumulated point splats a Gaussian kernel weighted by its contribution. The 3D Gaussian Splatting technique (Kerbl et al., 2023) extends this with learned covariances and spherical harmonics for neural radiance fields, but the core splatting concept applies directly to 2D flame rendering.

```wgsl
struct SplatUniforms {
    mvp: mat4x4f,
    viewport: vec2f,
    splat_radius: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) local_pos: vec2f,
    @location(1) color: vec3f,
    @location(2) sigma: f32,
}

// Instanced quad rendering for each splat
const QUAD: array<vec2f, 6> = array<vec2f, 6>(
    vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0),
    vec2f(-1.0, 1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0)
);

@vertex
fn vs_splat(
    @builtin(vertex_index) vid: u32,
    @builtin(instance_index) iid: u32
) -> VertexOutput {
    var out: VertexOutput;
    let splat = splats[iid];
    let corner = QUAD[vid];
    
    let clip = uniforms.mvp * vec4f(splat.position, 1.0);
    let ndc = clip.xy / clip.w;
    let screen_radius = uniforms.splat_radius / clip.w;
    let screen_offset = corner * screen_radius * 2.0 / uniforms.viewport;
    
    out.position = vec4f(ndc + screen_offset, clip.z / clip.w, 1.0);
    out.local_pos = corner * screen_radius;
    out.color = splat.color;
    out.sigma = screen_radius * 0.5;
    return out;
}

@fragment
fn fs_splat(in: VertexOutput) -> @location(0) vec4f {
    let dist_sq = dot(in.local_pos, in.local_pos);
    let gaussian = exp(-dist_sq / (2.0 * in.sigma * in.sigma));
    
    if (gaussian < 0.01) { discard; }
    
    return vec4f(in.color * gaussian, gaussian);
}
```

**Performance**: Requires depth sorting for correct alpha compositing (not strictly single-pass). With GPU radix sort, achieves **200+ FPS at 1080p with 1M+ splats** on modern hardware. For fractal flames without depth, simpler additive blending eliminates sorting requirement.

### Temporal accumulation with single-frame reprojection reduces noise dramatically

With access to one previous frame, **temporal accumulation** blends current noisy output with history using an exponential moving average. The key challenge is **reprojection**—finding where each pixel was in the previous frame to sample the correct history.

For static cameras (common in fractal flame viewing), reprojection is trivial (UV coordinates match). For animated views, motion vectors computed from the camera transform enable accurate history sampling.

```wgsl
struct TemporalUniforms {
    current_vp: mat4x4f,
    prev_vp: mat4x4f,
    blend_factor: f32,  // 0.05-0.2 typical
}

@group(0) @binding(0) var current_frame: texture_2d<f32>;
@group(0) @binding(1) var history_frame: texture_2d<f32>;
@group(0) @binding(2) var depth_buffer: texture_2d<f32>;
@group(0) @binding(3) var tex_sampler: sampler;
@group(0) @binding(4) var<uniform> u: TemporalUniforms;

fn reproject_uv(current_uv: vec2f, depth: f32) -> vec2f {
    // Reconstruct world position from current UV and depth
    let ndc = vec3f(current_uv * 2.0 - 1.0, depth);
    // ... inverse projection to world, then project with prev_vp
    // For static camera, simply return current_uv
    return current_uv;
}

// Neighborhood clamping prevents ghosting artifacts
fn clip_to_aabb(history: vec3f, color_min: vec3f, color_max: vec3f) -> vec3f {
    let center = 0.5 * (color_max + color_min);
    let half_extent = 0.5 * (color_max - color_min);
    let clip = history - center;
    let unit = clip / max(half_extent, vec3f(0.0001));
    let max_unit = max(abs(unit.x), max(abs(unit.y), abs(unit.z)));
    if (max_unit > 1.0) {
        return center + clip / max_unit;
    }
    return history;
}

@fragment
fn temporal_accumulate(@location(0) uv: vec2f) -> @location(0) vec4f {
    let current = textureSample(current_frame, tex_sampler, uv).rgb;
    let depth = textureSample(depth_buffer, tex_sampler, uv).r;
    let history_uv = reproject_uv(uv, depth);
    
    // Validate reprojection bounds
    let valid = all(history_uv >= vec2f(0.0)) && all(history_uv <= vec2f(1.0));
    
    if (!valid) {
        return vec4f(current, 1.0);
    }
    
    var history = textureSample(history_frame, tex_sampler, history_uv).rgb;
    
    // Compute 3x3 neighborhood bounds for clamping
    var color_min = current;
    var color_max = current;
    let texel = 1.0 / vec2f(textureDimensions(current_frame));
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let neighbor = textureSample(current_frame, tex_sampler, 
                uv + vec2f(f32(x), f32(y)) * texel).rgb;
            color_min = min(color_min, neighbor);
            color_max = max(color_max, neighbor);
        }
    }
    
    history = clip_to_aabb(history, color_min, color_max);
    let result = mix(history, current, u.blend_factor);
    
    return vec4f(result, 1.0);
}
```

**Buffer requirements**: Current frame, history frame (ping-pong), optionally depth and motion vectors. **Single pass** for accumulation, but requires frame-to-frame state management.

### SVGF provides production-quality denoising for sparse data

**Spatiotemporal Variance-Guided Filtering** (Schied et al., 2017) is the industry standard for real-time ray tracing denoising and applies excellently to sparse point reconstruction. It combines temporal accumulation with **variance-guided spatial filtering** using À-Trous wavelets.

The key innovation: blur strength adapts based on estimated **per-pixel variance**—noisy areas receive stronger filtering while converged areas stay sharp.

```wgsl
// SVGF À-Trous filter iteration
struct ATrousUniforms {
    step_size: i32,      // 1, 2, 4, 8, 16 for each iteration
    phi_color: f32,      // Color edge-stopping (4.0 typical)
    phi_normal: f32,     // Normal edge-stopping (128.0)
}

// B3-spline kernel weights (5x5)
const KERNEL_WEIGHTS: array<f32, 25> = array<f32, 25>(
    1.0/256.0, 4.0/256.0, 6.0/256.0, 4.0/256.0, 1.0/256.0,
    4.0/256.0, 16.0/256.0, 24.0/256.0, 16.0/256.0, 4.0/256.0,
    6.0/256.0, 24.0/256.0, 36.0/256.0, 24.0/256.0, 6.0/256.0,
    4.0/256.0, 16.0/256.0, 24.0/256.0, 16.0/256.0, 4.0/256.0,
    1.0/256.0, 4.0/256.0, 6.0/256.0, 4.0/256.0, 1.0/256.0
);

fn svgf_edge_weight(
    center_lum: f32, sample_lum: f32,
    center_normal: vec3f, sample_normal: vec3f,
    center_depth: f32, sample_depth: f32,
    variance: f32, phi_color: f32, phi_normal: f32
) -> f32 {
    // Luminance weight (variance-guided)
    let sigma_l = phi_color * sqrt(max(0.00001, variance));
    let w_lum = exp(-abs(center_lum - sample_lum) / sigma_l);
    
    // Normal weight
    let w_normal = pow(max(0.0, dot(center_normal, sample_normal)), phi_normal);
    
    // Depth weight
    let w_depth = exp(-abs(center_depth - sample_depth) / max(center_depth * 0.1, 0.01));
    
    return w_lum * w_normal * w_depth;
}

@compute @workgroup_size(8, 8)
fn atrous_filter(@builtin(global_invocation_id) id: vec3u) {
    let coord = vec2i(id.xy);
    let step = u.step_size;
    
    let center_color = textureLoad(color_tex, coord, 0).rgb;
    let center_normal = textureLoad(normal_tex, coord, 0).xyz;
    let center_depth = textureLoad(depth_tex, coord, 0).r;
    let variance = textureLoad(variance_tex, coord, 0).r;
    let center_lum = luminance(center_color);
    
    var sum_color = vec3f(0.0);
    var sum_weight = 0.0;
    
    for (var dy = -2; dy <= 2; dy++) {
        for (var dx = -2; dx <= 2; dx++) {
            let offset = vec2i(dx, dy) * step;
            let sample_coord = coord + offset;
            let idx = (dy + 2) * 5 + (dx + 2);
            
            let sample_color = textureLoad(color_tex, sample_coord, 0).rgb;
            let sample_normal = textureLoad(normal_tex, sample_coord, 0).xyz;
            let sample_depth = textureLoad(depth_tex, sample_coord, 0).r;
            
            let edge_w = svgf_edge_weight(
                center_lum, luminance(sample_color),
                center_normal, sample_normal,
                center_depth, sample_depth,
                variance, u.phi_color, u.phi_normal
            );
            
            let weight = KERNEL_WEIGHTS[idx] * edge_w;
            sum_color += sample_color * weight;
            sum_weight += weight;
        }
    }
    
    textureStore(output_tex, coord, vec4f(sum_color / max(sum_weight, 0.0001), 1.0));
}
```

**Pass requirements**: 5-7 passes total (temporal accumulation, variance estimation, 3-5 À-Trous iterations). Each iteration doubles effective blur radius via step_size.

**Memory budget (1080p)**: ~48 MB for full SVGF pipeline (color×2, history, moments, depth, normals, motion vectors).

### Guided filtering offers O(N) edge-preserving smoothing

The **Guided Filter** (He et al., 2010) assumes a local linear relationship between output and guidance image, solved via ridge regression. Unlike bilateral filtering, it has **O(N) complexity independent of kernel size** when implemented with integral images.

```wgsl
// Guided filter - 3-pass implementation
// Pass 1: Compute local means and correlations
@fragment
fn compute_moments(@location(0) uv: vec2f) -> @location(0) vec4f {
    var sum_I = 0.0;   // Guide mean
    var sum_p = 0.0;   // Input mean  
    var sum_Ip = 0.0;  // Correlation
    var sum_II = 0.0;  // Guide variance
    var count = 0.0;
    
    let texel = 1.0 / u.resolution;
    for (var dy = -u.radius; dy <= u.radius; dy++) {
        for (var dx = -u.radius; dx <= u.radius; dx++) {
            let offset = vec2f(f32(dx), f32(dy)) * texel;
            let I = textureSample(guide_tex, samp, uv + offset).r;
            let p = textureSample(input_tex, samp, uv + offset).r;
            
            sum_I += I;
            sum_p += p;
            sum_Ip += I * p;
            sum_II += I * I;
            count += 1.0;
        }
    }
    return vec4f(sum_I, sum_p, sum_Ip, sum_II) / count;
}

// Pass 2: Compute coefficients a, b
@fragment
fn compute_coefficients(@location(0) uv: vec2f) -> @location(0) vec2f {
    let m = textureSample(moments_tex, samp, uv);
    let mean_I = m.x;
    let mean_p = m.y;
    let mean_Ip = m.z;
    let mean_II = m.w;
    
    let cov_Ip = mean_Ip - mean_I * mean_p;
    let var_I = mean_II - mean_I * mean_I;
    
    let a = cov_Ip / (var_I + u.epsilon);
    let b = mean_p - a * mean_I;
    return vec2f(a, b);
}

// Pass 3: Average coefficients and apply
@fragment
fn apply_guided_filter(@location(0) uv: vec2f) -> @location(0) vec4f {
    var sum_a = 0.0;
    var sum_b = 0.0;
    var count = 0.0;
    
    let texel = 1.0 / u.resolution;
    for (var dy = -u.radius; dy <= u.radius; dy++) {
        for (var dx = -u.radius; dx <= u.radius; dx++) {
            let coef = textureSample(coef_tex, samp, uv + vec2f(f32(dx), f32(dy)) * texel);
            sum_a += coef.x;
            sum_b += coef.y;
            count += 1.0;
        }
    }
    
    let mean_a = sum_a / count;
    let mean_b = sum_b / count;
    let I = textureSample(guide_tex, samp, uv).r;
    
    return vec4f(vec3f(mean_a * I + mean_b), 1.0);
}
```

**Suitability for sparse data**: Excellent. The linear model naturally handles upsampling from low-resolution accumulation buffers using high-resolution depth/geometry as guidance.

### Perona-Malik anisotropic diffusion fills sparse regions while preserving edges

**Anisotropic diffusion** iteratively smooths images with a gradient-dependent diffusion coefficient that slows near edges. For sparse data, this naturally "fills in" missing regions while preserving existing structure.

```wgsl
struct DiffusionUniforms {
    k: f32,              // Gradient threshold (conductance)
    dt: f32,             // Time step (≤0.25 for stability)
    diffusion_type: u32, // 0=exp, 1=inverse
}

fn conductance(gradient_mag: f32, k: f32, dtype: u32) -> f32 {
    let ratio = gradient_mag / k;
    if (dtype == 0u) {
        return exp(-ratio * ratio);  // Perona-Malik function 1
    } else {
        return 1.0 / (1.0 + ratio * ratio);  // Function 2
    }
}

@fragment
fn perona_malik_step(@location(0) uv: vec2f) -> @location(0) vec4f {
    let texel = 1.0 / u.resolution;
    let c = textureSample(input_tex, samp, uv).rgb;
    
    // Sample 4-connected neighbors
    let n = textureSample(input_tex, samp, uv + vec2f(0.0, -texel.y)).rgb;
    let s = textureSample(input_tex, samp, uv + vec2f(0.0, texel.y)).rgb;
    let e = textureSample(input_tex, samp, uv + vec2f(texel.x, 0.0)).rgb;
    let w = textureSample(input_tex, samp, uv + vec2f(-texel.x, 0.0)).rgb;
    
    // Gradients
    let grad_n = n - c;
    let grad_s = s - c;
    let grad_e = e - c;
    let grad_w = w - c;
    
    // Diffusion coefficients
    let c_n = conductance(length(grad_n), u.k, u.diffusion_type);
    let c_s = conductance(length(grad_s), u.k, u.diffusion_type);
    let c_e = conductance(length(grad_e), u.k, u.diffusion_type);
    let c_w = conductance(length(grad_w), u.k, u.diffusion_type);
    
    let diffusion = c_n * grad_n + c_s * grad_s + c_e * grad_e + c_w * grad_w;
    return vec4f(c + u.dt * diffusion, 1.0);
}
```

**Pass requirements**: Iterative—typically **10-100+ iterations**, each a single pass. Time step must satisfy CFL condition: `dt ≤ 0.25`.

---

## Part 2: Psychedelic and Creative Shader Effects

### Feedback effects create infinite recursive trails

Feedback loops render the current frame to a texture, then sample that texture with transformations (zoom, rotation) in the next frame. This creates trails, tunnels, and kaleidoscopic recursion.

```wgsl
struct FeedbackUniforms {
    time: f32,
    feedback_amount: f32,  // 0.9-0.99 for long trails
    zoom: f32,             // <1.0 zooms in, >1.0 zooms out
    rotation: f32,         // Radians per frame
}

fn rotate2d(angle: f32) -> mat2x2f {
    let c = cos(angle);
    let s = sin(angle);
    return mat2x2f(vec2f(c, -s), vec2f(s, c));
}

@fragment
fn feedback(@location(0) uv: vec2f) -> @location(0) vec4f {
    // Transform UVs for feedback sampling
    var centered = uv - 0.5;
    centered *= u.zoom;
    centered = rotate2d(u.rotation) * centered;
    let feedback_uv = centered + 0.5;
    
    let prev = textureSample(prev_frame, samp, feedback_uv) * u.feedback_amount;
    
    // Current frame content (e.g., animated shape)
    let dist = length(uv - 0.5);
    let pulse = sin(u.time * 3.0) * 0.1 + 0.15;
    let shape = smoothstep(pulse + 0.02, pulse, dist);
    let current = vec4f(shape, shape * 0.5, shape * 0.8, 1.0);
    
    return max(current, prev);
}
```

**Requires multi-pass**: Yes—ping-pong between two render targets (cannot read and write same texture).

### Kaleidoscope effects through polar coordinate folding

Kaleidoscope creates N-fold rotational symmetry by converting to polar coordinates and "folding" the angle component into a single wedge using modulo arithmetic, then mirroring within that wedge.

```wgsl
const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 6.28318530718;

fn kaleidoscope(uv: vec2f, segments: f32, rotation: f32) -> vec2f {
    var centered = uv - 0.5;
    let radius = length(centered);
    var angle = atan2(centered.y, centered.x) + rotation;
    
    let segment_angle = TWO_PI / segments;
    angle = angle - segment_angle * floor(angle / segment_angle);
    angle = min(angle, segment_angle - angle);  // Mirror within segment
    
    return vec2f(cos(angle), sin(angle)) * radius + 0.5;
}

@fragment
fn kaleidoscope_main(@location(0) uv: vec2f) -> @location(0) vec4f {
    let kaleido_uv = kaleidoscope(uv, 6.0, u.time * 0.3);
    return textureSample(input_tex, samp, kaleido_uv);
}
```

**Single pass**: Yes—pure UV manipulation.

### IQ's cosine palette generates infinite color schemes from 4 vectors

Inigo Quilez's cosine palette function creates smooth, customizable color gradients from just four vec3 parameters, enabling procedural color cycling with minimal code.

```wgsl
fn iq_palette(t: f32, a: vec3f, b: vec3f, c: vec3f, d: vec3f) -> vec3f {
    return a + b * cos(6.28318 * (c * t + d));
}

// Preset palettes
fn rainbow(t: f32) -> vec3f {
    return iq_palette(t,
        vec3f(0.5, 0.5, 0.5),      // Brightness
        vec3f(0.5, 0.5, 0.5),      // Contrast
        vec3f(1.0, 1.0, 1.0),      // Frequency
        vec3f(0.00, 0.33, 0.67)); // Phase offset
}

fn sunset(t: f32) -> vec3f {
    return iq_palette(t,
        vec3f(0.5, 0.5, 0.5),
        vec3f(0.5, 0.5, 0.5),
        vec3f(1.0, 1.0, 0.5),
        vec3f(0.8, 0.9, 0.3));
}

fn neon(t: f32) -> vec3f {
    return iq_palette(t,
        vec3f(0.5, 0.5, 0.5),
        vec3f(0.5, 0.5, 0.5),
        vec3f(2.0, 1.0, 0.0),
        vec3f(0.5, 0.2, 0.25));
}
```

**Parameter guide**: `a` controls base brightness, `b` controls contrast, `c` controls color cycling frequency (integers create seamless loops), `d` offsets each RGB channel's phase to define the color scheme.

### Domain warping creates organic flowing patterns

Domain warping uses nested noise functions where the output of one noise distorts the input coordinates of another, creating organic, flowing, psychedelic patterns.

```wgsl
fn fbm(p: vec2f, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;
    let rot = mat2x2f(0.8, -0.6, 0.6, 0.8);
    
    for (var i = 0; i < octaves; i++) {
        value += amplitude * simplex_noise(pos);
        pos = rot * pos * 2.02;
        amplitude *= 0.5;
    }
    return value;
}

fn domain_warp(p: vec2f, time: f32) -> f32 {
    let q = vec2f(
        fbm(p + vec2f(0.0, 0.0) + time * 0.1, 4),
        fbm(p + vec2f(5.2, 1.3) + time * 0.12, 4)
    );
    
    let r = vec2f(
        fbm(p + 4.0 * q + vec2f(1.7, 9.2), 4),
        fbm(p + 4.0 * q + vec2f(8.3, 2.8), 4)
    );
    
    return fbm(p + 4.0 * r, 4);
}
```

### Simplex noise outperforms Perlin for procedural textures

Simplex noise (Gustavson/McEwan) uses a simplex grid requiring only **N+1 samples** vs Perlin's **2^N**, with better isotropy and no directional artifacts.

```wgsl
fn permute3(x: vec3f) -> vec3f {
    return ((x * 34.0 + 1.0) * x) % 289.0;
}

fn simplex_noise(v: vec2f) -> f32 {
    let C = vec4f(0.211324865405187, 0.366025403784439, -0.577350269189626, 0.024390243902439);
    
    var i = floor(v + dot(v, C.yy));
    let x0 = v - i + dot(i, C.xx);
    
    let i1 = select(vec2f(0.0, 1.0), vec2f(1.0, 0.0), x0.x > x0.y);
    var x12 = x0.xyxy + C.xxzz;
    x12 = vec4f(x12.xy - i1, x12.zw);
    
    i = i % 289.0;
    let p = permute3(permute3(i.y + vec3f(0.0, i1.y, 1.0)) + i.x + vec3f(0.0, i1.x, 1.0));
    
    var m = max(0.5 - vec3f(dot(x0, x0), dot(x12.xy, x12.xy), dot(x12.zw, x12.zw)), vec3f(0.0));
    m = m * m * m * m;
    
    let x = 2.0 * fract(p * C.www) - 1.0;
    let h = abs(x) - 0.5;
    let ox = floor(x + 0.5);
    let a0 = x - ox;
    
    m *= 1.79284291400159 - 0.85373472095314 * (a0 * a0 + h * h);
    
    var g: vec3f;
    g.x = a0.x * x0.x + h.x * x0.y;
    g.y = a0.y * x12.x + h.y * x12.y;
    g.z = a0.z * x12.z + h.z * x12.w;
    
    return 130.0 * dot(m, g);
}
```

### Worley noise creates cellular organic patterns

Worley (cellular) noise computes distance to nearest random feature points, creating cell-like patterns resembling reptile scales, cracked earth, or biological structures.

```wgsl
fn hash2(p: vec2f) -> vec2f {
    let n = vec2f(dot(p, vec2f(127.1, 311.7)), dot(p, vec2f(269.5, 183.3)));
    return fract(sin(n) * 43758.5453);
}

fn worley(p: vec2f, time: f32) -> vec2f {
    let n = floor(p);
    let f = fract(p);
    
    var F1 = 8.0;
    var F2 = 8.0;
    
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let g = vec2f(f32(i), f32(j));
            let o = hash2(n + g);
            let animated = 0.5 + 0.5 * sin(time + 6.28318 * o);
            let r = g - f + animated;
            let d = dot(r, r);
            
            if (d < F1) { F2 = F1; F1 = d; }
            else if (d < F2) { F2 = d; }
        }
    }
    return vec2f(sqrt(F1), sqrt(F2));
}

fn worley_edges(p: vec2f, time: f32) -> f32 {
    let w = worley(p, time);
    return w.y - w.x;  // Cell boundaries highlighted
}
```

### Reaction-diffusion creates organic self-organizing patterns

The **Gray-Scott model** simulates two interacting chemicals producing spots, stripes, and labyrinthine patterns that evolve organically over time.

```wgsl
struct GrayScottParams {
    feed_rate: f32,      // F: try 0.0545
    kill_rate: f32,      // K: try 0.062
    diffusion_a: f32,    // DA: 1.0
    diffusion_b: f32,    // DB: 0.5
    delta_time: f32,
}

fn laplacian(coord: vec2i) -> vec2f {
    var lap = vec2f(0.0);
    lap += textureLoad(state, coord, 0).rg * -1.0;
    lap += textureLoad(state, coord + vec2i(1, 0), 0).rg * 0.2;
    lap += textureLoad(state, coord + vec2i(-1, 0), 0).rg * 0.2;
    lap += textureLoad(state, coord + vec2i(0, 1), 0).rg * 0.2;
    lap += textureLoad(state, coord + vec2i(0, -1), 0).rg * 0.2;
    lap += textureLoad(state, coord + vec2i(1, 1), 0).rg * 0.05;
    lap += textureLoad(state, coord + vec2i(-1, 1), 0).rg * 0.05;
    lap += textureLoad(state, coord + vec2i(1, -1), 0).rg * 0.05;
    lap += textureLoad(state, coord + vec2i(-1, -1), 0).rg * 0.05;
    return lap;
}

@compute @workgroup_size(8, 8)
fn gray_scott_step(@builtin(global_invocation_id) id: vec3u) {
    let coord = vec2i(id.xy);
    let current = textureLoad(state, coord, 0).rg;
    let A = current.r;
    let B = current.g;
    
    let lap = laplacian(coord);
    let reaction = A * B * B;
    
    let newA = A + p.delta_time * (p.diffusion_a * lap.x - reaction + p.feed_rate * (1.0 - A));
    let newB = B + p.delta_time * (p.diffusion_b * lap.y + reaction - (p.kill_rate + p.feed_rate) * B);
    
    textureStore(output_state, coord, vec4f(clamp(newA, 0.0, 1.0), clamp(newB, 0.0, 1.0), 0.0, 1.0));
}
```

**Requirements**: Two RG16F textures (ping-pong), **10-50 compute iterations per rendered frame**, initialization with A=1.0 everywhere and B=1.0 in seed regions.

| Pattern | Feed (F) | Kill (K) |
|---------|----------|----------|
| Mitosis/Coral | 0.0545 | 0.062 |
| Spots | 0.030 | 0.062 |
| Stripes | 0.022 | 0.051 |
| Waves | 0.014 | 0.054 |

### Bloom with chromatic aberration creates dreamy neon effects

Bloom extracts bright pixels, blurs them, and composites back. Adding **chromatic aberration**—offsetting RGB channels radially—creates lens-like color fringing for a psychedelic glow.

```wgsl
fn chromatic_aberration(uv: vec2f, strength: f32) -> vec3f {
    let dir = uv - 0.5;
    let r = textureSample(tex, samp, uv + dir * strength * 0.01).r;
    let g = textureSample(tex, samp, uv + dir * strength * 0.005).g;
    let b = textureSample(tex, samp, uv - dir * strength * 0.005).b;
    return vec3f(r, g, b);
}

fn extract_bright(color: vec3f, threshold: f32) -> vec3f {
    let lum = dot(color, vec3f(0.2126, 0.7152, 0.0722));
    return color * max(0.0, lum - threshold);
}

@fragment
fn bloom_chromatic(@location(0) uv: vec2f) -> @location(0) vec4f {
    let base = chromatic_aberration(uv, u.aberration_strength);
    let bright = extract_bright(gaussian_blur(uv), u.bloom_threshold);
    return vec4f(base + bright * u.bloom_intensity, 1.0);
}
```

**For proper bloom**: Use separate passes—brightness extraction, horizontal blur, vertical blur, composite. Single-pass approximation shown above has limited blur radius.

### Edge detection with glow creates neon outline effects

Sobel edge detection combined with bloom creates striking neon wireframe aesthetics.

```wgsl
fn sobel_edge(uv: vec2f, texel: vec2f) -> f32 {
    let n0 = luminance(textureSample(tex, samp, uv + vec2f(-texel.x, -texel.y)).rgb);
    let n1 = luminance(textureSample(tex, samp, uv + vec2f(0.0, -texel.y)).rgb);
    let n2 = luminance(textureSample(tex, samp, uv + vec2f(texel.x, -texel.y)).rgb);
    let n3 = luminance(textureSample(tex, samp, uv + vec2f(-texel.x, 0.0)).rgb);
    let n5 = luminance(textureSample(tex, samp, uv + vec2f(texel.x, 0.0)).rgb);
    let n6 = luminance(textureSample(tex, samp, uv + vec2f(-texel.x, texel.y)).rgb);
    let n7 = luminance(textureSample(tex, samp, uv + vec2f(0.0, texel.y)).rgb);
    let n8 = luminance(textureSample(tex, samp, uv + vec2f(texel.x, texel.y)).rgb);
    
    let gx = n2 + 2.0*n5 + n8 - (n0 + 2.0*n3 + n6);
    let gy = n0 + 2.0*n1 + n2 - (n6 + 2.0*n7 + n8);
    
    return sqrt(gx*gx + gy*gy);
}
```

### Classic demoscene effects remain visually compelling

**Plasma** (summed sinusoids), **tunnel** (polar coordinate mapping), and **Mandelbrot/Julia fractals** are foundational demoscene effects that create mesmerizing animations with minimal code.

```wgsl
// Plasma
fn plasma(uv: vec2f, time: f32) -> f32 {
    var v = sin(uv.x * 10.0 + time);
    v += sin((uv.y * 10.0 + time) / 2.0);
    v += sin((uv.x * 10.0 + uv.y * 10.0 + time) / 2.0);
    let cx = uv.x + 0.5 * sin(time / 5.0);
    let cy = uv.y + 0.5 * cos(time / 3.0);
    v += sin(sqrt(100.0 * (cx*cx + cy*cy) + 1.0) + time);
    return v / 4.0;
}

// Tunnel
fn tunnel(uv: vec2f, time: f32) -> vec2f {
    let p = (uv - 0.5) * vec2f(u.aspect, 1.0);
    let angle = atan2(p.y, p.x) / PI;
    let radius = length(p);
    return vec2f(angle + time * 0.1, 1.0 / radius + time * 0.5);
}

// Mandelbrot
fn mandelbrot(c: vec2f, max_iter: i32) -> f32 {
    var z = vec2f(0.0);
    for (var i = 0; i < max_iter; i++) {
        z = vec2f(z.x*z.x - z.y*z.y, 2.0*z.x*z.y) + c;
        if (dot(z, z) > 4.0) { return f32(i) / f32(max_iter); }
    }
    return 0.0;
}
```

---

## Recommended pipeline for fractal flame GPU rendering

For a fractal flame renderer with sparse point output and single previous frame access, the optimal reconstruction pipeline is:

1. **Accumulation phase**: Compute shader with Gaussian splatting per point using atomic operations
2. **Temporal pass**: Reproject and blend with clamped history (exponential moving average)
3. **Variance estimation**: Compute spatial variance in 7×7 neighborhood, boost for low history count
4. **Spatial filtering**: 3 iterations of À-Trous wavelet filter with variance-guided edge stopping
5. **Post-process**: Apply psychedelic effects (color cycling, bloom, chromatic aberration) on filtered result

This achieves high-quality reconstruction while maintaining real-time performance. The sparse data naturally fills in through the combination of Gaussian splatting during accumulation and variance-guided diffusion during filtering.

---

## Key references and sources

**Image Reconstruction**:
- Tomasi & Manduchi, "Bilateral Filtering for Gray and Color Images", ICCV 1998
- Schied et al., "Spatiotemporal Variance-Guided Filtering", HPG 2017 — cg.ivd.kit.edu/publications/2017/svgf/
- Dammertz et al., "Edge-Avoiding À-Trous Wavelet Transform", HPG 2010 — jo.dreggn.org/home/2010_atrous.pdf
- Kerbl et al., "3D Gaussian Splatting for Real-Time Radiance Field Rendering", ACM TOG 2023 — repo-sam.inria.fr/fungraph/3d-gaussian-splatting/
- He et al., "Guided Image Filtering", ECCV 2010 — people.csail.mit.edu/kaiming/publications/eccv10guidedfilter.pdf
- LYGIA Shader Library — lygia.xyz

**Creative Effects**:
- Inigo Quilez articles — iquilezles.org/articles/ (palettes, noise, SDFs, domain warping)
- Book of Shaders — thebookofshaders.com (noise, patterns, color)
- Stefan Gustavson webgl-noise — github.com/ashima/webgl-noise
- Karl Sims Reaction-Diffusion — karlsims.com/rd.html
- Shadertoy community — shadertoy.com

**WebGPU/WGSL Implementations**:
- web-splat (Gaussian splatting) — github.com/KeKsBoTer/web-splat
- gaussian-splatting-webgpu — github.com/Scthe/gaussian-splatting-webgpu
- Cuburn (GPU fractal flames) — github.com/stevenrobertson/cuburn
- Fractorium — fractorium.com
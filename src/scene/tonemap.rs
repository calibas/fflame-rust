use serde::{Deserialize, Serialize};

/// Tone mapping mode (affects how HDR is compressed to display range)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToneMapMode {
    /// Linear tone mapping (simple clamping)
    #[serde(rename = "linear", alias = "Linear")]
    Linear,
    /// Logarithmic tone mapping (compresses bright areas)
    #[serde(rename = "logarithmic", alias = "Logarithmic")]
    Logarithmic,
    /// Raw density visualization (shows accumulated density as grayscale)
    /// Useful for analyzing density distribution and fine detail.
    /// The wire/cloud-blob form for this variant is `"density"`.
    #[serde(rename = "density", alias = "DensityVisualization")]
    DensityVisualization,
}

impl Default for ToneMapMode {
    fn default() -> Self {
        Self::Logarithmic
    }
}

/// How to handle color values that exceed [0,1] after exposure/gamma/vibrancy.
/// The shader's clamp-to-[0,1] cliff at the end of the tonemap pass is the
/// Apophysis/JWildfire-compatible default; `MaxNorm` divides all channels by
/// the maximum so hue is preserved exactly when any channel would clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HighlightMode {
    /// Per-channel clamp to [0,1]. Causes hue shift toward CMY/white at high
    /// brightness (orange (1, 0.5, 0) × 5 → (5, 2.5, 0) → clamps to pure
    /// yellow). Matches Apophysis / JWildfire.
    #[serde(rename = "clip", alias = "Clip")]
    Clip,
    /// Hue-preserving rescale. If `max(r,g,b) > 1`, divide all channels by
    /// the max so the brightest channel lands at 1.0 and the others stay in
    /// ratio. No hue shift; bright pixels desaturate by lowering value
    /// rather than blowing out per-channel.
    #[serde(rename = "max_norm", alias = "MaxNorm")]
    MaxNorm,
    /// Reinhard luminance-preserving tonemap: `L_mapped = L / (1 + L)` where
    /// L is Rec.709 luminance. Color is scaled by `L_mapped / L`. Smooth
    /// roll-off, slight desaturation in highlights. Soft "photographic" look.
    #[serde(rename = "reinhard", alias = "Reinhard")]
    Reinhard,
    /// ACES filmic tonemap (Narkowicz approximation): the standard film/HDR
    /// curve used in games and cinema. Slight contrast boost in midtones,
    /// gentle highlight roll-off, hue-preserving.
    #[serde(rename = "filmic", alias = "Filmic")]
    Filmic,
    /// AgX (Troy Sobotka's, Blender's view transform since 4.0; Benjamin
    /// Wrensch's minimal fit of it): the scene's light through a log
    /// encoding and a sigmoid in a wider gamut, so bright colours roll
    /// toward white without the hue skew a per-channel curve gives them
    /// (3d-look-and-tools P4): Filmic's Narkowicz fit is per channel, so a
    /// bright colour's channels saturate apart and its hue slides toward
    /// yellow, cyan or magenta.
    #[serde(rename = "agx", alias = "AgX")]
    AgX,
    /// AgX with its Punchy look (Wrensch's: a power of 1.35 and
    /// saturation 1.4 in AgX's own space): the base's roll-off with more
    /// contrast and colour, as Blender's AgX looks offer.
    #[serde(rename = "agx_punchy", alias = "AgXPunchy")]
    AgXPunchy,
}

impl Default for HighlightMode {
    fn default() -> Self {
        // Preserve existing visual regression baselines.
        Self::Clip
    }
}

impl HighlightMode {
    /// The tonemap shader's `highlight_mode` (`shaders/tonemap.wgsl`).
    pub fn shader_code(self) -> u32 {
        match self {
            HighlightMode::Clip => 0,
            HighlightMode::MaxNorm => 1,
            HighlightMode::Reinhard => 2,
            HighlightMode::Filmic => 3,
            HighlightMode::AgX => 4,
            HighlightMode::AgXPunchy => 5,
        }
    }
}

/// AgX as the tonemap applies it (`highlight_mode` 4, or 5 with the
/// Punchy look), on the CPU: the scene-referred colour to the display's,
/// encoded (about gamma 2.2).
#[cfg(test)]
pub(crate) fn agx_display(rgb: [f32; 3], punchy: bool) -> [f32; 3] {
    // Column-major, as the shader's mat3x3 (and the GLSL it comes from).
    const INSET: [[f32; 3]; 3] = [
        [0.842479062253094, 0.0423282422610123, 0.0423756549057051],
        [0.0784335999999992, 0.878468636469772, 0.0784336],
        [0.0792237451477643, 0.0791661274605434, 0.879142973793104],
    ];
    const OUTSET: [[f32; 3]; 3] = [
        [1.19687900512017, -0.0528968517574562, -0.0529716355144438],
        [-0.0980208811401368, 1.15190312990417, -0.0980434501171241],
        [-0.0990297440797205, -0.0989611768448433, 1.15107367264116],
    ];
    let mul = |m: &[[f32; 3]; 3], v: [f32; 3]| [0, 1, 2].map(|i| m[0][i] * v[0] + m[1][i] * v[1] + m[2][i] * v[2]);
    let (min_ev, max_ev) = (-12.47393f32, 4.026069f32);
    let v = mul(&INSET, rgb.map(|c| c.max(0.0)));
    let v = v.map(|c| (c.max(1.0e-10).log2().clamp(min_ev, max_ev) - min_ev) / (max_ev - min_ev));
    let v = v.map(|x| {
        let x2 = x * x;
        let x4 = x2 * x2;
        15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x - 0.00232
    });
    let v = if punchy {
        let p = v.map(|c| c.max(0.0).powf(1.35));
        let luma = 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
        p.map(|c| luma + 1.4 * (c - luma))
    } else {
        v
    };
    mul(&OUTSET, v).map(|c| c.clamp(0.0, 1.0))
}

/// A single control point on the tone curve
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct CurvePoint {
    /// Input value (0.0 to 1.0)
    pub x: f32,
    /// Output value (0.0 to 1.0)
    pub y: f32,
}

impl CurvePoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x: x.clamp(0.0, 1.0),
            y: y.clamp(0.0, 1.0),
        }
    }
}

/// Tone curve for fine-grained control over tone mapping
/// Similar to Photoshop/Apophysis curves system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToneCurve {
    /// Control points defining the curve (always includes (0,0) and (1,1))
    /// Sorted by x coordinate
    pub points: Vec<CurvePoint>,
}

impl Default for ToneCurve {
    fn default() -> Self {
        Self::linear()
    }
}

impl ToneCurve {
    /// Create a linear curve (identity: output = input)
    pub fn linear() -> Self {
        Self {
            points: vec![
                CurvePoint::new(0.0, 0.0),
                CurvePoint::new(1.0, 1.0),
            ],
        }
    }

    /// Create an S-curve (enhances contrast)
    pub fn s_curve() -> Self {
        Self {
            points: vec![
                CurvePoint::new(0.0, 0.0),
                CurvePoint::new(0.25, 0.15),
                CurvePoint::new(0.5, 0.5),
                CurvePoint::new(0.75, 0.85),
                CurvePoint::new(1.0, 1.0),
            ],
        }
    }

    /// Create a curve that brightens shadows
    pub fn brighten_shadows() -> Self {
        Self {
            points: vec![
                CurvePoint::new(0.0, 0.0),
                CurvePoint::new(0.25, 0.4),
                CurvePoint::new(0.5, 0.6),
                CurvePoint::new(0.75, 0.8),
                CurvePoint::new(1.0, 1.0),
            ],
        }
    }

    /// Create a curve that darkens highlights
    pub fn darken_highlights() -> Self {
        Self {
            points: vec![
                CurvePoint::new(0.0, 0.0),
                CurvePoint::new(0.25, 0.2),
                CurvePoint::new(0.5, 0.4),
                CurvePoint::new(0.75, 0.6),
                CurvePoint::new(1.0, 1.0),
            ],
        }
    }

    /// Evaluate the curve at a given input value using monotonic cubic Hermite interpolation
    /// This creates smooth curves that pass through all control points without overshooting
    pub fn evaluate(&self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);

        // Find the segment containing x
        for i in 0..self.points.len() - 1 {
            let p0 = &self.points[i];
            let p1 = &self.points[i + 1];

            if x >= p0.x && x <= p1.x {
                // Current segment slope
                let delta = (p1.y - p0.y) / (p1.x - p0.x);

                // Compute tangents using Catmull-Rom finite differences
                let m0 = if i == 0 {
                    // First point: use current segment slope
                    delta
                } else {
                    let p_prev = &self.points[i - 1];
                    // Average of left and right slopes
                    let slope_left = (p0.y - p_prev.y) / (p0.x - p_prev.x);
                    (slope_left + delta) * 0.5
                };

                let m1 = if i == self.points.len() - 2 {
                    // Last point: use current segment slope
                    delta
                } else {
                    let p_next = &self.points[i + 2];
                    // Average of left and right slopes
                    let slope_right = (p_next.y - p1.y) / (p_next.x - p1.x);
                    (delta + slope_right) * 0.5
                };

                // Apply monotonicity constraint to prevent overshoot
                let (m0_final, m1_final) = if delta.abs() < 1e-6 {
                    // Flat segment - zero tangents
                    (0.0, 0.0)
                } else {
                    // Fritsch-Carlson monotonicity constraints
                    let alpha = m0 / delta;
                    let beta = m1 / delta;

                    // Check if tangents would cause non-monotonic behavior
                    if alpha < 0.0 || beta < 0.0 {
                        // One tangent points wrong direction - use linear
                        (delta, delta)
                    } else {
                        // Apply constraint: alpha^2 + beta^2 <= 9
                        let sum_sq = alpha * alpha + beta * beta;
                        if sum_sq > 9.0 {
                            let tau = 3.0 / sum_sq.sqrt();
                            (tau * alpha * delta, tau * beta * delta)
                        } else {
                            // No constraint needed
                            (m0, m1)
                        }
                    }
                };

                // Cubic Hermite interpolation
                let t = (x - p0.x) / (p1.x - p0.x);
                let t2 = t * t;
                let t3 = t2 * t;

                let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
                let h10 = t3 - 2.0 * t2 + t;
                let h01 = -2.0 * t3 + 3.0 * t2;
                let h11 = t3 - t2;

                let dx = p1.x - p0.x;
                return h00 * p0.y + h10 * dx * m0_final + h01 * p1.y + h11 * dx * m1_final;
            }
        }

        // Fallback (shouldn't reach here if points include 0 and 1)
        x
    }

    /// Generate a 256-sample lookup texture for fast GPU evaluation
    /// Returns Rgba16Float data as bytes (R channel = curve value, GBA = 0)
    pub fn generate_lut(&self) -> Vec<u8> {
        use half::f16;
        let mut bytes = Vec::with_capacity(256 * 4 * 2); // 256 pixels * 4 components * 2 bytes
        for i in 0..256 {
            // Generate LUT at edge values (i / 255) for proper range coverage
            // This ensures LUT[0] = f(0.0) and LUT[255] = f(1.0)
            let x = i as f32 / 255.0;
            let y = self.evaluate(x);
            // Convert to f16 and write bytes
            bytes.extend_from_slice(&f16::from_f32(y).to_le_bytes());    // R
            bytes.extend_from_slice(&f16::from_f32(0.0).to_le_bytes());  // G
            bytes.extend_from_slice(&f16::from_f32(0.0).to_le_bytes());  // B
            bytes.extend_from_slice(&f16::from_f32(1.0).to_le_bytes());  // A
        }
        bytes
    }

    /// Add a control point and maintain sorted order
    pub fn add_point(&mut self, point: CurvePoint) {
        // Don't allow modifying endpoints
        if point.x <= 0.01 || point.x >= 0.99 {
            return;
        }

        // Check if a point exists at this x coordinate
        if let Some(existing) = self.points.iter_mut().find(|p| (p.x - point.x).abs() < 0.01) {
            existing.y = point.y;
        } else {
            self.points.push(point);
            self.points.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
        }
    }

    /// Remove a control point (can't remove endpoints)
    pub fn remove_point(&mut self, index: usize) {
        // Don't allow removing endpoints
        if index == 0 || index == self.points.len() - 1 {
            return;
        }
        if index < self.points.len() {
            self.points.remove(index);
        }
    }

    /// Move an existing point, and return its index afterwards.
    ///
    /// The points stay sorted by x, so an interior point dragged past a
    /// neighbour changes index. A caller that holds the index across a
    /// drag must carry on with the returned one: the editor kept the old
    /// index, so the moment the point passed its neighbour it started
    /// moving the neighbour instead. Endpoints move vertically only, and
    /// an interior point stays between them.
    pub fn move_point(&mut self, index: usize, new_x: f32, new_y: f32) -> usize {
        let len = self.points.len();
        if index >= len {
            return index;
        }

        // Don't allow moving endpoints horizontally
        if index == 0 || index == len - 1 {
            self.points[index].y = new_y.clamp(0.0, 1.0);
            return index;
        }

        let mut point = self.points.remove(index);
        point.x = new_x.clamp(0.0, 1.0);
        point.y = new_y.clamp(0.0, 1.0);
        // Among the rest (still sorted): after every point left of the
        // new x, before every point right of it, and among equal x where
        // it already was -- what the stable sort this replaces did.
        let left = self.points.partition_point(|p| p.x < point.x);
        let right = self.points.partition_point(|p| p.x <= point.x);
        let at = index.clamp(left, right).clamp(1, self.points.len() - 1);
        self.points.insert(at, point);
        at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AgX's shape (`agx_display`): rising, greys grey, black black, the
    /// scene's middle grey a little under the display's middle, white at
    /// 16 and above, and a bright saturated colour's channels kept in
    /// order -- its hue -- where Clip saturates them apart.
    #[test]
    fn agx_rolls_off_keeping_hue() {
        let grey = |x: f32| agx_display([x, x, x], false);
        let mut last = -1.0f32;
        for i in 0..200 {
            let x = 1.0e-4 * 1.08f32.powi(i);
            let g = grey(x);
            assert!((g[0] - g[1]).abs() < 2.0e-3 && (g[1] - g[2]).abs() < 2.0e-3, "a grey stays grey at {x}: {g:?}");
            assert!(g[0] >= last - 1.0e-6, "rising at {x}");
            last = g[0];
        }
        assert!(grey(0.0)[0] < 0.01, "{:?}", grey(0.0));
        let mid = grey(0.18)[0];
        println!("agx: 0.18 to {mid:.3}, 1 to {:.3}, 16 to {:.3}", grey(1.0)[0], grey(16.0)[0]);
        assert!((0.35..0.55).contains(&mid), "{mid}");
        assert!(grey(16.0)[0] > 0.97);
        let orange = agx_display([8.0, 4.0, 0.5], false);
        assert!(orange[0] > orange[1] && orange[1] > orange[2], "{orange:?}");
        // Punchy: more contrast about the middle, more colour, greys grey.
        let punchy = |x: f32| agx_display([x, x, x], true);
        assert!(punchy(0.05)[0] < grey(0.05)[0] && (punchy(0.05)[0] - punchy(0.05)[2]).abs() < 2.0e-3);
        let (o, p) = (agx_display([1.0, 0.3, 0.1], false), agx_display([1.0, 0.3, 0.1], true));
        assert!(p[0] - p[2] > o[0] - o[2], "punchy saturates: {o:?} {p:?}");
    }

    #[test]
    fn test_linear_curve() {
        let curve = ToneCurve::linear();
        assert_eq!(curve.evaluate(0.0), 0.0);
        assert_eq!(curve.evaluate(0.5), 0.5);
        assert_eq!(curve.evaluate(1.0), 1.0);
    }

    #[test]
    fn test_s_curve() {
        let curve = ToneCurve::s_curve();
        // S-curve should darken shadows and brighten highlights
        assert!(curve.evaluate(0.25) < 0.25);
        assert_eq!(curve.evaluate(0.5), 0.5);
        assert!(curve.evaluate(0.75) > 0.75);
    }

    #[test]
    fn test_add_point() {
        let mut curve = ToneCurve::linear();
        curve.add_point(CurvePoint::new(0.5, 0.7));
        assert_eq!(curve.points.len(), 3);
        assert_eq!(curve.evaluate(0.5), 0.7);
    }

    /// Dragging a point past its neighbour re-sorts the curve; the
    /// returned index is where the dragged point went, so the next
    /// frame's move is still the same point.
    #[test]
    fn a_point_moved_past_its_neighbour_reports_its_new_index() {
        let mut curve = ToneCurve::linear();
        curve.add_point(CurvePoint::new(0.3, 0.3));
        curve.add_point(CurvePoint::new(0.6, 0.6));

        let at = curve.move_point(1, 0.8, 0.9);
        assert_eq!(at, 2);
        assert_eq!((curve.points[at].x, curve.points[at].y), (0.8, 0.9));
        assert_eq!(curve.points[1].x, 0.6, "the neighbour moved instead");
        let xs: Vec<f32> = curve.points.iter().map(|p| p.x).collect();
        assert_eq!(xs, vec![0.0, 0.6, 0.8, 1.0]);

        // Back again, across it and down to the left endpoint: it stays
        // inside the endpoints.
        let at = curve.move_point(at, 0.0, 0.1);
        assert_eq!(at, 1);
        assert_eq!(curve.points[0].x, 0.0);
        assert_eq!(curve.points[0].y, 0.0, "the endpoint was disturbed");

        // Endpoints move vertically only.
        assert_eq!(curve.move_point(3, 0.5, 0.7), 3);
        assert_eq!((curve.points[3].x, curve.points[3].y), (1.0, 0.7));
    }

    #[test]
    fn test_lut_generation() {
        let curve = ToneCurve::linear();
        let lut = curve.generate_lut();
        // LUT is now f16 (2 bytes per component): 256 pixels × 4 components × 2 bytes
        assert_eq!(lut.len(), 256 * 4 * 2);
    }
}

/// AgX (`HighlightMode::AgX`) in the tonemap shader is the curve on the
/// CPU (`agx_display`): an escape picture's grey ramp, Linear at exposure
/// 1 and gamma 1, read back -- each pixel's scene value recovered from the
/// same render under Clip (the output encode undone), and its AgX pixel
/// the curve of it, to the 8-bit rounding.
#[cfg(all(test, feature = "engine-escape"))]
mod agx_gpu_tests {
    use super::*;

    fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .ok()?;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("agx tests"),
            required_features: adapter.features() & wgpu::Features::CLEAR_TEXTURE,
            required_limits: adapter.limits(),
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        }))
        .ok()
    }

    #[test]
    fn agx_in_the_tonemap_is_the_curve() {
        let Some((device, queue)) = device() else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        let (w, h) = (128u32, 96u32);
        let mut config = crate::config::FractalConfig::default();
        config.render_mode = crate::scene::transforms::RenderMode::Escape;
        config.tonemap_mode = ToneMapMode::Linear;
        config.exposure = 1.0;
        config.gamma = 1.0;
        config.levels_enabled = false;
        config.use_curve = false;
        config.background_color = [0.0, 0.0, 0.0];
        let stop = |position: f32, v: f32| crate::scene::palette::ColorStop { position, color: [v, v, v] };
        config.palette.stops = vec![stop(0.0, 0.0), stop(1.0, 1.0)];
        config.escape.center_re = "-0.75".to_string();
        config.escape.center_im = "0.1".to_string();
        config.escape.zoom_log2 = 1.5;
        config.escape.max_iter = 300;
        config.escape.coloring_params.insert("scale".to_string(), 0.05);
        let render = |c: &crate::config::FractalConfig| {
            let job = crate::renderer::RenderJob::new(c, w, h);
            pollster::block_on(crate::renderer::render(&device, &queue, job, &mut crate::renderer::NoProgress)).expect("render").rgba_data
        };
        config.highlight_mode = HighlightMode::Clip;
        let clip = render(&config);
        for (mode, punchy) in [(HighlightMode::AgX, false), (HighlightMode::AgXPunchy, true)] {
            config.highlight_mode = mode;
            let agx = render(&config);
            let (mut n, mut worst) = (0usize, 0.0f32);
            for (c, a) in clip.chunks_exact(4).zip(agx.chunks_exact(4)) {
                if c[0] < 16 || c[0] > 240 {
                    continue;
                }
                let x = (c[0] as f32 / 255.0).powf(2.2);
                let want = agx_display([x, x, x], punchy);
                for k in 0..3 {
                    worst = worst.max((a[k] as f32 - want[k] * 255.0).abs());
                }
                n += 1;
            }
            println!("agx {mode:?}: {n} ramp pixels, worst {worst:.2} levels from the curve");
            assert!(n > 1000, "the ramp is in the frame: {n}");
            assert!(worst <= 3.0, "{mode:?}: {worst}");
        }
    }
}

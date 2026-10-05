//! Exact disc averages by FFT, for McCabe's "Exact discs" averaging
//! (mccabe-multiscale plan, section 6b).
//!
//! McCabe needs `a - b` per scale, never `a` and `b` apart, so scale `i`
//! is one field,
//!
//! ```text
//! D_i = IFFT( FFT(f) . H_i ),  H_i = FFT(disc(r_a)) / sum - FFT(disc(r_b)) / sum
//! ```
//!
//! with antialiased discs (one-cell edge, as Reusser's) centred on cell
//! (0, 0) and wrapped. Each disc is symmetric, so its spectrum is real;
//! the two discs of a scale share one complex FFT (activator in the real
//! part, inhibitor in the imaginary) and each is normalised by its own
//! zero-frequency term, which IS its sum. Two scales' outputs are real,
//! so they share one inverse: `IFFT(F.H_i + i F.H_j) = D_i + i D_j`.
//! Six scales cost one forward and three inverse 2D FFTs a step.
//!
//! A circular convolution is exactly the periodic boundary, which is the
//! only one this serves.
//!
//! A scale's discs can be stretched into ellipses of the same area at an
//! angle (section 9's "lean on purpose"). An ellipse is centrally
//! symmetric too, so its spectrum stays real and nothing else changes.

use super::fft::Fft2d;
use wgpu::*;

/// The most scales a stage holds: McCabe's six. Its spectra and
/// difference fields are sized for them: at 1080p 50 MB each, plus 50 MB
/// of complex scratch.
pub const MAX_SPECTRAL_SCALES: usize = 6;

/// One scale's two discs: activator radius `ra`, inhibitor radius `rb`,
/// both stretched into ellipses of the same area, `stretch` times longer
/// than wide, the long axis at `angle` radians from the x axis. A stretch
/// of 1 is the round disc.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpectralScale {
    pub ra: f32,
    pub rb: f32,
    pub stretch: f32,
    pub angle: f32,
}

impl SpectralScale {
    /// Round discs.
    pub fn round(ra: f32, rb: f32) -> Self {
        SpectralScale { ra, rb, stretch: 1.0, angle: 0.0 }
    }

    /// The map taking a cell offset to the disc's frame: rotate by
    /// `-angle`, then divide the long axis by `sqrt(stretch)` and
    /// multiply the short one by it. Row-major. Exactly the identity
    /// for a round disc, so its fill is the round one's to the bit.
    fn metric(&self) -> [f32; 4] {
        let (s, c) = self.angle.sin_cos();
        let q = self.stretch.max(1.0).sqrt();
        [c / q, s / q, -s * q, c * q]
    }
}

/// One mini-pass's uniform. Mirrored by `SpecParams` in [`SPECTRAL_WGSL`].
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SpecGpu {
    width: u32,
    height: u32,
    /// The field slice the pack pass reads.
    layer: u32,
    scale_i: u32,
    scale_j: u32,
    /// Whether the pair has a second scale.
    has_j: u32,
    ra: f32,
    rb: f32,
    /// 1 / (width height): the inverse's scale, folded into the multiply.
    inv_n: f32,
    /// The first slice of the difference fields this stage writes.
    base: u32,
    /// The fill's [`SpectralScale::metric`], row-major.
    m00: f32,
    m01: f32,
    m10: f32,
    m11: f32,
    pad: [u32; 2],
}

const SPECTRAL_WGSL: &str = r#"
struct SpecParams {
    width: u32,
    height: u32,
    layer: u32,
    scale_i: u32,
    scale_j: u32,
    has_j: u32,
    ra: f32,
    rb: f32,
    inv_n: f32,
    base: u32,
    m00: f32,
    m01: f32,
    m10: f32,
    m11: f32,
    pad1: u32,
    pad2: u32,
};

@group(0) @binding(0) var<uniform> sp: SpecParams;
@group(0) @binding(1) var<storage, read> cin: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read_write> cout: array<vec2<f32>>;
@group(0) @binding(3) var<storage, read_write> spectra: array<f32>;
@group(0) @binding(4) var diffs: texture_storage_2d_array<r32float, write>;
@group(0) @binding(5) var field: texture_2d_array<f32>;

fn cell(gid: vec3<u32>) -> i32 {
    if (gid.x >= sp.width || gid.y >= sp.height) {
        return -1;
    }
    return i32(gid.y * sp.width + gid.x);
}

// The field's first channel, as a complex number.
@compute @workgroup_size(8, 8, 1)
fn pack(@builtin(global_invocation_id) gid: vec3<u32>) {
    let c = cell(gid);
    if (c < 0) {
        return;
    }
    let v = textureLoad(field, vec2<i32>(gid.xy), i32(sp.layer), 0).x;
    cout[c] = vec2<f32>(v, 0.0);
}

// Coverage of the cell at distance d by a disc of radius r, antialiased
// over one cell.
fn disc(r: f32, d: f32) -> f32 {
    return clamp(r + 0.5 - d, 0.0, 1.0);
}

// A scale's two discs, centred on cell (0, 0) and wrapped: one image,
// so a disc wider than the grid is cut off at it (as Reusser's is).
// Activator in the real part, inhibitor in the imaginary. The distance
// is measured in the disc's own frame, which makes a stretched disc an
// ellipse; the identity for a round one.
@compute @workgroup_size(8, 8, 1)
fn fill(@builtin(global_invocation_id) gid: vec3<u32>) {
    let c = cell(gid);
    if (c < 0) {
        return;
    }
    let x = f32(select(i32(gid.x), i32(gid.x) - i32(sp.width), gid.x * 2u > sp.width));
    let y = f32(select(i32(gid.y), i32(gid.y) - i32(sp.height), gid.y * 2u > sp.height));
    let d = length(vec2<f32>(sp.m00 * x + sp.m01 * y, sp.m10 * x + sp.m11 * y));
    cout[c] = vec2<f32>(disc(sp.ra, d), disc(sp.rb, d));
}

// A scale's spectrum from its two discs' transform: each disc is
// symmetric, so its transform is real and sits in one part; its
// zero-frequency term is its sum, so dividing by it normalises it.
@compute @workgroup_size(8, 8, 1)
fn extract(@builtin(global_invocation_id) gid: vec3<u32>) {
    let c = cell(gid);
    if (c < 0) {
        return;
    }
    let n = sp.width * sp.height;
    let dc = cin[0];
    let v = cin[c];
    spectra[sp.scale_i * n + u32(c)] = v.x / dc.x - v.y / dc.y;
}

// F . (H_i + i H_j) / n: two scales' convolutions in one complex field.
@compute @workgroup_size(8, 8, 1)
fn multiply(@builtin(global_invocation_id) gid: vec3<u32>) {
    let c = cell(gid);
    if (c < 0) {
        return;
    }
    let n = sp.width * sp.height;
    let f = cin[c];
    let hi = spectra[sp.scale_i * n + u32(c)];
    var hj = 0.0;
    if (sp.has_j != 0u) {
        hj = spectra[sp.scale_j * n + u32(c)];
    }
    cout[c] = vec2<f32>(f.x * hi - f.y * hj, f.y * hi + f.x * hj) * sp.inv_n;
}

// The inverse's real part is scale i's difference field, its imaginary
// part scale j's.
@compute @workgroup_size(8, 8, 1)
fn unpack(@builtin(global_invocation_id) gid: vec3<u32>) {
    let c = cell(gid);
    if (c < 0) {
        return;
    }
    let v = cin[c];
    textureStore(diffs, vec2<i32>(gid.xy), i32(sp.base + sp.scale_i), vec4<f32>(v.x, 0.0, 0.0, 0.0));
    if (sp.has_j != 0u) {
        textureStore(diffs, vec2<i32>(gid.xy), i32(sp.base + sp.scale_j), vec4<f32>(v.y, 0.0, 0.0, 0.0));
    }
}
"#;

/// The exact-disc averages of one layer of one grid. Its difference
/// fields go to slices `base..base + scales` of an `R32Float` array the
/// renderer owns, one for every layer that has a stage, which is what
/// the steps read.
pub struct SpectralAverages {
    pub width: u32,
    pub height: u32,
    fft: Fft2d,
    /// Per scale, its real spectrum: `scales * width * height` floats.
    spectra: Buffer,
    params: Buffer,
    stride: u32,
    layout: BindGroupLayout,
    pack: ComputePipeline,
    fill: ComputePipeline,
    extract: ComputePipeline,
    multiply: ComputePipeline,
    unpack: ComputePipeline,
    /// `groups[side][a][b]`: the field's ping-pong side `side`, complex
    /// buffer `a` read, `b` written.
    groups: Vec<Vec<Vec<Option<BindGroup>>>>,
    /// The field views and the difference array the groups were made for.
    bound: Option<[TextureView; 3]>,
    /// The scales the spectra were made for.
    radii: Vec<SpectralScale>,
    layer: u32,
    base: u32,
}

/// Uniform slot 0: pack. 1..=MAX: fill/extract for scale k. Then the
/// pairs, multiply and unpack for pair q.
const SLOT_PACK: u32 = 0;
const fn slot_scale(k: usize) -> u32 {
    1 + k as u32
}
const fn slot_pair(q: usize) -> u32 {
    1 + MAX_SPECTRAL_SCALES as u32 + q as u32
}
const SLOTS: u32 = 1 + MAX_SPECTRAL_SCALES as u32 + (MAX_SPECTRAL_SCALES as u32).div_ceil(2);

impl SpectralAverages {
    /// The stage for a `width` x `height` grid reading field slice
    /// `layer` and writing difference slices from `base`, or `None` when
    /// the grid has no FFT plan or its buffers would pass the device's
    /// binding limit.
    pub fn new(device: &Device, width: u32, height: u32, layer: u32, base: u32) -> Option<Self> {
        let cells = (width as u64) * (height as u64);
        let limits = device.limits();
        let spectra_size = cells * 4 * MAX_SPECTRAL_SCALES as u64;
        if spectra_size > limits.max_storage_buffer_binding_size as u64
            || cells * 8 > limits.max_storage_buffer_binding_size as u64
        {
            return None;
        }
        let fft = Fft2d::new(device, width, height)?;
        let spectra = device.create_buffer(&BufferDescriptor {
            label: Some("Sim Spectra"),
            size: spectra_size,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let align = limits.min_uniform_buffer_offset_alignment;
        let stride = (std::mem::size_of::<SpecGpu>() as u32).div_ceil(align) * align;
        let params = device.create_buffer(&BufferDescriptor {
            label: Some("Sim Spectral Params"),
            size: (stride * SLOTS) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let storage = |binding: u32, read_only: bool| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Sim Spectral"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage(1, true),
                storage(2, false),
                storage(3, false),
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::R32Float,
                        view_dimension: TextureViewDimension::D2Array,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Sim Spectral"),
            source: ShaderSource::Wgsl(SPECTRAL_WGSL.into()),
        });
        let pl = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Sim Spectral"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let make = |entry: &str| {
            device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Sim Spectral"),
                layout: Some(&pl),
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let s = SpectralAverages {
            width,
            height,
            spectra,
            params,
            stride,
            pack: make("pack"),
            fill: make("fill"),
            extract: make("extract"),
            multiply: make("multiply"),
            unpack: make("unpack"),
            layout,
            fft,
            groups: Vec::new(),
            bound: None,
            radii: Vec::new(),
            layer,
            base,
        };
        Some(s)
    }

    /// An `R32Float` array for difference fields: `slices` of them.
    pub fn create_diffs(device: &Device, width: u32, height: u32, slices: u32) -> (Texture, TextureView) {
        let tex = device.create_texture(&TextureDescriptor {
            label: Some("Sim Diffs"),
            size: Extent3d { width, height, depth_or_array_layers: slices.max(1) },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R32Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2Array),
            ..Default::default()
        });
        (tex, view)
    }

    /// Make the bind groups for the field's two ping-pong views and the
    /// difference array, if they are not the ones already made for.
    pub fn bind(&mut self, device: &Device, field: [&TextureView; 2], diffs: &TextureView) {
        let want = [field[0].clone(), field[1].clone(), diffs.clone()];
        if self.bound.as_ref() == Some(&want) && !self.groups.is_empty() {
            return;
        }
        self.groups = (0..2)
            .map(|side| {
                (0..3)
                    .map(|a| {
                        (0..3)
                            .map(|b| {
                                (a != b).then(|| {
                                    device.create_bind_group(&BindGroupDescriptor {
                                        label: Some("Sim Spectral"),
                                        layout: &self.layout,
                                        entries: &[
                                            BindGroupEntry {
                                                binding: 0,
                                                resource: BindingResource::Buffer(BufferBinding {
                                                    buffer: &self.params,
                                                    offset: 0,
                                                    size: std::num::NonZeroU64::new(std::mem::size_of::<SpecGpu>() as u64),
                                                }),
                                            },
                                            BindGroupEntry { binding: 1, resource: self.fft.buffer(a).as_entire_binding() },
                                            BindGroupEntry { binding: 2, resource: self.fft.buffer(b).as_entire_binding() },
                                            BindGroupEntry { binding: 3, resource: self.spectra.as_entire_binding() },
                                            BindGroupEntry { binding: 4, resource: BindingResource::TextureView(diffs) },
                                            BindGroupEntry { binding: 5, resource: BindingResource::TextureView(field[side]) },
                                        ],
                                    })
                                })
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect();
        self.bound = Some(want);
    }

    /// Make the scales' spectra for these discs, if they are not the
    /// ones already made. Submits its own work; the stage must be bound
    /// first.
    pub fn set_radii(&mut self, device: &Device, queue: &Queue, radii: &[SpectralScale]) {
        let radii: Vec<SpectralScale> = radii.iter().take(MAX_SPECTRAL_SCALES).copied().collect();
        if radii == self.radii {
            return;
        }
        self.write_params(queue, &radii);
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Sim Spectra") });
        {
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("Sim Spectra"), timestamp_writes: None });
            let (gx, gy) = (self.width.div_ceil(8), self.height.div_ceil(8));
            for k in 0..radii.len() {
                let off = slot_scale(k) * self.stride;
                pass.set_pipeline(&self.fill);
                pass.set_bind_group(0, self.group(0, 1, 0), &[off]);
                pass.dispatch_workgroups(gx, gy, 1);
                let out = self.fft.encode(&mut pass, false, 0, 1);
                let other = if out == 0 { 1 } else { 0 };
                pass.set_pipeline(&self.extract);
                pass.set_bind_group(0, self.group(0, out, other), &[off]);
                pass.dispatch_workgroups(gx, gy, 1);
            }
        }
        queue.submit(std::iter::once(enc.finish()));
        self.radii = radii;
    }

    fn write_params(&self, queue: &Queue, radii: &[SpectralScale]) {
        let base = SpecGpu {
            width: self.width,
            height: self.height,
            layer: self.layer,
            scale_i: 0,
            scale_j: 0,
            has_j: 0,
            ra: 0.0,
            rb: 0.0,
            inv_n: 1.0 / (self.width as f32 * self.height as f32),
            base: self.base,
            m00: 1.0,
            m01: 0.0,
            m10: 0.0,
            m11: 1.0,
            pad: [0; 2],
        };
        let mut bytes = vec![0u8; (self.stride * SLOTS) as usize];
        let mut put = |slot: u32, v: SpecGpu| {
            let at = (slot * self.stride) as usize;
            bytes[at..at + std::mem::size_of::<SpecGpu>()].copy_from_slice(bytemuck::bytes_of(&v));
        };
        put(SLOT_PACK, base);
        for (k, s) in radii.iter().enumerate() {
            let [m00, m01, m10, m11] = s.metric();
            put(slot_scale(k), SpecGpu { scale_i: k as u32, ra: s.ra, rb: s.rb, m00, m01, m10, m11, ..base });
        }
        for q in 0..radii.len().div_ceil(2) {
            let (i, j) = (2 * q, 2 * q + 1);
            let has_j = j < radii.len();
            put(slot_pair(q), SpecGpu { scale_i: i as u32, scale_j: j as u32, has_j: u32::from(has_j), ..base });
        }
        queue.write_buffer(&self.params, 0, &bytes);
    }

    fn group(&self, side: usize, a: usize, b: usize) -> &BindGroup {
        self.groups[side][a][b].as_ref().expect("bind first; a != b")
    }

    /// Record one step's difference fields from the field on ping-pong
    /// side `side`: pack, forward, and per pair of scales multiply,
    /// inverse and unpack.
    pub fn encode(&self, pass: &mut ComputePass<'_>, side: usize) {
        let (gx, gy) = (self.width.div_ceil(8), self.height.div_ceil(8));
        pass.set_pipeline(&self.pack);
        pass.set_bind_group(0, self.group(side, 1, 0), &[SLOT_PACK * self.stride]);
        pass.dispatch_workgroups(gx, gy, 1);
        // The spectrum stays in `spec` while each pair's inverse
        // ping-pongs through the other two buffers.
        let spec = self.fft.encode(pass, false, 0, 1);
        let others: Vec<usize> = (0..3).filter(|&k| k != spec).collect();
        for q in 0..self.radii.len().div_ceil(2) {
            let off = slot_pair(q) * self.stride;
            pass.set_pipeline(&self.multiply);
            pass.set_bind_group(0, self.group(side, spec, others[0]), &[off]);
            pass.dispatch_workgroups(gx, gy, 1);
            let out = self.fft.encode(pass, true, others[0], others[1]);
            pass.set_pipeline(&self.unpack);
            let other = if out == others[0] { others[1] } else { others[0] };
            pass.set_bind_group(0, self.group(side, out, other), &[off]);
            pass.dispatch_workgroups(gx, gy, 1);
        }
    }

    /// Whether this stage serves field slice `layer` from difference
    /// slice `base`.
    pub fn serves(&self, layer: u32, base: u32) -> bool {
        self.layer == layer && self.base == base
    }

    /// Dispatches one step's [`Self::encode`] makes.
    pub fn dispatches(&self) -> usize {
        let pairs = self.radii.len().div_ceil(2);
        1 + self.fft.stage_count() + pairs * (2 + self.fft.stage_count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::naga;

    #[test]
    fn the_spectral_shaders_validate() {
        let module = naga::front::wgsl::parse_str(SPECTRAL_WGSL).expect("parses");
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module)
            .expect("validates");
    }

    #[test]
    fn a_round_disc_s_metric_is_exactly_the_identity() {
        assert_eq!(SpectralScale::round(3.0, 6.0).metric(), [1.0, 0.0, -0.0, 1.0]);
    }

    #[test]
    fn the_uniform_matches_its_wgsl_mirror() {
        // 16 four-byte fields, the WGSL struct's size.
        assert_eq!(std::mem::size_of::<SpecGpu>(), 64);
    }
}

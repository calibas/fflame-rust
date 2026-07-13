//! Solid-rendering deferred shade pass (Phase 1).
//!
//! Host side of `shaders/shade.wgsl`: reads the current accumulator + the
//! depth region inside the histogram buffer, writes a shaded Rgba32Float
//! texture that feeds `tonemap_pass_with_input` (density effects, when
//! enabled, read the shade output instead of the accumulator).
//!
//! Zero cost when off: the pass is simply not dispatched (and this
//! module's output texture is the only standing allocation, created
//! lazily on first use). See docs/projects/solid-rendering.md.

use egui_wgpu::wgpu::*;

use crate::config::SolidShadingSettings;

/// One light, GPU layout. Directions are precomputed host-side in
/// camera space; `enabled` is baked into intensity (0 = off).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadeLight {
    dir_intensity: [f32; 4],
    color: [f32; 4],
}

/// Mirrors WGSL `ShadeParams` (shade.wgsl) — 20 scalars + 6 vec4s +
/// 4 lights × 32 B = 304 bytes.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadeParams {
    width: u32,
    height: u32,
    zoom: f32,
    rotation: f32,
    pan_x: f32,
    pan_y: f32,
    perspective_strength: f32,
    shading_strength: f32,
    ambient: f32,
    diffuse: f32,
    specular: f32,
    shininess: f32,
    ssao_strength: f32,
    ssao_radius: f32,
    surface_thickness: f32,
    depth_word_offset: u32,
    tex_y0: u32,
    tex_height: u32,
    use_normal_tex: u32,
    gap_fill: u32,
    // Phase 2 camera + volume block (see shade.wgsl ShadeParams).
    cam_row0: [f32; 4],
    cam_row1: [f32; 4],
    cam_row2: [f32; 4],
    cam_pos: [f32; 4],
    vol_center: [f32; 4],
    volume_dim: u32,
    volume_extent: f32,
    vol_density_scale: f32,
    shadow_strength: f32,
    vol_trust: f32,
    temporal_ema: f32,
    solid_strength: f32,
    base_alpha: f32,
    lights: [ShadeLight; 4],
}

/// Mirrors WGSL `MipParams` (volume_mip.wgsl).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct MipParams {
    dim: u32,
    half_dim: u32,
    radius: u32,
    /// Pre-smoothing per-voxel count cap (see volume_mip.wgsl).
    clamp_raw: f32,
}

/// Mirrors WGSL `AtrousParams` (atrous.wgsl).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct AtrousParams {
    width: u32,
    height: u32,
    stride: u32,
    _pad0: u32,
    sigma_z: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
}

/// Phase 2 density-volume inputs for the shade pass. Camera values are
/// the same (radian) values the main compute pass receives — the shade
/// pass rebuilds the identical world→camera rotation to invert it.
pub struct VolumeShadeInput<'a> {
    pub buffer: &'a Buffer,
    pub dim: u32,
    pub extent: f32,
    /// Total samples splatted into the volume (≈ samples in the
    /// histogram) — normalizes voxel counts to an iteration-invariant
    /// density scale.
    pub splats: f32,
    pub camera_rotation_x: f32,
    pub camera_rotation_y: f32,
    pub camera_bank: f32,
    pub camera_pos: [f32; 3],
    /// World-space center of the grid cube (must match what the splat
    /// used — FlameRenderer::volume_placement is the single source).
    pub center: [f32; 3],
    /// Morphological closing radius (half-res voxels, 0-2). See
    /// volume_mip.wgsl.
    pub closing: u32,
    /// 0-1: how well the grid resolves the current view (host-computed
    /// from voxels across the visible width). Scales every volume
    /// shading feature.
    pub trust: f32,
    /// Scene-mean per-pixel accumulated density (Σaccum.a / pixels) —
    /// the tonemap alpha for base-coat-only pixels and the detail
    /// layer's confidence scale.
    pub base_alpha: f32,
}

/// Effective world→camera rotation rows, replicating EXACTLY what the
/// main pass computes: `build_camera_matrix(0, -pitch, -bank, yaw)`
/// (utilities.wgsl — yaw rides the matrix's roll slot, see the comments
/// there) applied through `camera_transform`'s transposed indexing.
/// Row r satisfies cam[r] = dot(row_r, world − camera_pos); the matrix
/// is orthonormal, so the shade pass inverts it by transposition.
pub fn effective_camera_rows(camera_rotation_x: f32, camera_rotation_y: f32, camera_bank: f32) -> [[f32; 3]; 3] {
    let (sy, cy) = (0.0f32, 1.0f32); // WGSL yaw slot is always 0
    let (sp, cp) = ((-camera_rotation_x).sin(), (-camera_rotation_x).cos());
    let (sb, cb) = ((-camera_bank).sin(), (-camera_bank).cos());
    let (sr, cr) = (camera_rotation_y.sin(), camera_rotation_y.cos());
    // Columns exactly as written in build_camera_matrix.
    let col0 = [
        cy * cb * cr - sy * cp * sr + sy * sp * sb * cr,
        -sy * cb * cr - cy * cp * sr + cy * sp * sb * cr,
        sp * sr + cp * sb * cr,
    ];
    let col1 = [
        cy * cb * sr + sy * cp * cr + sy * sp * sb * sr,
        -sy * cb * sr + cy * cp * cr + cy * sp * sb * sr,
        -sp * cr + cp * sb * sr,
    ];
    let col2 = [
        -cy * sb + sy * sp * cb,
        sy * sb + cy * sp * cb,
        cp * cb,
    ];
    // camera_transform reads m[col][row]: effective row r gathers
    // component r of each column.
    [
        [col0[0], col1[0], col2[0]],
        [col0[1], col1[1], col2[1]],
        [col0[2], col1[2], col2[2]],
    ]
}

pub struct ShadePass {
    pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
    params_buffer: Buffer,
    // Normal-estimation + à-trous smoothing pipelines (full-image paths).
    normals_pipeline: ComputePipeline,
    normals_bgl: BindGroupLayout,
    atrous_pipeline: ComputePipeline,
    atrous_bgl: BindGroupLayout,
    /// One uniform buffer per à-trous iteration (strides 1, 2, 4) —
    /// distinct buffers because queue writes flatten before dispatches.
    atrous_params: [Buffer; 3],
    /// Full-image (normal.xyz, depth) ping-pong; sized (w, h) when present.
    normal_texs: Option<(u32, u32, [(Texture, TextureView); 2])>,
    /// 1×1 stand-in bound when the inline-normal path is used.
    dummy_normal: (Texture, TextureView),
    /// 4-byte stand-in bound at bindings 5/6 when no density volume
    /// exists (read-read aliasing of one dummy is fine).
    dummy_volume: Buffer,
    // Volume derivation (volume_mip.wgsl): raw splat grid → half-res
    // smoothed (avg) + morphologically closed fields, rebuilt each shade.
    mip_reduce_pipeline: ComputePipeline,
    mip_dilate_pipeline: ComputePipeline,
    mip_erode_pipeline: ComputePipeline,
    mip_bgl: BindGroupLayout,
    mip_params: Buffer,
    /// (raw dim, [avg, vmax, tmp, closed, color]) — allocated on first
    /// volume shade, recreated when the raw dim changes. `color` is
    /// hd³ × vec4<f32> (base-coat rgb + mean count).
    vol_derived: Option<(u32, [Buffer; 5])>,
    /// Full-image output ping-pong (interactive path; two textures so
    /// the temporal blend can read last frame's result while writing
    /// this frame's). Absent for pipeline-only users (exporters bring
    /// their own region-sized outputs).
    output: Option<[(Texture, TextureView); 2]>,
    /// Which output the LAST interactive shade wrote (display source).
    output_front: usize,
    /// Whether output[1 - front] holds a valid previous frame.
    output_has_history: bool,
    width: u32,
    height: u32,
}

impl ShadePass {
    /// Pipeline-only construction (exporters): no standing output
    /// allocation; callers provide region outputs to `run_region`.
    pub fn new_pipeline_only(device: &Device) -> Self {
        let mut this = Self::new(device, 1, 1);
        this.output = None;
        this.normal_texs = None;
        this.vol_derived = None;
        this.width = 0;
        this.height = 0;
        this
    }

    pub fn new(device: &Device, width: u32, height: u32) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Solid Shade Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/shade.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Shade Bind Group Layout"),
            entries: &[
                // 0: accumulator (Rgba32Float, textureLoad — non-filterable)
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 1: histogram buffer (read-only; depth region at W*H*4 words)
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 2: ShadeParams uniform
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 3: shaded output (storage write)
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba32Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                // 4: pre-smoothed normals (1×1 dummy when inline path used)
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 5: smoothed half-res density (4-byte dummy when absent)
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 6: closed half-res density (4-byte dummy when absent)
                BindGroupLayoutEntry {
                    binding: 6,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 7: previous frame's shade output (temporal blend;
                // 1×1 dummy when off)
                BindGroupLayoutEntry {
                    binding: 7,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // 8: half-res base-coat color (4-byte dummy when absent)
                BindGroupLayoutEntry {
                    binding: 8,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 9: RAW splat grid (march opacity; dummy when absent)
                BindGroupLayoutEntry {
                    binding: 9,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Shade Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Shade Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("shade_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Shade Params"),
            size: std::mem::size_of::<ShadeParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── Normal estimation pipeline ──
        let normals_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Solid Normals Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/normals.wgsl").into()),
        });
        let normals_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Normals Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba32Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let normals_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Normals Pipeline Layout"),
            bind_group_layouts: &[Some(&normals_bgl)],
            immediate_size: 0,
        });
        let normals_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Normals Pipeline"),
            layout: Some(&normals_layout),
            module: &normals_shader,
            entry_point: Some("normals_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // ── À-trous smoothing pipeline ──
        let atrous_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Solid Atrous Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/atrous.wgsl").into()),
        });
        let atrous_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Atrous Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba32Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let atrous_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Atrous Pipeline Layout"),
            bind_group_layouts: &[Some(&atrous_bgl)],
            immediate_size: 0,
        });
        let atrous_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Atrous Pipeline"),
            layout: Some(&atrous_layout),
            module: &atrous_shader,
            entry_point: Some("atrous_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let atrous_params = [
            Self::create_atrous_params(device, 0),
            Self::create_atrous_params(device, 1),
            Self::create_atrous_params(device, 2),
        ];
        let dummy_normal = Self::create_normal_tex(device, 1, 1, "Shade Dummy Normal");
        let dummy_volume = device.create_buffer(&BufferDescriptor {
            label: Some("Shade Dummy Volume"),
            // 16 bytes: binding 8 is array<vec4<f32>> (16-byte element),
            // and one dummy serves bindings 5/6/8 when the volume is off.
            size: 16,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        // ── Volume derivation pipelines (reduce / dilate / erode) ──
        let mip_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Volume Mip Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/volume_mip.wgsl").into()),
        });
        let mip_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Volume Mip Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 5: half-res base-coat color (reduce writes; the
                // dilate/erode passes bind it unused).
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let mip_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Volume Mip Pipeline Layout"),
            bind_group_layouts: &[Some(&mip_bgl)],
            immediate_size: 0,
        });
        let mip_pipeline = |entry: &str, label: &str| {
            device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&mip_layout),
                module: &mip_shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let mip_reduce_pipeline = mip_pipeline("reduce_main", "Volume Reduce Pipeline");
        let mip_dilate_pipeline = mip_pipeline("dilate_main", "Volume Dilate Pipeline");
        let mip_erode_pipeline = mip_pipeline("erode_main", "Volume Erode Pipeline");
        let mip_params = device.create_buffer(&BufferDescriptor {
            label: Some("Volume Mip Params"),
            size: std::mem::size_of::<MipParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let output = Some([
            Self::create_output(device, width, height),
            Self::create_output(device, width, height),
        ]);
        let normal_texs = Some((
            width,
            height,
            [
                Self::create_normal_tex(device, width, height, "Shade Normals A"),
                Self::create_normal_tex(device, width, height, "Shade Normals B"),
            ],
        ));

        Self {
            pipeline,
            bind_group_layout,
            params_buffer,
            normals_pipeline,
            normals_bgl,
            atrous_pipeline,
            atrous_bgl,
            atrous_params,
            normal_texs,
            dummy_normal,
            dummy_volume,
            mip_reduce_pipeline,
            mip_dilate_pipeline,
            mip_erode_pipeline,
            mip_bgl,
            mip_params,
            vol_derived: None,
            output,
            output_front: 0,
            output_has_history: false,
            width,
            height,
        }
    }

    fn create_atrous_params(device: &Device, idx: u32) -> Buffer {
        device.create_buffer(&BufferDescriptor {
            label: Some(&format!("Atrous Params {}", idx)),
            size: std::mem::size_of::<AtrousParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn create_normal_tex(device: &Device, width: u32, height: u32, label: &str) -> (Texture, TextureView) {
        let tex = device.create_texture(&TextureDescriptor {
            label: Some(label),
            size: Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&TextureViewDescriptor::default());
        (tex, view)
    }

    /// Allocate (or resize) the full-image normal ping-pong — exporters
    /// call this before `run_region` to enable à-trous on one-shot shades.
    pub fn ensure_normal_textures(&mut self, device: &Device, width: u32, height: u32) {
        let needs = match &self.normal_texs {
            Some((w, h, _)) => *w != width || *h != height,
            None => true,
        };
        if needs {
            self.normal_texs = Some((
                width,
                height,
                [
                    Self::create_normal_tex(device, width, height, "Shade Normals A"),
                    Self::create_normal_tex(device, width, height, "Shade Normals B"),
                ],
            ));
        }
    }

    fn create_output(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
        let tex = device.create_texture(&TextureDescriptor {
            label: Some("Shade Output"),
            size: Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&TextureViewDescriptor::default());
        (tex, view)
    }

    pub fn resize(&mut self, device: &Device, width: u32, height: u32) {
        if width == self.width && height == self.height {
            return;
        }
        self.output = Some([
            Self::create_output(device, width, height),
            Self::create_output(device, width, height),
        ]);
        self.output_has_history = false;
        self.ensure_normal_textures(device, width, height);
        self.width = width;
        self.height = height;
    }

    pub fn output_view(&self) -> &TextureView {
        &self.output.as_ref().expect("ShadePass built pipeline-only has no output")[self.output_front].1
    }

    /// Drop the temporal history (lighting params changed, iteration
    /// reset, ...): the next shade starts fresh instead of blending
    /// against a stale look.
    pub fn reset_temporal(&mut self) {
        self.output_has_history = false;
    }

    /// Allocate (or re-size) the half-res derived-volume buffers
    /// (smoothed + closing chain) for a raw grid of `dim`. Must run
    /// before a `run_region` that receives a volume — the dispatch
    /// itself is `&self` (exporters share it) so it cannot allocate.
    pub fn ensure_vol_derived(&mut self, device: &Device, dim: u32) {
        if dim == 0 {
            return;
        }
        if matches!(&self.vol_derived, Some((d, _)) if *d == dim) {
            return;
        }
        let hd = (dim / 2).max(1);
        let size = u64::from(hd).pow(3) * 4;
        let mk = |label: &str, sz: u64| {
            device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size: sz,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: false,
            })
        };
        self.vol_derived = Some((dim, [
            mk("Volume Avg", size),
            mk("Volume Max", size),
            mk("Volume Closing Tmp", size),
            mk("Volume Closed", size),
            mk("Volume Base Color", size * 4),
        ]));
    }

    /// Interactive-path dispatch: full image, depth region inside the
    /// histogram binding, internal output texture. The caller guarantees
    /// the depth region exists and this runs after the frame's
    /// accumulate pass.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        accumulation_view: &TextureView,
        histogram_buffer: &Buffer,
        shading: &SolidShadingSettings,
        zoom: f32,
        rotation: f32,
        pan_x: f32,
        pan_y: f32,
        perspective_strength: f32,
        surface_thickness: f32,
        solid_strength: f32,
        volume: Option<&VolumeShadeInput>,
        temporal_ema: f32,
    ) {
        if let Some(v) = volume {
            self.ensure_vol_derived(device, v.dim);
        }
        // Temporal blend ping-pong: write the back buffer while reading
        // the front (last frame). No history → blend weight 0.
        let back = 1 - self.output_front;
        let ema = if self.output_has_history { temporal_ema.clamp(0.0, 0.95) } else { 0.0 };
        let output = self.output.take().expect("interactive ShadePass has an output");
        self.run_region(
            device,
            queue,
            encoder,
            accumulation_view,
            histogram_buffer,
            &output[back].1,
            shading,
            zoom,
            rotation,
            pan_x,
            pan_y,
            perspective_strength,
            surface_thickness,
            solid_strength,
            self.width,
            self.height,
            self.width * self.height * 4, // depth region inside the histogram
            0,
            self.height,
            volume,
            ema,
            Some(&output[self.output_front].1),
        );
        self.output = Some(output);
        self.output_front = back;
        self.output_has_history = true;
    }

    /// Region dispatch (exporters): shade full-width rows
    /// [tex_y0, tex_y0 + tex_height) of a full_width×full_height image.
    /// `depth_buffer` holds encoded depths at `depth_word_offset` (0 for
    /// a dedicated depth buffer); the input/output textures are
    /// region-sized.
    #[allow(clippy::too_many_arguments)]
    pub fn run_region(
        &self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        accumulation_view: &TextureView,
        depth_buffer: &Buffer,
        output_view: &TextureView,
        shading: &SolidShadingSettings,
        zoom: f32,
        rotation: f32,
        pan_x: f32,
        pan_y: f32,
        perspective_strength: f32,
        surface_thickness: f32,
        solid_strength: f32,
        full_width: u32,
        full_height: u32,
        depth_word_offset: u32,
        tex_y0: u32,
        tex_height: u32,
        volume: Option<&VolumeShadeInput>,
        temporal_ema: f32,
        prev_shade: Option<&TextureView>,
    ) {
        let mut lights = [ShadeLight {
            dir_intensity: [0.0; 4],
            color: [0.0; 4],
        }; 4];
        for (i, l) in shading.lights.iter().enumerate().take(4) {
            let az = l.azimuth.to_radians();
            let el = l.elevation.to_radians();
            // Camera-space direction TO the light: az = 0, el = 0 is a
            // headlight (from the viewer, +z); az swings around the
            // vertical axis, el tilts up.
            let dir = [
                el.cos() * az.sin(),
                el.sin(),
                el.cos() * az.cos(),
            ];
            let intensity = if l.enabled { l.intensity.max(0.0) } else { 0.0 };
            lights[i] = ShadeLight {
                dir_intensity: [dir[0], dir[1], dir[2], intensity],
                color: [l.color[0], l.color[1], l.color[2], 0.0],
            };
        }

        // Full-image normal path (normals pass + à-trous) is available when
        // the dispatch covers the whole image and the ping-pong textures
        // match it; the strip-tiled export path falls back to the inline
        // estimator in shade.wgsl.
        let smoothing = shading.normal_smoothing.min(3) as usize;
        let full_cover = tex_y0 == 0 && tex_height == full_height;
        let normal_view: Option<&TextureView> = match &self.normal_texs {
            Some((w, h, texs)) if full_cover && *w == full_width && *h == full_height => {
                Some(&texs[smoothing % 2].1) // final resting texture after ping-pong
            }
            _ => None,
        };

        // Derive the smoothed + closed half-res fields the shade shader
        // actually samples (see volume_mip.wgsl for why raw is unusable).
        // The buffers must already exist (`ensure_vol_derived`, called by
        // run()); a caller that skipped it gets the volume features
        // silently disabled rather than out-of-bounds sampling.
        let mut vol_avg_buffer: &Buffer = &self.dummy_volume;
        let mut vol_closed_buffer: &Buffer = &self.dummy_volume;
        let mut vol_color_buffer: &Buffer = &self.dummy_volume;
        let mut vol_raw_buffer: &Buffer = &self.dummy_volume;
        let mut vol_ready = false;
        if let Some(v) = volume {
            if v.dim > 0 {
                let hd = (v.dim / 2).max(1);
                let prepared = matches!(&self.vol_derived, Some((d, _)) if *d == v.dim);
                if !prepared {
                    log::warn!("ShadePass: derived volume buffers not prepared; volume shading skipped");
                }
                if prepared {
                vol_ready = true;
                let (_, bufs) = self.vol_derived.as_ref().unwrap();
                let (avg, vmax, tmp, closed, colorb) =
                    (&bufs[0], &bufs[1], &bufs[2], &bufs[3], &bufs[4]);
                let closing = v.closing.min(2);
                // 8× the solid threshold in raw counts: normalized scale
                // is dim³/(splats·8), so "normalized 8" = 64·splats/dim³.
                let clamp_raw = (64.0 * f64::from(v.splats.max(1.0))
                    / (v.dim as f64).powi(3)) as f32;
                queue.write_buffer(&self.mip_params, 0, bytemuck::bytes_of(&MipParams {
                    dim: v.dim,
                    half_dim: hd,
                    radius: closing.max(1),
                    clamp_raw: clamp_raw.max(1.0),
                }));
                let groups = hd.div_ceil(4);
                let mip_bg = |src: &Buffer, out_a: &Buffer, out_b: &Buffer, src_f: &Buffer, out_c: &Buffer, label: &str| {
                    device.create_bind_group(&BindGroupDescriptor {
                        label: Some(label),
                        layout: &self.mip_bgl,
                        entries: &[
                            BindGroupEntry { binding: 0, resource: src.as_entire_binding() },
                            BindGroupEntry { binding: 1, resource: self.mip_params.as_entire_binding() },
                            BindGroupEntry { binding: 2, resource: out_a.as_entire_binding() },
                            BindGroupEntry { binding: 3, resource: out_b.as_entire_binding() },
                            BindGroupEntry { binding: 4, resource: src_f.as_entire_binding() },
                            BindGroupEntry { binding: 5, resource: out_c.as_entire_binding() },
                        ],
                    })
                };
                // reduce: raw → avg + vmax (src_f slot needs a distinct,
                // unused buffer — tmp).
                let reduce_bg = mip_bg(v.buffer, avg, vmax, tmp, colorb, "Volume Reduce BG");
                // dilate: vmax → tmp; erode: tmp → closed. out_b and
                // out_c are written by neither entry point — bind
                // distinct scratch to satisfy aliasing rules.
                let dilate_bg = mip_bg(v.buffer, tmp, closed, vmax, colorb, "Volume Dilate BG");
                let erode_bg = mip_bg(v.buffer, closed, vmax, tmp, colorb, "Volume Erode BG");
                {
                    let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                        label: Some("Volume Derive Pass"),
                        timestamp_writes: None,
                    });
                    pass.set_pipeline(&self.mip_reduce_pipeline);
                    pass.set_bind_group(0, &reduce_bg, &[]);
                    pass.dispatch_workgroups(groups, groups, groups);
                    if closing > 0 {
                        pass.set_pipeline(&self.mip_dilate_pipeline);
                        pass.set_bind_group(0, &dilate_bg, &[]);
                        pass.dispatch_workgroups(groups, groups, groups);
                        pass.set_pipeline(&self.mip_erode_pipeline);
                        pass.set_bind_group(0, &erode_bg, &[]);
                        pass.dispatch_workgroups(groups, groups, groups);
                    }
                }
                vol_avg_buffer = avg;
                // Closing off: the march's σ source is the plain
                // half-res max field.
                vol_closed_buffer = if closing > 0 { closed } else { vmax };
                vol_color_buffer = colorb;
                vol_raw_buffer = v.buffer;
                }
            }
        }

        // Phase 2 volume inputs. Density normalizer: rho_norm = 1 marks
        // "solid" at 8× the uniform-spread per-voxel mean — concentrated
        // fractal surfaces sit far above it, Poisson noise far below.
        let (vol_dim, vol_extent, vol_scale, cam_rows, cam_pos, vol_center, vol_trust, vol_base_alpha) = match volume {
            Some(v) if v.dim > 0 && vol_ready => {
                let voxels = (v.dim as f64).powi(3);
                let scale = (voxels / (f64::from(v.splats.max(1.0)) * 8.0)) as f32;
                (
                    v.dim,
                    v.extent,
                    scale,
                    effective_camera_rows(v.camera_rotation_x, v.camera_rotation_y, v.camera_bank),
                    v.camera_pos,
                    v.center,
                    v.trust.clamp(0.0, 1.0),
                    v.base_alpha.max(1e-6),
                )
            }
            _ => (0, 0.0, 0.0, [[0.0; 3]; 3], [0.0; 3], [0.0; 3], 0.0, 1.0),
        };


        let params = ShadeParams {
            width: full_width,
            height: full_height,
            zoom,
            rotation,
            pan_x,
            pan_y,
            perspective_strength,
            shading_strength: shading.shading_strength,
            ambient: shading.ambient,
            diffuse: shading.diffuse,
            specular: shading.specular,
            shininess: shading.shininess,
            ssao_strength: shading.ssao_strength,
            ssao_radius: shading.ssao_radius,
            surface_thickness,
            depth_word_offset,
            tex_y0,
            tex_height,
            use_normal_tex: u32::from(normal_view.is_some()),
            // Surface closing requires the full-image normal texture.
            gap_fill: if normal_view.is_some() { shading.gap_fill.min(3) } else { 0 },
            cam_row0: [cam_rows[0][0], cam_rows[0][1], cam_rows[0][2], 0.0],
            cam_row1: [cam_rows[1][0], cam_rows[1][1], cam_rows[1][2], 0.0],
            cam_row2: [cam_rows[2][0], cam_rows[2][1], cam_rows[2][2], 0.0],
            cam_pos: [cam_pos[0], cam_pos[1], cam_pos[2], 0.0],
            vol_center: [vol_center[0], vol_center[1], vol_center[2], 0.0],
            volume_dim: vol_dim,
            volume_extent: vol_extent,
            vol_density_scale: vol_scale,
            // Coarse grids cast voxel-slab shadows — fade with trust.
            shadow_strength: shading.shadow_strength * vol_trust,
            vol_trust,
            temporal_ema: if prev_shade.is_some() { temporal_ema.clamp(0.0, 0.95) } else { 0.0 },
            solid_strength,
            base_alpha: vol_base_alpha,
            lights,
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Encode the normal-estimation + smoothing chain ahead of the shade
        // dispatch (same encoder, ordered).
        if normal_view.is_some() {
            let (_, _, texs) = self.normal_texs.as_ref().unwrap();
            let normals_bg = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Normals Bind Group"),
                layout: &self.normals_bgl,
                entries: &[
                    BindGroupEntry { binding: 0, resource: depth_buffer.as_entire_binding() },
                    BindGroupEntry { binding: 1, resource: self.params_buffer.as_entire_binding() },
                    BindGroupEntry { binding: 2, resource: BindingResource::TextureView(&texs[0].1) },
                ],
            });
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Solid Normals Pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.normals_pipeline);
                pass.set_bind_group(0, &normals_bg, &[]);
                pass.dispatch_workgroups(full_width.div_ceil(8), full_height.div_ceil(8), 1);
            }
            // σ_z tracks the depth-noise scale (the surface shell).
            let sigma_z = surface_thickness.max(0.005) * 2.0;
            for i in 0..smoothing {
                let ap = AtrousParams {
                    width: full_width,
                    height: full_height,
                    stride: 1u32 << i,
                    _pad0: 0,
                    sigma_z,
                    _pad1: 0.0,
                    _pad2: 0.0,
                    _pad3: 0.0,
                };
                queue.write_buffer(&self.atrous_params[i], 0, bytemuck::bytes_of(&ap));
                let src = i % 2;
                let dst = 1 - src;
                let bg = device.create_bind_group(&BindGroupDescriptor {
                    label: Some("Atrous Bind Group"),
                    layout: &self.atrous_bgl,
                    entries: &[
                        BindGroupEntry { binding: 0, resource: BindingResource::TextureView(&texs[src].1) },
                        BindGroupEntry { binding: 1, resource: self.atrous_params[i].as_entire_binding() },
                        BindGroupEntry { binding: 2, resource: BindingResource::TextureView(&texs[dst].1) },
                    ],
                });
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Solid Atrous Pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.atrous_pipeline);
                pass.set_bind_group(0, &bg, &[]);
                pass.dispatch_workgroups(full_width.div_ceil(8), full_height.div_ceil(8), 1);
            }
        }

        // The accumulator ping-pongs every frame, so the bind group is
        // per-dispatch (same pattern as tonemap_pass_with_input).
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Shade Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(accumulation_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: depth_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: self.params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(output_view),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::TextureView(
                        normal_view.unwrap_or(&self.dummy_normal.1),
                    ),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: vol_avg_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: vol_closed_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 7,
                    resource: BindingResource::TextureView(
                        prev_shade.unwrap_or(&self.dummy_normal.1),
                    ),
                },
                BindGroupEntry {
                    binding: 8,
                    resource: vol_color_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 9,
                    resource: vol_raw_buffer.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Solid Shade Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(full_width.div_ceil(8), tex_height.div_ceil(8), 1);
    }
}

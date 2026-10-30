//! **The average of several renders of one image**, in the accumulator's
//! `Rgba32Float` layout: the escape engine's antialiasing by accumulation
//! (several renders, each displaced within the pixel) and motion blur's
//! moments (docs/archive/projects/motion-blur.md), for every engine that renders
//! whole images.
//!
//! Each render is added pre-scaled by 1/n and weighted by its coverage
//! (straight alpha in and out), so the target holds the MEAN throughout --
//! it is complete only after the last add, its coverage building up. A
//! pair of write-only textures, ping-ponged: read-write storage access is
//! not supported for `rgba32float`, and a storage buffer would pass the
//! binding limit at export sizes.
//!
//! `present` copies a finished average aside, for a viewport to show while
//! the next one builds in the pair.

use egui_wgpu::wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoder, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, Extent3d, PipelineLayoutDescriptor, Queue,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StorageTextureAccess, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureView, TextureViewDescriptor, TextureViewDimension,
};

const SHADER: &str = r#"
struct AverageParams {
    inv_n: f32,
    clear: u32,
}

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var dst_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var<uniform> p: AverageParams;
@group(0) @binding(3) var prev_tex: texture_2d<f32>;

@compute @workgroup_size(8, 8, 1)
fn average_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = vec2<i32>(textureDimensions(dst_tex));
    let q = vec2<i32>(i32(gid.x), i32(gid.y));
    if (q.x >= dims.x || q.y >= dims.y) {
        return;
    }
    let s = textureLoad(src_tex, q, 0);
    let prev = select(textureLoad(prev_tex, q, 0), vec4<f32>(0.0), p.clear != 0u);
    // Straight alpha in and out: each render's colour counts by its
    // coverage, so a pixel one render did not draw keeps the colour of
    // those that did, at their share of the alpha. Averaging the colours
    // directly darkened every edge.
    let a = prev.a + s.a * p.inv_n;
    let c = prev.rgb * prev.a + s.rgb * (s.a * p.inv_n);
    textureStore(dst_tex, q, vec4<f32>(select(vec3<f32>(0.0), c / a, a > 0.0), a));
}
"#;

#[derive(Default)]
pub struct ImageAverage {
    /// The ping-pong pair, and which of it holds the running mean.
    pair: Option<(Texture, TextureView, Texture, TextureView)>,
    front: usize,
    /// Whether the next add is the first since `begin`: it clears.
    first: bool,
    /// The params of a first add and of every other, both written at
    /// `begin`, so any number of adds can be recorded in one submission
    /// (a write between them would land before all of them).
    params: Option<(Buffer, Buffer)>,
    pipeline: Option<(ComputePipeline, BindGroupLayout)>,
    /// A finished average kept aside (`present`).
    presented: Option<(Texture, TextureView)>,
}

fn target(device: &Device, label: &str, size: Extent3d, usage: TextureUsages) -> (Texture, TextureView) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba32Float,
        usage,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    (texture, view)
}

impl ImageAverage {
    /// Start an average of `count` renders of a `w` x `h` image.
    pub fn begin(&mut self, device: &Device, queue: &Queue, w: u32, h: u32, count: u32) {
        let (w, h) = (w.max(1), h.max(1));
        if self.pair.as_ref().is_none_or(|(t, _, _, _)| t.width() != w || t.height() != h) {
            self.release();
            let size = Extent3d { width: w, height: h, depth_or_array_layers: 1 };
            // Storage write from the add; read by the next add and the tail;
            // COPY_SRC for `present` and a test's read-back.
            let usage = TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC;
            let (ta, va) = target(device, "Image Average A", size, usage);
            let (tb, vb) = target(device, "Image Average B", size, usage);
            self.pair = Some((ta, va, tb, vb));
        }
        self.front = 0;
        self.first = true;
        let params = self.params.get_or_insert_with(|| {
            let make = |label| {
                device.create_buffer(&BufferDescriptor {
                    label: Some(label),
                    size: 16,
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            };
            (make("Image Average First"), make("Image Average Params"))
        });
        let inv = 1.0f32 / count.max(1) as f32;
        for (buffer, clear) in [(&params.0, 1u32), (&params.1, 0u32)] {
            let mut raw = [0u8; 16];
            raw[0..4].copy_from_slice(&inv.to_le_bytes());
            raw[4..8].copy_from_slice(&clear.to_le_bytes());
            queue.write_buffer(buffer, 0, &raw);
        }
    }

    /// Fold a render (`src`, the image's size) into the average.
    pub fn add(&mut self, device: &Device, encoder: &mut CommandEncoder, src: &TextureView) {
        let (Some((ta, va, _, vb)), Some((first, rest))) = (self.pair.as_ref(), self.params.as_ref()) else {
            return;
        };
        let (prev, next) = if self.front == 0 { (va, vb) } else { (vb, va) };
        let params = if self.first { first } else { rest };
        let (pipeline, layout) = self.pipeline.get_or_insert_with(|| {
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some("Image Average"),
                source: ShaderSource::Wgsl(SHADER.into()),
            });
            let texture = |binding| BindGroupLayoutEntry {
                binding,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: false },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            };
            let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Image Average Layout"),
                entries: &[
                    texture(0),
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::StorageTexture {
                            access: StorageTextureAccess::WriteOnly,
                            format: TextureFormat::Rgba32Float,
                            view_dimension: TextureViewDimension::D2,
                        },
                        count: None,
                    },
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
                    texture(3),
                ],
            });
            let pl = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Image Average Pipeline Layout"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Image Average"),
                layout: Some(&pl),
                module: &module,
                entry_point: Some("average_main"),
                compilation_options: Default::default(),
                cache: None,
            });
            (pipeline, layout)
        });
        let bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Image Average Bind Group"),
            layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: BindingResource::TextureView(src) },
                BindGroupEntry { binding: 1, resource: BindingResource::TextureView(next) },
                BindGroupEntry { binding: 2, resource: params.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: BindingResource::TextureView(prev) },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Image Average Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bg, &[]);
        pass.dispatch_workgroups(ta.width().div_ceil(8), ta.height().div_ceil(8), 1);
        drop(pass);
        self.front ^= 1;
        self.first = false;
    }

    /// The running mean; complete once every render is added.
    pub fn view(&self) -> Option<&TextureView> {
        self.pair.as_ref().map(|(_, va, _, vb)| if self.front == 0 { va } else { vb })
    }

    #[cfg(test)]
    pub(crate) fn texture(&self) -> Option<&Texture> {
        self.pair.as_ref().map(|(ta, _, tb, _)| if self.front == 0 { ta } else { tb })
    }

    /// Keep the finished average aside, for a viewport to show
    /// (`presented_view`) while the next builds in the pair.
    pub fn present(&mut self, device: &Device, encoder: &mut CommandEncoder) {
        let Some((ta, _, tb, _)) = self.pair.as_ref() else {
            return;
        };
        let src = if self.front == 0 { ta } else { tb }.clone();
        self.present_texture(device, encoder, &src);
    }

    /// Keep `src` aside instead, an image in the same layout: a finished
    /// single render, shown while the next renders over several frames.
    pub fn present_texture(&mut self, device: &Device, encoder: &mut CommandEncoder, src: &Texture) {
        let size = Extent3d { width: src.width(), height: src.height(), depth_or_array_layers: 1 };
        if self.presented.as_ref().is_none_or(|(t, _)| t.width() != size.width || t.height() != size.height) {
            if let Some((t, _)) = self.presented.take() {
                t.destroy();
            }
            // COPY_SRC so a test can read it back.
            let usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC;
            self.presented = Some(target(device, "Image Average Presented", size, usage));
        }
        let (dst, _) = self.presented.as_ref().expect("made above");
        encoder.copy_texture_to_texture(src.as_image_copy(), dst.as_image_copy(), size);
    }

    /// The average kept aside, while there is one.
    pub fn presented_view(&self) -> Option<&TextureView> {
        self.presented.as_ref().map(|(_, v)| v)
    }

    #[cfg(test)]
    pub(crate) fn presented_texture(&self) -> Option<&Texture> {
        self.presented.as_ref().map(|(t, _)| t)
    }

    /// Stop keeping an average aside; true if one was kept.
    pub fn drop_presented(&mut self) -> bool {
        match self.presented.take() {
            Some((t, _)) => {
                t.destroy();
                true
            }
            None => false,
        }
    }

    /// Drop the one kept aside if it is not `w` x `h`.
    pub fn fit_presented(&mut self, w: u32, h: u32) {
        if self.presented.as_ref().is_some_and(|(t, _)| t.width() != w || t.height() != h) {
            self.drop_presented();
        }
    }

    /// Free the pair (`begin` makes it again); the one kept aside stays.
    pub fn release(&mut self) {
        if let Some((a, _, b, _)) = self.pair.take() {
            a.destroy();
            b.destroy();
        }
    }

    /// Whether the pair is held.
    pub fn holds(&self) -> bool {
        self.pair.is_some()
    }

    /// The GPU memory held, in bytes.
    pub fn bytes(&self) -> u64 {
        let tex = |t: &Texture| t.width() as u64 * t.height() as u64 * 16;
        let mut n = self.params.as_ref().map_or(0, |(a, b)| a.size() + b.size());
        if let Some((a, _, b, _)) = &self.pair {
            n += tex(a) + tex(b);
        }
        if let Some((t, _)) = &self.presented {
            n += tex(t);
        }
        n
    }

    /// Free everything -- on WebGPU `Drop` frees nothing.
    pub fn destroy(&self) {
        for t in [self.pair.as_ref().map(|p| &p.0), self.pair.as_ref().map(|p| &p.2), self.presented.as_ref().map(|p| &p.0)].into_iter().flatten() {
            t.destroy();
        }
        if let Some((a, b)) = &self.params {
            a.destroy();
            b.destroy();
        }
    }
}

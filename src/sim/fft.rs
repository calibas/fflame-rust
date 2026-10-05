//! A 2D FFT on the GPU, for exact convolutions (mccabe-multiscale plan,
//! section 6b).
//!
//! Mixed-radix Stockham, out of place, one dispatch per radix stage per
//! axis, on `vec2<f32>` storage buffers in row-major order. The radices
//! are 4, 2, 3, 5 and, for any other prime factor, a general pass up to
//! [`MAX_RADIX`]; a length with a larger prime factor has no plan. In-app
//! grids follow the viewport, so lengths like 1920 and 1080 are the
//! normal case, not powers of two.
//!
//! Each stage of length `n` with radix `r` takes sub-transforms of length
//! `p` (1 at the first stage, multiplied by each radix in turn) and makes
//! ones of length `p r`: butterfly `i` reads `src[i + t n/r]` for
//! `t < r`, twiddles input `t` by `exp(s 2 pi i t k / (p r))` with
//! `k = i mod p`, takes an `r`-point DFT, and writes output `u` to
//! `dst[(i / p) p r + k + u p]`. After the last stage the output is in
//! natural order. `s` is -1 forward and +1 inverse; the inverse is not
//! scaled here -- a caller folds the `1/(w h)` into whatever it does to
//! the spectrum.

use wgpu::util::DeviceExt;
use wgpu::*;

/// The largest radix a pass handles. A larger prime factor has no plan.
pub const MAX_RADIX: u32 = 64;

/// The radices of a length, in the order its stages run, or `None` when
/// a prime factor is larger than [`MAX_RADIX`]. Fours first, then the
/// two left over, then three, five and any other prime.
pub fn radices(n: u32) -> Option<Vec<u32>> {
    if n == 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut m = n;
    while m % 4 == 0 {
        out.push(4);
        m /= 4;
    }
    if m % 2 == 0 {
        out.push(2);
        m /= 2;
    }
    let mut f = 3;
    while m > 1 {
        if f > MAX_RADIX {
            return None;
        }
        while m % f == 0 {
            out.push(f);
            m /= f;
        }
        f += 2;
    }
    Some(out)
}

/// One stage's uniform. Mirrored by `FftPass` in [`PASS_WGSL`].
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PassGpu {
    /// The axis's length.
    n: u32,
    /// The sub-transform length this stage starts from.
    p: u32,
    radix: u32,
    /// 0 transforms along rows (x), 1 along columns (y).
    axis: u32,
    width: u32,
    height: u32,
    /// -1 forward, +1 inverse.
    sign: f32,
    pad: u32,
}

const PASS_WGSL: &str = r#"
struct FftPass {
    n: u32,
    p: u32,
    radix: u32,
    axis: u32,
    width: u32,
    height: u32,
    sign: f32,
    pad: u32,
};

@group(0) @binding(0) var<uniform> stage: FftPass;
@group(0) @binding(1) var<storage, read> src: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read_write> dst: array<vec2<f32>>;

const FFT_TAU: f32 = 6.283185307179586;

fn cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

// i z, times the transform's sign.
fn rot(z: vec2<f32>) -> vec2<f32> {
    return stage.sign * vec2<f32>(-z.y, z.x);
}

// exp(sign * 2 pi i * num / den), the angle reduced to one turn first
// so the sine and cosine see an argument in [0, 2 pi).
fn twiddle(num: u32, den: u32) -> vec2<f32> {
    let a = stage.sign * FFT_TAU * f32(num % den) / f32(den);
    return vec2<f32>(cos(a), sin(a));
}

// Element e of line `line`: a row (y = line) or a column (x = line).
fn addr(line: u32, e: u32) -> u32 {
    if (stage.axis == 0u) {
        return line * stage.width + e;
    }
    return e * stage.width + line;
}

// Which butterfly and which line this invocation is. Along rows the
// butterflies run across x, so neighbouring invocations touch
// neighbouring elements; down columns that would put them a whole row
// apart, so there neighbouring invocations take neighbouring COLUMNS
// instead, and the reads coalesce either way.
fn place(gid: vec3<u32>) -> vec2<u32> {
    if (stage.axis == 0u) {
        return gid.xy;
    }
    return gid.yx;
}

fn lines() -> u32 {
    return select(stage.width, stage.height, stage.axis == 0u);
}

// Radices 2, 3, 4 and 5, written out: no arrays, so nothing spills.
@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let r = stage.radix;
    let m = stage.n / r;
    let at = place(gid);
    let i = at.x;
    let line = at.y;
    if (i >= m || line >= lines()) {
        return;
    }
    let p = stage.p;
    let k = i % p;
    let j = (i / p) * p * r + k;
    let pr = p * r;

    let x0 = src[addr(line, i)];
    let x1 = cmul(src[addr(line, i + m)], twiddle(k, pr));
    if (r == 2u) {
        dst[addr(line, j)] = x0 + x1;
        dst[addr(line, j + p)] = x0 - x1;
        return;
    }
    let x2 = cmul(src[addr(line, i + 2u * m)], twiddle(2u * k, pr));
    if (r == 3u) {
        // w = exp(s 2 pi i / 3) = -1/2 + s i sqrt(3)/2.
        let t1 = x1 + x2;
        let t2 = x0 - 0.5 * t1;
        let t3 = rot(0.8660254037844386 * (x1 - x2));
        dst[addr(line, j)] = x0 + t1;
        dst[addr(line, j + p)] = t2 + t3;
        dst[addr(line, j + 2u * p)] = t2 - t3;
        return;
    }
    let x3 = cmul(src[addr(line, i + 3u * m)], twiddle(3u * k, pr));
    if (r == 4u) {
        let ac0 = x0 + x2;
        let ac1 = x0 - x2;
        let bd0 = x1 + x3;
        let bd1 = rot(x1 - x3);
        dst[addr(line, j)] = ac0 + bd0;
        dst[addr(line, j + p)] = ac1 + bd1;
        dst[addr(line, j + 2u * p)] = ac0 - bd0;
        dst[addr(line, j + 3u * p)] = ac1 - bd1;
        return;
    }
    // r == 5. With c_k = cos(2 pi k / 5), s_k = sin(2 pi k / 5):
    // y1,4 = x0 + c1 t1 + c2 t2 +- i s (s1 t3 + s2 t4), and
    // y2,3 = x0 + c2 t1 + c1 t2 +- i s (s2 t3 - s1 t4).
    let x4 = cmul(src[addr(line, i + 4u * m)], twiddle(4u * k, pr));
    let c1 = 0.30901699437494745;
    let c2 = -0.8090169943749475;
    let s1 = 0.9510565162951535;
    let s2 = 0.5877852522924731;
    let t1 = x1 + x4;
    let t2 = x2 + x3;
    let t3 = x1 - x4;
    let t4 = x2 - x3;
    let a1 = x0 + c1 * t1 + c2 * t2;
    let a2 = x0 + c2 * t1 + c1 * t2;
    let b1 = rot(s1 * t3 + s2 * t4);
    let b2 = rot(s2 * t3 - s1 * t4);
    dst[addr(line, j)] = x0 + t1 + t2;
    dst[addr(line, j + p)] = a1 + b1;
    dst[addr(line, j + 2u * p)] = a2 + b2;
    dst[addr(line, j + 3u * p)] = a2 - b2;
    dst[addr(line, j + 4u * p)] = a1 - b1;
}

// Any other prime radix up to 64: twiddle, then an r-point DFT written
// out. Its own entry point, so its array's registers are not every
// stage's.
@compute @workgroup_size(64, 1, 1)
fn main_generic(@builtin(global_invocation_id) gid: vec3<u32>) {
    let r = stage.radix;
    let m = stage.n / r;
    let at = place(gid);
    let i = at.x;
    let line = at.y;
    if (i >= m || line >= lines()) {
        return;
    }
    let p = stage.p;
    let k = i % p;
    let j = (i / p) * p * r + k;
    var v: array<vec2<f32>, 64>;
    for (var t = 0u; t < r; t = t + 1u) {
        v[t] = cmul(src[addr(line, i + t * m)], twiddle(t * k, p * r));
    }
    for (var u = 0u; u < r; u = u + 1u) {
        var acc = vec2<f32>(0.0, 0.0);
        for (var t = 0u; t < r; t = t + 1u) {
            acc = acc + cmul(v[t], twiddle(t * u, r));
        }
        dst[addr(line, j + u * p)] = acc;
    }
}
"#;

/// A 2D FFT of one size, with the three complex buffers it works in.
///
/// Three, not two, because the spectral convolution keeps one forward
/// spectrum while several inverses ping-pong between the other two.
/// A transform runs from buffer `src` through `tmp` and ends in
/// whichever of the two the stage count leaves it in.
pub struct Fft2d {
    pub width: u32,
    pub height: u32,
    buffers: [Buffer; 3],
    pipeline: ComputePipeline,
    generic: ComputePipeline,
    /// `groups[a][b]` reads buffer `a` and writes buffer `b`.
    groups: Vec<Vec<Option<BindGroup>>>,
    /// Stage lists, forward then inverse: (dynamic offset, workgroups
    /// in x, in y, whether the radix needs the general pass).
    stages: [Vec<(u32, u32, u32, bool)>; 2],
}

impl Fft2d {
    /// A plan for `width` x `height`, or `None` when either length has a
    /// prime factor past [`MAX_RADIX`].
    pub fn new(device: &Device, width: u32, height: u32) -> Option<Self> {
        let rows = radices(width)?;
        let cols = radices(height)?;
        let align = device.limits().min_uniform_buffer_offset_alignment as usize;
        let stride = std::mem::size_of::<PassGpu>().div_ceil(align) * align;

        let mut bytes: Vec<u8> = Vec::new();
        let mut stages: [Vec<(u32, u32, u32, bool)>; 2] = [Vec::new(), Vec::new()];
        for (dir, sign) in [(0usize, -1.0f32), (1, 1.0)] {
            for (axis, (n, list, lines)) in [(width, &rows, height), (height, &cols, width)].into_iter().enumerate() {
                let mut p = 1u32;
                for &r in list.iter() {
                    let g = PassGpu { n, p, radix: r, axis: axis as u32, width, height, sign, pad: 0 };
                    let at = bytes.len();
                    bytes.resize(at + stride, 0);
                    bytes[at..at + std::mem::size_of::<PassGpu>()].copy_from_slice(bytemuck::bytes_of(&g));
                    // Rows: butterflies across x. Columns: lines across x,
                    // so neighbouring invocations read neighbouring
                    // addresses (see `place` in the shader).
                    let (gx, gy) = if axis == 0 { ((n / r).div_ceil(64), lines) } else { (lines.div_ceil(64), n / r) };
                    stages[dir].push((at as u32, gx, gy, !matches!(r, 2 | 3 | 4 | 5)));
                    p *= r;
                }
            }
        }
        let params = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Sim FFT Stages"),
            contents: &bytes,
            usage: BufferUsages::UNIFORM,
        });
        let size = (width as u64) * (height as u64) * 8;
        let buffers = [0, 1, 2].map(|k| {
            device.create_buffer(&BufferDescriptor {
                label: Some(["Sim FFT 0", "Sim FFT 1", "Sim FFT 2"][k]),
                size,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        });
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Sim FFT"),
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
                storage_entry(1, true),
                storage_entry(2, false),
            ],
        });
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Sim FFT"),
            source: ShaderSource::Wgsl(PASS_WGSL.into()),
        });
        let pl = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Sim FFT"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let make = |entry: &str| {
            device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Sim FFT"),
                layout: Some(&pl),
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let pipeline = make("main");
        let generic = make("main_generic");
        let groups = (0..3)
            .map(|a| {
                (0..3)
                    .map(|b| {
                        (a != b).then(|| {
                            device.create_bind_group(&BindGroupDescriptor {
                                label: Some("Sim FFT"),
                                layout: &layout,
                                entries: &[
                                    BindGroupEntry {
                                        binding: 0,
                                        resource: BindingResource::Buffer(BufferBinding {
                                            buffer: &params,
                                            offset: 0,
                                            size: std::num::NonZeroU64::new(std::mem::size_of::<PassGpu>() as u64),
                                        }),
                                    },
                                    BindGroupEntry { binding: 1, resource: buffers[a].as_entire_binding() },
                                    BindGroupEntry { binding: 2, resource: buffers[b].as_entire_binding() },
                                ],
                            })
                        })
                    })
                    .collect()
            })
            .collect();
        Some(Fft2d { width, height, buffers, pipeline, generic, groups, stages })
    }

    /// The complex buffer `k` (0, 1 or 2), `width * height` `vec2<f32>`.
    pub fn buffer(&self, k: usize) -> &Buffer {
        &self.buffers[k]
    }

    /// Record a 2D transform of buffer `src`, ping-ponging through `tmp`,
    /// into `pass`. Returns the buffer the result is in. `inverse` flips
    /// the sign; neither direction is scaled.
    pub fn encode(&self, pass: &mut ComputePass<'_>, inverse: bool, src: usize, tmp: usize) -> usize {
        let (mut a, mut b) = (src, tmp);
        for &(offset, gx, gy, generic) in &self.stages[usize::from(inverse)] {
            pass.set_pipeline(if generic { &self.generic } else { &self.pipeline });
            pass.set_bind_group(0, self.groups[a][b].as_ref().expect("a != b"), &[offset]);
            pass.dispatch_workgroups(gx, gy, 1);
            std::mem::swap(&mut a, &mut b);
        }
        a
    }

    /// Dispatches one 2D transform makes.
    pub fn stage_count(&self) -> usize {
        self.stages[0].len()
    }
}

fn storage_entry(binding: u32, read_only: bool) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::naga;

    #[test]
    fn radices_cover_the_length_and_refuse_large_primes() {
        for n in [1u32, 2, 4, 8, 12, 30, 48, 53, 360, 1080, 1920, 2048, 4096] {
            let r = radices(n).unwrap_or_else(|| panic!("{n} should have a plan"));
            assert_eq!(r.iter().product::<u32>(), n, "{n}: {r:?}");
            assert!(r.iter().all(|&x| x <= MAX_RADIX));
        }
        assert_eq!(radices(1920).unwrap(), vec![4, 4, 4, 2, 3, 5]);
        assert!(radices(67).is_none(), "67 is a prime past the largest radix");
        assert!(radices(2 * 1031).is_none());
    }

    #[test]
    fn the_stage_shader_validates() {
        let module = naga::front::wgsl::parse_str(PASS_WGSL).expect("parses");
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module)
            .expect("validates");
    }
}

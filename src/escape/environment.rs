//! HDRI environments (docs/archive/projects/hdri-environment.md): a sky photographed
//! all round, that lights a solid or a terrain, is what its reflections
//! show, and is the backdrop the camera sees.
//!
//! The skies are bundled (`assets/environments/`, Poly Haven, CC0) and named
//! by id, as palette packs are -- a config carries the id, never the image.
//! Each is an equirectangular Radiance `.hdr` (2:1): row 0 the zenith,
//! the middle column straight ahead at azimuth 0 ([`map_direction`]).
//!
//! **The sun is lifted out of the map** ([`EnvMap::sun`]). In the Sunflowers
//! sky it is nine texels of half a million that hold 47% of the light: a
//! bounce that does not aim at it finds it by chance, and half the light
//! would arrive as fireflies, while the lit tier, lighting by the map's
//! smooth irradiance, would draw no shadow from it. So the texels above
//! [`SUN_THRESHOLD`] times the sky's median, joined to the brightest,
//! become a directional light -- the rig's next free one, which both tiers
//! sample and shadow already (as Blender's EEVEE does with a world's "sun
//! threshold") -- and the map keeps the rest, the sun's place filled from
//! the texels around it.
//!
//! **The lit tier** traces no rays, so it reads the rest of the map
//! prefiltered: its diffuse light as nine spherical harmonics of the
//! irradiance ([`irradiance_sh`], Ramamoorthi and Hanrahan 2001), and a
//! coat's reflection from the texture's mips, level k the map seen through
//! a GGX lobe of roughness k / [`ENV_LODS`] ([`EnvMap::reflection_levels`],
//! Karis 2013's split sum, by filtered importance sampling). [`ENV_WGSL`]
//! reads both, and the map itself for the path tracer and the backdrop.

/// A bundled sky.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvDef {
    /// What a config names it by.
    pub id: &'static str,
    /// What the UI shows.
    pub name: &'static str,
    /// Its file in `assets/environments/`.
    pub file: &'static str,
    /// The display level (0 to 1, sRGB) the sky's median shows at, at Sky
    /// light 1: how bright the sky reads -- a photograph's own exposure is
    /// arbitrary (Poly Haven's night skies are not five times darker than
    /// its days, in the file).
    pub level: f32,
    /// The median luminance of the map's upper half, in its own units
    /// ([`EnvMap::median`], held here so settings need not decode it:
    /// `the_catalog_knows_each_skys_median`).
    pub median: f32,
    /// The credit owed (CC0 asks none; Poly Haven asks for a link); empty
    /// for none.
    pub credit: &'static str,
    pub url: &'static str,
    /// A night sky: choosing it in the panel brings the procedural stars
    /// out (`PathTraceConfig::environment_stars`).
    pub stars: bool,
}

/// The bundled skies, in the order the UI lists them.
pub const ENVIRONMENTS: &[EnvDef] = &[
    EnvDef {
        id: "sunflowers",
        name: "Sunflowers",
        file: "sunflowers_puresky_1k.hdr",
        level: 0.6,
        median: 0.280516,
        credit: "Sunflowers (Pure Sky), Poly Haven, CC0",
        url: "https://polyhaven.com/a/sunflowers_puresky",
        stars: false,
    },
    EnvDef {
        id: "rogland_night",
        name: "Rogland Clear Night",
        file: "rogland_clear_night_1k.hdr",
        level: 0.2,
        median: 0.29219,
        credit: "Rogland Clear Night, Poly Haven, CC0",
        url: "https://polyhaven.com/a/rogland_clear_night",
        stars: true,
    },
    // Stars alone: no photograph, a black sky the stars are drawn over
    // ([`load`] makes it).
    EnvDef {
        id: "starfield",
        name: "Starfield",
        file: "",
        level: 0.0,
        median: 1.0,
        credit: "",
        url: "",
        stars: true,
    },
];

/// The bundled sky called `id`.
pub fn get(id: &str) -> Option<&'static EnvDef> {
    ENVIRONMENTS.iter().find(|e| e.id == id)
}

/// How many times the sky's median a texel's luminance is, at least, to be
/// part of the sun. The Sunflowers sun's nine texels are 2 000 to 300 000
/// times its median, the glare around them under 800; the brightest of the
/// night sky's stars, 460.
pub const SUN_THRESHOLD: f32 = 1000.0;

/// The least share of the whole sky's light a sun holds to be lifted out:
/// less is a bright patch the bounces find well enough.
pub const SUN_MIN_SHARE: f32 = 0.1;

/// The sun lifted out of a map, in the map's frame (y up).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvSun {
    /// Toward it, a unit vector.
    pub dir: [f32; 3],
    /// What it lights a surface facing it with -- its radiance over its
    /// disc -- in the map's units, per channel.
    pub irradiance: [f32; 3],
    /// Its angular radius, radians: the disc its texels cover.
    pub radius: f32,
}

impl EnvSun {
    /// The sun as the rig's light: its intensity per channel (the
    /// irradiance over pi, `pt_light`), the map scaled by `scale`
    /// (`EnvSettings::scale`).
    pub fn power(&self, scale: f32) -> [f32; 3] {
        self.irradiance.map(|v| v * scale / std::f32::consts::PI)
    }
}

/// A sky decoded, its sun lifted out.
#[derive(Debug, Clone)]
pub struct EnvMap {
    pub width: u32,
    pub height: u32,
    /// Linear radiance, row-major from the zenith: the sun's place filled.
    pub texels: Vec<[f32; 3]>,
    pub sun: Option<EnvSun>,
    /// The median luminance of the upper half: what [`EnvDef::level`]
    /// scales.
    pub median: f32,
}

fn lum(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// The direction (in the map's frame: x right, y up, -z ahead) at a texel
/// centre's `u`, `v` (0 to 1): azimuth 2 pi (u - 1/2), the polar angle pi v
/// from the zenith. The shaders invert it (`pt_env`).
pub fn map_direction(u: f32, v: f32) -> [f32; 3] {
    let phi = std::f32::consts::TAU * (u - 0.5);
    let theta = std::f32::consts::PI * v;
    [theta.sin() * phi.sin(), theta.cos(), -theta.sin() * phi.cos()]
}

/// A texel's solid angle in a `w` by `h` map, at row `y`.
fn texel_solid_angle(y: u32, w: u32, h: u32) -> f32 {
    let theta = std::f32::consts::PI * (y as f32 + 0.5) / h as f32;
    theta.sin() * (std::f32::consts::PI / h as f32) * (std::f32::consts::TAU / w as f32)
}

impl EnvMap {
    /// A Radiance `.hdr`, decoded, its sun lifted out.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        Self::decode_at(bytes, SUN_THRESHOLD)
    }

    /// [`decode`](Self::decode), the sun's texels those above `threshold`
    /// times the median.
    pub fn decode_at(bytes: &[u8], threshold: f32) -> Result<Self, String> {
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Hdr).map_err(|e| format!("not a Radiance HDR: {e}"))?;
        let img = img.into_rgb32f();
        let (width, height) = img.dimensions();
        if width < 4 || height < 2 {
            return Err(format!("an environment of {width}x{height} is too small"));
        }
        let texels: Vec<[f32; 3]> = img.pixels().map(|p| [p.0[0].max(0.0), p.0[1].max(0.0), p.0[2].max(0.0)]).map(|c| if c.iter().all(|v| v.is_finite()) { c } else { [0.0; 3] }).collect();
        let mut map = EnvMap { width, height, texels, sun: None, median: 0.0 };
        map.median = map.upper_median();
        map.sun = map.lift_sun(threshold);
        Ok(map)
    }

    fn upper_median(&self) -> f32 {
        let mut l: Vec<f32> = self.texels[..(self.width * self.height / 2) as usize].iter().map(|&c| lum(c)).collect();
        if l.is_empty() {
            return 0.0;
        }
        let mid = l.len() / 2;
        l.select_nth_unstable_by(mid, |a, b| a.total_cmp(b));
        l[mid]
    }

    /// The sun: the texels above [`SUN_THRESHOLD`] times the median joined
    /// to the brightest (8-connected, across the seam), when they hold at
    /// least [`SUN_MIN_SHARE`] of the light. Their light above the texels
    /// around them becomes the sun's; they keep that surround.
    fn lift_sun(&mut self, threshold: f32) -> Option<EnvSun> {
        let (w, h) = (self.width, self.height);
        let at = |x: u32, y: u32| (y * w + x) as usize;
        let threshold = threshold * self.median.max(1.0e-12);
        let (peak, _) = self.texels.iter().enumerate().map(|(i, &c)| (i, lum(c))).max_by(|a, b| a.1.total_cmp(&b.1))?;
        if lum(self.texels[peak]) <= threshold {
            return None;
        }
        // The region, by flood from the peak.
        let mut inside = vec![false; self.texels.len()];
        let mut stack = vec![(peak as u32 % w, peak as u32 / w)];
        inside[peak] = true;
        let mut region = Vec::new();
        while let Some((x, y)) = stack.pop() {
            region.push((x, y));
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let ny = y as i32 + dy;
                    if ny < 0 || ny >= h as i32 {
                        continue;
                    }
                    let nx = (x as i32 + dx).rem_euclid(w as i32) as u32;
                    let i = at(nx, ny as u32);
                    if !inside[i] && lum(self.texels[i]) > threshold {
                        inside[i] = true;
                        stack.push((nx, ny as u32));
                    }
                }
            }
        }
        // The surround: the region's neighbours outside it, averaged.
        let (mut ring, mut ring_n) = ([0.0f64; 3], 0.0f64);
        for &(x, y) in &region {
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let ny = y as i32 + dy;
                    if ny < 0 || ny >= h as i32 {
                        continue;
                    }
                    let i = at((x as i32 + dx).rem_euclid(w as i32) as u32, ny as u32);
                    if !inside[i] {
                        for k in 0..3 {
                            ring[k] += self.texels[i][k] as f64;
                        }
                        ring_n += 1.0;
                    }
                }
            }
        }
        let fill = ring.map(|v| (v / ring_n.max(1.0)) as f32);
        // Its light above the surround, and where it comes from.
        let mut total = 0.0f64;
        for y in 0..h {
            let sa = texel_solid_angle(y, w, h) as f64;
            for x in 0..w {
                total += lum(self.texels[at(x, y)]) as f64 * sa;
            }
        }
        let (mut e, mut dir, mut omega) = ([0.0f64; 3], [0.0f64; 3], 0.0f64);
        for &(x, y) in &region {
            let sa = texel_solid_angle(y, w, h) as f64;
            let c = self.texels[at(x, y)];
            let extra: [f64; 3] = [0, 1, 2].map(|k| (c[k] - fill[k]).max(0.0) as f64 * sa);
            let d = map_direction((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
            let weight = 0.2126 * extra[0] + 0.7152 * extra[1] + 0.0722 * extra[2];
            for k in 0..3 {
                e[k] += extra[k];
                dir[k] += d[k] as f64 * weight;
            }
            omega += sa;
        }
        let share = (0.2126 * e[0] + 0.7152 * e[1] + 0.0722 * e[2]) / total.max(1.0e-30);
        if share < SUN_MIN_SHARE as f64 {
            return None;
        }
        for &(x, y) in &region {
            self.texels[at(x, y)] = fill;
        }
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt().max(1.0e-30);
        Some(EnvSun {
            dir: dir.map(|v| (v / len) as f32),
            irradiance: e.map(|v| v as f32),
            radius: (omega / std::f64::consts::PI).sqrt() as f32,
        })
    }

    /// The light the map holds, luminance by solid angle (the sun's not
    /// counted when lifted out).
    pub fn light(&self) -> f32 {
        let mut total = 0.0f64;
        for y in 0..self.height {
            let sa = texel_solid_angle(y, self.width, self.height) as f64;
            for x in 0..self.width {
                total += lum(self.texels[(y * self.width + x) as usize]) as f64 * sa;
            }
        }
        total as f32
    }
}

/// A sky on the GPU: its map, filterable (`rgba16float`), its reflection
/// levels in the mips, and its sun; and the map small, for the
/// irradiance's harmonics, which the sky's rotation and horizon move.
pub struct EnvGpu {
    pub id: &'static str,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sun: Option<EnvSun>,
    /// The reflection levels its mips hold: [`ENV_LODS`], or fewer for a
    /// map too small for them.
    pub lods: u32,
    small: EnvLevel,
    sh: std::sync::Mutex<Option<([u32; 2], [[f32; 3]; 9])>>,
}

impl EnvGpu {
    /// `map`, uploaded with its reflection levels.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, id: &'static str, map: &EnvMap) -> Self {
        let mut levels = vec![map.level()];
        levels.extend(map.reflection_levels());
        // A texture's mips stop at 1x1.
        levels.truncate((32 - map.width.max(map.height).leading_zeros()) as usize);
        let lods = levels.len() as u32 - 1;
        let (texture, view) = env_texture(device, queue, &levels);
        EnvGpu { id, texture, view, sun: map.sun, lods, small: map.small(), sh: std::sync::Mutex::new(None) }
    }

    /// The irradiance's harmonics with the sky turned by `rotation` (turns)
    /// and its horizon at `horizon` (radians): [`irradiance_sh`], kept for
    /// the last pair asked.
    pub fn irradiance(&self, rotation: f32, horizon: f32) -> [[f32; 3]; 9] {
        let key = [rotation.to_bits(), horizon.to_bits()];
        let mut memo = self.sh.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((k, sh)) = *memo {
            if k == key {
                return sh;
            }
        }
        let sh = irradiance_sh(&self.small, rotation, horizon);
        *memo = Some((key, sh));
        sh
    }
}

/// An `rgba16float` texture of `levels`, each a mip: each half the one
/// before.
pub fn env_texture(device: &wgpu::Device, queue: &wgpu::Queue, levels: &[EnvLevel]) -> (wgpu::Texture, wgpu::TextureView) {
    let (w, h) = (levels[0].width, levels[0].height);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Environment"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (k, l) in levels.iter().enumerate() {
        debug_assert_eq!((l.width, l.height), ((w >> k).max(1), (h >> k).max(1)));
        let data: Vec<u16> = l.texels.iter().flat_map(|c| [c[0], c[1], c[2], 1.0]).map(|v| half::f16::from_f32(v.min(65000.0)).to_bits()).collect();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: k as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            bytemuck::cast_slice(&data),
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(l.width * 8), rows_per_image: Some(l.height) },
            wgpu::Extent3d { width: l.width, height: l.height, depth_or_array_layers: 1 },
        );
    }
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// A 1x1 map of nothing, for a binding no pass reads (wgpu zeroes a new
/// texture).
pub fn env_stand_in(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Environment (none)"),
        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// The sampler a map is read with: around in azimuth, clamped at the
/// poles, linear -- between the reflection levels too.
pub fn env_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Environment"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    })
}

/// The world's elevation (radians) a map's elevation `em` shows at, the
/// map's horizon raised to `horizon`: above it the sky stretched from
/// `[0, pi/2]` to `[horizon, pi/2]`, below likewise -- the zenith and the
/// nadir fixed. `pt_env` reads the map by the inverse.
pub fn warp_elevation(em: f32, horizon: f32) -> f32 {
    let half_pi = std::f32::consts::FRAC_PI_2;
    if em >= 0.0 {
        horizon + em * (half_pi - horizon) / half_pi
    } else {
        horizon + em * (half_pi + horizon) / half_pi
    }
}

/// A sun in the map's frame, its horizon raised to `horizon` (radians:
/// [`warp_elevation`]), the azimuth kept.
pub fn raised(dir: [f32; 3], horizon: f32) -> [f32; 3] {
    let em = dir[1].clamp(-1.0, 1.0).asin();
    let e = warp_elevation(em, horizon);
    let flat = (dir[0] * dir[0] + dir[2] * dir[2]).sqrt();
    let (s, c) = e.sin_cos();
    if flat <= 1.0e-12 {
        return [0.0, s.signum(), 0.0];
    }
    [dir[0] / flat * c, s, dir[2] / flat * c]
}

/// A sun in the map's frame, turned by `rotation` (turns, about y: the
/// map's azimuth grows by it, as `pt_env` reads it).
pub fn turned(dir: [f32; 3], rotation: f32) -> [f32; 3] {
    let a = std::f32::consts::TAU * rotation;
    let (s, c) = a.sin_cos();
    // Azimuth from -z toward +x: (x, -z) turns by a.
    [dir[0] * c - dir[2] * s, dir[1], dir[2] * c + dir[0] * s]
}

/// The reflection levels past the map: level k of the texture's mips is
/// the sky seen through a GGX lobe of roughness k / `ENV_LODS` (its alpha
/// the square), level 0 the map. The lit tier reads a coat of roughness r
/// at level r times this (`ifs_env_reflect`).
pub const ENV_LODS: u32 = 5;

/// One level of a map: linear radiance, row-major from the zenith.
#[derive(Debug, Clone)]
pub struct EnvLevel {
    pub width: u32,
    pub height: u32,
    pub texels: Vec<[f32; 3]>,
}

impl EnvLevel {
    /// Read at `(u, v)` as the sampler reads: linear, around in u,
    /// clamped in v.
    pub fn bilinear(&self, u: f32, v: f32) -> [f32; 3] {
        let (w, h) = (self.width as f32, self.height as f32);
        let x = u * w - 0.5;
        let y = (v * h - 0.5).clamp(0.0, h - 1.0);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let col = |k: f32| (k as i64).rem_euclid(self.width as i64) as u32;
        let (xa, xb) = (col(x0), col(x0 + 1.0));
        let ya = y0 as u32;
        let yb = (ya + 1).min(self.height - 1);
        let at = |x: u32, y: u32| self.texels[(y * self.width + x) as usize];
        let (a, b, c, d) = (at(xa, ya), at(xb, ya), at(xa, yb), at(xb, yb));
        [0, 1, 2].map(|k| (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy)
    }

    /// Half the size, each texel the mean of four by their solid angles:
    /// a row nearer a pole covers less of the sky, and an even mean
    /// over-weighted the poles -- 3.5% of the Sunflowers sky's light lost
    /// by its fourth reflection level.
    fn halved(&self) -> EnvLevel {
        let (w, h) = ((self.width / 2).max(1), (self.height / 2).max(1));
        let row = |y: u32| y.min(self.height - 1);
        let at = |x: u32, y: u32| self.texels[(row(y) * self.width + x.min(self.width - 1)) as usize];
        let texels = (0..w * h)
            .map(|i| {
                let (x, y) = (2 * (i % w), 2 * (i / w));
                let (sa, sb) = (texel_solid_angle(row(y), self.width, self.height), texel_solid_angle(row(y + 1), self.width, self.height));
                let q = [at(x, y), at(x + 1, y), at(x, y + 1), at(x + 1, y + 1)];
                [0, 1, 2].map(|k| ((q[0][k] + q[1][k]) * sa + (q[2][k] + q[3][k]) * sb) / (2.0 * (sa + sb)).max(1.0e-30))
            })
            .collect();
        EnvLevel { width: w, height: h, texels }
    }
}

/// A direction's `(u, v)` in a map ([`map_direction`] inverted).
pub fn map_uv(d: [f32; 3]) -> (f32, f32) {
    let u = (d[0].atan2(-d[2]) / std::f32::consts::TAU + 0.5).rem_euclid(1.0);
    (u, d[1].clamp(-1.0, 1.0).acos() / std::f32::consts::PI)
}

/// Where in the map a direction `s` of the sky's frame (x right, y up, -z
/// ahead) reads, the sky turned by `rotation` (turns) and its horizon at
/// `horizon` (radians): `ifs_env_uv`, in Rust.
pub fn env_uv(s: [f32; 3], rotation: f32, horizon: f32) -> (f32, f32) {
    let half_pi = std::f32::consts::FRAC_PI_2;
    let u = s[0].atan2(-s[2]) / std::f32::consts::TAU + 0.5 - rotation;
    let e = s[1].clamp(-1.0, 1.0).asin() - horizon;
    let em = e * half_pi / if e >= 0.0 { half_pi - horizon } else { half_pi + horizon };
    (u.rem_euclid(1.0), 0.5 - em / std::f32::consts::PI)
}

/// An orthonormal pair about `n`.
fn basis(n: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let up = if n[1].abs() < 0.999 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let t = [up[1] * n[2] - up[2] * n[1], up[2] * n[0] - up[0] * n[2], up[0] * n[1] - up[1] * n[0]];
    let l = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
    let t = t.map(|v| v / l);
    let b = [n[1] * t[2] - n[2] * t[1], n[2] * t[0] - n[0] * t[2], n[0] * t[1] - n[1] * t[0]];
    (t, b)
}

/// An ordered map over `0..n`, in parallel off the web (as `bla`'s).
#[cfg(not(target_arch = "wasm32"))]
fn par_map<T: Send>(n: usize, f: impl Fn(usize) -> T + Sync + Send) -> Vec<T> {
    use rayon::prelude::*;
    (0..n).into_par_iter().map(f).collect()
}

#[cfg(target_arch = "wasm32")]
fn par_map<T>(n: usize, f: impl Fn(usize) -> T) -> Vec<T> {
    (0..n).map(f).collect()
}

/// The samples a reflection level takes of each texel's lobe: fewer
/// where the lobe is narrow and its texels many.
const LEVEL_SAMPLES: [u32; ENV_LODS as usize + 1] = [1, 48, 64, 96, 128, 128];

impl EnvMap {
    /// The map as a level.
    pub fn level(&self) -> EnvLevel {
        EnvLevel { width: self.width, height: self.height, texels: self.texels.clone() }
    }

    /// The map halved down to its smallest: `[0]` the map.
    fn pyramid(&self) -> Vec<EnvLevel> {
        let mut out = vec![self.level()];
        while out.last().is_some_and(|l| l.width > 4 && l.height > 2) {
            let next = out.last().expect("one").halved();
            out.push(next);
        }
        out
    }

    /// The map 128 wide (or its own width below that): what the
    /// irradiance's harmonics are taken over, smooth as they are.
    fn small(&self) -> EnvLevel {
        let mut l = self.level();
        while l.width > 128 && l.height > 2 {
            l = l.halved();
        }
        l
    }

    /// The reflection levels 1 to [`ENV_LODS`], each half the one before
    /// (the texture's mips): level k is the map seen through a GGX lobe of
    /// roughness k / `ENV_LODS`, the normal, the view and the reflection
    /// one (Karis 2013's split sum) -- each texel the lobe's directions
    /// drawn by a fixed Hammersley set, weighted by their cosine, each read
    /// from the level of the map's pyramid whose texel is the sample's
    /// share of the lobe (Krivanek and Colbert 2008's filtered importance
    /// sampling), so a few dozen samples leave no grain.
    pub fn reflection_levels(&self) -> Vec<EnvLevel> {
        let pyramid = self.pyramid();
        let last = (pyramid.len() - 1) as f32;
        let texel = 4.0 * std::f32::consts::PI / (self.width as f32 * self.height as f32);
        let read = |d: [f32; 3], lod: f32| {
            let lod = lod.clamp(0.0, last);
            let (u, v) = map_uv(d);
            let k = lod.floor() as usize;
            let a = pyramid[k].bilinear(u, v);
            if k as f32 >= last {
                return a;
            }
            let f = lod - k as f32;
            let b = pyramid[k + 1].bilinear(u, v);
            [0, 1, 2].map(|c| a[c] * (1.0 - f) + b[c] * f)
        };
        (1..=ENV_LODS)
            .map(|k| {
                let (w, h) = ((self.width >> k).max(1), (self.height >> k).max(1));
                let rough = k as f32 / ENV_LODS as f32;
                let a2 = (rough * rough) * (rough * rough);
                let m = LEVEL_SAMPLES[k as usize];
                // The lobe's samples about +z: the direction, its cosine,
                // and the pyramid's level for its share.
                let set: Vec<([f32; 3], f32, f32)> = (0..m)
                    .filter_map(|i| {
                        let u1 = (i as f32 + 0.5) / m as f32;
                        let u2 = (i.reverse_bits() as f64 / 4294967296.0) as f32;
                        let cos_h = ((1.0 - u1) / (1.0 + (a2 - 1.0) * u1)).sqrt();
                        let sin_h = (1.0 - cos_h * cos_h).max(0.0).sqrt();
                        let phi = std::f32::consts::TAU * u2;
                        let nl = 2.0 * cos_h * cos_h - 1.0;
                        if nl <= 0.0 {
                            return None;
                        }
                        let l = [2.0 * cos_h * sin_h * phi.cos(), 2.0 * cos_h * sin_h * phi.sin(), nl];
                        let q = cos_h * cos_h * (a2 - 1.0) + 1.0;
                        let pdf = a2 / (std::f32::consts::PI * q * q) / 4.0;
                        let lod = (0.5 * (1.0 / (m as f32 * pdf * texel)).log2() + 1.0).max(0.0);
                        Some((l, nl, lod))
                    })
                    .collect();
                let texels = par_map((w * h) as usize, |i| {
                    let (x, y) = (i as u32 % w, i as u32 / w);
                    let n = map_direction((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32);
                    let (t, b) = basis(n);
                    let (mut sum, mut weight) = ([0.0f32; 3], 0.0f32);
                    for &(l, nl, lod) in &set {
                        let d = [0, 1, 2].map(|c| t[c] * l[0] + b[c] * l[1] + n[c] * l[2]);
                        let r = read(d, lod);
                        for c in 0..3 {
                            sum[c] += r[c] * nl;
                        }
                        weight += nl;
                    }
                    sum.map(|v| v / weight.max(1.0e-30))
                });
                EnvLevel { width: w, height: h, texels }
            })
            .collect()
    }
}

/// The real spherical harmonics to band 2 at a unit direction `s`.
pub fn sh9(s: [f32; 3]) -> [f32; 9] {
    let [x, y, z] = s;
    [
        0.282095,
        0.488603 * y,
        0.488603 * z,
        0.488603 * x,
        1.092548 * x * y,
        1.092548 * y * z,
        0.315392 * (3.0 * z * z - 1.0),
        1.092548 * x * z,
        0.546274 * (x * x - y * y),
    ]
}

/// The irradiance over pi a surface facing `s` (the sky's frame) receives
/// from `map`, the sky turned by `rotation` (turns) and its horizon at
/// `horizon` (radians), as nine coefficients per channel of [`sh9`]: the
/// radiance projected, each band scaled by the cosine lobe's (pi, 2 pi /
/// 3, pi / 4; Ramamoorthi and Hanrahan 2001) over pi. A white Lambertian
/// surface's radiance, in the map's units; `ifs_env_irradiance` sums it.
pub fn irradiance_sh(map: &EnvLevel, rotation: f32, horizon: f32) -> [[f32; 3]; 9] {
    // Over the sky's own directions, 128 by 64, each its solid angle.
    let (gw, gh) = (128u32, 64u32);
    let mut c = [[0.0f64; 3]; 9];
    for gy in 0..gh {
        let sa = texel_solid_angle(gy, gw, gh) as f64;
        for gx in 0..gw {
            let s = map_direction((gx as f32 + 0.5) / gw as f32, (gy as f32 + 0.5) / gh as f32);
            let (u, v) = env_uv(s, rotation, horizon);
            let l = map.bilinear(u, v);
            for (k, y) in sh9(s).into_iter().enumerate() {
                for ch in 0..3 {
                    c[k][ch] += l[ch] as f64 * y as f64 * sa;
                }
            }
        }
    }
    let band = [1.0, 2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 0.25, 0.25, 0.25, 0.25, 0.25];
    let mut out = [[0.0f32; 3]; 9];
    for k in 0..9 {
        for ch in 0..3 {
            out[k][ch] = (c[k][ch] * band[k]) as f32;
        }
    }
    out
}

/// The sky's slots in a pass's `fdata` (both mode D's and a terrain's):
/// from [`ENV_FDATA`], [`ENV_FDATA_LEN`] of them -- what [`ENV_WGSL`] reads.
pub const ENV_FDATA: usize = 36;
pub const ENV_FDATA_LEN: usize = 9;

/// The sky's slots: [36] 1, its turn (turns), the accumulator's units a
/// map unit, and 1 when the camera sees it; [37] its horizon's elevation
/// (radians), the reflection levels its mips hold (`EnvGpu::lods`), the
/// stars' brightness in the accumulator's units (0, none), and 0;
/// [38..45] the irradiance's harmonics ([`irradiance_sh`]), 27 floats in
/// order.
pub fn env_fdata(gpu: &EnvGpu, set: &crate::escape::path_core::EnvSettings) -> [[f32; 4]; ENV_FDATA_LEN] {
    let mut out = [[0.0f32; 4]; ENV_FDATA_LEN];
    let sh = gpu.irradiance(set.rotation, set.horizon);
    out[0] = [1.0, set.rotation, set.scale, if set.backdrop { 1.0 } else { 0.0 }];
    out[1] = [set.horizon, gpu.lods as f32, set.stars.max(0.0), 0.0];
    for (k, v) in sh.as_flattened().iter().enumerate() {
        out[2 + k / 4][k % 4] = *v;
    }
    out
}

/// The sky as every pass that lights with the rig reads it (spliced into
/// the rig, `assembler::ifs_rig`): its map and reflection levels at group
/// 0's bindings 14 and 15 -- a 1x1 stand-in when there is none -- and its
/// slots of `params.fdata` ([`env_fdata`]). A pass that never reads it
/// leaves the bindings out of its layout.
pub const ENV_WGSL: &str = r#"
// The HDRI sky (escape::environment): its map, and in the mips the map
// seen through GGX lobes of growing roughness.
@group(0) @binding(14) var ifs_env_tex: texture_2d<f32>;
@group(0) @binding(15) var ifs_env_smp: sampler;

fn ifs_env_on() -> bool { return params.fdata[36].x > 0.5; }
fn ifs_env_backdrop() -> bool { return params.fdata[36].w > 0.5; }

// atan2, exact at a zero pair as IEEE has it (Metal's is not: CLAUDE.md),
// for the map's azimuth -- straight up and down are a zero pair.
fn ifs_env_atan2(y: f32, x: f32) -> f32 {
    if (y == 0.0 && x == 0.0) {
        let pi = 3.14159265358979;
        let mag = select(0.0, pi, (bitcast<u32>(x) & 0x80000000u) != 0u);
        return select(mag, -mag, (bitcast<u32>(y) & 0x80000000u) != 0u);
    }
    return atan2(y, x);
}

// A world direction in the sky's frame (x right, y up, -z ahead): the
// world's, z up, whatever the lights follow -- a sky read in the studio's
// frame (a camera's, at the solids' usual view) stood its horizon at the
// camera's pitch, level only from that view's yaw.
fn ifs_env_dir(d: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(d.x, d.z, -d.y);
}

// Where in the map `s` (the sky's frame) reads: `environment::map_uv`
// turned by fdata[36].y, the horizon raised to fdata[37].x -- each side
// of it stretched to fit (`environment::warp_elevation`).
fn ifs_env_uv(s: vec3<f32>) -> vec2<f32> {
    let u = ifs_env_atan2(s.x, -s.z) / 6.283185307 + 0.5 - params.fdata[36].y;
    let half_pi = 1.570796327;
    let h = params.fdata[37].x;
    let e = asin(clamp(s.y, -1.0, 1.0)) - h;
    let em = e * half_pi / select(half_pi + h, half_pi - h, e >= 0.0);
    return vec2<f32>(u, 0.5 - em / 3.141592654);
}

// The sky's radiance toward `s` (its frame) at reflection level `lod`, in
// the accumulator's units, its stars over it. An explicit level, so from
// any control flow.
fn ifs_env(s: vec3<f32>, lod: f32) -> vec3<f32> {
    let uv = ifs_env_uv(s);
    return textureSampleLevel(ifs_env_tex, ifs_env_smp, uv, lod).rgb * params.fdata[36].z + ifs_env_stars(uv, lod);
}

// Three uniform numbers from a cell (Jarzynski and Olano 2020's pcg3d).
fn ifs_env_hash(c: vec3<u32>) -> vec3<f32> {
    var v = c * 1664525u + vec3<u32>(1013904223u);
    v.x = v.x + v.y * v.z;
    v.y = v.y + v.z * v.x;
    v.z = v.z + v.x * v.y;
    v = v ^ (v >> vec3<u32>(16u));
    v.x = v.x + v.y * v.z;
    v.y = v.y + v.z * v.x;
    v.z = v.z + v.x * v.y;
    return vec3<f32>(v >> vec3<u32>(8u)) / 16777216.0;
}

// The procedural stars (fdata[37].z their brightness in the accumulator's
// units, 0 for none) at the map's `uv`: fixed to the map, so they turn
// and rise with it, and only above its horizon. The map's sphere in cells
// -- a cube's faces, 160 by 160 -- a star in about one cell in eleven,
// placed in it at random; its brightness by a power law (most faint, a
// few at the full), its colour from orange through white to blue-white.
// A star is a pixel across at any zoom (its radius 0.6 of the camera's
// pixel, as a camera ray sees it; at most about a cell), and as bright:
// sharp, and too faint to light anything. A rough reflection (`lod` past
// half a level) shows none.
fn ifs_env_stars(uv: vec2<f32>, lod: f32) -> vec3<f32> {
    let k = params.fdata[37].z;
    if (!(k > 0.0) || lod >= 0.5) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    let phi = 6.283185307 * (fract(uv.x) - 0.5);
    let theta = 3.141592654 * clamp(uv.y, 0.0, 1.0);
    let m = vec3<f32>(sin(theta) * sin(phi), cos(theta), -sin(theta) * cos(phi));
    if (m.y <= 0.0) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    let a = abs(m);
    var face = 0u;
    var st = vec2<f32>(0.0, 0.0);
    if (a.x >= a.y && a.x >= a.z) {
        face = select(1u, 0u, m.x > 0.0);
        st = vec2<f32>(m.y, m.z) / a.x;
    } else if (a.y >= a.z) {
        face = select(3u, 2u, m.y > 0.0);
        st = vec2<f32>(m.x, m.z) / a.y;
    } else {
        face = select(5u, 4u, m.z > 0.0);
        st = vec2<f32>(m.x, m.y) / a.z;
    }
    let n = 160.0;
    let here = min(floor((st * 0.5 + vec2<f32>(0.5, 0.5)) * n), vec2<f32>(n - 1.0, n - 1.0));
    let side = select(-1.0, 1.0, face % 2u == 0u);
    let pixel = 2.0 * tan(ifs_fov() * 0.5) / f32(max(params.height, 1u));
    let r = clamp(0.6 * pixel, 2.0e-5, 0.01);
    var out = vec3<f32>(0.0, 0.0, 0.0);
    // This cell's star and its neighbours': a disc a pixel across may
    // reach past its cell's edge (a small view's pixel is near a cell).
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            let cell = here + vec2<f32>(f32(dx), f32(dy));
            if (any(cell < vec2<f32>(0.0, 0.0)) || any(cell > vec2<f32>(n - 1.0, n - 1.0))) {
                continue;
            }
            let h = ifs_env_hash(vec3<u32>(u32(cell.x), u32(cell.y), face));
            if (h.x > 0.09) {
                continue;
            }
            let q = (cell + vec2<f32>(h.y, h.z)) / n * 2.0 - vec2<f32>(1.0, 1.0);
            var sd = vec3<f32>(q.x, q.y, side);
            if (face < 2u) {
                sd = vec3<f32>(side, q.x, q.y);
            } else if (face < 4u) {
                sd = vec3<f32>(q.x, side, q.y);
            }
            let d = length(m - normalize(sd));
            if (d >= r) {
                continue;
            }
            let g = ifs_env_hash(vec3<u32>(u32(cell.x) + 7919u, u32(cell.y) + 104729u, face + 11u));
            let bright = min(0.04 * pow(max(g.x, 1.0e-4), -0.8), 1.0);
            // Orange through white to blue-white.
            let t = g.y;
            var tint = vec3<f32>(1.0, 1.0, 1.0);
            if (t < 0.25) {
                tint = mix(vec3<f32>(1.0, 0.76, 0.52), tint, t / 0.25);
            } else if (t >= 0.6) {
                tint = mix(tint, vec3<f32>(0.72, 0.82, 1.0), (t - 0.6) / 0.4);
            }
            out = out + tint * (bright * (1.0 - smoothstep(0.5 * r, r, d)));
        }
    }
    return out * (k * (1.0 - 2.0 * lod) * smoothstep(0.0, 0.05, m.y));
}

// The irradiance's harmonic `i` (`environment::irradiance_sh`).
fn ifs_env_sh(i: u32) -> vec3<f32> {
    let k = 3u * i;
    return vec3<f32>(
        params.fdata[38u + k / 4u][k % 4u],
        params.fdata[38u + (k + 1u) / 4u][(k + 1u) % 4u],
        params.fdata[38u + (k + 2u) / 4u][(k + 2u) % 4u],
    );
}

// What the sky lights a surface facing `s` (its frame) with, over pi: a
// white Lambertian surface's radiance (`environment::sh9`).
fn ifs_env_irradiance(s: vec3<f32>) -> vec3<f32> {
    let x = s.x;
    let y = s.y;
    let z = s.z;
    let e = ifs_env_sh(0u) * 0.282095
        + ifs_env_sh(1u) * (0.488603 * y)
        + ifs_env_sh(2u) * (0.488603 * z)
        + ifs_env_sh(3u) * (0.488603 * x)
        + ifs_env_sh(4u) * (1.092548 * x * y)
        + ifs_env_sh(5u) * (1.092548 * y * z)
        + ifs_env_sh(6u) * (0.315392 * (3.0 * z * z - 1.0))
        + ifs_env_sh(7u) * (1.092548 * x * z)
        + ifs_env_sh(8u) * (0.546274 * (x * x - y * y));
    return max(e, vec3<f32>(0.0, 0.0, 0.0)) * params.fdata[36].z;
}

// What a coat of roughness `rough` reflects of the sky toward `v` at `n`
// (the world's frame; its Fresnel is the caller's): the reflection level
// for the roughness, read about the lobe's dominant direction, which
// leans to the normal as the coat roughens (Lagarde and de Rousiers 2014,
// as `ifs_studio_reflect`).
fn ifs_env_reflect(n: vec3<f32>, v: vec3<f32>, rough: f32) -> vec3<f32> {
    let r = 2.0 * dot(n, v) * n - v;
    let s = (1.0 - rough) * (sqrt(1.0 - rough) + rough);
    let d = normalize(n + (r - n) * s);
    return ifs_env(ifs_env_dir(d), clamp(rough, 0.0, 1.0) * params.fdata[37].y);
}
"#;

/// A pass's HDRI sky: its map on the GPU while one is in use, the settings
/// it is read with, and the 1x1 stand-in and the sampler its bindings
/// (`ENV_WGSL`'s 14 and 15) take either way. Mode D's renderer and a
/// terrain's each hold one.
#[derive(Default)]
pub struct EnvSlot {
    gpu: Option<EnvGpu>,
    settings: Option<crate::escape::path_core::EnvSettings>,
    stand_in: Option<(wgpu::Texture, wgpu::TextureView)>,
    sampler: Option<wgpu::Sampler>,
}

impl EnvSlot {
    /// The sky `settings` names, or none: its bundled map loaded and
    /// uploaded when the id changes (desktop; the web build has none yet).
    /// True when anything about it changed.
    pub fn set(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, settings: Option<crate::escape::path_core::EnvSettings>) -> bool {
        let id = settings.map(|s| s.def.id);
        if self.gpu.as_ref().map(|e| e.id) != id {
            if let Some(e) = self.gpu.take() {
                e.texture.destroy();
            }
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(def) = settings.map(|s| s.def) {
                match load(def) {
                    Ok(map) => self.gpu = Some(EnvGpu::new(device, queue, def.id, &map)),
                    Err(e) => log::warn!("environment {}: {e}", def.id),
                }
            }
            #[cfg(target_arch = "wasm32")]
            let _ = (device, queue);
        }
        let changed = self.settings != settings;
        self.settings = settings;
        changed
    }

    /// Test-only: `map` as the sky's map, read with `settings`, in place of
    /// the bundled file its id names.
    #[cfg(test)]
    pub fn set_map(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, map: &EnvMap, settings: crate::escape::path_core::EnvSettings) {
        if let Some(e) = self.gpu.take() {
            e.texture.destroy();
        }
        self.gpu = Some(EnvGpu::new(device, queue, settings.def.id, map));
        self.settings = Some(settings);
    }

    /// The sky a pass reads: its settings, when its map is loaded.
    pub fn in_use(&self) -> Option<crate::escape::path_core::EnvSettings> {
        self.gpu.as_ref().and(self.settings)
    }

    /// Its sun, with the settings it is read with, when one is in use.
    pub fn sun(&self) -> Option<(EnvSun, crate::escape::path_core::EnvSettings)> {
        Some((self.gpu.as_ref()?.sun?, self.in_use()?))
    }

    /// The view and the sampler bindings 14 and 15 take: the sky's map, or
    /// the stand-in no pass reads.
    pub fn bindings(&mut self, device: &wgpu::Device) -> (wgpu::TextureView, wgpu::Sampler) {
        let sampler = self.sampler.get_or_insert_with(|| env_sampler(device)).clone();
        let view = match &self.gpu {
            Some(e) => e.view.clone(),
            None => self.stand_in.get_or_insert_with(|| env_stand_in(device)).1.clone(),
        };
        (view, sampler)
    }

    /// What of the sky decides a walk's shadow rays: whether it has a sun
    /// and where it stands. Empty with none.
    pub fn key(&self) -> String {
        match (&self.gpu, self.in_use()) {
            (Some(e), Some(set)) => format!("|env:{}:{}:{}:{}", e.id, e.sun.is_some(), set.rotation, set.horizon),
            _ => String::new(),
        }
    }

    /// The sky into a pass's `fdata` (mode D's or a terrain's): its sun the
    /// rig's next free light after the `fdata[7].z` there -- in place of an
    /// `untouched` rig's stand-in, the sky lighting the scene -- in the
    /// world's frame, z up (`ifs_env_dir` inverted), its intensity the
    /// irradiance over pi (`pt_light`) and its colour the share of each
    /// channel; and the rest of the map at [`ENV_FDATA`] ([`env_fdata`]).
    /// Nothing without one.
    pub fn pack(&self, fdata: &mut [[f32; 4]], untouched: bool) {
        let (Some(e), Some(set)) = (&self.gpu, self.in_use()) else { return };
        // An untouched rig's stand-in key gives way to the sky's sun; under
        // a sky without one (a night's) it stays, or the scene is dark.
        let mut n = if untouched && e.sun.is_some() { 0 } else { fdata[7][2] as usize };
        if let Some(sun) = e.sun.filter(|_| n < 4) {
            let s = turned(raised(sun.dir, set.horizon), set.rotation);
            let d = [s[0], -s[2], s[1]];
            let e3 = sun.power(set.scale);
            let peak = e3.iter().cloned().fold(0.0f32, f32::max).max(1.0e-30);
            fdata[11 + n * 2] = [d[0], d[1], d[2], peak];
            fdata[12 + n * 2] = [e3[0] / peak, e3[1] / peak, e3[2] / peak, 0.0];
            fdata[28 + n] = [0.0; 4];
            n += 1;
        }
        fdata[7][2] = n as f32;
        fdata[ENV_FDATA..ENV_FDATA + ENV_FDATA_LEN].copy_from_slice(&env_fdata(e, &set));
    }

    pub fn destroy(&mut self) {
        if let Some(e) = self.gpu.take() {
            e.texture.destroy();
        }
        if let Some((t, _)) = self.stand_in.take() {
            t.destroy();
        }
    }
}

/// The bundled sky `def`'s file, decoded (desktop: from the shipped
/// `assets/environments/`).
#[cfg(not(target_arch = "wasm32"))]
pub fn load(def: &EnvDef) -> Result<EnvMap, String> {
    if def.file.is_empty() {
        return Ok(EnvMap { width: 64, height: 32, texels: vec![[0.0; 3]; 64 * 32], sun: None, median: 1.0 });
    }
    let path = crate::resources::resource_path(format!("assets/environments/{}", def.file));
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    EnvMap::decode(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundled(id: &str) -> EnvMap {
        let def = get(id).expect("bundled");
        let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/environments").join(def.file)).expect("the file");
        EnvMap::decode(&bytes).expect("decodes")
    }

    fn whole_light(map: &EnvMap) -> f32 {
        map.light() + map.sun.map_or(0.0, |s| lum(s.irradiance))
    }

    /// Every bundled sky decodes as a 2:1 map of finite radiance.
    #[test]
    fn the_bundled_skies_decode() {
        for def in ENVIRONMENTS.iter().filter(|d| !d.file.is_empty()) {
            let map = bundled(def.id);
            assert_eq!((map.width, map.height), (1024, 512), "{}", def.id);
            assert!(map.texels.iter().all(|c| c.iter().all(|v| v.is_finite() && *v >= 0.0)), "{}", def.id);
            assert!(map.median > 0.0, "{}", def.id);
        }
    }

    /// The Sunflowers sun (measured 2026-10-20: 9 texels holding 47.1% of
    /// the light, its brightest at azimuth 216 degrees -- 36 degrees left
    /// of behind, in this map's frame -- and elevation 43) is lifted out:
    /// its direction and share as measured, the light it and the map hold
    /// together what the map held, and the map left without a texel near
    /// the threshold.
    #[test]
    fn the_sunflowers_sun_is_lifted_out() {
        let def = get("sunflowers").expect("bundled");
        let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/environments").join(def.file)).expect("the file");
        let whole = {
            let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Hdr).expect("decodes").into_rgb32f();
            let (w, h) = img.dimensions();
            let mut t = 0.0f64;
            for (i, p) in img.pixels().enumerate() {
                t += lum([p.0[0], p.0[1], p.0[2]]) as f64 * texel_solid_angle(i as u32 / w, w, h) as f64;
            }
            t as f32
        };
        let map = bundled("sunflowers");
        let sun = map.sun.expect("a sun");
        let share = lum(sun.irradiance) / whole;
        let elevation = sun.dir[1].asin().to_degrees();
        let azimuth = sun.dir[0].atan2(-sun.dir[2]).to_degrees().rem_euclid(360.0);
        println!("sun: {share:.3} of the light, elevation {elevation:.1}, azimuth {azimuth:.1}, radius {:.2} degrees", sun.radius.to_degrees());
        assert!((0.44..0.50).contains(&share), "{share}");
        assert!((elevation - 43.0).abs() < 1.0, "{elevation}");
        assert!((azimuth - 36.0).abs() < 1.0 || (azimuth - 216.0).abs() < 1.0, "{azimuth}");
        assert!((whole_light(&map) - whole).abs() <= 1.0e-3 * whole, "{} against {whole}", whole_light(&map));
        let peak = map.texels.iter().map(|&c| lum(c)).fold(0.0f32, f32::max);
        assert!(peak < SUN_THRESHOLD * map.median, "a texel of {peak} left");
    }

    /// The catalog holds each sky's median as its file has it.
    #[test]
    fn the_catalog_knows_each_skys_median() {
        for def in ENVIRONMENTS.iter().filter(|d| !d.file.is_empty()) {
            let map = bundled(def.id);
            println!("{}: median {}", def.id, map.median);
            assert!((def.median - map.median).abs() <= 1.0e-3 * map.median, "{}: {} against {}", def.id, def.median, map.median);
        }
    }

    /// The night sky has no sun: its brightest stars stay in the map.
    #[test]
    fn the_night_sky_keeps_its_stars() {
        let map = bundled("rogland_night");
        assert!(map.sun.is_none(), "{:?}", map.sun);
    }

    /// A uniform sky is itself at every reflection level, and lights every
    /// face alike with its own radiance (over pi, a white surface's).
    #[test]
    fn a_uniform_sky_is_its_own_at_every_level() {
        let c = [0.5f32, 1.0, 2.0];
        let map = EnvMap { width: 64, height: 32, texels: vec![c; 64 * 32], sun: None, median: 1.0 };
        let levels = map.reflection_levels();
        assert_eq!(levels.len(), ENV_LODS as usize);
        for (k, l) in levels.iter().enumerate() {
            assert_eq!((l.width, l.height), (64 >> (k + 1), 32 >> (k + 1)));
            for t in &l.texels {
                for ch in 0..3 {
                    assert!((t[ch] - c[ch]).abs() <= 1.0e-4 * c[ch], "level {}: {t:?}", k + 1);
                }
            }
        }
        let sh = irradiance_sh(&map.level(), 0.3, 0.2);
        for s in [[0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.6, 0.0, -0.8]] {
            let y = sh9(s);
            for ch in 0..3 {
                let e: f32 = (0..9).map(|k| sh[k][ch] * y[k]).sum();
                assert!((e - c[ch]).abs() <= 2.0e-3 * c[ch], "{s:?}: {e} against {}", c[ch]);
            }
        }
    }

    /// The map read through `env_uv` with no turn and a level horizon is
    /// the map read through `map_uv`; turned a quarter, a quarter round.
    #[test]
    fn the_skys_frame_is_the_maps_untouched() {
        for s in [[0.3f32, 0.5, -0.81], [-0.7, -0.2, 0.68], [0.0, 1.0, 0.0]] {
            let n = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
            let s = s.map(|v| v / n);
            let (a, b) = (env_uv(s, 0.0, 0.0), map_uv(s));
            assert!((a.0 - b.0).abs() < 1.0e-5 && (a.1 - b.1).abs() < 1.0e-5, "{a:?} against {b:?}");
            let q = env_uv(s, 0.25, 0.0);
            if s[1].abs() < 0.99 {
                assert!(((q.0 - a.0).rem_euclid(1.0) - 0.75).abs() < 1.0e-5, "{q:?} against {a:?}");
            }
        }
    }

    /// The harmonics light as the sky does: on the Sunflowers sky without
    /// its sun, a white surface's radiance by the nine coefficients against
    /// the cosine-weighted integral over the whole map, facing up, down
    /// and to six sides -- within 6% of the sky's brightest face.
    #[test]
    fn the_irradiance_harmonics_light_as_the_sky() {
        let map = bundled("sunflowers");
        let small = map.small();
        let sh = irradiance_sh(&small, 0.0, 0.0);
        let faces = [[0.0f32, 1.0, 0.0], [0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0], [0.6, 0.6, 0.53], [-0.5, -0.4, 0.77]];
        let mut results = Vec::new();
        for n in faces {
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let n = n.map(|v| v / l);
            let mut e = 0.0f64;
            for y in 0..map.height {
                let sa = texel_solid_angle(y, map.width, map.height) as f64;
                for x in 0..map.width {
                    let d = map_direction((x as f32 + 0.5) / map.width as f32, (y as f32 + 0.5) / map.height as f32);
                    let c = (n[0] * d[0] + n[1] * d[1] + n[2] * d[2]).max(0.0) as f64;
                    e += lum(map.texels[(y * map.width + x) as usize]) as f64 * c * sa;
                }
            }
            let truth = (e / std::f64::consts::PI) as f32;
            let y = sh9(n);
            let got: f32 = (0..9).map(|k| lum(sh[k]) * y[k]).sum();
            results.push((n, truth, got));
        }
        let top = results.iter().map(|r| r.1).fold(0.0f32, f32::max);
        for (n, truth, got) in &results {
            println!("face {n:?}: integral {truth:.4}, harmonics {got:.4}");
            assert!((got - truth).abs() <= 0.06 * top, "face {n:?}: {got} against {truth}");
        }
    }

    /// The reflection levels keep the sky's light: each level's whole
    /// (luminance by solid angle) within 1.5% of the map's, and the Sunflowers
    /// sky's levels made in well under a second.
    #[test]
    fn the_reflection_levels_keep_the_light() {
        let map = bundled("sunflowers");
        let t = std::time::Instant::now();
        let levels = map.reflection_levels();
        println!("reflection levels in {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
        let whole = |l: &EnvLevel| -> f64 {
            (0..l.height)
                .map(|y| {
                    let sa = texel_solid_angle(y, l.width, l.height) as f64;
                    (0..l.width).map(|x| lum(l.texels[(y * l.width + x) as usize]) as f64).sum::<f64>() * sa
                })
                .sum()
        };
        let base = whole(&map.level());
        for (k, l) in levels.iter().enumerate() {
            let r = whole(l) / base;
            println!("level {}: {}x{}, light {:.4} of the map's", k + 1, l.width, l.height, r);
            assert!((r - 1.0).abs() < 0.015, "level {}: {r}", k + 1);
        }
    }

    /// The horizon's warp: the map's horizon at the elevation asked, its
    /// zenith and nadir where they were, monotone between.
    #[test]
    fn the_horizon_is_raised_and_the_poles_stay() {
        let h = 0.3f32;
        let half_pi = std::f32::consts::FRAC_PI_2;
        assert!((warp_elevation(0.0, h) - h).abs() < 1.0e-6);
        assert!((warp_elevation(half_pi, h) - half_pi).abs() < 1.0e-6);
        assert!((warp_elevation(-half_pi, h) + half_pi).abs() < 1.0e-6);
        let mut last = -half_pi;
        for k in 0..=100 {
            let e = warp_elevation(-half_pi + std::f32::consts::PI * k as f32 / 100.0, h);
            assert!(e >= last - 1.0e-6);
            last = e;
        }
        let up = raised(map_direction(0.6, 0.5), h);
        assert!((up[1].asin() - h).abs() < 1.0e-5, "a horizon sun stands at the horizon: {up:?}");
    }

    /// `map_direction` is the equirectangular convention the shaders invert:
    /// the middle straight ahead (-z), the top up, a quarter turn right +x.
    #[test]
    fn the_maps_directions() {
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1.0e-6);
        assert!(close(map_direction(0.5, 0.5), [0.0, 0.0, -1.0]));
        assert!(close(map_direction(0.75, 0.5), [1.0, 0.0, 0.0]));
        assert!(close(map_direction(0.3, 0.0), [0.0, 1.0, 0.0]));
    }
}

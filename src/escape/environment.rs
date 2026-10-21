//! HDRI environments (docs/projects/hdri-environment.md): a sky photographed
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
    /// The credit owed (CC0 asks none; Poly Haven asks for a link).
    pub credit: &'static str,
    pub url: &'static str,
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
    },
    EnvDef {
        id: "rogland_night",
        name: "Rogland Clear Night",
        file: "rogland_clear_night_1k.hdr",
        level: 0.2,
        median: 0.29219,
        credit: "Rogland Clear Night, Poly Haven, CC0",
        url: "https://polyhaven.com/a/rogland_clear_night",
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
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Hdr).map_err(|e| format!("not a Radiance HDR: {e}"))?;
        let img = img.into_rgb32f();
        let (width, height) = img.dimensions();
        if width < 4 || height < 2 {
            return Err(format!("an environment of {width}x{height} is too small"));
        }
        let texels: Vec<[f32; 3]> = img.pixels().map(|p| [p.0[0].max(0.0), p.0[1].max(0.0), p.0[2].max(0.0)]).map(|c| if c.iter().all(|v| v.is_finite()) { c } else { [0.0; 3] }).collect();
        let mut map = EnvMap { width, height, texels, sun: None, median: 0.0 };
        map.median = map.upper_median();
        map.sun = map.lift_sun();
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
    fn lift_sun(&mut self) -> Option<EnvSun> {
        let (w, h) = (self.width, self.height);
        let at = |x: u32, y: u32| (y * w + x) as usize;
        let threshold = SUN_THRESHOLD * self.median.max(1.0e-12);
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

/// A sky on the GPU: its map, filterable (`rgba16float`), and its sun.
pub struct EnvGpu {
    pub id: &'static str,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sun: Option<EnvSun>,
}

impl EnvGpu {
    /// `map`, uploaded.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, id: &'static str, map: &EnvMap) -> Self {
        let (texture, view) = env_texture(device, queue, map.width, map.height, &map.texels);
        EnvGpu { id, texture, view, sun: map.sun }
    }
}

/// An `rgba16float` texture of `texels` (w by h).
pub fn env_texture(device: &wgpu::Device, queue: &wgpu::Queue, w: u32, h: u32, texels: &[[f32; 3]]) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Environment"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let data: Vec<u16> = texels.iter().flat_map(|c| [c[0], c[1], c[2], 1.0]).map(|v| half::f16::from_f32(v.min(65000.0)).to_bits()).collect();
    queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        bytemuck::cast_slice(&data),
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 8), rows_per_image: Some(h) },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
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
/// poles, linear.
pub fn env_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Environment"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
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

/// The bundled sky `def`'s file, decoded (desktop: from the shipped
/// `assets/environments/`).
#[cfg(not(target_arch = "wasm32"))]
pub fn load(def: &EnvDef) -> Result<EnvMap, String> {
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
        for def in ENVIRONMENTS {
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
        for def in ENVIRONMENTS {
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

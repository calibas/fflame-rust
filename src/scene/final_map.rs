//! **A flame's final transforms, for the inverse walk** (tracker item
//! C2c, `docs/projects/deep-zoom-tracker.md`).
//!
//! A final transform reshapes what is plotted and feeds nothing forward:
//! the chaos game's orbit is the normal transforms' alone, and each point
//! is plotted at `F(x)`. So a view `V` holds exactly the orbit's points
//! in `F⁻¹(V)`, and the walk plans for that -- a disc in the attractor's
//! own space, pulled back from the view ([`FinalMap::pull_back`]). The
//! render applies the final itself, after the forced word, as it does
//! after every free step.
//!
//! A final is followed here when it is affine, or `bipolar` between its
//! affine and post-affine: one-to-one, so the pull-back is one piece.
//! One `bipolar` in the chain at most: after it, the plane's far points
//! are a bounded place (its poles), and a second one's pull-back would
//! split around them.
//!
//! **At depth the finals run in offsets** too (`FinalMap::diff`, and its
//! twin `ct_final_diff` in `replay_delta.wgsl`): the render's final chain
//! would apply them to the absolute point, where f32 had already lost
//! the picture.

use crate::scene::cylinder::View;
use crate::scene::ifs_analysis::{transform_affine_2d_ordered, Affine2};
use crate::scene::transforms::{Flame, Transform};
use crate::variations::VariationRegistry;
use std::f64::consts::{FRAC_PI_2, PI};

/// One final transform's map.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    Affine { f: Affine2, inv: Affine2 },
    /// `post(w · bipolar(pre(p)))`.
    Bipolar { pre: Affine2, pre_inv: Affine2, w: f64, shift: f64, post: Affine2, post_inv: Affine2 },
}

/// The final transforms every normal transform is plotted through, in
/// order. See the module.
#[derive(Debug, Clone, PartialEq)]
pub struct FinalMap {
    steps: Vec<Step>,
}

/// `bipolar`, as its WGSL computes it: bipolar coordinates, the angle
/// halved, shifted and wrapped into `[−π/2, π/2]`.
pub fn bipolar(p: [f64; 2], shift: f64) -> [f64; 2] {
    let x2y2 = p[0] * p[0] + p[1] * p[1];
    let mut y = 0.5 * (2.0 * p[1]).atan2(x2y2 - 1.0) - FRAC_PI_2 * shift;
    if y > FRAC_PI_2 {
        y = -FRAC_PI_2 + (y + FRAC_PI_2) % PI;
    } else if y < -FRAC_PI_2 {
        y = FRAC_PI_2 - (FRAC_PI_2 - y) % PI;
    }
    let t = x2y2 + 1.0;
    let (f, g) = (t + 2.0 * p[0], t - 2.0 * p[0]);
    if g == 0.0 || f / g <= 0.0 {
        return [0.0, 0.0];
    }
    [(f / g).ln() / (2.0 * PI), 2.0 / PI * y]
}

/// **`bipolar`'s one preimage**, or `None` outside its range. Its `x`
/// is `ln(|p+1|/|p−1|)/π` and its angle, before the halving,
/// `φ = arg((p−1)/(p+1))`; so `q = (p−1)/(p+1) = e^{−πx + iφ}`, and
/// `p = (1+q)/(1−q)`. The wrap turns the angle round the circle, which
/// loses nothing: of the half-angles `θ ≡ πy/2 + π·shift/2 (mod π)`,
/// one lies in `(−π/2, π/2]`, where `atan2`'s half does.
pub fn bipolar_inverse(u: [f64; 2], shift: f64) -> Option<[f64; 2]> {
    if !(u[1].abs() <= 1.0) || !u[0].is_finite() {
        return None;
    }
    let theta = (FRAC_PI_2 * (u[1] + shift) + FRAC_PI_2).rem_euclid(PI) - FRAC_PI_2;
    let (m, phi) = ((-PI * u[0]).exp(), 2.0 * theta);
    let q = [m * phi.cos(), m * phi.sin()];
    // (1+q)/(1−q)
    let (a, b) = ([1.0 + q[0], q[1]], [1.0 - q[0], -q[1]]);
    let d = b[0] * b[0] + b[1] * b[1];
    if !(d > 1e-300) {
        return None;
    }
    let p = [(a[0] * b[0] + a[1] * b[1]) / d, (a[1] * b[0] - a[0] * b[1]) / d];
    (p[0].is_finite() && p[1].is_finite()).then_some(p)
}

/// `ln|1 + a|` for complex `a`, to the precision of `a`.
fn ln_abs_1p(a: [f64; 2]) -> f64 {
    0.5 * (2.0 * a[0] + a[0] * a[0] + a[1] * a[1]).ln_1p()
}

/// `a / b` for complex numbers, or `None` at `b = 0`.
fn cdiv(a: [f64; 2], b: [f64; 2]) -> Option<[f64; 2]> {
    let d = b[0] * b[0] + b[1] * b[1];
    (d > 0.0).then(|| [(a[0] * b[0] + a[1] * b[1]) / d, (a[1] * b[0] - a[0] * b[1]) / d])
}

/// **`bipolar(v + e) - bipolar(v)`, without forming the difference**.
/// With `a+- = e/(v +- 1)`, `|v + e +- 1| = |v +- 1|*|1 + a+-|`, so the
/// log term changes by `(ln|1 + a+| - ln|1 + a-|)/pi`, and the angle
/// `arg((v-1)/(v+1))` by `arg(1 + a-) - arg(1 + a+)`, which halved and
/// scaled is `/pi` of the output. The angle's wrap is a seam in the plot:
/// a step across it lands the whole strip away, `None` -- far off any
/// view deep enough to take offsets.
pub fn bipolar_diff(v: [f64; 2], e: [f64; 2], shift: f64) -> Option<[f64; 2]> {
    let ap = cdiv(e, [v[0] + 1.0, v[1]])?;
    let am = cdiv(e, [v[0] - 1.0, v[1]])?;
    let dx = (ln_abs_1p(ap) - ln_abs_1p(am)) / PI;
    let arg_1p = |a: [f64; 2]| a[1].atan2(1.0 + a[0]);
    let dy = (arg_1p(am) - arg_1p(ap)) / PI;
    let y = bipolar(v, shift)[1];
    (dx.is_finite() && (y + dy).abs() <= 1.0).then_some([dx, dy])
}

/// Floats per final step in the shader's table: see [`FinalMap::rows`].
pub const FINAL_ROW: usize = 13;

impl Step {
    fn of(t: &Transform, registry: &VariationRegistry, order: &[String]) -> Result<Step, String> {
        if let Ok(f) = transform_affine_2d_ordered(t, registry, order) {
            let inv = f.inverse().ok_or("a final transform that is singular")?;
            return Ok(Step::Affine { f, inv });
        }
        let live: Vec<(&String, f32)> = t.variations.iter().filter(|(_, w)| **w != 0.0).map(|(n, w)| (n, *w)).collect();
        match live.as_slice() {
            [(name, w)] if name.as_str() == "bipolar" => {
                let pre = Affine2 { m: [[t.a as f64, t.b as f64], [t.c as f64, t.d as f64]], t: [t.e as f64, t.f as f64] };
                let post = if t.post_affine_enabled {
                    Affine2 { m: [[t.post_a as f64, t.post_b as f64], [t.post_c as f64, t.post_d as f64]], t: [t.post_e as f64, t.post_f as f64] }
                } else {
                    Affine2::IDENTITY
                };
                let (Some(pre_inv), Some(post_inv)) = (pre.inverse(), post.inverse()) else {
                    return Err("a final transform that is singular".into());
                };
                Ok(Step::Bipolar {
                    pre,
                    pre_inv,
                    w: *w as f64,
                    shift: t.get_variation_param_or_default("bipolar", "shift", registry) as f64,
                    post,
                    post_inv,
                })
            }
            _ => {
                let names: Vec<&str> = live.iter().map(|(n, _)| n.as_str()).collect();
                Err(format!("a final transform of {} is not pulled back yet (affine and bipolar are)", names.join(" + ")))
            }
        }
    }

    fn forward(&self, p: [f64; 2]) -> [f64; 2] {
        match self {
            Step::Affine { f, .. } => f.apply(p),
            Step::Bipolar { pre, w, shift, post, .. } => {
                let b = bipolar(pre.apply(p), *shift);
                post.apply([w * b[0], w * b[1]])
            }
        }
    }

    /// `self(z + d) - self(z)`, formed without the difference.
    fn diff(&self, z: [f64; 2], d: [f64; 2]) -> Option<[f64; 2]> {
        let lin = |a: &Affine2, d: [f64; 2]| [a.m[0][0] * d[0] + a.m[0][1] * d[1], a.m[1][0] * d[0] + a.m[1][1] * d[1]];
        match self {
            Step::Affine { f, .. } => Some(lin(f, d)),
            Step::Bipolar { pre, w, shift, post, .. } => {
                let k = bipolar_diff(pre.apply(z), lin(pre, d), *shift)?;
                Some(lin(post, [w * k[0], w * k[1]]))
            }
        }
    }

    /// The step as the shader's row: `[kind, pre linear (4), pre
    /// translation (2), w, shift, post linear (4)]`, kind 0 an affine (its
    /// linear part in the pre's place), 1 `bipolar`.
    fn row(&self) -> [f32; FINAL_ROW] {
        let (kind, pre, w, shift, post) = match self {
            Step::Affine { f, .. } => (0.0, *f, 1.0, 0.0, Affine2::IDENTITY),
            Step::Bipolar { pre, w, shift, post, .. } => (1.0, *pre, *w, *shift, *post),
        };
        [
            kind,
            pre.m[0][0] as f32,
            pre.m[0][1] as f32,
            pre.m[1][0] as f32,
            pre.m[1][1] as f32,
            pre.t[0] as f32,
            pre.t[1] as f32,
            w as f32,
            shift as f32,
            post.m[0][0] as f32,
            post.m[0][1] as f32,
            post.m[1][0] as f32,
            post.m[1][1] as f32,
        ]
    }

    fn inverse(&self, y: [f64; 2]) -> Option<[f64; 2]> {
        match self {
            Step::Affine { inv, .. } => Some(inv.apply(y)),
            Step::Bipolar { pre_inv, w, shift, post_inv, .. } => {
                let u = post_inv.apply(y);
                bipolar_inverse([u[0] / w, u[1] / w], *shift).map(|p| pre_inv.apply(p))
            }
        }
    }
}

impl FinalMap {
    /// The finals `flame`'s normal transforms are plotted through, or
    /// `None` when it has none; `Err` when they cannot be followed. Each
    /// normal transform carries its own list, and the walk follows one:
    /// every transform that is drawn must carry the same.
    pub fn of(flame: &Flame, registry: &VariationRegistry) -> Result<Option<FinalMap>, String> {
        let live: Vec<&Transform> = flame.transforms.iter().filter(|t| t.weight > 0.0).collect();
        if live.iter().any(|t| !t.linked_attachments.is_empty()) {
            return Err("a linked transform feeds the orbit, and is not planned yet".into());
        }
        let Some(first) = live.first() else { return Ok(None) };
        if live.iter().any(|t| t.final_attachments != first.final_attachments) {
            return Err("final transforms attached to some transforms and not others are not planned yet".into());
        }
        if first.final_attachments.is_empty() {
            return Ok(None);
        }
        let order = flame.active_variation_names_ordered(registry);
        let steps = first
            .final_attachments
            .iter()
            .map(|&i| {
                let t = flame.final_transforms.get(i).ok_or("a final attachment names no final transform")?;
                Step::of(t, registry, &order)
            })
            .collect::<Result<Vec<_>, String>>()?;
        if steps.iter().filter(|s| matches!(s, Step::Bipolar { .. })).count() > 1 {
            return Err("two bipolar finals are not pulled back yet".into());
        }
        Ok(Some(FinalMap { steps }))
    }

    /// **Where the plane's far points are plotted**, when that is a
    /// point: `bipolar` sends every point far from its poles towards
    /// one, `(0, −shift)` wrapped into its range, and the affines carry
    /// it on. `None` when the finals are affine and far points stay far.
    pub fn infinity(&self) -> Option<[f64; 2]> {
        let at = self.steps.iter().position(|s| matches!(s, Step::Bipolar { .. }))?;
        let Step::Bipolar { w, shift, post, .. } = self.steps[at] else { unreachable!() };
        // `bipolar` of a point on the positive x axis far out: its angle
        // term is 0, its log term tends to 0.
        let b = bipolar([1e300f64.sqrt(), 0.0], shift);
        let y = post.apply([w * 0.0, w * b[1]]);
        Some(self.steps[at + 1..].iter().fold(y, |p, s| s.forward(p)))
    }

    /// Where `x` is plotted.
    pub fn forward(&self, x: [f64; 2]) -> [f64; 2] {
        self.steps.iter().fold(x, |p, s| s.forward(p))
    }

    /// The point before each step, `x` first: where the shader's forms
    /// are taken for a reference that ends at `x` (`RefChain`).
    pub fn positions(&self, x: [f64; 2]) -> Vec<[f64; 2]> {
        let mut out = Vec::with_capacity(self.steps.len());
        let mut p = x;
        for s in &self.steps {
            out.push(p);
            p = s.forward(p);
        }
        out
    }

    /// **`F(x + d) - F(x)` in offsets**: each step's difference form in
    /// turn, carried from the point before it. `None` where a step drops
    /// the point (across `bipolar`'s seam).
    pub fn diff(&self, x: [f64; 2], d: [f64; 2]) -> Option<[f64; 2]> {
        let mut p = x;
        let mut d = d;
        for s in &self.steps {
            d = s.diff(p, d)?;
            p = s.forward(p);
        }
        Some(d)
    }

    /// The finals' Jacobian at `x`, from the difference forms.
    pub fn jacobian(&self, x: [f64; 2]) -> Option<[[f64; 2]; 2]> {
        let h = 1e-7 * x[0].hypot(x[1]).max(1e-6);
        let a = self.diff(x, [h, 0.0])?;
        let b = self.diff(x, [0.0, h])?;
        Some([[a[0] / h, b[0] / h], [a[1] / h, b[1] / h]])
    }

    /// The finals as the shader's table section: their count, then one
    /// [`FINAL_ROW`] per step (`replay_delta.wgsl`'s `ct_final_diff`).
    pub fn rows(&self) -> Vec<f32> {
        let mut out = vec![self.steps.len() as f32];
        for s in &self.steps {
            out.extend_from_slice(&s.row());
        }
        out
    }

    /// The one point plotted at `y`, or `None` outside the finals' range.
    pub fn inverse(&self, y: [f64; 2]) -> Option<[f64; 2]> {
        self.steps.iter().rev().try_fold(y, |p, s| s.inverse(p))
    }

    /// **The view, pulled back through the finals**: a disc in the
    /// attractor's space holding every point plotted in `view`, or `None`
    /// when no point is. `whole` holds the attractor: the answer where
    /// the view holds [`Self::infinity`], whose pull-back reaches past
    /// every disc -- the orbit is bounded, so `whole` holds what of it
    /// is there.
    ///
    /// The view's rim and interior are pulled back and bounded from the
    /// preimage of its centre, with a 2% margin. The map is one-to-one
    /// and continuous across the view -- `bipolar` is conformal, and its
    /// wrap is a seam in the plot, not in the plane -- so the preimage is
    /// the region its rim bounds, and the farthest point of it from an
    /// interior point is on the rim. The interior samples take the rest
    /// where the view reaches past the finals' range.
    pub fn pull_back(&self, view: View, whole: View) -> Option<View> {
        if self.infinity().is_some_and(|p| (p[0] - view.centre[0]).hypot(p[1] - view.centre[1]) <= 1.02 * view.radius) {
            return Some(whole);
        }
        const RIM: usize = 256;
        const RINGS: usize = 8;
        let mut pts: Vec<[f64; 2]> = Vec::with_capacity(RIM * (RINGS + 1) + 1);
        let at = |r: f64, a: f64| [view.centre[0] + r * view.radius * a.cos(), view.centre[1] + r * view.radius * a.sin()];
        for ring in 1..=RINGS {
            let r = ring as f64 / RINGS as f64;
            let n = if ring == RINGS { RIM } else { 8 * ring };
            for k in 0..n {
                if let Some(p) = self.inverse(at(r, std::f64::consts::TAU * k as f64 / n as f64)) {
                    pts.push(p);
                }
            }
        }
        let centre = self.inverse(view.centre).or_else(|| {
            let n = pts.len() as f64;
            (n > 0.0).then(|| [pts.iter().map(|p| p[0]).sum::<f64>() / n, pts.iter().map(|p| p[1]).sum::<f64>() / n])
        })?;
        let far = pts.iter().map(|p| (p[0] - centre[0]).hypot(p[1] - centre[1])).fold(0.0f64, f64::max);
        (far > 0.0 && far.is_finite()).then(|| View { centre, radius: 1.02 * far })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: u64) -> impl FnMut() -> f64 {
        let mut st = seed;
        move || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((st >> 11) as f64) / ((1u64 << 53) as f64)
        }
    }

    /// `bipolar`'s inverse undoes it, at every shift and across its
    /// wrap: its one preimage is the point that went in.
    #[test]
    fn bipolar_inverse_undoes_bipolar() {
        let mut u = lcg(11);
        for _ in 0..20_000 {
            let p = [u() * 8.0 - 4.0, u() * 8.0 - 4.0];
            let shift = u() * 4.0 - 2.0;
            let y = bipolar(p, shift);
            let q = bipolar_inverse(y, shift).expect("in range");
            let tol = 1e-9 * (1.0 + p[0].hypot(p[1])) / ((p[0] - 1.0).hypot(p[1]) * (p[0] + 1.0).hypot(p[1])).min(1.0);
            assert!((q[0] - p[0]).hypot(q[1] - p[1]) < tol, "shift {shift}: {p:?} -> {y:?} -> {q:?}");
        }
        assert!(bipolar_inverse([0.3, 1.2], 0.0).is_none(), "past the strip is outside the range");
    }

    /// **The difference form is the difference**: against the direct
    /// subtraction in f64 where that still has digits (offsets of 1e-3),
    /// and against the Jacobian where it does not (1e-11), across shifts,
    /// near the poles and far out.
    #[test]
    fn bipolar_diff_is_the_difference() {
        let mut u = lcg(3);
        let mut worst = 0.0f64;
        for _ in 0..5000 {
            let v = [u() * 6.0 - 3.0, u() * 6.0 - 3.0];
            let shift = u() * 4.0 - 2.0;
            let dir = u() * std::f64::consts::TAU;
            let near = ((v[0] - 1.0).hypot(v[1])).min((v[0] + 1.0).hypot(v[1]));
            for scale in [1e-3, 1e-11] {
                let e = [scale * near * dir.cos(), scale * near * dir.sin()];
                let a = bipolar(v, shift);
                let b = bipolar([v[0] + e[0], v[1] + e[1]], shift);
                let Some(got) = bipolar_diff(v, e, shift) else {
                    // Only across the seam.
                    assert!((b[1] - a[1]).abs() > 1.0, "{v:?} + {e:?} dropped away from the seam");
                    continue;
                };
                let want = if scale > 1e-6 {
                    [b[0] - a[0], b[1] - a[1]]
                } else {
                    let h = 1e-6 * near;
                    let da = bipolar([v[0] + h * dir.cos(), v[1] + h * dir.sin()], shift);
                    [(da[0] - a[0]) / h * scale * near, (da[1] - a[1]) / h * scale * near]
                };
                let norm = want[0].hypot(want[1]);
                let err = (got[0] - want[0]).hypot(got[1] - want[1]) / norm;
                let tol = if scale > 1e-6 { 1e-9 } else { 1e-4 };
                assert!(err < tol, "{v:?} + {e:?}, shift {shift}: {got:?} against {want:?} ({err:.1e})");
                if scale < 1e-6 {
                    worst = worst.max(err);
                }
            }
        }
        println!("  worst against the Jacobian at 1e-11: {worst:.1e}");
    }

    /// **The shader's final forms are the CPU's**: `ct_final_diff` in
    /// `replay_delta.wgsl`, for a `bipolar` final with an affine either
    /// side, against [`FinalMap::diff`] in f64 on the SAME f32 inputs, at
    /// offsets from 1e-1 to 1e-12 of the point -- judged relative to the
    /// difference itself, like the word's forms
    /// (`forward_delta::the_shader_forward_forms_are_the_cpu_ones`).
    #[test]
    #[ignore = "needs a GPU"]
    fn the_shader_final_forms_are_the_cpu_ones() {
        use wgpu::util::DeviceExt;
        let pre = Affine2 { m: [[0.9, 0.3], [-0.2, 1.1]], t: [0.1, -0.2] };
        let post = Affine2 { m: [[0.8, -0.1], [0.2, 0.7]], t: [0.3, 0.4] };
        let f = FinalMap {
            steps: vec![Step::Bipolar { pre, pre_inv: pre.inverse().unwrap(), w: 1.3, shift: 0.7, post, post_inv: post.inverse().unwrap() }],
        };
        let rows = f.rows();
        let util = std::fs::read_to_string("shaders/core/utilities.wgsl").expect("utilities.wgsl");
        let a = util.find("fn ff_atan2(").expect("ff_atan2");
        let end = a + util[a..].find("\n}\n").expect("its end") + 3;
        let delta = std::fs::read_to_string("shaders/core/replay_delta.wgsl").expect("replay_delta.wgsl");
        let src = format!(
            "@group(0) @binding(0) var<storage, read> cylinders: array<f32>;\n\
             @group(0) @binding(1) var<storage, read> jobs: array<vec4<f32>>;\n\
             @group(0) @binding(2) var<storage, read_write> out: array<vec2<f32>>;\n{}\n{delta}\n\
             @compute @workgroup_size(64)\n\
             fn main(@builtin(global_invocation_id) id: vec3<u32>) {{\n\
                 if (id.x >= arrayLength(&out)) {{ return; }}\n\
                 let j = jobs[id.x];\n\
                 out[id.x] = ct_final_diff(1u, j.xy, j.zw);\n\
             }}\n",
            &util[a..end]
        );
        let mut jobs: Vec<[f32; 4]> = Vec::new();
        let mut u = lcg(9);
        for _ in 0..600 {
            let z = [u() as f32 * 3.0 - 1.5, u() as f32 * 3.0 - 1.5];
            for p in 1..=12 {
                let mag = 10f32.powi(-p) * z[0].hypot(z[1]).max(0.1);
                let a = u() as f32 * std::f32::consts::TAU;
                jobs.push([z[0], z[1], mag * a.cos(), mag * a.sin()]);
            }
        }
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).expect("adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(src.into()) });
        let pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let st = wgpu::BufferUsages::STORAGE;
        let init = |bytes: &[u8]| device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytes, usage: st });
        let rows_buf = init(bytemuck::cast_slice(&rows));
        let jobs_buf = init(bytemuck::cast_slice(&jobs));
        let bytes = (jobs.len() * 8) as u64;
        let out = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: bytes, usage: st | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
        let stage = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipe.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: rows_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: jobs_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: out.as_entire_binding() },
            ],
        });
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
            pass.set_pipeline(&pipe);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups((jobs.len() as u32).div_ceil(64), 1, 1);
        }
        enc.copy_buffer_to_buffer(&out, 0, &stage, 0, bytes);
        queue.submit(Some(enc.finish()));
        stage.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let got: Vec<[f32; 2]> = bytemuck::cast_slice::<u8, [f32; 2]>(&stage.slice(..).get_mapped_range()).to_vec();
        let mut by_mag = [0.0f64; 13];
        let (mut n, mut worst) = (0usize, 0.0f64);
        for (k, (j, g)) in jobs.iter().zip(&got).enumerate() {
            let z = [j[0] as f64, j[1] as f64];
            let d = [j[2] as f64, j[3] as f64];
            let Some(want) = f.diff(z, d) else { continue };
            let norm = want[0].hypot(want[1]);
            if !(norm > 0.0) || g[0].abs() > 1e20 {
                continue;
            }
            let err = (g[0] as f64 - want[0]).hypot(g[1] as f64 - want[1]) / norm;
            n += 1;
            worst = worst.max(err);
            let p = k % 12 + 1;
            by_mag[p] = by_mag[p].max(err);
        }
        println!("  {n} differences; worst relative error by offset 1e-1..1e-12: {:?}", by_mag[1..].iter().map(|e| format!("{e:.1e}")).collect::<Vec<_>>());
        assert!(n > 5000, "only {n} checkable");
        assert!(worst < 1e-5, "worst relative error {worst:.2e}");
    }

    /// **The pulled-back view holds everything plotted in the view**:
    /// points of the plane whose image lands in a view are inside the
    /// disc it pulls back to, near the poles, at the wrap and far out.
    #[test]
    fn a_pulled_back_view_holds_what_lands_in_it() {
        let step = Step::Bipolar {
            pre: Affine2 { m: [[0.9, 0.3], [-0.2, 1.1]], t: [0.1, -0.2] },
            pre_inv: Affine2 { m: [[0.9, 0.3], [-0.2, 1.1]], t: [0.1, -0.2] }.inverse().unwrap(),
            w: 1.3,
            shift: 0.7,
            post: Affine2::IDENTITY,
            post_inv: Affine2::IDENTITY,
        };
        let f = FinalMap { steps: vec![step] };
        let whole = View { centre: [0.0, 0.0], radius: 1e6 };
        let mut u = lcg(5);
        for (centre, radius) in [([0.3, 0.2], 0.05), ([1.9, -0.4], 1e-3), ([-0.2, 1.28], 0.03), ([0.0, 0.0], 0.8), ([-2.5, 0.9], 1e-5)] {
            let view = View { centre, radius };
            let d = f.pull_back(view, whole).expect("the view holds plotted points");
            // Preimages of points in the view, found by sampling the
            // plane near the disc.
            let mut inside = 0;
            for _ in 0..20_000 {
                let r = d.radius * 3.0 * u().sqrt();
                let a = u() * std::f64::consts::TAU;
                let x = [d.centre[0] + r * a.cos(), d.centre[1] + r * a.sin()];
                let y = f.forward(x);
                if (y[0] - centre[0]).hypot(y[1] - centre[1]) <= radius {
                    inside += 1;
                    assert!((x[0] - d.centre[0]).hypot(x[1] - d.centre[1]) <= d.radius, "view {centre:?}: {x:?} lands but is outside {d:?}");
                }
            }
            assert!(inside > 0, "view {centre:?}: nothing sampled near {d:?} lands in it");
        }
        assert!(f.pull_back(View { centre: [0.0, 5.0], radius: 0.1 }, whole).is_none(), "a view past the range holds nothing");
    }

    /// **A view holding where far points are plotted pulls back to the
    /// whole attractor**: its preimage is the outside of a circle, which
    /// no disc drawn round its rim holds.
    #[test]
    fn a_view_holding_infinity_pulls_back_to_everything() {
        let step = Step::Bipolar {
            pre: Affine2::IDENTITY,
            pre_inv: Affine2::IDENTITY,
            w: 1.0,
            shift: 0.4,
            post: Affine2::IDENTITY,
            post_inv: Affine2::IDENTITY,
        };
        let f = FinalMap { steps: vec![step] };
        let inf = f.infinity().expect("bipolar plots far points at a point");
        let far = f.forward([300.0, -200.0]);
        assert!((far[0] - inf[0]).hypot(far[1] - inf[1]) < 1e-2, "{far:?} is not near {inf:?}");
        let whole = View { centre: [0.0, 0.0], radius: 500.0 };
        let view = View { centre: [inf[0] + 0.01, inf[1]], radius: 0.05 };
        assert_eq!(f.pull_back(view, whole), Some(whole));
        let away = View { centre: [inf[0] + 0.5, inf[1]], radius: 0.05 };
        assert!(f.pull_back(away, whole).is_some_and(|d| d.radius < 100.0), "a view away from it is bounded");
    }
}

//! A Rhai variation's own inverse (docs/projects/rhai-planners.md, B1).
//!
//! The Flame Attractor walks a flame's attractor backwards, so it needs
//! each variation's exact inverse, and the built-ins carry theirs in Rust
//! (`inverse::INVERSES`). A Rhai variation declares its own beside its
//! forward map:
//!
//! ```rhai
//! const INFO = #{ title: "My Julia", inverse_branches: 2 };
//! fn variation(z, p) { ... }
//! fn inverse(z, p, branch) { ... }   // the preimage on branch 0 .. n-1, or no_preimage()
//! ```
//!
//! **It is checked, at the parameters it is used with.** Forward then
//! inverse at points sampled over a disc must give each point back on
//! some branch ([`check`]); an inverse that does not undo its map is
//! refused with the worst point in the message, so an edit to a twin's
//! forward map that its inverse no longer fits drops the inverse rather
//! than drawing a wrong picture.
//!
//! Where the inverse has no preimage the walk would know nothing of that
//! piece, so a variation whose image is not the whole plane says how far
//! a point lies outside it, `fn image_gap(z, p)` -- a lower bound, 0 or
//! less inside -- as the built-ins' unit discs do (`image_gap`), checked
//! too: no image point reads outside, and none a step `d` away reads
//! farther than `|d|`.
//!
//! A kernel of one (`scene::ifs_analysis::Kernel::User`) is a slot in
//! [`kernel`]'s table: the inverse and one transform's parameter values,
//! with what the GPU reads of them packed ([`UserKernel::packed`]).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use crate::lang::builtins::C64;
use crate::lang::interp::{self, Env, ParamValues, VariationRun};
use crate::lang::ir::Program;

/// The most branches an inverse may declare: each is a row of the Flame
/// Attractor's walk (disc's rings are capped at twelve).
pub const MAX_BRANCHES: u32 = 16;

/// How many arms a forward map that draws is taken at
/// ([`UserInverse::forward_at`]): every draw `(arm + ½)/16`, so a root
/// picked by `floor(n·rand())` meets each of its arms up to a power of 16.
pub const FORWARD_ARMS: u32 = 16;

/// The name mode D's shader calls an inverse by, before the assembler
/// makes it a slot's own (`escape::assembler`).
pub const WGSL_NAME: &str = "ifs_user_inv";

/// The name it calls an image gap by, likewise.
pub const GAP_WGSL_NAME: &str = "ifs_user_gap";

/// The name it calls a forward map by, likewise: a sum's Newton solve
/// steps on it (B2).
pub const FORWARD_WGSL_NAME: &str = "ifs_user_fwd";

/// A variation's inverse, compiled.
pub struct UserInverse {
    /// The variation's key.
    pub key: String,
    /// `fn variation(z, p)`.
    pub forward: Program,
    /// `fn inverse(z, p, branch)`.
    pub inverse: Program,
    /// `INFO.inverse_branches`, 1 when it does not say.
    pub branches: u32,
    /// Whether `variation` draws random numbers: a many-valued map, each
    /// of whose values is an arm (`forward_at`).
    pub forward_draws: bool,
    /// The inverse as mode D's shader calls it, `fn ifs_user_inv(v, base,
    /// branch)` ([`WGSL_NAME`]; `lang::wgsl::inverse_kernel`).
    pub wgsl: String,
    /// `fn image_gap(z, p)`, and as the shader calls it, `fn
    /// ifs_user_gap(v, base, 0.0)` ([`GAP_WGSL_NAME`]), its gap the real
    /// part; none where the variation declares none.
    pub gap: Option<(Program, String)>,
    /// `fn variation` as the shader calls it, `fn ifs_user_fwd(z, base,
    /// 0.0)` ([`FORWARD_WGSL_NAME`]), for a sum's Newton solve (B2); none
    /// for one that draws, or reaches for what only a flame has -- such a
    /// kernel is not summed (`scene::ifs_analysis`).
    pub forward_wgsl: Option<String>,
}

impl UserInverse {
    /// The forward map at `z`, along `arm` (mod [`FORWARD_ARMS`]): every
    /// draw of the call `(arm + ½)/FORWARD_ARMS`, so a many-valued map
    /// (julian's `rand()` picking its root) gives each of its values on
    /// some arm, the same one each time. Seeded draws missed an arm:
    /// eight of them met only one of julia's two, and its ball came out
    /// short.
    pub fn forward_at(&self, params: &ParamValues, z: [f64; 2], arm: u32) -> [f64; 2] {
        let u = ((arm % FORWARD_ARMS) as f64 + 0.5) / FORWARD_ARMS as f64;
        let out = VariationRun::new(&self.forward, params, 1).with_draw(u).call(C64::new(z[0], z[1]), C64::ZERO, 1.0).0;
        [out.re, out.im]
    }

    /// Whether the forward map sends the pre-origin's neighbourhood far
    /// out -- past 100 anywhere on the circle of radius 1e-3 -- as an
    /// inversion does: the ball is then the sample's bulk
    /// (`Kernel::unbounded_at_origin`). Spherical's twin is bounded, by its
    /// 1e-6, at 500: a ball grown to hold that is not the built-in's.
    pub fn unbounded_at_origin(&self, params: &ParamValues) -> bool {
        (0..8).any(|k| {
            let a = std::f64::consts::TAU * k as f64 / 8.0;
            let v = self.forward_at(params, [1e-3 * a.cos(), 1e-3 * a.sin()], 0);
            !(v[0].hypot(v[1]) <= 100.0)
        })
    }

    /// The inverse at `v` on `branch`: its preimage, or one at 1e30 (or
    /// any non-finite point) where it has none.
    pub fn inverse_at(&self, params: &ParamValues, v: [f64; 2], branch: u32) -> [f64; 2] {
        let out = VariationRun::new(&self.inverse, params, 1).with_color(branch as f64).call(C64::new(v[0], v[1]), C64::ZERO, 1.0).0;
        [out.re, out.im]
    }

    /// How far `v` lies outside the forward map's image, a lower bound,
    /// where `fn image_gap` says.
    pub fn gap_at(&self, params: &ParamValues, v: [f64; 2]) -> Option<f64> {
        let (p, _) = self.gap.as_ref()?;
        Some(VariationRun::new(p, params, 1).call(C64::new(v[0], v[1]), C64::ZERO, 1.0).0.re)
    }
}

/// Whether `u` is a preimage at all: finite and short of the 1e30 the
/// kernels (and `no_preimage()`) give where there is none.
pub fn is_preimage(u: [f64; 2]) -> bool {
    u[0].abs() < 1e29 && u[1].abs() < 1e29
}

/// The variation's inverse from its source: `None` when it declares
/// none; why it does not compile otherwise. Checked at its parameters'
/// defaults.
pub fn compile(source: &str, key: &str) -> Option<Result<UserInverse, String>> {
    if !crate::lang::rhai::has_function(source, "inverse") {
        return None;
    }
    Some(compile_declared(source, key))
}

fn compile_declared(source: &str, key: &str) -> Result<UserInverse, String> {
    let first = |d: &[crate::lang::Diag]| {
        d.iter().find(|x| x.severity == crate::lang::Severity::Error).map(|x| x.render(source)).unwrap_or_else(|| "it does not compile".to_string())
    };
    if !crate::lang::rhai::has_function(source, "variation") {
        return Err("an inverse is of a 2D variation, `fn variation(z, p)`".into());
    }
    let forward = crate::lang::rhai::transformation(source).map_err(|d| first(&d))?;
    let inverse = crate::lang::rhai::inverse_transformation(source).map_err(|d| format!("its inverse: {}", first(&d)))?;
    let branches = match crate::lang::rhai::info_number(source, "inverse_branches") {
        None => 1,
        Some(n) if n.fract() == 0.0 && (1.0..=MAX_BRANCHES as f64).contains(&n) => n as u32,
        Some(n) => return Err(format!("`INFO.inverse_branches` is a whole number from 1 to {MAX_BRANCHES}, not {n}")),
    };
    let wgsl = crate::lang::wgsl::inverse_kernel(&inverse, WGSL_NAME)?;
    #[cfg(not(target_arch = "wasm32"))]
    crate::lang::wgsl::validate_inverse_kernel(&wgsl).map_err(|e| format!("the compiler produced invalid WGSL for the inverse (a bug): {e}"))?;
    let gap = if crate::lang::rhai::has_function(source, "image_gap") {
        let p = crate::lang::rhai::image_gap_transformation(source).map_err(|d| format!("its image gap: {}", first(&d)))?;
        let w = crate::lang::wgsl::inverse_kernel(&p, GAP_WGSL_NAME).map_err(|e| e.replace("an inverse", "an image gap"))?;
        #[cfg(not(target_arch = "wasm32"))]
        crate::lang::wgsl::validate_inverse_kernel(&w).map_err(|e| format!("the compiler produced invalid WGSL for the image gap (a bug): {e}"))?;
        Some((p, w))
    } else {
        None
    };
    let forward_draws = crate::lang::wgsl::variation_shape(&forward).is_ok_and(|s| s.rng);
    let forward_wgsl = crate::lang::wgsl::inverse_kernel(&forward, FORWARD_WGSL_NAME).ok();
    #[cfg(not(target_arch = "wasm32"))]
    let forward_wgsl = forward_wgsl.filter(|w| crate::lang::wgsl::validate_inverse_kernel(w).is_ok());
    let inv = UserInverse { key: key.to_string(), forward, inverse, branches, forward_draws, wgsl, gap, forward_wgsl };
    check(&inv, &ParamValues::defaults(&inv.forward))?;
    Ok(inv)
}

/// **Forward then inverse gives the point back.** At 400 points over the
/// disc of radius 4 -- past bubble's fold at 2, where its outer branch
/// begins -- each sent forward (on an arm of its own, for a
/// many-valued map), some branch of the inverse must return it to a
/// relative 1e-6; points the forward map sends to infinity or past 1e6
/// are skipped. One in fifty may miss -- a point within rounding of a
/// branch cut takes the next branch's value -- and no more: an inverse
/// wrong over a fiftieth of the disc is refused, the worst point named.
pub fn check(inv: &UserInverse, params: &ParamValues) -> Result<(), String> {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut tried, mut missed) = (0u32, 0u32);
    let mut gap_missed = 0u32;
    let mut gap_worst: Option<(f64, [f64; 2], f64)> = None;
    let mut worst: Option<(f64, [f64; 2], [f64; 2], u32, [f64; 2])> = None;
    for i in 0..400u32 {
        let (a, r) = (std::f64::consts::TAU * next(), 4.0 * next().sqrt());
        let z = [r * a.cos(), r * a.sin()];
        let v = inv.forward_at(params, z, i);
        if !(v[0].is_finite() && v[1].is_finite()) || v[0].hypot(v[1]) > 1e6 {
            continue;
        }
        tried += 1;
        let scale = 1.0f64.max(z[0].hypot(z[1]));
        // An image point is inside the image, and one `d` from it is no
        // farther outside than `|d|`.
        if inv.gap.is_some() {
            let tol = 1e-6 * scale;
            for (k, rho) in [0.0, 0.01, 0.1, 0.5].into_iter().enumerate() {
                let a = std::f64::consts::TAU * next();
                let q = [v[0] + rho * a.cos(), v[1] + rho * a.sin()];
                let g = inv.gap_at(params, q).unwrap_or(f64::NAN);
                if !(g <= rho + tol) {
                    gap_missed += 1;
                    if gap_worst.is_none_or(|w: (f64, [f64; 2], f64)| !(g - rho <= w.0)) {
                        gap_worst = Some((g - rho, q, if k == 0 { 0.0 } else { rho }));
                    }
                }
            }
        }
        let mut best = (f64::INFINITY, 0, [f64::NAN; 2]);
        for b in 0..inv.branches {
            let u = inv.inverse_at(params, v, b);
            let e = if is_preimage(u) { (u[0] - z[0]).hypot(u[1] - z[1]) / scale } else { f64::INFINITY };
            if e < best.0 || best.0.is_nan() {
                best = (e, b, u);
            }
        }
        if !(best.0 <= 1e-6) {
            missed += 1;
            if worst.is_none_or(|w| !(best.0 <= w.0)) {
                worst = Some((best.0, z, v, best.1, best.2));
            }
        }
    }
    if tried < 100 {
        return Err(format!("its forward map gave {tried} finite points of 400 over the disc of radius 4: too few to check its inverse"));
    }
    if missed * 50 > tried {
        let (_, z, v, b, u) = worst.expect("a miss");
        let near = if is_preimage(u) { format!("the nearest, on branch {b}, is ({:.6}, {:.6})", u[0], u[1]) } else { "every branch says it has none".to_string() };
        return Err(format!(
            "its inverse does not undo it: `variation` sends ({:.6}, {:.6}) to ({:.6}, {:.6}), and no branch of `inverse` gives it back ({near}); {missed} of {tried} points missed",
            z[0], z[1], v[0], v[1]
        ));
    }
    if gap_missed * 50 > tried * 4 {
        let (over, q, rho) = gap_worst.expect("a miss");
        let at = if rho > 0.0 { format!("{rho} from an image point") } else { "an image point".to_string() };
        return Err(format!(
            "its image gap is not a lower bound: at ({:.6}, {:.6}), {at}, it reads {over:.6} too far; {gap_missed} of {} readings missed",
            q[0],
            q[1],
            tried * 4
        ));
    }
    Ok(())
}

/// The inverse of the variation `key`, compiled and checked once per key
/// (a key is its source's, so an edit is another): a Rhai variation's
/// own, or for a built-in its twin's -- so writing a twin's inverse gives
/// the built-in one in the Flame Attractor, under its own name and
/// parameters, where it has none in Rust (`scene::ifs_analysis` asks
/// this only then). `None` when it declares none; why it is refused
/// otherwise.
pub fn of(key: &str) -> Option<Result<Arc<UserInverse>, String>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Result<Arc<UserInverse>, String>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().ok()?.get(key) {
        return hit.clone();
    }
    let source = crate::variations::custom::source_of(key).or_else(|| crate::variations::twins::source(key).map(str::to_string));
    let built = source.and_then(|s| compile(&s, key)).map(|r| r.map(Arc::new));
    if let Ok(mut c) = cache.lock() {
        c.insert(key.to_string(), built.clone());
    }
    built
}

/// The variation `key`'s name, kept for the session: a sum names its
/// kernel by a `&'static str` (`SumMap2::kind`), and a key is short and
/// asked for once per variation.
pub fn static_name(key: &str) -> &'static str {
    static NAMES: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let mut names = NAMES.get_or_init(Default::default).lock().expect("the names");
    names.entry(key.to_string()).or_insert_with(|| Box::leak(key.to_string().into_boxed_str()))
}

/// An inverse at one transform's parameter values: a user kernel.
pub struct UserKernel {
    pub inverse: Arc<UserInverse>,
    /// The values the interpreter runs both maps at.
    pub params: ParamValues,
    /// What mode D's shader reads through `get_param(base, 0u, slot)`: the
    /// parameters' slots, then the inverse's `prepare` values
    /// (`interp::globals`).
    pub packed: Vec<f32>,
}

impl UserKernel {
    pub fn forward(&self, z: [f64; 2], arm: u32) -> [f64; 2] {
        self.inverse.forward_at(&self.params, z, arm)
    }

    pub fn inverse(&self, v: [f64; 2], branch: u32) -> [f64; 2] {
        self.inverse.inverse_at(&self.params, v, branch)
    }

    pub fn unbounded_at_origin(&self) -> bool {
        self.inverse.unbounded_at_origin(&self.params)
    }

    /// How far `v` lies outside the image, where the variation says.
    pub fn gap(&self, v: [f64; 2]) -> Option<f64> {
        self.inverse.gap_at(&self.params, v)
    }
}

fn table() -> &'static RwLock<(Vec<Arc<UserKernel>>, HashMap<(String, Vec<u32>), Result<u32, String>>)> {
    static TABLE: OnceLock<RwLock<(Vec<Arc<UserKernel>>, HashMap<(String, Vec<u32>), Result<u32, String>>)>> = OnceLock::new();
    TABLE.get_or_init(Default::default)
}

/// The slot of `inv` at the parameter slots `slots` (as a flame packs
/// them), made and checked ([`check`]) the first time those values are
/// asked for. The same values give the same slot, so a kernel compares
/// equal to itself across analyses; an inverse that does not undo its
/// map at these values is refused here.
pub fn kernel_slot(inv: &Arc<UserInverse>, slots: &[f32]) -> Result<u32, String> {
    let id = (inv.key.clone(), slots.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    if let Some(hit) = table().read().ok().and_then(|t| t.1.get(&id).cloned()) {
        return hit;
    }
    let params = ParamValues::from_slots(&inv.forward, slots);
    let result = check(inv, &params).map(|()| {
        let mut packed = slots.to_vec();
        packed.extend(interp::globals(&inv.inverse, &params, &Env::at(C64::ZERO, 1)));
        UserKernel { inverse: inv.clone(), params, packed }
    });
    let mut t = table().write().expect("the user kernels' table");
    if let Some(hit) = t.1.get(&id) {
        return hit.clone();
    }
    let slot = result.map(|k| {
        t.0.push(Arc::new(k));
        (t.0.len() - 1) as u32
    });
    t.1.insert(id, slot.clone());
    slot
}

/// The user kernel in `slot` ([`kernel_slot`]).
pub fn kernel(slot: u32) -> Arc<UserKernel> {
    table().read().expect("the user kernels' table").0[slot as usize].clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bubble's twin with its inverse, as the twins will carry it.
    pub(crate) const BUBBLE: &str = r#"
const PARAMS = #{};
const INFO = #{ id: "6a3f1c2e-0000-4000-8000-00000000b0b1", title: "Bubble Inverse Test", inverse_branches: 2 };
fn variation(z, p) {
    let r = 4.0 / (z.norm_sqr() + 4.0);
    z * r
}
fn inverse(z, p, branch) {
    let x = z.norm_sqr();
    if x > 1.0 {
        return no_preimage();
    }
    let root = sqrt(max(0.0, 1.0 - x));
    if branch == 0 {
        z * (2.0 / (1.0 + root))
    } else {
        z * (2.0 * (1.0 + root) / max(x, 1.0e-30))
    }
}
"#;

    /// **A user kernel is its built-in's**: bubble's twin's inverse, as
    /// the analysis runs it (`Kernel::User`), against `Kernel::Bubble` --
    /// the preimage, its Jacobian and the local σ the walk bounds with.
    #[test]
    fn a_user_kernel_is_its_builtins() {
        use crate::scene::ifs_analysis::Kernel;
        let src = crate::lang::rhai::with_own_id(crate::variations::twins::source("bubble").expect("a twin"), &crate::scripted::mint_id());
        let inv = Arc::new(compile(&src, "bubble_user_kernel").expect("declared").expect("checks"));
        let user = Kernel::User { slot: kernel_slot(&inv, &[]).expect("a slot") };
        for v in [[0.1, 0.2], [-0.5, 0.3], [0.7, -0.6], [0.02, -0.01]] {
            for b in 0..2 {
                let (got, want) = (user.inverse(v, b), Kernel::Bubble.inverse(v, b));
                assert!((got[0] - want[0]).hypot(got[1] - want[1]) < 1e-9 * (1.0 + want[0].hypot(want[1])), "{v:?} {b}: {got:?} {want:?}");
                let (jg, jw) = (user.inverse_jacobian(v, b).expect("a Jacobian"), Kernel::Bubble.inverse_jacobian(v, b).expect("a Jacobian"));
                for i in 0..2 {
                    for j in 0..2 {
                        assert!((jg[i][j] - jw[i][j]).abs() < 1e-6 * (1.0 + jw[i][j].abs()), "{v:?} {b}: {jg:?} {jw:?}");
                    }
                }
                let (sg, sw) = (user.local_sigma_factor(v, b), Kernel::Bubble.local_sigma_factor(v, b));
                assert!((sg - sw).abs() < 1e-6 * sw.max(1e-9), "{v:?} {b}: sigma {sg} {sw}");
            }
        }
        for z in [[0.3, -0.4], [2.5, 1.0]] {
            let (got, want) = (user.forward(z, 0), Kernel::Bubble.forward(z, 0));
            assert!((got[0] - want[0]).hypot(got[1] - want[1]) < 1e-12, "{z:?}: {got:?} {want:?}");
        }
    }

    #[test]
    fn a_true_inverse_is_taken() {
        let inv = compile(BUBBLE, "bubble_test").expect("declared").expect("compiles and checks");
        assert_eq!(inv.branches, 2);
        let params = ParamValues::defaults(&inv.forward);
        let z = [0.7, -1.9];
        let v = inv.forward_at(&params, z, 0);
        let back = (0..2).map(|b| inv.inverse_at(&params, v, b)).find(|u| (u[0] - z[0]).hypot(u[1] - z[1]) < 1e-12);
        assert!(back.is_some(), "{v:?}");
        assert!(!is_preimage(inv.inverse_at(&params, [1.5, 0.0], 0)), "past the image, no preimage");
        assert!(inv.wgsl.contains("fn ifs_user_inv("), "{}", inv.wgsl);
    }

    #[test]
    fn a_wrong_inverse_is_refused_with_its_worst_point() {
        let wrong = BUBBLE.replace("z * (2.0 / (1.0 + root))", "z * (2.1 / (1.0 + root))");
        let err = compile(&wrong, "bubble_wrong").expect("declared").err().expect("refused");
        assert!(err.contains("does not undo it"), "{err}");
        // One branch alone does not reach the outer half.
        let one = BUBBLE.replace("inverse_branches: 2", "inverse_branches: 1");
        assert!(compile(&one, "bubble_one").expect("declared").is_err());
    }

    #[test]
    fn an_inverse_reaches_for_nothing_only_a_flame_has() {
        let random = BUBBLE.replace("let x = z.norm_sqr();", "let x = z.norm_sqr() + 0.0 * rand();");
        let err = compile(&random, "bubble_rand").expect("declared").err().expect("refused");
        assert!(err.contains("random"), "{err}");
        let misplaced = BUBBLE.replace("z * r\n}", "if z.re > 9.0 { no_preimage() } else { z * r }\n}");
        assert!(compile(&misplaced, "bubble_misplaced").expect("declared").err().expect("refused").contains("no_preimage"));
        assert!(compile(&BUBBLE.replace("fn inverse(z, p, branch)", "fn inverse_not(z, p, branch)"), "none").is_none());
    }
}

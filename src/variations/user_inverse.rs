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

/// The grid a forward map's draws are taken on
/// ([`UserInverse::forward_at`]; docs/projects/rhai-planners.md, C1): code
/// `c` sets every draw of the call to `(c + ½)/256` -- the interpreter's
/// fixed draw and the replay's forced one (`bound::force_arms`) alike,
/// exact in both. An arm is named by such a code ([`UserKernel::arms`]),
/// which is what a word's symbol carries for it.
pub const DRAW_GRID: u32 = 256;

/// How many draws a forward map that draws is taken at where its arms are
/// not known -- its checks, and a draw that picks no arm -- spread over
/// the grid ([`spread`]): a root picked by `floor(n·rand())` meets each of
/// its arms up to a power of 16.
pub const FORWARD_ARMS: u32 = 16;

/// The `i`th of the [`FORWARD_ARMS`] draws spread over the grid, as a code.
pub fn spread(i: u32) -> u32 {
    let step = DRAW_GRID / FORWARD_ARMS;
    (i % FORWARD_ARMS) * step + step / 2
}

/// One of a forward map's arms (C1): the image its draws pick over some
/// interval of `u`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arm {
    /// The grid draw it is taken at, and forced at ([`DRAW_GRID`]).
    pub code: u32,
    /// The share of draws that pick it.
    pub share: f64,
}

/// The name mode D's shader calls an inverse by, before the assembler
/// makes it a slot's own (`escape::assembler`).
pub const WGSL_NAME: &str = "ifs_user_inv";

/// The name it calls an image gap by, likewise.
pub const GAP_WGSL_NAME: &str = "ifs_user_gap";

/// The name it calls a forward map by, likewise: a sum's Newton solve
/// steps on it (B2).
pub const FORWARD_WGSL_NAME: &str = "ifs_user_fwd";

/// The name the offset replay calls a forward difference by, before
/// `forward_delta::replay_delta_wgsl` makes it a key's own (C3).
pub const DIFFERENCE_WGSL_NAME: &str = "fd_user_dif";

/// The name mode D's delta walk calls an inverse's difference by, before
/// the assembler makes it a slot's own (D3).
pub const INVERSE_DIFFERENCE_WGSL_NAME: &str = "ifs_user_dinv";

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
    /// u)` ([`FORWARD_WGSL_NAME`]), for a sum's Newton solve (B2): one that
    /// draws reads every draw as `u`, its arm's (D2; `delta::draw_as_color`).
    /// None for one that reaches for what only a flame has, or draws and
    /// reads its colour too -- such a kernel is not summed in mode D
    /// (`scene::ifs_analysis`).
    pub forward_wgsl: Option<String>,
    /// Its forward difference `F(Z + d) - F(Z)`, derived from `variation`
    /// without the cancellation (`lang::delta::variation_difference`;
    /// rhai-planners C3): the offset replay's step through it. None where
    /// it does not derive, and the replay keeps its steps absolute.
    pub difference: Option<Program>,
    /// The difference as the offset replay calls it, `fn fd_user_dif(z, d,
    /// base, u)` ([`DIFFERENCE_WGSL_NAME`]; `lang::wgsl::difference_kernel`).
    pub difference_wgsl: Option<String>,
    /// Its INVERSE's difference `G(V + d) - G(V)` along a branch, derived as
    /// the forward's is, the branch the uniform channel (D3): mode D's delta
    /// walk carries a pixel's offset through it. None where it does not
    /// derive, and a lineage through it rebases.
    pub inverse_difference: Option<Program>,
    /// As the delta walk's shader calls it, `fn ifs_user_dinv(v, d, base,
    /// branch)` ([`INVERSE_DIFFERENCE_WGSL_NAME`]).
    pub inverse_difference_wgsl: Option<String>,
    /// The inverse as one pass of a loop, by branch, for a run in big
    /// floats (`lang::bigeval`; [`UserKernel::big_inverse`]).
    big: Mutex<HashMap<u32, Arc<Program>>>,
}

impl UserInverse {
    /// The forward map at `z` at the grid draw `code` (mod [`DRAW_GRID`]):
    /// every draw of the call `(code + ½)/256`, so a many-valued map
    /// (julian's `rand()` picking its root) gives one of its values, the
    /// same one each time. Seeded draws missed an arm: eight of them met
    /// only one of julia's two, and its ball came out short.
    pub fn forward_at(&self, params: &ParamValues, z: [f64; 2], code: u32) -> [f64; 2] {
        self.forward_u(params, z, ((code % DRAW_GRID) as f64 + 0.5) / DRAW_GRID as f64)
    }

    /// The forward map at `z` with every draw of the call `u`, in [0, 1).
    pub fn forward_u(&self, params: &ParamValues, z: [f64; 2], u: f64) -> [f64; 2] {
        let out = VariationRun::new(&self.forward, params, 1).with_draw(u).call(C64::new(z[0], z[1]), C64::ZERO, 1.0).0;
        [out.re, out.im]
    }

    /// Whether the forward map sends the pre-origin's neighbourhood far
    /// out, as an inversion does: the ball is then the sample's bulk
    /// (`Kernel::unbounded_at_origin`), and its image of the ball has a
    /// hole (`UserKernel::hole_radius`). Either past 100 anywhere on the
    /// circle of radius 1e-3 -- spherical's twin is bounded, by its 1e-6,
    /// at 500, and a ball grown to hold that is not the built-in's -- or
    /// rising toward the origin as a pole does: each of the circles of
    /// radius 1e-2, 1e-4, 1e-6 and 1e-8 wholly farther out than the last,
    /// by the same factor each time. That is a root of negative distance
    /// at any power, as the built-in's `d < 0` is: at power 2 its modulus
    /// at 1e-3 is 32, at power 15 it is 1.6. `log`'s twin, unbounded there
    /// too but by a logarithm, is not an inversion.
    pub fn unbounded_at_origin(&self, params: &ParamValues) -> bool {
        unbounded_by_rings(|r| {
            moduli((0..8).map(|k| {
                let a = std::f64::consts::TAU * k as f64 / 8.0;
                let v = self.forward_at(params, [r * a.cos(), r * a.sin()], 0);
                v[0].hypot(v[1])
            }))
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
    let forward_wgsl = if forward_draws {
        crate::lang::delta::draw_as_color(&forward).and_then(|p| crate::lang::wgsl::inverse_kernel(&p, FORWARD_WGSL_NAME).ok())
    } else {
        crate::lang::wgsl::inverse_kernel(&forward, FORWARD_WGSL_NAME).ok()
    };
    #[cfg(not(target_arch = "wasm32"))]
    let forward_wgsl = forward_wgsl.filter(|w| crate::lang::wgsl::validate_inverse_kernel(w).is_ok());
    let difference = crate::lang::delta::variation_difference(&forward);
    let difference_wgsl = difference.as_ref().and_then(|p| crate::lang::wgsl::difference_kernel(p, DIFFERENCE_WGSL_NAME).ok());
    #[cfg(not(target_arch = "wasm32"))]
    let difference_wgsl = difference_wgsl.filter(|w| crate::lang::wgsl::validate_inverse_kernel(w).is_ok());
    // One without the other is none: the walk takes a form both run.
    let difference = difference.filter(|_| difference_wgsl.is_some());
    let inverse_difference = crate::lang::delta::variation_difference(&inverse);
    let inverse_difference_wgsl = inverse_difference.as_ref().and_then(|p| crate::lang::wgsl::difference_kernel(p, INVERSE_DIFFERENCE_WGSL_NAME).ok());
    #[cfg(not(target_arch = "wasm32"))]
    let inverse_difference_wgsl = inverse_difference_wgsl.filter(|w| crate::lang::wgsl::validate_inverse_kernel(w).is_ok());
    let inverse_difference = inverse_difference.filter(|_| inverse_difference_wgsl.is_some());
    let inv = UserInverse {
        key: key.to_string(),
        forward,
        inverse,
        branches,
        forward_draws,
        wgsl,
        gap,
        forward_wgsl,
        difference,
        difference_wgsl,
        inverse_difference,
        inverse_difference_wgsl,
        big: Mutex::new(HashMap::new()),
    };
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
        let v = inv.forward_at(params, z, spread(i));
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
    /// The parameters' slots, as the flame packs them.
    slots: Vec<f32>,
    /// [`Self::hole_radius`] by the bits of the reach it was asked at.
    holes: Mutex<HashMap<u64, f64>>,
    /// [`Self::arms`], read once.
    arms: OnceLock<Option<Vec<Arm>>>,
}

/// Which of a kernel's programs a kept run runs ([`with_run`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Which {
    Forward,
    Inverse,
    Gap,
    Difference,
    InverseDifference,
    Forward3,
    Inverse3,
}

thread_local! {
    /// Each kernel's runs on this thread, by the kernel and the program:
    /// a kernel is the table's for the session (`kernel_slot`), so its
    /// programs and values outlive any run of them.
    static RUNS: std::cell::RefCell<HashMap<(usize, Which), VariationRun<'static>>> = std::cell::RefCell::new(HashMap::new());
}

/// `f` on this thread's run of `prog` at `params`, for the kernel at
/// `key`, made the first time and its locals zeroed each call
/// (rhai-planners D1): building a run afresh was half of every call.
fn with_run<R>(key: usize, which: Which, prog: &'static Program, params: &'static ParamValues, f: impl FnOnce(&mut VariationRun<'static>) -> R) -> R {
    RUNS.with(|runs| {
        let mut runs = runs.borrow_mut();
        let run = runs.entry((key, which)).or_insert_with(|| VariationRun::new(prog, params, 1));
        run.reset_locals();
        f(run)
    })
}

impl UserKernel {
    /// The forward map at the grid draw `code` ([`UserInverse::forward_at`]).
    pub fn forward(&'static self, z: [f64; 2], code: u32) -> [f64; 2] {
        self.forward_u(z, ((code % DRAW_GRID) as f64 + 0.5) / DRAW_GRID as f64)
    }

    /// The forward map with every draw of the call `u`.
    pub fn forward_u(&'static self, z: [f64; 2], u: f64) -> [f64; 2] {
        with_run(self as *const _ as usize, Which::Forward, &self.inverse.forward, &self.params, |run| {
            run.set_draw(u);
            let out = run.call(C64::new(z[0], z[1]), C64::ZERO, 1.0).0;
            [out.re, out.im]
        })
    }


    /// Its forward map's arms (docs/projects/rhai-planners.md, C1): one, of
    /// every draw, for a map that draws nothing; for one that draws, the
    /// images its draws pick among (julian's twin's `floor(n·rand())`
    /// roots), or `None` when they fall into more than
    /// [`MAX_ARMS`](crate::variations::bound::MAX_ARMS) -- a blur's
    /// continuous draw -- which no word can name.
    ///
    /// Read off the map ([`find_arms`]): every draw of a call set to each
    /// step of the grid in turn, at six points; a run of steps with the
    /// same images at all six is an arm's, runs with the same images one
    /// arm, its share their lengths with each edge bisected -- so a power
    /// of 2.5 shares 0.4, 0.4, 0.2, as its draws do -- and its code the
    /// middle step of its longest run.
    pub fn arms(&'static self) -> Option<&'static [Arm]> {
        self.arms.get_or_init(|| find_arms(|z, u| self.forward_u(z, u), self.inverse.forward_draws)).as_deref()
    }

    /// `F(z + d) - F(z)` at the grid draw `code`, without forming the
    /// difference ([`UserInverse::difference`]); none where it has no form.
    pub fn forward_difference(&'static self, z: [f64; 2], d: [f64; 2], code: u32) -> Option<[f64; 2]> {
        let p = self.inverse.difference.as_ref()?;
        let u = ((code % DRAW_GRID) as f64 + 0.5) / DRAW_GRID as f64;
        Some(with_run(self as *const _ as usize, Which::Difference, p, &self.params, |run| {
            run.set_color(u);
            let out = run.call(C64::new(z[0], z[1]), C64::new(d[0], d[1]), 1.0).0;
            [out.re, out.im]
        }))
    }

    /// `G(v + d) - G(v)` along `branch`, without forming the difference
    /// ([`UserInverse::inverse_difference`]); none where it has no form.
    pub fn inverse_difference(&'static self, v: [f64; 2], d: [f64; 2], branch: u32) -> Option<[f64; 2]> {
        let p = self.inverse.inverse_difference.as_ref()?;
        Some(with_run(self as *const _ as usize, Which::InverseDifference, p, &self.params, |run| {
            run.set_color(branch as f64);
            let out = run.call(C64::new(v[0], v[1]), C64::new(d[0], d[1]), 1.0).0;
            [out.re, out.im]
        }))
    }

    /// The inverse at `v` along `branch` in big floats, `v`'s width: one
    /// pass of the inverse as a loop, run by `lang::bigeval` (D3) -- the
    /// Flame Attractor's handover past f64. None where a big float has no
    /// answer (a division by zero, a root off its domain) or there is no
    /// preimage.
    pub fn big_inverse(&'static self, v: &[crate::escape::bigfloat::BigFloat; 2], branch: u32) -> Option<[crate::escape::bigfloat::BigFloat; 2]> {
        let prog = {
            let mut big = self.inverse.big.lock().ok()?;
            big.entry(branch).or_insert_with(|| Arc::new(crate::lang::delta::one_pass(&self.inverse.inverse, branch as f64))).clone()
        };
        let n = v[0].n_limbs().max(v[1].n_limbs());
        let pixel = crate::escape::bigfloat::BigComplex { re: v[0].clone(), im: v[1].clone() };
        let mut m = crate::lang::bigeval::BigMachine::new(prog, self.params.clone(), Env::at(C64::ZERO, 1), pixel, n).ok()?;
        m.step().ok()?;
        let z = m.z();
        Some([z.re.clone(), z.im.clone()])
    }

    /// What the offset replay's difference reads through `get_param(base,
    /// 0u, slot)`: the parameters' slots, then the difference's `prepare`
    /// values -- the forward map's, which it derives from.
    pub fn difference_values(&self) -> Vec<f32> {
        let Some(p) = &self.inverse.difference else { return Vec::new() };
        let mut v = self.slots.clone();
        v.extend(interp::globals(p, &self.params, &Env::at(C64::ZERO, 1)));
        v
    }

    /// The grid draws the analysis takes its forward map at: its arms', or,
    /// where its draws pick no arms, [`FORWARD_ARMS`] spread over the grid.
    pub fn draws(&'static self) -> Vec<u32> {
        match self.arms() {
            Some(arms) => arms.iter().map(|a| a.code).collect(),
            None => (0..FORWARD_ARMS).map(spread).collect(),
        }
    }

    pub fn inverse(&'static self, v: [f64; 2], branch: u32) -> [f64; 2] {
        with_run(self as *const _ as usize, Which::Inverse, &self.inverse.inverse, &self.params, |run| {
            run.set_color(branch as f64);
            let out = run.call(C64::new(v[0], v[1]), C64::ZERO, 1.0).0;
            [out.re, out.im]
        })
    }

    pub fn unbounded_at_origin(&self) -> bool {
        self.inverse.unbounded_at_origin(&self.params)
    }

    /// How far `v` lies outside the image, where the variation says.
    pub fn gap(&'static self, v: [f64; 2]) -> Option<f64> {
        let (p, _) = self.inverse.gap.as_ref()?;
        Some(with_run(self as *const _ as usize, Which::Gap, p, &self.params, |run| run.call(C64::new(v[0], v[1]), C64::ZERO, 1.0).0.re))
    }

    /// The hole its image of the disc `|z| <= r_pre` leaves about the
    /// origin, as `Kernel::hole_radius` gives the inversions' -- where the
    /// forward map is unbounded at the origin, as theirs are; 0 for any
    /// other kernel, as for any other built-in.
    ///
    /// **Sampled**: the least `|K(z)|` over sixteen rings of the disc at
    /// 128 angles each, every arm, less a thousandth -- and none where the
    /// inverse finds a preimage in the disc for a point well inside that.
    /// An inversion's least
    /// is on the rim and its modulus the same all round it (spherical's,
    /// a root's), so there the sample is the exact hole and the walk the
    /// built-in's; a kernel that dips between samples is read a little
    /// past its hole there. Kept by reach, since the Extent asks again at
    /// every radius it is dragged through.
    pub fn hole_radius(&'static self, r_pre: f64) -> f64 {
        if !(r_pre > 0.0) || !self.unbounded_at_origin() {
            return 0.0;
        }
        if let Some(&h) = self.holes.lock().expect("the holes").get(&r_pre.to_bits()) {
            return h;
        }
        let draws = self.draws();
        let mut least = f64::INFINITY;
        for ring in 1..=16 {
            let r = r_pre * ring as f64 / 16.0;
            for k in 0..128 {
                let a = std::f64::consts::TAU * k as f64 / 128.0;
                for &code in &draws {
                    let v = self.forward([r * a.cos(), r * a.sin()], code);
                    // A sample that is not a number says nothing of where
                    // the image is, so it cannot make a hole; an infinite
                    // one is far from it.
                    least = least.min(if v[0].is_nan() || v[1].is_nan() { 0.0 } else { v[0].hypot(v[1]) });
                }
            }
        }
        let mut h = if least.is_finite() { least * (1.0 - 1e-3) } else { 0.0 };
        // A hole is where the INVERSE gives no preimage in the disc. A map
        // unbounded along one line only -- hyperbolic's `x / |z|^2`, its
        // `y` kept -- reads one off the rings, which never come near the
        // origin, and has none: its inverse takes `(0, y)` to itself.
        // Spherical's twin, whose 1e-6 brings the origin's neighbourhood
        // back in, keeps its hole: its inverse gives the outer preimage
        // alone, as the built-in kernel's (rhai-planners E2).
        'probe: for f in [0.5, 1e-1, 1e-2, 1e-4] {
            for k in 0..64 {
                let a = std::f64::consts::TAU * (k as f64 + 0.5) / 64.0;
                let v = [h * f * a.cos(), h * f * a.sin()];
                for branch in 0..self.inverse.branches {
                    let u = self.inverse(v, branch);
                    if is_preimage(u) && u[0].hypot(u[1]) <= r_pre {
                        h = 0.0;
                        break 'probe;
                    }
                }
            }
        }
        self.holes.lock().expect("the holes").insert(r_pre.to_bits(), h);
        h
    }
}

type Table = (Vec<&'static UserKernel>, HashMap<(String, Vec<u32>), Result<u32, String>>);

/// The kernels, each kept for the session (leaked: the table never let
/// one go), so a run of one can borrow it ([`with_run`]).
fn table() -> &'static RwLock<Table> {
    static TABLE: OnceLock<RwLock<Table>> = OnceLock::new();
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
        UserKernel { inverse: inv.clone(), params, packed, slots: slots.to_vec(), holes: Default::default(), arms: OnceLock::new() }
    });
    let mut t = table().write().expect("the user kernels' table");
    if let Some(hit) = t.1.get(&id) {
        return hit.clone();
    }
    let slot = result.map(|k| {
        t.0.push(Box::leak(Box::new(k)));
        (t.0.len() - 1) as u32
    });
    t.1.insert(id, slot.clone());
    slot
}

thread_local! {
    /// This thread's copy of the kernels: the table only grows and keeps
    /// every kernel for the session, so a copy stays true, and the walk's
    /// threads stop meeting at the table's lock -- 40% of a call's time,
    /// measured with every core calling.
    static KERNELS: std::cell::RefCell<Vec<&'static UserKernel>> = const { std::cell::RefCell::new(Vec::new()) };
    static KERNELS3: std::cell::RefCell<Vec<&'static UserKernel3>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The user kernel in `slot` ([`kernel_slot`]).
pub fn kernel(slot: u32) -> &'static UserKernel {
    KERNELS.with(|k| {
        if let Some(&hit) = k.borrow().get(slot as usize) {
            return hit;
        }
        let all = table().read().expect("the user kernels' table").0.clone();
        let hit = all[slot as usize];
        *k.borrow_mut() = all;
        hit
    })
}

#[cfg(test)]
thread_local! {
    /// Off: the handover takes a user kernel's inverse in f64, lifted, as
    /// it did before [`UserKernel::big_inverse`] -- the control of the gate
    /// that proves the big floats matter (`a_twins_flame_attractor_holds_at_depth`).
    pub(crate) static BIG_INVERSE: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// The offset replay's name for the variation `key`'s difference: a row
/// names it (`forward_delta`'s kernel 9) by this, and the shader switches on
/// it -- the key's own, so a shader that kept a variation the flame dropped
/// (the sticky superset) still picks the right one. FNV-1a, to 24 bits, so
/// it rides a row's `f32` exactly.
pub fn difference_id(key: &str) -> u32 {
    let h = key.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193));
    h & 0x00ff_ffff
}

/// Whether the variation `key` is a Rhai variation whose forward map draws
/// and that has an inverse: one whose draws a word names by an arm
/// (`UserKernel::arms`), so a flame with it is the inverse walk's
/// (`scene::cylinder`) and a replay forces its draws
/// (`bound::force_arms`). A built-in's arms are `bound::ARMED`'s; an
/// unedited twin that borrows its built-in (`twins::planner_name`, julia's)
/// is planned and replayed as the built-in is.
pub fn draws_arms(key: &str) -> bool {
    crate::variations::bound::arms_for(key).is_none()
        && crate::variations::custom::is_registered(key)
        && crate::variations::twins::planner_name(key) == key
        && matches!(of(key), Some(Ok(inv)) if inv.forward_draws)
}

/// The arms of a forward map `f(z, u)` -- every draw of a call `u` -- read
/// off it ([`UserKernel::arms`]); `draws` false is a map that draws
/// nothing, one arm.
fn find_arms(f: impl Fn([f64; 2], f64) -> [f64; 2], draws: bool) -> Option<Vec<Arm>> {
    if !draws {
        return Some(vec![Arm { code: 0, share: 1.0 }]);
    }
    // Off every axis and circle a twin's arms might meet on.
    const PROBES: [[f64; 2]; 6] = [[0.37, 0.21], [-0.83, 0.45], [1.31, -0.66], [-0.24, -1.87], [2.63, 0.94], [0.06, 0.61]];
    let at = |u: f64| -> Vec<[f64; 2]> { PROBES.iter().map(|&z| f(z, u)).collect() };
    let close = |p: &[f64; 2], q: &[f64; 2]| {
        (p[0].to_bits() == q[0].to_bits() && p[1].to_bits() == q[1].to_bits()) || (p[0] - q[0]).hypot(p[1] - q[1]) <= 1e-9 * (1.0 + p[0].hypot(p[1]))
    };
    let same = |a: &[[f64; 2]], b: &[[f64; 2]]| a.iter().zip(b).all(|(p, q)| close(p, q));
    let u_of = |c: u32| (c as f64 + 0.5) / DRAW_GRID as f64;
    let images: Vec<Vec<[f64; 2]>> = (0..DRAW_GRID).map(|c| at(u_of(c))).collect();
    // Runs of steps with the same images, `(start, end, arm)`; an arm is
    // named by the images of its first step.
    let mut runs: Vec<(u32, u32, usize)> = Vec::new();
    let mut first: Vec<u32> = Vec::new();
    for c in 0..DRAW_GRID {
        if let Some(run) = runs.last_mut() {
            if same(&images[c as usize], &images[run.0 as usize]) {
                run.1 = c + 1;
                continue;
            }
        }
        let arm = match first.iter().position(|&s| same(&images[c as usize], &images[s as usize])) {
            Some(a) => a,
            None => {
                first.push(c);
                if first.len() > crate::variations::bound::MAX_ARMS as usize {
                    return None;
                }
                first.len() - 1
            }
        };
        runs.push((c, c + 1, arm));
    }
    // A draw that changes nothing: one arm, named 0 as every map with one.
    if first.len() == 1 {
        return Some(vec![Arm { code: 0, share: 1.0 }]);
    }
    // Each edge between two runs, bisected: a draw is 24 bits, so thirty
    // halvings of a step reach it.
    let mut edges = vec![0.0];
    for w in runs.windows(2) {
        let left = &images[w[0].0 as usize];
        let (mut lo, mut hi) = (u_of(w[0].1 - 1), u_of(w[1].0));
        for _ in 0..30 {
            let mid = 0.5 * (lo + hi);
            if same(&at(mid), left) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        edges.push(0.5 * (lo + hi));
    }
    edges.push(1.0);
    let mut arms = vec![Arm { code: 0, share: 0.0 }; first.len()];
    let mut longest = vec![0u32; first.len()];
    for (i, &(s, e, a)) in runs.iter().enumerate() {
        arms[a].share += edges[i + 1] - edges[i];
        if e - s > longest[a] {
            longest[a] = e - s;
            arms[a].code = (s + e - 1) / 2;
        }
    }
    Some(arms)
}

// ---------------------------------------------------------------- 3D

/// The name mode D's solid shader calls a 3D inverse by (B3).
pub const WGSL_NAME3: &str = "ifs_user_inv3";

/// A variation's 3D inverse (`fn inverse3(v, p)`; docs/projects/
/// rhai-planners.md, B3): one branch, as the solid walk's kernels have,
/// checked as the plane's is (`check3`).
pub struct UserInverse3 {
    pub key: String,
    /// `fn variation3(v, p)`.
    pub forward: Program,
    /// `fn inverse3(v, p)`.
    pub inverse: Program,
    /// Whether `variation3` draws: a many-valued map, each value an arm.
    pub forward_draws: bool,
    /// The inverse as the solid shader calls it, `fn ifs_user_inv3(v, base,
    /// 0.0) -> vec3<f32>` ([`WGSL_NAME3`]).
    pub wgsl: String,
}

impl UserInverse3 {
    /// `variation3` at `v` at the grid draw `code`, as
    /// [`UserInverse::forward_at`].
    pub fn forward_at(&self, params: &ParamValues, v: [f64; 3], code: u32) -> [f64; 3] {
        let u = ((code % DRAW_GRID) as f64 + 0.5) / DRAW_GRID as f64;
        VariationRun::new(&self.forward, params, 1).with_draw(u).call3(v, [0.0; 3], 1.0).0
    }

    /// `inverse3` at `v`.
    pub fn inverse_at(&self, params: &ParamValues, v: [f64; 3]) -> [f64; 3] {
        VariationRun::new(&self.inverse, params, 1).call3(v, [0.0; 3], 1.0).0
    }

    /// Whether `variation3` sends the pre-origin's neighbourhood far out,
    /// as [`UserInverse::unbounded_at_origin`].
    pub fn unbounded_at_origin(&self, params: &ParamValues) -> bool {
        unbounded_by_rings(|r| {
            moduli((0..8).map(|k| {
                let a = std::f64::consts::TAU * k as f64 / 8.0;
                let v = self.forward_at(params, [r * a.cos(), r * a.sin(), 0.5 * r * (k as f64 - 3.5).signum()], 0);
                (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
            }))
        })
    }
}

/// The least and greatest of a ring's moduli; a modulus that is not a
/// number counts as none (least 0) and as far out (greatest infinite).
fn moduli(m: impl Iterator<Item = f64>) -> (f64, f64) {
    m.fold((f64::INFINITY, 0.0f64), |(lo, hi), m| if m.is_nan() { (0.0, f64::INFINITY) } else { (lo.min(m), hi.max(m)) })
}

/// [`UserInverse::unbounded_at_origin`]'s rule over `ring(r)`, the least
/// and greatest modulus of the forward map on the circle (sphere) of
/// radius `r` about the origin.
fn unbounded_by_rings(ring: impl Fn(f64) -> (f64, f64)) -> bool {
    if !(ring(1e-3).1 <= 100.0) {
        return true;
    }
    let rings: Vec<(f64, f64)> = [1e-2, 1e-4, 1e-6, 1e-8].into_iter().map(ring).collect();
    if !rings.windows(2).all(|w| w[1].0 > w[0].1) {
        return false;
    }
    // A pole's growth: the same factor each ring, past 1.05. A
    // logarithm's falls off (log's twin: 2.0, 1.5, 1.33), and its
    // singularity is too mild for the inversions' bulk ball.
    let grow: Vec<f64> = rings.windows(2).map(|w| (w[1].0 * w[1].1).sqrt() / (w[0].0 * w[0].1).sqrt()).collect();
    let (lo, hi) = grow.iter().fold((f64::INFINITY, 0.0f64), |(lo, hi), &g| (lo.min(g), hi.max(g)));
    lo > 1.05 && hi <= 1.1 * lo
}

/// The variation's 3D inverse from its source: `None` when it declares
/// none; why it does not compile otherwise. Checked at its defaults.
pub fn compile3(source: &str, key: &str) -> Option<Result<UserInverse3, String>> {
    if !crate::lang::rhai::has_function(source, "inverse3") {
        return None;
    }
    Some((|| {
        let first = |d: &[crate::lang::Diag]| {
            d.iter().find(|x| x.severity == crate::lang::Severity::Error).map(|x| x.render(source)).unwrap_or_else(|| "it does not compile".to_string())
        };
        if !crate::lang::rhai::has_function(source, "variation3") {
            return Err("a 3D inverse is of a 3D variation, `fn variation3(v, p)`".to_string());
        }
        let forward = crate::lang::rhai::transformation3(source).map_err(|d| first(&d))?;
        let inverse = crate::lang::rhai::inverse_transformation3(source).map_err(|d| format!("its 3D inverse: {}", first(&d)))?;
        let wgsl = crate::lang::wgsl::inverse_kernel3(&inverse, WGSL_NAME3)?;
        #[cfg(not(target_arch = "wasm32"))]
        crate::lang::wgsl::validate_inverse_kernel(&wgsl).map_err(|e| format!("the compiler produced invalid WGSL for the 3D inverse (a bug): {e}"))?;
        let forward_draws = crate::lang::wgsl::variation_shape(&forward).is_ok_and(|s| s.rng);
        let inv = UserInverse3 { key: key.to_string(), forward, inverse, forward_draws, wgsl };
        check3(&inv, &ParamValues::defaults(&inv.forward))?;
        Ok(inv)
    })())
}

/// [`check`] in the ball of radius 4: `variation3` then `inverse3` gives
/// each point back to a relative 1e-6, but one in fifty.
pub fn check3(inv: &UserInverse3, params: &ParamValues) -> Result<(), String> {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut tried, mut missed) = (0u32, 0u32);
    let mut worst: Option<(f64, [f64; 3], [f64; 3], [f64; 3])> = None;
    for i in 0..400u32 {
        // A point in the ball, by volume.
        let (z, ph) = (2.0 * next() - 1.0, std::f64::consts::TAU * next());
        let r = 4.0 * next().cbrt();
        let s = (1.0 - z * z).max(0.0).sqrt();
        let x = [r * s * ph.cos(), r * s * ph.sin(), r * z];
        let v = inv.forward_at(params, x, spread(i));
        let m = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if !m.is_finite() || m > 1e6 {
            continue;
        }
        tried += 1;
        let u = inv.inverse_at(params, v);
        let scale = 1.0f64.max(r);
        let e = ((u[0] - x[0]).powi(2) + (u[1] - x[1]).powi(2) + (u[2] - x[2]).powi(2)).sqrt() / scale;
        if !(e <= 1e-6) {
            missed += 1;
            if worst.is_none_or(|w| !(e <= w.0)) {
                worst = Some((e, x, v, u));
            }
        }
    }
    if tried < 100 {
        return Err(format!("its 3D forward map gave {tried} finite points of 400 over the ball of radius 4: too few to check its inverse"));
    }
    if missed * 50 > tried {
        let (_, x, v, u) = worst.expect("a miss");
        return Err(format!(
            "its 3D inverse does not undo it: `variation3` sends ({:.6}, {:.6}, {:.6}) to ({:.6}, {:.6}, {:.6}), and `inverse3` gives ({:.6}, {:.6}, {:.6}); {missed} of {tried} points missed",
            x[0], x[1], x[2], v[0], v[1], v[2], u[0], u[1], u[2]
        ));
    }
    Ok(())
}

/// The 3D inverse of the variation `key`, compiled and checked once per
/// key: a Rhai variation's own, or for a built-in its twin's, as [`of`].
pub fn of3(key: &str) -> Option<Result<Arc<UserInverse3>, String>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Result<Arc<UserInverse3>, String>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().ok()?.get(key) {
        return hit.clone();
    }
    let source = crate::variations::custom::source_of(key).or_else(|| crate::variations::twins::source(key).map(str::to_string));
    let built = source.and_then(|s| compile3(&s, key)).map(|r| r.map(Arc::new));
    if let Ok(mut c) = cache.lock() {
        c.insert(key.to_string(), built.clone());
    }
    built
}

/// A 3D inverse at one transform's parameter values.
pub struct UserKernel3 {
    pub inverse: Arc<UserInverse3>,
    pub params: ParamValues,
    /// The parameters' slots, then the inverse's `prepare` values.
    pub packed: Vec<f32>,
}

impl UserKernel3 {
    /// `variation3` at `v` on the `arm`th of the [`FORWARD_ARMS`] draws
    /// spread over the grid (`Kernel3::arms`).
    pub fn forward(&'static self, v: [f64; 3], arm: u32) -> [f64; 3] {
        let u = (spread(arm) as f64 + 0.5) / DRAW_GRID as f64;
        with_run(self as *const _ as usize, Which::Forward3, &self.inverse.forward, &self.params, |run| {
            run.set_draw(u);
            run.call3(v, [0.0; 3], 1.0).0
        })
    }

    pub fn inverse(&'static self, v: [f64; 3]) -> [f64; 3] {
        with_run(self as *const _ as usize, Which::Inverse3, &self.inverse.inverse, &self.params, |run| run.call3(v, [0.0; 3], 1.0).0)
    }

    pub fn unbounded_at_origin(&self) -> bool {
        self.inverse.unbounded_at_origin(&self.params)
    }
}

type Table3 = (Vec<&'static UserKernel3>, HashMap<(String, Vec<u32>), Result<u32, String>>);

fn table3() -> &'static RwLock<Table3> {
    static TABLE: OnceLock<RwLock<Table3>> = OnceLock::new();
    TABLE.get_or_init(Default::default)
}

/// [`kernel_slot`] for a 3D inverse ([`check3`] at these values).
pub fn kernel_slot3(inv: &Arc<UserInverse3>, slots: &[f32]) -> Result<u32, String> {
    let id = (inv.key.clone(), slots.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
    if let Some(hit) = table3().read().ok().and_then(|t| t.1.get(&id).cloned()) {
        return hit;
    }
    let params = ParamValues::from_slots(&inv.forward, slots);
    let result = check3(inv, &params).map(|()| {
        let mut packed = slots.to_vec();
        packed.extend(interp::globals(&inv.inverse, &params, &Env::at(C64::ZERO, 1)));
        UserKernel3 { inverse: inv.clone(), params, packed }
    });
    let mut t = table3().write().expect("the 3D user kernels' table");
    if let Some(hit) = t.1.get(&id) {
        return hit.clone();
    }
    let slot = result.map(|k| {
        t.0.push(Box::leak(Box::new(k)));
        (t.0.len() - 1) as u32
    });
    t.1.insert(id, slot.clone());
    slot
}

/// The 3D user kernel in `slot` ([`kernel_slot3`]), as [`kernel`].
pub fn kernel3(slot: u32) -> &'static UserKernel3 {
    KERNELS3.with(|k| {
        if let Some(&hit) = k.borrow().get(slot as usize) {
            return hit;
        }
        let all = table3().read().expect("the 3D user kernels' table").0.clone();
        let hit = all[slot as usize];
        *k.borrow_mut() = all;
        hit
    })
}

/// A twin's parameter slots at its defaults, with `set` by name -- as a
/// flame packs them.
#[cfg(test)]
pub(crate) fn named(name: &str, set: &[(&str, f64)]) -> Vec<f32> {
    let src = crate::variations::twins::source(name).expect("a twin");
    let p = crate::lang::rhai::transformation(src).expect("compiles");
    let mut slots = Vec::new();
    for x in &p.params {
        let v = set.iter().find(|(n, _)| *n == x.name).map_or(x.default, |&(_, v)| (v, 0.0));
        slots.push(v.0 as f32);
        if x.ty() == crate::lang::ir::Ty::Complex {
            slots.push(v.1 as f32);
        }
    }
    slots.extend(p.funcs.iter().map(|f| f.default as f32));
    slots
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

    /// **A user inversion has its built-in's hole**: julian's twin at a
    /// negative distance -- power 2 as in the reported grand julian, and
    /// power 15, whose modulus at 1e-3 is 1.6 -- and spherical's twin are
    /// unbounded at the origin as their built-ins are, and leave the hole
    /// theirs leave in the image of a disc (`Kernel::hole_radius`), less
    /// the sample's thousandth; julian's at a positive distance, bubble's
    /// and log's, neither. Without the hole a Rhai julian of negative
    /// distance drew another picture than its built-in.
    #[test]
    fn a_user_inversion_has_its_builtins_hole() {
        use crate::scene::ifs_analysis::Kernel;
        let user = |name: &str, slots: &[f32]| {
            let src = crate::lang::rhai::with_own_id(crate::variations::twins::source(name).expect("a twin"), &crate::scripted::mint_id());
            let inv = Arc::new(compile(&src, &format!("{name}_hole")).expect("declared").expect("checks"));
            Kernel::User { slot: kernel_slot(&inv, slots).expect("a slot") }
        };
        let cases = [
            (user("julian", &[2.0, -1.0]), Kernel::Root { n: 2, d: -1.0, mirror: false }),
            (user("julian", &[15.0, -1.0]), Kernel::Root { n: 15, d: -1.0, mirror: false }),
            (user("julian", &[3.0, 1.0]), Kernel::Root { n: 3, d: 1.0, mirror: false }),
            (user("spherical", &[]), Kernel::Spherical),
            (user("bubble", &[]), Kernel::Bubble),
        ];
        for (got, want) in cases {
            assert_eq!(got.unbounded_at_origin(), want.unbounded_at_origin(), "{want:?}");
            for r in [0.3, 1.0, 4.5] {
                let (hg, hw) = (got.hole_radius(r), want.hole_radius(r));
                assert!(hg <= hw * (1.0 + 1e-9) && hg >= hw * (1.0 - 2e-3), "{want:?} at {r}: {hg} against {hw}");
            }
        }
        // Unbounded at the origin by a logarithm, not a pole: no
        // inversion, as its grown ball drew it.
        let log = user("log", &[std::f32::consts::E]);
        assert!(!log.unbounded_at_origin() && log.hole_radius(1.0) == 0.0, "log's twin read as an inversion");
        // Unbounded along one line only, and no hole: hyperbolic's inverse
        // takes `(0, y)` to itself (E2). Read as a hole of 0.12, it stopped
        // the deep zoom's handover at its first level.
        let hyperbolic = user("hyperbolic", &[]);
        for r in [0.3, 1.0, 4.5] {
            assert_eq!(hyperbolic.hole_radius(r), 0.0, "hyperbolic's twin at {r}");
        }
    }

    /// **A forward map's arms are read off it** (rhai-planners C1):
    /// julian's twin at power 2, 3, 15 and -3 has that many arms, an equal
    /// share each, each forced by a code inside its own interval of the
    /// draw; a pick of `floor(2.5·u)` shares 0.4, 0.4, 0.2, as its draws
    /// do; a continuous draw has no arms, and a draw that changes nothing
    /// one, named 0.
    #[test]
    fn a_forward_maps_arms_are_read_off_it() {
        let src = crate::lang::rhai::with_own_id(crate::variations::twins::source("julian").expect("a twin"), &crate::scripted::mint_id());
        let inv = Arc::new(compile(&src, "julian_arms").expect("declared").expect("checks"));
        for power in [2.0f32, 3.0, 15.0, -3.0] {
            let k = kernel(kernel_slot(&inv, &[power, 1.0]).expect("a slot"));
            let arms = k.arms().expect("arms");
            let n = power.abs() as usize;
            assert_eq!(arms.len(), n, "power {power}");
            for (i, a) in arms.iter().enumerate() {
                assert!((a.share - 1.0 / n as f64).abs() < 1e-6, "power {power}, arm {i}: share {}", a.share);
                let u = (a.code as f64 + 0.5) / DRAW_GRID as f64;
                assert_eq!((n as f64 * u).floor() as usize, i, "power {power}: arm {i}'s code {} picks another", a.code);
            }
            assert_eq!(k.draws().len(), n);
        }
        let arms = find_arms(|z, u| [z[0] + (2.5 * u).floor(), z[1]], true).expect("arms");
        let shares: Vec<f64> = arms.iter().map(|a| a.share).collect();
        assert!(shares.len() == 3 && (shares[0] - 0.4).abs() < 1e-6 && (shares[1] - 0.4).abs() < 1e-6 && (shares[2] - 0.2).abs() < 1e-6, "{shares:?}");
        assert!(find_arms(|z, u| [z[0] + u, z[1]], true).is_none(), "a continuous draw named arms");
        assert_eq!(find_arms(|z, _| z, true), Some(vec![Arm { code: 0, share: 1.0 }]));
        assert_eq!(find_arms(|z, _| z, false), Some(vec![Arm { code: 0, share: 1.0 }]));
    }

    /// **A twin's derived difference is its built-in's hand form**
    /// (rhai-planners C3): the forward difference `F(z + d) − F(z)` derived
    /// from julian's and bubble's twin bodies
    /// (`lang::delta::variation_difference`), run in the interpreter,
    /// against the built-in kernels' (`forward_delta`), at deviations from
    /// 1e-2 to 1e-13 of the point -- where forming the difference keeps
    /// none of its digits -- arm for arm.
    #[test]
    fn a_twins_derived_difference_is_its_builtins() {
        use crate::scene::forward_delta::kernel_forward_difference_gen;
        use crate::scene::ifs_analysis::Kernel;
        let user = |name: &str, slots: &[f32]| {
            let src = crate::lang::rhai::with_own_id(crate::variations::twins::source(name).expect("a twin"), &crate::scripted::mint_id());
            let inv = Arc::new(compile(&src, &format!("{name}_difference")).expect("declared").expect("checks"));
            assert!(inv.difference.is_some(), "{name}'s twin derives no difference");
            kernel_slot(&inv, slots).expect("a slot")
        };
        let cases = [
            (user("julian", &[2.0, 1.0]), Kernel::Root { n: 2, d: 1.0, mirror: false }),
            (user("julian", &[2.0, -1.0]), Kernel::Root { n: 2, d: -1.0, mirror: false }),
            (user("julian", &[15.0, -1.0]), Kernel::Root { n: 15, d: -1.0, mirror: false }),
            (user("julian", &[-3.0, 0.5]), Kernel::Root { n: -3, d: 0.5, mirror: false }),
            (user("bubble", &[]), Kernel::Bubble),
            // Its odd arms mirrored (E2: its exponent read from an int).
            (user("juliascope", &[3.0, 1.0]), Kernel::Root { n: 3, d: 1.0, mirror: true }),
            (user("juliascope", &[2.0, -1.0]), Kernel::Root { n: 2, d: -1.0, mirror: true }),
        ];
        let mut worst = 0.0f64;
        for (slot, builtin) in cases {
            let k = kernel(slot);
            let codes = k.draws();
            for v in [[0.37, 0.21], [-0.83, 0.45], [1.31, -0.66], [-0.24, -1.87]] {
                for (i, &code) in codes.iter().enumerate() {
                    for scale in [1e-2, 1e-5, 1e-9, 1e-13] {
                        let r = scale * f64::hypot(v[0], v[1]);
                        let e = [r * 0.6, -r * 0.8];
                        let want = kernel_forward_difference_gen(&builtin, &v, &e, i as u32).expect("the built-in's");
                        let got = kernel_forward_difference_gen(&Kernel::User { slot }, &v, &e, code).expect("the twin's");
                        let err = (got[0] - want[0]).hypot(got[1] - want[1]) / want[0].hypot(want[1]);
                        worst = worst.max(err);
                        assert!(err < 1e-9, "{builtin:?} at {v:?}, arm {i}, deviation {scale:e}: {got:?} against {want:?}");
                    }
                }
            }
        }
        println!("worst relative difference: {worst:.1e}");
    }

    #[test]
    #[ignore = "measurement"]
    fn which_twins_derive_differences() {
        for (name, src) in crate::variations::twins::TWINS.iter().filter(|(_, s)| s.contains("fn inverse(")) {
            let src = crate::lang::rhai::with_own_id(src, &crate::scripted::mint_id());
            match compile(&src, name) {
                Some(Ok(inv)) => println!("  {name}: forward {}, inverse {}", inv.difference.is_some(), inv.inverse_difference.is_some()),
                other => println!("  {name}: {:?}", other.map(|r| r.err())),
            }
        }
    }

    /// **Every twin's derived differences are the direct ones, in big
    /// floats** (rhai-planners E2): `F(z + d) − F(z)` at each of its draws
    /// and `G(v + d) − G(v)` along each branch, as the derived forms give
    /// them in f64, against both maps run in big floats (`bigeval`, four
    /// limbs) at `z` and at `z + d` exactly and subtracted -- for every
    /// twin with an inverse, at its defaults and deviations down to 1e-13.
    /// Where a deviation crosses a step or a branch's edge the form gives
    /// the plain difference, which is large; the direct one agrees there
    /// too: the last three points are 1e-3 from an edge (x = 0: splits'
    /// steps, juliascope's sector at its power 2; splits' quadrant at its
    /// shift), which the largest deviation crosses. A point either map
    /// leaves undefined is skipped.
    #[test]
    fn every_twins_derived_differences_are_the_direct_ones_in_big_floats() {
        use crate::escape::bigfloat::{BigComplex, BigFloat};
        let big = |x: f64| BigFloat::from_f64(x, 4);
        let run = |prog: &Arc<Program>, params: &ParamValues, z: [f64; 2], d: [f64; 2]| -> Option<[f64; 2]> {
            let at = |w: [BigFloat; 2]| -> Option<[BigFloat; 2]> {
                let mut m = crate::lang::bigeval::BigMachine::new(prog.clone(), params.clone(), Env::at(C64::ZERO, 1), BigComplex { re: w[0].clone(), im: w[1].clone() }, 4).ok()?;
                m.step().ok()?;
                let out = m.z();
                Some([out.re.clone(), out.im.clone()])
            };
            let a = at([big(z[0]), big(z[1])])?;
            let b = at([big(z[0]).add(&big(d[0])), big(z[1]).add(&big(d[1]))])?;
            let out = [b[0].sub(&a[0]).to_f64(), b[1].sub(&a[1]).to_f64()];
            let sane = |w: &[BigFloat; 2]| w.iter().all(|c| c.to_f64().abs() < 1e29);
            (sane(&a) && sane(&b) && out.iter().all(|c| c.is_finite())).then_some(out)
        };
        let names: Vec<&str> = crate::variations::twins::TWINS.iter().filter(|(_, s)| s.contains("fn inverse(")).map(|(n, _)| *n).collect();
        let (mut worst, mut compared) = ((0.0f64, String::new()), 0usize);
        let mut failures = Vec::new();
        for name in names {
            let mut each = [0usize; 2];
            let src = crate::lang::rhai::with_own_id(crate::variations::twins::source(name).expect("a twin"), &crate::scripted::mint_id());
            let inv = Arc::new(compile(&src, &format!("{name}_e2")).expect("declared").expect("checks"));
            let slot = kernel_slot(&inv, &named(name, &[])).expect("a slot");
            let k = kernel(slot);
            let forward = crate::lang::delta::draw_as_color(&inv.forward).unwrap_or_else(|| inv.forward.clone());
            for z in [[0.37, 0.21], [-0.53, 0.45], [0.31, -0.26], [-0.24, -0.47], [-1e-3, 0.5], [0.399, 0.2], [0.5, 1e-3]] {
                for scale in [1e-2, 1e-6, 1e-10, 1e-13] {
                    let r = scale * f64::hypot(z[0], z[1]);
                    let d = [r * 0.6, -r * 0.8];
                    let mut check = |what: String, side: usize, got: Option<[f64; 2]>, want: Option<[f64; 2]>| {
                        let (Some(got), Some(want)) = (got, want) else { return };
                        let size = want[0].hypot(want[1]);
                        let err = (got[0] - want[0]).hypot(got[1] - want[1]) / size.max(1e-300);
                        if size == 0.0 && got == [0.0, 0.0] {
                            return;
                        }
                        compared += 1;
                        each[side] += 1;
                        if err > worst.0 {
                            worst = (err, what.clone());
                        }
                        if !(err < 1e-8) {
                            failures.push(format!("{what}: {got:?} against {want:?} ({err:.1e})"));
                        }
                    };
                    for &code in &k.draws() {
                        let u = ((code % DRAW_GRID) as f64 + 0.5) / DRAW_GRID as f64;
                        let prog = Arc::new(crate::lang::delta::one_pass(&forward, u));
                        check(format!("{name} forward at {z:?}, draw {code}, deviation {scale:e}"), 0, k.forward_difference(z, d, code), run(&prog, &k.params, z, d));
                    }
                    for branch in 0..inv.branches {
                        let prog = Arc::new(crate::lang::delta::one_pass(&inv.inverse, branch as f64));
                        check(format!("{name} inverse at {z:?}, branch {branch}, deviation {scale:e}"), 1, k.inverse_difference(z, d, branch), run(&prog, &k.params, z, d));
                    }
                }
            }
            // Each map met somewhere.
            if each.contains(&0) {
                failures.push(format!("{name}: compared {} forward and {} inverse", each[0], each[1]));
            }
        }
        println!("{compared} differences compared; worst {:.1e} ({})", worst.0, worst.1);
        assert!(failures.is_empty(), "{} of {compared}:\n{}", failures.len(), failures.join("\n"));
    }

    /// **A twin's derived inverse difference, and its big-float inverse,
    /// are its built-in's** (rhai-planners D3): `G(v + d) − G(v)` derived
    /// from julian's, bubble's and hemisphere's twins' inverses against the
    /// built-in kernels' hand forms (`kernel_difference_gen`), down to
    /// deviations of 1e-13; and julian's twin's inverse run in big floats
    /// (`bigeval`) against the built-in's generic body in big floats, at
    /// four limbs, to 1e-60.
    #[test]
    fn a_twins_inverse_difference_and_big_inverse_are_its_builtins() {
        use crate::escape::bigfloat::BigFloat;
        use crate::scene::ifs_analysis::{kernel_difference_gen, kernel_inverse_gen, Kernel};
        let user = |name: &str, slots: &[f32]| {
            let src = crate::lang::rhai::with_own_id(crate::variations::twins::source(name).expect("a twin"), &crate::scripted::mint_id());
            let inv = Arc::new(compile(&src, &format!("{name}_dinv")).expect("declared").expect("checks"));
            assert!(inv.inverse_difference.is_some(), "{name}'s twin derives no inverse difference");
            kernel_slot(&inv, slots).expect("a slot")
        };
        let cases = [
            (user("julian", &[2.0, 1.0]), Kernel::Root { n: 2, d: 1.0, mirror: false }),
            (user("julian", &[3.0, 1.0]), Kernel::Root { n: 3, d: 1.0, mirror: false }),
            (user("bubble", &[]), Kernel::Bubble),
            (user("hemisphere", &[]), Kernel::Hemisphere),
            // E2: its quadrant test over bools. At its defaults, x = y = 0.4
            // and no shear: the steps (0.8, 0) and (0, 0.8) from (-0.4, -0.4).
            (user("splits", &named("splits", &[])), Kernel::Splits { base: [-0.4, -0.4], x: [0.8, 0.0], y: [0.0, 0.8] }),
        ];
        let mut worst = 0.0f64;
        for (slot, builtin) in cases {
            assert!(crate::scene::ifs_analysis::kernel_has_difference(&builtin), "{builtin:?} has a form");
            for v in [[0.37, 0.21], [-0.53, 0.45], [0.31, -0.26], [-0.24, -0.47]] {
                for branch in 0..kernel(slot).inverse.branches {
                    for scale in [1e-2, 1e-6, 1e-10, 1e-13] {
                        let r = scale * f64::hypot(v[0], v[1]);
                        let d = [r * 0.6, -r * 0.8];
                        let w = [v[0] + d[0], v[1] + d[1]];
                        let Some(want) = kernel_difference_gen(&builtin, &v, &w, &d, branch) else { continue };
                        let got = kernel_difference_gen(&Kernel::User { slot }, &v, &w, &d, branch).expect("the twin's");
                        let err = (got[0] - want[0]).hypot(got[1] - want[1]) / want[0].hypot(want[1]);
                        worst = worst.max(err);
                        assert!(err < 1e-9, "{builtin:?} at {v:?}, branch {branch}, deviation {scale:e}: {got:?} against {want:?}");
                    }
                }
            }
        }
        println!("worst relative difference: {worst:.1e}");
        // In big floats.
        let slot = user("julian", &[3.0, 1.0]);
        let big = |x: f64| BigFloat::from_f64(x, 4);
        for v in [[0.37, 0.21], [-0.53, 0.45]] {
            let bv = [big(v[0]).add(&BigFloat::from_f64(1e-30, 4)), big(v[1])];
            let got = kernel(slot).big_inverse(&bv, 0).expect("a big inverse");
            let want = kernel_inverse_gen(&Kernel::Root { n: 3, d: 1.0, mirror: false }, &bv, 0);
            for i in 0..2 {
                let rel = got[i].sub(&want[i]).to_f64().abs() / want[i].to_f64().abs().max(1e-300);
                assert!(rel < 1e-60, "{v:?} part {i}: {rel:e}");
            }
        }
    }

    /// **A drawing kernel summed with an affine** (rhai-planners D2):
    /// julian's twin at power 3 plus a linear part has a map per arm, each
    /// solved by Newton from the inverse, and every one undoes its own
    /// arm's forward map; bubble's twin, which draws nothing, keeps a map
    /// per branch of its inverse.
    #[test]
    fn a_drawing_kernel_sums_along_each_arm() {
        use crate::scene::ifs_analysis::{Affine2, Kernel, SumMap2};
        let twin = |name: &str, slots: &[f32]| {
            let src = crate::lang::rhai::with_own_id(crate::variations::twins::source(name).expect("a twin"), &crate::scripted::mint_id());
            let inv = Arc::new(compile(&src, &format!("{name}_sum")).expect("declared").expect("checks"));
            kernel_slot(&inv, slots).expect("a slot")
        };
        let sum = |slot: u32, branch: u32| {
            let pre = Affine2 { m: [[0.8, 0.2], [-0.2, 0.8]], t: [0.1, -0.15] };
            let lin = Affine2 { m: [[0.3, 0.0], [0.0, 0.3]], t: [0.05, 0.02] };
            SumMap2 {
                pre,
                pre_inv: pre.inverse().expect("invertible"),
                post: Affine2::IDENTITY,
                post_inv: Affine2::IDENTITY,
                lin,
                lin_inv: lin.inverse(),
                kernel: Kernel::User { slot },
                kind: "user",
                branch,
                kw: 0.7,
                third: 0.0,
                kernel_leads: true,
            }
        };
        let julian = twin("julian", &[3.0, 1.0]);
        assert_eq!(sum(julian, 0).branches(), 3, "julian's three arms, its inverse's one branch");
        let codes: Vec<u32> = kernel(julian).arms().expect("arms").iter().map(|a| a.code).collect();
        for b in 0..3 {
            let s = sum(julian, b);
            assert_eq!((s.arm(), s.seed_branch()), (codes[b as usize], 0));
            // A sum along one arm need not be one-to-one: the solve finds a
            // preimage there, which its arm sends back to the point.
            for p in [[0.4, 0.3], [-0.6, 0.2], [0.1, -0.7]] {
                let q = s.apply_branch(p, s.arm());
                let (back, res) = s.solve(q).unwrap_or_else(|| panic!("arm {b}: no solve at {p:?}"));
                let again = s.apply_branch(back, s.arm());
                assert!(res < 1e-10 && (again[0] - q[0]).hypot(again[1] - q[1]) < 1e-9, "arm {b} at {p:?}: {back:?} goes to {again:?}, not {q:?}");
            }
        }
        let bubble = twin("bubble", &[]);
        assert_eq!(sum(bubble, 0).branches(), 2, "bubble's two inverse branches, no arms");
        assert_eq!((sum(bubble, 1).arm(), sum(bubble, 1).seed_branch()), (1, 1));
    }

    /// What a user kernel's call costs (rhai-planners D1): through the
    /// kernel's kept run against a run held, one thread; and with every
    /// core calling, looking the kernel up against holding it.
    #[test]
    #[ignore = "measurement"]
    fn time_user_kernel_calls() {
        let src = crate::lang::rhai::with_own_id(crate::variations::twins::source("julian").expect("a twin"), &crate::scripted::mint_id());
        let inv = Arc::new(compile(&src, "julian_time").expect("declared").expect("checks"));
        let k = kernel(kernel_slot(&inv, &[2.0, -1.0]).expect("a slot"));
        let n = 200_000;
        let t0 = std::time::Instant::now();
        let mut acc = 0.0;
        for i in 0..n {
            acc += k.forward([0.3 + 1e-6 * i as f64, 0.2], 63)[0];
        }
        let fresh = t0.elapsed().as_secs_f64() / n as f64;
        let t0 = std::time::Instant::now();
        let mut run = VariationRun::new(&inv.forward, &k.params, 1).with_draw(63.5 / 256.0);
        for i in 0..n {
            acc += run.call(C64::new(0.3 + 1e-6 * i as f64, 0.2), C64::ZERO, 1.0).0.re;
        }
        let reused = t0.elapsed().as_secs_f64() / n as f64;
        println!("fresh run a call {:.2} us, reused {:.2} us ({acc})", fresh * 1e6, reused * 1e6);
        use rayon::prelude::*;
        let slot = kernel_slot(&inv, &[2.0, -1.0]).expect("a slot");
        let m = 4_000_000u64;
        let t0 = std::time::Instant::now();
        let s: f64 = (0..m).into_par_iter().map(|i| kernel(slot).forward([0.3 + 1e-9 * i as f64, 0.2], 63)[0]).sum();
        let looked = t0.elapsed().as_secs_f64() / m as f64;
        let k = kernel(slot);
        let t0 = std::time::Instant::now();
        let s2: f64 = (0..m).into_par_iter().map(|i| k.forward([0.3 + 1e-9 * i as f64, 0.2], 63)[0]).sum();
        let held = t0.elapsed().as_secs_f64() / m as f64;
        println!("parallel: {:.3} us a call looking the kernel up, {:.3} us holding it ({s} {s2})", looked * 1e6, held * 1e6);
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

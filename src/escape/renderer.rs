//! The escape-time GPU stage.
//!
//! A single compute pass per render: iterate every pixel, classify,
//! color, write an `Rgba32Float` image the flame tonemap tail consumes
//! via `tonemap_pass_with_input`. No accumulation loop — one dispatch
//! is the whole image at the configured `max_iter`.
//!
//! WebGPU discipline: buffer/texture `Drop` frees nothing on wasm, so
//! everything created here is destroyed in [`EscapeRenderer::destroy`]
//! (mirrors `EffectChainRunner`). Pipelines/bind-group layouts carry
//! no memory and are left to `Drop`.

use std::collections::HashMap;

use egui_wgpu::wgpu::{
    self, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoder, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, Extent3d, PipelineLayoutDescriptor, Queue,
    SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    StorageTextureAccess, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
};

use crate::config::escape::EscapeConfig;

use super::assembler::{self, PARAM_VEC4S};
use super::reference::OrbitCache;

/// Above this zoom the direct path's f32 pixel mapping visibly
/// pixelates: the center's f32 ulp (~6e-8 near |c| = 1) stops
/// resolving pixel spacing a couple of octaves before it equals it —
/// field-observed at zoom 16, so the switch sits at 14. The scaled
/// f32 delta pipeline holds to roughly zoom 54 (w-squared overflow);
/// the floatexp rung takes over beyond [`PERTURB_FLOATEXP_ZOOM`].
pub const PERTURB_MIN_ZOOM: f64 = 14.0;

/// Above this zoom the scaled-f32 delta rung approaches its w-squared
/// overflow (~zoom 54) and the floatexp rung takes over. Below it the
/// scaled rung is preferred: same images, several times faster.
///
/// Every tier has a deep rung, so crossing this is a change of
/// REPRESENTATION and nothing else. A tier that lacked one used to
/// fall through to the direct path here, which past zoom 14 resolves
/// nothing -- Phoenix shipped that way and rendered a single flat
/// colour one thousandth of an octave past this line.
pub const PERTURB_FLOATEXP_ZOOM: f64 = 48.0;

/// How far through its iteration budget the current escape render is:
/// (iterations done, iterations wanted).
///
/// Reported through a static for the same reason reference-build
/// progress is: the panel has no handle on the renderer, and threading
/// one through the whole UI to display a number would be a worse
/// trade than two atomics. Both zero means nothing is in flight.
static RENDER_DONE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static RENDER_WANT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// (done, wanted) while an escape render still has chunks to submit,
/// else None. A settled render reports nothing -- there is no progress
/// to watch, which is itself the answer the panel shows.
pub fn render_progress() -> Option<(u32, u32)> {
    let want = RENDER_WANT.load(std::sync::atomic::Ordering::Relaxed);
    if want == 0 {
        return None;
    }
    Some((RENDER_DONE.load(std::sync::atomic::Ordering::Relaxed), want))
}

/// Why the derivative-based colorings have nothing to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivativeGap {
    /// The formula defines no `wgsl_derivative` (13 of 25 do not).
    Formula,
    /// The perturbed rungs do not iterate a derivative orbit, whatever
    /// the formula defines.
    Perturbed,
}

/// What depth a config can reach, for the panel to report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UsableDepth {
    /// Deltas against a fixed-point reference: no practical limit.
    Perturbed,
    /// The direct path, which stops resolving past this zoom.
    Direct(f64),
}

/// Iteration budget per perturbed dispatch, in pixel-iterations
/// (pixels x iterations). One unbounded dispatch at high max_iter is
/// a Windows TDR (driver reset; observed in the field as a 0xc0000409
/// abort at 200k iterations deep, reproduced as "Parent device is
/// lost" at 1080p). The budget targets a fraction of the 2-second TDR
/// window on a mid-range GPU; the floatexp rung's iterations cost
/// several times the scaled rung's, so its budget is smaller.
pub const PERTURB_CHUNK_BUDGET: u64 = 8_000_000_000;

/// Pixel-iterations the DIRECT path may put in one dispatch, which
/// [`EscapeRenderer::direct_rows_per_dispatch`] turns into a row band.
///
/// The direct path has no per-pixel resume state, so its whole render
/// is a single dispatch: cost is pixels x max_iter, with nothing
/// bounding it. That is fine at the iteration counts a shallow view
/// normally carries, and fatal when a deep-zoom config keeps its
/// max_iter and the view zooms OUT past the perturbation threshold —
/// 10.1M iterations over a supersampled viewport is tens of seconds
/// in one submission, Windows resets the driver at two, and wgpu's
/// device-lost aborts the process (0xc0000409). Reported from an app
/// session zooming out of an f3-depth location, and again from an
/// animation whose zoom track crossed the same line.
///
/// Deliberately fixed and conservative, not adaptive. Getting this
/// wrong is not a slow frame but a LOST DEVICE, which is fatal and
/// unrecoverable, and the wall-clock feedback that paces the
/// iteration chunks does not transfer: a small band is latency-bound
/// rather than throughput-bound, so several doublings all come back
/// under target and the next one is the whole frame again. Measured
/// exactly that - blind doubling lost the device in 2.6 s.
///
/// The numbers behind the value: at 4x this budget a supersampled
/// 10M-iteration view lost the device, while this budget completed
/// both that view and its non-supersampled twin. Small bands do cost
/// throughput (they under-fill the GPU: this view renders in 20 s
/// against ~3 s for one unbounded dispatch) and that is the price of
/// not gambling the process on a config the user can reach with one
/// zoom-out.
pub const DIRECT_DISPATCH_BUDGET: u64 = 250_000_000_000;
// DF mantissas cost ~3-5x per iteration vs plain f32 CFe.
pub const PERTURB_CHUNK_BUDGET_FE: u64 = 600_000_000;

/// Session-wide halvings of [`DIRECT_DISPATCH_BUDGET`] (clamped to 6).
///
/// The budget is a px-iteration bound, but the COST of a px-iteration
/// is config-dependent (formula + coloring), and a band over the set
/// interior runs every pixel to full max_iter -- observed in the
/// field as a band that exceeded Windows' ~2 s TDR window on a config
/// the calibrated budget survived elsewhere, giving a fatal loop:
/// device loss -> recovery -> restart from the top -> the same band
/// dies again. Two SHRINK-ONLY triggers feed this: a device loss
/// while a banded direct render was in flight (the band is almost
/// certainly what killed the device), and a surviving band measured
/// over [`DIRECT_BAND_SLOW_MS`]. It never grows back within a session
/// -- growth is exactly the feedback the ledger measured losing the
/// device -- and it only affects renders big enough to be banded at
/// all: even at shift 6 (~3.9e9 px-iters) an ordinary iteration count
/// still renders in one full-height dispatch.
static DIRECT_BUDGET_SHIFT: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);

/// True while a banded (multi-dispatch) direct render still has bands
/// to go -- the window in which a device loss is attributed to our
/// band size. (A loss during the LAST band goes uncounted; the next
/// attempt re-opens the window on its first band.)
static DIRECT_RENDER_IN_FLIGHT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// A surviving band slower than this still halves the session budget
/// (shrink only -- see [`DIRECT_BUDGET_SHIFT`]): it is pushing the
/// ~2 s window, and it is also what "the app hangs between lines" is.
const DIRECT_BAND_SLOW_MS: u128 = 700;

/// Halvings of the PERTURBED path's chunk budget and ceiling, the
/// mirror of [`DIRECT_BUDGET_SHIFT`] for the other generator.
///
/// The perturbed path chunks by ITERATION, and its adaptive sizer has
/// a structural blind spot: at high max_iter over a view containing
/// set interior the early chunks are nearly free (most pixels escape
/// at once, BLA skips the rest), so the size doubles to the ceiling
/// -- and then the cost profile flips, because the pixels still alive
/// are exactly the ones where skips stop applying and they grind
/// per-step. Cost per iteration is violently non-stationary, so no
/// feedback loop can see that cliff coming; this bounds what happens
/// when it arrives.
static PERTURB_BUDGET_SHIFT: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);

/// True while a perturbed render still has chunks to go -- the window
/// in which a device loss is attributed to our chunk size. Mutually
/// exclusive with [`DIRECT_RENDER_IN_FLIGHT`] by construction: a
/// render is one path or the other, and each closes the other's flag.
static PERTURB_RENDER_IN_FLIGHT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Ceiling on an adaptively grown chunk, before the session shift.
/// The feedback loop stops well below this on any real configuration;
/// it exists so a pathological measurement (a frame that reports
/// ~0 ms) cannot run away into a TDR-length dispatch.
const CHUNK_ITERS_MAX_BASE: u32 = 1_048_576;

/// Both session shifts, persisted across runs.
///
/// Without persistence every session re-learns the same lesson by
/// losing the device one to four times, and each loss is a spin of
/// the driver-state roulette (the field crash.log shows losses
/// ~10 s apart while a user hunted for a working setting). The file
/// sits next to the orbit cache rather than in SystemSettings: this
/// is renderer-side state written from a device-lost callback with no
/// ConfigManager in reach, and it is machine tuning, not a user
/// preference -- it should not travel with a synced profile.
#[cfg(not(target_arch = "wasm32"))]
mod tuning {
    use std::sync::atomic::Ordering;

    fn path() -> Option<std::path::PathBuf> {
        Some(crate::storage::backend::get_app_data_dir().ok()?.join("gpu_tuning.json"))
    }

    /// The file's contents for a pair of shifts.
    pub(super) fn encode(direct: u32, perturb: u32) -> String {
        serde_json::json!({ "direct_shift": direct, "perturb_shift": perturb }).to_string()
    }

    /// Shifts from file contents. Anything unreadable, missing or out
    /// of range reads as zero-to-six: a hand-edited or corrupted file
    /// must never be able to shrink a budget into uselessness, and a
    /// missing key just means that generator never lost a device.
    pub(super) fn decode(text: &str) -> (u32, u32) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
            return (0, 0);
        };
        let get = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0).min(6) as u32;
        (get("direct_shift"), get("perturb_shift"))
    }

    /// Load once per process, before the first read of either shift.
    ///
    /// Inert under `cfg(test)`: the shifts are process-global, so a
    /// developer whose real machine has learned a shift would
    /// otherwise get different test results than one whose has not.
    /// [`decode`] carries the parsing, and is tested directly.
    pub(super) fn ensure_loaded() {
        #[cfg(test)]
        {
            // See above: tests start from an unshifted, deterministic
            // state and never consult the machine's tuning file.
        }
        #[cfg(not(test))]
        {
            static ONCE: std::sync::Once = std::sync::Once::new();
            ONCE.call_once(|| {
                let Some(p) = path() else { return };
                let Ok(text) = std::fs::read_to_string(&p) else { return };
                let (d, pt) = decode(&text);
                super::DIRECT_BUDGET_SHIFT.store(d, Ordering::Relaxed);
                super::PERTURB_BUDGET_SHIFT.store(pt, Ordering::Relaxed);
                if d > 0 || pt > 0 {
                    log::info!(
                        "escape: restored GPU tuning from a previous session \
                         (direct shift {d}, perturbed shift {pt}) -- this machine \
                         lost the device at the unshifted budgets"
                    );
                }
            });
        }
    }

    /// Best effort: tuning is an optimization, never a correctness
    /// input, so a write failure is silent.
    ///
    /// Inert under `cfg(test)` for a blunter reason than [`ensure_loaded`]:
    /// the breaker tests drive the shift to its clamp, and writing that
    /// to the developer's real tuning file would quietly cripple their
    /// app's chunk budgets from then on.
    pub(super) fn save() {
        #[cfg(not(test))]
        {
            let Some(p) = path() else { return };
            let _ = std::fs::write(
                p,
                encode(
                    super::DIRECT_BUDGET_SHIFT.load(Ordering::Relaxed),
                    super::PERTURB_BUDGET_SHIFT.load(Ordering::Relaxed),
                ),
            );
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod tuning {
    pub(super) fn ensure_loaded() {}
    pub(super) fn save() {}
}

/// Called by the GPU device-lost callback (`gpu::device`): halve the
/// budget of whichever generator was mid-render, so the
/// post-recovery retry cannot repeat the fatal dispatch. The flags
/// are mutually exclusive, and a loss with neither open (an
/// unrelated cause -- a driver update, another app's fault) shifts
/// nothing: this only learns from losses it can attribute.
pub fn note_device_lost() {
    use std::sync::atomic::Ordering;
    tuning::ensure_loaded();
    let mut changed = false;
    if DIRECT_RENDER_IN_FLIGHT.swap(false, Ordering::Relaxed) {
        let s = DIRECT_BUDGET_SHIFT.load(Ordering::Relaxed).min(5) + 1;
        DIRECT_BUDGET_SHIFT.store(s, Ordering::Relaxed);
        changed = true;
        log::warn!(
            "escape: device lost during a banded direct render -- halving the \
             direct dispatch budget to {} px-iters for this session",
            DIRECT_DISPATCH_BUDGET >> s
        );
    }
    if PERTURB_RENDER_IN_FLIGHT.swap(false, Ordering::Relaxed) {
        let s = PERTURB_BUDGET_SHIFT.load(Ordering::Relaxed).min(5) + 1;
        PERTURB_BUDGET_SHIFT.store(s, Ordering::Relaxed);
        changed = true;
        log::warn!(
            "escape: device lost during a perturbed render -- halving the \
             perturbed chunk budget (ceiling now {} iterations) for this session",
            CHUNK_ITERS_MAX_BASE >> s
        );
    }
    if changed {
        tuning::save();
    }
}

/// The perturbed path's seed chunk for a pixel count, after the
/// session shift. Free function so the sizing is testable without a
/// GPU (the renderer method just supplies its own dimensions).
fn perturb_chunk_seed(floatexp: bool, px: u64) -> u32 {
    tuning::ensure_loaded();
    let budget = if floatexp {
        PERTURB_CHUNK_BUDGET_FE
    } else {
        PERTURB_CHUNK_BUDGET
    };
    let shift = PERTURB_BUDGET_SHIFT.load(std::sync::atomic::Ordering::Relaxed);
    // Floor 16, not 256: at supersampled resolutions (or plain 4K+),
    // 256 iterations x tens of megapixels is a multi-second dispatch --
    // the TDR class of crash the budget exists to prevent. 16 still
    // makes visible progress every frame.
    ((budget >> shift) / px.max(1)).clamp(16, 65_536) as u32
}

/// The adaptively-grown ceiling, after the session shift.
fn perturb_chunk_ceiling() -> u32 {
    tuning::ensure_loaded();
    let shift = PERTURB_BUDGET_SHIFT.load(std::sync::atomic::Ordering::Relaxed);
    (CHUNK_ITERS_MAX_BASE >> shift).max(64)
}

/// Uniform block — must match `EscapeParams` in the WGSL template
/// (std140: vec2 pairs pack the head, the vec4 arrays start at a
/// 16-byte boundary, total 192 bytes).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct EscapeParamsGpu {
    center: [f32; 2],
    julia_c: [f32; 2],
    span: [f32; 2],
    rot_cs: [f32; 2],
    width: u32,
    height: u32,
    max_iter: u32,
    flags: u32,
    bailout: f32,
    /// First row of the band this dispatch covers (direct and field
    /// templates; the perturbed ones chunk by iteration and leave it
    /// zero). Occupies what used to be explicit padding, so the
    /// uniform layout is unchanged.
    tile_y0: u32,
    /// Mann α (re, im); the shader reads it only in damped pipelines.
    damping: [f32; 2],
    /// 1 = the relief pass slopes the WRAPPED palette coordinate
    /// rather than the coloring's raw value (`ShadingField::Banded`).
    shade_flags: u32,
    _pad_shade: [u32; 3],
    fparams: [[f32; 4]; PARAM_VEC4S],
    cparams: [[f32; 4]; PARAM_VEC4S],
    /// CPU-derived formula data (`FormulaDef::derived_data`),
    /// vec4-packed; zero for formulas without the hook.
    fdata: [[f32; 4]; FDATA_VEC4S],
}

/// 64 vec4s = 256 floats — Origami's 64 fold lines exactly.
const FDATA_VEC4S: usize = 64;

/// Uniform for the relief pass — must match `ShadeParams` in
/// [`EscapeRenderer::run_resolve`]'s WGSL.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadeParamsGpu {
    /// Unit vector toward the light, in pixel space.
    light: [f32; 2],
    /// Vertical exaggeration applied to the slope.
    height: f32,
    /// 0 = the pass only resolves (box downsample), 1 = shade first.
    enabled: u32,
    shadow_color: [f32; 3],
    shadow_strength: f32,
    highlight_color: [f32; 3],
    highlight_strength: f32,
    shadow_blend: u32,
    highlight_blend: u32,
    /// Normal-estimation radius in RENDER pixels (the config's display
    /// pixels times the supersample factor). 0 = the ±1 difference.
    softness: f32,
    _pad: u32,
}

/// Uniform for the perturbed pipeline — must match `PerturbParams`
/// in the WGSL template.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PerturbParamsGpu {
    s: f32,
    inv_s: f32,
    orbit_len: u32,
    /// bit 0: skip quadratic term; bit 1: Julia mode.
    flags: u32,
    /// Pixel spacing as mantissa * 2^exponent (floatexp rung) —
    /// computed symbolically from zoom_log2, valid at any depth.
    s_m: f32,
    s_e: i32,
    /// (view - reference) in pixel units (nucleus relocation).
    ref_offset: [f32; 2],
    /// Chunk window [iter_start, iter_end) for this dispatch.
    iter_start: u32,
    iter_end: u32,
    /// The reference orbit's own `c`, for the maps whose parameter
    /// MULTIPLIES (Lambda). Occupies what used to be padding, so the
    /// layout is unchanged.
    ref_c: [f32; 2],
}

/// The |Z|² channel for the delta-aware escape margin, computed on
/// the CPU in f64 from the DF entries the orbit already carries.
///
/// Stored as a DF pair (hi, lo) so the shader can form
/// `(r2.x - bailout) + (r2.y + 2·Z·δ + |δ|²)` with the first
/// subtraction EXACT near the threshold (both f32, within a factor of
/// two of each other — Sterbenz) and the sub-ulp remainder carried in
/// `r2.y`. A plain f32 |z_full|² test quantizes away the per-pixel
/// delta once 2·Z·δ drops below one ulp of the bailout, which is what
/// broke Feather past zoom ~22: its slow-growth escape boundary is
/// decided by exactly those sub-ulp differences, where a
/// chaos-amplified boundary (Mandelbrot's) never is.
///
/// Computed from hi + lo (the ~2^-48 DF shadow) rather than from the
/// fixed-point value, so it can be rebuilt at UPLOAD time from data
/// already in hand — no reference recompute, no orbit-store format
/// change, and cache-loaded orbits get it for free. 2^-48 relative is
/// the perturbation pipeline's own reference precision, so nothing
/// downstream can tell the difference from an exact channel.
fn r2_channel(hi: &[[f32; 2]], lo: &[[f32; 2]], e: &[i32]) -> Vec<[f32; 2]> {
    hi.iter()
        .zip(lo)
        .zip(e)
        .map(|((h, l), &ex)| {
            let s = (ex as f64).exp2();
            let x = (h[0] as f64 + l[0] as f64) * s;
            let y = (h[1] as f64 + l[1] as f64) * s;
            let r2 = x * x + y * y;
            let r2_hi = r2 as f32;
            [r2_hi, (r2 - r2_hi as f64) as f32]
        })
        .collect()
}

pub struct EscapeRenderer {
    width: u32,
    height: u32,
    output_texture: Texture,
    output_view: TextureView,
    params_buffer: Buffer,
    /// Linear-filtering sampler for the palette (the flame buffers'
    /// shared sampler is non-filtering, chosen for the Rgba32Float
    /// accumulator; the palette is Rgba8Unorm and filters fine).
    palette_sampler: wgpu::Sampler,
    bind_group_layout: BindGroupLayout,
    /// Compiled pipelines keyed `"formula|coloring"` — tiny shaders,
    /// but a live panel flips combinations and recompiles add up.
    pipelines: HashMap<String, ComputePipeline>,
    /// Test-only: force the perturbed path regardless of zoom so the
    /// direct/perturbed agreement test can render the SAME shallow
    /// view both ways.
    #[cfg(test)]
    pub(crate) force_perturbed: bool,
    /// Test-only: with `force_perturbed`, use the floatexp rung.
    #[cfg(test)]
    pub(crate) force_floatexp: bool,
    /// Deep-zoom state: CPU reference-orbit cache (append-on-deepen),
    /// its GPU mirror, and the perturbed pipeline's own layout (two
    /// extra bindings: the orbit buffer and the perturb uniform).
    orbit_cache: OrbitCache,
    /// Progressive mode (the app sets this): reference orbits come
    /// from the worker thread and frames render with whatever prefix
    /// has landed — `render` reports whether the image is final.
    /// Off (the default), orbit compute is synchronous and exports
    /// stay deterministic.
    #[cfg(not(target_arch = "wasm32"))]
    pub progressive: bool,
    #[cfg(not(target_arch = "wasm32"))]
    orbit_worker: Option<super::reference::OrbitWorker>,
    /// Generation of the orbit CONTENT currently uploaded (the
    /// worker's generation in progressive mode,
    /// `OrbitCache::generation` otherwise): re-upload only when it
    /// changes, append when it does not. Keying this on the request
    /// EPOCH re-uploaded the entire orbit on every request -- ~200 MB
    /// per wheel notch at f3 orbit sizes, for content that had not
    /// changed.
    orbit_generation: u64,
    orbit_buffer: Option<Buffer>,
    /// DF residuals, parallel to orbit_buffer (binding 8).
    orbit_lo_buffer: Option<Buffer>,
    /// |Z|² as a DF pair per entry — see [`r2_channel`].
    orbit_r2_buffer: Option<Buffer>,
    /// Per-entry reference exponents (binding 9), parallel to the
    /// orbit buffer.
    orbit_e_buffer: Option<Buffer>,
    orbit_capacity: u32,
    orbit_uploaded: u32,
    perturb_params_buffer: Buffer,
    perturb_bind_group_layout: BindGroupLayout,
    /// Relocation of the current reference (pixel units); fed into
    /// the perturb uniform each render.
    current_ref_offset: [f32; 2],
    /// Per-pixel iteration state for chunked perturbed dispatches
    /// (48 B/px, created on demand, explicit destroy).
    iter_state_buffer: Option<Buffer>,
    iter_state_px: u32,
    /// Bytes per pixel the live buffer was built for. Phoenix's deep
    /// rung carries a second delta and needs a wider one, so a tier
    /// switch reallocates just as a resize does.
    iter_state_stride: u64,
    /// Next chunk's starting iteration and the render it belongs to;
    /// a key change restarts from chunk 0.
    chunk_next: u32,
    chunk_key: Option<String>,
    /// Adaptive chunk sizing. The static `budget / pixels` rule is a
    /// safe SEED, not a good steady state: it sized a chunk at ~1.6 ms
    /// of GPU work on the measured hardware, and since the in-app path
    /// runs exactly one chunk per redraw, a 10.1M-iteration render at
    /// 3x supersampling needed ~140,000 frames — the GPU idle most of
    /// each one. These three fields close a feedback loop on the
    /// wall-clock time between calls instead.
    chunk_iters: u32,
    chunk_count: u32,
    /// First row the next DIRECT dispatch covers (row-band chunking;
    /// the perturbed path chunks by iteration instead).
    direct_tile_y: u32,
    /// When the previous direct band was dispatched (same render):
    /// while the escape-dirty loop is saturated the inter-frame gap
    /// approximates that band's GPU time, and a slow band shrinks the
    /// session budget (see [`DIRECT_BUDGET_SHIFT`]).
    direct_last: Option<web_time::Instant>,

    /// Largest chunk that PROVABLY completed inside the target: the
    /// next call after it came in on time. Growth is capped at a
    /// multiple of this rather than at the raw ceiling, so a cliff
    /// costs one honest doubling over proven ground instead of a jump
    /// to the maximum (see [`GROWTH_HEADROOM`]).
    chunk_proven: u32,
    /// Calls since the last doubling. Growth waits for
    /// [`GROWTH_INTERVAL`] of them so the measurement is not taken
    /// while earlier submissions are still in flight.
    chunk_since_growth: u32,

    /// Running minimum of the observed inter-call time: the cost of a
    /// frame whose chunk work is negligible. Under vsync this settles
    /// at the refresh period, which is exactly the baseline the target
    /// must be measured against — a fixed millisecond target would
    /// read 16.7 ms as "over budget" and shrink to the floor forever.
    chunk_base_ms: f32,
    chunk_last: Option<web_time::Instant>,
    /// Explicit per-chunk time target (headless sets one). Zero means
    /// derive it from `chunk_base_ms`, the interactive behaviour.
    chunk_target_ms: f32,
    /// Diagnostic escape hatch (ESCAPE_CHUNK_MS): force a per-chunk
    /// time target. The render must be IDENTICAL at every setting -
    /// that is the property the chunk-invariance test checks.
    chunk_target_env: Option<f32>,
    /// GPU-time pacing, when the device supports timestamp queries.
    timestamps: Option<TimestampPacer>,
    /// Milliseconds per iteration measured on the GPU by the most
    /// recent completed query. None until the first result lands, and
    /// on devices without the feature -- the wall-clock path then
    /// carries the pacing unchanged.
    gpu_ms_per_iter: Option<f32>,
    /// Test hook: shrink the chunk to force multi-chunk renders.
    #[cfg(test)]
    pub(crate) chunk_override: Option<u32>,
    /// Disable the adaptive chunk feedback and use the static seed
    /// every call (see [`EscapeRenderer::set_fixed_chunk`]).
    chunk_fixed: bool,
    /// BLA iteration-skip table (binding 7): GPU buffer + what it was
    /// built for. A zeroed dummy (n_levels = 0) is bound whenever the
    /// table is inapplicable, so there is no pipeline permutation.
    bla_buffer: Option<Buffer>,
    bla_dummy: Option<Buffer>,
    bla_built: Option<BlaBuilt>,
    /// Test hook: force per-step iteration to compare against skips.
    #[cfg(test)]
    pub(crate) disable_bla: bool,
    /// Test hook: build the direct shader WITHOUT interior detection,
    /// so the agreement test can render a view both ways.
    #[cfg(test)]
    pub(crate) disable_interior: bool,
    /// Display (output) size. `width`/`height` are the RENDER size =
    /// display × supersample; everything internal keys off those, so
    /// the whole pipeline (pixel spacing, iteration state, chunk
    /// keys, nucleus offsets) is supersampling-consistent for free.
    out_width: u32,
    out_height: u32,
    supersample: u32,
    /// Display-size target of the box downsample (None at 1×:
    /// `output_view` then serves the render texture directly).
    final_texture: Option<Texture>,
    final_view: Option<TextureView>,
    /// (factor, pipeline, layout) — the factor is spliced into the
    /// WGSL, so a factor change recompiles (rare).
    downsample: Option<(u32, wgpu::ComputePipeline, BindGroupLayout)>,
    /// The coloring's scalar field at RENDER resolution, written by
    /// every escape template and finite-differenced by the relief
    /// pass. A 1×1 dummy while shading is off: the templates store to
    /// it unconditionally and WGSL discards the out-of-bounds writes,
    /// which is what keeps one shader variant instead of two.
    height_texture: Texture,
    height_view: TextureView,
    /// Whether `height_texture` is full-size (shading on) or the dummy.
    height_full: bool,
    shade_params_buffer: Buffer,
}

/// GPU-time pacing for the perturbed path (TDR-safety plan item C).
///
/// The fallback proxy for chunk cost is the wall-clock gap between
/// calls, which is honest only once the queue has drained: with
/// submissions in flight it reads short, so the size doubles on
/// measurements that have not happened yet. A timestamp query around
/// the dispatch measures the work itself.
///
/// The result returns through a buffer map, landing two or three
/// calls later -- fine, since pacing is a trailing control loop
/// either way. One measurement is in flight at a time; frames in
/// between simply do not measure. What is kept is COST PER ITERATION
/// rather than the raw duration, because the chunk size changes
/// between a measurement and its use and the per-iteration cost is
/// the part that transfers.
struct TimestampPacer {
    query_set: wgpu::QuerySet,
    /// Resolve target for the two timestamps (u64 each).
    resolve: Buffer,
    /// Mappable copy of `resolve`.
    staging: Buffer,
    /// Nanoseconds per timestamp tick.
    period_ns: f32,
    phase: TsPhase,
    /// Iterations the in-flight measurement covers.
    iters: u32,
    /// Map completion, set from the callback: 0 pending, 1 mapped,
    /// 2 failed.
    done: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

#[derive(Clone, Copy, PartialEq)]
enum TsPhase {
    /// Nothing in flight: the next dispatch may be measured.
    Idle,
    /// Timestamps written and copied, but the caller has not
    /// submitted that encoder yet -- so the map cannot be requested
    /// until the next call.
    Encoded,
    /// Map requested; waiting on the callback.
    Mapping,
}

impl TimestampPacer {
    /// Two timestamps: 16 bytes resolved, 16 copied back.
    const BYTES: u64 = 16;

    fn new(device: &Device, period_ns: f32) -> Self {
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("Escape Chunk Timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: 2,
        });
        let resolve = device.create_buffer(&BufferDescriptor {
            label: Some("Escape Timestamp Resolve"),
            size: Self::BYTES,
            usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let staging = device.create_buffer(&BufferDescriptor {
            label: Some("Escape Timestamp Staging"),
            size: Self::BYTES,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            query_set,
            resolve,
            staging,
            period_ns,
            phase: TsPhase::Idle,
            iters: 0,
            done: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }
}

/// What the current BLA table was built for (rebuild trigger).
struct BlaBuilt {
    /// Orbit-content generation ([`EscapeRenderer`]'s
    /// `orbit_generation`) the table was built from -- a different
    /// orbit of the same length must not reuse it.
    generation: u64,
    /// EFFECTIVE length the table was built over:
    /// min(orbit_len, max_iter + 1). A retained deep orbit can be
    /// thousands of times longer than the view's max_iter, and
    /// entries past max_iter are unreachable (a pixel's reference
    /// index never exceeds its iteration count, and the shader
    /// bounds every skip by `params.max_iter - i`).
    orbit_len: u32,
    /// Consecutive dc-growth rebuilds against this same orbit: each
    /// one doubles the |dc| headroom, so a deep-to-shallow zoom
    /// journey (thousands of octaves) costs ~a dozen rebuilds, not
    /// one per fixed headroom step.
    dc_rebuilds: u32,
    power: u32,
    julia: bool,
    /// log2 of the |δc| bound the radii used (finite at ANY zoom).
    /// Zooming IN shrinks the actual bound below it (still
    /// conservative — no rebuild); widening the view past it forces
    /// one. Julia renders carry −∞ (δc = 0 exactly).
    dc_log2: f64,
}

impl EscapeRenderer {
    pub fn new(device: &Device, width: u32, height: u32) -> Self {
        let (output_texture, output_view) = Self::create_output(device, width, height);

        let params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Escape Params"),
            size: std::mem::size_of::<EscapeParamsGpu>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let palette_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Escape Palette Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let perturb_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Escape Perturb Params"),
            size: std::mem::size_of::<PerturbParamsGpu>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let perturb_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Escape Perturbed Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
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
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
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
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 6,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 7,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
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
                // The coloring's scalar field, for the relief pass.
                BindGroupLayoutEntry {
                    binding: 10,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::R32Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                // |Z|² DF pair for the delta-aware escape margin.
                BindGroupLayoutEntry {
                    binding: 11,
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

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Escape Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
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
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                // The coloring's scalar field, for the relief pass.
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::R32Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        // Dummy until shading turns on: see the field's doc comment.
        let (height_texture, height_view) = Self::create_height(device, 1, 1);
        let shade_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Escape Shade Params"),
            size: std::mem::size_of::<ShadeParamsGpu>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            width,
            height,
            output_texture,
            output_view,
            params_buffer,
            palette_sampler,
            bind_group_layout,
            pipelines: HashMap::new(),
            #[cfg(test)]
            force_perturbed: false,
            #[cfg(test)]
            force_floatexp: false,
            orbit_cache: OrbitCache::default(),
            #[cfg(not(target_arch = "wasm32"))]
            progressive: false,
            #[cfg(not(target_arch = "wasm32"))]
            orbit_worker: None,
            orbit_generation: 0,
            orbit_buffer: None,
            orbit_lo_buffer: None,
            orbit_r2_buffer: None,
            orbit_e_buffer: None,
            orbit_capacity: 0,
            orbit_uploaded: 0,
            perturb_params_buffer,
            perturb_bind_group_layout,
            height_texture,
            height_view,
            height_full: false,
            shade_params_buffer,
            current_ref_offset: [0.0, 0.0],
            iter_state_buffer: None,
            iter_state_px: 0,
            iter_state_stride: 0,
            chunk_next: 0,
            chunk_key: None,
            timestamps: None,
            gpu_ms_per_iter: None,
            chunk_iters: 0,
            chunk_count: 0,
            chunk_proven: 0,
            chunk_since_growth: 0,
            direct_tile_y: 0,
            direct_last: None,
            chunk_base_ms: f32::MAX,
            chunk_last: None,
            chunk_target_ms: 0.0,
            chunk_target_env: std::env::var("ESCAPE_CHUNK_MS")
                .ok()
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| *v > 0.0),
            #[cfg(test)]
            chunk_override: None,
            chunk_fixed: false,
            bla_buffer: None,
            bla_dummy: None,
            bla_built: None,
            #[cfg(test)]
            disable_bla: false,
            #[cfg(test)]
            disable_interior: false,
            out_width: width,
            out_height: height,
            supersample: 1,
            final_texture: None,
            final_view: None,
            downsample: None,
        }
    }

    /// Build/refresh the BLA table when skipping applies to this
    /// render; returns whether `bla_buffer` is current. Inapplicable
    /// (Ship tier, per-iteration colorings, unreachable CPU orbit):
    /// false, and the caller binds the zeroed dummy.
    fn ensure_bla(
        &mut self,
        device: &Device,
        queue: &Queue,
        escape: &EscapeConfig,
        orbit_len: u32,
        tier: assembler::PerturbTier,
        progressive: bool,
        orbit_done: bool,
    ) -> bool {
        #[cfg(test)]
        if self.disable_bla {
            return false;
        }
        // Diagnostic escape hatch (all builds): isolate BLA-dependent
        // differences from per-step rendering.
        if std::env::var("ESCAPE_DISABLE_BLA").is_ok() {
            return false;
        }
        let power = match tier {
            assembler::PerturbTier::Power(p) => p.clamp(2, 12),
            assembler::PerturbTier::Ship(_) => return false,
            // Anti-holomorphic: `conj` is not complex-linear, so the
            // A*delta + B*delta_c model BLA is built on does not hold.
            // Per-step iteration carries the Tricorn family.
            assembler::PerturbTier::Tricorn(_) => return false,
            // A skip advances the reference index without running the
            // steps in between, but Phoenix's step also ADVANCES ITS
            // HISTORY -- w_prev would be left measured against the
            // wrong iterate. A two-term BLA needs 2x2 coefficients;
            // until then Phoenix iterates per-step.
            assembler::PerturbTier::Phoenix | assembler::PerturbTier::Manowar => return false,
            // Lambda's BLA is derivable -- the map is holomorphic, so
            // A = C(1-2Z) and B = Z(1-Z) -- but the table builder is
            // written for the power tier's A = p*Z^(p-1) and has no
            // hook for a per-tier coefficient yet. Per-step until it
            // does; correctness first, skips second.
            assembler::PerturbTier::Lambda => return false,
            // Feather's denominator reads the components of z
            // separately, so the map is not holomorphic and the
            // A*delta + B*delta_c model has no derivation -- the same
            // reason the Tricorn family has no BLA.
            assembler::PerturbTier::Feather(_) => return false,
            // Rational with a pole: a BLA bound would have to model the
            // pole term's growth, which the power-tier builder does not.
            assembler::PerturbTier::McMullen(_, _) => return false,
            // A BLA skip advances the reference index without running
            // the steps between -- but Magnet's loop can TERMINATE in
            // those steps, on convergence. Skipping past a settle
            // would report the wrong iteration count.
            assembler::PerturbTier::Magnet(_) => return false,
        };
        // Skipped iterations never run the accumulator/period updates,
        // so those colorings keep the per-step path.
        let coloring = super::get_coloring(&escape.coloring);
        if coloring.has_feature(super::ColoringFeature::NeedsOrbitAccum)
            || coloring.has_feature(super::ColoringFeature::NeedsPeriod)
        {
            return false;
        }
        // |δc| bound over the viewport: half-diagonal plus the nucleus
        // relocation offset, in PIXEL units, carried against the pixel
        // spacing in LOG SPACE — an f64 absolute bound underflows past
        // ~zoom 1000, which used to disable BLA exactly where deep
        // renders need the skips most.
        let h = self.height.max(1) as f64;
        let dc_log2 = if escape.julia {
            f64::NEG_INFINITY
        } else {
            let half_diag = 0.5 * (self.width as f64).hypot(h);
            let off = (self.current_ref_offset[0] as f64).hypot(self.current_ref_offset[1] as f64);
            (half_diag + off).max(1e-30).log2() + (2.0 - escape.zoom_log2 - h.log2())
        };
        // Entries past the view's max_iter are dead weight: a
        // pixel's reference index never exceeds its iteration count
        // (rebasing restarts it at 0), and the shader bounds every
        // skip by `params.max_iter - i` -- so cap the build there.
        // A 10.1M-entry orbit retained while the view sits at a few
        // thousand iterations otherwise pays a ~2 GB transient and a
        // seconds-long main-thread build for a table it cannot use.
        let orbit_len_eff = orbit_len.min(escape.max_iter.saturating_add(1));
        let mut dc_grew = false;
        let mut dc_streak = 0u32;
        if let Some(b) = &self.bla_built {
            // A table from an earlier, SHORTER prefix of the same
            // orbit stays valid while the orbit grows (its spans are
            // a prefix of the appended orbit -- fewer skips, never
            // wrong), so during growth the length mismatch alone
            // does not force a rebuild.
            if b.generation == self.orbit_generation
                && b.power == power
                && b.julia == escape.julia
                && (b.orbit_len == orbit_len_eff || !orbit_done)
            {
                if dc_log2 <= b.dc_log2 + 1e-9 {
                    return self.bla_buffer.is_some();
                }
                // Same orbit, same shape -- invalidated ONLY because
                // the view widened past the built |dc| bound. This is
                // the rebuild that pads (see below).
                dc_grew = true;
                dc_streak = b.dc_rebuilds;
            }
        }
        // While the reference is still GROWING, never (re)build:
        // the table would be rebuilt every frame as the orbit
        // lengthens -- a full f64 copy of the orbit taken under the
        // worker mutex plus an O(n) merge tree, per frame -- and
        // per-step iteration is exact anyway. Build once, when the
        // orbit completes.
        if !orbit_done {
            return false;
        }
        // The CPU copy of the orbit the GPU mirror holds.
        // BLA takes the reference at FULL precision: hi + lo, scaled
        // by the per-entry exponent, in f64. The deltas iterate in DF
        // at ~2^-48, so an A coefficient built from the f32 half alone
        // would inject 2^-24 of error on every skip - error BLA_EPS
        // never bounds, since it governs the dropped nonlinear term
        // rather than the coefficient. f64 also carries the deep dips
        // (2^-183 and below) that f32 flushes to zero, which is what
        // made those steps un-skippable.
        let cap = orbit_len_eff as usize;
        let with_exp = |hi: &[[f32; 2]], lo: &[[f32; 2]], e: &[i32]| -> Vec<[f64; 2]> {
            hi.iter()
                .take(cap)
                .enumerate()
                .map(|(i, z)| {
                    let l = lo.get(i).copied().unwrap_or([0.0, 0.0]);
                    let scale = match e.get(i).copied().unwrap_or(0) {
                        0 => 1.0,
                        k => (k as f64).exp2(),
                    };
                    [
                        (z[0] as f64 + l[0] as f64) * scale,
                        (z[1] as f64 + l[1] as f64) * scale,
                    ]
                })
                .collect()
        };
        #[cfg(not(target_arch = "wasm32"))]
        let orbit_data: Option<Vec<[f64; 2]>> = if progressive {
            self.orbit_worker.as_ref().map(|wk| {
                let p = wk.progress.lock().unwrap();
                with_exp(&p.orbit, &p.orbit_lo, &p.orbit_e)
            })
        } else {
            self.orbit_cache
                .peek()
                .map(|o| with_exp(&o.orbit, &o.orbit_lo, &o.orbit_e))
        };
        #[cfg(target_arch = "wasm32")]
        let orbit_data: Option<Vec<[f64; 2]>> = {
            let _ = progressive;
            self.orbit_cache
                .peek()
                .map(|o| with_exp(&o.orbit, &o.orbit_lo, &o.orbit_e))
        };
        let Some(orbit_data) = orbit_data else {
            return false;
        };
        let usable = (orbit_len as usize).min(orbit_data.len());
        if usable < 3 {
            return false;
        }
        // Truncate at the reference's own escape: a skip riding the
        // escaped tail would overshoot the pixel's escape iteration
        // by up to the span.
        let bail = escape.bailout.max(1e-6) as f64;
        let mut prefix = usable;
        for (i, z) in orbit_data[..usable].iter().enumerate() {
            let q = z[0] * z[0] + z[1] * z[1];
            if q > bail {
                prefix = (i + 1).max(2);
                break;
            }
        }
        // A rebuild forced by |dc| GROWTH builds for a padded bound,
        // not the view exactly: dc grows monotonically as the view
        // zooms OUT, so an exact bound forced a full O(n) rebuild on
        // every zoom-out tick -- at 10M-entry orbits a main-thread
        // stall and a ~half-GB buffer rewrite per wheel notch. Four
        // octaves makes further rebuilds a once-per-16x-widening
        // event; the cost is slightly smaller skip radii (the dc
        // term in the merge), which is conservative, never wrong.
        //
        // A FRESH build (new orbit, new shape -- every one-shot
        // headless render) keeps the exact bound: padding there would
        // change settled pixels for no churn benefit, and the visual
        // suite pins those. Measured: uniform padding moved up to 13%
        // of pixels on the deep parameter-plane suite frames.
        const DC_HEADROOM_LOG2: f64 = 4.0;
        // Each consecutive dc-growth rebuild doubles the headroom
        // (4, 8, 16, 32, 64 octaves): a zoom-out from an f3-depth
        // location to the perturbation threshold crosses ~2300
        // octaves, which at a fixed 4-octave pad is ~580 rebuilds --
        // with backoff it is ~a dozen. The wider pad only shrinks
        // skip radii further (conservative), and the streak resets
        // on any fresh build.
        let pad = if dc_grew {
            DC_HEADROOM_LOG2 * f64::from(1u32 << dc_streak.min(4))
        } else {
            0.0
        };
        let dc_built = if dc_log2 == f64::NEG_INFINITY {
            f64::NEG_INFINITY
        } else {
            dc_log2 + pad
        };
        let dc = if dc_built == f64::NEG_INFINITY {
            super::bla::MagFe::zero()
        } else {
            let e = dc_built.floor();
            super::bla::MagFe { m: 2f64.powf(dc_built - e), e: e as i64 }
        };
        let table =
            super::bla::BlaTable::build_with_dc(&orbit_data[..prefix], power, dc, dc_built);
        let n_levels = table.levels.len().min(30);
        let total: usize = table.levels[..n_levels].iter().map(|l| l.len()).sum();
        if total == 0 {
            self.bla_built = None;
            return false;
        }
        let mut bytes = Vec::with_capacity(144 + total * 32);
        let mut offsets = [0u32; 32];
        let mut acc = 0u32;
        for (l, lev) in table.levels[..n_levels].iter().enumerate() {
            offsets[l] = acc;
            acc += lev.len() as u32;
        }
        for o in offsets {
            bytes.extend_from_slice(&o.to_le_bytes());
        }
        bytes.extend_from_slice(&(n_levels as u32).to_le_bytes());
        bytes.extend_from_slice(&((prefix - 1) as u32).to_le_bytes());
        bytes.extend_from_slice(&[0u8; 8]);
        let clamp_e = |e: i64| -> i32 { e.clamp(-1_000_000_000, 1_000_000_000) as i32 };
        for lev in &table.levels[..n_levels] {
            for ent in lev {
                bytes.extend_from_slice(&(ent.a.re as f32).to_le_bytes());
                bytes.extend_from_slice(&(ent.a.im as f32).to_le_bytes());
                bytes.extend_from_slice(&(ent.b.re as f32).to_le_bytes());
                bytes.extend_from_slice(&(ent.b.im as f32).to_le_bytes());
                bytes.extend_from_slice(&clamp_e(ent.a.e).to_le_bytes());
                bytes.extend_from_slice(&clamp_e(ent.b.e).to_le_bytes());
                let (rm, re) = if ent.r.m > 0.0 {
                    (ent.r.m as f32, clamp_e(ent.r.e))
                } else {
                    (0.0f32, 0)
                };
                bytes.extend_from_slice(&rm.to_le_bytes());
                bytes.extend_from_slice(&re.to_le_bytes());
            }
        }
        let size = bytes.len() as u64;
        let recreate = match &self.bla_buffer {
            Some(b) => b.size() < size,
            None => true,
        };
        if recreate {
            if let Some(old) = self.bla_buffer.take() {
                old.destroy();
            }
            // Headroom so progressive deepening doesn't recreate
            // every rebuild.
            self.bla_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape BLA Table"),
                size: (size + size / 2).max(176),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        queue.write_buffer(self.bla_buffer.as_ref().unwrap(), 0, &bytes);
        self.bla_built = Some(BlaBuilt {
            generation: self.orbit_generation,
            orbit_len: orbit_len_eff,
            power,
            julia: escape.julia,
            dc_log2: dc_built,
            dc_rebuilds: if dc_grew { dc_streak.saturating_add(1) } else { 0 },
        });
        true
    }

    /// Use the static per-dispatch seed for every chunk instead of
    /// the adaptive feedback loop.
    ///
    /// The feedback loop measures WALL-CLOCK TIME BETWEEN CALLS as a
    /// proxy for GPU cost. That proxy holds for a caller that lets
    /// the queue drain between chunks (an interactive frame loop, or
    /// the headless path, which waits). It is a LIE for a caller that
    /// submits chunk after chunk without waiting -- the measurement
    /// then covers CPU command-encoding only, every chunk reads as
    /// free, and the size doubles until it hits the ceiling. That is
    /// how a browser export earns a GPU-watchdog device loss: the
    /// same failure mode as a desktop TDR, with the same cause (one
    /// dispatch too long).
    ///
    /// The seed is the TDR-calibrated `budget / pixels`, so a fixed
    /// chunk is bounded by construction. The cost is loop count: a
    /// high-iteration export runs many small dispatches rather than
    /// few large ones. The render is chunk-INVARIANT (pinned by the
    /// chunk-invariance test), so this cannot change the image.
    pub fn set_fixed_chunk(&mut self, fixed: bool) {
        self.chunk_fixed = fixed;
    }

    /// Per-chunk wall-clock target for callers that are not driving a
    /// UI. A headless export only needs the FINAL image, so its chunks
    /// should be big enough that the per-chunk downsample stops
    /// mattering; interactive callers leave this at zero and get a
    /// target derived from the observed frame baseline.
    pub fn set_chunk_time_target(&mut self, ms: f32) {
        self.chunk_target_ms = ms.max(0.0);
    }

    /// Consume a landed timestamp result, and advance the pacer's
    /// phase. Call once per render, before deciding to measure.
    fn ts_poll(&mut self) {
        use std::sync::atomic::Ordering;
        let Some(ts) = self.timestamps.as_mut() else {
            return;
        };
        if ts.phase != TsPhase::Mapping {
            return;
        }
        match ts.done.load(Ordering::Relaxed) {
            0 => {}
            1 => {
                let elapsed = {
                    let view = ts.staging.slice(..).get_mapped_range();
                    let start = u64::from_le_bytes(view[0..8].try_into().unwrap_or_default());
                    let end = u64::from_le_bytes(view[8..16].try_into().unwrap_or_default());
                    end.saturating_sub(start)
                };
                ts.staging.unmap();
                ts.phase = TsPhase::Idle;
                let ms = elapsed as f32 * ts.period_ns * 1e-6;
                if ms > 0.0 && ts.iters > 0 {
                    self.gpu_ms_per_iter = Some(ms / ts.iters as f32);
                }
            }
            _ => {
                // Map failed: nothing is mapped, so nothing to unmap.
                ts.phase = TsPhase::Idle;
            }
        }
    }

    /// The most recent GPU-measured cost per iteration, if the device
    /// supports timestamps and a result has landed (test observation
    /// point: the pacer is otherwise invisible by design).
    #[cfg(test)]
    pub(crate) fn gpu_ms_per_iter(&self) -> Option<f32> {
        self.gpu_ms_per_iter
    }

    /// Whether this dispatch should carry timestamp writes. Creates
    /// the pacer on first use (the queue supplies the tick period),
    /// and moves a previously-encoded measurement into its map -- by
    /// now the caller has submitted the encoder that produced it.
    fn ts_prepare(&mut self, device: &Device, queue: &Queue) -> bool {
        use std::sync::atomic::Ordering;
        if self.timestamps.is_none() {
            if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
                return false;
            }
            self.timestamps = Some(TimestampPacer::new(device, queue.get_timestamp_period()));
        }
        let ts = self.timestamps.as_mut().expect("created above");
        if ts.phase == TsPhase::Encoded {
            ts.done.store(0, Ordering::Relaxed);
            let done = ts.done.clone();
            ts.staging.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                done.store(if r.is_ok() { 1 } else { 2 }, Ordering::Relaxed);
            });
            ts.phase = TsPhase::Mapping;
        }
        ts.phase == TsPhase::Idle
    }

    /// Resolve the pair this dispatch just wrote, and remember how
    /// many iterations it covered.
    fn ts_after_dispatch(&mut self, encoder: &mut CommandEncoder, iters: u32) {
        let Some(ts) = self.timestamps.as_mut() else {
            return;
        };
        encoder.resolve_query_set(&ts.query_set, 0..2, &ts.resolve, 0);
        encoder.copy_buffer_to_buffer(&ts.resolve, 0, &ts.staging, 0, TimestampPacer::BYTES);
        ts.iters = iters;
        ts.phase = TsPhase::Encoded;
    }

    /// Per-chunk GPU-time target when timestamps are driving the
    /// pacing. Well inside a 60 Hz frame, and an order of magnitude
    /// inside the ~2 s window a driver reset watches.
    const GPU_TARGET_MS: f32 = 10.0;

    /// Reset the size feedback (new render state, or a finished one).
    fn reset_chunk_pacing(&mut self) {
        self.chunk_iters = 0;
        self.chunk_base_ms = f32::MAX;
        self.chunk_last = None;
        self.chunk_count = 0;
        self.chunk_proven = 0;
        self.chunk_since_growth = 0;
    }

    /// Calls a doubling must wait between attempts.
    ///
    /// The wall-clock gap is only an honest proxy for GPU cost once
    /// the queue has drained. With two or three submissions in flight
    /// it reads short -- so doubling on every call hands out several
    /// free passes before backpressure tells the truth, and the size
    /// is already at the ceiling when the first real measurement
    /// lands. Waiting three calls costs a slower ramp on genuinely
    /// cheap renders and nothing else.
    const GROWTH_INTERVAL: u32 = 3;

    /// How far past PROVEN work a chunk may grow (see
    /// [`Self::chunk_proven`]). Five doublings: enough to ramp
    /// quickly on a cheap render, small enough that arriving at a
    /// cost cliff overshoots by a bounded factor rather than by the
    /// whole ceiling.
    const GROWTH_HEADROOM: u32 = 32;

    /// The next chunk's iteration count, grown or shrunk to hold the
    /// caller's per-call time near its target.
    ///
    /// The first chunk of any render uses the static seed, so a
    /// configuration that turns out to be expensive per iteration is
    /// never met with a large chunk; growth is at most 2x per call and
    /// only when the previous call came in under target, which bounds
    /// an overshoot to roughly one target period.
    fn next_chunk(&mut self, floatexp: bool) -> u32 {
        let seed = self.chunk_size(floatexp);
        #[cfg(test)]
        if self.chunk_override.is_some() {
            return seed;
        }
        if self.chunk_fixed {
            return seed;
        }
        // Measured GPU cost, when a timestamp result has landed: size
        // the chunk directly from cost per iteration instead of
        // inferring it from call spacing. Growth stays bounded to 2x
        // per call even here -- the measurement is honest, but the
        // cost itself is non-stationary (pixels die as a render
        // proceeds, and the survivors are the expensive ones).
        if let Some(mspi) = self.gpu_ms_per_iter {
            if mspi > 0.0 {
                let now = web_time::Instant::now();
                self.chunk_last = Some(now);
                self.chunk_count = self.chunk_count.saturating_add(1);
                let target = if let Some(forced) = self.chunk_target_env {
                    forced
                } else if self.chunk_target_ms > 0.0 {
                    self.chunk_target_ms
                } else {
                    Self::GPU_TARGET_MS
                };
                let current = self.chunk_iters.max(seed);
                let ideal = (target / mspi).clamp(16.0, perturb_chunk_ceiling() as f32) as u32;
                let chunk = ideal
                    .clamp(
                        (current / 2).max(16),
                        current.saturating_mul(2),
                    )
                    .clamp(16, perturb_chunk_ceiling());
                self.chunk_iters = chunk;
                return chunk;
            }
        }

        let now = web_time::Instant::now();
        self.chunk_count = self.chunk_count.saturating_add(1);
        let Some(last) = self.chunk_last.replace(now) else {
            // First chunk: seed, and start the clock.
            self.chunk_iters = seed;
            self.chunk_count = 0;
            return seed;
        };
        let elapsed_ms = now.duration_since(last).as_secs_f32() * 1000.0;
        if elapsed_ms > 0.0 {
            self.chunk_base_ms = self.chunk_base_ms.min(elapsed_ms);
        }
        let target = if let Some(forced) = self.chunk_target_env {
            forced
        } else if self.chunk_target_ms > 0.0 {
            self.chunk_target_ms
        } else {
            // Baseline plus headroom: under vsync this lands a little
            // above the refresh period (grow until the chunk eats the
            // frame's slack, then stop); with vsync off the 12 ms
            // floor keeps the UI at ~60+ fps rather than chasing a
            // 2 ms baseline.
            (self.chunk_base_ms + 8.0).clamp(12.0, 33.0)
        };
        let mut chunk = self.chunk_iters.max(seed);
        self.chunk_since_growth = self.chunk_since_growth.saturating_add(1);
        if elapsed_ms < target {
            // The chunk just measured came in on time, so it is
            // proven-completable work: growth may build on it.
            self.chunk_proven = self.chunk_proven.max(chunk);
            let cap = self
                .chunk_proven
                .max(seed)
                .saturating_mul(Self::GROWTH_HEADROOM)
                .min(perturb_chunk_ceiling());
            if self.chunk_since_growth >= Self::GROWTH_INTERVAL && chunk < cap {
                chunk = chunk.saturating_mul(2).min(cap);
                self.chunk_since_growth = 0;
            }
        } else if elapsed_ms > target * 1.4 {
            chunk = (chunk / 2).max(16);
            self.chunk_since_growth = 0;
        }
        self.chunk_iters = chunk;
        chunk
    }

    fn chunk_size(&self, floatexp: bool) -> u32 {
        #[cfg(test)]
        if let Some(c) = self.chunk_override {
            return c;
        }
        perturb_chunk_seed(floatexp, self.width as u64 * self.height as u64)
    }

    /// Everything that invalidates in-flight chunk state.
    /// A reference predicted to take longer than this is waited for
    /// rather than rendered against progressively. Low enough that
    /// anything interactive still refines in front of the user, high
    /// enough that a minutes-long build does not flash flat colour
    /// the whole time.
    const ORBIT_WAIT_SECONDS: f64 = 0.75;

    fn chunk_key_for(&self, escape: &EscapeConfig, orbit_tag: u64, orbit_done: bool) -> String {
        format!(
            "{}|{:?}|{}|{}|{}|{}|{}|{}|{:?}|{}x{}|{}|{}",
            escape.formula,
            escape.formula_params,
            escape.coloring,
            escape.center_re,
            escape.center_im,
            escape.zoom_log2,
            escape.max_iter,
            escape.julia,
            escape.coloring_params,
            self.width,
            self.height,
            orbit_tag,
            orbit_done,
        )
    }

    fn ensure_iter_state(&mut self, device: &Device, stride: u64) {
        let px = self.width * self.height;
        if self.iter_state_px != px
            || self.iter_state_stride != stride
            || self.iter_state_buffer.is_none()
        {
            if let Some(old) = self.iter_state_buffer.take() {
                old.destroy();
            }
            self.iter_state_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Iter State"),
                size: (px as u64) * stride,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: false,
            }));
            self.iter_state_px = px;
            self.iter_state_stride = stride;
        }
    }

    /// How deep this config can usefully go, for the UI to say so.
    ///
    /// Two tiers, and the difference is four thousand octaves: a
    /// formula with a perturbation tier iterates deltas against a
    /// fixed-point reference and keeps resolving essentially without
    /// limit, while the rest run the direct path, whose f32 pixel
    /// mapping stops separating neighbouring pixels at about zoom 14.
    /// Past that the image is honest mush, and the panel should say
    /// which case the user is in rather than letting them zoom into a
    /// wash and wonder what broke.
    pub fn usable_depth(escape: &EscapeConfig) -> UsableDepth {
        if Self::perturb_tier(escape).is_some()
            && !escape.is_damped()
            && escape.biomorph == crate::config::escape::BiomorphMode::Off
        {
            UsableDepth::Perturbed
        } else {
            UsableDepth::Direct(PERTURB_MIN_ZOOM)
        }
    }

    /// Why a derivative-based coloring has nothing to work with here,
    /// or `None` when it does.
    ///
    /// `distance_estimate` and `normal_map` read `sum.dz`, and when no
    /// derivative is compiled that is the constant seed rather than a
    /// derivative — so both return a flat value instead of a
    /// confident wrong one. Flat is honest but silent, and the panel
    /// is where the silence gets explained. Pinned against the
    /// assembler's own `HAS_DERIVATIVE` decision by test, so the two
    /// cannot drift.
    pub fn derivative_gap(escape: &EscapeConfig) -> Option<DerivativeGap> {
        // The deep rungs do not iterate a derivative orbit at all, so
        // this outranks the formula: a Mandelbrot dive past
        // PERTURB_MIN_ZOOM loses its derivative even though the
        // formula defines one.
        if Self::wants_perturbation(escape) {
            return Some(DerivativeGap::Perturbed);
        }
        if super::fields::get_field(&escape.formula).is_some() {
            return Some(DerivativeGap::Formula);
        }
        if super::get_formula(&escape.formula).wgsl_derivative.is_empty() {
            return Some(DerivativeGap::Formula);
        }
        None
    }

    /// Whether this view renders through the perturbation path:
    /// Mandelbrot parameter plane, plain iteration, past the direct
    /// path's f32 ceiling. Everything else stays direct (and deep
    /// zooms of unsupported combinations render the direct path's
    /// f32 mush honestly rather than wrong perturbation math).
    fn wants_perturbation(escape: &EscapeConfig) -> bool {
        escape.zoom_log2 > PERTURB_MIN_ZOOM
            && Self::perturb_tier(escape).is_some()
            && !escape.is_damped()
            && escape.biomorph == crate::config::escape::BiomorphMode::Off
    }

    /// Rows the direct path may cover in one dispatch.
    ///
    /// The direct and field templates have no per-pixel resume state,
    /// so a whole render is one dispatch: cost is pixels x max_iter
    /// with nothing bounding it. Fine at ordinary iteration counts,
    /// fatal when a deep-zoom config keeps its max_iter and the view
    /// zooms OUT past the perturbation threshold - tens of seconds in
    /// one submission, which Windows answers by resetting the driver
    /// and wgpu by aborting the process (0xc0000409). Reported from an
    /// app session zooming out of an f3-depth location, and again from
    /// an animation whose zoom track crossed the same line.
    ///
    /// Splitting by ROW BAND bounds the dispatch without needing any
    /// resume state: each band is a complete render of its own rows,
    /// and the output texture accumulates them. It also gives the
    /// direct path progressive top-to-bottom feedback it never had.
    fn direct_rows_per_dispatch(&self, escape: &EscapeConfig) -> u32 {
        let per_row = (self.width as u64).saturating_mul(escape.max_iter.max(1) as u64);
        // The session shift only ever shrinks (see DIRECT_BUDGET_SHIFT),
        // and carries over from previous sessions.
        tuning::ensure_loaded();
        let budget = DIRECT_DISPATCH_BUDGET
            >> DIRECT_BUDGET_SHIFT.load(std::sync::atomic::Ordering::Relaxed);
        let rows = budget / per_row.max(1);
        (rows.max(1) as u32).min(self.height.max(1))
    }


    /// The map's continuous parameter, which is part of the reference
    /// orbit's identity (a different `p` is a different orbit, so a
    /// cache keyed without it would silently reuse a stale one).
    /// Zero for every family that has no such parameter.
    fn map_params_for(escape: &EscapeConfig) -> [f32; 2] {
        // Manowar is the same two-term map with p = 1, fixed by the
        // formula rather than by a parameter.
        if escape.formula == "manowar" {
            return [1.0, 0.0];
        }
        // McMullen carries its POLE POWER here: a MapId has one
        // integer exponent (n) and this family needs two. Resolved
        // through the registry defaults for the same reason Phoenix's
        // p is, below -- an absent key means "the default", and
        // reading it as zero built the reference for c/z^1 while the
        // delta step used c/z^3. Measured: every pixel escaped.
        // Magnet's VARIANT selects between two different maps, so it
        // is part of the reference orbit's identity.
        if escape.formula == "magnet" {
            let v = escape
                .formula_params
                .get("variant")
                .copied()
                .unwrap_or_else(|| {
                    super::get_formula("magnet")
                        .parameters
                        .first()
                        .map_or(0.0, |d| d.default)
                });
            return [v, 0.0];
        }
        if escape.formula == "mcmullen" {
            let m = escape
                .formula_params
                .get("m")
                .copied()
                .unwrap_or_else(|| {
                    super::get_formula("mcmullen")
                        .parameters
                        .iter()
                        .find(|d| d.name == "m")
                        .map_or(3.0, |d| d.default)
                });
            return [m, 0.0];
        }
        if escape.formula != "phoenix" {
            return [0.0, 0.0];
        }
        // Resolve through the REGISTRY DEFAULTS, exactly as
        // `pack_params` does when filling the shader's uniform. An
        // absent key means "the default", not zero -- and reading it
        // as zero here built the reference for p = 0, i.e. the plain
        // quadratic, while the delta step used the real p. The two
        // then describe different maps and the perturbed render is a
        // different fractal, which is precisely what a fresh Phoenix
        // config did (a config that had been edited carried the keys
        // and worked, which is why the agreement test missed it).
        let defs = super::get_formula("phoenix").parameters;
        let mut out = [0.0f32; 2];
        for (i, def) in defs.iter().take(2).enumerate() {
            out[i] = escape
                .formula_params
                .get(def.name)
                .copied()
                .unwrap_or(def.default);
        }
        out
    }

    /// The delta tier this view can use, if any: Mandelbrot (p = 2)
    /// and integer-power Multibrot (the binomial expansion needs an
    /// integer exponent), the Burning Ship variants via diffabs,
    /// Tricorn/Multicorn via the conjugated binomial, and Phoenix's
    /// two-term recurrence. Every one of them has BOTH rungs, so a
    /// tier is either supported at all depths or not supported at
    /// all — a tier missing its deep rung silently becomes the direct
    /// path's f32 mush at the threshold, which is how Phoenix came to
    /// render one flat colour past zoom 48.
    pub(crate) fn perturb_tier(escape: &EscapeConfig) -> Option<assembler::PerturbTier> {
        match escape.formula.as_str() {
            "mandelbrot" => Some(assembler::PerturbTier::Power(2)),
            "multibrot" => {
                let p = escape.formula_params.get("power").copied().unwrap_or(3.0);
                let rounded = p.round();
                if (p - rounded).abs() < 1e-6 && (2.0..=12.0).contains(&rounded) {
                    Some(assembler::PerturbTier::Power(rounded as u32))
                } else {
                    None
                }
            }
            "phoenix" => Some(assembler::PerturbTier::Phoenix),
            // c*z*(1-z): the first tier whose parameter MULTIPLIES,
            // so its delta step reads the reference's own c.
            "lambda" => Some(assembler::PerturbTier::Lambda),
            // c*z^p over a component-wise denominator. Gated off
            // 2026-08-30 when its slow-growth escape boundary exposed
            // the f32 escape test's delta-blindness; re-enabled the
            // same day once the delta-aware margin (ref_r2 + the
            // margin test in both rung templates) resolved it -- the
            // full story is in escape-time-fractals.md.
            // JULIA ONLY. Our McMullen seeds its parameter plane at
            // z_0 = c, which is not a critical point of this map --
            // measured, 0 of 4000 sampled parameters have a bounded
            // orbit, so that plane has no interior to zoom into. The
            // classic Sierpinski-carpet pictures are Julia sets.
            "magnet" => {
                let v = escape.formula_params.get("variant").copied().unwrap_or(0.0);
                let rv = v.round();
                if (v - rv).abs() < 1e-6 && (0.0..=1.0).contains(&rv) {
                    Some(assembler::PerturbTier::Magnet(rv as u32))
                } else {
                    None
                }
            }
            "mcmullen" if escape.julia => {
                let n = escape.formula_params.get("n").copied().unwrap_or(2.0);
                let m = escape.formula_params.get("m").copied().unwrap_or(3.0);
                let (rn, rm) = (n.round(), m.round());
                if (n - rn).abs() < 1e-6
                    && (m - rm).abs() < 1e-6
                    && (2.0..=8.0).contains(&rn)
                    && (1.0..=8.0).contains(&rm)
                {
                    Some(assembler::PerturbTier::McMullen(rn as u32, rm as u32))
                } else {
                    None
                }
            }
            "feather" => {
                let p = escape.formula_params.get("power").copied().unwrap_or(3.0);
                let rounded = p.round();
                if (p - rounded).abs() < 1e-6 && (2.0..=8.0).contains(&rounded) {
                    Some(assembler::PerturbTier::Feather(rounded as u32))
                } else {
                    None
                }
            }
            // z^2 + z_prev + c: Phoenix's recurrence with p = 1 and a
            // pixel seed, so it rides the same two-term machinery.
            "manowar" => Some(assembler::PerturbTier::Manowar),
            "tricorn" => {
                // conj(z)^p: the binomial expansion needs an integer
                // exponent, exactly as Multibrot does.
                let p = escape.formula_params.get("power").copied().unwrap_or(2.0);
                let rounded = p.round();
                if (p - rounded).abs() < 1e-6 && (2.0..=12.0).contains(&rounded) {
                    Some(assembler::PerturbTier::Tricorn(rounded as u32))
                } else {
                    None
                }
            }
            "burning_ship" => {
                let variant = escape.formula_params.get("variant").copied().unwrap_or(0.0);
                let v = variant.round();
                if (variant - v).abs() < 1e-6 && (0.0..=5.0).contains(&v) {
                    Some(assembler::PerturbTier::Ship(v as u32))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Ensure the perturbed pipeline for this coloring exists.
    fn ensure_perturbed_pipeline(
        &mut self,
        device: &Device,
        escape: &EscapeConfig,
        floatexp: bool,
    ) -> String {
        let coloring = super::get_coloring(&escape.coloring);
        let tier = Self::perturb_tier(escape)
            .unwrap_or(assembler::PerturbTier::Power(2));
        let key = format!("perturbed|{}|{}|{:?}", coloring.name, floatexp, tier);
        if !self.pipelines.contains_key(&key) {
            let source = assembler::assemble_perturbed(coloring, floatexp, tier);
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some(&format!("Escape Shader {key}")),
                source: ShaderSource::Wgsl(source.into()),
            });
            let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Escape Perturbed Pipeline Layout"),
                bind_group_layouts: &[Some(&self.perturb_bind_group_layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(&format!("Escape Pipeline {key}")),
                layout: Some(&layout),
                module: &module,
                entry_point: Some("escape_main"),
                compilation_options: Default::default(),
                cache: None,
            });
            self.pipelines.insert(key.clone(), pipeline);
        }
        key
    }

    /// Progressive orbit acquisition: post the request to the worker,
    /// upload whatever prefix has landed. Returns
    /// (usable_len, complete); (0, _) means nothing to render yet.
    #[cfg(not(target_arch = "wasm32"))]
    fn ensure_orbit_progressive(
        &mut self,
        device: &Device,
        queue: &Queue,
        escape: &EscapeConfig,
    ) -> (u32, bool) {
        use super::reference::{OrbitRequest, OrbitWorker};
        let julia_c = if escape.julia {
            Some((escape.julia_re, escape.julia_im))
        } else {
            None
        };
        let tier = Self::perturb_tier(escape)
            .unwrap_or(assembler::PerturbTier::Power(2));
        let (power, ship, ship_variant) = match tier {
            assembler::PerturbTier::Power(p) => (p, false, 0),
            assembler::PerturbTier::Ship(v) => (2, true, v),
            assembler::PerturbTier::Tricorn(p) => {
                (p, false, super::reference::MAP_CONJ)
            }
            assembler::PerturbTier::Phoenix => {
                (2, false, super::reference::MAP_PHOENIX)
            }
            assembler::PerturbTier::Manowar => {
                (2, false, super::reference::MAP_MANOWAR)
            }
            assembler::PerturbTier::Lambda => {
                (2, false, super::reference::MAP_LAMBDA)
            }
            assembler::PerturbTier::Feather(p) => {
                (p, false, super::reference::MAP_FEATHER)
            }
            assembler::PerturbTier::McMullen(n, _) => {
                (n, false, super::reference::MAP_MCMULLEN)
            }
            assembler::PerturbTier::Magnet(_) => {
                (2, false, super::reference::MAP_MAGNET)
            }
        };
        let height_px = self.height.max(1) as f64;
        let worker = self.orbit_worker.get_or_insert_with(OrbitWorker::new);
        let epoch = worker.request(OrbitRequest {
            center_re: escape.center_re.clone(),
            center_im: escape.center_im.clone(),
            n_limbs: super::fixedpoint::limbs_for_view(
                &escape.center_re,
                &escape.center_im,
                escape.zoom_log2,
            ),
            max_iter: escape.max_iter,
            julia_c,
            power,
            ship,
            ship_variant,
            map_params: Self::map_params_for(escape),
            reference_period: if julia_c.is_none() && !ship {
                escape.reference_period.filter(|&p| p > 0)
            } else {
                None
            },
            zoom_log2: escape.zoom_log2,
            height_px,
        });
        let (len, done, gen, data, data_lo, data_e) = {
            let p = worker.progress.lock().unwrap();
            if p.epoch == epoch {
                // Rescale to this view (see the blocking path). The
                // reuse guard retires orbits whose relocation can't
                // rescale, so a None here is at most one transitional
                // frame — render it centered rather than displaced.
                self.current_ref_offset = super::reference::rescale_offset(
                    p.ref_offset,
                    p.off_zoom_log2,
                    p.off_height_px,
                    escape.zoom_log2,
                    self.height.max(1) as f64,
                )
                .unwrap_or([0.0, 0.0]);
            }
            if p.epoch == epoch {
                super::reference::set_live_reference_period(p.detected_period);
            }
            if p.epoch != epoch {
                (0u32, false, 0u64, Vec::new(), Vec::new(), Vec::new())
            } else {
                // Append onto what is already uploaded when the GPU
                // mirror holds this same orbit CONTENT (generation);
                // a mere epoch change -- a zoom tick reusing the
                // orbit -- must not restart the upload.
                let start = if self.orbit_generation == p.generation {
                    self.orbit_uploaded as usize
                } else {
                    0
                };
                (
                    p.orbit.len() as u32,
                    p.done,
                    p.generation,
                    p.orbit[start.min(p.orbit.len())..].to_vec(),
                    p.orbit_lo[start.min(p.orbit_lo.len())..].to_vec(),
                    p.orbit_e[start.min(p.orbit_e.len())..].to_vec(),
                )
            }
        };
        if len < 2 {
            return (0, false);
        }
        // (Re)create the buffer as needed, then append the new tail.
        let recreate = self.orbit_buffer.is_none() || len > self.orbit_capacity;
        // A recreated buffer must be filled FROM SCRATCH even under an
        // unchanged generation: the append branch writes only the new
        // tail, so a capacity crossing mid-growth otherwise leaves
        // everything before that tail garbage in the new buffer. This
        // was the cold-vs-warm divergence a user caught by eye: a
        // reference STREAMED over minutes crosses several capacity
        // boundaries and rendered a structurally wrong (self-similar
        // sibling) frame, while the same orbit reloaded complete from
        // the store uploads whole in one pass and renders correctly.
        let fresh = recreate || self.orbit_generation != gen;
        if recreate {
            if let Some(old) = self.orbit_buffer.take() {
                old.destroy();
            }
            if let Some(old) = self.orbit_lo_buffer.take() {
                old.destroy();
            }
            if let Some(old) = self.orbit_r2_buffer.take() {
                old.destroy();
            }
            if let Some(old) = self.orbit_e_buffer.take() {
                old.destroy();
            }
            let capacity = (len + len / 2).max(1024);
            self.orbit_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit"),
                size: (capacity as u64) * 8,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.orbit_lo_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit Lo"),
                size: (capacity as u64) * 8,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.orbit_r2_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit R2"),
                size: (capacity as u64) * 8,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.orbit_e_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit Exp"),
                size: (capacity as u64) * 4,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.orbit_capacity = capacity;
            self.orbit_uploaded = 0;
        }
        if fresh {
            // New orbit content: upload from scratch.
            let worker = self.orbit_worker.as_ref().unwrap();
            let p = worker.progress.lock().unwrap();
            queue.write_buffer(
                self.orbit_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&p.orbit),
            );
            queue.write_buffer(
                self.orbit_lo_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&p.orbit_lo),
            );
            queue.write_buffer(
                self.orbit_r2_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&r2_channel(&p.orbit, &p.orbit_lo, &p.orbit_e)),
            );
            queue.write_buffer(
                self.orbit_e_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&p.orbit_e),
            );
            self.orbit_uploaded = p.orbit.len() as u32;
            self.orbit_generation = p.generation;
        } else if !data.is_empty() {
            queue.write_buffer(
                self.orbit_buffer.as_ref().unwrap(),
                (self.orbit_uploaded as u64) * 8,
                bytemuck::cast_slice(&data),
            );
            queue.write_buffer(
                self.orbit_lo_buffer.as_ref().unwrap(),
                (self.orbit_uploaded as u64) * 8,
                bytemuck::cast_slice(&data_lo),
            );
            queue.write_buffer(
                self.orbit_r2_buffer.as_ref().unwrap(),
                (self.orbit_uploaded as u64) * 8,
                bytemuck::cast_slice(&r2_channel(&data, &data_lo, &data_e)),
            );
            queue.write_buffer(
                self.orbit_e_buffer.as_ref().unwrap(),
                (self.orbit_uploaded as u64) * 4,
                bytemuck::cast_slice(&data_e),
            );
            self.orbit_uploaded += data.len() as u32;
        }
        (self.orbit_uploaded, done)
    }

    /// Compute/extend the reference orbit and mirror it to the GPU.
    /// Returns the usable orbit length, or None if the center failed
    /// to parse (caller falls back to the direct path).
    fn ensure_orbit(&mut self, device: &Device, queue: &Queue, escape: &EscapeConfig) -> Option<u32> {
        self.ensure_orbit_with(device, queue, escape, None).map(|(len, _)| len)
    }

    /// Blocking (`budget` None) or time-sliced (`budget` Some) orbit
    /// acquisition + GPU mirror. The sliced form is the WASM path's
    /// per-frame call — no worker thread exists there, so the orbit
    /// grows a bounded amount each frame and partial-orbit renders
    /// refine via rebasing, exactly like the desktop worker's
    /// progressive prefixes.
    fn ensure_orbit_with(
        &mut self,
        device: &Device,
        queue: &Queue,
        escape: &EscapeConfig,
        budget: Option<u32>,
    ) -> Option<(u32, bool)> {
        let julia_c = if escape.julia {
            Some((escape.julia_re, escape.julia_im))
        } else {
            None
        };
        let tier = Self::perturb_tier(escape)
            .unwrap_or(assembler::PerturbTier::Power(2));
        let (power, ship, ship_variant) = match tier {
            assembler::PerturbTier::Power(p) => (p, false, 0),
            assembler::PerturbTier::Ship(v) => (2, true, v),
            assembler::PerturbTier::Tricorn(p) => {
                (p, false, super::reference::MAP_CONJ)
            }
            assembler::PerturbTier::Phoenix => {
                (2, false, super::reference::MAP_PHOENIX)
            }
            assembler::PerturbTier::Manowar => {
                (2, false, super::reference::MAP_MANOWAR)
            }
            assembler::PerturbTier::Lambda => {
                (2, false, super::reference::MAP_LAMBDA)
            }
            assembler::PerturbTier::Feather(p) => {
                (p, false, super::reference::MAP_FEATHER)
            }
            assembler::PerturbTier::McMullen(n, _) => {
                (n, false, super::reference::MAP_MCMULLEN)
            }
            assembler::PerturbTier::Magnet(_) => {
                (2, false, super::reference::MAP_MAGNET)
            }
        };
        let period_hint = if julia_c.is_none() && !ship {
            escape.reference_period.filter(|&p| p > 0)
        } else {
            None
        };
        self.orbit_cache.set_reference_period(period_hint);
        self.orbit_cache.set_height(self.height.max(1) as f64);
        // Retire a cached relocation this view can't express (zoomed
        // far out from where its nucleus was found) BEFORE borrowing
        // the slot — the recompute then happens inside get() at the
        // current view.
        if self
            .orbit_cache
            .peek()
            .is_some_and(|o| !o.relocation_serves(escape.zoom_log2, self.height.max(1) as f64))
        {
            self.orbit_cache.clear();
        }
        let done = match budget {
            None => {
                self.orbit_cache.get(
                    &escape.center_re,
                    &escape.center_im,
                    escape.zoom_log2,
                    escape.max_iter,
                    julia_c,
                    power,
                    ship,
                    ship_variant,
                    Self::map_params_for(escape),
                )?;
                true
            }
            Some(b) => {
                self.orbit_cache.get_budgeted(
                    &escape.center_re,
                    &escape.center_im,
                    escape.zoom_log2,
                    escape.max_iter,
                    julia_c,
                    power,
                    ship,
                    ship_variant,
                    Self::map_params_for(escape),
                    b,
                )?
                .1
            }
        };
        // Re-borrow immutably: the generation read must not overlap
        // get()'s mutable borrow, and the slot is guaranteed Some
        // after a successful get.
        let gen = self.orbit_cache.generation();
        let orbit = self.orbit_cache.peek()?;
        // The offset's pixel units are rescaled to THIS view (zoom
        // drags and supersampling change the units under a reused
        // orbit). A fresh orbit is always at the current view, so the
        // rescale is the identity there and the fallback never fires
        // after the pre-borrow retirement above.
        let h_px = self.height.max(1) as f64;
        self.current_ref_offset = orbit
            .offset_for_view(escape.zoom_log2, h_px)
            .unwrap_or([0.0, 0.0]);
        super::reference::set_live_reference_period(orbit.periodic);
        let len = orbit.len();
        let needed_bytes = (len as u64) * 8;
        let recreate = match &self.orbit_buffer {
            Some(_) => len > self.orbit_capacity,
            None => true,
        };
        if recreate {
            if let Some(old) = self.orbit_buffer.take() {
                old.destroy();
            }
            if let Some(old) = self.orbit_lo_buffer.take() {
                old.destroy();
            }
            if let Some(old) = self.orbit_r2_buffer.take() {
                old.destroy();
            }
            if let Some(old) = self.orbit_e_buffer.take() {
                old.destroy();
            }
            // Grow with headroom so deepening doesn't recreate every
            // frame.
            let capacity = (len + len / 2).max(1024);
            self.orbit_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit"),
                size: (capacity as u64) * 8,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.orbit_lo_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit Lo"),
                size: (capacity as u64) * 8,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.orbit_r2_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit R2"),
                size: (capacity as u64) * 8,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.orbit_e_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit Exp"),
                size: (capacity as u64) * 4,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.orbit_capacity = capacity;
            self.orbit_uploaded = 0;
        }
        if self.orbit_uploaded != len || self.orbit_generation != gen {
            // Upload the whole orbit (append-only uploads are a later
            // optimization; a full orbit at max_iter 100k is 800 KB).
            // The generation compare matters even at equal length: a
            // pan at fixed max_iter swaps in a DIFFERENT orbit of the
            // same length, which a length test alone leaves stale on
            // the GPU.
            queue.write_buffer(
                self.orbit_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&orbit.orbit),
            );
            queue.write_buffer(
                self.orbit_lo_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&orbit.orbit_lo),
            );
            queue.write_buffer(
                self.orbit_r2_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&r2_channel(&orbit.orbit, &orbit.orbit_lo, &orbit.orbit_e)),
            );
            queue.write_buffer(
                self.orbit_e_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&orbit.orbit_e),
            );
            self.orbit_uploaded = len;
            self.orbit_generation = gen;
        }
        Some((len, done))
    }

    /// The scalar-field target the relief pass differences. R32Float:
    /// one channel is all the surface needs, and at 4 bytes per render
    /// pixel it is a quarter of what carrying an analytic normal
    /// alongside it would cost.
    fn create_height(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Escape Height Field"),
            size: Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R32Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        (texture, view)
    }

    /// Size the height field to match the render target, or shrink it
    /// back to the 1×1 dummy when shading is off — an idle escape view
    /// should not hold 81 MB of scalar field it never reads (the cost
    /// at 4500², which is 3× supersampling of a 1500² viewport).
    fn ensure_height(&mut self, device: &Device, want_full: bool) {
        if want_full == self.height_full
            && (!want_full
                || (self.height_texture.width() == self.width
                    && self.height_texture.height() == self.height))
        {
            return;
        }
        self.height_texture.destroy();
        let (t, v) = if want_full {
            Self::create_height(device, self.width, self.height)
        } else {
            Self::create_height(device, 1, 1)
        };
        self.height_texture = t;
        self.height_view = v;
        self.height_full = want_full;
    }

    /// The relief pass writes somewhere other than the colour it
    /// reads, so it needs the display-sized target that supersampling
    /// already allocates. At 1x with shading on there is no downsample
    /// to borrow it from, so allocate one; turning shading back off at
    /// 1x releases it and `output_view` goes back to serving the render
    /// texture directly.
    fn ensure_resolve_target(&mut self, device: &Device, shading: bool) {
        let want = self.supersample > 1 || shading;
        let sized_right = self
            .final_texture
            .as_ref()
            .is_some_and(|t| t.width() == self.out_width && t.height() == self.out_height);
        if want && sized_right {
            return;
        }
        if let Some(t) = self.final_texture.take() {
            t.destroy();
        }
        self.final_view = None;
        if want {
            let (t, v) = Self::create_output(device, self.out_width, self.out_height);
            self.final_texture = Some(t);
            self.final_view = Some(v);
        }
    }

    fn create_output(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Escape Output"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            // Storage write from the compute pass; read (textureLoad)
            // by the tonemap pass and the density-effect chain.
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        (texture, view)
    }

    /// The rendered image, in the flame-accumulator format the tonemap
    /// tail expects — display-sized (the downsampled target when
    /// supersampling is on).
    pub fn output_view(&self) -> &TextureView {
        self.final_view.as_ref().unwrap_or(&self.output_view)
    }

    /// Recreate the output for new DISPLAY dimensions and
    /// supersampling factor (render size = display × factor). Cheap
    /// relative to a render; pipelines and params survive. Returns
    /// true when anything changed — the output is stale until the
    /// next `render`.
    pub fn resize(&mut self, device: &Device, width: u32, height: u32, supersample: u32) -> bool {
        let mut ss = supersample.clamp(1, 3);
        // Cap the RENDER pixel count: the perturbed path carries up
        // to ITER_STATE_BYTES_MAX per pixel of iteration state (plus
        // the 16 B/px accumulator),
        // and an unbounded supersample x display product is a device
        // OOM (observed as a device-loss abort). 32 Mpx ~ 1.5 GB of
        // state — reduce the factor until it fits rather than crash.
        const MAX_RENDER_PX: u64 = 32 * 1024 * 1024;
        // The perturbed path binds its iteration state as ONE storage
        // buffer; the device's binding limit (browsers often grant far
        // less than desktop adapters) caps render pixels harder than
        // the fixed ceiling. Sized for the WIDEST tier: the cap is
        // computed at resize, which does not know which formula will
        // be rendered into the surface afterwards.
        let device_px_cap = device.limits().max_storage_buffer_binding_size as u64
            / assembler::ITER_STATE_BYTES_MAX;
        let px_cap = MAX_RENDER_PX.min(device_px_cap.max(1))
            .min({
                let d = device.limits().max_texture_dimension_2d as u64;
                d * d
            });
        while ss > 1
            && (width as u64 * ss as u64) * (height as u64 * ss as u64) > px_cap
        {
            ss -= 1;
        }
        if ss != supersample.clamp(1, 3) {
            log::warn!(
                "Escape supersample clamped to {ss}x at {width}x{height} (render-pixel budget)"
            );
        }
        if width == self.out_width && height == self.out_height && ss == self.supersample {
            return false;
        }
        self.out_width = width;
        self.out_height = height;
        self.supersample = ss;
        let (rw, rh) = (width.saturating_mul(ss).max(1), height.saturating_mul(ss).max(1));
        self.output_texture.destroy();
        let (texture, view) = Self::create_output(device, rw, rh);
        self.output_texture = texture;
        self.output_view = view;
        self.width = rw;
        self.height = rh;
        if let Some(t) = self.final_texture.take() {
            t.destroy();
        }
        self.final_view = None;
        if ss > 1 {
            let (t, v) = Self::create_output(device, width, height);
            self.final_texture = Some(t);
            self.final_view = Some(v);
        }
        true
    }

    /// The box-downsample pass: render texture → display texture,
    /// factor² samples averaged per output pixel. Linear-space (the
    /// texture is pre-tonemap accumulator format), so the average is
    /// the radiometrically correct one.
    /// Shade and resolve: the relief layer and the supersample box
    /// downsample, fused into one pass.
    ///
    /// Fused deliberately. Shading needs a destination distinct from
    /// the colour it reads, and a third full-resolution RGBA32Float
    /// target would cost another 16 bytes per render pixel — 324 MB at
    /// 4500², on top of the 324 MB the colour already holds. Folding
    /// the relief into the pass that was going to resolve anyway costs
    /// nothing extra, and it puts the shading BEFORE the box average,
    /// which is the correct order: the slope is then measured at
    /// render resolution and antialiased along with everything else,
    /// rather than being computed from already-blurred pixels.
    ///
    /// With shading off and a factor of 1 there is nothing to do and
    /// `output_view` serves the render texture directly, exactly as
    /// before this pass existed.
    fn run_resolve(&mut self, device: &Device, queue: &Queue, encoder: &mut CommandEncoder, shading: &crate::config::escape::EscapeShading) {
        let factor = self.supersample;
        let shade_on = shading.enabled;
        if factor <= 1 && !shade_on {
            return;
        }
        let Some(final_view) = self.final_view.as_ref() else {
            return;
        };

        // The light: azimuth from the config, elevation fixed at 45°.
        // One angle rather than two because the elevation is the one
        // nobody adjusts — too low and the relief is all shadow, too
        // high and it vanishes — while the azimuth decides whether the
        // surface reads as raised or sunken, which is a real choice.
        let a = shading.light_angle.to_radians();
        let params = ShadeParamsGpu {
            light: [a.cos(), a.sin()],
            // Per DISPLAY pixel, not per render pixel: the difference
            // is taken on the supersampled grid, where a given slope
            // spans `factor` times as many samples and would read
            // `factor` times shallower. Without this, turning on 3x AA
            // quietly flattens the relief.
            height: shading.height * factor as f32,
            enabled: u32::from(shade_on),
            shadow_color: shading.shadow_color,
            shadow_strength: shading.shadow_strength,
            highlight_color: shading.highlight_color,
            highlight_strength: shading.highlight_strength,
            shadow_blend: shading.shadow_blend.to_gpu(),
            highlight_blend: shading.highlight_blend.to_gpu(),
            // Display pixels to render pixels, for the same reason the
            // height is scaled: a radius fixed in render pixels would
            // shrink as antialiasing raised the resolution.
            softness: shading.softness * factor as f32,
            _pad: 0,
        };
        queue.write_buffer(&self.shade_params_buffer, 0, bytemuck::bytes_of(&params));

        let rebuild = match &self.downsample {
            Some((f, _, _)) => *f != factor,
            None => true,
        };
        if rebuild {
            let src = format!(
                r#"
struct ShadeParams {{
    light: vec2<f32>,
    height: f32,
    enabled: u32,
    shadow_color: vec3<f32>,
    shadow_strength: f32,
    highlight_color: vec3<f32>,
    highlight_strength: f32,
    shadow_blend: u32,
    highlight_blend: u32,
    softness: f32,
}}

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var dst_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var height_tex: texture_2d<f32>;
@group(0) @binding(3) var<uniform> shade: ShadeParams;

fn height_at(p: vec2<i32>, dims: vec2<i32>) -> f32 {{
    let q = clamp(p, vec2<i32>(0, 0), dims - vec2<i32>(1, 1));
    return textureLoad(height_tex, q, 0).r;
}}

// 0 multiply, 1 screen, 2 overlay, 3 mix. `amt` is how far to travel
// from the base toward the blended result, so strength 0 is always
// exactly the untouched image.
fn shade_blend(base: vec3<f32>, layer: vec3<f32>, mode: u32, amt: f32) -> vec3<f32> {{
    // BLENDED IN A PERCEPTUAL SPACE, NOT IN LINEAR LIGHT, and that is
    // what makes the two strength sliders mean the same thing.
    //
    // The escape pass emits linear light (the palette is raised to
    // 2.2 on lookup). Multiplying linear light by black is what a
    // shadow physically does -- but on a dark base there is almost
    // nothing to take away, while `screen` toward white has the whole
    // range to add into, and the tonemap's gamma then expands the dark
    // end further. Measured on the shipped relief config, a
    // full-strength black shadow moved 22.45/255 where a full-strength
    // white highlight moved 52.53: the same control reading 2.3x
    // weaker on one side, and far worse on a darker image (reported
    // from the app as needing strength 1.0 against 0.03).
    //
    // Converting to ~sRGB first makes every mode reach its extreme at
    // amt = 1 regardless of the base: multiply lands on the layer
    // colour, screen lands on it too, and the sliders are symmetric.
    // It also fixes the layer COLOUR, which comes from the picker in
    // sRGB and was previously composited against linear light.
    let inv_g = 1.0 / 2.2;
    let bp = pow(max(base, vec3<f32>(0.0)), vec3<f32>(inv_g, inv_g, inv_g));
    var res = layer;
    if (mode == 0u) {{
        res = bp * layer;
    }} else if (mode == 1u) {{
        res = 1.0 - (1.0 - bp) * (1.0 - layer);
    }} else if (mode == 2u) {{
        res = select(
            1.0 - 2.0 * (1.0 - bp) * (1.0 - layer),
            2.0 * bp * layer,
            bp < vec3<f32>(0.5),
        );
    }}
    // CLAMPED: the strengths now range past 1 so a shadow can be
    // driven to saturation on an image with little room below it, and
    // an unclamped mix would extrapolate past the layer colour into
    // negative light instead of stopping there.
    let outp = mix(bp, res, clamp(amt, 0.0, 1.0));
    return pow(max(outp, vec3<f32>(0.0)), vec3<f32>(2.2, 2.2, 2.2));
}}

fn shade_pixel(rgb: vec3<f32>, p: vec2<i32>) -> vec3<f32> {{
    let dims = vec2<i32>(textureDimensions(height_tex));
    // Central differences: the slope of the coloring's own value
    // field, in value units per pixel.
    var dx: f32;
    var dy: f32;
    let sr = i32(round(shade.softness));
    if (sr <= 0) {{
        // The sharp estimator: a plain +-1 central difference. Kept
        // bit-for-bit as the default so every existing config renders
        // unchanged.
        dx = (height_at(p + vec2<i32>(1, 0), dims) - height_at(p - vec2<i32>(1, 0), dims)) * 0.5;
        dy = (height_at(p + vec2<i32>(0, 1), dims) - height_at(p - vec2<i32>(0, 1), dims)) * 0.5;
    }} else {{
        // A Sobel stencil widened to radius r. Two things soften here
        // and they are different: the RADIUS spreads the difference
        // over 2r pixels, and the 1-2-1 weighting PERPENDICULAR to
        // each axis averages across the gradient, which is what kills
        // the single-pixel wobble a plain wide difference would keep.
        // EIGHT taps regardless of r -- the four corners and the
        // four edge midpoints of the ring; both axes share them.
        let pp = height_at(p + vec2<i32>(sr, sr), dims);
        let pm = height_at(p + vec2<i32>(sr, -sr), dims);
        let mp = height_at(p + vec2<i32>(-sr, sr), dims);
        let mm = height_at(p + vec2<i32>(-sr, -sr), dims);
        let ep = height_at(p + vec2<i32>(sr, 0), dims);
        let em = height_at(p + vec2<i32>(-sr, 0), dims);
        let eu = height_at(p + vec2<i32>(0, sr), dims);
        let ed = height_at(p + vec2<i32>(0, -sr), dims);
        let inv = 1.0 / (8.0 * f32(sr));
        dx = ((pm + 2.0 * ep + pp) - (mm + 2.0 * em + mp)) * inv;
        dy = ((mp + 2.0 * eu + pp) - (mm + 2.0 * ed + pm)) * inv;
    }}
    // Exaggerated gradient. +y is DOWN in pixel space, so dy is
    // negated to put the light where the azimuth says it is.
    let g = vec2<f32>(-dx, dy) * shade.height;

    // THE SIGNED TILT TOWARD THE LIGHT, and not a Lambert dot product.
    //
    // `s` is the slope along the light's direction and the divide
    // turns it into sin(tilt angle) -- so the response runs -1..+1,
    // is exactly zero on flat ground, is MONOTONIC in the tilt, and
    // is symmetric: the same slope facing toward or away gives the
    // same magnitude to the highlight or the shadow.
    //
    // The Lambert version this replaces had none of those last two
    // properties, and both were visible. Because the normal's z
    // component is always positive, `dot(n, l)` could not fall below
    // -|l.xy| = -0.707, while the highlight side was normalized over
    // a span of only 1 - l.z = 0.293: at a 45-degree tilt the
    // highlight was already saturated at 1.000 while the shadow had
    // reached 0.414, which is why black-on-white at full strength
    // came out mid-grey. It was also non-monotonic -- a vertical wall
    // facing the light got NO highlight, since the dot product peaks
    // at 45 degrees and falls back.
    //
    // The divide doubles as the saturation, so an over-large `height`
    // walks the response toward +-1 instead of blowing out, which is
    // what makes one log slider workable across colorings whose value
    // scales differ by orders of magnitude.
    let s = dot(g, shade.light);
    let response = s * inverseSqrt(1.0 + dot(g, g));
    let hi = clamp(response, 0.0, 1.0);
    let lo = clamp(-response, 0.0, 1.0);
    var out = rgb;
    out = shade_blend(out, shade.shadow_color, shade.shadow_blend, lo * shade.shadow_strength);
    out = shade_blend(out, shade.highlight_color, shade.highlight_blend, hi * shade.highlight_strength);
    return out;
}}

@compute @workgroup_size(8, 8, 1)
fn downsample_main(@builtin(global_invocation_id) gid: vec3<u32>) {{
    let dims = textureDimensions(dst_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) {{
        return;
    }}
    var sum = vec4<f32>(0.0);
    for (var dy = 0u; dy < {factor}u; dy = dy + 1u) {{
        for (var dx = 0u; dx < {factor}u; dx = dx + 1u) {{
            let p = vec2<i32>(i32(gid.x * {factor}u + dx), i32(gid.y * {factor}u + dy));
            var texel = textureLoad(src_tex, p, 0);
            if (shade.enabled == 1u) {{
                texel = vec4<f32>(shade_pixel(texel.rgb, p), texel.a);
            }}
            sum = sum + texel;
        }}
    }}
    textureStore(
        dst_tex,
        vec2<i32>(i32(gid.x), i32(gid.y)),
        sum / f32({factor}u * {factor}u),
    );
}}
"#
            );
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some("Escape Resolve Shader"),
                source: ShaderSource::Wgsl(src.into()),
            });
            let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Escape Resolve Layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: TextureFormat::Rgba32Float,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
            let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Escape Resolve Pipeline Layout"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Escape Resolve Pipeline"),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some("downsample_main"),
                compilation_options: Default::default(),
                cache: None,
            });
            self.downsample = Some((factor, pipeline, layout));
        }
        let (_, pipeline, layout) = self.downsample.as_ref().unwrap();
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Escape Resolve Bind Group"),
            layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&self.output_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(final_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&self.height_view),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: self.shade_params_buffer.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Escape Resolve Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(self.out_width.div_ceil(8), self.out_height.div_ceil(8), 1);
    }

    /// Compile (or fetch from cache) the pipeline for this config's
    /// (formula, coloring) pair; returns its cache key.
    fn ensure_pipeline(&mut self, device: &Device, escape: &EscapeConfig) -> String {
        // Mode B routing: a formula name resolving in the FIELD
        // registry compiles the field template instead. Same bind
        // group layout, same dispatch — only the shader differs.
        let (key, source_for) = if let Some(field) = super::fields::get_field(&escape.formula) {
            let coloring = super::fields::get_field_coloring(&escape.coloring, field);
            (
                format!("field|{}|{}", field.name, coloring.name),
                Some(assembler::assemble_field(field, coloring)),
            )
        } else {
            (String::new(), None)
        };
        if let Some(source) = source_for {
            if !self.pipelines.contains_key(&key) {
                let module = device.create_shader_module(ShaderModuleDescriptor {
                    label: Some(&format!("Escape Shader {key}")),
                    source: ShaderSource::Wgsl(source.into()),
                });
                let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some("Escape Pipeline Layout"),
                    bind_group_layouts: &[Some(&self.bind_group_layout)],
                    immediate_size: 0,
                });
                let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                    label: Some(&format!("Escape Pipeline {key}")),
                    layout: Some(&layout),
                    module: &module,
                    entry_point: Some("escape_main"),
                    compilation_options: Default::default(),
                    cache: None,
                });
                self.pipelines.insert(key.clone(), pipeline);
            }
            return key;
        }
        let formula = super::get_formula(&escape.formula);
        let coloring = super::get_coloring(&escape.coloring);
        let damped = escape.is_damped();
        #[cfg(test)]
        let interior = !self.disable_interior;
        #[cfg(not(test))]
        let interior = true;
        let key = format!("{}|{}|{}|{}", formula.name, coloring.name, damped, interior);
        if !self.pipelines.contains_key(&key) {
            let source = assembler::assemble_with(formula, coloring, damped, interior);
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some(&format!("Escape Shader {key}")),
                source: ShaderSource::Wgsl(source.into()),
            });
            let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Escape Pipeline Layout"),
                bind_group_layouts: &[Some(&self.bind_group_layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(&format!("Escape Pipeline {key}")),
                layout: Some(&layout),
                module: &module,
                entry_point: Some("escape_main"),
                compilation_options: Default::default(),
                cache: None,
            });
            self.pipelines.insert(key.clone(), pipeline);
        }
        key
    }

    fn params_for(&self, escape: &EscapeConfig) -> EscapeParamsGpu {
        // zoom_log2 = 0 is the home view: vertical span 4 complex
        // units (the EscapeConfig doc contract); width follows aspect.
        let span_y = 4.0 / escape.zoom_factor();
        let span_x = span_y * (self.width as f64 / self.height.max(1) as f64);
        let (cx, cy) = escape.center_f64();

        let mut fparams = [[0.0f32; 4]; PARAM_VEC4S];
        let mut cparams = [[0.0f32; 4]; PARAM_VEC4S];
        let mut fdata = [[0.0f32; 4]; FDATA_VEC4S];
        if let Some(field) = super::fields::get_field(&escape.formula) {
            // Mode B: pack the field's params + its resolved coloring's.
            let coloring = super::fields::get_field_coloring(&escape.coloring, field);
            super::pack_params(field.parameters, &escape.formula_params, fparams.as_flattened_mut());
            super::pack_params(coloring.parameters, &escape.coloring_params, cparams.as_flattened_mut());
        } else {
            let formula = super::get_formula(&escape.formula);
            let coloring = super::get_coloring(&escape.coloring);
            super::pack_params(formula.parameters, &escape.formula_params, fparams.as_flattened_mut());
            super::pack_params(coloring.parameters, &escape.coloring_params, cparams.as_flattened_mut());
            if let Some(derive) = formula.derived_data {
                let flat = fdata.as_flattened_mut();
                for (slot, v) in flat.iter_mut().zip(derive(fparams.as_flattened())) {
                    *slot = v;
                }
            }
        }

        EscapeParamsGpu {
            center: [cx as f32, cy as f32],
            julia_c: [escape.julia_re, escape.julia_im],
            span: [span_x as f32, span_y as f32],
            rot_cs: [escape.rotation.cos(), escape.rotation.sin()],
            width: self.width,
            height: self.height,
            max_iter: escape.max_iter.max(1),
            flags: {
                // bit 0 = Julia; bits 1-2 = biomorph classification axis.
                let bio = match escape.biomorph {
                    crate::config::escape::BiomorphMode::Off => 0u32,
                    crate::config::escape::BiomorphMode::Re => 1,
                    crate::config::escape::BiomorphMode::Im => 2,
                };
                (if escape.julia { 1 } else { 0 }) | (bio << 1)
            },
            bailout: escape.bailout.max(1e-6),
            tile_y0: 0,
            damping: [escape.damping_re, escape.damping_im],
            shade_flags: escape.shading.field.to_gpu(),
            _pad_shade: [0; 3],
            fparams,
            cparams,
            fdata,
        }
    }

    /// One full-image escape pass into the output texture.
    ///
    /// `palette_view` is the flame renderer's palette texture (already
    /// carrying rotation/squeeze from `update_palette`), so escape mode
    /// inherits the whole palette pipeline for free.
    /// Returns whether the rendered image is FINAL (false only in
    /// progressive mode while the reference orbit is still growing —
    /// the caller should keep the escape image marked dirty).
    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        escape: &EscapeConfig,
        palette_view: &TextureView,
    ) -> bool {
        // Relief needs its scalar field and a destination distinct
        // from the colour it reads; both are allocated on demand, so
        // an escape view with shading off carries neither.
        self.ensure_height(device, escape.shading.enabled);
        self.ensure_resolve_target(device, escape.shading.enabled);
        let mut params = self.params_for(escape);
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Deep zoom: the perturbation path. Falls back to direct on a
        // center-parse failure (matching center_f64's fallback view).
        #[cfg(test)]
        let use_perturbed = Self::wants_perturbation(escape) || self.force_perturbed;
        #[cfg(not(test))]
        let use_perturbed = Self::wants_perturbation(escape);
        if use_perturbed {
            // Not a banded direct render: close its loss-attribution
            // window (a stale one would blame an unrelated loss).
            DIRECT_RENDER_IN_FLIGHT.store(false, std::sync::atomic::Ordering::Relaxed);
            #[cfg(not(target_arch = "wasm32"))]
            let progressive = self.progressive;
            #[cfg(target_arch = "wasm32")]
            let progressive = false;
            let orbit_state: Option<(u32, bool)> = if progressive {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let (len, done) = self.ensure_orbit_progressive(device, queue, escape);
                    if len == 0 {
                        // Nothing to render yet: keep the previous
                        // frame's texture, come back next frame.
                        return false;
                    }
                    // Rendering against a small fraction of a long
                    // reference is not progressive refinement, it is
                    // noise: every pixel wraps almost immediately and
                    // the frame is flat colour that changes wholesale
                    // as the prefix grows. Where the reference is
                    // quick that flicker is invisible and the early
                    // frames are useful; where it takes minutes, it
                    // is all the user sees. So: predict the build
                    // cost, and if it is more than a moment, hold the
                    // last good frame and report progress instead.
                    let want = escape.max_iter;
                    let limbs = super::fixedpoint::limbs_for_view(
                        &escape.center_re,
                        &escape.center_im,
                        escape.zoom_log2,
                    );
                    let slow = super::reference::predicted_orbit_seconds(want, limbs)
                        > Self::ORBIT_WAIT_SECONDS;
                    if !done && slow {
                        super::reference::set_orbit_progress(len, want);
                        return false;
                    }
                    super::reference::set_orbit_progress(0, 0);
                    Some((len, done))
                }
                #[cfg(target_arch = "wasm32")]
                None
            } else {
                // WASM has no worker thread: slice the fixed-point
                // compute per frame (budget shrinks with limb count so
                // a slice stays in tens of milliseconds) and let the
                // partial orbit render progressively via rebasing.
                // Desktop non-progressive (CLI, tests) stays blocking
                // and deterministic.
                #[cfg(target_arch = "wasm32")]
                {
                    let limbs =
                        super::fixedpoint::limbs_for_zoom(escape.zoom_log2).max(1) as u32;
                    let budget = (1_000_000 / limbs).clamp(256, 50_000);
                    match self.ensure_orbit_with(device, queue, escape, Some(budget)) {
                        Some((len, done)) if len >= 2 => Some((len, done)),
                        Some(_) => return false,
                        None => None,
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.ensure_orbit(device, queue, escape).map(|l| (l, true))
                }
            };
            if let Some((orbit_len, orbit_done)) = orbit_state {
                #[cfg(test)]
                let mut floatexp = escape.zoom_log2 > PERTURB_FLOATEXP_ZOOM || self.force_floatexp;
                #[cfg(not(test))]
                let mut floatexp = escape.zoom_log2 > PERTURB_FLOATEXP_ZOOM;
                // Manowar perturbs on the DEEP rung at every depth.
                // Its history term carries the delta forward with
                // coefficient 1, so where a one-term map's delta
                // decays near the reference, Manowar's persists and
                // f32 mantissa error accumulates across hundreds of
                // iterations. Measured against an exact orbit: 18.4%
                // of pixels wrong at zoom 20 and 27.0% at zoom 26 on
                // the scaled rung, against 1.6% and 2.1% on this one.
                if matches!(
                    Self::perturb_tier(escape),
                    Some(assembler::PerturbTier::Manowar)
                ) {
                    floatexp = true;
                }
                // Pixel spacing S = 2^(2 - zoom) / height. The scaled
                // rung takes it as f64->f32 (normal down to ~zoom 119,
                // past its own ceiling); the floatexp rung takes it
                // SYMBOLICALLY as mantissa * 2^exponent so no float
                // ever underflows however deep the zoom goes.
                let h = self.height.max(1) as f64;
                let s_f64 = if escape.zoom_log2 < 1000.0 {
                    4.0 / escape.zoom_factor() / h
                } else {
                    0.0
                };
                let x = 2.0 - escape.zoom_log2 - h.log2();
                let s_e = x.floor();
                let s_m = 2f64.powf(x - s_e);
                // Chunk window: restart on any render-state change,
                // else continue where the last dispatch stopped.
                let key = self.chunk_key_for(escape, self.orbit_generation, orbit_done);
                if self.chunk_key.as_deref() != Some(key.as_str()) {
                    self.chunk_key = Some(key);
                    self.chunk_next = 0;
                    self.reset_chunk_pacing();
                }
                // Consume any landed GPU measurement first: next_chunk
                // sizes from it.
                self.ts_poll();
                let measure_gpu = self.ts_prepare(device, queue);
                let chunk = self.next_chunk(floatexp);
                let iter_start = self.chunk_next.min(escape.max_iter);
                let iter_end = iter_start.saturating_add(chunk).min(escape.max_iter);
                let tier = Self::perturb_tier(escape)
                    .unwrap_or(assembler::PerturbTier::Power(2));
                self.ensure_iter_state(
                    device,
                    assembler::iter_state_bytes(tier, floatexp),
                );
                // BLA table: build/refresh when skipping applies,
                // else bind the zeroed dummy (n_levels = 0).
                let bla_ready = self.ensure_bla(
                    device, queue, escape, orbit_len, tier, progressive, orbit_done,
                );
                if self.bla_dummy.is_none() {
                    self.bla_dummy = Some(device.create_buffer(&BufferDescriptor {
                        label: Some("Escape BLA Dummy"),
                        size: 176,
                        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }));
                }
                // The reference's own c: the Julia constant on the
                // dynamical plane, the reference CENTRE on the
                // parameter plane. Only the multiplying-parameter
                // tiers read it, and only as a factor, so f32 of the
                // exact decimal centre is enough.
                let ref_c = if escape.julia {
                    [escape.julia_re, escape.julia_im]
                } else {
                    let (cx, cy) = escape.center_f64();
                    [cx as f32, cy as f32]
                };
                let pp = PerturbParamsGpu {
                    s: s_f64 as f32,
                    inv_s: if s_f64 > 0.0 { (1.0 / s_f64) as f32 } else { 0.0 },
                    orbit_len: orbit_len.max(2),
                    flags: if escape.julia { 2 } else { 0 },
                    s_m: s_m as f32,
                    s_e: s_e as i32,
                    ref_offset: self.current_ref_offset,
                    iter_start,
                    iter_end,
                    ref_c,
                };
                queue.write_buffer(&self.perturb_params_buffer, 0, bytemuck::bytes_of(&pp));

                let key = self.ensure_perturbed_pipeline(device, escape, floatexp);
                let bind_group = device.create_bind_group(&BindGroupDescriptor {
                    label: Some("Escape Perturbed Bind Group"),
                    layout: &self.perturb_bind_group_layout,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: self.params_buffer.as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::TextureView(&self.output_view),
                        },
                        BindGroupEntry {
                            binding: 2,
                            resource: BindingResource::TextureView(palette_view),
                        },
                        BindGroupEntry {
                            binding: 3,
                            resource: BindingResource::Sampler(&self.palette_sampler),
                        },
                        BindGroupEntry {
                            binding: 4,
                            resource: self.orbit_buffer.as_ref().unwrap().as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 5,
                            resource: self.perturb_params_buffer.as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 6,
                            resource: self
                                .iter_state_buffer
                                .as_ref()
                                .unwrap()
                                .as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 7,
                            resource: if bla_ready {
                                self.bla_buffer.as_ref().unwrap().as_entire_binding()
                            } else {
                                self.bla_dummy.as_ref().unwrap().as_entire_binding()
                            },
                        },
                        BindGroupEntry {
                            binding: 8,
                            resource: self
                                .orbit_lo_buffer
                                .as_ref()
                                .unwrap()
                                .as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 9,
                            resource: self
                                .orbit_e_buffer
                                .as_ref()
                                .unwrap()
                                .as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 10,
                            resource: BindingResource::TextureView(&self.height_view),
                        },
                        BindGroupEntry {
                            binding: 11,
                            resource: self
                                .orbit_r2_buffer
                                .as_ref()
                                .unwrap()
                                .as_entire_binding(),
                        },
                    ],
                });
                let ts_qs = if measure_gpu {
                    self.timestamps.as_ref().map(|t| &t.query_set)
                } else {
                    None
                };
                let pipeline = &self.pipelines[&key];
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Escape Perturbed Pass"),
                    timestamp_writes: ts_qs.map(|qs| wgpu::ComputePassTimestampWrites {
                        query_set: qs,
                        beginning_of_pass_write_index: Some(0),
                        end_of_pass_write_index: Some(1),
                    }),
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
                drop(pass);
                if measure_gpu {
                    self.ts_after_dispatch(encoder, iter_end.saturating_sub(iter_start));
                }
                // Every chunk refreshes the display image, so
                // progressive refinement stays visible under AA.
                self.run_resolve(device, queue, encoder, &escape.shading);
                let iterations_done = iter_end >= escape.max_iter;
                // Attribution window for the device-lost callback: open
                // while this render still has chunks to submit.
                PERTURB_RENDER_IN_FLIGHT.store(
                    !(orbit_done && iterations_done),
                    std::sync::atomic::Ordering::Relaxed,
                );
                self.chunk_next = if iterations_done { 0 } else { iter_end };
                // Progress for the panel: cleared the moment the last
                // chunk lands, so "settled" needs no separate signal.
                RENDER_WANT.store(
                    if orbit_done && iterations_done { 0 } else { escape.max_iter },
                    std::sync::atomic::Ordering::Relaxed,
                );
                RENDER_DONE.store(iter_end, std::sync::atomic::Ordering::Relaxed);
                if iterations_done {
                    log::debug!(
                        "escape: {} iterations in {} chunks (final chunk {}, baseline {:.1} ms)",
                        escape.max_iter,
                        self.chunk_count + 1,
                        self.chunk_iters,
                        self.chunk_base_ms,
                    );
                    // A repeat of the same render starts fresh.
                    self.chunk_key = None;
                    self.reset_chunk_pacing();
                }
                return orbit_done && iterations_done;
            }
            log::warn!("Deep zoom requested but the center failed to parse; rendering direct");
        }

        // Not a perturbed render: close its loss-attribution window
        // (a stale one would blame an unrelated loss).
        PERTURB_RENDER_IN_FLIGHT.store(false, std::sync::atomic::Ordering::Relaxed);

        // Row-band chunking for the unchunkable-by-iteration templates
        // (direct and field). A band is a complete render of its own
        // rows, so no resume state is needed and the output texture
        // accumulates the frame top to bottom.
        let key = self.chunk_key_for(escape, 0, true);
        if self.chunk_key.as_deref() != Some(key.as_str()) {
            self.chunk_key = Some(key);
            self.direct_tile_y = 0;
            self.direct_last = None;
        }
        // Shrink-only pacing: while continuing a banded render, the
        // gap since the previous band approximates that band's GPU
        // time (the dirty loop redraws as fast as the GPU drains). A
        // band that survived but ran long still halves the session
        // budget; growth is never attempted (measured losing the
        // device -- see DIRECT_BUDGET_SHIFT).
        if self.direct_tile_y > 0 {
            if let Some(t0) = self.direct_last {
                if t0.elapsed().as_millis() > DIRECT_BAND_SLOW_MS {
                    use std::sync::atomic::Ordering;
                    let sh = DIRECT_BUDGET_SHIFT.load(Ordering::Relaxed);
                    if sh < 6 {
                        DIRECT_BUDGET_SHIFT.store(sh + 1, Ordering::Relaxed);
                    }
                }
            }
        }
        let rows = self.direct_rows_per_dispatch(escape);
        if self.direct_tile_y >= self.height {
            self.direct_tile_y = 0;
        }
        let tile_y0 = self.direct_tile_y;
        let band = rows.min(self.height - tile_y0);
        params.tile_y0 = tile_y0;
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Built per pass: the palette view can be recreated under us
        // (palette-size changes), and one bind group per render is
        // noise next to the dispatch itself.
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Escape Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&self.output_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(palette_view),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::Sampler(&self.palette_sampler),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::TextureView(&self.height_view),
                },
            ],
        });

        let key = self.ensure_pipeline(device, escape);
        let pipeline = &self.pipelines[&key];

        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Escape Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(self.width.div_ceil(8), band.div_ceil(8), 1);
        drop(pass);
        self.run_resolve(device, queue, encoder, &escape.shading);
        self.direct_tile_y = tile_y0.saturating_add(band);
        let done = self.direct_tile_y >= self.height;
        DIRECT_RENDER_IN_FLIGHT.store(
            !done && band < self.height,
            std::sync::atomic::Ordering::Relaxed,
        );
        self.direct_last = Some(web_time::Instant::now());
        if done {
            // A repeat of the same render starts from the top.
            self.chunk_key = None;
            self.direct_tile_y = 0;
            self.direct_last = None;
        }
        done
    }

    /// Free GPU memory explicitly — on WebGPU `Drop` frees nothing.
    /// Idempotent; safe once no submitted work references the texture.
    pub fn destroy(&self) {
        self.output_texture.destroy();
        self.params_buffer.destroy();
        self.perturb_params_buffer.destroy();
        if let Some(b) = &self.orbit_buffer {
            b.destroy();
        }
        if let Some(b) = &self.orbit_e_buffer {
            b.destroy();
        }
        if let Some(b) = &self.orbit_r2_buffer {
            b.destroy();
        }
        if let Some(b) = &self.orbit_lo_buffer {
            b.destroy();
        }
        if let Some(b) = &self.iter_state_buffer {
            b.destroy();
        }
        if let Some(b) = &self.bla_buffer {
            b.destroy();
        }
        if let Some(b) = &self.bla_dummy {
            b.destroy();
        }
        if let Some(t) = &self.final_texture {
            t.destroy();
        }
        if let Some(ts) = &self.timestamps {
            ts.resolve.destroy();
            ts.staging.destroy();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The budget shifts and in-flight flags are process-global, so
    /// the tests that drive them must not run concurrently.
    static BREAKER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn a_direct_render_is_split_into_bands_it_can_survive() {
        // Zooming a 10M-iteration config OUT past the perturbation
        // threshold used to hand the direct path a single dispatch of
        // pixels x max_iter with nothing bounding it: tens of seconds
        // in one submission, which Windows answers by resetting the
        // driver and wgpu by aborting the process.
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.zoom_log2 = 10.0;
        esc.max_iter = 10_100_100;
        assert!(!EscapeRenderer::wants_perturbation(&esc), "shallow stays direct");

        // A band's work must respect the budget, and a render must
        // still be reachable in a finite number of them.
        let (w, h) = (1280u32, 768u32);
        let rows = rows_for(w, h, esc.max_iter);
        assert!(rows >= 1, "a band must cover at least one row");
        let work = (w as u64) * (rows as u64) * (esc.max_iter as u64);
        assert!(
            work <= DIRECT_DISPATCH_BUDGET,
            "a band is {work} pixel-iterations, over the {DIRECT_DISPATCH_BUDGET} budget"
        );
        assert!(rows < h, "10M iterations over this viewport must take several bands");

        // Ordinary iteration counts still render in one pass, so
        // nothing about the common case changes.
        esc.max_iter = 2_000;
        assert_eq!(rows_for(w, h, esc.max_iter), h);
    }

    /// The band size without needing a GPU: mirrors
    /// EscapeRenderer::direct_rows_per_dispatch.
    fn rows_for(width: u32, height: u32, max_iter: u32) -> u32 {
        let per_row = (width as u64).saturating_mul(max_iter.max(1) as u64);
        let rows = DIRECT_DISPATCH_BUDGET / per_row.max(1);
        (rows.max(1) as u32).min(height.max(1))
    }


    #[test]
    fn device_loss_halves_the_perturbed_budget_only_when_attributable() {
        use std::sync::atomic::Ordering;
        let _guard = BREAKER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        PERTURB_BUDGET_SHIFT.store(0, Ordering::Relaxed);
        PERTURB_RENDER_IN_FLIGHT.store(false, Ordering::Relaxed);
        DIRECT_RENDER_IN_FLIGHT.store(false, Ordering::Relaxed);
        let seed0 = perturb_chunk_seed(false, 1920 * 1080);
        let ceil0 = perturb_chunk_ceiling();

        note_device_lost();
        assert_eq!(
            PERTURB_BUDGET_SHIFT.load(Ordering::Relaxed),
            0,
            "a loss with no perturbed render in flight must not shrink"
        );

        PERTURB_RENDER_IN_FLIGHT.store(true, Ordering::Relaxed);
        note_device_lost();
        assert_eq!(PERTURB_BUDGET_SHIFT.load(Ordering::Relaxed), 1);
        assert!(
            !PERTURB_RENDER_IN_FLIGHT.load(Ordering::Relaxed),
            "attribution consumes the in-flight window"
        );
        // Both the seed and the ceiling must actually shrink -- the
        // ceiling is what a runaway growth loop would otherwise reach.
        assert_eq!(perturb_chunk_seed(false, 1920 * 1080), seed0 / 2);
        assert_eq!(perturb_chunk_ceiling(), ceil0 / 2);

        for _ in 0..10 {
            PERTURB_RENDER_IN_FLIGHT.store(true, Ordering::Relaxed);
            note_device_lost();
        }
        assert_eq!(
            PERTURB_BUDGET_SHIFT.load(Ordering::Relaxed),
            6,
            "clamped: even the floor is a usable budget"
        );
        assert!(perturb_chunk_seed(false, 1920 * 1080) >= 16, "floor holds");
        PERTURB_BUDGET_SHIFT.store(0, Ordering::Relaxed);
    }

    #[test]
    fn device_loss_halves_the_direct_budget_only_when_attributable() {
        use std::sync::atomic::Ordering;
        let _guard = BREAKER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        DIRECT_BUDGET_SHIFT.store(0, Ordering::Relaxed);
        DIRECT_RENDER_IN_FLIGHT.store(false, Ordering::Relaxed);
        PERTURB_RENDER_IN_FLIGHT.store(false, Ordering::Relaxed);
        note_device_lost();
        assert_eq!(
            DIRECT_BUDGET_SHIFT.load(Ordering::Relaxed),
            0,
            "a loss with no banded render in flight must not shrink"
        );
        DIRECT_RENDER_IN_FLIGHT.store(true, Ordering::Relaxed);
        note_device_lost();
        assert_eq!(DIRECT_BUDGET_SHIFT.load(Ordering::Relaxed), 1);
        assert!(
            !DIRECT_RENDER_IN_FLIGHT.load(Ordering::Relaxed),
            "attribution consumes the in-flight window"
        );
        for _ in 0..10 {
            DIRECT_RENDER_IN_FLIGHT.store(true, Ordering::Relaxed);
            note_device_lost();
        }
        assert_eq!(
            DIRECT_BUDGET_SHIFT.load(Ordering::Relaxed),
            6,
            "clamped: even the floor is a usable budget"
        );
        DIRECT_BUDGET_SHIFT.store(0, Ordering::Relaxed);
        DIRECT_RENDER_IN_FLIGHT.store(false, Ordering::Relaxed);
    }

    /// The persisted tuning file: round-trip, and refuse to let a
    /// broken one do damage. Persistence is the whole point of the
    /// breaker -- without it every session re-learns by losing the
    /// device -- but a file that says "shift 40" must not be believed.
    #[test]
    fn tuning_file_round_trips_and_clamps_hostile_input() {
        assert_eq!(tuning::decode(&tuning::encode(0, 0)), (0, 0));
        assert_eq!(tuning::decode(&tuning::encode(2, 5)), (2, 5));
        assert_eq!(tuning::decode(&tuning::encode(6, 6)), (6, 6));
        // Out of range, wrong types, missing keys, and outright
        // garbage all read as "no tuning learned" or a clamp.
        assert_eq!(tuning::decode(r#"{"direct_shift":40,"perturb_shift":99}"#), (6, 6));
        assert_eq!(tuning::decode(r#"{"direct_shift":"lots"}"#), (0, 0));
        assert_eq!(tuning::decode("{}"), (0, 0));
        assert_eq!(tuning::decode("not json at all"), (0, 0));
        assert_eq!(tuning::decode(""), (0, 0));
    }

    /// The reference orbit's parameter must be the SAME VALUE the
    /// shader's uniform gets.
    ///
    /// They are resolved by different code (`map_params_for` here,
    /// `pack_params` for the uniform), and they disagreed: an absent
    /// key meant "the registry default" to one and "zero" to the
    /// other. A fresh Phoenix config therefore iterated deltas for
    /// p = -0.5 against a reference built for p = 0 -- the plain
    /// quadratic -- so the perturbed render was a different fractal
    /// entirely, while an EDITED config carried the keys and worked.
    /// That is why the agreement test missed it: it set the
    /// parameters explicitly.
    #[test]
    fn reference_parameters_match_the_shader_uniform() {
        // Which of a formula's own parameters ride `map_params` into
        // the reference orbit's identity. EVERY formula that can
        // perturb needs an entry, and the assertion below enforces
        // that — a new tier whose parameters change the MAP but never
        // reach the reference is exactly the bug this table exists to
        // stop, and it has now happened twice (Phoenix's p, then
        // McMullen's pole power m, which built the reference for
        // c/z^1 while the delta step used c/z^3; every pixel escaped).
        let carried: &[(&str, &[&str])] = &[
            ("mandelbrot", &[]),
            ("multibrot", &[]),
            ("tricorn", &[]),
            ("burning_ship", &[]),
            ("lambda", &[]),
            ("feather", &[]),
            ("phoenix", &["p_re", "p_im"]),
            // Manowar's p = 1 is fixed by the formula, not a parameter.
            ("manowar", &[]),
            ("mcmullen", &["m"]),
            // The VARIANT selects between two different maps.
            ("magnet", &["variant"]),
        ];

        for (formula, names) in carried {
            for edited in [false, true] {
                let mut esc = crate::config::escape::EscapeConfig::default();
                esc.formula = (*formula).to_string();
                // McMullen only perturbs on the dynamical plane.
                esc.julia = *formula == "mcmullen";
                if edited {
                    // Push every parameter off its default, so a
                    // resolver that silently returns zero (or the
                    // default) is caught.
                    for (i, d) in super::super::get_formula(formula).parameters.iter().enumerate() {
                        let v = if d.name == "m" || d.name == "n" {
                            (d.default + 1.0).min(d.max)
                        } else {
                            d.default + 0.125 * (i as f32 + 1.0)
                        };
                        esc.formula_params.insert(d.name.to_string(), v);
                    }
                }
                let mine = EscapeRenderer::map_params_for(&esc);
                let mut packed = [0.0f32; 16];
                super::super::pack_params(
                    super::super::get_formula(formula).parameters,
                    &esc.formula_params,
                    &mut packed,
                );
                for (slot, name) in names.iter().enumerate() {
                    let idx = super::super::get_formula(formula)
                        .parameters
                        .iter()
                        .position(|d| d.name == *name)
                        .expect("named parameter exists");
                    assert_eq!(
                        mine[slot], packed[idx],
                        "{formula} (edited={edited}): the reference resolved \
                         {name} as {} but the shader got {} -- they describe \
                         different maps",
                        mine[slot], packed[idx]
                    );
                }
                for slot in names.len()..2 {
                    // Manowar's fixed p = 1 is the one legitimate
                    // nonzero that is not a config parameter.
                    if *formula == "manowar" {
                        continue;
                    }
                    assert_eq!(
                        mine[slot], 0.0,
                        "{formula}: map_params[{slot}] carries {} but the table \
                         declares nothing there",
                        mine[slot]
                    );
                }
            }
        }

        // The table must cover every formula that can actually
        // perturb. This is the half that catches the NEXT tier.
        for f in crate::escape::FORMULAS {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.formula = f.name.to_string();
            let plain = EscapeRenderer::perturb_tier(&esc).is_some();
            esc.julia = true;
            let julia = EscapeRenderer::perturb_tier(&esc).is_some();
            if plain || julia {
                assert!(
                    carried.iter().any(|(n, _)| *n == f.name),
                    "{} perturbs but is missing from the map_params table -- \
                     declare which of its parameters reach the reference orbit \
                     (an empty list is a valid answer, and saying so is the point)",
                    f.name
                );
            }
        }
    }

    #[test]
    fn perturbation_gate_is_tight() {
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.zoom_log2 = 30.0;
        assert!(EscapeRenderer::wants_perturbation(&esc));
        esc.zoom_log2 = 10.0;
        assert!(!EscapeRenderer::wants_perturbation(&esc), "shallow stays direct");
        esc.zoom_log2 = 30.0;
        esc.julia = true;
        assert!(EscapeRenderer::wants_perturbation(&esc), "julia is in the trivial tier");
        esc.julia = false;
        esc.formula = "burning_ship".to_string();
        assert!(
            EscapeRenderer::wants_perturbation(&esc),
            "the Ship family is in the diffabs tier"
        );
        esc.zoom_log2 = 60.0;
        assert!(
            EscapeRenderer::wants_perturbation(&esc),
            "deep Ship rides the floatexp diffabs rung"
        );
        esc.zoom_log2 = 30.0;
        esc.formula_params.insert("variant".to_string(), 3.0);
        assert!(
            EscapeRenderer::wants_perturbation(&esc),
            "every fold variant has its own delta algebra now"
        );
        esc.formula_params.clear();
        esc.formula = "weierstrass".to_string();
        assert!(
            !EscapeRenderer::wants_perturbation(&esc),
            "mode B fields never perturb"
        );
        esc.formula = "burning_ship".to_string();
        esc.formula = "multibrot".to_string();
        assert!(EscapeRenderer::wants_perturbation(&esc), "integer multibrot is in the tier");
        esc.formula_params.insert("power".to_string(), 3.5);
        assert!(
            !EscapeRenderer::wants_perturbation(&esc),
            "non-integer power stays direct (binomial needs an integer exponent)"
        );
        esc.formula_params.clear();
        esc.formula = "mandelbrot".to_string();
        esc.damping_re = 0.5;
        assert!(!EscapeRenderer::wants_perturbation(&esc), "damped not in v1");
    }

    #[test]
    fn params_struct_matches_wgsl_layout() {
        // 4 vec2 (32) + 4 u32 (16) + f32 + 3 pad (16) + the shading
        // flags + 3 pad (16) + 2 param arrays (128) + the derived-data
        // table (1024) = 1232, and the arrays must start 16-byte
        // aligned.
        assert_eq!(std::mem::size_of::<EscapeParamsGpu>(), 1232);
        assert_eq!(std::mem::offset_of!(EscapeParamsGpu, shade_flags), 64);
        assert_eq!(std::mem::offset_of!(EscapeParamsGpu, fparams), 80);
        assert_eq!(std::mem::offset_of!(EscapeParamsGpu, cparams), 144);
        assert_eq!(std::mem::offset_of!(EscapeParamsGpu, fdata), 208);
    }
}

//! Interactive-latency diagnostics for the escape renderer.
//!
//! One global snapshot, updated by [`super::renderer::EscapeRenderer`]
//! and the orbit worker as they run, read by the Escape panel (and by
//! tests). The point is attribution: when a change past the
//! perturbation threshold feels slower than the same change on the
//! direct path, this says WHICH stage the time went to — the
//! reference orbit (and whether it was recomputed or reused), the BLA
//! table, the GPU-mirror upload, or simply the number of chunked
//! frames the render needed to settle.
//!
//! A `Mutex` rather than atomics because the panel wants a coherent
//! snapshot (a settle time next to the frame count it belongs to),
//! writes are a handful per frame, and every reader is on the UI
//! thread.

use once_cell::sync::Lazy;
use std::sync::Mutex;

/// Where the current reference orbit came from — the difference
/// between "cached" and "recomputed" is the first question the panel
/// answers.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum OrbitSource {
    /// No perturbed render yet (or direct path).
    #[default]
    None,
    /// The worker (or blocking cache) kept the previous orbit: same
    /// center, same shape — only the view moved. Costs nothing.
    Reused,
    /// Loaded complete from the on-disk orbit store.
    Store,
    /// Computed fresh in fixed point (the expensive case).
    Computed,
}

impl OrbitSource {
    pub fn label(self) -> &'static str {
        match self {
            OrbitSource::None => "-",
            OrbitSource::Reused => "reused",
            OrbitSource::Store => "disk store",
            OrbitSource::Computed => "recomputed",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct EscapeDiag {
    /// Which pipeline the last frame ran: "direct", "perturbed f32",
    /// "perturbed floatexp".
    pub path: &'static str,

    // ---- reference orbit ------------------------------------------
    pub orbit_len: u32,
    pub orbit_source: OrbitSource,
    /// CPU milliseconds the CURRENT orbit's fixed-point compute has
    /// taken so far (worker: cumulative across its chunks; blocking
    /// path: the whole `get`). Zero for a reused orbit.
    pub orbit_ms: f32,
    /// Frames the renderer returned empty-handed because the worker
    /// had not yet published anything for the current request.
    pub orbit_wait_frames: u32,
    /// Times the orbit CONTENT was replaced (a fresh compute or a
    /// store load — not an extend, not a reuse) since the app
    /// started. Climbing during a pan/zoom gesture means each step
    /// threw the reference away.
    pub orbit_rebuilds: u64,
    /// Times a moved view was served by re-anchoring the existing
    /// reference (relocate_to) instead of recomputing it — the pan
    /// fast path. During a drag this should climb while
    /// `orbit_rebuilds` stays put.
    pub orbit_relocations: u64,

    // ---- BLA table ------------------------------------------------
    pub bla_active: bool,
    /// Size of the last-built table's GPU bytes.
    pub bla_bytes: u64,
    pub bla_build_ms: f32,

    // ---- GPU mirror -----------------------------------------------
    /// Bytes written to the orbit buffers for the current orbit
    /// generation (all four channels).
    pub upload_bytes: u64,

    // ---- chunked settle -------------------------------------------
    /// Chunk-key changes since start — each one is a render restarted
    /// from iteration 0 (or row 0 on the direct path).
    pub restarts: u64,
    /// The last COMPLETED render: how many frames of chunks it took
    /// and the wall time from restart to settled.
    pub settle_frames: u32,
    pub settle_ms: f32,
    /// The render in progress, if any (frames so far).
    pub inflight_frames: u32,
    pub last_chunk_iters: u32,
    /// CPU time spent inside the last `render()` call itself —
    /// encoder/bind-group work, orbit mirroring, BLA building; the
    /// GPU dispatch is not included (it is asynchronous).
    pub render_cpu_ms: f32,
}

static DIAG: Lazy<Mutex<EscapeDiag>> = Lazy::new(|| Mutex::new(EscapeDiag::default()));

/// Coherent copy of the current snapshot.
pub fn snapshot() -> EscapeDiag {
    DIAG.lock().unwrap().clone()
}

/// Mutate the snapshot in place (writer side).
pub fn update(f: impl FnOnce(&mut EscapeDiag)) {
    let mut d = DIAG.lock().unwrap();
    f(&mut d);
}

/// Reset everything — a new session (tests; not called by the app).
pub fn reset() {
    *DIAG.lock().unwrap() = EscapeDiag::default();
}

/// Drop guard that records the CPU time of the enclosing scope into
/// `render_cpu_ms` — used so every early return of `render()` still
/// reports.
pub struct CpuTimer(web_time::Instant);

impl CpuTimer {
    pub fn start() -> Self {
        CpuTimer(web_time::Instant::now())
    }
}

impl Drop for CpuTimer {
    fn drop(&mut self) {
        let ms = self.0.elapsed().as_secs_f32() * 1000.0;
        update(|d| d.render_cpu_ms = ms);
    }
}

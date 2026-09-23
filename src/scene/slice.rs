//! **Doing long work a frame at a time.**
//!
//! `docs/projects/gpu-cylinder-planning.md` phase 3. On the web a
//! cylinder plan runs on the page's only thread, polled once a frame, so
//! the work under it -- the inverse walk (`backward.rs`), and the flame
//! analysis it starts from (`ifs_analysis.rs`) -- is written as futures
//! that tick between pieces, and a tick past the frame's budget yields.
//! The desktop drives the same futures straight through.

/// **Where a plan gives its thread back.** A web plan runs on the page's
/// own thread and is polled once a frame; the walk calls [`Slicer::tick`]
/// between pieces of work, and a tick past this poll's budget yields, to
/// be resumed on the next frame. [`Slicer::never`] never yields -- the
/// desktop's worker runs the walk straight through.
pub struct Slicer {
    budget: Option<std::time::Duration>,
    since: std::cell::Cell<Option<web_time::Instant>>,
    /// When tracing ([`Slicer::traced`]): the last tick, and every gap
    /// between ticks of one poll longer than `TRACE_GAP`, with where it
    /// ran from and to -- the units of work too big for a frame.
    last: std::cell::Cell<Option<(web_time::Instant, &'static std::panic::Location<'static>)>>,
    gaps: Option<std::cell::RefCell<Vec<String>>>,
    /// Time spent compiling GPU kernels on this thread: see
    /// [`Slicer::compiled`].
    compiled: std::cell::Cell<std::time::Duration>,
}

/// See [`Slicer::traced`].
const TRACE_GAP: std::time::Duration = std::time::Duration::from_millis(8);

impl Slicer {
    pub fn never() -> Self {
        Self {
            budget: None,
            since: std::cell::Cell::new(None),
            last: std::cell::Cell::new(None),
            gaps: None,
            compiled: std::cell::Cell::new(std::time::Duration::ZERO),
        }
    }

    pub fn every(budget: std::time::Duration) -> Self {
        Self { budget: Some(budget), ..Self::never() }
    }

    /// [`Slicer::every`], recording each gap between ticks longer than
    /// 8 ms and where it ran: see [`Slicer::gaps`].
    pub fn traced(budget: std::time::Duration) -> Self {
        Self { gaps: Some(std::cell::RefCell::new(Vec::new())), ..Self::every(budget) }
    }

    /// The long gaps recorded so far (a traced slicer's).
    pub fn gaps(&self) -> Vec<String> {
        self.gaps.as_ref().map_or_else(Vec::new, |g| g.borrow().clone())
    }

    /// How long this thread has spent compiling GPU kernels, natively.
    /// Building a kernel is one piece no tick can split. A native driver
    /// compiles it on the calling thread; a browser compiles it in its GPU
    /// process, so it is not in the plan's step there -- the web's path,
    /// run natively, spends time here the page's step would not, and its
    /// timings subtract it. (It is not free in the browser either: the GPU
    /// process compiles on the thread that serves the page's own
    /// rendering, and the page's next frame can wait for it --
    /// `gpu-cylinder-planning.md` §17.)
    pub fn compiled(&self) -> std::time::Duration {
        self.compiled.get()
    }

    /// Record a native kernel compile: see [`Slicer::compiled`].
    pub fn add_compiled(&self, t: std::time::Duration) {
        self.compiled.set(self.compiled.get() + t);
    }

    /// Whether this slicer ever yields: the work between ticks then runs
    /// in order on this thread, rather than across threads.
    pub fn slices(&self) -> bool {
        self.budget.is_some()
    }

    /// Called by the driver at the start of each poll.
    #[track_caller]
    pub fn begin(&self) {
        if self.budget.is_some() {
            let now = web_time::Instant::now();
            self.since.set(Some(now));
            if self.gaps.is_some() {
                self.last.set(Some((now, std::panic::Location::caller())));
            }
        }
    }

    /// A point where the walk may yield: pending once if this poll has
    /// run past its budget.
    #[track_caller]
    pub fn tick(&self) -> Tick<'_> {
        if let Some(gaps) = &self.gaps {
            let here = std::panic::Location::caller();
            let now = web_time::Instant::now();
            if let Some((t, from)) = self.last.get() {
                let gap = now.duration_since(t);
                if gap > TRACE_GAP {
                    gaps.borrow_mut().push(format!(
                        "{:.1} ms: {}:{} -> {}:{}",
                        gap.as_secs_f64() * 1e3,
                        from.file(),
                        from.line(),
                        here.file(),
                        here.line()
                    ));
                }
            }
            self.last.set(Some((now, here)));
        }
        Tick { slicer: self, yielded: false }
    }
}

/// See [`Slicer::tick`].
pub struct Tick<'s> {
    slicer: &'s Slicer,
    yielded: bool,
}

impl std::future::Future for Tick<'_> {
    type Output = ();
    fn poll(mut self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        if self.yielded {
            return std::task::Poll::Ready(());
        }
        match (self.slicer.budget, self.slicer.since.get()) {
            (Some(b), Some(t)) if t.elapsed() >= b => {
                // The driver polls again next frame; nothing to wake.
                self.yielded = true;
                std::task::Poll::Pending
            }
            _ => std::task::Poll::Ready(()),
        }
    }
}

/// Run `f` to the end on this thread. The desktop's way to run the walk,
/// whose evaluators answer at once: a pending poll can only be a tick,
/// and a tick with no budget never pends.
pub fn drive<F: std::future::Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

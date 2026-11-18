//! One background request (docs/archive/projects/catalog.md, *Phase 3*): a future
//! run off the frame -- a thread on desktop (the client is blocking
//! `ureq`), `spawn_local` on the web -- whose result the UI takes when it
//! is in, with a wake-up (a repaint) when it lands.
//!
//! Before this, every online section wrote its own thread-or-`spawn_local`,
//! slot and poll, and only some asked for a redraw or treated a 401 as an
//! expired sign-in. A job asked with the session's credentials
//! ([`Job::authed`]) that comes back 401 marks the session expired
//! ([`take_session_expired`]), whoever asked: the UI signs out once.
//!
//! ```ignore
//! let ctx = ui.ctx().clone();
//! self.job = Some(job::spawn(async move { api.browse_flames(scope, &words).await }, move || ctx.request_repaint()));
//! // each frame:
//! if let Some(result) = self.job.as_ref().and_then(Job::take) { ... }
//! ```

use crate::resources::{FetchError, FetchResult};
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// A 401 came back to a request that carried the session's credentials.
static SESSION_EXPIRED: AtomicBool = AtomicBool::new(false);

/// Whether a request since the last call found the session expired.
pub fn take_session_expired() -> bool {
    SESSION_EXPIRED.swap(false, Ordering::Relaxed)
}

/// A request in flight, or done and not yet taken.
pub struct Job<T> {
    slot: Arc<Mutex<Option<FetchResult<T>>>>,
    authed: bool,
}

impl<T> Job<T> {
    /// The request carried the session's credentials: a 401 means the
    /// session expired (a guest's 401 means only "sign in for this").
    pub fn authed(mut self, signed_in: bool) -> Self {
        self.authed = signed_in;
        self
    }

    /// The result, once: `None` while it is in flight (and after it was
    /// taken).
    pub fn take(&self) -> Option<FetchResult<T>> {
        let result = self.slot.lock().ok()?.take()?;
        if self.authed && matches!(result, Err(FetchError::Unauthorized)) {
            SESSION_EXPIRED.store(true, Ordering::Relaxed);
        }
        Some(result)
    }
}

/// Run `request` off the frame; `wake` runs once its result is in.
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn<T, F, W>(request: F, wake: W) -> Job<T>
where
    T: Send + 'static,
    F: Future<Output = FetchResult<T>> + Send + 'static,
    W: FnOnce() + Send + 'static,
{
    let slot = Arc::new(Mutex::new(None));
    let out = slot.clone();
    std::thread::spawn(move || {
        let result = pollster::block_on(request);
        if let Ok(mut s) = out.lock() {
            *s = Some(result);
        }
        wake();
    });
    Job { slot, authed: false }
}

/// Run `request` off the frame; `wake` runs once its result is in.
#[cfg(target_arch = "wasm32")]
pub fn spawn<T, F, W>(request: F, wake: W) -> Job<T>
where
    T: 'static,
    F: Future<Output = FetchResult<T>> + 'static,
    W: FnOnce() + 'static,
{
    let slot = Arc::new(Mutex::new(None));
    let out = slot.clone();
    #[cfg(browser)]
    wasm_bindgen_futures::spawn_local(async move {
        let result = request.await;
        if let Ok(mut s) = out.lock() {
            *s = Some(result);
        }
        wake();
    });
    #[cfg(not(browser))]
    {
        // No executor and no network (WASI): it fails at once.
        drop((request, wake));
        if let Ok(mut s) = out.lock() {
            *s = Some(Err(FetchError::Network("No network in this build".to_string())));
        }
    }
    Job { slot, authed: false }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn wait<T>(job: &Job<T>) -> FetchResult<T> {
        for _ in 0..500 {
            if let Some(r) = job.take() {
                return r;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("the job never finished");
    }

    /// The result arrives once, the wake-up runs, and only a request that
    /// carried the session's credentials reports the session expired.
    #[test]
    fn a_job_delivers_once_and_reports_an_expired_session() {
        let woke = Arc::new(AtomicBool::new(false));
        let w = woke.clone();
        let job = spawn(async { Ok(7) }, move || w.store(true, Ordering::Relaxed));
        assert_eq!(wait(&job).unwrap(), 7);
        assert!(job.take().is_none(), "taken once");
        assert!(woke.load(Ordering::Relaxed), "the wake-up ran");

        take_session_expired();
        let guest = spawn(async { Err::<(), _>(FetchError::Unauthorized) }, || {});
        assert!(wait(&guest).is_err());
        assert!(!take_session_expired(), "a guest's 401 is not an expired session");
        let signed_in = spawn(async { Err::<(), _>(FetchError::Unauthorized) }, || {}).authed(true);
        assert!(wait(&signed_in).is_err());
        assert!(take_session_expired(), "a signed-in 401 is");
    }
}

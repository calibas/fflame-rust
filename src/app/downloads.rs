//! Downloads by name in flight: the one shape the variation and effect
//! fetches share (`variation_fetch`, `effect_fetch`; docs/projects/
//! curated-content.md, step 6). Each name is an [`api::job`] -- a thread on
//! desktop, a `spawn_local` on the web, the result taken once.

use crate::api::job::Job;
use crate::resources::FetchResult;

/// The jobs of one kind's downloads.
pub(crate) struct Downloads<T> {
    jobs: Vec<(String, Job<T>)>,
    /// When the current batch started (a variation batch times out).
    pub started: Option<web_time::Instant>,
}

impl<T> Default for Downloads<T> {
    fn default() -> Self {
        Downloads { jobs: Vec::new(), started: None }
    }
}

impl<T> Downloads<T> {
    /// A new batch: stragglers from an earlier one are dropped with their
    /// jobs (never counted against it).
    pub fn new_batch(&mut self) {
        self.jobs.clear();
        self.started = Some(web_time::Instant::now());
    }

    /// Add one download (a job the caller spawned: `api::job::spawn`, whose
    /// bounds differ between desktop and the web).
    pub fn push(&mut self, name: String, job: Job<T>) {
        if self.jobs.is_empty() && self.started.is_none() {
            self.started = Some(web_time::Instant::now());
        }
        self.jobs.push((name, job));
    }

    /// The downloads that finished since the last call.
    pub fn take_done(&mut self) -> Vec<(String, FetchResult<T>)> {
        let mut done = Vec::new();
        self.jobs.retain(|(name, job)| match job.take() {
            Some(result) => {
                done.push((name.clone(), result));
                false
            }
            None => true,
        });
        if self.jobs.is_empty() {
            self.started = None;
        }
        done
    }

    /// Any still in flight.
    pub fn busy(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// The names still in flight.
    pub fn names(&self) -> Vec<String> {
        self.jobs.iter().map(|(n, _)| n.clone()).collect()
    }

    /// Give up on everything in flight (a timeout).
    pub fn abandon(&mut self) {
        self.jobs.clear();
        self.started = None;
    }
}

//! When is the API "unreachable"? Not after one failed fetch.
//!
//! A browser fetch fails transiently for reasons that have nothing to
//! do with the server being down: a pooled HTTP connection the server
//! closed at its idle timeout (which can sit exactly at our 30 s check
//! interval — the request races the close and loses), a laptop waking,
//! a slow endpoint that the browser gives up on. The symptom in the
//! field was "Connection to server lost" while the same API answered
//! fine from another tab, and `NetworkError when attempting to fetch
//! resource` in the console — Firefox's wording for a fetch that was
//! aborted or reset, not refused.
//!
//! Treating one of those as "lost" did real damage: Save Online went
//! grey, a notification fired, and nothing tried again for 30 seconds.
//! This module is the policy that replaces that, kept free of clocks
//! and side effects so it can be tested as a table of transitions:
//!
//! * a network error is counted, not believed — `Unreachable` needs
//!   [`FAILURES_BEFORE_UNREACHABLE`] in a row;
//! * the first failure pulls the next check forward (5 s, not 30 s) so
//!   a transient clears before anyone notices, and a real outage is
//!   confirmed quickly;
//! * while unreachable, checks run every 15 s so recovery is noticed
//!   twice as fast as the healthy cadence, without hammering a server
//!   that may be coming back;
//! * any HTTP response at all — 200, 401, 500 — means the server is
//!   there, and resets the count.

use std::time::Duration;

use super::{ApiConnectivity, HealthCheckOutcome};

/// Consecutive network errors before the app stops believing the
/// server is up. Two: one is noise, two in a row ten seconds apart is
/// a signal.
pub const FAILURES_BEFORE_UNREACHABLE: u32 = 2;

/// Healthy cadence.
pub const INTERVAL_ONLINE: Duration = Duration::from_secs(30);
/// After one failure: confirm or clear it quickly.
pub const INTERVAL_AFTER_FAILURE: Duration = Duration::from_secs(5);
/// While unreachable: look for recovery faster than the healthy
/// cadence, slower than the confirmation retry.
pub const INTERVAL_UNREACHABLE: Duration = Duration::from_secs(15);

/// What the UI should tell the user, if anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    None,
    /// Was online, now confirmed unreachable.
    Lost,
    /// Was unreachable, answered again.
    Restored,
}

#[derive(Debug, Clone)]
pub struct HealthPolicy {
    connectivity: ApiConnectivity,
    consecutive_failures: u32,
}

impl Default for HealthPolicy {
    fn default() -> Self {
        Self { connectivity: ApiConnectivity::Unknown, consecutive_failures: 0 }
    }
}

impl HealthPolicy {
    pub fn connectivity(&self) -> ApiConnectivity {
        self.connectivity
    }

    /// Record one health-check outcome; returns what changed.
    pub fn record(&mut self, outcome: &HealthCheckOutcome) -> Transition {
        let before = self.connectivity;

        match outcome {
            // The server answered. What it said is the caller's
            // business (auth, errors); here it only means "reachable".
            HealthCheckOutcome::Authenticated { .. }
            | HealthCheckOutcome::TokenExpired
            | HealthCheckOutcome::ServerError(_) => {
                self.consecutive_failures = 0;
                self.connectivity = ApiConnectivity::Online;
            }
            HealthCheckOutcome::NetworkError(_) => {
                self.consecutive_failures = self.consecutive_failures.saturating_add(1);
                if self.consecutive_failures >= FAILURES_BEFORE_UNREACHABLE {
                    self.connectivity = ApiConnectivity::Unreachable;
                }
                // Below the threshold the previous state stands: a
                // single miss does not take "Online" away.
            }
        }

        match (before, self.connectivity) {
            (ApiConnectivity::Online, ApiConnectivity::Unreachable) => Transition::Lost,
            (ApiConnectivity::Unreachable, ApiConnectivity::Online) => Transition::Restored,
            _ => Transition::None,
        }
    }

    /// How long to wait after the last check before the next one.
    pub fn interval(&self) -> Duration {
        match self.consecutive_failures {
            0 => INTERVAL_ONLINE,
            1 => INTERVAL_AFTER_FAILURE,
            _ => INTERVAL_UNREACHABLE,
        }
    }

    /// For the log line: "1 of 2" reads as a warning, "2 of 2" as news.
    pub fn failures(&self) -> u32 {
        self.consecutive_failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok() -> HealthCheckOutcome {
        HealthCheckOutcome::Authenticated { email: None, user_id: "u".into() }
    }
    fn net() -> HealthCheckOutcome {
        HealthCheckOutcome::NetworkError("NetworkError when attempting to fetch resource".into())
    }

    /// The field report, as a transition table: one failed fetch must
    /// not change anything the user can see.
    #[test]
    fn one_network_error_is_noise() {
        let mut p = HealthPolicy::default();
        assert_eq!(p.record(&ok()), Transition::None);
        assert_eq!(p.connectivity(), ApiConnectivity::Online);

        assert_eq!(p.record(&net()), Transition::None, "no notification on a single miss");
        assert_eq!(p.connectivity(), ApiConnectivity::Online, "Save Online stays enabled");
        assert_eq!(p.interval(), INTERVAL_AFTER_FAILURE, "but the next check comes soon");

        assert_eq!(p.record(&ok()), Transition::None, "and clearing it is silent too");
        assert_eq!(p.interval(), INTERVAL_ONLINE);
    }

    #[test]
    fn two_in_a_row_is_a_signal_and_recovery_is_announced_once() {
        let mut p = HealthPolicy::default();
        p.record(&ok());
        p.record(&net());
        assert_eq!(p.record(&net()), Transition::Lost);
        assert_eq!(p.connectivity(), ApiConnectivity::Unreachable);
        assert_eq!(p.interval(), INTERVAL_UNREACHABLE);

        // Still down: no repeated "lost" notifications.
        assert_eq!(p.record(&net()), Transition::None);
        assert_eq!(p.failures(), 3);

        assert_eq!(p.record(&ok()), Transition::Restored);
        assert_eq!(p.connectivity(), ApiConnectivity::Online);
        assert_eq!(p.interval(), INTERVAL_ONLINE);
    }

    /// Never having reached the server is not "losing" it — no
    /// connection-lost toast at startup on a plane.
    #[test]
    fn failing_from_unknown_is_not_a_loss() {
        let mut p = HealthPolicy::default();
        assert_eq!(p.record(&net()), Transition::None);
        assert_eq!(p.connectivity(), ApiConnectivity::Unknown);
        assert_eq!(p.record(&net()), Transition::None);
        assert_eq!(p.connectivity(), ApiConnectivity::Unreachable);
    }

    /// An HTTP error is the server talking. A 500 during a deploy must
    /// not read as a network outage, and must clear a pending miss.
    #[test]
    fn any_http_response_counts_as_reachable() {
        let mut p = HealthPolicy::default();
        p.record(&ok());
        p.record(&net());
        assert_eq!(p.record(&HealthCheckOutcome::ServerError("500".into())), Transition::None);
        assert_eq!(p.connectivity(), ApiConnectivity::Online);
        assert_eq!(p.failures(), 0);

        let mut q = HealthPolicy::default();
        q.record(&ok());
        q.record(&net());
        q.record(&net());
        assert_eq!(q.record(&HealthCheckOutcome::TokenExpired), Transition::Restored);
    }
}

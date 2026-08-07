//! HTTP-backed source of operator-requested emergency stops.
//!
//! Operator E-stop requests are recorded in the neems-api database, which
//! `neems-data` does not connect to. This module mirrors
//! [`schedule_http`](super::schedule_http): it polls neems-api's
//! `GET /api/1/Sites/<id>/EmergencyStop/Pending` endpoint into a shared cache
//! that the synchronous
//! [`EstopRequestSource`](super::control::EstopRequestSource) used by
//! [`ControlLogicTask`](super::control::ControlLogicTask) can read, and reports
//! dispatch back with
//! `POST /api/1/Sites/<id>/EmergencyStop/<request_id>/Dispatch`.
//!
//! Reporting dispatch is what lets neems-api tell "the command never went out"
//! apart from "the command went out and the RTAC did not trip".

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::{
    control::{EstopRequestHandle, EstopRequestSource},
    schedule_http::ApiClientConfig,
};

/// Shared cache of the site's unresolved E-stop request (or `None`).
pub type EstopCache = Arc<Mutex<Option<EstopRequestHandle>>>;

/// Lock the cache, recovering from poisoning rather than propagating a panic.
///
/// The cached value is a `Copy` `Option` written in a single assignment, so a
/// panic elsewhere cannot leave it half-built. Refusing the lock would instead
/// mean a panic in the poller silently swallows every operator E-stop from then
/// on, and unwrapping would take the control loop down with it.
fn lock_cache(cache: &EstopCache) -> std::sync::MutexGuard<'_, Option<EstopRequestHandle>> {
    cache.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// An [`EstopRequestSource`] backed by the polled cache.
///
/// `mark_dispatched` hands the id to the async reporter over a channel rather
/// than blocking the control loop on an HTTP round trip. If the report is lost,
/// neems-api's pending-dispatch timeout still resolves the request, so a lost
/// report degrades to a reported failure rather than a request stuck forever.
pub struct HttpEstopSource {
    cache: EstopCache,
    dispatched_tx: mpsc::UnboundedSender<i64>,
}

impl HttpEstopSource {
    pub fn new(cache: EstopCache, dispatched_tx: mpsc::UnboundedSender<i64>) -> Self {
        Self { cache, dispatched_tx }
    }
}

impl EstopRequestSource for HttpEstopSource {
    fn unresolved(&self) -> Option<EstopRequestHandle> {
        *lock_cache(&self.cache)
    }

    fn mark_dispatched(&self, request_id: i64) {
        // Reflect the dispatch locally straight away so the next control tick
        // does not re-send the command while the report is still in flight.
        {
            let mut cached = lock_cache(&self.cache);
            if let Some(handle) = cached.as_mut() {
                if handle.id == request_id {
                    handle.awaiting_dispatch = false;
                }
            }
        }
        if self.dispatched_tx.send(request_id).is_err() {
            error!(request_id, "E-stop dispatch reporter is gone; cannot report to neems-api");
        }
    }
}

// --- Wire format mirroring neems-api's EstopRequestDto ---

#[derive(Debug, Deserialize)]
struct WireEstopRequest {
    id: i32,
    /// snake_case: "pending" | "dispatched" | "confirmed" | "failed".
    status: String,
}

impl WireEstopRequest {
    fn into_handle(self) -> Option<EstopRequestHandle> {
        match self.status.as_str() {
            "pending" => Some(EstopRequestHandle {
                id: self.id as i64,
                awaiting_dispatch: true,
            }),
            // `dispatched` means the signal reached the RTAC, which is the whole
            // of this system's job — there is nothing left to do. `failed` means
            // it never got there and has timed out. Anything unrecognized is
            // treated the same way: the control loop must not act on a request
            // it cannot interpret.
            other => {
                if other != "dispatched" && other != "failed" {
                    warn!(status = other, "Unknown E-stop request status from API, ignoring");
                }
                None
            }
        }
    }
}

/// Fetch the unresolved E-stop request. `Err(true)` signals an authentication
/// failure (re-login needed).
async fn fetch_pending_estop(
    client: &reqwest::Client,
    config: &ApiClientConfig,
    session_token: &str,
) -> Result<Option<EstopRequestHandle>, bool> {
    let url = format!("{}/api/1/Sites/{}/EmergencyStop/Pending", config.base_url, config.site_id);
    let resp = client
        .get(&url)
        .header(reqwest::header::COOKIE, format!("session={session_token}"))
        .send()
        .await
        .map_err(|e| {
            warn!(error = %e, "Pending E-stop request failed");
            false
        })?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED
        || resp.status() == reqwest::StatusCode::FORBIDDEN
    {
        return Err(true);
    }
    if !resp.status().is_success() {
        warn!(status = %resp.status(), "Pending E-stop returned non-success");
        return Err(false);
    }

    let parsed: Option<WireEstopRequest> = match resp.json().await {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "Failed to parse pending E-stop response");
            return Err(false);
        }
    };
    Ok(parsed.and_then(WireEstopRequest::into_handle))
}

/// Report that the E-stop command has been written to the RTAC.
async fn report_dispatch(
    client: &reqwest::Client,
    config: &ApiClientConfig,
    session_token: &str,
    request_id: i64,
) -> Result<(), bool> {
    let url = format!(
        "{}/api/1/Sites/{}/EmergencyStop/{}/Dispatch",
        config.base_url, config.site_id, request_id
    );
    let resp = client
        .post(&url)
        .header(reqwest::header::COOKIE, format!("session={session_token}"))
        .send()
        .await
        .map_err(|e| {
            warn!(error = %e, request_id, "E-stop dispatch report failed");
            false
        })?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED
        || resp.status() == reqwest::StatusCode::FORBIDDEN
    {
        return Err(true);
    }
    if !resp.status().is_success() {
        warn!(status = %resp.status(), request_id, "E-stop dispatch report returned non-success");
        return Err(false);
    }
    info!(request_id, "Reported E-stop dispatch to neems-api");
    Ok(())
}

/// Poll neems-api for outstanding E-stop requests and report dispatches.
///
/// Runs until the process stops. Polls faster than the schedule poller: an
/// operator pressing E-stop should not wait on a schedule-length interval.
pub async fn run_estop_poller(
    config: ApiClientConfig,
    cache: EstopCache,
    mut dispatched_rx: mpsc::UnboundedReceiver<i64>,
) {
    if !config.has_credentials() {
        warn!(
            "No API credentials (NEEMS_API_EMAIL/PASSWORD); operator E-stop requests will not reach the RTAC"
        );
        return;
    }

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to build HTTP client for E-stop polling");
            return;
        }
    };

    info!(
        base_url = %config.base_url,
        site_id = config.site_id,
        "Starting E-stop request poller"
    );

    let mut interval = tokio::time::interval(ESTOP_POLL_INTERVAL);
    let mut session: Option<String> = None;

    loop {
        // Report dispatches promptly rather than waiting for the next tick, so
        // neems-api starts its confirmation timer when the command actually
        // went out.
        let pending_report = tokio::select! {
            _ = interval.tick() => None,
            Some(request_id) = dispatched_rx.recv() => Some(request_id),
        };

        let token = match &session {
            Some(t) => t.clone(),
            None => match super::schedule_http::login(&client, &config).await {
                Ok(t) => {
                    debug!("Authenticated to neems-api for E-stop polling");
                    session = Some(t.clone());
                    t
                }
                Err(e) => {
                    warn!(error = %e, "Failed to authenticate to neems-api for E-stop polling");
                    continue;
                }
            },
        };

        if let Some(request_id) = pending_report {
            match report_dispatch(&client, &config, &token, request_id).await {
                Ok(()) => {}
                Err(true) => session = None,
                Err(false) => {}
            }
            continue;
        }

        match fetch_pending_estop(&client, &config, &token).await {
            Ok(handle) => {
                match &handle {
                    Some(h) => debug!(
                        request_id = h.id,
                        awaiting_dispatch = h.awaiting_dispatch,
                        "Outstanding E-stop request"
                    ),
                    None => debug!("No outstanding E-stop request"),
                }
                // Preserve a locally-recorded dispatch: neems-api may not have
                // processed our report yet, and re-arming `awaiting_dispatch`
                // would make the control loop send the command a second time.
                let mut cached = lock_cache(&cache);
                *cached = match (handle, *cached) {
                    (Some(fresh), Some(old)) if fresh.id == old.id && !old.awaiting_dispatch => {
                        Some(EstopRequestHandle { awaiting_dispatch: false, ..fresh })
                    }
                    (fresh, _) => fresh,
                };
            }
            Err(true) => {
                debug!("Session expired, will re-authenticate");
                session = None;
            }
            Err(false) => {
                // Transient error; keep the previous cached value.
            }
        }
    }
}

/// How often to check for outstanding E-stop requests.
const ESTOP_POLL_INTERVAL: Duration = Duration::from_secs(1);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_status_awaits_dispatch() {
        let handle = WireEstopRequest { id: 7, status: "pending".to_string() }
            .into_handle()
            .expect("pending is outstanding");
        assert_eq!(handle.id, 7);
        assert!(handle.awaiting_dispatch);
    }

    /// Once the signal has reached the RTAC there is nothing further owed: what
    /// the RTAC does with it is reported through alarm 104, not through the
    /// request.
    #[test]
    fn resolved_and_unknown_statuses_are_not_outstanding() {
        for status in ["dispatched", "failed", "something-new"] {
            assert!(
                WireEstopRequest { id: 7, status: status.to_string() }.into_handle().is_none(),
                "{status} should not be outstanding"
            );
        }
    }

    #[test]
    fn mark_dispatched_clears_awaiting_locally() {
        let cache: EstopCache =
            Arc::new(Mutex::new(Some(EstopRequestHandle { id: 3, awaiting_dispatch: true })));
        let (tx, mut rx) = mpsc::unbounded_channel();
        let source = HttpEstopSource::new(cache.clone(), tx);

        source.mark_dispatched(3);

        assert_eq!(
            source.unresolved(),
            Some(EstopRequestHandle { id: 3, awaiting_dispatch: false }),
            "the control loop must not see the request as needing dispatch again"
        );
        assert_eq!(rx.try_recv().ok(), Some(3), "dispatch should be queued for reporting");
    }

    #[test]
    fn mark_dispatched_ignores_a_different_request() {
        let cache: EstopCache =
            Arc::new(Mutex::new(Some(EstopRequestHandle { id: 3, awaiting_dispatch: true })));
        let (tx, _rx) = mpsc::unbounded_channel();
        let source = HttpEstopSource::new(cache.clone(), tx);

        source.mark_dispatched(99);

        assert_eq!(
            source.unresolved().map(|h| h.awaiting_dispatch),
            Some(true),
            "a stale dispatch must not clear the current request"
        );
    }
}

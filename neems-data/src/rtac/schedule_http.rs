//! HTTP-backed schedule provider.
//!
//! The schedule a site runs on lives in the neems-api database, which
//! `neems-data` does not connect to. Instead, this module polls neems-api's
//! `GET /api/1/Sites/<id>/ActiveCommand` endpoint and caches the active command
//! so the synchronous [`ScheduleProvider`](super::control::ScheduleProvider)
//! used by [`ControlLogicTask`](super::control::ControlLogicTask) can read it.

use std::{
    env,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use tracing::{debug, error, info, warn};

use super::{
    control::{ScheduleProvider, ScheduledCommand},
    protocol::CommandType,
};

/// Shared cache of the currently-active scheduled command (or `None`).
pub type CommandCache = Arc<Mutex<Option<ScheduledCommand>>>;

/// Configuration for polling the neems-api active-command endpoint.
#[derive(Debug, Clone)]
pub struct ApiClientConfig {
    /// Base URL of neems-api, e.g. `http://neems-api:8000`.
    pub base_url: String,
    /// Login email for the service session.
    pub email: String,
    /// Login password for the service session.
    pub password: String,
    /// Site whose schedule we follow.
    pub site_id: i32,
    /// How often to poll for the active command.
    pub poll_interval: Duration,
}

impl ApiClientConfig {
    /// Build configuration from the environment for `site_id`.
    ///
    /// Honors `NEEMS_API_URL`, `NEEMS_API_EMAIL`, `NEEMS_API_PASSWORD`, falling
    /// back to `NEEMS_DEFAULT_EMAIL` / `NEEMS_DEFAULT_PASSWORD` for
    /// credentials.
    pub fn from_env(site_id: i32) -> Self {
        let email = env::var("NEEMS_API_EMAIL")
            .or_else(|_| env::var("NEEMS_DEFAULT_EMAIL"))
            .unwrap_or_default();
        let password = env::var("NEEMS_API_PASSWORD")
            .or_else(|_| env::var("NEEMS_DEFAULT_PASSWORD"))
            .unwrap_or_default();
        Self {
            base_url: env::var("NEEMS_API_URL")
                .unwrap_or_else(|_| "http://neems-api:8000".to_string()),
            email,
            password,
            site_id,
            poll_interval: Duration::from_secs(5),
        }
    }

    /// Whether credentials are configured.
    pub fn has_credentials(&self) -> bool {
        !self.email.is_empty() && !self.password.is_empty()
    }
}

/// A [`ScheduleProvider`] that returns the latest command fetched from
/// neems-api.
pub struct HttpScheduleProvider {
    cache: CommandCache,
}

impl HttpScheduleProvider {
    pub fn new(cache: CommandCache) -> Self {
        Self { cache }
    }
}

impl ScheduleProvider for HttpScheduleProvider {
    fn get_active_command(&self, _now: DateTime<Utc>) -> Option<ScheduledCommand> {
        // The endpoint already determined which command is active; return it
        // verbatim. `None` means no schedule, so the control logic falls back
        // to standby.
        self.cache.lock().unwrap().clone()
    }

    fn reload(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Refresh is handled by the background poller.
        Ok(())
    }
}

// --- Wire format mirroring neems-api's ActiveCommandResponse ---

#[derive(Debug, Deserialize)]
struct ActiveCommandResponse {
    command: Option<WireCommand>,
}

#[derive(Debug, Deserialize)]
struct WireCommand {
    command_id: i32,
    /// snake_case: "charge" | "discharge" | "trickle_charge".
    command_type: String,
    target_soc_percent: Option<i32>,
    duration_seconds: Option<i32>,
    ramp_duration_seconds: i32,
    starts_at: chrono::NaiveDateTime,
}

impl WireCommand {
    fn into_scheduled(self) -> Option<ScheduledCommand> {
        let command_type = match self.command_type.as_str() {
            "charge" => CommandType::Charge,
            "discharge" => CommandType::Discharge,
            "trickle_charge" => CommandType::TrickleCharge,
            other => {
                warn!(command_type = other, "Unknown command type from API, ignoring");
                return None;
            }
        };
        Some(ScheduledCommand {
            id: self.command_id as i64,
            command_type,
            starts_at: self.starts_at.and_utc(),
            ends_at: None,
            duration_seconds: self.duration_seconds,
            target_soc_percent: self.target_soc_percent,
            ramp_duration_seconds: Some(self.ramp_duration_seconds),
        })
    }
}

/// Log in to neems-api and return the session token.
///
/// neems-api flags the session cookie `Secure`, so reqwest's cookie store will
/// not resend it over plain http between containers. We capture the token from
/// the login response and attach it manually on subsequent requests.
async fn login(client: &reqwest::Client, config: &ApiClientConfig) -> Result<String, String> {
    let url = format!("{}/api/1/login", config.base_url);
    let body = serde_json::json!({ "email": config.email, "password": config.password });
    let resp = client.post(&url).json(&body).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("login returned HTTP {}", resp.status()));
    }
    resp.cookies()
        .find(|c| c.name() == "session")
        .map(|c| c.value().to_string())
        .ok_or_else(|| "login response had no session cookie".to_string())
}

/// Fetch the active command. `Ok(None)` means no active command; `Err(true)`
/// signals an authentication failure (re-login needed).
async fn fetch_active_command(
    client: &reqwest::Client,
    config: &ApiClientConfig,
    session_token: &str,
) -> Result<Option<ScheduledCommand>, bool> {
    let url = format!("{}/api/1/Sites/{}/ActiveCommand", config.base_url, config.site_id);
    let resp = client
        .get(&url)
        .header(reqwest::header::COOKIE, format!("session={session_token}"))
        .send()
        .await
        .map_err(|e| {
            warn!(error = %e, "ActiveCommand request failed");
            false
        })?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED
        || resp.status() == reqwest::StatusCode::FORBIDDEN
    {
        return Err(true);
    }
    if !resp.status().is_success() {
        warn!(status = %resp.status(), "ActiveCommand returned non-success");
        return Err(false);
    }

    let parsed: ActiveCommandResponse = match resp.json().await {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "Failed to parse ActiveCommand response");
            return Err(false);
        }
    };
    Ok(parsed.command.and_then(WireCommand::into_scheduled))
}

/// Poll the neems-api active-command endpoint forever, updating `cache`.
///
/// Re-authenticates on session expiry. Network/parse errors are logged and the
/// previous cached value is retained.
pub async fn run_active_command_poller(config: ApiClientConfig, cache: CommandCache) {
    if !config.has_credentials() {
        warn!(
            "No API credentials (NEEMS_API_EMAIL/PASSWORD); RTAC commands will not be driven by schedules"
        );
        return;
    }

    let client = match reqwest::Client::builder()
        // Bound every request so a slow/restarting neems-api can never stall the
        // poll loop indefinitely.
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to build HTTP client for schedule polling");
            return;
        }
    };

    info!(
        base_url = %config.base_url,
        site_id = config.site_id,
        "Starting active-command poller"
    );

    let mut interval = tokio::time::interval(config.poll_interval);
    let mut session: Option<String> = None;

    loop {
        interval.tick().await;

        let token = match &session {
            Some(t) => t.clone(),
            None => match login(&client, &config).await {
                Ok(t) => {
                    debug!("Authenticated to neems-api");
                    session = Some(t.clone());
                    t
                }
                Err(e) => {
                    warn!(error = %e, "Failed to authenticate to neems-api");
                    continue;
                }
            },
        };

        match fetch_active_command(&client, &config, &token).await {
            Ok(command) => {
                match &command {
                    Some(c) => info!(
                        command_id = c.id,
                        command_type = %c.command_type,
                        "Active command updated from neems-api"
                    ),
                    None => debug!("neems-api reports no active command"),
                }
                *cache.lock().unwrap() = command;
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

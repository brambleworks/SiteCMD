//! Integration scheduler: drives all `IntegrationAdapter` implementations on their declared
//! cadences, and exposes an immediate-poll path for `verify_issue` to use.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, Mutex, Semaphore};

use crate::db::Database;
use crate::integrations::adapters::{Credentials, IntegrationAdapter, PollContext};

/// Composite key for tracking last-run time per (source, project, env).
type AdapterKey = (String, i64, String);
const MAX_CONCURRENT_ADAPTER_POLLS: usize = 4;

/// Multiplier applied to a Free-tier project's adapter cadence so we don't
/// burn external API quota on a tier that doesn't pay. Core/Pro use the
/// adapter's declared cadence as-is.
const FREE_CADENCE_MULTIPLIER: u32 = 6;

fn tier_adjusted_cadence(
    base: std::time::Duration,
    tier: crate::licensing::config::Tier,
) -> std::time::Duration {
    if matches!(tier, crate::licensing::config::Tier::Free) {
        base * FREE_CADENCE_MULTIPLIER
    } else {
        base
    }
}

/// Keep the first, production-preferred environment for each project.
fn first_env_per_project(project_envs: &[(i64, String)]) -> Vec<(i64, String)> {
    let mut seen_projects = std::collections::HashSet::new();
    project_envs
        .iter()
        .filter(|(project_id, _)| seen_projects.insert(*project_id))
        .cloned()
        .collect()
}

fn mark_due_or_seed(
    last_run: &mut HashMap<AdapterKey, Instant>,
    key: AdapterKey,
    now: Instant,
    cadence: Duration,
) -> bool {
    match last_run.get(&key).copied() {
        Some(previous) if now.duration_since(previous) >= cadence => {
            last_run.insert(key, now);
            true
        }
        Some(_) => false,
        None => {
            last_run.insert(key, now);
            false
        }
    }
}

pub struct ImmediateRequest {
    pub source: String,
    pub project_id: i64,
    pub env_url: Option<String>,
}

pub struct IntegrationScheduler {
    adapters: Vec<Arc<dyn IntegrationAdapter>>,
    last_run: Mutex<HashMap<AdapterKey, Instant>>,
    poll_limiter: Arc<Semaphore>,
    immediate_tx: mpsc::UnboundedSender<ImmediateRequest>,
    // The single consumer takes ownership once at startup, avoiding a mutex
    // held across recv.await.
    immediate_rx: Mutex<Option<mpsc::UnboundedReceiver<ImmediateRequest>>>,
}

/// Map credential-bearing adapter sources to their `IntegrationType` names.
#[tracing::instrument(fields(source = %source))]
pub(crate) fn source_to_service_type(source: &str) -> Option<&'static str> {
    match source {
        "uptimerobot" => Some("uptimerobot"),
        "plausible" => Some("plausible"),
        "cloudflare" => Some("cloudflare"),
        "gsc" => Some("googlesearchconsole"),
        "ga4" => Some("googleanalytics"),
        _ => None,
    }
}

/// Map a Google adapter source to its `IntegrationType` so the scheduler can
/// refresh the OAuth token before polling. Non-Google sources return None and
/// use their stored credentials verbatim.
fn google_integration_type(source: &str) -> Option<crate::integrations::IntegrationType> {
    use crate::integrations::IntegrationType;
    match source {
        "gsc" => Some(IntegrationType::GoogleSearchConsole),
        "ga4" => Some(IntegrationType::GoogleAnalytics),
        _ => None,
    }
}

/// Distinguish an absent GitHub integration from one that could not be read.
/// Only `NotConfigured` proves there is no CI state to observe.
#[derive(Debug)]
pub enum GithubContextResolution {
    NotConfigured,
    Resolved(crate::integrations::adapters::GithubContext),
    Unobservable,
}

/// Resolve GitHub context with injected keyring readers.
/// Readers run only when stored configuration requires credential hydration.
pub(crate) fn github_context_from_configs(
    configs: Vec<crate::integrations::IntegrationConfig>,
    get_api_key: impl FnOnce() -> Result<Option<String>, String>,
    get_tokens: impl FnOnce() -> Result<Option<String>, String>,
) -> GithubContextResolution {
    use crate::integrations::adapters::GithubContext;
    use crate::integrations::IntegrationType;

    let Some(mut cfg) = configs
        .into_iter()
        .find(|c| c.enabled && c.integration_type == IntegrationType::GitHub)
    else {
        return GithubContextResolution::NotConfigured;
    };

    // site_id is stored as "owner/repo". A missing or malformed repo spec is
    // a deliberate (mis)configuration with nothing to observe, not a
    // transient failure.
    let Some(repo_spec) = cfg.site_id.as_deref().filter(|s| !s.is_empty()) else {
        return GithubContextResolution::NotConfigured;
    };
    let Some((owner, repo)) = repo_spec.split_once('/') else {
        return GithubContextResolution::NotConfigured;
    };
    let (owner, repo) = (owner.to_string(), repo.to_string());

    // Hydrate API key (PAT) from keyring if placeholder. A keyring read error
    // (locked keychain) means a stored credential exists but could not be
    // observed this pass - never "not configured".
    if cfg.api_key.as_deref() == Some(crate::keyring::KEYRING_PLACEHOLDER) {
        match get_api_key() {
            Ok(Some(real)) => cfg.api_key = Some(real),
            Ok(None) => {}
            Err(e) => {
                tracing::warn!("resolve_github_context: keyring API key read failed: {}", e);
                return GithubContextResolution::Unobservable;
            }
        }
    }

    // Prefer the PAT; otherwise take the OAuth token from the keychain. An
    // inline `extra.tokens` is never consulted: the keychain is the boundary,
    // and `without_unmigrated_plaintext_secrets` has already removed any
    // plaintext copy a failed migration left in SQLite.
    let pat = cfg
        .api_key
        .filter(|k| !k.is_empty() && k != crate::keyring::KEYRING_PLACEHOLDER);
    let token = match pat {
        Some(token) => Some(token),
        None => match get_tokens() {
            Ok(tokens_json) => tokens_json.and_then(|tokens_json| {
                serde_json::from_str::<serde_json::Value>(&tokens_json)
                    .ok()
                    .and_then(|v| {
                        v.pointer("/access_token")
                            .and_then(|t| t.as_str())
                            .filter(|t| !t.is_empty())
                            .map(|t| t.to_string())
                    })
            }),
            Err(e) => {
                tracing::warn!("resolve_github_context: keyring tokens read failed: {}", e);
                return GithubContextResolution::Unobservable;
            }
        },
    };

    match token {
        Some(token) => GithubContextResolution::Resolved(GithubContext { owner, repo, token }),
        // Configured integration with no credential stored anywhere: an
        // at-rest state (not a transient failure), so it keeps the
        // pre-existing skip-and-resolve behavior.
        None => GithubContextResolution::NotConfigured,
    }
}

/// Resolve credentials for a matched integration config with injected keyring
/// readers, mirroring how `github_context_from_configs` separates from
/// `resolve_github_context` so the wiring is testable without a live
/// `AppHandle` (whose default `Wry` runtime cannot be constructed in tests).
///
/// Applies `without_unmigrated_plaintext_secrets` before any `api_key` or
/// OAuth token is read below, so a plaintext SQLite credential left by a
/// failed keyring migration is dropped rather than handed to an adapter's
/// outbound poll. `audit` is the sink that refusal records to; production
/// passes `keyring::audit_to_log`.
pub(crate) fn credentials_from_configs(
    configs: Vec<crate::integrations::IntegrationConfig>,
    integration_type: crate::integrations::IntegrationType,
    github: Option<crate::integrations::adapters::GithubContext>,
    github_unobservable: bool,
    get_api_key: impl FnOnce() -> Result<Option<String>, String>,
    get_tokens: impl FnOnce() -> Result<Option<String>, String>,
    audit: crate::keyring::RefusalAudit<'_>,
) -> Credentials {
    let configs = crate::keyring::without_unmigrated_plaintext_secrets_with(configs, audit);

    let mut cfg = match configs
        .into_iter()
        .find(|c| c.enabled && c.integration_type == integration_type)
    {
        Some(c) => c,
        None => return Credentials::empty(),
    };

    // Hydrate API key from keychain if it holds the placeholder.
    if cfg.api_key.as_deref() == Some(crate::keyring::KEYRING_PLACEHOLDER) {
        if let Ok(Some(real)) = get_api_key() {
            cfg.api_key = Some(real);
        }
    }

    // The OAuth access token comes from the keychain only: the refusal above
    // removed any `extra.tokens` a failed migration left in SQLite.
    let oauth_token = match get_tokens() {
        Ok(Some(tokens_json)) => serde_json::from_str::<serde_json::Value>(&tokens_json)
            .ok()
            .and_then(|v| {
                v.pointer("/access_token")
                    .and_then(|t| t.as_str())
                    .filter(|t| !t.is_empty())
                    .map(|t| t.to_string())
            }),
        _ => None,
    };

    Credentials {
        api_key: cfg
            .api_key
            .filter(|k| !k.trim().is_empty() && k != crate::keyring::KEYRING_PLACEHOLDER),
        oauth_token,
        site_id: cfg.site_id,
        github,
        github_unobservable,
    }
}

impl IntegrationScheduler {
    #[tracing::instrument(skip(adapters))]
    pub fn new(adapters: Vec<Arc<dyn IntegrationAdapter>>) -> Arc<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        Arc::new(Self {
            adapters,
            last_run: Mutex::new(HashMap::new()),
            poll_limiter: Arc::new(Semaphore::new(MAX_CONCURRENT_ADAPTER_POLLS)),
            immediate_tx: tx,
            immediate_rx: Mutex::new(Some(rx)),
        })
    }

    /// Returns a sender that callers can use to request out-of-band polls.
    #[tracing::instrument(skip(self))]
    pub fn immediate_sender(&self) -> mpsc::UnboundedSender<ImmediateRequest> {
        self.immediate_tx.clone()
    }

    /// Resolve an adapter's enabled configuration and hydrate keyring placeholders.
    #[tracing::instrument(skip(app, db), fields(source = %source, project_id))]
    pub fn resolve_credentials(
        app: &tauri::AppHandle,
        db: &Database,
        source: &str,
        project_id: i64,
    ) -> Credentials {
        use crate::integrations::IntegrationType;

        let (github, github_unobservable) = match Self::resolve_github_context(app, db, project_id)
        {
            GithubContextResolution::Resolved(gh) => (Some(gh), false),
            GithubContextResolution::NotConfigured => (None, false),
            GithubContextResolution::Unobservable => (None, true),
        };
        let type_str = match source_to_service_type(source) {
            Some(t) => t,
            None => {
                return Credentials {
                    github,
                    github_unobservable,
                    ..Credentials::empty()
                };
            }
        };

        // Parse to IntegrationType for config matching.
        let integration_type: IntegrationType =
            match serde_json::from_value(serde_json::Value::String(type_str.to_string())) {
                Ok(t) => t,
                Err(_) => return Credentials::empty(),
            };

        let configs = match db.get_integrations(project_id) {
            Ok(c) => c,
            Err(_) => return Credentials::empty(),
        };

        credentials_from_configs(
            configs,
            integration_type,
            github,
            github_unobservable,
            || crate::keyring::get_api_key(app, db, project_id, type_str),
            || crate::keyring::get_tokens(app, db, project_id, type_str),
            &crate::keyring::audit_to_log,
        )
    }

    /// Loads and hydrates the linked GitHub integration for CI detection.
    /// `NotConfigured` permits normal resolution; `Unobservable` prevents the
    /// CI family from resolving findings during this pass.
    #[tracing::instrument(skip(app, db), fields(project_id))]
    pub fn resolve_github_context(
        app: &tauri::AppHandle,
        db: &Database,
        project_id: i64,
    ) -> GithubContextResolution {
        let configs = match db.get_integrations(project_id) {
            Ok(configs) => configs,
            Err(e) => {
                // Whether a GitHub integration is configured is itself
                // unknown: report unobservable, never "not configured", so a
                // transient DB failure cannot false-resolve active CI items.
                tracing::warn!("resolve_github_context: reading integrations failed: {}", e);
                return GithubContextResolution::Unobservable;
            }
        };
        github_context_from_configs(
            crate::keyring::without_unmigrated_plaintext_secrets(configs),
            || crate::keyring::get_api_key(app, db, project_id, "github"),
            || crate::keyring::get_tokens(app, db, project_id, "github"),
        )
    }

    /// Runs forever. Call once on startup via `tokio::spawn`.
    #[tracing::instrument(skip(self, db, app))]
    pub async fn run(self: Arc<Self>, db: Arc<Database>, app: tauri::AppHandle) {
        let mut tick = tokio::time::interval(crate::constants::INTEGRATION_SCHEDULER_TICK);

        // Take the receiver exclusively. A second `run` call would find
        // None and return - that's correct: this scheduler is single-consumer
        // by design.
        let Some(rx) = self.immediate_rx.lock().await.take() else {
            tracing::error!(
                "integration scheduler: run() called twice, but the immediate-receiver has already been taken; aborting this loop"
            );
            return;
        };

        // Immediate runs drain on their own task so a slow integration fetch
        // cannot delay scheduled ticks or later immediate requests behind it
        // in the loop.
        {
            let scheduler = self.clone();
            let db = db.clone();
            let app = app.clone();
            spawn_immediate_worker(rx, move |req| {
                let scheduler = scheduler.clone();
                let db = db.clone();
                let app = app.clone();
                async move { scheduler.run_immediate(req, db, app).await }
            });
        }

        loop {
            tick.tick().await;
            self.clone().tick_all(db.clone(), app.clone()).await;
        }
    }

    async fn tick_all(self: Arc<Self>, db: Arc<Database>, app: tauri::AppHandle) {
        let now = Instant::now();
        let project_envs = match db.list_all_project_envs() {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("scheduler: list project envs failed: {}", e);
                return;
            }
        };
        // Resolve licensing once per scheduler tick.
        let tier = db.get_effective_tier();
        let one_env_per_project = first_env_per_project(&project_envs);

        for adapter in &self.adapters {
            // Credential-scoped adapters poll once per project (against the
            // production env), never once per environment - a per-env fan-out
            // would upsert one identical alert per env_url.
            let adapter_envs = if adapter.env_scoped() {
                &project_envs
            } else {
                &one_env_per_project
            };
            for (project_id, env_url) in adapter_envs {
                let key: AdapterKey = (adapter.source().to_string(), *project_id, env_url.clone());

                let due = {
                    let mut last = self.last_run.lock().await;
                    mark_due_or_seed(
                        &mut last,
                        key,
                        now,
                        tier_adjusted_cadence(adapter.cadence(), tier),
                    )
                };
                if !due {
                    continue;
                }

                let adapter = adapter.clone();
                let scheduler = self.clone();
                let db = db.clone();
                let app = app.clone();
                let env_url = env_url.clone();
                let project_id = *project_id;
                tokio::spawn(async move {
                    scheduler
                        .run_adapter_once_limited(adapter, db, app, project_id, env_url)
                        .await;
                });
            }
        }
    }

    async fn run_immediate(
        self: Arc<Self>,
        req: ImmediateRequest,
        db: Arc<Database>,
        app: tauri::AppHandle,
    ) {
        let adapter = match self.adapters.iter().find(|a| a.source() == req.source) {
            Some(a) => a.clone(),
            None => {
                tracing::warn!("immediate poll: unknown source '{}'", req.source);
                return;
            }
        };
        // Credential-scoped adapters always poll against the project's
        // production environment, even when the request names a specific one;
        // honoring a non-production env here would recreate the per-env
        // duplicate alerts the scheduled path avoids.
        let project_envs = if adapter.env_scoped() {
            match req.env_url {
                Some(env_url) => vec![env_url],
                None => db.list_project_envs(req.project_id).unwrap_or_default(),
            }
        } else {
            db.list_project_envs(req.project_id)
                .unwrap_or_default()
                .into_iter()
                .take(1)
                .collect()
        };
        for env_url in project_envs {
            self.clone()
                .run_adapter_once_limited(
                    adapter.clone(),
                    db.clone(),
                    app.clone(),
                    req.project_id,
                    env_url,
                )
                .await;
        }
    }

    async fn run_adapter_once_limited(
        self: Arc<Self>,
        adapter: Arc<dyn IntegrationAdapter>,
        db: Arc<Database>,
        app: tauri::AppHandle,
        project_id: i64,
        env_url: String,
    ) {
        let permit = match self.poll_limiter.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(error) => {
                tracing::warn!("scheduler poll limiter closed: {}", error);
                return;
            }
        };
        run_adapter_once(adapter, db, app, project_id, env_url).await;
        drop(permit);
    }
}

/// Process immediate polls serially in FIFO order.
fn spawn_immediate_worker<H, Fut>(
    mut rx: mpsc::UnboundedReceiver<ImmediateRequest>,
    mut handler: H,
) -> tokio::task::JoinHandle<()>
where
    H: FnMut(ImmediateRequest) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    tokio::spawn(async move {
        while let Some(req) = rx.recv().await {
            handler(req).await;
        }
    })
}

#[tracing::instrument(skip(adapter, db, app, env_url), fields(source = adapter.source(), project_id))]
async fn run_adapter_once(
    adapter: Arc<dyn IntegrationAdapter>,
    db: Arc<Database>,
    app: tauri::AppHandle,
    project_id: i64,
    env_url: String,
) {
    let mut creds =
        IntegrationScheduler::resolve_credentials(&app, &db, adapter.source(), project_id);

    // Refresh Google tokens before scheduled polls and skip the observation if
    // refresh fails, rather than resolving findings from invalid credentials.
    if let Some(integration_type) = google_integration_type(adapter.source()) {
        match crate::integrations::google_oauth::resolve_valid_google_token(
            &app,
            &db,
            project_id,
            &integration_type,
        )
        .await
        {
            Ok(token) => creds.oauth_token = Some(token),
            Err(e) => {
                tracing::info!(
                    "adapter '{}' skipped: could not resolve a valid Google token: {}",
                    adapter.source(),
                    e
                );
                return;
            }
        }
    }

    if !adapter.is_configured(&creds) {
        tracing::debug!(
            "adapter '{}' skipped because it is not configured",
            adapter.source()
        );
        return;
    }

    let ctx = PollContext {
        project_id,
        env_url: env_url.clone(),
        detected_stack: None,
        credentials: creds,
    };
    match adapter.poll(&ctx).await {
        Ok(out) => {
            let now_ms = chrono::Utc::now().timestamp_millis();
            apply_poll_output(&db, adapter.source(), project_id, &env_url, out, now_ms);
        }
        Err(e) => match &e {
            crate::integrations::adapters::AdapterError::AuthFailed(_) => {
                tracing::info!(
                    "adapter '{}' skipped because credentials were rejected: {}",
                    adapter.source(),
                    e
                );
            }
            crate::integrations::adapters::AdapterError::RateLimited => {
                tracing::info!(
                    "adapter '{}' skipped because the upstream API rate limit is exhausted",
                    adapter.source()
                );
            }
            _ => {
                tracing::warn!("adapter '{}' poll failed: {}", adapter.source(), e);
            }
        },
    }
}

/// Persists a poll without resolving findings whose absence was not observed.
///
/// Partial polls resolve nothing; scoped gaps preserve only their signal
/// families; complete polls use normal diff resolution.
pub(crate) fn apply_poll_output(
    db: &Database,
    source: &str,
    project_id: i64,
    env_url: &str,
    out: crate::integrations::adapters::PollOutput,
    now_ms: i64,
) {
    let upsert_result = if out.partial {
        db.upsert_work_items_observe_only(source, project_id, env_url, out.work_items, now_ms)
    } else if out.unobserved_signal_prefixes.is_empty() {
        db.upsert_work_items_diff(source, project_id, env_url, out.work_items, now_ms)
    } else {
        db.upsert_work_items_diff_except_unobserved(
            source,
            project_id,
            env_url,
            out.work_items,
            now_ms,
            &out.unobserved_signal_prefixes,
        )
    };
    if let Err(e) = upsert_result {
        tracing::error!("{} upsert work_items failed: {}", source, e);
    }
    for alert in out.alerts {
        if let Err(e) = db.upsert_alert(alert) {
            tracing::error!("{} upsert alert failed: {}", source, e);
        }
    }
}

// Shared trigger for immediate verification polls.

static IMMEDIATE_TX: OnceLock<mpsc::UnboundedSender<ImmediateRequest>> = OnceLock::new();

/// Store the scheduler's immediate sender in the global handle.
/// Must be called once after `IntegrationScheduler::new`.
#[tracing::instrument(skip(tx))]
pub fn set_immediate_sender(tx: mpsc::UnboundedSender<ImmediateRequest>) {
    let _ = IMMEDIATE_TX.set(tx);
}

/// Request an out-of-band poll for a specific source/project, optionally scoped
/// to one environment URL.
/// Returns an error if the scheduler has not been initialized yet.
#[tracing::instrument(skip(env_url), fields(source = %source, project_id, has_env_url = env_url.is_some()))]
pub async fn request_immediate_poll(
    source: &str,
    project_id: i64,
    env_url: Option<&str>,
) -> Result<(), String> {
    send_immediate_request(IMMEDIATE_TX.get(), source, project_id, env_url)
}

fn send_immediate_request(
    tx: Option<&mpsc::UnboundedSender<ImmediateRequest>>,
    source: &str,
    project_id: i64,
    env_url: Option<&str>,
) -> Result<(), String> {
    tx.ok_or_else(|| "scheduler not initialized".to_string())?
        .send(ImmediateRequest {
            source: source.to_string(),
            project_id,
            env_url: env_url.map(str::to_string),
        })
        .map_err(|e| format!("send failed: {}", e))
}

#[cfg(test)]
#[path = "integration_scheduler_tests.rs"]
mod tests;

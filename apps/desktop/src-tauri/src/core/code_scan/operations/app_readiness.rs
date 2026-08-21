use super::*;

struct AppReadinessContext<'a> {
    files: &'a [SourceFile],
    manifests: &'a [PackageManifest],
    route_files: &'a [&'a SourceFile],
    llm_files: &'a [&'a SourceFile],
    llm_file: Option<&'a SourceFile>,
    db_file: Option<&'a SourceFile>,
    background_job_files: &'a [&'a SourceFile],
    client_auth_files: &'a [&'a SourceFile],
    server_action_files: &'a [&'a SourceFile],
    route_db_files: &'a [&'a SourceFile],
    app_like: bool,
    complex_app: bool,
    frontend_app: bool,
    db_backed: bool,
    ai_heavy_project: bool,
    has_healthcheck: bool,
    has_error_reporting: bool,
    has_structured_logging: bool,
    has_ai_observability_integration: bool,
    has_server_auth_enforcement: bool,
    has_error_boundary: bool,
    has_feature_flags: bool,
    has_job_visibility: bool,
    has_migration_workflow: bool,
    has_shared_data_layer: bool,
    has_recovery_notes: bool,
    has_rollback_notes: bool,
    has_backup_restore_notes: bool,
    deploy_configs: &'a [TextArtifact],
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_app_readiness_issues(
    issues: &mut Vec<CodeIssue>,
    files: &[SourceFile],
    manifests: &[PackageManifest],
    route_files: &[&SourceFile],
    llm_files: &[&SourceFile],
    llm_file: Option<&SourceFile>,
    db_file: Option<&SourceFile>,
    background_job_files: &[&SourceFile],
    client_auth_files: &[&SourceFile],
    server_action_files: &[&SourceFile],
    route_db_files: &[&SourceFile],
    app_like: bool,
    complex_app: bool,
    frontend_app: bool,
    db_backed: bool,
    ai_heavy_project: bool,
    has_healthcheck: bool,
    has_error_reporting: bool,
    has_structured_logging: bool,
    has_ai_observability_integration: bool,
    has_server_auth_enforcement: bool,
    has_error_boundary: bool,
    has_feature_flags: bool,
    has_job_visibility: bool,
    has_migration_workflow: bool,
    has_shared_data_layer: bool,
    has_recovery_notes: bool,
    has_rollback_notes: bool,
    has_backup_restore_notes: bool,
    deploy_configs: &[TextArtifact],
) {
    let context = AppReadinessContext {
        files,
        manifests,
        route_files,
        llm_files,
        llm_file,
        db_file,
        background_job_files,
        client_auth_files,
        server_action_files,
        route_db_files,
        app_like,
        complex_app,
        frontend_app,
        db_backed,
        ai_heavy_project,
        has_healthcheck,
        has_error_reporting,
        has_structured_logging,
        has_ai_observability_integration,
        has_server_auth_enforcement,
        has_error_boundary,
        has_feature_flags,
        has_job_visibility,
        has_migration_workflow,
        has_shared_data_layer,
        has_recovery_notes,
        has_rollback_notes,
        has_backup_restore_notes,
        deploy_configs,
    };

    collect_service_operations_issues(issues, &context);
    collect_runtime_control_issues(issues, &context);
    collect_data_operations_issues(issues, &context);
}

fn collect_service_operations_issues(
    issues: &mut Vec<CodeIssue>,
    context: &AppReadinessContext<'_>,
) {
    let app_like = context.app_like;
    let route_files = context.route_files;
    let complex_app = context.complex_app;
    let frontend_app = context.frontend_app;
    let ai_heavy_project = context.ai_heavy_project;
    let db_backed = context.db_backed;
    let background_job_files = context.background_job_files;
    let client_auth_files = context.client_auth_files;
    let has_error_reporting = context.has_error_reporting;
    let manifests = context.manifests;
    let deploy_configs = context.deploy_configs;
    let has_rollback_notes = context.has_rollback_notes;
    let has_structured_logging = context.has_structured_logging;
    let has_ai_observability_integration = context.has_ai_observability_integration;
    let llm_files = context.llm_files;
    let has_healthcheck = context.has_healthcheck;

    if app_like && !has_healthcheck {
        if let Some(file) = route_files.first() {
            issues.push(build_issue(
                "healthcheck-missing",
                "operations",
                Severity::Medium,
                "No recognized health or readiness endpoint was found",
                "The scanned routes indicate a server application, but SiteCMD found no recognized liveness, readiness, health, status, or ping endpoint. A deploy-platform probe, externally configured route, custom naming convention, or code outside the scanned tree may already provide the required signal.",
                file,
                None,
                Some("Server routes were detected, but no route path or handler looked like /health, /ready, /status, or /ping.".into()),
                Some("First check the deploy platform for an existing process or instance probe. If an HTTP endpoint is needed, keep liveness cheap and dependency-free, and use a separate bounded readiness check only for dependencies required before the instance receives traffic. Return no sensitive diagnostics.".into()),
                Some("Configure the actual platform probe and simulate both a healthy instance and a required-dependency outage. Confirm readiness can remove traffic without causing a liveness restart loop, and that unauthenticated responses expose no internals.".into()),
            ));
        }
    }

    let needs_error_reporting = complex_app
        || ai_heavy_project
        || db_backed
        || !background_job_files.is_empty()
        || !client_auth_files.is_empty();

    if (app_like || frontend_app) && needs_error_reporting && !has_error_reporting {
        let anchor = manifests
            .first()
            .map(|manifest| {
                (
                    manifest.relative_path.clone(),
                    manifest.absolute_path.to_string_lossy().to_string(),
                    None,
                )
            })
            .or_else(|| {
                route_files.first().map(|file| {
                    (
                        file.relative_path.clone(),
                        file.absolute_path.to_string_lossy().to_string(),
                        None,
                    )
                })
            });

        if let Some((relative_path, absolute_path, line)) = anchor {
            // Absence-of-signal heuristic: graded by the shared confidence
            // policy (NeedsReview), like its build_issue siblings.
            let (confidence, confidence_reason) = policy_confidence("error-reporting-missing");
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("error-reporting-missing:{}", relative_path),
                category: "operations".into(),
                severity: Severity::Medium,
                title: "No recognized production error-reporting path was found".into(),
                description: "The scanned dependencies and initialization patterns do not show a recognized error-reporting integration for an app with several failure-prone surfaces. This does not establish that reporting is absent: a custom collector, platform logs, wrapper package, inherited runtime, or external configuration may provide it outside the patterns inspected.".into(),
                relative_path,
                absolute_path,
                line,
                source_excerpt: None,
                evidence: Some(redact_evidence("High-risk app surfaces were detected, but no common error-reporting dependency or initialization pattern was found.")),
                why_now: Some("Without a tested failure-reporting path, production-only errors can be difficult to detect, group, and diagnose; unfiltered reporting can also expose personal or secret data.".into()),
                likely_fix: Some("First verify whether the deploy platform or an internal wrapper already captures failures. If coverage is missing, add an appropriate server and client reporting path with environment tags, release identifiers, sampling, access controls, and secret or personal-data scrubbing. A commercial vendor is optional.".into()),
                confidence,
                confidence_reason,
                verify_hint: Some("Trigger a safe test exception in staging and confirm the approved reporting destination receives one scrubbed event with the expected environment and release context, while the public response remains generic.".into()),
            });
        }
    }

    let has_release_script = manifests.iter().any(|manifest| {
        serde_json::from_str::<Value>(&manifest.content)
            .ok()
            .and_then(|json| json.get("scripts").and_then(Value::as_object).cloned())
            .is_some_and(|scripts| {
                ["build", "deploy", "start"]
                    .iter()
                    .any(|name| scripts.contains_key(*name))
            })
    });
    if app_like && (has_release_script || !deploy_configs.is_empty()) && !has_rollback_notes {
        let anchor = deploy_configs
            .first()
            .map(|config| {
                (
                    config.relative_path.clone(),
                    config.absolute_path.to_string_lossy().to_string(),
                )
            })
            .or_else(|| {
                manifests.first().map(|manifest| {
                    (
                        manifest.relative_path.clone(),
                        manifest.absolute_path.to_string_lossy().to_string(),
                    )
                })
            })
            .or_else(|| {
                route_files.first().map(|file| {
                    (
                        file.relative_path.clone(),
                        file.absolute_path.to_string_lossy().to_string(),
                    )
                })
            });

        if let Some((relative_path, absolute_path)) = anchor {
            let (confidence, confidence_reason) = policy_confidence("deploy-rollback-plan-missing");
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("deploy-rollback-plan-missing:{}", relative_path),
                category: "operations".into(),
                severity: Severity::Medium,
                title: "No rollback note found for the deploy path".into(),
                description: "A recognized release script or deploy configuration is present, but the scanned documentation contains no recognized rollback or last-known-good redeploy note. Provider-side runbooks, dashboard procedures, or documentation outside the scanned tree may already cover this path.".into(),
                relative_path,
                absolute_path,
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence("A release script or deploy config was detected, but no rollback, roll back, redeploy, incident, recovery, or runbook note was found in docs or ops notes.")),
                why_now: Some("A tested rollback path reduces recovery time after a bad release, but its exact form depends on the provider, deployment model, database compatibility, and irreversible side effects.".into()),
                likely_fix: Some("Confirm whether an approved provider-side runbook already exists. Otherwise add a short repository link or note naming the deploy provider, owner, last-known-good selection or redeploy procedure, required access role, validation steps, and conditions where rollback is unsafe because of schema changes or external side effects. Do not store credentials in the note.".into()),
                confidence,
                confidence_reason,
                verify_hint: Some("Have an authorized teammate follow the procedure in staging or preview, identify a known-good release, perform or dry-run the provider-supported rollback, and confirm application plus schema compatibility without relying on undocumented credentials or verbal context.".into()),
            });
        }
    }

    if complex_app && !has_structured_logging {
        if let Some(file) = route_files.first() {
            issues.push(build_issue(
                "structured-logging-missing",
                "operations",
                Severity::Medium,
                "No recognized structured logging path was found for server routes",
                "The scanned server code contains several route or background-work surfaces, but no recognized structured logger or stable logging wrapper was found. Structured logs may still exist through platform instrumentation, a custom wrapper, inherited middleware, or code outside the scanned tree, so this finding needs review.",
                file,
                None,
                Some("App-like server routes were detected, but no pino, winston, tracing, logger wrapper, or structured error log pattern was found.".into()),
                Some("Check the runtime and deploy platform for existing structured logs first. If coverage is missing, add one project logging path with stable event names, severity, request or trace correlation, user-safe context, secret and personal-data redaction, and access or retention controls. Avoid logging request bodies or credentials by default.".into()),
                Some("Trigger a controlled server-side failure in staging and confirm one redacted structured event can be correlated to the request without exposing credentials, tokens, personal data, or raw request bodies.".into()),
            ));
        }
    }

    if ai_heavy_project && !has_ai_observability_integration {
        let anchor = manifests
            .first()
            .map(|manifest| {
                (
                    manifest.relative_path.clone(),
                    manifest.absolute_path.to_string_lossy().to_string(),
                    None,
                )
            })
            .or_else(|| {
                llm_files.first().map(|file| {
                    (
                        file.relative_path.clone(),
                        file.absolute_path.to_string_lossy().to_string(),
                        None,
                    )
                })
            });

        if let Some((relative_path, absolute_path, line)) = anchor {
            let (confidence, confidence_reason) =
                policy_confidence("ai-observability-integration-missing");
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("ai-observability-integration-missing:{}", relative_path),
                category: "ai-safety".into(),
                severity: Severity::Medium,
                title: "No recognized AI tracing or usage-cost integration was found".into(),
                description: "The scanned source and dependencies contain AI-heavy usage signals, but SiteCMD found no recognized AI-specific tracing, token-usage, or spend integration. Generic telemetry, provider dashboards, custom wrappers, inherited instrumentation, or external configuration may already provide equivalent coverage.".into(),
                relative_path,
                absolute_path,
                line,
                source_excerpt: None,
                evidence: Some(redact_evidence("AI-heavy usage signals were detected, but no recognized provider-aware tracing, token-usage, spend, or AI-observability integration was found in the scanned dependencies and source patterns.")),
                why_now: Some("Without correlated latency, status, model, retry, and usage signals at the provider boundary, cost and reliability regressions are harder to attribute. Telemetry itself can create privacy risk if prompts, outputs, or identifiers are collected indiscriminately.".into()),
                likely_fix: Some("First inventory existing platform and provider telemetry. If coverage is missing, instrument the server-side model boundary with a request or trace ID, approved provider/model identifier, latency, status/error class, retry count, and provider-reported token usage or cost when available. Exclude prompts, responses, credentials, and direct user identifiers by default; define access, sampling, retention, and deletion controls.".into()),
                confidence,
                confidence_reason,
                verify_hint: Some("Exercise representative successful, failed, retried, and streamed calls in staging. Confirm traces correlate to the right feature and release, usage totals match provider records within documented limits, and captured attributes contain no prompt/output content, secrets, or unapproved personal data.".into()),
            });
        }
    }
}

fn collect_runtime_control_issues(issues: &mut Vec<CodeIssue>, context: &AppReadinessContext<'_>) {
    let files = context.files;
    let route_files = context.route_files;
    let llm_file = context.llm_file;
    let background_job_files = context.background_job_files;
    let client_auth_files = context.client_auth_files;
    let server_action_files = context.server_action_files;
    let app_like = context.app_like;
    let frontend_app = context.frontend_app;
    let has_server_auth_enforcement = context.has_server_auth_enforcement;
    let has_error_boundary = context.has_error_boundary;
    let has_feature_flags = context.has_feature_flags;
    let has_job_visibility = context.has_job_visibility;

    if !client_auth_files.is_empty()
        && (!route_files.is_empty() || !server_action_files.is_empty())
        && !has_server_auth_enforcement
    {
        if let Some(file) = client_auth_files.first() {
            issues.push(build_issue(
                "client-auth-without-server-enforcement",
                "security",
                Severity::High,
                "Client auth signals exist without recognized server enforcement",
                "The scanned frontend contains client-side auth state or UI signals, while SiteCMD found no recognized route, Server Action, or middleware auth enforcement in the scanned backend surface. This does not prove a protected server path is exposed: routes may be public, enforcement may use a custom wrapper, or relevant code may live outside the scanned tree.",
                file,
                first_match_line(&file.content, &AUTH_CLIENT_PATTERNS),
                Some("Client auth hooks or components were detected, but no server route auth patterns or middleware protection were found in the project.".into()),
                Some("Inventory which routes, Server Actions, and RPC methods are intentionally public versus protected. For every protected entry point, verify the session/token at the trusted server boundary and enforce role, tenant, ownership, or capability checks before protected reads, writes, or side effects. Shared middleware can reduce duplication, but each route's authorization requirement remains explicit.".into()),
                Some("Call representative protected paths directly, bypassing the UI, with no session, an invalid session, the wrong role, and the wrong tenant/owner. Confirm the server rejects each case before protected work and that intentionally public paths remain available.".into()),
            ));
        }
    }

    if frontend_app && app_like && !has_error_boundary {
        let anchor = files
            .iter()
            .find(|file| {
                let rel = file.relative_path.to_ascii_lowercase();
                rel.ends_with("/app/page.tsx")
                    || rel.ends_with("/app/page.jsx")
                    || rel.ends_with("/pages/index.tsx")
                    || rel.ends_with("/pages/index.jsx")
            })
            .or_else(|| route_files.first().copied());

        if let Some(file) = anchor {
            issues.push(build_issue(
                "error-boundary-missing",
                "operations",
                Severity::Medium,
                "No recognized frontend error boundary or route error surface was found",
                "The scanned project has frontend application and server-route signals, but SiteCMD found no recognized React error boundary or framework route error file. A custom wrapper, another framework convention, or platform-provided error handling may exist outside the patterns inspected; without an appropriate boundary, some render or route failures can replace the affected UI with an uncontrolled error state.",
                file,
                None,
                Some("React/Next-style app structure was detected, but no ErrorBoundary, app/error.tsx, pages/_error, or equivalent error surface was found.".into()),
                Some("Use the framework-native route error surface and place React boundaries around independently recoverable client subtrees. Provide a safe message and an appropriate retry, reset, or navigation action, and report the original error through the approved scrubbed server/client telemetry path. Remember that React boundaries do not catch every event-handler, async callback, or server failure.".into()),
                Some("Trigger controlled render, route-loader/server, and event-handler failures in a non-production environment. Confirm each is handled by the intended layer, users receive a recoverable response without stack details, and one scrubbed error reaches telemetry.".into()),
            ));
        }
    }

    if let Some(file) = llm_file.filter(|_| !has_feature_flags) {
        issues.push(build_issue(
            "ai-kill-switch-missing",
            "operations",
            Severity::Medium,
            "No recognized operational disable path was found for the AI feature",
            "AI provider usage is present, but SiteCMD found no recognized feature flag, environment gate, or kill-switch pattern in the scanned project. A provider-side control, centralized flag service, gateway policy, or wrapper outside the scanned patterns may already provide an operational disable path.",
            file,
            first_match_line(&file.content, &LLM_PATTERNS),
            Some("AI provider usage was detected, but no feature flag, ENABLE_AI, DISABLE_AI, or kill-switch pattern was found anywhere in the project.".into()),
            Some("Confirm any existing provider, gateway, or centralized feature control first. If coverage is missing, enforce one server-side disable decision before provider work starts and return a deliberate fallback or unavailable state. Use a remotely changeable, audited flag when incident response must not wait for a deploy; an environment flag is suitable only when its restart/redeploy behavior meets the response objective.".into()),
            Some("Disable the feature in staging through the same control operators would use in an incident. Confirm new provider calls and queued retries stop within the documented propagation time, in-flight behavior is defined, users receive the intended fallback, and re-enabling is audited and safe.".into()),
        ));
    }

    if !background_job_files.is_empty() && !has_job_visibility {
        if let Some(file) = background_job_files.first() {
            issues.push(build_issue(
                "job-visibility-missing",
                "operations",
                Severity::Medium,
                "No recognized visibility path was found for background work",
                "The scanned source contains background-job, worker, queue, or schedule signals, but SiteCMD found no recognized worker-event handling, job-status surface, or correlated structured logging. This does not establish that visibility is absent: the queue provider, deploy platform, inherited instrumentation, or an external dashboard may supply it.",
                file,
                first_match_line(&file.content, &BACKGROUND_JOB_PATTERNS),
                Some("Background job or queue patterns were detected, but no clear worker events, queue dashboard, /jobs surface, or job-status hooks were found.".into()),
                Some("Inventory the queue or scheduler's built-in telemetry first. For durable user-visible work, persist an authorized state machine such as queued, running, succeeded, failed, cancelled, and retrying with stable job/tenant correlation. For short internal jobs, correlated structured events and alerts may be sufficient. Redact payloads, errors, and record identifiers according to the data policy.".into()),
                Some("In staging, run successful, failed, retried, cancelled, duplicate, and stuck-job scenarios. Confirm an authorized operator can locate the job, attempt count, timestamps, safe error class, and owning scope without reading raw payloads or unrelated tenant data.".into()),
            ));
        }
    }
}

fn collect_data_operations_issues(issues: &mut Vec<CodeIssue>, context: &AppReadinessContext<'_>) {
    let manifests = context.manifests;
    let db_file = context.db_file;
    let route_db_files = context.route_db_files;
    let complex_app = context.complex_app;
    let db_backed = context.db_backed;
    let has_migration_workflow = context.has_migration_workflow;
    let has_shared_data_layer = context.has_shared_data_layer;
    let has_recovery_notes = context.has_recovery_notes;
    let has_backup_restore_notes = context.has_backup_restore_notes;

    if db_backed && !has_migration_workflow {
        let anchor = manifests
            .first()
            .map(|manifest| {
                (
                    manifest.relative_path.clone(),
                    manifest.absolute_path.to_string_lossy().to_string(),
                )
            })
            .or_else(|| {
                db_file.map(|file| {
                    (
                        file.relative_path.clone(),
                        file.absolute_path.to_string_lossy().to_string(),
                    )
                })
            });

        if let Some((relative_path, absolute_path)) = anchor {
            let (confidence, confidence_reason) = policy_confidence("migration-workflow-missing");
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("migration-workflow-missing:{}", relative_path),
                category: "operations".into(),
                severity: Severity::Medium,
                title: "No recognized schema-change workflow was found".into(),
                description: "The scanned source or dependencies indicate database use, but SiteCMD found no recognized migration directory or migrate/schema-push script showing how schema changes are applied. A framework command, external deployment workflow, custom tool, or code outside the scanned tree may own this process. A seed script or schema definition alone does not establish a schema-change workflow.".into(),
                relative_path,
                absolute_path,
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence("Database-use signals were detected, but no recognized migration directory or migrate/schema-push script was found in the scanned project.")),
                why_now: Some("An undocumented or missing schema-change path can make fresh environments, deploy ordering, rollback compatibility, and recovery difficult to reproduce. The scan does not establish how an external platform applies schema changes.".into()),
                likely_fix: Some("Identify the schema authority and the workflow already intended for this stack. Prefer reviewed, ordered migrations for shared/staging/production databases; a schema-push workflow may be acceptable for disposable development databases when documented. Expose the approved command through a script or runbook, define locking and deploy order, and add seed data only when the product actually requires a reproducible dataset.".into()),
                confidence,
                confidence_reason,
                verify_hint: Some("Create a fresh disposable database and apply only the documented schema workflow to reach the expected schema. Then test an upgrade from the previous release, a failed/partial migration, concurrent deploy protection, and application compatibility before and after the change.".into()),
            });
        }
    }

    if route_db_files.len() >= 3 && !has_shared_data_layer {
        if let Some(file) = route_db_files.first() {
            issues.push(build_issue(
                "db-scattered-across-routes",
                "architecture",
                Severity::Medium,
                "Direct database access appears across several route handlers",
                "SiteCMD found database-use signatures in several route-like files and no recognized shared service, repository, or data-layer pattern. This does not prove the design is duplicated or unsafe: handlers may be thin, shared policy may use custom aliases/wrappers, and some frameworks intentionally colocate small queries with routes.",
                file,
                first_match_line(&file.content, &DB_PATTERNS),
                Some(format!(
                    "Detected direct database usage in {} route files without a clear shared backend layer.",
                    route_db_files.len()
                )),
                Some("Review the named route files for actual duplication or inconsistent authorization, tenant scope, transaction, retry, and query policy. Extract a shared service/repository only where it creates a real policy or reuse boundary; keep genuinely small, cohesive route-local queries when that is the clearer design.".into()),
                Some("Compare equivalent route behaviors before and after any extraction. Confirm authorization, tenant scope, transaction boundaries, error mapping, query plans, and focused tests remain correct; if no harmful duplication exists, document or ignore the finding rather than adding an empty abstraction.".into()),
            ));
        }
    }

    if db_backed && complex_app && !has_recovery_notes {
        let anchor = manifests
            .first()
            .map(|manifest| {
                (
                    manifest.relative_path.clone(),
                    manifest.absolute_path.to_string_lossy().to_string(),
                )
            })
            .or_else(|| {
                db_file.map(|file| {
                    (
                        file.relative_path.clone(),
                        file.absolute_path.to_string_lossy().to_string(),
                    )
                })
            });

        if let Some((relative_path, absolute_path)) = anchor {
            let (confidence, confidence_reason) = policy_confidence("recovery-runbook-missing");
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("recovery-runbook-missing:{}", relative_path),
                category: "operations".into(),
                severity: Severity::Medium,
                title: "No recognized recovery documentation was found for the stateful app".into(),
                description: "Database-backed and complex-app signals are present, but SiteCMD found no recognized backup, restore, recovery, incident, or runbook note in the scanned operational documentation. Provider-side procedures, private runbooks, or controls outside the scanned tree may already cover recovery.".into(),
                relative_path,
                absolute_path,
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence("Database-backed app patterns were detected, but no backup, restore, recovery, or runbook file names were found in the project.")),
                why_now: Some("Recovery capability depends on more than the presence of a document, but an accessible, tested procedure reduces improvisation after bad migrations, accidental deletion, corruption, or provider failure.".into()),
                likely_fix: Some("Confirm the approved private or provider-side procedure first. If documentation is missing, add a repository link or access-controlled runbook covering incident ownership, escalation, backup source and retention, recovery-point/time objectives, safe rollback-versus-repair decisions, non-production restore steps, validation, and communications. Reference access roles or secret locations, never credential values.".into()),
                confidence,
                confidence_reason,
                verify_hint: Some("Have an authorized teammate who did not author the runbook restore a recent backup to an isolated non-production destination. Confirm access, audit logging, data freshness and integrity, application compatibility, cleanup, and measured recovery objectives without undocumented credentials or verbal steps.".into()),
            });
        }
    }

    if db_backed && complex_app && has_recovery_notes && !has_backup_restore_notes {
        let anchor = manifests
            .first()
            .map(|manifest| {
                (
                    manifest.relative_path.clone(),
                    manifest.absolute_path.to_string_lossy().to_string(),
                )
            })
            .or_else(|| {
                db_file.map(|file| {
                    (
                        file.relative_path.clone(),
                        file.absolute_path.to_string_lossy().to_string(),
                    )
                })
            });

        if let Some((relative_path, absolute_path)) = anchor {
            let (confidence, confidence_reason) = policy_confidence("backup-restore-plan-missing");
            issues.push(CodeIssue {
                check_id: String::new(),
                id: format!("backup-restore-plan-missing:{}", relative_path),
                category: "operations".into(),
                severity: Severity::Medium,
                title: "Recovery notes do not explain backup or restore".into(),
                description: "A scanned recovery, runbook, or incident note exists, but SiteCMD found no recognized backup, restore, snapshot, point-in-time recovery, or database-dump language in that operational documentation. The procedure may be provider-managed, linked indirectly, or stored outside the scanned tree.".into(),
                relative_path,
                absolute_path,
                line: None,
                source_excerpt: None,
                evidence: Some(redact_evidence("A recovery, runbook, or incident note was detected, but no backup, restore, snapshot, point-in-time recovery, PITR, or database dump language was found.")),
                why_now: Some("Code rollback and data recovery address different failure modes. A stateful service needs a verified path for the recovery objectives it actually promises, even when the database provider manages the underlying backups.".into()),
                likely_fix: Some("Link or add the approved backup source, scope, schedule, retention, encryption/access model, recovery-point/time objectives, restore owner, provider-supported restore path, target-isolation safeguards, and post-restore validation. Reference credential locations or access roles without placing secrets in the runbook.".into()),
                confidence,
                confidence_reason,
                verify_hint: Some("Restore a recent backup to an isolated non-production destination using the documented access path. Confirm audit logging, data freshness and integrity, application compatibility, measured recovery time, and cleanup; never require credential values to be written in the runbook.".into()),
            });
        }
    }
}

//! On-demand PageSpeed Insights adapter.

use async_trait::async_trait;
use std::time::Duration;

use crate::checks::Severity;
use crate::core::correlation::signal_mapping::resolve_check_id;
use crate::db::work_items::{WorkItemInput, WorkItemMetadata};
use crate::integrations::adapters::{AdapterError, IntegrationAdapter, PollContext, PollOutput};
use crate::integrations::pagespeed::{
    fetch_pagespeed_report, is_pagespeed_rate_limit_error, CwvRating,
};

pub struct PsiAdapter;

impl Default for PsiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PsiAdapter {
    #[tracing::instrument]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl IntegrationAdapter for PsiAdapter {
    fn source(&self) -> &'static str {
        "psi"
    }

    fn cadence(&self) -> Duration {
        // allow-inline-duration: per-adapter polling cadence.
        Duration::from_secs(1800) // 30 minutes
    }

    async fn poll(&self, ctx: &PollContext) -> Result<PollOutput, AdapterError> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut work_items: Vec<WorkItemInput> = Vec::new();

        // Fetch both strategies; failures on one should not abort the other.
        for strategy in &["mobile", "desktop"] {
            // PSI runs keyless here (see note above); the optional API key is
            // threaded through the dashboard/scheduled-scan paths instead.
            let report = match fetch_pagespeed_report(&ctx.env_url, strategy, None).await {
                Ok(r) => r,
                Err(e) => return Err(classify_pagespeed_fetch_error(e)),
            };

            for opp in &report.opportunities {
                let savings_ms = opp.savings_ms.unwrap_or(0.0);

                let severity = if savings_ms >= 1000.0 {
                    Severity::High
                } else if savings_ms >= 200.0 {
                    Severity::Medium
                } else {
                    // Below 200ms savings - not worth surfacing as a work item.
                    continue;
                };

                let check_id = resolve_check_id("psi", &opp.id);
                let signal_id = format!("psi:{}:{}:{}", opp.id, strategy, ctx.env_url);

                let detail_json = serde_json::to_string(&serde_json::json!({
                    "id": opp.id,
                    "title": opp.title,
                    "description": opp.description,
                    "savings_ms": savings_ms,
                    "strategy": strategy,
                }))
                .ok();

                work_items.push(WorkItemInput {
                    project_id: ctx.project_id,
                    env_url: ctx.env_url.clone(),
                    source: "psi".to_string(),
                    signal_id,
                    check_id,
                    category: "performance".to_string(),
                    severity,
                    title: opp.title.clone(),
                    description: opp.description.clone(),
                    detail_json,
                    scan_ref: None,
                    page_url: Some(ctx.env_url.clone()),
                    fix_prompt: None,
                    manual_fix: None,
                    why_it_matters: None,
                    observed_at: now_ms,
                    metadata: WorkItemMetadata::default(),
                });
            }

            if let Some(lcp_ms) = report.lcp_ms {
                let rating = CwvRating::for_lcp(lcp_ms);
                let severity = match rating {
                    CwvRating::Poor => Some(Severity::High),
                    CwvRating::NeedsImprovement => Some(Severity::Medium),
                    CwvRating::Good => None,
                };
                if let Some(sev) = severity {
                    work_items.push(WorkItemInput {
                        project_id: ctx.project_id,
                        env_url: ctx.env_url.clone(),
                        source: "psi".to_string(),
                        signal_id: format!("psi:lab-lcp:{}:{}", strategy, ctx.env_url),
                        check_id: resolve_check_id("psi", "lab-lcp"),
                        category: "performance".to_string(),
                        severity: sev,
                        title: "Largest Contentful Paint".to_string(),
                        description: format!(
                            "LCP is {:.0}ms ({}). Target: <= 2500ms.",
                            lcp_ms, strategy
                        ),
                        detail_json: serde_json::to_string(&serde_json::json!({
                            "metric": "LCP",
                            "value_ms": lcp_ms,
                            "strategy": strategy,
                            "rating": format!("{:?}", rating),
                        }))
                        .ok(),
                        scan_ref: None,
                        page_url: Some(ctx.env_url.clone()),
                        fix_prompt: None,
                        manual_fix: None,
                        why_it_matters: None,
                        observed_at: now_ms,
                        metadata: WorkItemMetadata::default(),
                    });
                }
            }

            if let Some(cls) = report.cls {
                let rating = CwvRating::for_cls(cls);
                let severity = match rating {
                    CwvRating::Poor => Some(Severity::High),
                    CwvRating::NeedsImprovement => Some(Severity::Medium),
                    CwvRating::Good => None,
                };
                if let Some(sev) = severity {
                    work_items.push(WorkItemInput {
                        project_id: ctx.project_id,
                        env_url: ctx.env_url.clone(),
                        source: "psi".to_string(),
                        signal_id: format!("psi:lab-cls:{}:{}", strategy, ctx.env_url),
                        check_id: resolve_check_id("psi", "lab-cls"),
                        category: "performance".to_string(),
                        severity: sev,
                        title: "Cumulative Layout Shift".to_string(),
                        description: format!("CLS is {:.3} ({}). Target: <= 0.1.", cls, strategy),
                        detail_json: serde_json::to_string(&serde_json::json!({
                            "metric": "CLS",
                            "value": cls,
                            "strategy": strategy,
                            "rating": format!("{:?}", rating),
                        }))
                        .ok(),
                        scan_ref: None,
                        page_url: Some(ctx.env_url.clone()),
                        fix_prompt: None,
                        manual_fix: None,
                        why_it_matters: None,
                        observed_at: now_ms,
                        metadata: WorkItemMetadata::default(),
                    });
                }
            }

            if let Some(tbt_ms) = report.tbt_ms {
                let rating = CwvRating::for_tbt(tbt_ms);
                let severity = match rating {
                    CwvRating::Poor => Some(Severity::High),
                    CwvRating::NeedsImprovement => Some(Severity::Medium),
                    CwvRating::Good => None,
                };
                if let Some(sev) = severity {
                    work_items.push(WorkItemInput {
                        project_id: ctx.project_id,
                        env_url: ctx.env_url.clone(),
                        source: "psi".to_string(),
                        signal_id: format!("psi:lab-tbt:{}:{}", strategy, ctx.env_url),
                        check_id: resolve_check_id("psi", "lab-tbt"),
                        category: "performance".to_string(),
                        severity: sev,
                        title: "Total Blocking Time (lab)".to_string(),
                        description: format!(
                            "TBT is {:.0}ms ({}). Target: <= 200ms.",
                            tbt_ms, strategy
                        ),
                        detail_json: serde_json::to_string(&serde_json::json!({
                            "metric": "TBT",
                            "value_ms": tbt_ms,
                            "strategy": strategy,
                            "rating": format!("{:?}", rating),
                        }))
                        .ok(),
                        scan_ref: None,
                        page_url: Some(ctx.env_url.clone()),
                        fix_prompt: None,
                        manual_fix: None,
                        why_it_matters: None,
                        observed_at: now_ms,
                        metadata: WorkItemMetadata::default(),
                    });
                }
            }

            if let Some(lcp) = report.field_lcp_ms {
                if lcp > 2500.0 {
                    work_items.push(WorkItemInput {
                        project_id: ctx.project_id,
                        env_url: ctx.env_url.clone(),
                        source: "psi".to_string(),
                        signal_id: format!("psi:field-lcp:field:{}:{}", strategy, ctx.env_url),
                        check_id: resolve_check_id("psi", "field-lcp"),
                        category: "performance".to_string(),
                        severity: if lcp > 4000.0 { Severity::High } else { Severity::Medium },
                        title: format!("LCP over target: {:.0}ms (p75, field)", lcp),
                        description: "Real users are seeing Largest Contentful Paint above Google's 2.5s target.".to_string(),
                        detail_json: serde_json::to_string(&serde_json::json!({
                            "p75_ms": lcp,
                            "source": report.field_source,
                            "strategy": strategy,
                        }))
                        .ok(),
                        scan_ref: None,
                        page_url: Some(ctx.env_url.clone()),
                        fix_prompt: None,
                        manual_fix: None,
                        why_it_matters: None,
                        observed_at: now_ms,
                        metadata: WorkItemMetadata::default(),
                    });
                }
            }

            if let Some(cls) = report.field_cls {
                if cls > 0.1 {
                    work_items.push(WorkItemInput {
                        project_id: ctx.project_id,
                        env_url: ctx.env_url.clone(),
                        source: "psi".to_string(),
                        signal_id: format!("psi:field-cls:field:{}:{}", strategy, ctx.env_url),
                        check_id: resolve_check_id("psi", "field-cls"),
                        category: "performance".to_string(),
                        severity: if cls > 0.25 { Severity::High } else { Severity::Medium },
                        title: format!("CLS over target: {:.3} (p75, field)", cls),
                        description: "Real users are seeing Cumulative Layout Shift above Google's 0.1 target.".to_string(),
                        detail_json: serde_json::to_string(&serde_json::json!({
                            "p75": cls,
                            "source": report.field_source,
                            "strategy": strategy,
                        }))
                        .ok(),
                        scan_ref: None,
                        page_url: Some(ctx.env_url.clone()),
                        fix_prompt: None,
                        manual_fix: None,
                        why_it_matters: None,
                        observed_at: now_ms,
                        metadata: WorkItemMetadata::default(),
                    });
                }
            }

            if let Some(inp) = report.field_inp_ms {
                if inp > 200.0 {
                    work_items.push(WorkItemInput {
                        project_id: ctx.project_id,
                        env_url: ctx.env_url.clone(),
                        source: "psi".to_string(),
                        signal_id: format!("psi:field-inp:field:{}:{}", strategy, ctx.env_url),
                        check_id: resolve_check_id("psi", "field-inp"),
                        category: "performance".to_string(),
                        severity: if inp > 500.0 { Severity::High } else { Severity::Medium },
                        title: format!("INP over target: {:.0}ms (p75, field)", inp),
                        description: "Real users are seeing Interaction to Next Paint above Google's 200ms target.".to_string(),
                        detail_json: serde_json::to_string(&serde_json::json!({
                            "p75_ms": inp,
                            "source": report.field_source,
                            "strategy": strategy,
                        }))
                        .ok(),
                        scan_ref: None,
                        page_url: Some(ctx.env_url.clone()),
                        fix_prompt: None,
                        manual_fix: None,
                        why_it_matters: None,
                        observed_at: now_ms,
                        metadata: WorkItemMetadata::default(),
                    });
                }
            }
        }

        Ok(PollOutput {
            work_items,
            alerts: vec![],
            partial: false,
            unobserved_signal_prefixes: Vec::new(),
        })
    }
}

fn classify_pagespeed_fetch_error(error: String) -> AdapterError {
    if is_pagespeed_rate_limit_error(&error) {
        AdapterError::RateLimited
    } else {
        AdapterError::Transport(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_is_psi() {
        assert_eq!(PsiAdapter::new().source(), "psi");
    }

    #[test]
    fn cadence_is_30_minutes() {
        assert_eq!(PsiAdapter::new().cadence(), Duration::from_secs(1800));
    }

    #[test]
    fn is_not_configured_without_explicit_opt_in() {
        use crate::integrations::adapters::Credentials;
        assert!(!PsiAdapter::new().is_configured(&Credentials::empty()));
    }

    #[test]
    fn pagespeed_quota_errors_are_rate_limited() {
        let error =
            "PageSpeed API returned 429 Too Many Requests: rate limit exhausted".to_string();

        assert!(matches!(
            classify_pagespeed_fetch_error(error),
            AdapterError::RateLimited
        ));
    }

    #[test]
    fn pagespeed_non_quota_errors_remain_transport_errors() {
        let error = "PageSpeed API request failed: connection reset".to_string();

        assert!(matches!(
            classify_pagespeed_fetch_error(error),
            AdapterError::Transport(_)
        ));
    }
}

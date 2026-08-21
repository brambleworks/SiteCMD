use crate::integrations::{plausible, uptimerobot, IntegrationConfig, IntegrationType};

use super::{
    AnalyticsSummary, DailyPoint, SectionConfig, TopPageEntry, TopSourceEntry, UptimeSummary,
};

#[cfg(feature = "desktop")]
pub(super) async fn fetch_analytics_summary(
    configs: &[IntegrationConfig],
    sections: &SectionConfig,
    period_days: u32,
) -> Option<AnalyticsSummary> {
    if !sections.analytics {
        return None;
    }

    let config = configs
        .iter()
        .find(|c| c.integration_type == IntegrationType::Plausible)?;
    let (Some(api_key), Some(site_id)) = (config.api_key.as_deref(), config.site_id.as_deref())
    else {
        return None;
    };

    let period_str = format!("{}d", period_days);
    let prev_period_str = format!("{}d", period_days.saturating_mul(2));
    let (current, prev) = tokio::join!(
        plausible::fetch_analytics(api_key, site_id, &period_str),
        plausible::fetch_analytics(api_key, site_id, &prev_period_str),
    );
    let Ok(data) = current else {
        return None;
    };

    let (prev_visitors, prev_pageviews, prev_bounce_rate, prev_visit_duration) =
        if let Ok(prev_data) = prev {
            (
                Some(
                    prev_data
                        .aggregate
                        .visitors
                        .saturating_sub(data.aggregate.visitors),
                ),
                Some(
                    prev_data
                        .aggregate
                        .pageviews
                        .saturating_sub(data.aggregate.pageviews),
                ),
                Some(prev_data.aggregate.bounce_rate),
                Some(prev_data.aggregate.visit_duration),
            )
        } else {
            (None, None, None, None)
        };

    Some(AnalyticsSummary {
        visitors: data.aggregate.visitors,
        pageviews: data.aggregate.pageviews,
        bounce_rate: data.aggregate.bounce_rate,
        visit_duration: data.aggregate.visit_duration,
        prev_visitors,
        prev_pageviews,
        prev_bounce_rate,
        prev_visit_duration,
        top_pages: data
            .top_pages
            .into_iter()
            .take(10)
            .map(|p| TopPageEntry {
                page: p.page,
                visitors: p.visitors,
            })
            .collect(),
        top_sources: data
            .top_sources
            .into_iter()
            .take(10)
            .map(|s| TopSourceEntry {
                source: s.source,
                visitors: s.visitors,
            })
            .collect(),
        daily_visitors: data
            .points
            .into_iter()
            .map(|p| DailyPoint {
                date: p.date,
                value: p.visitors,
            })
            .collect(),
    })
}

#[cfg(feature = "desktop")]
pub(super) async fn fetch_uptime_summary(
    configs: &[IntegrationConfig],
    sections: &SectionConfig,
) -> Option<UptimeSummary> {
    if !sections.uptime {
        return None;
    }

    let config = configs
        .iter()
        .find(|c| c.integration_type == IntegrationType::UptimeRobot)?;
    let api_key = config.api_key.as_deref()?;

    match uptimerobot::fetch_stats(api_key, None).await {
        Ok(data) => data.monitors.first().map(|monitor| UptimeSummary {
            uptime_pct: monitor.uptime_ratio,
            incidents: monitor.logs.iter().filter(|l| l.log_type == 1).count() as u32,
            avg_response_ms: monitor.average_response,
        }),
        Err(_) => None,
    }
}

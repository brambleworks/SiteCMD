use super::analytics::{
    choose_analytics_integration_url, cloudflare_zone_candidates, is_analytics_integration,
    normalize_plausible_site_id, plan_google_fetch, plausible_site_candidates,
    take_enabled_analytics_configs, GoogleFetchPlan,
};
use super::{
    parse_integration_type, require_api_key, take_enabled_integration_config, usable_api_key,
};
use crate::integrations::{IntegrationConfig, IntegrationType};

#[test]
fn configured_google_integration_is_never_silently_skipped() {
    assert_eq!(
        plan_google_fetch(true, false),
        GoogleFetchPlan::Attempt,
        "configured + not cached must attempt (surfaces a reconnect error), regardless of extra",
    );
    assert_eq!(
        plan_google_fetch(true, true),
        GoogleFetchPlan::ServeCached,
        "configured + cached serves the cached payload",
    );
    assert_eq!(
        plan_google_fetch(false, false),
        GoogleFetchPlan::Skip,
        "only an unconfigured integration is skipped",
    );
}

#[test]
fn parse_integration_type_rejects_unknown_values() {
    assert_eq!(
        parse_integration_type("github").expect("known type"),
        IntegrationType::GitHub
    );
    assert!(parse_integration_type("unknown-service").is_err());
}

#[test]
fn take_enabled_integration_config_ignores_unrelated_and_disabled_configs() {
    let configs = vec![
        IntegrationConfig {
            integration_type: IntegrationType::GitHub,
            api_key: Some("gh".into()),
            site_id: None,
            extra: None,
            enabled: true,
        },
        IntegrationConfig {
            integration_type: IntegrationType::Cloudflare,
            api_key: Some("cf".into()),
            site_id: Some("zone".into()),
            extra: None,
            enabled: false,
        },
    ];

    let config = take_enabled_integration_config(configs, &IntegrationType::GitHub)
        .expect("enabled github config");
    assert_eq!(config.integration_type, IntegrationType::GitHub);
    assert!(take_enabled_integration_config(
        vec![IntegrationConfig {
            integration_type: IntegrationType::Cloudflare,
            api_key: Some("cf".into()),
            site_id: Some("zone".into()),
            extra: None,
            enabled: false,
        }],
        &IntegrationType::Cloudflare
    )
    .is_err());
}

#[test]
fn take_enabled_analytics_configs_excludes_unrelated_integrations() {
    let configs = vec![
        IntegrationConfig {
            integration_type: IntegrationType::Plausible,
            api_key: Some("pl".into()),
            site_id: Some("site".into()),
            extra: None,
            enabled: true,
        },
        IntegrationConfig {
            integration_type: IntegrationType::GitHub,
            api_key: Some("gh".into()),
            site_id: Some("owner/repo".into()),
            extra: None,
            enabled: true,
        },
        IntegrationConfig {
            integration_type: IntegrationType::Jira,
            api_key: Some("jira".into()),
            site_id: None,
            extra: None,
            enabled: true,
        },
        IntegrationConfig {
            integration_type: IntegrationType::GoogleAnalytics,
            api_key: None,
            site_id: Some("property".into()),
            extra: Some(serde_json::json!({ "tokens": { "access_token": "secret" } })),
            enabled: true,
        },
    ];

    let filtered = take_enabled_analytics_configs(configs);
    assert_eq!(filtered.len(), 2);
    assert!(filtered
        .iter()
        .all(|config| is_analytics_integration(&config.integration_type)));
    assert!(filtered.iter().all(|config| !matches!(
        config.integration_type,
        IntegrationType::GitHub | IntegrationType::Jira
    )));
}

#[test]
fn plausible_site_candidates_prefers_current_public_environment_host() {
    let candidates = plausible_site_candidates(
        Some("Example.com"),
        Some("Https://sitecmd.com/pricing?ref=test"),
    );

    assert_eq!(candidates, vec!["sitecmd.com", "example.com"]);
}

#[test]
fn plausible_site_candidates_ignores_local_environment_hosts() {
    let candidates = plausible_site_candidates(Some("sitecmd.com"), Some("http://127.0.0.1:4321"));

    assert_eq!(candidates, vec!["sitecmd.com"]);
}

#[test]
fn plausible_site_candidates_ignores_local_configured_site_ids() {
    let candidates = plausible_site_candidates(
        Some("http://localhost:4321"),
        Some("https://production.example.com"),
    );

    assert_eq!(candidates, vec!["production.example.com"]);
    assert!(plausible_site_candidates(Some("localhost"), Some("http://127.0.0.1:4321")).is_empty());
}

#[test]
fn analytics_integration_url_falls_back_from_local_to_public_environment() {
    let project_urls = vec![
        "http://localhost:4321".to_string(),
        "http://127.0.0.1:4321".to_string(),
        "Https://SiteCMD.com".to_string(),
    ];

    assert_eq!(
        choose_analytics_integration_url(Some("http://localhost:4321"), &project_urls),
        Some("https://sitecmd.com/".to_string())
    );
}

#[test]
fn analytics_integration_url_prefers_requested_public_environment() {
    let project_urls = vec!["https://production.example.com".to_string()];

    assert_eq!(
        choose_analytics_integration_url(Some("https://staging.example.com/app"), &project_urls),
        Some("https://staging.example.com/app".to_string())
    );
}

#[test]
fn plausible_site_candidates_keeps_configured_domain_first_for_related_hosts() {
    let candidates =
        plausible_site_candidates(Some("example.com"), Some("https://staging.example.com"));

    assert_eq!(candidates, vec!["example.com", "staging.example.com"]);
}

#[test]
fn normalize_plausible_site_id_accepts_plain_domains_and_urls() {
    assert_eq!(
        normalize_plausible_site_id(" https://Example.COM/docs "),
        Some("example.com".into())
    );
    assert_eq!(
        normalize_plausible_site_id("SITEcmd.com/"),
        Some("sitecmd.com".into())
    );
}

#[test]
fn cloudflare_zone_candidates_prefers_zone_id_and_falls_back_from_localhost() {
    assert_eq!(
        cloudflare_zone_candidates(
            Some("0123456789abcdef0123456789abcdef"),
            Some("https://example.com")
        ),
        vec!["0123456789abcdef0123456789abcdef"]
    );
    assert_eq!(
        cloudflare_zone_candidates(Some("localhost"), Some("https://production.example.com")),
        vec!["production.example.com"]
    );
    assert!(cloudflare_zone_candidates(Some("http://127.0.0.1:3000"), None).is_empty());
}

#[test]
fn usable_api_key_rejects_missing_keyring_placeholder() {
    let config = IntegrationConfig {
        integration_type: IntegrationType::Cloudflare,
        api_key: Some(crate::keyring::KEYRING_PLACEHOLDER.to_string()),
        site_id: Some("zone".into()),
        extra: None,
        enabled: true,
    };

    assert_eq!(usable_api_key(&config), None);
}

#[test]
fn require_api_key_distinguishes_unreadable_from_unset() {
    let unreadable = IntegrationConfig {
        integration_type: IntegrationType::Plausible,
        api_key: Some(crate::keyring::KEYRING_PLACEHOLDER.to_string()),
        site_id: Some("example.com".into()),
        extra: None,
        enabled: true,
    };
    let err = require_api_key(&unreadable).unwrap_err();
    assert!(err.to_lowercase().contains("reconnect"), "got: {err}");
    assert_ne!(err, "No API key configured");

    // No key stored at all -> the genuine "not configured" message.
    let unset = IntegrationConfig {
        integration_type: IntegrationType::Plausible,
        api_key: None,
        site_id: Some("example.com".into()),
        extra: None,
        enabled: true,
    };
    assert_eq!(
        require_api_key(&unset).unwrap_err(),
        "No API key configured"
    );

    // A real key resolves.
    let configured = IntegrationConfig {
        integration_type: IntegrationType::Plausible,
        api_key: Some("real-key-123".into()),
        site_id: Some("example.com".into()),
        extra: None,
        enabled: true,
    };
    assert_eq!(require_api_key(&configured).unwrap(), "real-key-123");
}

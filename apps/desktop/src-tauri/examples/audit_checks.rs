use app_lib::checks::{
    accessibility, compliance, config, performance, security, seo, AsyncCheck, Check, CheckContext,
    CheckStatus,
};

#[tokio::main]
async fn main() {
    let sites = vec![
        "https://example.com",
        "https://github.com",
        "https://stripe.com",
        "https://www.drupal.org",
        "https://wordpress.org",
    ];

    let client = app_lib::http_client::client().clone();

    for site_url in &sites {
        println!("\n{}", "=".repeat(80));
        println!("  SCANNING: {}", site_url);
        println!("{}\n", "=".repeat(80));

        let url = url::Url::parse(site_url).unwrap();

        let response = match client.get(site_url.to_owned()).send().await {
            Ok(r) => r,
            Err(e) => {
                println!("  ❌ FETCH FAILED: {}\n", e);
                continue;
            }
        };

        let status_code = response.status().as_u16();
        let response_headers = response.headers().clone();
        let body =
            match tokio::time::timeout(std::time::Duration::from_secs(15), response.text()).await {
                Ok(Ok(b)) => b,
                Ok(Err(e)) => {
                    println!("  ❌ BODY READ FAILED: {}\n", e);
                    continue;
                }
                Err(_) => {
                    println!("  ❌ BODY READ TIMED OUT\n");
                    continue;
                }
            };

        println!("  Status: {}  Body: {} bytes\n", status_code, body.len());

        let ctx = CheckContext {
            page: app_lib::checks::PageContext {
                evaluation_time: chrono::Utc::now(),
                url: url.clone(),
                response_headers,
                status_code,
                body,
                is_localhost: false,
                is_strict_localhost: false,
                http_version: None,
                body_lower_cache: std::sync::OnceLock::new(),
            },
            client: client.clone(),
            probe_cache: Default::default(),
        };

        let sync_checks: Vec<Box<dyn Check>> = vec![
            security::sync_checks(),
            seo::sync_checks(),
            performance::sync_checks(),
            accessibility::sync_checks(),
            compliance::sync_checks(),
            config::sync_checks(),
        ]
        .into_iter()
        .flatten()
        .collect();

        let async_checks: Vec<Box<dyn AsyncCheck>> = vec![
            security::async_checks(),
            seo::async_checks(),
            performance::async_checks(),
            compliance::async_checks(),
            config::async_checks(),
        ]
        .into_iter()
        .flatten()
        .collect();

        let mut all_results = Vec::new();

        for check in &sync_checks {
            let results =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check.run(&ctx)))
                    .unwrap_or_else(|_| {
                        println!("  💥 PANIC in sync check: {}", check.id());
                        vec![]
                    });
            all_results.extend(results);
        }

        for check in &async_checks {
            let results = tokio::time::timeout(std::time::Duration::from_secs(15), check.run(&ctx))
                .await
                .unwrap_or_else(|_| {
                    println!("  ⏱️  TIMEOUT on async check: {}", check.id());
                    vec![]
                });
            all_results.extend(results);
        }

        let mut by_category: std::collections::BTreeMap<String, Vec<_>> =
            std::collections::BTreeMap::new();
        for r in &all_results {
            by_category
                .entry(format!("{:?}", r.category))
                .or_default()
                .push(r);
        }

        for (cat, results) in &by_category {
            println!("  ── {} ({} checks) ──", cat, results.len());
            for r in results {
                let icon = match r.status {
                    CheckStatus::Pass => "✅",
                    CheckStatus::Fail => "❌",
                    CheckStatus::Warn => "⚠️ ",
                    CheckStatus::Skipped => "⏭️ ",
                };
                let sev = format!("{:?}", r.severity);
                println!(
                    "    {} [{}] {} - {}",
                    icon,
                    sev,
                    r.check_id,
                    truncate(&r.description, 100)
                );
                if r.status != CheckStatus::Pass {
                    if let Some(fix) = &r.manual_fix {
                        println!("       Fix: {}", truncate(fix, 90));
                    }
                }
            }
            println!();
        }

        let pass = all_results
            .iter()
            .filter(|r| r.status == CheckStatus::Pass)
            .count();
        let fail = all_results
            .iter()
            .filter(|r| r.status == CheckStatus::Fail)
            .count();
        let warn = all_results
            .iter()
            .filter(|r| r.status == CheckStatus::Warn)
            .count();
        let skip = all_results
            .iter()
            .filter(|r| r.status == CheckStatus::Skipped)
            .count();
        println!(
            "  TOTAL: {} checks - ✅ {} pass, ❌ {} fail, ⚠️  {} warn, ⏭️  {} skip\n",
            all_results.len(),
            pass,
            fail,
            warn,
            skip
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max])
    } else {
        s.to_string()
    }
}

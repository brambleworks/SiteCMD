//! Project detection helpers for local site folders.

use std::collections::HashSet;
use std::path::Path;

mod ecosystems;
mod environments;
mod helpers;
mod types;

use ecosystems::{
    detect_drupal, detect_framework_in_subpackages, detect_laravel, detect_node, detect_python,
    detect_static_generators, detect_wordpress,
};
use environments::{detect_env_urls, detect_hosting_configs, detect_local_dev_env};
pub use types::{DetectedUrl, ProjectInfo};

fn is_loopback_host(host: &str) -> bool {
    matches!(
        host.to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "0.0.0.0" | "::1" | "[::1]"
    )
}

/// Canonical URL identity shared by project detection and environment deduplication.
pub fn url_identity_key(raw: &str) -> String {
    match url::Url::parse(raw) {
        Ok(parsed) => {
            let host = parsed
                .host_str()
                .map(|host| {
                    if is_loopback_host(host) {
                        "localhost".to_string()
                    } else {
                        host.to_ascii_lowercase()
                    }
                })
                .unwrap_or_default();
            let port = parsed
                .port()
                .map(|port| format!(":{}", port))
                .unwrap_or_default();
            let path = {
                let trimmed = parsed.path().trim_end_matches('/');
                if trimmed.is_empty() {
                    "/"
                } else {
                    trimmed
                }
            };
            let query = parsed
                .query()
                .map(|q| format!("?{}", q))
                .unwrap_or_default();
            format!(
                "{}://{}{}{}{}",
                parsed.scheme().to_ascii_lowercase(),
                host,
                port,
                path,
                query
            )
        }
        Err(_) => raw.trim().trim_end_matches('/').to_ascii_lowercase(),
    }
}

fn normalize_detected_urls(urls: Vec<DetectedUrl>) -> Vec<DetectedUrl> {
    let mut seen = HashSet::new();
    urls.into_iter()
        .filter_map(|mut detected| {
            detected.environment = crate::core::localhost::resolve_environment_name(
                &detected.url,
                Some(&detected.environment),
            )
            .to_string();
            let key = url_identity_key(&detected.url);
            if seen.insert(key) {
                Some(detected)
            } else {
                None
            }
        })
        .collect()
}

/// Scan a project directory for config files and detect URLs + framework
#[tracing::instrument(skip(dir))]
pub fn detect_project(dir: &Path) -> ProjectInfo {
    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let mut urls = Vec::new();
    let mut framework = None;

    detect_drupal(dir, &mut urls, &mut framework);
    detect_wordpress(dir, &mut urls, &mut framework);
    detect_laravel(dir, &mut urls, &mut framework);
    detect_node(dir, &mut urls, &mut framework);
    detect_python(dir, &mut framework);
    detect_static_generators(dir, &mut urls, &mut framework);

    detect_local_dev_env(dir, &mut urls, &mut framework);
    detect_env_urls(dir, &mut urls);
    detect_hosting_configs(dir, &mut urls, &mut framework);

    // Monorepo fallback: the framework often lives in a sub-package (packages/web,
    // apps/web) rather than the workspace root. Only runs when nothing matched
    // above, and only fills the framework field.
    detect_framework_in_subpackages(dir, &mut framework);

    let urls = normalize_detected_urls(urls);

    ProjectInfo {
        path: dir.to_string_lossy().to_string(),
        name,
        urls,
        framework,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn url_identity_key_folds_loopback_and_normalizes() {
        // All loopback aliases collapse to one key so project detection and
        // environment dedupe agree on identity.
        let k = url_identity_key("http://127.0.0.1:4321/");
        assert_eq!(k, "http://localhost:4321/");
        assert_eq!(url_identity_key("http://localhost:4321"), k);
        assert_eq!(url_identity_key("http://0.0.0.0:4321/"), k);
        assert_eq!(url_identity_key("http://[::1]:4321"), k);

        // Scheme + host lowercased; path case preserved; trailing slash trimmed.
        assert_eq!(
            url_identity_key("HTTPS://Example.COM/Path/"),
            "https://example.com/Path"
        );
        // Empty path normalizes to "/"; query is kept verbatim.
        assert_eq!(
            url_identity_key("https://example.com"),
            "https://example.com/"
        );
        assert_eq!(
            url_identity_key("https://example.com/?a=1"),
            "https://example.com/?a=1"
        );

        // Unparseable input falls back to a trimmed, lowercased raw string.
        assert_eq!(url_identity_key("  NOT A URL/  "), "not a url");
    }

    #[test]
    fn detects_framework_in_monorepo_subpackage() {
        let tmp = std::env::temp_dir().join("shk_test_monorepo_framework");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("apps/web")).unwrap();
        // Root is just a workspace manifest with no framework dependency.
        fs::write(
            tmp.join("package.json"),
            r#"{"name":"root","private":true,"workspaces":["apps/*"]}"#,
        )
        .unwrap();
        fs::write(
            tmp.join("apps/web/package.json"),
            r#"{"name":"web","dependencies":{"next":"^15.0.0"}}"#,
        )
        .unwrap();

        let info = detect_project(&tmp);
        assert_eq!(info.framework, Some("Next.js".into()));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn detects_drupal_module_repo_in_subdir() {
        let tmp = std::env::temp_dir().join("shk_test_drupal_module_repo");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("geo_optimizer")).unwrap();
        // Root composer.json is only PHP linting tooling under require-dev.
        fs::write(
            tmp.join("composer.json"),
            r#"{"require-dev":{"drupal/coder":"^8.3"}}"#,
        )
        .unwrap();
        fs::write(
            tmp.join("geo_optimizer/composer.json"),
            r#"{"name":"drupal/geo_optimizer","type":"drupal-module","require":{"drupal/core":"^10.3 || ^11"}}"#,
        )
        .unwrap();
        fs::write(
            tmp.join("geo_optimizer/geo_optimizer.info.yml"),
            "name: Geo Optimizer\ntype: module\ncore_version_requirement: ^10.3 || ^11\n",
        )
        .unwrap();

        let info = detect_project(&tmp);
        assert_eq!(info.framework, Some("Drupal".into()));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_detect_drupal_with_ddev() {
        let tmp = std::env::temp_dir().join("shk_test_drupal_ddev");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("web/core/lib")).unwrap();
        fs::create_dir_all(tmp.join("web/sites/default")).unwrap();
        fs::create_dir_all(tmp.join(".ddev")).unwrap();

        fs::write(tmp.join("web/core/lib/Drupal.php"), "<?php\n").unwrap();
        fs::write(
            tmp.join("web/sites/default/settings.php"),
            "<?php\n$settings['base_url'] = 'https://mysite.com';\n",
        )
        .unwrap();
        fs::write(
            tmp.join(".ddev/config.yaml"),
            "name: mysite\ntype: drupal\n",
        )
        .unwrap();
        fs::write(
            tmp.join("composer.json"),
            r#"{"require":{"drupal/core":"^10"}}"#,
        )
        .unwrap();
        fs::write(tmp.join(".env"), "DRUSH_OPTIONS_URI=https://mysite.com\n").unwrap();

        let info = detect_project(&tmp);
        assert_eq!(info.framework, Some("Drupal".into()));
        // The DDEV hostname resolves to loopback, so normalization has to keep
        // the label the config file declared instead of reading it as a
        // second production site.
        assert!(
            info.urls
                .iter()
                .any(|u| u.url.contains("mysite.ddev.site") && u.environment == "local"),
            "Expected a local DDEV URL, got: {:?}",
            info.urls
        );
        assert!(
            info.urls.iter().any(|u| u.url == "https://mysite.com"),
            "Expected prod URL, got: {:?}",
            info.urls
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_detect_nextjs() {
        let tmp = std::env::temp_dir().join("shk_test_nextjs2");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        fs::write(
            tmp.join("package.json"),
            r#"{"dependencies":{"next":"14.0.0","react":"18.0.0"},"scripts":{"dev":"next dev"}}"#,
        )
        .unwrap();
        fs::write(
            tmp.join(".env.production"),
            "NEXT_PUBLIC_SITE_URL=https://myapp.vercel.app\n",
        )
        .unwrap();

        let info = detect_project(&tmp);
        assert_eq!(info.framework, Some("Next.js".into()));
        assert!(
            info.urls.iter().any(|u| u.url == "http://localhost:3000"),
            "Expected localhost, got: {:?}",
            info.urls
        );
        assert!(
            info.urls
                .iter()
                .any(|u| u.url == "https://myapp.vercel.app"),
            "Expected prod URL, got: {:?}",
            info.urls
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_detect_project_dedupes_loopback_aliases_and_corrects_environment() {
        let tmp = std::env::temp_dir().join("shk_test_loopback_aliases");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        fs::write(
            tmp.join("package.json"),
            r#"{"dependencies":{"astro":"^5.0.0"},"homepage":"http://127.0.0.1:4321/","scripts":{"dev":"astro dev"}}"#,
        )
        .unwrap();

        let info = detect_project(&tmp);
        let loopback_urls: Vec<_> = info
            .urls
            .iter()
            .filter(|url| url.url.contains("127.0.0.1:4321") || url.url.contains("localhost:4321"))
            .collect();

        assert_eq!(
            loopback_urls.len(),
            1,
            "Expected one loopback URL, got: {:?}",
            info.urls
        );
        assert_eq!(loopback_urls[0].environment, "local");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_detect_wordpress() {
        let tmp = std::env::temp_dir().join("shk_test_wp");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        fs::write(tmp.join("wp-config.php"),
            "<?php\ndefine('WP_HOME', 'https://myblog.com');\ndefine('WP_SITEURL', 'https://myblog.com');\n").unwrap();
        fs::create_dir_all(tmp.join("wp-content")).unwrap();

        let info = detect_project(&tmp);
        assert_eq!(info.framework, Some("WordPress".into()));
        assert!(
            info.urls.iter().any(|u| u.url == "https://myblog.com"),
            "Expected WP URL, got: {:?}",
            info.urls
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[cfg(unix)]
    #[test]
    fn project_detection_ignores_symlinked_metadata_and_subpackages() {
        use std::os::unix::fs::symlink;

        let project = tempfile::tempdir().expect("project");
        let outside = tempfile::tempdir().expect("outside");
        fs::write(
            outside.path().join(".env"),
            "APP_URL=https://private.example\n",
        )
        .expect("write outside env");
        fs::write(
            outside.path().join("package.json"),
            r#"{"dependencies":{"next":"15.0.0"}}"#,
        )
        .expect("write outside manifest");
        fs::create_dir_all(project.path().join("apps")).expect("create apps");
        symlink(outside.path().join(".env"), project.path().join(".env"))
            .expect("link outside env");
        symlink(outside.path(), project.path().join("apps/linked")).expect("link outside package");

        let info = detect_project(project.path());

        assert!(info
            .urls
            .iter()
            .all(|url| url.url != "https://private.example"));
        assert_ne!(info.framework.as_deref(), Some("Next.js"));
    }
}

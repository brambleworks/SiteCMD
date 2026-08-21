use std::path::Path;

use super::helpers::{extract_toml_string, parse_yaml_urls, read_project_text};
use super::DetectedUrl;

// Local dev environments (DDEV, Lando, Docksal, Docker Compose)
pub(super) fn detect_local_dev_env(
    dir: &Path,
    urls: &mut Vec<DetectedUrl>,
    _framework: &mut Option<String>,
) {
    // DDEV
    if let Some(content) = read_project_text(dir, ".ddev/config.yaml") {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("name:") {
                let project_name = trimmed
                    .trim_start_matches("name:")
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                if !project_name.is_empty() {
                    urls.push(DetectedUrl {
                        url: format!("https://{}.ddev.site", project_name),
                        environment: "local".into(),
                        source: ".ddev/config.yaml".into(),
                    });
                }
            }
            // Additional hostnames
            if trimmed.starts_with("additional_fqdns:") || trimmed.starts_with("- ") {
                let val = trimmed
                    .trim_start_matches("- ")
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                if val.contains('.') && !val.starts_with('#') {
                    let url = if val.starts_with("http") {
                        val.to_string()
                    } else {
                        format!("https://{}", val)
                    };
                    urls.push(DetectedUrl {
                        url,
                        environment: "local".into(),
                        source: ".ddev/config.yaml".into(),
                    });
                }
            }
        }
    }

    // Lando
    if let Some(content) = read_project_text(dir, ".lando.yml") {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("name:") {
                let name = trimmed
                    .trim_start_matches("name:")
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                if !name.is_empty() {
                    urls.push(DetectedUrl {
                        url: format!("https://{}.lndo.site", name),
                        environment: "local".into(),
                        source: ".lando.yml".into(),
                    });
                }
            }
        }
    }
    // Also check.lando.local.yml for overrides
    if let Some(content) = read_project_text(dir, ".lando.local.yml") {
        for line in content.lines() {
            if line.trim().starts_with("name:") {
                let name = line
                    .trim()
                    .trim_start_matches("name:")
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                if !name.is_empty() {
                    urls.push(DetectedUrl {
                        url: format!("https://{}.lndo.site", name),
                        environment: "local".into(),
                        source: ".lando.local.yml".into(),
                    });
                }
            }
        }
    }

    // Docksal
    if let Some(content) = read_project_text(dir, ".docksal/docksal.env") {
        for line in content.lines() {
            if line.starts_with("VIRTUAL_HOST=") {
                let host = line
                    .trim_start_matches("VIRTUAL_HOST=")
                    .trim()
                    .trim_matches('"');
                if !host.is_empty() {
                    urls.push(DetectedUrl {
                        url: format!("http://{}", host),
                        environment: "local".into(),
                        source: ".docksal/docksal.env".into(),
                    });
                }
            }
        }
    }

    // Docker Compose - check for port mappings
    for compose_file in &[
        "docker-compose.yml",
        "docker-compose.yaml",
        "compose.yml",
        "compose.yaml",
    ] {
        if let Some(content) = read_project_text(dir, compose_file) {
            for line in content.lines() {
                let trimmed = line.trim();
                // Look for port mappings like "8080:80" or "- 3000:3000"
                if trimmed.contains("ports:")
                    || (trimmed.starts_with("- ") && trimmed.contains(':'))
                {
                    let val = trimmed
                        .trim_start_matches("- ")
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    if let Some(host_port) = val.split(':').next() {
                        if let Ok(port) = host_port.trim().parse::<u16>() {
                            if port >= 80 {
                                let url = format!("http://localhost:{}", port);
                                if !urls.iter().any(|u| u.url == url) {
                                    urls.push(DetectedUrl {
                                        url,
                                        environment: "local".into(),
                                        source: compose_file.to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn detect_env_urls(dir: &Path, urls: &mut Vec<DetectedUrl>) {
    let env_files = [
        ".env",
        ".env.local",
        ".env.development",
        ".env.dev",
        ".env.staging",
        ".env.production",
        ".env.prod",
    ];

    let url_keys: &[&str] = &[
        // Universal
        "SITE_URL",
        "APP_URL",
        "BASE_URL",
        "PUBLIC_URL",
        "DEPLOY_URL",
        // Node / Vite / Next / Nuxt
        "NEXT_PUBLIC_SITE_URL",
        "NEXT_PUBLIC_URL",
        "NEXT_PUBLIC_BASE_URL",
        "NEXT_PUBLIC_APP_URL",
        "VITE_APP_URL",
        "VITE_BASE_URL",
        "VITE_SITE_URL",
        "VITE_PUBLIC_URL",
        "NUXT_PUBLIC_SITE_URL",
        // WordPress
        "WP_HOME",
        "WP_SITEURL",
        // Drupal
        "DRUSH_OPTIONS_URI",
        // Hosting
        "VERCEL_URL",
        "NETLIFY_URL",
        "CF_PAGES_URL",
        "HEROKU_APP_NAME",
    ];

    for env_file in &env_files {
        let content = match read_project_text(dir, env_file) {
            Some(content) => content,
            None => continue,
        };

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || !line.contains('=') {
                continue;
            }
            let (key, value) = match line.split_once('=') {
                Some((k, v)) => (k.trim(), v.trim().trim_matches('"').trim_matches('\'')),
                None => continue,
            };

            let is_url_key = url_keys.iter().any(|k| key.eq_ignore_ascii_case(k));
            if !is_url_key {
                continue;
            }

            // Value might be a URL or just a hostname
            let url = if value.starts_with("http") {
                value.to_string()
            } else if !value.is_empty() && value.contains('.') {
                format!("https://{}", value)
            } else {
                continue;
            };

            let environment = if env_file.contains("prod") {
                "production"
            } else if env_file.contains("staging") {
                "staging"
            } else if env_file.contains("dev") {
                "development"
            } else if env_file.contains("local") {
                "local"
            } else {
                "production"
            };

            urls.push(DetectedUrl {
                url,
                environment: environment.into(),
                source: format!("{} ({})", env_file, key),
            });
        }
    }
}

// Hosting platform configs
pub(super) fn detect_hosting_configs(
    dir: &Path,
    urls: &mut Vec<DetectedUrl>,
    framework: &mut Option<String>,
) {
    if dir.join("vercel.json").exists() && framework.is_none() {
        *framework = Some("Vercel".into());
    }

    if let Some(content) = read_project_text(dir, "netlify.toml") {
        if framework.is_none() {
            *framework = Some("Netlify".into());
        }
        for line in content.lines() {
            let t = line.trim().to_lowercase();
            if (t.starts_with("url") || t.starts_with("deploy_url")) && line.contains('"') {
                if let Some(url) = extract_toml_string(line) {
                    urls.push(DetectedUrl {
                        url,
                        environment: "production".into(),
                        source: "netlify.toml".into(),
                    });
                }
            }
        }
    }

    if let Some(content) = read_project_text(dir, "wrangler.toml") {
        if framework.is_none() {
            *framework = Some("Cloudflare".into());
        }
        for line in content.lines() {
            if line.trim().starts_with("route") {
                if let Some(url) = extract_toml_string(line) {
                    urls.push(DetectedUrl {
                        url,
                        environment: "production".into(),
                        source: "wrangler.toml".into(),
                    });
                }
            }
        }
    }

    // Pantheon
    if dir.join("pantheon.yml").exists() {
        if let Some(content) = read_project_text(dir, "pantheon.yml") {
            for line in content.lines() {
                if line.trim().starts_with("site:") || line.trim().starts_with("site_name:") {
                    let name = line
                        .split(':')
                        .nth(1)
                        .unwrap_or("")
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'');
                    if !name.is_empty() {
                        urls.push(DetectedUrl {
                            url: format!("https://live-{}.pantheonsite.io", name),
                            environment: "production".into(),
                            source: "pantheon.yml".into(),
                        });
                        urls.push(DetectedUrl {
                            url: format!("https://dev-{}.pantheonsite.io", name),
                            environment: "development".into(),
                            source: "pantheon.yml".into(),
                        });
                    }
                }
            }
        }
    }

    // Acquia
    if dir.join("acquia.json").exists() || dir.join("blt/blt.yml").exists() {
        // BLT config often has multisites
        if let Some(content) = read_project_text(dir, "blt/blt.yml") {
            parse_yaml_urls(&content, "blt/blt.yml", urls);
        }
    }

    // Platform.sh
    if dir.join(".platform.app.yaml").exists() || dir.join(".platform/routes.yaml").exists() {
        if let Some(content) = read_project_text(dir, ".platform/routes.yaml") {
            parse_yaml_urls(&content, ".platform/routes.yaml", urls);
        }
    }
}

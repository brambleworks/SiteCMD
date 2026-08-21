use std::path::Path;

use super::helpers::{
    extract_php_url, extract_port, extract_toml_string, is_safe_project_directory, parse_yaml_urls,
    read_json, read_project_text,
};
use super::DetectedUrl;

// Drupal
pub(super) fn detect_drupal(
    dir: &Path,
    urls: &mut Vec<DetectedUrl>,
    framework: &mut Option<String>,
) {
    let webroot = if dir.join("web/core/lib/Drupal.php").exists() {
        Some("web")
    } else if dir.join("core/lib/Drupal.php").exists() {
        Some(".")
    } else if dir.join("docroot/core/lib/Drupal.php").exists() {
        Some("docroot")
    } else {
        None
    };

    let composer_drupal = read_json(dir, "composer.json")
        .and_then(|c| c.get("require")?.as_object().cloned())
        .map(|r| r.keys().any(|k| k.starts_with("drupal/")))
        .unwrap_or(false);

    // Drupal extensions may live at the root or one level down without a webroot;
    // recognize their Composer type or `*.info.yml` core requirement.
    let drupal_extension = is_drupal_extension(dir) || subdir_is_drupal_extension(dir);

    if webroot.is_none() && !composer_drupal && !drupal_extension {
        return;
    }

    *framework = Some("Drupal".into());

    let webroot_dir = webroot.unwrap_or("web");

    // settings.php - look for $base_url assignment (not comments)
    for path in &[
        format!("{}/sites/default/settings.php", webroot_dir),
        format!("{}/sites/default/settings.local.php", webroot_dir),
    ] {
        if let Some(content) = read_project_text(dir, path) {
            for line in content.lines() {
                let trimmed = line.trim();
                // Skip PHP comments
                if trimmed.starts_with('#')
                    || trimmed.starts_with("//")
                    || trimmed.starts_with('*')
                    || trimmed.starts_with("/*")
                {
                    continue;
                }
                // Skip commented-out code (lines starting with # or //)
                // Also skip lines that are just documentation examples
                if trimmed.contains("example.com") || trimmed.contains("example.org") {
                    continue;
                }
                // Match actual $base_url assignment: $base_url = 'https://...';
                if trimmed.starts_with("$base_url") && trimmed.contains('=') {
                    if let Some(url) = extract_php_url(trimmed) {
                        urls.push(DetectedUrl {
                            url,
                            environment: if path.contains("local") {
                                "local"
                            } else {
                                "production"
                            }
                            .into(),
                            source: path.clone(),
                        });
                    }
                }
            }
        }
    }

    // Drush site aliases
    for alias_file in &[
        "drush/sites/self.site.yml",
        "drush/sites/default.site.yml",
        ".drush/sites/self.site.yml",
    ] {
        if let Some(content) = read_project_text(dir, alias_file) {
            parse_yaml_urls(&content, alias_file, urls);
        }
    }

    // sites.php - multisite
    let sites_php = format!("{}/sites/sites.php", webroot_dir);
    if let Some(content) = read_project_text(dir, &sites_php) {
        for line in content.lines() {
            if let Some(url) = extract_php_url(line) {
                urls.push(DetectedUrl {
                    url,
                    environment: "production".into(),
                    source: sites_php.clone(),
                });
            }
        }
    }
}

/// A Drupal module/theme/profile repo, identified by a composer `type: drupal-*`
/// or a `*.info.yml` manifest carrying a Drupal core requirement.
fn is_drupal_extension(dir: &Path) -> bool {
    if let Some(composer) = read_json(dir, "composer.json") {
        if let Some(kind) = composer.get("type").and_then(|v| v.as_str()) {
            if kind.starts_with("drupal-") {
                return true;
            }
        }
    }
    dir_has_drupal_info_yml(dir)
}

/// Whether `dir` contains a Drupal extension manifest.
fn dir_has_drupal_info_yml(dir: &Path) -> bool {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_info_yml = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with(".info.yml"))
            .unwrap_or(false);
        if !is_info_yml {
            continue;
        }
        let Ok(relative_path) = path.strip_prefix(dir) else {
            continue;
        };
        if let Some(content) = read_project_text(dir, relative_path) {
            if content.contains("core_version_requirement") || content.contains("core:") {
                return true;
            }
        }
    }
    false
}

/// Contrib repos place the extension one directory down
/// (`my_module/my_module.info.yml`). Check immediate subdirectories, skipping
/// dependency and VCS directories, and cap the work for large trees.
fn subdir_is_drupal_extension(dir: &Path) -> bool {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return false,
    };
    entries
        .flatten()
        .filter(|entry| is_safe_project_directory(dir, &entry.path()))
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| !matches!(name, "vendor" | "node_modules") && !name.starts_with('.'))
                .unwrap_or(false)
        })
        .take(64)
        .any(|entry| is_drupal_extension(&entry.path()))
}

// WordPress
pub(super) fn detect_wordpress(
    dir: &Path,
    urls: &mut Vec<DetectedUrl>,
    framework: &mut Option<String>,
) {
    if framework.is_some() {
        return;
    }

    let is_wp = dir.join("wp-config.php").exists()
        || dir.join("wp-content").exists()
        || dir.join("wp-includes").exists();
    if !is_wp {
        return;
    }

    *framework = Some("WordPress".into());

    if let Some(content) = read_project_text(dir, "wp-config.php") {
        for line in content.lines() {
            let trimmed = line.trim();
            // Skip comments
            if trimmed.starts_with('#')
                || trimmed.starts_with("//")
                || trimmed.starts_with('*')
                || trimmed.starts_with("/*")
            {
                continue;
            }
            if trimmed.contains("example.com") || trimmed.contains("example.org") {
                continue;
            }
            if trimmed.contains("WP_HOME") || trimmed.contains("WP_SITEURL") {
                if let Some(url) = extract_php_url(trimmed) {
                    urls.push(DetectedUrl {
                        url,
                        environment: "production".into(),
                        source: "wp-config.php".into(),
                    });
                }
            }
        }
    }
}

// Laravel
pub(super) fn detect_laravel(
    dir: &Path,
    _urls: &mut Vec<DetectedUrl>,
    framework: &mut Option<String>,
) {
    if framework.is_some() {
        return;
    }

    let is_laravel = dir.join("artisan").exists()
        && (dir.join("app/Http").exists() || dir.join("app/Providers").exists());
    if !is_laravel {
        return;
    }

    *framework = Some("Laravel".into());
    // `APP_URL` from `.env` is handled by `detect_env_urls`.
}

// Python framework detection does not infer URLs from conventional ports.
pub(super) fn detect_python(dir: &Path, framework: &mut Option<String>) {
    if framework.is_some() {
        return;
    }

    if dir.join("manage.py").exists() {
        *framework = Some("Django".into());
    } else if dir.join("requirements.txt").exists() || dir.join("pyproject.toml").exists() {
        // Check for Flask/FastAPI in requirements
        let content = read_project_text(dir, "requirements.txt").unwrap_or_default()
            + &read_project_text(dir, "pyproject.toml").unwrap_or_default();
        let lower = content.to_lowercase();
        if lower.contains("flask") {
            *framework = Some("Flask".into());
        } else if lower.contains("fastapi") {
            *framework = Some("FastAPI".into());
        }
    }
}

/// Detect a framework in common workspace subpackages without importing their URLs.
pub(super) fn detect_framework_in_subpackages(dir: &Path, framework: &mut Option<String>) {
    if framework.is_some() {
        return;
    }
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    for workspace in ["apps", "packages"] {
        if let Ok(entries) = std::fs::read_dir(dir.join(workspace)) {
            for entry in entries.flatten() {
                if is_safe_project_directory(dir, &entry.path()) {
                    candidates.push(entry.path());
                }
            }
        }
    }
    for name in ["frontend", "web", "client", "app", "server", "backend"] {
        let candidate = dir.join(name);
        if is_safe_project_directory(dir, &candidate) {
            candidates.push(candidate);
        }
    }

    let mut throwaway_urls = Vec::new();
    for candidate in candidates.into_iter().take(32) {
        if framework.is_some() {
            break;
        }
        detect_node(&candidate, &mut throwaway_urls, framework);
        if framework.is_none() {
            detect_static_generators(&candidate, &mut throwaway_urls, framework);
        }
    }
}

pub(super) fn detect_node(dir: &Path, urls: &mut Vec<DetectedUrl>, framework: &mut Option<String>) {
    let pkg = match read_json(dir, "package.json") {
        Some(p) => p,
        None => return,
    };

    if framework.is_none() {
        if let Some(deps) = pkg.get("dependencies").and_then(|d| d.as_object()) {
            let fw = if deps.contains_key("next") {
                "Next.js"
            } else if deps.contains_key("nuxt") || deps.contains_key("nuxt3") {
                "Nuxt"
            } else if deps.contains_key("@sveltejs/kit") {
                "SvelteKit"
            } else if deps.contains_key("svelte") {
                "Svelte"
            } else if deps.contains_key("gatsby") {
                "Gatsby"
            } else if deps.contains_key("astro") {
                "Astro"
            } else if deps.contains_key("@remix-run/node") || deps.contains_key("remix") {
                "Remix"
            } else if deps.contains_key("@angular/core") {
                "Angular"
            } else if deps.contains_key("react") {
                "React"
            } else if deps.contains_key("vue") {
                "Vue"
            } else if deps.contains_key("express") {
                "Express"
            } else {
                ""
            };
            if !fw.is_empty() {
                *framework = Some(fw.into());
            }
        }
    }

    // homepage field → production URL
    if let Some(hp) = pkg.get("homepage").and_then(|v| v.as_str()) {
        if hp.starts_with("http") {
            urls.push(DetectedUrl {
                url: hp.into(),
                environment: "production".into(),
                source: "package.json (homepage)".into(),
            });
        }
    }

    // Dev script → local URL
    if let Some(scripts) = pkg.get("scripts").and_then(|s| s.as_object()) {
        for key in &["dev", "start", "serve"] {
            if let Some(cmd) = scripts.get(*key).and_then(|v| v.as_str()) {
                if let Some(port) = extract_port(cmd) {
                    if !urls.iter().any(|u| u.environment == "local") {
                        urls.push(DetectedUrl {
                            url: format!("http://localhost:{}", port),
                            environment: "local".into(),
                            source: format!("package.json ({} script)", key),
                        });
                    }
                }
            }
        }
    }
}

// Static Site Generators (Hugo, Jekyll, Eleventy)
pub(super) fn detect_static_generators(
    dir: &Path,
    urls: &mut Vec<DetectedUrl>,
    framework: &mut Option<String>,
) {
    if framework.is_some() {
        return;
    }

    // Hugo
    for cfg in &["hugo.toml", "hugo.yaml", "config.toml"] {
        if let Some(content) = read_project_text(dir, cfg) {
            if content.contains("baseURL") || content.contains("baseurl") {
                *framework = Some("Hugo".into());
                for line in content.lines() {
                    if line.trim().to_lowercase().starts_with("baseurl") {
                        if let Some(url) = extract_toml_string(line) {
                            urls.push(DetectedUrl {
                                url,
                                environment: "production".into(),
                                source: cfg.to_string(),
                            });
                        }
                    }
                }
                return;
            }
        }
    }

    // Jekyll
    if dir.join("_config.yml").exists()
        && (dir.join("_layouts").exists() || dir.join("_posts").exists())
    {
        *framework = Some("Jekyll".into());
        if let Some(content) = read_project_text(dir, "_config.yml") {
            for line in content.lines() {
                if line.trim().starts_with("url:") {
                    let val = line
                        .split(':')
                        .skip(1)
                        .collect::<Vec<_>>()
                        .join(":")
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string();
                    if val.starts_with("http") {
                        urls.push(DetectedUrl {
                            url: val,
                            environment: "production".into(),
                            source: "_config.yml".into(),
                        });
                    }
                }
            }
        }
    }

    // Eleventy
    if dir.join(".eleventy.js").exists() || dir.join("eleventy.config.js").exists() {
        *framework = Some("Eleventy".into());
    }
}

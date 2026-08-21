use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::{DesktopWatchRequest, DesktopWatchSignal};

struct WatchSpec {
    kind: &'static str,
    page: &'static str,
    focus: Option<&'static str>,
    title: &'static str,
    detail: &'static str,
    relative_paths: &'static [&'static str],
}

const fn watch_spec(
    kind: &'static str,
    page: &'static str,
    focus: Option<&'static str>,
    title: &'static str,
    detail: &'static str,
    relative_paths: &'static [&'static str],
) -> WatchSpec {
    WatchSpec {
        kind,
        page,
        focus,
        title,
        detail,
        relative_paths,
    }
}

const DEPENDENCY_WATCH_PATHS: &[&str] = &[
    "package.json",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lockb",
    "composer.json",
    "composer.lock",
    "Cargo.toml",
    "Cargo.lock",
    "go.mod",
    "go.sum",
    "requirements.txt",
    "pyproject.toml",
    "Gemfile",
    "Gemfile.lock",
];

const ROBOTS_WATCH_PATHS: &[&str] = &[
    "robots.txt",
    "public/robots.txt",
    "app/robots.*",
    "src/app/robots.*",
    "app/routes/robots.txt.*",
    "src/pages/robots.txt.*",
    "server/routes/robots.txt.*",
    "src/routes/robots.txt/+server.*",
];

const SITEMAP_WATCH_PATHS: &[&str] = &[
    "sitemap.xml",
    "public/sitemap.xml",
    "app/sitemap.*",
    "src/app/sitemap.*",
    "app/routes/sitemap.xml.*",
    "src/pages/sitemap.xml.*",
    "src/pages/sitemap-index.xml.*",
    "server/routes/sitemap.xml.*",
    "src/routes/sitemap.xml/+server.*",
    "next-sitemap.config.*",
];

const LAUNCH_CONFIG_WATCH_PATHS: &[&str] = &[
    "astro.config.*",
    "next.config.*",
    "nuxt.config.*",
    "nitro.config.*",
    "remix.config.*",
    "svelte.config.*",
    "vite.config.*",
    "serverless.y*ml",
    "apphosting.yaml",
    "ecosystem.config.*",
    "vercel.json",
    "netlify.toml",
    "wrangler.toml",
    "fly.toml",
    "railway.json",
    "firebase.json",
    "amplify.yml",
    "app.yaml",
    "render.y*ml",
    "Dockerfile",
    "docker-compose.y*ml",
    "compose.y*ml",
    "Procfile",
    ".env",
    ".env.local",
    ".env.production",
];

const SECURITY_HEADERS_WATCH_PATHS: &[&str] = &[
    "_headers",
    "public/_headers",
    "static/_headers",
    "nginx.conf",
    "Caddyfile",
    ".htaccess",
    "apache2.conf",
    "httpd.conf",
    "haproxy.cfg",
    "traefik.y*ml",
    "traefik.toml",
    "middleware.*",
    "_middleware.*",
    "pages/_middleware.*",
    "src/pages/_middleware.*",
    "src/middleware.*",
    "src/hooks.server.*",
];

const AUTH_SESSION_WATCH_PATHS: &[&str] = &[
    "auth.ts",
    "auth.config.*",
    "auth.js",
    "src/auth.ts",
    "src/auth.js",
    "src/auth.config.*",
    "lib/auth.*",
    "src/lib/auth.*",
    "src/lib/server/auth.*",
    "src/lib/server/session.*",
    "server/auth.*",
    "server/utils/auth.*",
    "app/session.server.*",
    "app/utils/session.server.*",
    "config/auth.php",
    "config/session.php",
    "app/api/auth/[...nextauth]/route.*",
    "src/app/api/auth/[...nextauth]/route.*",
    "pages/api/auth/[...nextauth].*",
    "src/pages/api/auth/[...nextauth].*",
];

const AUTH_GUARD_WATCH_PATHS: &[&str] = &[
    "middleware/auth.*",
    "src/middleware/auth.*",
    "app/middleware/auth.*",
    "server/middleware/auth.*",
    "server/guards/auth.*",
    "src/server/guards/auth.*",
    "lib/guard.*",
    "lib/guards.*",
    "src/lib/guard.*",
    "src/lib/guards.*",
    "app/guards/auth.server.*",
];

const CORS_CONFIG_WATCH_PATHS: &[&str] = &[
    "cors.ts",
    "cors.js",
    "cors.config.*",
    "lib/cors.*",
    "src/lib/cors.*",
    "server/cors.*",
    "src/server/cors.*",
    "middleware/cors.*",
    "src/middleware/cors.*",
    "proxy.ts",
    "proxy.js",
    "lib/proxy.*",
    "src/lib/proxy.*",
    "server/proxy.*",
    "src/server/proxy.*",
    "server/middleware/cors.*",
    "src/server/middleware/cors.*",
];

const WATCH_SPECS: &[WatchSpec] = &[
    watch_spec(
        "dependencies",
        "updates",
        None,
        "Dependency files changed",
        "Lockfiles or package manifests changed. Re-check dependency risk for this project.",
        DEPENDENCY_WATCH_PATHS,
    ),
    watch_spec(
        "robots",
        "search-console",
        Some("seo.robots"),
        "robots.txt changed",
        "Crawl directives changed. Verify indexability and search visibility before shipping.",
        ROBOTS_WATCH_PATHS,
    ),
    watch_spec(
        "sitemap",
        "search-console",
        Some("seo.sitemap"),
        "Sitemap changed",
        "Sitemap output changed. Re-check search visibility and indexing coverage.",
        SITEMAP_WATCH_PATHS,
    ),
    watch_spec(
        "launch-config",
        "checklist",
        None,
        "Launch config changed",
        "Deployment or runtime configuration changed. Re-open Launch Plan before shipping again.",
        LAUNCH_CONFIG_WATCH_PATHS,
    ),
    watch_spec(
        "security-headers",
        "issues",
        Some("sec.headers"),
        "Header config changed",
        "Header or server config changed. Re-check security headers and exposed infrastructure signals.",
        SECURITY_HEADERS_WATCH_PATHS,
    ),
    watch_spec(
        "auth-session",
        "issues",
        Some("sec.cookies"),
        "Auth/session config changed",
        "Auth, cookie, or session handling changed. Re-check cookie security, CSRF, and session hardening.",
        AUTH_SESSION_WATCH_PATHS,
    ),
    watch_spec(
        "auth-guard",
        "issues",
        Some("sec.auth"),
        "Auth guard changed",
        "Route protection or authorization logic changed. Re-check server-side auth enforcement and access control.",
        AUTH_GUARD_WATCH_PATHS,
    ),
    watch_spec(
        "cors-config",
        "issues",
        Some("sec.cors"),
        "CORS or API boundary changed",
        "Cross-origin or proxy handling changed. Re-check CORS policy, credential exposure, and API boundary hardening.",
        CORS_CONFIG_WATCH_PATHS,
    ),
];

pub(crate) fn resolve_existing_path(project_root: &Path, relative_path: &str) -> Option<PathBuf> {
    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }

    let project_root = project_root.canonicalize().ok()?;
    let candidate = project_root.join(relative);
    if !candidate.exists() {
        return None;
    }
    let candidate = candidate.canonicalize().ok()?;
    if !candidate.starts_with(&project_root) {
        return None;
    }
    Some(candidate)
}

pub(crate) fn matches_watch_pattern(candidate: &str, pattern: &str) -> bool {
    if !pattern.contains('*') {
        return candidate == pattern;
    }

    let mut remainder = candidate;
    let mut parts = pattern.split('*').peekable();
    let starts_with_wildcard = pattern.starts_with('*');
    let ends_with_wildcard = pattern.ends_with('*');

    if let Some(first) = parts.peek().copied() {
        if !starts_with_wildcard {
            if !remainder.starts_with(first) {
                return false;
            }
            remainder = &remainder[first.len()..];
            parts.next();
        }
    }

    let mut parts_vec = parts.collect::<Vec<_>>();
    if !ends_with_wildcard {
        if let Some(last) = parts_vec.pop() {
            let mut middle = parts_vec.into_iter();
            for part in &mut middle {
                if part.is_empty() {
                    continue;
                }
                let Some(index) = remainder.find(part) else {
                    return false;
                };
                remainder = &remainder[index + part.len()..];
            }
            return remainder.ends_with(last);
        }
    }

    for part in parts_vec {
        if part.is_empty() {
            continue;
        }
        let Some(index) = remainder.find(part) else {
            return false;
        };
        remainder = &remainder[index + part.len()..];
    }

    true
}

pub(crate) fn resolve_existing_watch_paths(
    project_root: &Path,
    relative_pattern: &str,
) -> Vec<(String, PathBuf)> {
    if !relative_pattern.contains('*') {
        return resolve_existing_path(project_root, relative_pattern)
            .map(|path| vec![(relative_pattern.to_string(), path)])
            .unwrap_or_default();
    }

    let (parent, file_pattern) = match relative_pattern.rsplit_once('/') {
        Some((parent, file_pattern)) => (parent, file_pattern),
        None => ("", relative_pattern),
    };

    if parent.contains('*') {
        return Vec::new();
    }

    let Some(dir) = resolve_existing_path(project_root, parent) else {
        return Vec::new();
    };
    if !dir.is_dir() {
        return Vec::new();
    }

    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut matches = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            if !file_type.is_file() {
                return None;
            }
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !matches_watch_pattern(&file_name, file_pattern) {
                return None;
            }
            let relative_path = if parent.is_empty() {
                file_name
            } else {
                format!("{parent}/{file_name}")
            };
            Some((relative_path, entry.path()))
        })
        .collect::<Vec<_>>();

    matches.sort_by(|a, b| a.0.cmp(&b.0));
    matches
}

fn modified_ms(path: &Path) -> Option<u64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    modified
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis()
        .try_into()
        .ok()
}

#[tracing::instrument(skip(requests))]
pub(crate) fn inspect_watch_files(requests: &[DesktopWatchRequest]) -> Vec<DesktopWatchSignal> {
    let mut signals = Vec::new();

    for request in requests {
        let project_root = PathBuf::from(&request.project_path);
        if !project_root.is_dir() {
            continue;
        }

        for spec in WATCH_SPECS {
            for relative_pattern in spec.relative_paths {
                for (relative_path, existing_path) in
                    resolve_existing_watch_paths(&project_root, relative_pattern)
                {
                    let Some(modified_ms) = modified_ms(&existing_path) else {
                        continue;
                    };
                    let absolute_path = existing_path
                        .canonicalize()
                        .unwrap_or(existing_path.clone())
                        .to_string_lossy()
                        .to_string();

                    signals.push(DesktopWatchSignal {
                        project_id: request.project_id,
                        url: request.primary_url.clone(),
                        kind: spec.kind.to_string(),
                        relative_path,
                        absolute_path,
                        modified_ms,
                        page: spec.page.to_string(),
                        focus: spec.focus.map(str::to_string),
                        title: spec.title.to_string(),
                        detail: spec.detail.to_string(),
                    });
                }
            }
        }
    }

    signals
}

//! Which projects receive hygiene review, and where their findings anchor.

use super::manifest_scripts::ScriptInventory;
use super::*;

pub(super) struct ProjectKind {
    /// Server routes plus data or AI access: the original hygiene gate.
    pub(super) app_like: bool,
    /// A buildable web project without server routes (static sites, SPAs,
    /// composer roots), which still needs build, lint, CI, and hook review.
    pub(super) site_like: bool,
    /// A JavaScript web project by framework config or dependency; the only
    /// kind whose package.json is expected to own a production build script.
    pub(super) js_site_like: bool,
    /// A composer.json was read, so PHP rather than package.json drives the app.
    pub(super) composer_root: bool,
    /// Root-level hosting configs; a host build on push is not a quality gate.
    pub(super) hosting_configs: Vec<String>,
    /// package.json or composer.json to anchor manifest-level findings.
    pub(super) manifest_anchor: Option<(String, String)>,
    /// Manifest anchor, else the first route file.
    pub(super) anchor: Option<(String, String)>,
}

impl ProjectKind {
    pub(super) fn hygiene_eligible(&self) -> bool {
        self.app_like || self.site_like
    }
}

pub(super) fn classify_project(
    context: &ProjectHygieneContext<'_>,
    scripts: &ScriptInventory,
) -> ProjectKind {
    let paths = context.project_paths_lower;
    let framework_config = paths.iter().any(|path| {
        let file_name = path.rsplit('/').next().unwrap_or(path);
        SITE_FRAMEWORK_CONFIG_FILES.contains(&file_name)
    });
    let web_dependency = has_named_dependency(context.declared_dependencies, WEB_PROJECT_PACKAGES);
    let js_site_like = framework_config || web_dependency;
    let composer_root = !scripts.composer.is_empty();
    let site_like = js_site_like || scripts.has_build_script() || composer_root;
    let hosting_configs = paths
        .iter()
        .filter(|path| HOSTING_CONFIG_FILES.contains(&path.as_str()))
        .cloned()
        .collect();

    let manifest_anchor = context
        .manifests
        .first()
        .map(|manifest| {
            (
                manifest.relative_path.clone(),
                manifest.absolute_path.to_string_lossy().to_string(),
            )
        })
        .or_else(|| {
            scripts.composer.first().map(|manifest| {
                (
                    manifest.relative_path.clone(),
                    context
                        .root
                        .join(&manifest.relative_path)
                        .to_string_lossy()
                        .to_string(),
                )
            })
        });
    let anchor = manifest_anchor.clone().or_else(|| {
        context.route_files.first().map(|file| {
            (
                file.relative_path.clone(),
                file.absolute_path.to_string_lossy().to_string(),
            )
        })
    });

    ProjectKind {
        app_like: context.app_like,
        site_like,
        js_site_like,
        composer_root,
        hosting_configs,
        manifest_anchor,
        anchor,
    }
}

//! Whether a checked-in hook configuration is actually active in this clone.
//!
//! Husky, Lefthook, pre-commit, and simple-git-hooks all need an install step
//! that writes into `.git/hooks` or points `core.hooksPath` at their runtime
//! directory. A committed config without that step protects nothing.

use super::*;

const NATIVE_HOOK_NAMES: &[&str] = &["pre-commit", "pre-push", "commit-msg", "pre-merge-commit"];

#[derive(Debug, Default)]
pub(super) struct HookInstallState {
    /// `.git` is a directory, so the hook facts below could be observed.
    pub(super) git_dir: bool,
    /// Hook scripts present in `.git/hooks` (samples excluded by name).
    pub(super) native_hooks: Vec<&'static str>,
    /// Husky's generated `.husky/_` runtime directory exists.
    pub(super) husky_runtime: bool,
    /// `.git/config` sets `core.hooksPath`.
    pub(super) hooks_path_configured: bool,
}

impl HookInstallState {
    pub(super) fn active(&self) -> bool {
        !self.native_hooks.is_empty() || self.husky_runtime || self.hooks_path_configured
    }
}

pub(super) fn inspect_hook_install(root: &Path) -> HookInstallState {
    let git = root.join(".git");
    if !git.is_dir() {
        return HookInstallState::default();
    }
    let hooks_dir = git.join("hooks");
    let native_hooks = NATIVE_HOOK_NAMES
        .iter()
        .copied()
        .filter(|name| hooks_dir.join(name).is_file())
        .collect();
    let husky_runtime = root.join(".husky").join("_").is_dir();
    // Only the presence of the key is inspected; the config text is never
    // surfaced because remote URLs can embed credentials.
    let hooks_path_configured =
        crate::core::code_scan::filesystem::read_text_under_root(root, &git.join("config"))
            .map(|config| config.to_ascii_lowercase().contains("hookspath"))
            .unwrap_or(false);
    HookInstallState {
        git_dir: true,
        native_hooks,
        husky_runtime,
        hooks_path_configured,
    }
}

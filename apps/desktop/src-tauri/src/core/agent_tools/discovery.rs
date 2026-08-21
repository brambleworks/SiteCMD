//! Executable discovery for agent CLIs and the Node runtime used by MCP.

use std::path::{Path, PathBuf};

pub(super) fn home_dir() -> Result<PathBuf, String> {
    #[cfg(windows)]
    const HOME_VAR: &str = "USERPROFILE";
    #[cfg(not(windows))]
    const HOME_VAR: &str = "HOME";

    std::env::var_os(HOME_VAR)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| format!("could not resolve the home directory ({HOME_VAR} is not set)"))
}

fn candidate_binary_names(name: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        vec![
            name.to_string(),
            format!("{name}.exe"),
            format!("{name}.cmd"),
        ]
    }
    #[cfg(not(windows))]
    {
        vec![name.to_string()]
    }
}

/// Fallback binary directories for GUI launches with a minimal PATH.
/// System-wide package managers precede per-user locations.
#[cfg(not(windows))]
fn fallback_binary_dirs() -> Vec<PathBuf> {
    fallback_binary_dirs_for(home_dir().ok().as_deref())
}

/// Build fallback directories without reading the process home directory.
#[cfg(not(windows))]
pub(super) fn fallback_binary_dirs_for(home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    let Some(home) = home else {
        return dirs;
    };
    dirs.push(home.join(".local").join("bin"));
    dirs.push(home.join(".claude").join("local"));

    // Search stable Node manager install roots, excluding per-shell shims.
    dirs.push(
        home.join(".local")
            .join("share")
            .join("fnm")
            .join("aliases")
            .join("default")
            .join("bin"),
    );
    dirs.push(
        home.join(".fnm")
            .join("aliases")
            .join("default")
            .join("bin"),
    );
    dirs.push(home.join(".volta").join("bin"));
    dirs.push(home.join(".asdf").join("shims"));
    dirs.push(home.join(".local").join("share").join("mise").join("shims"));

    // Enumerate nvm installs because its default alias may be symbolic.
    let nvm = home.join(".nvm");
    dirs.push(nvm.join("current").join("bin"));
    if let Some(bin) = nvm_default_bin(&nvm) {
        dirs.push(bin);
    }
    for bin in nvm_installed_bins(&nvm) {
        if !dirs.contains(&bin) {
            dirs.push(bin);
        }
    }
    dirs
}

/// Resolve nvm's `default` alias to its `versions/node/<v>/bin` directory when
/// the alias names a concrete installed version. Symbolic aliases are covered
/// by the installed-version enumeration below.
#[cfg(not(windows))]
fn nvm_default_bin(nvm: &Path) -> Option<PathBuf> {
    let alias = std::fs::read_to_string(nvm.join("alias").join("default")).ok()?;
    let version = alias.trim();
    if version.is_empty() {
        return None;
    }
    let candidate = nvm.join("versions").join("node").join(version).join("bin");
    candidate.is_dir().then_some(candidate)
}

#[cfg(not(windows))]
fn nvm_installed_bins(nvm: &Path) -> Vec<PathBuf> {
    let versions_dir = nvm.join("versions").join("node");
    let Ok(entries) = std::fs::read_dir(&versions_dir) else {
        return Vec::new();
    };
    let mut versions = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let version = parse_nvm_version(name.to_str()?)?;
            let bin = entry.path().join("bin");
            bin.is_dir().then_some((version, bin))
        })
        .collect::<Vec<_>>();
    versions.sort_by(|(left, _), (right, _)| right.cmp(left));
    versions.into_iter().map(|(_, bin)| bin).collect()
}

#[cfg(not(windows))]
fn parse_nvm_version(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.strip_prefix('v').unwrap_or(value).split('.');
    let version = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(version)
}

/// Windows GUI apps inherit the full user PATH, so no fallback scan is needed.
#[cfg(windows)]
fn fallback_binary_dirs() -> Vec<PathBuf> {
    Vec::new()
}

/// Resolve every matching binary: first the PATH scan, then fallback dirs.
/// Compatibility probes can skip an outdated Node and continue to a newer
/// version-manager installation without changing agent-CLI selection.
pub(super) fn binary_paths(name: &str) -> Vec<PathBuf> {
    fn find_in(dir: &Path, candidates: &[String]) -> Option<PathBuf> {
        candidates
            .iter()
            .map(|candidate| dir.join(candidate))
            .find(|full| full.is_file())
    }

    let candidates = candidate_binary_names(name);
    let mut binaries = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for found in std::env::split_paths(&path).filter_map(|dir| find_in(&dir, &candidates)) {
            if !binaries.contains(&found) {
                binaries.push(found);
            }
        }
    }
    for found in fallback_binary_dirs()
        .into_iter()
        .filter_map(|dir| find_in(&dir, &candidates))
    {
        if !binaries.contains(&found) {
            binaries.push(found);
        }
    }
    binaries
}

pub(super) fn binary_on_path(name: &str) -> Option<PathBuf> {
    binary_paths(name).into_iter().next()
}

pub(super) fn binary_available(name: &str) -> bool {
    binary_on_path(name).is_some()
}

/// Registration runs the bundled MCP server through `node`, so that binary
/// has to exist. The parent module separately probes for built-in node:sqlite.
pub fn node_available() -> bool {
    binary_available("node")
}

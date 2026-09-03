use super::*;

mod imports;
mod manifests;
mod package_names;
mod peers;
mod registry;
mod scripts;

pub(super) use imports::{
    collect_js_package_refs, collect_package_names_in_content, normalize_package_spec,
};
pub(super) use manifests::collect_package_manifests;
pub(super) use package_names::{
    dependency_spec_is_local, has_named_dependency, should_ignore_unused_dependency,
    suspicious_package_match,
};
pub(super) use peers::collect_lockfile_peer_dependencies;
pub(super) use registry::{
    allowed_registry_hosts_for_dependency, collect_lockfile_registry_hosts,
    collect_registry_config, dependency_spec_uses_remote_url, format_registry_host_list,
};
pub(super) use scripts::collect_script_referenced_packages;

#[cfg(test)]
pub(super) use scripts::appears_as_script_token;

#[derive(Debug, Clone)]
pub(super) struct PackageManifest {
    pub(super) absolute_path: PathBuf,
    pub(super) relative_path: String,
    pub(super) content: String,
    pub(super) package_name: Option<String>,
    /// Every declared name across `dependencies`, `devDependencies`,
    /// `peerDependencies`, and `optionalDependencies`.
    pub(super) dependencies: HashSet<String>,
    /// Names from `dependencies` and `devDependencies` only: the packages this
    /// manifest installs itself. A peer dependency is the consuming app's
    /// install and an optional one may legitimately be absent, so lockfile and
    /// usage rules must not hold this package responsible for them.
    pub(super) installed_dependencies: HashSet<String>,
    pub(super) local_dependencies: HashSet<String>,
    pub(super) dependency_specs: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub(super) struct PackageReference {
    pub(super) package_name: String,
    pub(super) relative_path: String,
    pub(super) absolute_path: String,
    pub(super) line: Option<u32>,
}

#[derive(Debug, Default, Clone)]
pub(super) struct RegistryConfig {
    pub(super) default_hosts: HashSet<String>,
    pub(super) scope_hosts: HashMap<String, HashSet<String>>,
}

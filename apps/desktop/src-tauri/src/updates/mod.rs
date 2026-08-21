//! Dependency discovery, registry updates, and vulnerability lookup.

pub mod ci;
pub mod composer;
pub mod drupal;
pub mod golang;
pub mod npm;
mod npm_lockfiles;
mod npm_workspaces;
pub mod python;
pub mod registry;
pub mod ruby;
pub mod rust_crates;
pub mod ssl;
pub mod types;
pub mod wordpress;

use std::cell::Cell;
use std::collections::HashSet;
use std::io::Read;
use std::path::Path;
use types::{Ecosystem, InstalledPackage};

thread_local! {
    // Thread-local partial flag scoped to one synchronous dependency-detection
    // pass, shared by all parsers.
    static PRESENT_BUT_UNREADABLE: Cell<bool> = const { Cell::new(false) };
}

/// Marks dependency detection partial when an existing path cannot be read,
/// preventing prior findings from resolving as if the path were empty.
fn set_present_but_unreadable() {
    PRESENT_BUT_UNREADABLE.with(|flag| flag.set(true));
}

/// [`set_present_but_unreadable`] shaped for `read_dependency_file`'s
/// early-return paths: flag the pass partial and yield "no content".
fn note_present_but_unreadable() -> Option<String> {
    set_present_but_unreadable();
    None
}

pub(crate) fn read_dependency_file(path: &Path) -> Option<String> {
    // Reject symlinks and use O_NOFOLLOW for untrusted project files. Missing
    // files are authoritative absence; other read failures mark the pass partial.
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return None;
        }
        Err(_) => return note_present_but_unreadable(),
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > crate::constants::MAX_DEPENDENCY_FILE_BYTES
    {
        return note_present_but_unreadable();
    }

    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        match std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
        {
            Ok(file) => file,
            Err(_) => return note_present_but_unreadable(),
        }
    };
    #[cfg(not(unix))]
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return note_present_but_unreadable(),
    };

    let opened_metadata = match file.metadata() {
        Ok(metadata) => metadata,
        Err(_) => return note_present_but_unreadable(),
    };
    if !opened_metadata.is_file()
        || opened_metadata.len() > crate::constants::MAX_DEPENDENCY_FILE_BYTES
    {
        return note_present_but_unreadable();
    }

    let mut bytes = Vec::with_capacity(opened_metadata.len() as usize);
    if file
        .take(crate::constants::MAX_DEPENDENCY_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > crate::constants::MAX_DEPENDENCY_FILE_BYTES
    {
        return note_present_but_unreadable();
    }
    // The bytes were fully observed; non-UTF8 content is an authoritative
    // "not a parseable dependency file" (like malformed JSON), not a partial.
    String::from_utf8(bytes).ok()
}

/// Result of one dependency detection pass over a project directory.
pub struct DependencyDetection {
    pub packages: Vec<InstalledPackage>,
    /// True when a dependency source was unreadable, oversized, or failed to
    /// parse, making absent packages unproven.
    pub partial: bool,
}

/// Detect all installed dependencies from a project directory.
/// Iterates all ecosystem parsers, collects results, deduplicates.
pub fn detect_dependencies(dir: &Path) -> DependencyDetection {
    let mut packages = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new(); // (ecosystem_name, package_name)
    PRESENT_BUT_UNREADABLE.with(|flag| flag.set(false));
    let mut partial = false;

    #[allow(clippy::type_complexity)]
    let parsers: Vec<(&str, Box<dyn Fn(&Path) -> Vec<InstalledPackage>>)> = vec![
        ("npm", Box::new(npm::parse)),
        ("composer", Box::new(composer::parse)),
        ("wordpress", Box::new(wordpress::parse)),
        ("drupal", Box::new(drupal::parse)),
        ("python", Box::new(python::parse)),
        ("ruby", Box::new(ruby::parse)),
        ("go", Box::new(golang::parse)),
        ("rust", Box::new(rust_crates::parse)),
    ];

    for (label, parser) in &parsers {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parser(dir)));
        match result {
            Ok(pkgs) => {
                let count = pkgs.len();
                for pkg in pkgs {
                    let key = (format!("{:?}", pkg.ecosystem), pkg.name.clone());
                    if seen.insert(key) {
                        packages.push(pkg);
                    }
                }
                if count > 0 {
                    tracing::info!("updates: {} - detected {} packages", label, count);
                }
            }
            Err(_) => {
                tracing::warn!("updates: {} parser panicked, skipping", label);
                // The panicked family was not observed; its packages must not
                // read as removed.
                partial = true;
            }
        }
    }

    partial |= PRESENT_BUT_UNREADABLE.with(|flag| flag.get());
    DependencyDetection { packages, partial }
}

/// Get list of detected ecosystems from a set of packages
pub fn detected_ecosystems(packages: &[InstalledPackage]) -> Vec<Ecosystem> {
    let mut ecosystems: Vec<Ecosystem> = Vec::new();
    for pkg in packages {
        if !ecosystems.contains(&pkg.ecosystem) {
            ecosystems.push(pkg.ecosystem.clone());
        }
    }
    ecosystems
}

#[cfg(test)]
mod bounded_file_tests {
    use super::*;

    #[test]
    fn dependency_file_reader_rejects_files_over_the_byte_budget() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("package-lock.json");
        let file = std::fs::File::create(&path).expect("create file");
        file.set_len(crate::constants::MAX_DEPENDENCY_FILE_BYTES + 1)
            .expect("grow sparse file");

        assert!(read_dependency_file(&path).is_none());
    }

    #[test]
    fn oversized_lockfile_marks_detection_partial() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies": {"left-pad": "^1.0.0"}}"#,
        )
        .expect("write package.json");
        let file =
            std::fs::File::create(dir.path().join("package-lock.json")).expect("create lockfile");
        file.set_len(crate::constants::MAX_DEPENDENCY_FILE_BYTES + 1)
            .expect("grow sparse file");

        let detection = detect_dependencies(dir.path());
        assert!(detection.packages.is_empty());
        assert!(
            detection.partial,
            "a present-but-oversized lockfile must mark the detection pass partial"
        );
    }

    #[test]
    fn absent_lockfile_keeps_detection_authoritative() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies": {"left-pad": "^1.0.0"}}"#,
        )
        .expect("write package.json");
        std::fs::create_dir(dir.path().join(".git")).expect("create .git");

        let detection = detect_dependencies(dir.path());
        assert!(detection.packages.is_empty());
        assert!(
            !detection.partial,
            "an absent lockfile is an authoritative observation, not a partial one"
        );
    }

    /// Returns `None` when the platform does not enforce the permission drop.
    #[cfg(unix)]
    fn detect_with_unreadable_dir(
        project_dir: &Path,
        unreadable_dir: &Path,
    ) -> Option<DependencyDetection> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(unreadable_dir, std::fs::Permissions::from_mode(0o000))
            .expect("chmod dir unreadable");
        let enforced = std::fs::read_dir(unreadable_dir).is_err();
        let detection = detect_dependencies(project_dir);
        std::fs::set_permissions(unreadable_dir, std::fs::Permissions::from_mode(0o755))
            .expect("restore dir permissions");
        enforced.then_some(detection)
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_wordpress_plugins_dir_marks_detection_partial() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plugins = dir.path().join("wp-content/plugins");
        std::fs::create_dir_all(&plugins).expect("create plugins dir");

        let Some(detection) = detect_with_unreadable_dir(dir.path(), &plugins) else {
            return; // permissions not enforced on this platform/user
        };
        assert!(
            detection.partial,
            "an unreadable plugins directory must mark the detection pass partial"
        );
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_wordpress_themes_dir_marks_detection_partial() {
        let dir = tempfile::tempdir().expect("tempdir");
        let themes = dir.path().join("wp-content/themes");
        std::fs::create_dir_all(&themes).expect("create themes dir");

        let Some(detection) = detect_with_unreadable_dir(dir.path(), &themes) else {
            return;
        };
        assert!(
            detection.partial,
            "an unreadable themes directory must mark the detection pass partial"
        );
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_drupal_modules_dir_marks_detection_partial() {
        // Same hole for Drupal: scan_info_yml swallowed the read_dir error,
        // so an unreadable modules/contrib read as "no modules installed".
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("web/core/lib")).expect("create core dir");
        std::fs::write(
            dir.path().join("web/core/lib/Drupal.php"),
            "<?php\nclass Drupal {\n  const VERSION = '10.2.3';\n}\n",
        )
        .expect("write Drupal.php");
        let modules = dir.path().join("web/modules/contrib");
        std::fs::create_dir_all(&modules).expect("create modules dir");

        let Some(detection) = detect_with_unreadable_dir(dir.path(), &modules) else {
            return;
        };
        assert!(
            detection.partial,
            "an unreadable Drupal modules directory must mark the detection pass partial"
        );
    }

    #[test]
    fn readable_lockfile_keeps_detection_authoritative() {
        // Happy path guard: a parseable lockfile yields packages AND a
        // non-partial pass, so normal diff-resolution keeps working.
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies": {"left-pad": "^1.0.0"}}"#,
        )
        .expect("write package.json");
        std::fs::write(
            dir.path().join("package-lock.json"),
            r#"{"lockfileVersion": 3, "packages": {"node_modules/left-pad": {"version": "1.3.0"}}}"#,
        )
        .expect("write lockfile");

        let detection = detect_dependencies(dir.path());
        assert!(
            detection.packages.iter().any(|p| p.name == "left-pad"),
            "expected left-pad, got: {:?}",
            detection
                .packages
                .iter()
                .map(|p| &p.name)
                .collect::<Vec<_>>()
        );
        assert!(!detection.partial);
    }
}

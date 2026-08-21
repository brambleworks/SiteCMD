//! Bounded reads from untrusted project trees.

use std::fs;
use std::io::Read;
use std::path::Path;

pub(crate) fn read_bounded_file(path: &Path, max_bytes: u64) -> Option<Vec<u8>> {
    let initial_metadata = fs::symlink_metadata(path).ok()?;
    if initial_metadata.file_type().is_symlink()
        || !initial_metadata.is_file()
        || initial_metadata.len() > max_bytes
    {
        return None;
    }

    #[cfg(unix)]
    let opened = {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .ok()?
    };
    #[cfg(not(unix))]
    let opened = fs::File::open(path).ok()?;

    let opened_metadata = opened.metadata().ok()?;
    if !opened_metadata.is_file() || opened_metadata.len() > max_bytes {
        return None;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if initial_metadata.dev() != opened_metadata.dev()
            || initial_metadata.ino() != opened_metadata.ino()
        {
            return None;
        }
    }

    let mut bytes = Vec::with_capacity(opened_metadata.len() as usize);
    opened
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= max_bytes).then_some(bytes)
}

pub(crate) fn read_bounded_file_under_root(
    root: &Path,
    path: &Path,
    max_bytes: u64,
) -> Option<Vec<u8>> {
    let canonical_root = fs::canonicalize(root).ok()?;
    let canonical_path = fs::canonicalize(path).ok()?;
    if !canonical_path.starts_with(&canonical_root) {
        return None;
    }
    // Read the verified canonical target so in-root symlinks work without
    // re-resolving an untrusted caller path during the bounded read.
    read_bounded_file(&canonical_path, max_bytes)
}

pub(crate) fn read_bounded_text_under_root(
    root: &Path,
    path: &Path,
    max_bytes: u64,
) -> Option<String> {
    String::from_utf8(read_bounded_file_under_root(root, path, max_bytes)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_project_read_accepts_regular_file_and_rejects_oversize() {
        let project = tempfile::tempdir().expect("project");
        let manifest = project.path().join("package.json");
        fs::write(&manifest, b"{\"name\":\"site\"}").expect("write manifest");

        assert_eq!(
            read_bounded_text_under_root(project.path(), &manifest, 64).as_deref(),
            Some("{\"name\":\"site\"}")
        );
        assert!(read_bounded_file_under_root(project.path(), &manifest, 4).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn bounded_project_read_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let project = tempfile::tempdir().expect("project");
        let outside = tempfile::NamedTempFile::new().expect("outside file");
        fs::write(outside.path(), b"APP_URL=https://private.example").expect("write outside file");
        let linked = project.path().join(".env");
        symlink(outside.path(), &linked).expect("create symlink");

        assert!(
            read_bounded_text_under_root(project.path(), &linked, 1024).is_none(),
            "repository metadata reads must not follow symlinks outside the project"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bounded_project_read_follows_in_project_symlink() {
        use std::os::unix::fs::symlink;

        let project = tempfile::tempdir().expect("project");
        let real = project.path().join("config.real.json");
        fs::write(&real, b"{\"name\":\"linked\"}").expect("write real target");
        let linked = project.path().join("package.json");
        symlink(&real, &linked).expect("create in-project symlink");

        assert_eq!(
            read_bounded_text_under_root(project.path(), &linked, 1024).as_deref(),
            Some("{\"name\":\"linked\"}"),
            "an in-project symlink must resolve to its in-project target so monorepo layouts still detect config"
        );
    }
}

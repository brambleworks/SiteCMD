//! Git integration - reads commit history by shelling out to `git log`.
//!
//! Used for deploy tracking: detects new commits since last scan.

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use ts_rs::TS;

use crate::db::{EventSeverity, EventSource, EventType, SiteEvent};

// allow-inline-duration: git-command wall-clock timeout is private to
// this module and has no constants.rs equivalent.
const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_GIT_OUTPUT_BYTES: usize = 1024 * 1024;

/// Command-line overrides applied to every git spawn. A registered project
/// tree may carry a hostile `.git/config`, so every repository key that names
/// an executable is neutralized before the subcommand, optional index writes
/// are skipped, and no transport protocol may be negotiated.
const GIT_HARDENING_ARGS: &[&str] = &[
    "--no-optional-locks",
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.hooksPath=",
    "-c",
    "core.sshCommand=",
    "-c",
    "diff.external=",
    "-c",
    "protocol.allow=never",
];

/// An empty config file for `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM`. Git
/// for Windows maps `/dev/null` to `NUL` internally, but naming the device
/// directly keeps the intent obvious on both platforms.
#[cfg(windows)]
const GIT_NULL_CONFIG_FILE: &str = "NUL";
#[cfg(not(windows))]
const GIT_NULL_CONFIG_FILE: &str = "/dev/null";

/// The only variables a git child may inherit. Everything else, including
/// every `GIT_*`, `SSH_*`, `LD_PRELOAD`, and shell hook variable, is dropped.
/// PATH is required to locate git; the rest keep git's own startup working.
const GIT_INHERITED_ENV: &[&str] = &[
    "PATH",
    "HOME",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "SYSTEMROOT",
    "TEMP",
    "TMP",
    "TMPDIR",
];

/// Variables set explicitly on every git child.
const GIT_FIXED_ENV: &[(&str, &str)] = &[
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_CONFIG_GLOBAL", GIT_NULL_CONFIG_FILE),
    ("GIT_CONFIG_SYSTEM", GIT_NULL_CONFIG_FILE),
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_OPTIONAL_LOCKS", "0"),
    ("LC_ALL", "C"),
];

/// The one place git is spawned. Every caller goes through `run_git`, which
/// a source-scanning test in `lib_tests.rs` enforces.
fn hardened_git_command(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command.env_clear();
    for key in GIT_INHERITED_ENV {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    for (key, value) in GIT_FIXED_ENV {
        command.env(key, value);
    }
    command.args(GIT_HARDENING_ARGS).args(args).current_dir(dir);
    command
}

/// A git commit from the project's log
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct GitCommit {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub date: String,          // ISO 8601
    pub relative_date: String, // "2 hours ago"
}

/// Git status for a project directory
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct GitStatus {
    pub is_git_repo: bool,
    pub branch: Option<String>,
    pub commits: Vec<GitCommit>,
    pub total_commits: u32,
    pub has_uncommitted: bool,
}

/// Build the shared deploy-event shape from a git commit.
pub fn commit_to_deploy_event(commit: &GitCommit, project_id: i64) -> SiteEvent {
    SiteEvent {
        id: 0,
        project_id,
        event_type: EventType::Deploy,
        severity: EventSeverity::Info,
        // Git author dates (%aI) are RFC 3339 with the author's local offset;
        // epoch ms normalizes them.
        occurred_at_ms: crate::db::timestamp_text_to_ms(&commit.date)
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
        title: commit
            .message
            .lines()
            .next()
            .unwrap_or(&commit.message)
            .to_string(),
        summary: format!("{} - {}", commit.short_hash, commit.author),
        detail: Some(
            serde_json::json!({
                "hash": commit.hash,
                "short_hash": commit.short_hash,
                "author": commit.author,
                "message": commit.message,
            })
            .to_string(),
        ),
        source: EventSource::Git,
        source_id: Some(commit.hash.clone()),
        metadata: None,
        // Git deploy events do not map to a specific check_id.
        affected_check_ids: None,
    }
}

/// `git log --format=...%x1f...` emits ASCII Unit Separator (`\x1f`) between
/// fields. `\x1f` is forbidden in well-formed commit messages, so it's safe
/// even when messages contain `|`, tabs, or other prose punctuation. Returns
/// None for malformed lines.
const FIELD_SEPARATOR: char = '\x1f';

fn parse_log_line(line: &str) -> Option<GitCommit> {
    let parts: Vec<&str> = line.splitn(6, FIELD_SEPARATOR).collect();
    if parts.len() < 6 {
        return None;
    }
    Some(GitCommit {
        hash: parts[0].to_string(),
        short_hash: parts[1].to_string(),
        message: parts[2].to_string(),
        author: parts[3].to_string(),
        date: parts[4].to_string(),
        relative_date: parts[5].to_string(),
    })
}

/// Parse multi-line `git log` output into commits, dropping blank/malformed lines.
fn parse_log_output(output: &str) -> Vec<GitCommit> {
    output
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(parse_log_line)
        .collect()
}

/// Read git log from a project directory
#[tracing::instrument(skip(project_path), fields(project_path_len = project_path.len(), limit))]
pub fn get_git_status(project_path: &str, limit: u32) -> GitStatus {
    let dir = Path::new(project_path);
    let limit = limit.clamp(1, 200);

    if !dir.join(".git").exists() && !has_git_dir(dir) {
        return GitStatus {
            is_git_repo: false,
            branch: None,
            commits: Vec::new(),
            total_commits: 0,
            has_uncommitted: false,
        };
    }

    let branch = run_git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).map(|s| s.trim().to_string());

    let has_uncommitted = run_git(dir, &["status", "--porcelain"])
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    // Use ASCII Unit Separator (\x1f) instead of `|` so commit messages
    // that contain `|` don't break field parsing.
    let log_format = "--format=%H%x1f%h%x1f%s%x1f%an%x1f%aI%x1f%ar";
    let log_output = run_git(
        dir,
        &["log", log_format, &format!("-{}", limit), "--no-merges"],
    );

    let commits = log_output
        .as_deref()
        .map(parse_log_output)
        .unwrap_or_default();

    let total_commits = run_git(dir, &["rev-list", "--count", "HEAD"])
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(commits.len() as u32);

    GitStatus {
        is_git_repo: true,
        branch,
        commits,
        total_commits,
        has_uncommitted,
    }
}

/// Get commits since a specific date (for detecting new deploys)
#[tracing::instrument(skip(project_path), fields(project_path_len = project_path.len(), since = %since))]
pub fn get_commits_since(project_path: &str, since: &str) -> Vec<GitCommit> {
    let dir = Path::new(project_path);
    // Use ASCII Unit Separator (\x1f) instead of `|` so commit messages
    // that contain `|` don't break field parsing.
    let log_format = "--format=%H%x1f%h%x1f%s%x1f%an%x1f%aI%x1f%ar";
    let since_arg = format!("--since={}", since);

    let output = run_git(dir, &["log", log_format, &since_arg, "--no-merges"]);
    output.as_deref().map(parse_log_output).unwrap_or_default()
}

/// Return commits in the inclusive window, newest first, plus the uncapped count.
/// The list is whole-or-empty and capped at 200; failures return an empty list.
#[tracing::instrument(skip(project_path), fields(project_path_len = project_path.len(), since = %since, until = %until))]
pub fn get_commits_between(project_path: &str, since: &str, until: &str) -> (Vec<GitCommit>, u32) {
    let dir = Path::new(project_path);
    let log_format = "--format=%H%x1f%h%x1f%s%x1f%an%x1f%aI%x1f%ar";
    let since_arg = format!("--since={}", since);
    let until_arg = format!("--until={}", until);

    let output = run_git(
        dir,
        &[
            "log",
            log_format,
            &since_arg,
            &until_arg,
            "--no-merges",
            "-n",
            "200",
        ],
    );
    let commits = output.as_deref().map(parse_log_output).unwrap_or_default();
    let total = run_git(
        dir,
        &[
            "rev-list",
            "--count",
            "--no-merges",
            &since_arg,
            &until_arg,
            "HEAD",
        ],
    )
    .and_then(|s| s.trim().parse::<u32>().ok())
    .unwrap_or(commits.len() as u32);
    (commits, total)
}

fn run_git(dir: &Path, args: &[&str]) -> Option<String> {
    let mut child = hardened_git_command(dir, args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let mut stdout = child.stdout.take()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut output = Vec::new();
        let mut exceeded_limit = false;
        let mut buf = [0_u8; 8192];

        loop {
            match stdout.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let remaining = MAX_GIT_OUTPUT_BYTES.saturating_sub(output.len());
                    if n > remaining {
                        exceeded_limit = true;
                    }
                    if remaining > 0 {
                        output.extend_from_slice(&buf[..n.min(remaining)]);
                    }
                }
                Err(_) => {
                    exceeded_limit = true;
                    break;
                }
            }
        }

        let _ = tx.send((output, exceeded_limit));
    });

    let started_at = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started_at.elapsed() >= GIT_COMMAND_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => return None,
        }
    };

    if !status.success() {
        return None;
    }

    let (stdout, exceeded_limit) = rx.recv_timeout(Duration::from_millis(250)).ok()?;
    if exceeded_limit {
        return None;
    }
    String::from_utf8(stdout).ok()
}

fn has_git_dir(dir: &Path) -> bool {
    // Check if we're inside a git repo (even if.git is in a parent)
    run_git(dir, &["rev-parse", "--git-dir"]).is_some()
}

/// The exact checkout a file walk would observe, without applying the commit
/// history view's `--no-merges` filter. Merge commits are ordinary checkout
/// heads and must remain eligible for exact deployment provenance.
pub fn checkout_head_and_clean(project_path: &str) -> Option<(String, bool)> {
    let dir = Path::new(project_path);
    let head = run_git(dir, &["rev-parse", "HEAD"])?;
    let head = head.trim();
    if head.len() < 7
        || head.len() > 64
        || !head
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
    {
        return None;
    }
    let status = run_git(dir, &["status", "--porcelain"])?;
    Some((head.to_string(), status.trim().is_empty()))
}

/// Get recent commits (convenience wrapper for backfill)
#[tracing::instrument(skip(project_path), fields(project_path_len = project_path.len(), limit))]
pub fn get_recent_commits(project_path: &str, limit: u32) -> Vec<GitCommit> {
    get_git_status(project_path, limit).commits
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    /// Build a single log line with the same separator the production code uses.
    fn log_line(fields: &[&str]) -> String {
        fields.join("\x1f")
    }

    #[test]
    fn commit_to_deploy_event_builds_expected_shape() {
        let commit = GitCommit {
            hash: "abc123def456".to_string(),
            short_hash: "abc123d".to_string(),
            message: "Fix login bug\n\nLonger body text".to_string(),
            author: "Kyle Piontek".to_string(),
            date: "2026-04-19T10:00:00Z".to_string(),
            relative_date: "3 hours ago".to_string(),
        };
        let event = commit_to_deploy_event(&commit, 7);

        assert_eq!(event.id, 0, "id must be 0 so insert auto-assigns");
        assert_eq!(event.project_id, 7);
        assert_eq!(event.event_type, crate::db::EventType::Deploy);
        assert_eq!(event.severity, crate::db::EventSeverity::Info);
        assert_eq!(event.source, crate::db::EventSource::Git);
        assert_eq!(event.source_id.as_deref(), Some("abc123def456"));
        // Title is the first line of the message only.
        assert_eq!(event.title, "Fix login bug");
        assert_eq!(event.summary, "abc123d - Kyle Piontek");
        // The author date parses to epoch ms (not the now fallback).
        assert_eq!(
            event.occurred_at_ms,
            crate::db::timestamp_text_to_ms("2026-04-19T10:00:00Z").unwrap()
        );
        // detail carries the full commit payload verbatim.
        let detail: serde_json::Value =
            serde_json::from_str(event.detail.as_deref().unwrap()).unwrap();
        assert_eq!(detail["hash"], "abc123def456");
        assert_eq!(detail["short_hash"], "abc123d");
        assert_eq!(detail["author"], "Kyle Piontek");
        assert_eq!(detail["message"], "Fix login bug\n\nLonger body text");
        assert!(event.affected_check_ids.is_none());
        assert!(event.metadata.is_none());
    }

    #[test]
    fn commit_to_deploy_event_falls_back_to_now_on_unparseable_date() {
        let before = chrono::Utc::now().timestamp_millis();
        let commit = GitCommit {
            hash: "deadbeef".to_string(),
            short_hash: "deadbee".to_string(),
            message: "single line".to_string(),
            author: "A".to_string(),
            date: "not-a-date".to_string(),
            relative_date: "just now".to_string(),
        };
        let event = commit_to_deploy_event(&commit, 1);
        let after = chrono::Utc::now().timestamp_millis();
        assert!(
            event.occurred_at_ms >= before && event.occurred_at_ms <= after,
            "unparseable date must fall back to Utc::now()"
        );
        // A single-line message becomes the title unchanged.
        assert_eq!(event.title, "single line");
    }

    #[test]
    fn parse_log_line_accepts_well_formed_input() {
        let line = log_line(&[
            "abc123def456",
            "abc123d",
            "Initial commit",
            "Kyle Piontek",
            "2026-04-19T10:00:00Z",
            "3 hours ago",
        ]);
        let commit = parse_log_line(&line).expect("should parse");
        assert_eq!(commit.hash, "abc123def456");
        assert_eq!(commit.short_hash, "abc123d");
        assert_eq!(commit.message, "Initial commit");
        assert_eq!(commit.author, "Kyle Piontek");
        assert_eq!(commit.date, "2026-04-19T10:00:00Z");
        assert_eq!(commit.relative_date, "3 hours ago");
    }

    #[test]
    fn parse_log_line_preserves_pipes_inside_message() {
        let line = log_line(&[
            "h",
            "s",
            "fix: foo | bar | baz",
            "me",
            "2026-04-19T10:00:00Z",
            "1d ago",
        ]);
        let commit = parse_log_line(&line).expect("should parse");
        assert_eq!(commit.message, "fix: foo | bar | baz");
        assert_eq!(commit.author, "me");
        assert_eq!(commit.relative_date, "1d ago");
    }

    #[test]
    fn parse_log_line_rejects_malformed_input() {
        // Fewer than 6 fields = malformed log output, skip rather than panic.
        assert!(parse_log_line(&log_line(&["only", "three", "fields"])).is_none());
        assert!(parse_log_line("").is_none());
        assert!(
            parse_log_line(&log_line(&["h", "s", "m", "a", "d"])).is_none(),
            "5 fields = missing relative_date",
        );
    }

    #[test]
    fn parse_log_output_skips_blank_and_malformed_lines() {
        let line1 = log_line(&[
            "hash1",
            "h1",
            "msg1",
            "author1",
            "2026-04-19T10:00:00Z",
            "1h ago",
        ]);
        let line2 = log_line(&[
            "hash2",
            "h2",
            "msg2",
            "author2",
            "2026-04-19T11:00:00Z",
            "2h ago",
        ]);
        let output = format!("{}\n\nmalformed line\n{}\n\n", line1, line2);
        let commits = parse_log_output(&output);
        assert_eq!(commits.len(), 2, "blanks + malformed lines must be dropped");
        assert_eq!(commits[0].hash, "hash1");
        assert_eq!(commits[1].hash, "hash2");
    }

    #[test]
    fn parse_log_output_returns_empty_for_empty_string() {
        assert!(parse_log_output("").is_empty());
    }

    /// Owns a tempdir + its path so the dir is cleaned up when the test ends.
    /// Derefs to `Path` so call sites can use `path.to_string_lossy`,
    /// `path.join(...)`, etc. directly.
    struct TestRepo {
        path: PathBuf,
        _dir: tempfile::TempDir, // dropped when TestRepo drops, cleaning the dir
    }

    impl std::ops::Deref for TestRepo {
        type Target = Path;
        fn deref(&self) -> &Self::Target {
            &self.path
        }
    }

    /// Init a git repo in a tempdir + create `commit_count` commits with
    /// distinct messages. Returns a `TestRepo` that cleans up on drop.
    fn make_repo(name: &str, commit_count: usize) -> TestRepo {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().to_path_buf();

        // Set HOME and GIT_CONFIG_GLOBAL so the test isn't tainted by the
        // user's git config (e.g. signing keys, custom hooks).
        let isolated = path.join("isolated_home");
        fs::create_dir_all(&isolated).expect("home");
        let env = [
            ("HOME", isolated.to_string_lossy().to_string()),
            ("GIT_CONFIG_GLOBAL", "/dev/null".to_string()),
            ("GIT_CONFIG_SYSTEM", "/dev/null".to_string()),
        ];

        let run = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(&path)
                .envs(env.iter().map(|(k, v)| (*k, v.as_str())))
                .output()
                .expect("git command")
        };

        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "Test User"]);
        run(&["config", "commit.gpgsign", "false"]);

        for i in 1..=commit_count {
            let file = path.join(format!("file_{}_{}.txt", name, i));
            fs::write(&file, format!("contents {}\n", i)).expect("write");
            run(&["add", "."]);
            // Sleep 1s between commits so git's per-second timestamps differ.
            // The git log --format=%aI uses second-resolution ISO 8601.
            if i > 1 {
                std::thread::sleep(std::time::Duration::from_millis(1100));
            }
            run(&["commit", "-q", "-m", &format!("commit {} from {}", i, name)]);
        }

        TestRepo { path, _dir: dir }
    }

    /// Init a git repo in a tempdir + create one commit per entry in `dates`,
    /// pinning GIT_AUTHOR_DATE/GIT_COMMITTER_DATE so window assertions are
    /// deterministic. Returns a `TestRepo` that cleans up on drop.
    fn make_repo_with_dates(name: &str, dates: &[&str]) -> TestRepo {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().to_path_buf();

        // Set HOME and GIT_CONFIG_GLOBAL so the test isn't tainted by the
        // user's git config (e.g. signing keys, custom hooks).
        let isolated = path.join("isolated_home");
        fs::create_dir_all(&isolated).expect("home");
        let env = [
            ("HOME", isolated.to_string_lossy().to_string()),
            ("GIT_CONFIG_GLOBAL", "/dev/null".to_string()),
            ("GIT_CONFIG_SYSTEM", "/dev/null".to_string()),
        ];

        let run = |args: &[&str], extra_env: &[(&str, &str)]| {
            Command::new("git")
                .args(args)
                .current_dir(&path)
                .envs(env.iter().map(|(k, v)| (*k, v.as_str())))
                .envs(extra_env.iter().copied())
                .output()
                .expect("git command")
        };

        run(&["init", "-q", "-b", "main"], &[]);
        run(&["config", "user.email", "test@example.com"], &[]);
        run(&["config", "user.name", "Test User"], &[]);
        run(&["config", "commit.gpgsign", "false"], &[]);

        for (i, date) in dates.iter().enumerate() {
            let file = path.join(format!("file_{}_{}.txt", name, i + 1));
            fs::write(&file, format!("contents {}\n", i + 1)).expect("write");
            run(&["add", "."], &[]);
            run(
                &[
                    "commit",
                    "-q",
                    "-m",
                    &format!("commit {} from {}", i + 1, name),
                ],
                &[("GIT_AUTHOR_DATE", *date), ("GIT_COMMITTER_DATE", *date)],
            );
        }

        TestRepo { path, _dir: dir }
    }

    /// Git runs `core.fsmonitor` as a hook command during index refresh when
    /// the value is a path, so a repository's own `.git/config` is an
    /// arbitrary-command vector. The control proves the installed git honors
    /// the planted hook; the assertion proves the hardened spawn never does.
    #[cfg(unix)]
    #[test]
    fn hardened_git_ignores_a_planted_fsmonitor_hook() {
        use std::os::unix::fs::PermissionsExt;

        let repo = make_repo("fsmonitor", 1);
        let sentinel = repo.join("fsmonitor-ran.sentinel");
        let hook = repo.join("fsmonitor-hook.sh");
        fs::write(
            &hook,
            format!("#!/bin/sh\ntouch '{}'\nprintf '/'\n", sentinel.display()),
        )
        .expect("write hook");
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("chmod hook");

        // The same isolated environment make_repo used, so the user's own git
        // config never participates; the repository-local config is the vector.
        let isolated_home = repo.join("isolated_home");
        let unhardened = |args: &[&str]| {
            let output = Command::new("git")
                .args(args)
                .current_dir(&*repo)
                .env("HOME", &isolated_home)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .output()
                .expect("git command");
            assert!(
                output.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        unhardened(&["config", "core.fsmonitor", &hook.to_string_lossy()]);

        // Control: the first status writes the fsmonitor index extension, the
        // second consults the hook. If this fails the installed git does not
        // honor hook-style fsmonitor and the test proves nothing.
        unhardened(&["status", "--porcelain"]);
        unhardened(&["status", "--porcelain"]);
        assert!(
            sentinel.exists(),
            "control failed: the installed git did not run the planted core.fsmonitor hook"
        );
        fs::remove_file(&sentinel).expect("reset sentinel");

        let status = get_git_status(&repo.to_string_lossy(), 5);
        assert!(
            status.is_git_repo,
            "hardened git must still read the repository"
        );
        assert!(run_git(&repo, &["status", "--porcelain"]).is_some());
        assert!(run_git(&repo, &["log", "-1", "--format=%H"]).is_some());
        assert!(checkout_head_and_clean(&repo.to_string_lossy()).is_some());

        assert!(
            !sentinel.exists(),
            "hardened git spawn ran the repository's planted core.fsmonitor hook"
        );
    }

    #[test]
    fn hardened_command_neutralizes_config_and_rebuilds_the_environment() {
        let repo = make_repo("hardened-env", 1);
        let command = hardened_git_command(&repo, &["status", "--porcelain"]);
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let expected_prefix = [
            "--no-optional-locks",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=",
            "-c",
            "core.sshCommand=",
            "-c",
            "diff.external=",
            "-c",
            "protocol.allow=never",
            "status",
            "--porcelain",
        ];
        assert_eq!(args, expected_prefix);

        let env: std::collections::BTreeMap<String, Option<String>> = command
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert_eq!(env.get("GIT_CONFIG_NOSYSTEM"), Some(&Some("1".to_string())));
        assert_eq!(
            env.get("GIT_CONFIG_GLOBAL"),
            Some(&Some(GIT_NULL_CONFIG_FILE.to_string()))
        );
        assert_eq!(env.get("GIT_TERMINAL_PROMPT"), Some(&Some("0".to_string())));
        assert!(
            env.get("PATH").is_some(),
            "PATH must be re-added after env_clear or git cannot be found"
        );
        let allowed: std::collections::BTreeSet<&str> = GIT_INHERITED_ENV
            .iter()
            .copied()
            .chain(GIT_FIXED_ENV.iter().map(|(key, _)| *key))
            .collect();
        for key in env.keys() {
            assert!(
                allowed.contains(key.as_str()),
                "{key} leaked into the git environment"
            );
        }
    }

    #[test]
    fn get_git_status_on_non_git_dir_returns_inactive() {
        let dir = tempfile::tempdir().expect("tempdir");
        let status = get_git_status(&dir.path().to_string_lossy(), 10);
        assert!(
            !status.is_git_repo,
            "non-git dir must report is_git_repo=false"
        );
        assert!(status.commits.is_empty());
        assert!(status.branch.is_none());
        assert_eq!(status.total_commits, 0);
        assert!(!status.has_uncommitted);
    }

    #[test]
    fn get_git_status_reports_branch_and_commits() {
        let path = make_repo("status", 3);
        let status = get_git_status(&path.to_string_lossy(), 10);

        assert!(status.is_git_repo);
        assert_eq!(status.branch.as_deref(), Some("main"));
        assert_eq!(status.commits.len(), 3);
        assert_eq!(status.total_commits, 3);
        assert!(
            !status.has_uncommitted,
            "no untracked files after every add+commit"
        );

        // Newest first: commit 3 is the most recent.
        assert_eq!(status.commits[0].message, "commit 3 from status");
        assert_eq!(status.commits[2].message, "commit 1 from status");

        // Each commit has hash + short hash + author populated.
        for commit in &status.commits {
            assert_eq!(commit.author, "Test User");
            assert_eq!(commit.hash.len(), 40, "full SHA should be 40 hex chars");
            assert!(!commit.short_hash.is_empty());
            assert!(!commit.date.is_empty());
            assert!(!commit.relative_date.is_empty());
        }
    }

    #[test]
    fn get_git_status_respects_commit_limit() {
        let path = make_repo("limit", 5);
        let status = get_git_status(&path.to_string_lossy(), 2);

        // total_commits reflects the whole repo even when commits is capped.
        assert_eq!(status.total_commits, 5);
        assert_eq!(status.commits.len(), 2);
        assert_eq!(status.commits[0].message, "commit 5 from limit");
    }

    #[test]
    fn get_git_status_detects_uncommitted_changes() {
        let path = make_repo("dirty", 1);
        // Create an untracked file.
        fs::write(path.join("dirty.txt"), "uncommitted\n").expect("write");

        let status = get_git_status(&path.to_string_lossy(), 10);
        assert!(
            status.has_uncommitted,
            "untracked file must register as dirty"
        );
    }

    #[test]
    fn checkout_provenance_names_a_merge_head() {
        let path = make_repo("merge-head", 1);
        let run = |args: &[&str]| {
            let output = Command::new("git")
                .args(args)
                .current_dir(&*path)
                .output()
                .expect("git command");
            assert!(
                output.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        run(&["checkout", "-q", "-b", "feature"]);
        fs::write(path.join("feature.txt"), "feature\n").expect("feature file");
        run(&["add", "feature.txt"]);
        run(&["commit", "-q", "-m", "feature"]);
        run(&["checkout", "-q", "main"]);
        fs::write(path.join("main.txt"), "main\n").expect("main file");
        run(&["add", "main.txt"]);
        run(&["commit", "-q", "-m", "main"]);
        run(&["merge", "--no-ff", "-q", "feature", "-m", "merge"]);

        let expected = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&*path)
            .output()
            .expect("head")
            .stdout;
        let expected = String::from_utf8(expected)
            .expect("utf8 head")
            .trim()
            .to_string();
        let (head, clean) = checkout_head_and_clean(&path.to_string_lossy()).expect("checkout");

        assert_eq!(head, expected);
        assert!(clean);
        assert_ne!(
            get_git_status(&path.to_string_lossy(), 1).commits[0].hash,
            head
        );
    }

    #[test]
    fn get_commits_since_filters_by_date() {
        let path = make_repo("since", 2);
        // Capture the date of the first commit so we can ask for commits
        // strictly after it.
        let first_status = get_git_status(&path.to_string_lossy(), 10);
        // commits[1] is older (newest first), so use its date as the cut.
        let cut = &first_status.commits[1].date;

        let after = get_commits_since(&path.to_string_lossy(), cut);
        assert!(
            !after.is_empty(),
            "at least one commit should be at-or-after the cut date"
        );
        assert!(
            after.iter().any(|c| c.message == "commit 2 from since"),
            "newest commit must be present",
        );
    }

    #[test]
    fn get_commits_between_filters_to_the_window() {
        let path = make_repo_with_dates(
            "window",
            &[
                "2026-01-01T10:00:00Z",
                "2026-01-03T10:00:00Z",
                "2026-01-05T10:00:00Z",
            ],
        );

        let (commits, total) = get_commits_between(
            &path.to_string_lossy(),
            "2026-01-02T00:00:00.123456+00:00",
            "2026-01-04T00:00:00.123456+00:00",
        );

        assert_eq!(total, 1, "only the middle commit falls inside the window");
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message, "commit 2 from window");
        assert_eq!(
            total,
            commits.len() as u32,
            "rev-list-backed total must agree with the listed commits when under the cap"
        );
    }

    #[test]
    fn get_commits_between_on_non_repo_returns_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (commits, total) = get_commits_between(
            &dir.path().to_string_lossy(),
            "2026-01-01T00:00:00",
            "2026-01-02T00:00:00",
        );
        assert!(commits.is_empty(), "non-repo dir must yield no commits");
        assert_eq!(total, 0);
    }

    #[test]
    fn get_recent_commits_delegates_to_get_git_status() {
        let path = make_repo("recent", 4);
        let recent = get_recent_commits(&path.to_string_lossy(), 3);
        let status = get_git_status(&path.to_string_lossy(), 3);

        assert_eq!(recent.len(), status.commits.len());
        assert_eq!(recent.len(), 3);
        for (a, b) in recent.iter().zip(status.commits.iter()) {
            assert_eq!(a.hash, b.hash);
        }
    }

    #[test]
    fn get_recent_commits_on_non_git_dir_returns_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let commits = get_recent_commits(&dir.path().to_string_lossy(), 10);
        assert!(commits.is_empty());
    }
}

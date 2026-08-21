use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command as TokioCommand;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};
use ts_rs::TS;

use super::sanitize_error;
use crate::constants::{PROJECT_COMMAND_OUTPUT_DRAIN_TIMEOUT, PROJECT_COMMAND_TIMEOUT};

const PROJECT_COMMAND_OUTPUT_LIMIT: usize = 12_000;

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "ipc-bindings.ts")]
pub struct DesktopCommandResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

fn trimmed_output(value: &[u8]) -> String {
    let text = String::from_utf8_lossy(value).trim().to_string();
    if text.len() <= PROJECT_COMMAND_OUTPUT_LIMIT {
        return text;
    }
    let mut truncated = text
        .chars()
        .take(PROJECT_COMMAND_OUTPUT_LIMIT)
        .collect::<String>();
    truncated.push_str("\n…output truncated…");
    truncated
}

fn append_truncation_marker(mut value: String, was_truncated_while_reading: bool) -> String {
    if was_truncated_while_reading && !value.ends_with("…output truncated…") {
        if !value.is_empty() {
            value.push('\n');
        }
        value.push_str("…output truncated…");
    }
    value
}

async fn read_limited_output<R>(mut reader: R) -> Result<(Vec<u8>, bool), std::io::Error>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::with_capacity(PROJECT_COMMAND_OUTPUT_LIMIT.min(4096));
    let mut buffer = [0_u8; 4096];
    let mut truncated = false;

    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let remaining = PROJECT_COMMAND_OUTPUT_LIMIT.saturating_sub(output.len());
        if remaining == 0 {
            truncated = true;
            continue;
        }
        let to_copy = remaining.min(read);
        output.extend_from_slice(&buffer[..to_copy]);
        if to_copy < read {
            truncated = true;
        }
    }

    Ok((output, truncated))
}

async fn collect_project_command_output(
    mut task: JoinHandle<Result<(Vec<u8>, bool), std::io::Error>>,
    drain_timeout: Duration,
) -> Result<(Vec<u8>, bool), String> {
    tokio::select! {
        result = &mut task => result.map_err(sanitize_error)?.map_err(sanitize_error),
        _ = sleep(drain_timeout) => {
            task.abort();
            Ok((Vec::new(), true))
        }
    }
}

pub(crate) fn parse_project_command(command: &str) -> Result<(String, Vec<String>), String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err("No command provided".into());
    }
    if trimmed.chars().any(|ch| matches!(ch, '\n' | '\r' | '\0')) {
        return Err("Multi-line commands are not supported".into());
    }

    let parts = shlex::split(trimmed)
        .ok_or_else(|| "Command has unmatched quotes or unsupported shell syntax".to_string())?;
    if parts.is_empty() {
        return Err("No command provided".into());
    }
    if parts.iter().any(|part| {
        matches!(
            part.as_str(),
            "&&" | "||" | "|" | ";" | "<" | ">" | ">>" | "<<" | "&" | "&>" | "1>" | "2>"
        ) || part.starts_with('>')
            || part.starts_with('<')
    }) {
        return Err("Shell chaining and redirection are not supported".into());
    }

    let executable = parts[0].clone();
    let args = parts.into_iter().skip(1).collect();
    Ok((executable, args))
}

const ALLOWED_EXECUTABLES: &[&str] = &[
    "npm", "pnpm", "yarn", "bun", "cargo", "go", "composer", "drush", "wp", "bundle", "gem",
];

fn has_flag(args: &[String], names: &[&str]) -> bool {
    args.iter().any(|arg| {
        names.iter().any(|name| {
            if arg == name {
                return true;
            }
            if name.contains('=') {
                return false;
            }
            arg.strip_prefix(name)
                .map(|suffix| matches!(suffix, "=true" | "=1"))
                .unwrap_or(false)
        })
    })
}

fn has_all_flags(args: &[String], names: &[&str]) -> bool {
    names
        .iter()
        .all(|name| has_flag(args, std::slice::from_ref(name)))
}

fn installer_requires_script_opt_out(
    executable: &str,
    command: &str,
) -> Option<&'static [&'static str]> {
    match executable {
        "npm" if matches!(command, "install" | "i" | "add" | "ci" | "update" | "up") => {
            Some(&["--ignore-scripts"])
        }
        "pnpm" if matches!(command, "install" | "i" | "add" | "update" | "up") => {
            Some(&["--ignore-scripts"])
        }
        "yarn" if matches!(command, "install" | "add" | "upgrade" | "up") => {
            Some(&["--ignore-scripts", "--mode=skip-builds"])
        }
        "bun" if matches!(command, "install" | "add" | "update" | "upgrade") => {
            Some(&["--ignore-scripts"])
        }
        _ => None,
    }
}

fn installer_must_run_manually(executable: &str, command: &str) -> bool {
    matches!(
        (executable, command),
        ("cargo", "install")
            | ("gem", "install" | "update")
            | ("bundle", "install" | "update")
            | ("go", "install")
    )
}

fn package_manager_script_alias_must_run_manually(executable: &str, args: &[String]) -> bool {
    let command = args.first().map(String::as_str).unwrap_or_default();
    let subcommand = args.get(1).map(String::as_str).unwrap_or_default();
    match executable {
        "npm" => matches!(
            command,
            "build"
                | "exec"
                | "explore"
                | "rebuild"
                | "restart"
                | "run"
                | "start"
                | "stop"
                | "test"
                | "uninstall"
                | "remove"
                | "x"
        ),
        "pnpm" => {
            const SAFE_COMMANDS: &[&str] = &["add", "i", "install", "up", "update"];
            matches!(
                command,
                "approve-builds"
                    | "build"
                    | "dlx"
                    | "exec"
                    | "rebuild"
                    | "restart"
                    | "run"
                    | "start"
                    | "stop"
                    | "test"
            ) || (!command.is_empty() && !SAFE_COMMANDS.contains(&command))
        }
        "yarn" => {
            const SAFE_COMMANDS: &[&str] = &["add", "install", "up", "upgrade"];
            matches!(
                command,
                "build" | "dlx" | "exec" | "node" | "rebuild" | "run" | "start" | "test"
            ) || (!command.is_empty() && !SAFE_COMMANDS.contains(&command))
        }
        "bun" => {
            const SAFE_COMMANDS: &[&str] = &["add", "install", "update", "upgrade"];
            matches!(
                command,
                "build" | "exec" | "rebuild" | "run" | "start" | "test" | "x"
            ) || (command == "pm" && subcommand == "trust")
                || (!command.is_empty() && !SAFE_COMMANDS.contains(&command))
        }
        _ => false,
    }
}

pub(crate) fn validate_project_command_policy(
    executable: &str,
    args: &[String],
) -> Result<(), String> {
    if !ALLOWED_EXECUTABLES.contains(&executable) {
        return Err(format!(
            "Executable '{}' is not in the allowed list for project commands.",
            executable
        ));
    }

    let first = args.first().map(String::as_str).unwrap_or_default();
    if first.starts_with('-') {
        return Err(
            "Project commands must put the command name before flags so SiteCMD can verify it."
                .into(),
        );
    }

    let blocked = match executable {
        "npm" | "pnpm" | "yarn" | "bun" => {
            package_manager_script_alias_must_run_manually(executable, args)
        }
        "cargo" => matches!(first, "run"),
        "go" => matches!(first, "run" | "generate"),
        "composer" => matches!(first, "exec" | "run" | "run-script"),
        "bundle" => matches!(first, "exec"),
        "gem" => matches!(first, "exec"),
        "wp" => matches!(first, "eval" | "eval-file" | "shell"),
        "drush" => matches!(first, "php:eval" | "ev" | "script" | "scr"),
        _ => false,
    };

    if blocked {
        return Err(format!(
            "'{} {}' is not allowed from the app. Run it manually in a terminal if you trust it.",
            executable, first
        ));
    }

    if let Some(required_flags) = installer_requires_script_opt_out(executable, first) {
        if !has_flag(args, required_flags) {
            return Err(format!(
                "'{} {}' can run dependency lifecycle scripts. Add {} or run it manually in a terminal if you trust it.",
                executable,
                first,
                required_flags.join(" or ")
            ));
        }
    }

    if executable == "composer"
        && matches!(first, "install" | "update" | "require" | "remove")
        && !has_all_flags(args, &["--no-scripts", "--no-plugins"])
    {
        return Err(format!(
            "'composer {}' can execute dependency scripts or Composer plugins. Add --no-scripts and --no-plugins, or run it manually in a terminal if you trust the project.",
            first
        ));
    }

    if installer_must_run_manually(executable, first) {
        return Err(format!(
            "'{} {}' can execute third-party build/install code. Run it manually in a terminal if you trust it.",
            executable, first
        ));
    }

    Ok(())
}

pub(crate) fn resolve_registered_project_target(
    projects: &[crate::db::ProjectRecord],
    path: &str,
) -> Result<PathBuf, String> {
    let target = PathBuf::from(path)
        .canonicalize()
        .map_err(|_| "Path does not exist".to_string())?;
    let in_project = projects
        .iter()
        .filter(|project| !project.path.is_empty())
        .any(|project| {
            PathBuf::from(&project.path)
                .canonicalize()
                .map(|root| target.starts_with(root))
                .unwrap_or(false)
        });
    if !in_project {
        return Err("Path is outside any registered project directory.".into());
    }
    Ok(target)
}

pub(crate) async fn run_project_command_process(
    executable: String,
    args: Vec<String>,
    working_dir: &Path,
) -> Result<DesktopCommandResult, String> {
    run_project_command_process_with_timeouts(
        executable,
        args,
        working_dir,
        PROJECT_COMMAND_TIMEOUT,
        PROJECT_COMMAND_OUTPUT_DRAIN_TIMEOUT,
    )
    .await
}

async fn run_project_command_process_with_timeouts(
    executable: String,
    args: Vec<String>,
    working_dir: &Path,
    command_timeout: Duration,
    output_drain_timeout: Duration,
) -> Result<DesktopCommandResult, String> {
    let mut process = TokioCommand::new(executable);
    let mut child = process
        .args(args)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(sanitize_error)?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture command stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture command stderr".to_string())?;
    let stdout_task = tokio::spawn(read_limited_output(stdout));
    let stderr_task = tokio::spawn(read_limited_output(stderr));

    let (exit_code, success, timed_out) = match timeout(command_timeout, child.wait()).await {
        Ok(Ok(status)) => (status.code(), status.success(), false),
        Ok(Err(error)) => return Err(sanitize_error(error)),
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            (None, false, true)
        }
    };

    let stdout = collect_project_command_output(stdout_task, output_drain_timeout).await?;
    let stderr = collect_project_command_output(stderr_task, output_drain_timeout).await?;
    let stdout_text = append_truncation_marker(trimmed_output(&stdout.0), stdout.1);
    let mut stderr_text = append_truncation_marker(trimmed_output(&stderr.0), stderr.1);
    if timed_out {
        if !stderr_text.is_empty() {
            stderr_text.push('\n');
        }
        stderr_text.push_str("Command timed out after 120 seconds and was stopped.");
    }

    Ok(DesktopCommandResult {
        exit_code,
        stdout: stdout_text,
        stderr: stderr_text,
        success,
    })
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::run_project_command_process_with_timeouts;

    #[cfg(unix)]
    #[tokio::test]
    async fn security_regression_project_command_output_reader_has_timeout() {
        use std::time::{Duration, Instant};

        let temp = tempfile::tempdir().expect("temp dir");
        let started = Instant::now();

        let result = run_project_command_process_with_timeouts(
            "sh".to_string(),
            vec![
                "-c".to_string(),
                "sleep 5 & echo parent-finished".to_string(),
            ],
            temp.path(),
            Duration::from_secs(1),
            Duration::from_millis(100),
        )
        .await
        .expect("command should complete without waiting for inherited output pipes");

        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(result.stdout.contains("output truncated"));
    }
}

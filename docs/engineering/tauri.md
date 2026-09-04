---
description: "SiteCMD-specific Tauri engineering rules for keeping the desktop app secure, maintainable, and easy for humans or agents to extend."
globs: "**/*.(rs|js|ts|tsx|json|toml|md)"
---

# SiteCMD Tauri Engineering Guide

This guide is for engineers and AI agents changing the desktop app. It is not a generic Tauri tutorial. It documents the rules that keep SiteCMD's local-first desktop app safe while it talks to local project files, credentials, scans, and external services.

## Operating Model

Treat the main renderer as untrusted.

The React UI is allowed to ask for work. Rust owns secrets, filesystem validation, project command execution, database writes, redaction, and any operation that could affect the user's machine or account.

Core expectations:

- Keep API keys and OAuth tokens in the OS keychain through Rust.
- Keep SQLite access behind Rust commands.
- Keep third-party API calls in Rust so credentials never need to enter frontend code.
- Keep destructive actions and filesystem writes behind native validation and confirmation.
- Keep secrets and log-sensitive values redacted before they reach the frontend.

## Adding A Command

When adding a Tauri command:

1. Put the command in the appropriate Rust command module.
2. Re-export it through the command module index if needed.
3. Register it in the Tauri invoke handler.
4. Add the command name to the app command list used by the build script.
5. Place the generated permission in the narrowest permission set that should expose it.
6. Add tests or guardrails for any security-sensitive behavior.

Do not default to the main window. The question is always: "What is the smallest renderer surface that needs this?"

## Permission Groups

Ordinary read/workflow commands can live in the baseline permission set when they do not expose secrets, local filesystem access, destructive data operations, or process execution.

Elevated command families must stay feature-scoped:

- Data administration: import/export database, destructive project/history/delete operations.
- Filesystem export: writes selected export files or database backups.
- Filesystem access: reads project-local files, resolves paths, opens editors, reveals files, reads logs, and runs Code Scan.
- External connectors: OAuth, analytics/search/deploy/uptime integrations, license/update checks, and webhook configuration.
- Project execution: running project commands such as test/build/lint from a linked project folder.

These families use hidden privileged bridge windows plus Rust brokers. The main window should not receive direct elevated broker permissions.

## Privileged Bridge Rules

The privileged bridge exists to keep elevated command families out of the ordinary renderer permission set.

Rules:

- Tokens are short-lived and bound to the broker command, inner command, and argument signature.
- Hidden bridge windows receive only the capability for their own elevated family.
- Rust still validates the requested command and arguments. The bridge is not a substitute for server-side checks.
- Destructive or filesystem-writing actions still need native confirmation or a user-intent proof.
- A new elevated command must be added to the broker command list, the bridge map, and the command-security manifest together.

The long-term direction is narrower user-intent tokens. Avoid adding broad issuer permissions unless there is no smaller option.

## Frontend Invoke Rules

Frontend code should call the shared invoke wrapper, not the raw Tauri invoke API directly.

The shared wrapper routes elevated commands through the privileged bridge and keeps the call shape consistent for tests. If a command needs special routing, teach the wrapper instead of bypassing it in a component.

Do not call plugin APIs directly from the renderer for keyring, updater install/download, filesystem writes, or secret-bearing work. Use Rust commands.

Application preferences use `get_app_setting` and `set_app_setting`. Rust fixes the store path to the existing app-data `settings.json`; renderer arguments select only a key within that file. Keep store plugin permissions out of renderer capabilities, including load, get, set, and save.

## Code Scan Payloads

Code Scan is a Rust-owned local-project audit engine. It produces `CodeScanReport` and `CodeIssue` data.

The complete Code Scan payload is part of the free local workbench. Summary
counts, issue detail, evidence, fix guides, AI prompts, reports, and dossier
data must not vary by subscription tier. Rust still sanitizes secrets, unsafe
paths, and log-sensitive values before returning or persisting them.

## Filesystem And Project Commands

Filesystem access must be scoped to the user's linked project or a user-selected export path.

Rules:

- Validate paths in Rust.
- Prefer native file/folder pickers for user-chosen paths.
- Do not trust renderer-provided absolute paths without server-side checks.
- Strip or redact query strings, fragments, tokens, and secret-looking data before logging.
- Project command execution must use an allowlisted command shape and native confirmation.

## HTTP And External Services

Use the shared Rust HTTP clients. Do not create ad-hoc HTTP clients inside individual modules.

Rules:

- Production URLs verify TLS.
- Localhost support is explicit and isolated.
- Redirect behavior should be intentional per check.
- Credentials stay in Rust.
- Public errors should be useful without leaking provider responses, tokens, emails, or secret-bearing URLs.

## Logging

Logs are useful, but this app handles private URLs, local paths, webhooks, and credentials.

Rules:

- Never log raw webhook URLs.
- Never log full target URLs when query strings or fragments may contain secrets.
- Redact emails, tokens, signed URLs, and provider error bodies before logging.
- Prefer short fingerprints when a value needs to be correlated across logs.

## Testing And Guardrails

Security-sensitive patterns need automated protection.

When changing the Tauri surface, run the focused Rust tests, frontend invoke tests, and repo guardrails. Add or update guardrails when a rule is easy to regress by accident, especially:

- command registration drift
- direct raw invoke bypasses
- elevated capabilities mounted on the main window
- broad plugin defaults on the main window
- unsafe filesystem writes
- raw secret-bearing URL logging
- subscription checks used to hide local finding detail

## Review Checklist

Before shipping a desktop security or Tauri change, confirm:

- The renderer cannot read secrets directly.
- The renderer cannot call destructive commands directly.
- Elevated work is scoped to the smallest command family.
- Rust validates every path, command, token, and hosted entitlement that matters.
- Logs do not contain secret-bearing URLs, tokens, or raw provider payloads.
- Tests or repo guardrails catch the regression this change was meant to prevent.

# Privileged broker threat model

**Status:** Accepted 2026-08-23. Written alongside the desktop Rust security
P3 simplification backlog, item 13f.

**Audience:** Whoever adds a command to a privileged broker scope, reviews a
change to `src/commands/privileged_command_broker/`, or is deciding whether a
new command needs native user-intent confirmation.

**After reading:** A reader should be able to say, for any command reachable
through the privileged bridge, what the broker actually stops, what it does
not stop, and where the boundary between the two sits.

`docs/engineering/tauri.md` describes the bridge's mechanics: hidden windows
carry only the capability for their own elevated family, tokens are
argument-bound and single-use, and destructive commands need native
confirmation. This document is about the mechanics' limits, not a repeat of
how they work.

## What the broker stops

The main renderer - the window a user actually looks at and where arbitrary
page content, extensions, or a supply-chain-compromised dependency are most
likely to run untrusted script - never holds a broker permission directly.
Elevated command families live behind their own hidden bridge window, each
scoped to exactly the commands that family needs. A bug or an XSS payload in
the main window cannot call `run_data_admin_command` at all; the Tauri
capability manifest never grants it that permission.

Within a bridge window, a token is required before dispatch, and the token
is bound to the exact `(broker, command, argument signature)` triple by
`BrokerScope::admit` (`src/commands/privileged_command_broker/mod.rs`).
Reusing a token, replaying it against different arguments, or presenting it
to a different broker all fail. Tokens expire after 15 seconds
(`PRIVILEGED_COMMAND_TOKEN_TTL`,
`src/commands/privileged_command_broker/token_state.rs:43-101`), so a token
minted for one dispatch cannot be stockpiled.

## What the broker does not defend against

**A compromised renderer inside an already-privileged bridge window.** The
broker's unit is the bridge window, not the individual command. Once a
renderer is running inside, say, the filesystem-access bridge window, it can
call that window's own scoped issuer
(`issue_filesystem_access_command_token`) to mint a token for any command in
`FILESYSTEM_ACCESS_COMMANDS` that is not in
`SENSITIVE_FILESYSTEM_ACCESS_COMMANDS`, with no human in the loop and no
native dialog. `src/commands/privileged_command_broker/filesystem_access.rs:18-36`
lists that scope's full allowlist; `run_code_scan_audit`,
`resolve_project_files`, `read_recent_logs`, and `get_git_status` are in it
and not in the sensitive list, so a compromised filesystem-access renderer
can self-mint tokens for all four and read a linked project's source,
recent logs, and git history without the user seeing a prompt. The broker
was designed to keep an elevated command out of a renderer that has no
business calling it, not to survive a renderer that has already been
compromised inside a scope that legitimately needs the command.

**Argument bounds, not argument meaning.** The token binds a command to the
exact bytes of its arguments (`args_signature`,
`src/commands/privileged_command_broker/token_state.rs`), so a stolen token
cannot be replayed with different arguments. It says nothing about whether
those arguments are safe; each dispatcher still validates its own inputs
(a path stays inside the linked project, a command string is not
reinterpreted as shell syntax) as a separate concern.

## Scopes

| Broker command                   | Label               | Sensitive commands (native user intent)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Confirmation path                                                                                                                                                                                        |
| -------------------------------- | ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `run_data_admin_command`         | data administration | none                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | Every dispatched command confirms natively inside its own handler; the token issuer confirms nothing.                                                                                                    |
| `run_external_connector_command` | external connector  | `save_integration`, `save_webhook_config`, `test_webhook`, `sync_connected_site`, `import_connected_connection`, `export_connected_connection`, `unlink_connected_site`, `disconnect_connected_site`, `erase_connected_site`, `create_connected_alert_webhook`, `test_connected_alert_webhook`, `delete_connected_alert_webhook`, `create_connected_destination`, `resend_connected_destination_verification`, `delete_connected_destination`, `revoke_connected_site_credential`, `revoke_connected_provider_connection`, `revoke_connected_report` | The scoped issuer refuses to issue a token for these; the frontend must go through `issue_sensitive_privileged_command_token`, which shows a native dialog with purpose-written copy before minting one. |
| `run_filesystem_access_command`  | filesystem access   | `update_project_path`, `open_path_in_editor`, `reveal_path`, `register_agent_tool`, `unregister_agent_tool`, `launch_agent_handoff`                                                                                                                                                                                                                                                                                                                                                                                                                  | Same shared sensitive-issuer path as above.                                                                                                                                                              |
| `run_filesystem_export_command`  | filesystem export   | none                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | Every dispatched command confirms natively inside its own handler (writing the destination file itself is the confirming action).                                                                        |
| `run_project_execution_command`  | project execution   | none                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | `run_project_command` confirms natively inside the handler before running anything.                                                                                                                      |

`SENSITIVE_CONNECTOR_COMMANDS` and `SENSITIVE_FILESYSTEM_ACCESS_COMMANDS`
(`src/commands/privileged_command_broker/mod.rs`) are the source of truth for
the sensitive columns above; `BrokerScope::admit` and
`privileged_token_issue_requires_user_intent` both read from the same
`SCOPES` table, so the two cannot drift from each other. This document can
still drift from the table, which is why `lib_tests.rs` asserts every
broker command and every sensitive command name from `SCOPES` appears in
this file.

## Decision

`run_code_scan_audit` stays off the native-intent list. It reads project
source and prepares an audit; it does not exfiltrate, delete, or mutate
anything the user has not already exposed to Code Scan through the same
window's `run_scan_execution`. Requiring a native dialog for every Code
Scan invocation - which the filesystem-access renderer calls routinely as
part of normal, expected use - would train users to click through the
dialog without reading it, which is worse than not prompting at all. The
default recorded here is to document the gap above, not to prompt for it.
Revisit this if `run_code_scan_audit`'s payload grows to include something
a native dialog would meaningfully let a user refuse.

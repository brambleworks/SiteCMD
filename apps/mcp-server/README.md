# sitecmd-mcp

MCP server for [SiteCMD](https://sitecmd.com) - lets AI coding tools read scan results and fix issues directly.

## What it does

This MCP server gives your AI coding tool (Cursor, Claude Code, Windsurf, etc.) direct access to your SiteCMD scan results. Your AI can:

- See your latest scan artifact score and category breakdown
- List all failing issues with severity and descriptions
- Get ready-to-use fix prompts for each issue
- Track scan artifact score history over time

## Setup

The SiteCMD desktop app registers this server for you: open Integrations and
connect your agent tool (Claude Code, Cursor, Codex). It writes a config that
runs the server bundled inside the app via your local Node, so there is nothing
to install separately.

SiteCMD does not treat a matching config key as proof that the connection
works. It compares the full command, arguments, and database environment, then
runs a bounded read-only health check against the configured server and
database. A stale path, old arguments, missing Node runtime, unreadable
database, or startup failure appears as **Repair**, with the detected reason;
it is never shown as connected until that probe succeeds.

Manual setup (rarely needed) requires Node.js 22.22.1+. The desktop copies the
server into persistent application data each time it starts. Point your agent
at that stable copy, not the app bundle or installation directory.

| OS      | Persistent MCP script                                                                                                                         |
| ------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS   | `~/Library/Application Support/com.sitecmd.app/sitecmd-mcp/sitecmd-mcp.mjs`                                                                   |
| Linux   | `$XDG_DATA_HOME/com.sitecmd.app/sitecmd-mcp/sitecmd-mcp.mjs` when set; otherwise `~/.local/share/com.sitecmd.app/sitecmd-mcp/sitecmd-mcp.mjs` |
| Windows | `%LOCALAPPDATA%\com.sitecmd.app\sitecmd-mcp\sitecmd-mcp.mjs`; `%APPDATA%` is used when `%LOCALAPPDATA%` is unavailable                        |

Expand the home directory or environment variable to an absolute path before
putting it in JSON; agent configuration files do not expand those placeholders.

```json
{
  "mcpServers": {
    "sitecmd": {
      "command": "node",
      "args": ["/absolute/path/to/com.sitecmd.app/sitecmd-mcp/sitecmd-mcp.mjs"]
    }
  }
}
```

## Requirements

- **SiteCMD** must be installed and have run at least one scan
- **Node.js** 22.22.1+ for manual setup

SiteCMD requires a maintenance release whose built-in `node:sqlite` runtime
passes the server's full test suite. Its automatic connection flow verifies the
Node version and SQLite support before writing agent configuration. The app does
not need to be running after setup.

The bundled MCP server follows the desktop and CLI release version. Its package
is private and is released only as a desktop resource, so the MCP handshake,
package metadata, and SiteCMD release are bumped together.

## Tools

| Tool                   | Description                                                       |
| ---------------------- | ----------------------------------------------------------------- |
| `get_projects`         | List projects with ids, URLs, frameworks, and linked folders      |
| `get_scan_score`       | Get latest scan artifact score and category breakdown             |
| `get_issues`           | Get failing issues, filterable by severity/category               |
| `get_fix_prompts`      | Get AI-ready fix prompts for each issue                           |
| `get_scan_history`     | Get scan artifact score history over time                         |
| `get_dismissed_issues` | List issues dismissed or marked not applicable                    |
| `compare_scans`        | Compare two web scans by id (default: the two most recent)        |
| `request_scan`         | Return guidance for running a scan manually and comparing results |
| `get_fix_brief`        | Get the fix brief for a fix attempt, with acceptance criteria     |
| `request_verification` | Tell SiteCMD a fix is done so it can re-run the check and verify  |
| `list_fix_attempts`    | List currently open fix attempts                                  |

### Correlation tools

These read v3-enriched correlation data and are all read-only.

| Tool                      | Description                                                          |
| ------------------------- | -------------------------------------------------------------------- |
| `get_active_correlations` | Active issue groups with causes, effects, events, and anomaly scores |
| `get_recent_events`       | Site events tied to check IDs within the last N days                 |
| `get_likely_causes`       | Direct and transitive likely causes for a check ID                   |
| `get_causal_graph`        | Active causal graph as a node-link payload for visualization         |
| `preview_deploy_risk`     | Predict which active issues may regress from a set of changed files  |
| `whatif_resolve`          | Downstream effects of hypothetically resolving a set of issues       |

Every correlation tool accepts `project_id` or `url`.

## Example usage

Once connected, just ask your AI:

- "What's my latest scan artifact score?"
- "Show me the critical security issues on my site"
- "Fix the CSP header issue on example.com"
- "What issues should I fix first to improve my SiteCMD score?"

## Configuration

The server auto-detects the SiteCMD database location:

| OS      | Path                                                       |
| ------- | ---------------------------------------------------------- |
| macOS   | `~/Library/Application Support/com.sitecmd.app/sitecmd.db` |
| Linux   | `~/.local/share/com.sitecmd.app/sitecmd.db`                |
| Windows | `%LOCALAPPDATA%/com.sitecmd.app/sitecmd.db`                |

On Linux, `$XDG_DATA_HOME` replaces `~/.local/share` when set. On Windows, the
server falls back to `%APPDATA%` when `%LOCALAPPDATA%` is unavailable. Override
the resolved path with the `SITECMD_DB_PATH` environment variable if needed.

## Recovery

The MCP server is read-mostly. Its only writes are bounded updates to existing
fix-attempt rows: `get_fix_brief` records the first time a brief is fetched, and
`request_verification` records the agent summary and asks SiteCMD to verify the
attempt. Neither operation can create rows or touch another table. If the
database needs backup or restore during an incident, follow the recovery
runbook (`apps/mcp-server/recovery-runbook.md` in the SiteCMD repository).

## License

Apache-2.0. See the repository root `LICENSE` file.

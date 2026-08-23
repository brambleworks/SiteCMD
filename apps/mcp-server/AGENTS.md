# AGENTS.md

Maintainer guidance for the private `sitecmd-mcp` package bundled with the
desktop app. User-facing setup, database-path resolution, and the current tool
table live in `README.md`.

## Commands

```bash
pnpm --filter sitecmd-mcp run typecheck
pnpm --filter sitecmd-mcp run test
pnpm --filter sitecmd-mcp run build
pnpm --filter sitecmd-mcp run dev
pnpm --filter sitecmd-mcp run bundle
```

Tests execute built output from `dist/`. Rebuild before running an individual
Node test, or use the package test command.

## Database boundary

The server reads the desktop SQLite database through Node's built-in
`node:sqlite`. It does not own the schema or run migrations.

Two modules may use the write connection. `db_fix_attempts`: `get_fix_brief`
may stamp the first brief fetch, and `request_verification` may update the
allowed fields of an existing `fix_attempts` row through `getDbWrite`.
`db_agent_requests`: `start_fix` and `run_scan` each insert one row into the
`agent_requests` queue; the desktop's own watcher claims and fulfils it and is
the only writer of its `status`, `result_json`, and `failure_detail` columns.
Neither module can create a row in, or touch, any other table. Do not add
another write path. Compute derived tool output from reads.

Database modules are coupled to the generated desktop schema snapshot. Tests
seed from `test/helpers/schema-fixture.mjs`; handwritten schema fixtures are
banned except the missing-table degradation fixture. The schema guardrail also
checks SQL literals and restricts the write connection to the fix-attempt and
agent-request modules.

Fail clearly when a database is too old or too new. Do not partially interpret
an incompatible schema. Issue lifecycle comes only from
`project_issue_states`.

## Generated assets

Do not hand-edit these JSON files:

- `src/causal_graph.json`;
- `src/fix_locations.json`;
- `src/impact_score.json`; and
- `src/license_constants.json`.

Rust parity tests regenerate and verify them. The build copies the assets beside
the JavaScript output, and the bundle includes them beside the MCP entry point.

## Workspace and tools

`src/workspace.ts` matches the caller's working directory against project roots
stored by the desktop. Investigate it first when tools select the wrong project.

Core tools read projects, scores, issues, prompts, history, dismissed state,
scan comparisons, and fix attempts. `how_to_rescan` (alias `request_scan`)
stays guidance; `run_scan` queues a desktop scan through `agent_requests` and
`get_scan_status` reads it back. `start_fix` and `get_fix_status` do the same
for fix attempts. These tools use only the bounded writes described above:
guarded updates to `fix_attempts` and inserts into `agent_requests`.

Correlation tools are registered in `src/correlation_tools.ts`. They read
Rust-computed groups and events, then walk generated graph and fix-location
assets with algorithms kept in parity with the Rust resolver. They do not
recompute or persist correlation truth.

Tool names are downstream public API. Renaming or removing a tool requires a
major package version and migration guidance. Adding a tool requires updating
the user-facing tool table, tests, and database-access guardrails.

## Distribution

The package is private and is not published to npm. `scripts/bundle.mjs`
produces the single bundled MCP entry point and copies generated assets into the
desktop resources. The desktop registers that resource with supported coding
agents.

Do not add `bin` or public package files, native runtime dependencies, or a
second distribution path without updating the release and licensing contract.

## References

- Root `../../AGENTS.md`: repository-wide rules.
- `README.md`: end-user setup and current tools.
- `recovery-runbook.md`: database recovery.

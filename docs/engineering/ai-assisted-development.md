# AI-assisted development

SiteCMD is built by one maintainer directing AI coding agents. That is stated
here rather than left to be inferred, because it changes what a reader should
look for. The useful question is not whether an agent wrote a given function; it
is whether the repository can tell the difference between a change that holds
and one that only looks finished. This document points at the machinery that
answers that question. All of it runs in this repository and can be read, run,
and disagreed with.

## How agents are directed

Guidance lives in `AGENTS.md` files, one per surface, read before editing that
surface:

- `AGENTS.md` (root) - product contract, repository map, commands, Tauri IPC
  registration rules, data and scoring invariants, the shared-source list, and
  the code and comment style rules.
- `apps/desktop/AGENTS.md` - app shell, navigation state, component and styling
  rules.
- `apps/desktop/src-tauri/AGENTS.md` - the database worker thread, migrations,
  the command surface, and crate layout.
- `apps/mcp-server/AGENTS.md` - the read boundary against the desktop database
  and the tool contract.

The `CLAUDE.md` beside each one is a thin pointer to its `AGENTS.md`, so a rule
has a single home no matter which tool reads it.

The rules that matter most are the ones a type checker cannot state. The root
guide's shared-sources list names the one authority for time formatting,
severity ordering, score bands, HTTP client construction, outbound URL policy,
audit logging, and webhook signing, because the characteristic failure of an
agent is a plausible second copy rather than a syntax error.

Guidance is itself checked. `tools/scripts/audit/check-agents-md.mjs` runs in
the push gate as the `agents-md` check and fails when an `AGENTS.md` cites a
file path, a `pnpm` script, or a Tauri command that does not exist, so guidance
cannot rot into confident false statements.
`tools/scripts/lib/guardrail-agent-guidance-rules.mjs` requires the per-surface
guide and its pointer to exist, keeps the styling instructions aimed at the
current CSS layout, and caps guidance prose line length.

## What stands in for a second reviewer

There is one maintainer, so there is no second human reviewer.
[GOVERNANCE.md](../../GOVERNANCE.md) says so directly: automated review and
deterministic checks add scrutiny but are not represented as independent human
approval. What follows is what that automation is.

`tools/scripts/check-repo-guardrails.mjs` imports ninety rule modules from
`tools/scripts/lib/`, several of which compose further modules. Each exports
functions that return failure strings, and the documented expectation is that
every rule carries a self-test planting the regression it claims to catch
(`pnpm guardrails:repo:test`). [guardrails.md](guardrails.md) covers the rule
shapes and how to add one. The categories that bear directly on agent-written
work:

- **Copy tone.** `guardrail-machine-smell-copy-rules.mjs` bans the upgrade verb
  "unlock" and the word "queue" from visible product copy, matching only inside
  quoted spans and JSX text so identifiers are not caught.
- **Comment quality.** `guardrail-comment-quality-rules.mjs` extracts comments
  from TypeScript, CSS, and Rust without matching comment syntax inside string
  literals, then rejects filler such as "basically", "essentially", "note that",
  and "as we can see". It also rejects comments that cite maintainer-only
  tooling absent from this repository as a design authority.
- **Punctuation.** `guardrail-em-dash-rules.mjs` bans U+2014 across maintained
  docs, per-app guides, tooling scripts, the Rust and desktop sources, and the
  MCP server. The same-line `allow-em-dash` marker is reserved for a line where
  the character is the thing being detected.
- **Publication hygiene.** `publication-hygiene-rules.mjs` pins the required
  public files, allows a fixed set of root files and directories, and rejects
  agent session directories such as `.claude`, `.codex`, `.cursor`, and
  `.superpowers` at any depth, so working artifacts never become repository
  content.
- **Line budgets.** Hot files carry a maximum line count, and
  `tools/scripts/check-budget-ratchet.mjs` blocks any commit that raises a
  budget or grows a `*_LIMIT` without an audited `[budget-raised: ...]` trailer.
  It runs as a `commit-msg` hook and as a CI gate. The documented default is to
  split the file instead.

## Where the gates run

`lefthook.yml` defines three hook tiers. Pre-commit runs typecheck, ESLint,
Prettier, `cargo fmt`, and gitleaks over staged files. Commit-msg runs the
message-format check and the budget ratchet. Pre-push runs `pnpm verify:push`.

`tools/scripts/verify-push.mjs` runs 41 checks in seven tiers, ordered from
static analysis toward builds and browser tests. The first tier holds
typecheck, lint, formatting, dependency and license audits, secret scanning,
`pnpm guardrails:repo`, and the `agents-md` staleness check. Later tiers run
the JavaScript suites, then Rust tests, clippy, the declared MSRV, and the wasm
targets, then performance baselines, the desktop build, size limits, and
Playwright. `pnpm verify:push:all` reports every failing tier in one run rather
than stopping at the first.

`pnpm guardrails:repo` is one command over four runners: the rule fleet,
repository label and protection contracts, and publication hygiene.

On GitHub, `main` is governed by the contract in
`.github/repository-protection.json`: pull requests required, linear history,
signed commits, no deletion or force push, and five required status checks.
Merges are squashes, so each merged pull request is one reviewable commit.
`pnpm protection:check` verifies that every required check names a job in a
workflow that runs on every pull request, and `--live` compares the contract
against the repository's actual settings.

## The product audits this repository

SiteCMD's Code Scan looks for what fast, assisted development leaves behind. The
`placeholder-density` check measures the density of `TODO`, `FIXME`, `HACK`,
`XXX`, `CHANGEME`, and `PLACEHOLDER` markers. The `ai-scaffolding` category in
`apps/desktop/src-tauri/src/core/code_scan/ai_scaffolding.rs` reads a project's
own agent instruction files and reports four things: a guide with almost no
meaningful content, several substantive instruction files where none points at
another, a credential-shaped literal in an instruction or MCP configuration
file, and a superseded single-file rule format. The second of those is the check
this repository's own `CLAUDE.md` pointers satisfy.

The same scanner runs against this repository.
`.github/workflows/app-guardrails.yml` builds the CLI and runs
`sitecmd_cli audit . --fail-on high` on every pull request and every push to
`main`, writes the summary to the job summary, and uploads the full review as an
artifact. Its job, `Audit repository code`, is one of the five required checks
on `main`, so a high finding from SiteCMD's own engine blocks a SiteCMD merge.

## What this process does not claim

- No fraction of this codebase is claimed to be agent-written or
  human-written. The proportion is not tracked and would not be checkable.
- Automated review is not human review. Deterministic checks catch the
  regressions someone thought to encode; they do not judge whether a design is
  right. The decisions that need a person are listed in
  [security-review-cadence.md](security-review-cadence.md) precisely because no
  check can make them.
- A guardrail is evidence, not proof. [guardrails.md](guardrails.md) is explicit
  that wanting a wide exemption usually means the rule is miscalibrated, and the
  ratchet exists because the tempting fix for a failing budget is to raise it.

# Context-efficiency benchmark

Quantifies the core SiteCMD claim: handing an AI agent a **scanner brief** (exact
issue, file:line, evidence, suggested fix) makes fixing cheaper and more reliable
than asking it to fix issues with weaker context.

It is a real closed loop, not an estimate. For each target repo it runs three arms
on **fresh copies of the same repo**, then **re-scans** to verify what each arm
actually fixed. The only variable across arms is how much context the agent gets.

## The three arms

| Arm          | Context handed to the agent                                                                    |
| ------------ | ---------------------------------------------------------------------------------------------- |
| `blind`      | Nothing. "Audit and fix the problems in this repo." Must discover everything.                  |
| `categories` | Issue **count + per-category breakdown**. Knows _what_ and _how many_, not _where_.            |
| `brief`      | The full SiteCMD code-scan review: every issue with file, line, evidence, and a suggested fix. |

The task framing and the closing constraints are **identical** across arms (see
`lib/arms.mjs`). Context richness is the independent variable.

## How it measures

- **Ground truth, not self-report.** Resolution is verified by re-running
  `sitecmd audit` and diffing `checkId` sets. `checkId` is `code_scan.<rule>:<path>`,
  so it survives the line shifts a fix introduces.
- **Economics from the agent itself.** `claude -p --output-format json` reports
  `usage`, `num_turns`, `total_cost_usd`, and `duration_ms` per run.
- **Headline metric: compute per issue actually fixed.** Arms resolve different
  amounts, so raw per-run cost is misleading. The report normalizes to
  tokens/cost **per resolved issue**.

## Fairness

- Same task, same model, same turn cap, same repo starting state, same verifier.
- The brief is produced by a deterministic scanner (no LLM tokens), so the brief
  arm is not charged model tokens for its context. Scanner wall-time is recorded
  separately in `raw.json` for transparency.
- `regressions` (new findings the fix introduced) are tracked per arm. A cheap arm
  that breaks things is not actually cheaper.

## Running

```bash
# Validate the whole pipeline without spending anything (no claude calls):
node tools/benchmark/run-context-benchmark.mjs --dry-run

# Full run with the configured targets and arms:
node tools/benchmark/run-context-benchmark.mjs

# One repo, brief vs blind only, 3 repeats for variance:
node tools/benchmark/run-context-benchmark.mjs --target node-express-boilerplate --arms blind,brief --repeats 3
```

Output lands in `tools/benchmark/results/<timestamp>/` as `report.md` (the table)
and `raw.json` (every per-run record). Both `results/` and the cloned/working repos
in `.work/` are gitignored.

## Requirements

- `cargo` (builds the shipped headless `sitecmd_cli` workspace package on first run)
- Claude Code 2.1.219 or newer on PATH and signed in
- `git`

Non-dry runs require Claude Code's OS sandbox on macOS, Linux, or WSL2. Linux
and WSL2 need the sandbox dependencies documented by Claude Code. Native
Windows is not supported by the benchmark.

Code Scan is local and free. No SiteCMD license or connected-service credential is
required. If the baseline scan returns 0 issues, the harness warns and skips the
target.

## Configuration

`context-efficiency.config.json`: model, `maxTurns`, `repeats`, `arms`, and the list
of targets. Every target must use a public GitHub HTTPS URL, a lowercase
kebab-case name, and a full lowercase commit SHA. The harness fetches and resets
the cache to that exact commit before every run.

## Safety

Non-dry runs use Claude Code's [OS-level sandbox](https://code.claude.com/docs/en/sandboxing)
in fail-closed mode. The harness exposes only the sandboxed Bash tool, blocks
unsandboxed retries, hides host environment variables from commands, denies reads
from the user's home directory except for the run copy, disables repository and
user customizations, and limits command network access to the npm registry. If
the sandbox is unavailable, the run fails before the agent starts.

The disposable copy protects benchmark state. The sandbox constrains filesystem
writes, home-directory reads, credentials, and network egress, but it is not a
complete host-isolation boundary. Do not weaken these controls to accommodate a
target. Use a disposable VM when evaluating code outside the maintained target
list.

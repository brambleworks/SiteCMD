# Agent workflow benchmarks

Use the [evaluation protocol](../../docs/qa/agent-workflow-benchmark.md) to measure
repair quality, compute efficiency, and developer effort. The paired workflow
tooling freezes assignments, runs the subscription calibration in an isolated
desktop, imports evidence, records blinded reviews, and reports uncertainty.
This guide is for the operator preparing and executing that calibration. The
runner supports the five included cases, not arbitrary repositories or Web Scan.

The [benchmark VM](vm/README.md) supplies a separate Linux environment for building
the desktop and running trials. Host projects and accounts are not mounted.
Only a committed source archive is exported for the product build; subscription
credentials must be created through login inside the guest.

## Configure the subscription pilot

```bash
pnpm benchmark pilot
```

`pilot` prints the approved models and limits from `pilot-policy.json`. It is a
policy, not an execution command. Once the VM is prepared and running,
`pnpm benchmark:vm doctor` reads the guest's installed
Codex/Claude versions and saved authentication status without issuing prompts.
It prints no account identifiers or credential values. Exit code 2 means a client
or subscription login is missing. The separate `pnpm benchmark doctor` probes the
host, not the guest. Neither doctor approves execution.
Successful authentication does not verify model access, quota, global configuration
isolation, or disabled paid overage.

The included corpus contains four seeded repairs (CORS, redirect, SQL injection,
and path traversal) and a parameterized-query negative control. Ordinary tests
are visible to agents; separate behavioral graders are not. These small, owned
examples are calibration, not representative customer projects or marketing evidence.

On a new VM, prepare the environment and build the committed product:

```bash
pnpm benchmark:vm setup
pnpm benchmark:vm:tools
pnpm benchmark:vm:build
pnpm benchmark:vm:smoke
pnpm benchmark:vm:selftest
pnpm benchmark:cases:validate
pnpm benchmark:cases:scan
```

These commands make no model calls. Building excludes uncommitted changes.
`benchmark:vm:build --install-existing` installs or checks an already completed
build without compiling again. The smoke test uses the shipped desktop/MCP flow
and an owned reference repair, not an AI agent. Validation runs baseline and
reference checks three times each. Scanning records actual full reports, including
missed defects; a clean scan is not independent proof that a case is safe.
The executor self-test checks file submissions through the Codex sandbox and a
pinned Anthropic sandbox runtime, including denied credential access, response
tampering, and Unix sockets. Synthetic Node clients and quota fixtures then test
submission grading, evidence validation and timeout handling. These tests make
no model calls and do not establish end-to-end inference success.

Use the exact evidence paths printed by validation and scanning as the first two
arguments, and choose a new run directory:

```bash
pnpm benchmark:prepare GRADES_JSON SCANS_JSON tools/benchmark/.work/calibration-run
```

Replace `GRADES_JSON` and `SCANS_JSON` with those paths. Preparation freezes the
assignments, product, sources, graders, reports, protocol and runner. Each exact
model gets all five cases in all three workflows; models sharing a client remain
separate configurations in the plan and report. Client
versions are Codex `0.153.0-alpha.5` and Claude Code `2.1.260`, both at explicit
high reasoning. Changed runner or grader bytes require a new registration, not
editing an existing plan. No agent has run merely because a plan exists.

If Code Scan does not produce the repair handoff, the MCP assignment records a
pre-agent product error with zero calls. Keep it in the assigned population;
do not substitute an easier case. Inspect the scan evidence before interpreting
workflow differences.

### Subscription logins

Open the guest shell and sign into the subscriptions there:

```bash
pnpm benchmark:vm shell
codex login --device-auth
claude auth login --claudeai
```

Follow each login's instructions, opening its URL in the host browser if needed.
Do not select Console/API billing or copy host credential files. Exit the guest
shell, then run `pnpm benchmark:vm doctor`. Login success does not prove all
requested models are available. Their first real assignments must establish that;
an unavailable or different model is a failure, never permission to fall back.

### Account quota evidence

Preparation creates blank quota files in the ignored run directory. Fill them
from actual provider readings. Use UTC timestamps for `capturedAt` and each `resetsAt`, stable
non-identifying account labels, `authMode: "subscription"`, and
`extraUsageEnabled: false` only after verifying those facts. Record an evidence
reference in `source`; preserve the complete provider reading privately. The
template deliberately fails validation until completed.

Record **used** percentages, not remaining percentages. Include every applicable
weekly, model-specific, and short-session limit. Add model-specific windows as
needed; remove a template window only if the provider confirms it does not exist,
not because its usage is unknown. The checker cannot discover omitted provider
windows or authenticate a manual reading. Do not store API keys, OAuth tokens, or
email addresses in these snapshots.

Freeze `quota-baseline.json` before the first real trial. Save a new current snapshot
before and after every trial and at each submission; do not overwrite the baseline.
Keep snapshots with the trial evidence, including readings that paused the batch.

```bash
pnpm benchmark quota --baseline tools/benchmark/.work/calibration-run/quota-baseline.json --current tools/benchmark/.work/calibration-run/quota-current.json
```

Exit 0 means only that the supplied quota readings passed. Exit 2 means pause;
invalid input exits 1. Readings older than five minutes, changed accounts or weekly
resets, unknown extra usage, or either account reaching a stop threshold block the
batch. A short-session reset does not replenish the weekly allocation. These are
checks made by the quota command, not a live quota collector or process supervisor.
The VM executor enforces the time/submission limits, stops on reported rate limits,
and never requests a fallback model. Provider percentages are not precise
in-flight cost caps.

### Run one assignment

With both logins verified, extra paid usage disabled, and actual baseline/current
quota files in the run directory:

```bash
pnpm benchmark:run tools/benchmark/.work/calibration-run
```

Each invocation runs only the next unrecorded assignment. It serializes guest
execution, creates a fresh source tree and desktop database, and imports the
result before returning. Keep `quota-current.json` refreshed from actual readings
while it runs, before its five-minute freshness window expires. The host forwards
changes to the guest; the supervisor checks them every five seconds and at each
submission. It does not collect provider quota automatically. Stale readings stop
the current trial, which remains a recorded failure rather than being retried.

When the agent stops, the command asks for new readings from both providers and
waits up to five minutes before exporting. A still-fresh pretrial reading is not
post-trial evidence. Missing closing readings record an infrastructure failure.
Quota bookkeeping happens after the timed repair; no model keeps running while
the command waits. The quota baseline is immutable once execution begins; never
reset it to replenish the pilot allowance. Nonzero
exit status means inspect the failure and preserved evidence before continuing.
Do not rerun an assignment after model usage. If transport or import fails, retain
the guest trial directory and recover/import that evidence instead of deleting it
to start over. Ctrl-C stops the active agent; keep the VM running for evidence export.

Normal/report workflows submit through the provided local submission command.
MCP repairs submit through the real `request_verification` tool. Every candidate
is frozen before forwarding verification, and hidden grader feedback is withheld
in every workflow. Editing after the final submission prevents final acceptance.
The product's verification result is retained separately from independent grading.

## Check the pipeline locally

Requires the repository's Node and pnpm versions, but no account, model, desktop,
API calls, or Docker daemon. Run from the repository root, choosing a new output
directory whose parent exists:

```bash
mkdir -p tools/benchmark/.work
pnpm benchmark:test
pnpm benchmark fixture --out tools/benchmark/.work/workflow-fixture
pnpm benchmark report --run tools/benchmark/.work/workflow-fixture
pnpm benchmark report --run tools/benchmark/.work/workflow-fixture --json
```

Choose another output name for another run; existing runs are never overwritten.
The fixture demonstrates nine assignments: two synthetic repair cases and a
negative control, each in three workflows. It checks a known-broken source and
reference implementation with independent owned tests. Usage, time, reviews, and
MCP traces are explicitly synthetic. This is not a real agent comparison, a
calibration corpus, or measured evidence about SiteCMD.

The generated `plan.json`, `inputs/*/trial.json`, and neighboring receipts are
complete examples of the schemas. Copy their shape, not their synthetic values,
when implementing a real runner. Keep generated runs in `.work/` or `results/`,
both ignored by Git. Evidence directories use owner-only permissions.

## Freeze a real study

Prepare and independently validate the case corpus before creating a plan. Use a
calibration phase first. The study JSON requires:

| Field                                 | Contract                                                                                                                                                                                                                      |
| ------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `schemaVersion`, `id`, `phase`        | `1`, lowercase kebab-case identifier, and `calibration` or `confirmatory`.                                                                                                                                                    |
| `protocol`, `protocolSha256`          | Protocol version and digest of the exact registered protocol bytes.                                                                                                                                                           |
| `seed`, `repeats`, `arms`             | Unsigned 32-bit seed, positive repeat count, and exactly `normal`, `report`, `mcp` in that order.                                                                                                                             |
| `limits`                              | Positive `trialSeconds` and `submissions`. API studies also require positive `trialTokens`, `trialCostUsd`, and `studyCostUsd`. Subscription studies require zero dollar caps and may use `trialTokens: null`.                |
| `billing`                             | For subscriptions, copy the pilot billing object: subscription mode, disabled paid fallback/resets, weekly allocation, remaining-allowance floor, and quota freshness limit. Omit for legacy API/fixture accounting.          |
| `sitecmd`                             | Version, full commit SHA, dirty status, and SHA-256 of the actual bundled MCP entry point. Archive its dependencies/environment too.                                                                                          |
| `configurations`                      | Unique IDs with exact agent, agent version, model, reasoning, and environment.                                                                                                                                                |
| `tasks`                               | Unique IDs, repository cluster, repair/negative-control kind, code/web surface, category, prompt, requirements, provenance, held-out status, source/reference/grader/report hashes, baseline/reference checks, and validator. |
| `registration`, `sampleSizeRationale` | Required for confirmation, along with a clean SiteCMD build and held-out tasks.                                                                                                                                               |

For source, reference, and grader digests, hash immutable archives or a documented
canonical file manifest. Record that encoding in the protocol and preserve the
bytes. The planner validates the supplied hashes' shape; the case validator must
actually compare them against the corpus and run baseline/reference tests.

```bash
pnpm benchmark plan --study tools/benchmark/.work/calibration.json --out tools/benchmark/.work/calibration-run
```

The planner stores a canonical study digest and randomized paired assignments.
Do not edit the plan after creation. It reports maximum per-trial-cap exposure
and the study cap; neither authorizes a purchase or enforces an external runner's
spend. Get explicit operator budget approval before starting paid trials.

## Execute and import assignments

Follow the frozen assignment order with fresh isolated workspaces. Use the actual
SiteCMD desktop/MCP flow for `mcp`, not the older brief-only harness. Preserve the
first submitted candidate before returning external feedback, and freeze the
final candidate before teardown. The grader must inspect those exact snapshots.
Record unsuccessful and interrupted trials too.

A trial JSON includes its assignment ID and study digest, model selection/agent
version, fixture flag, status, elapsed time, measured human-active time or `null`,
warm/cold setup, submissions, reviews, transcript path, and usage. Non-completed
statuses require a failure explanation. MCP trials also need a server digest and
complete trace artifact. Submission times must be ordered and within trial time.
The VM runner records its explicit CLI model request separately from identities
actually emitted by the provider. Missing observed identity stays `null`; it is
not copied from configuration. Missing or mismatched identity blocks claim review.
A setup failure before client launch has `agentInvoked: false`, no model, no
submissions, and zero calls. Interrupted or truncated usage remains unknown.

All artifact paths are relative to the trial JSON's directory. Only referenced
files are imported. Paths cannot be absolute, contain traversal, or use symlinks.
Each artifact is limited to 64 MiB and a trial to 256 MiB. For larger transcripts,
retain the private original and reference a documented, bounded lossless archive
or split raw usage logs; do not silently truncate the evidence used for review.

Each submission identifies `patch`, `patchSha256`, `elapsedMs`, `graderSha256`,
`acceptancePass`, `regressionsPass`, `integrityPass`, and a grading `receipt`.
The patch includes untracked additions and binary changes. An intentional no-op
uses an empty patch with its actual SHA-256, plus a triage explanation.

The grader receipt contains the trial/study/source/patch/grader identities, executor,
environment, nonempty `acceptance` and `regressions` check lists, and an `integrity`
object with `passed` and `reason`. Each check records its command, exit code
(`null` if interrupted), and log artifact. Only zero exit codes pass. The
independent grader, not the agent, produces these receipts.

Usage fields are disjoint `inputTokens`, `outputTokens`, `cacheReadTokens`,
`cacheWriteTokens`, `includesAllAgents`, `costUsd`, `costBasis`, and `receipt`.
Use `null` for unknown counts or cost and `unknown` for an unknown cost basis;
otherwise identify cost as `estimated` or `billed`. Reasoning tokens already
included in provider output must not be counted twice.

For subscription studies, use `costBasis: "subscription"`, `costUsd: null`, and
separate `incrementalCostUsd` and `apiEquivalentCostUsd`, each a nonnegative amount
or `null`. The first is verified additional spending; the second is a hypothetical
API estimate. Neither supplies a dollar-efficiency comparison. Unknown additional
spending blocks claim readiness; measured overages are retained and flagged.

The usage receipt contains the same usage object without `receipt`, an
`accountant`, a `method`, and a nonempty `raw` list of provider evidence paths.
When provider usage is unavailable, preserve the failure log and explain the
missing amount. Receipt equality proves consistency, not the truth of an
operator's accounting. Audit the provider records before publishing.

The `claudeUsage` normalizer accepts one final Claude result and prefers
whole-tree `modelUsage` over root-only `usage`. It counts cache reads/writes and
labels the reported dollar amount as an estimate. Root-only usage remains
incomplete unless the runner proves subagents were disabled. Do not sum cumulative
result messages. `codexUsage` accepts one `turn.completed` event, subtracts cached
input from total input, and does not double-count reasoning output. Codex cache
writes have no separate reported category and remain within uncached input. Codex
accounting is incomplete unless subagents were disabled or the runner separately
accounts for the whole tree and all turns. Deduplicate event records, not repeated
numeric counts, when assembling a trial receipt.

Both normalizers accept `billingMode: "subscription"`. They leave extra charges
unknown unless the caller supplies a verified `incrementalCostUsd`; they never
infer zero charges from a subscription login. Evidence must cover all calls,
including interrupted turns, retries, and delegated work. There is no generic
token guesser or automatic price lookup.

```bash
pnpm benchmark record --run tools/benchmark/.work/calibration-run --input tools/benchmark/.work/trial-evidence/trial.json
```

Import checks evidence consistency, copies referenced artifacts, and records their
digests. A second import for the same assignment fails. Loading results rechecks
digests and receipt contents. A partial import remains an explicit error, not an
omitted losing trial. Preserve damaged evidence for investigation rather than
overwriting it. Hashes detect changes, not a dishonest operator who rewrites both
data and hashes.

## Review and report

Import `reviews: []` when independent review is pending. Each review identifies a
submitted patch hash, reviewer, `blinded: true`, `decision: "accept"` or `"reject"`,
and a concrete reason. An inline review also references a JSON receipt with those
same fields. For a later review, provide just those fields without `receipt`:

```bash
pnpm benchmark review --run tools/benchmark/.work/calibration-run --trial TRIAL_ID --input tools/benchmark/.work/review.json
pnpm benchmark report --run tools/benchmark/.work/calibration-run
```

Reviews are appended without rewriting the original trial. Duplicate reviewers
for the same patch are rejected, and any rejection prevents acceptance. Do not
show reviewers the assignment arm or agent transcript before their decision.

Reports keep failures in the denominator, withhold complete rates when records or
reviews are missing, and separate configurations, surfaces, and negative controls.
Efficiency includes failed-trial spending; zero accepted repairs yield `n/a`.
Relative change and percentage-point change are separate. Confidence intervals
resample repositories and tasks with paired arms and repeats intact. An unavailable
interval or measurement is never filled in with zero.

The CLI writes reports to stdout and does not overwrite saved reports. Use
`--json` for structured analysis. No report automatically permits a marketing
claim; see the protocol's publication checklist. Configuration limits, MCP trace
presence, reviewer identities, and blinded flags still need operational auditing.

## Older scanner-context experiment

`run-context-benchmark.mjs` compares `blind`, `categories`, and `brief` prompts on
pinned public repositories using Claude Code. It does not exercise the MCP
workflow. Its checkId diff measures scanner clearance, not independent repair
correctness. The legacy aggregation excludes metric-less failures and has no
paired confidence intervals. Do not use its output for numerical product claims.

```bash
node tools/benchmark/run-context-benchmark.mjs --dry-run
node tools/benchmark/render-report.mjs tools/benchmark/results/RUN/raw.json
```

A legacy dry run still builds the Rust CLI, fetches repositories, and scans them;
it only skips paid model calls. It is not the no-network fixture above. Its config
pins target commits. Non-dry runs require Claude Code 2.1.219 or newer, a configured
account, and the fail-closed OS sandbox on macOS, Linux, or WSL2. Native Windows
is not supported. Read its sandbox controls before running untrusted target code.
The scanner does not spend model tokens to produce a brief, but the agent does
consume input tokens when reading it.

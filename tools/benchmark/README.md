# Agent workflow benchmarks

Use the [evaluation protocol](../../docs/qa/agent-workflow-benchmark.md) to measure
repair quality, compute efficiency, and developer effort. The paired workflow
tooling freezes assignments, imports evidence, records blinded reviews, and
reports uncertainty. It does not yet launch agents, operate an isolated desktop,
apply patches, or run arbitrary repository graders. Those are operator/adapter
responsibilities; the evidence checker does not replace them.

The [benchmark VM](vm/README.md) supplies a separate Linux environment for building
the desktop and running future trials. It does not copy host projects or accounts,
install agent clients, or implement the trial executor.

## Configure the subscription pilot

```bash
pnpm benchmark pilot
pnpm benchmark doctor
```

`pilot` prints the approved 30-trial settings from `pilot-policy.json`. It is a
policy, not a runnable study or a generated case corpus. `doctor` reads the installed
Codex/Claude CLI versions and saved authentication status without issuing prompts.
It prints no account identifiers or credential values. It rejects billing/auth
environment overrides and reports missing execution prerequisites. Exit code 2
means execution is blocked; it is expected until the isolated runner is implemented.
Successful authentication does not verify model access, quota, global configuration
isolation, or disabled paid overage.

For the pilot, prepare four independently validated Code Scan repairs and one
negative control. Freeze exact CLI versions, explicit reasoning settings, and the
execution environment in the manifest. Copy the policy's `limits` and `billing`
objects unchanged. Then require the policy when planning:

```bash
pnpm benchmark plan --pilot --study tools/benchmark/.work/pilot.json --out tools/benchmark/.work/pilot-run
```

No real case corpus or execution adapter is included yet. A desktop instance in a
disposable environment must own the test database and process real verification.
`SITECMD_DB_PATH` does not isolate the desktop itself. Do not use the personal
desktop database or execute generated patches on the maintainer's host.

### Account quota evidence

Copy `quota-template.json` into the ignored run directory and fill it from actual
provider readings. Use UTC timestamps for `capturedAt` and each `resetsAt`, stable
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
pnpm benchmark quota --baseline tools/benchmark/.work/pilot-run/quota-baseline.json --current tools/benchmark/.work/pilot-run/quota-current.json
```

Exit 0 means only that the supplied quota readings passed. Exit 2 means pause;
invalid input exits 1. Readings older than five minutes, changed accounts or weekly
resets, unknown extra usage, or either account reaching a stop threshold block the
batch. A short-session reset does not replenish the weekly allocation. These are
checks on supplied evidence, not a live quota collector or process supervisor.
The execution adapter must enforce deadlines/submission limits, disable fallback,
and stop on rate limits. Provider percentages are not precise in-flight cost caps.

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

A trial JSON includes its assignment ID and study digest, observed model/agent
version, fixture flag, status, elapsed time, measured human-active time or `null`,
warm/cold setup, submissions, reviews, transcript path, and usage. Non-completed
statuses require a failure explanation. MCP trials also need a server digest and
complete trace artifact. Submission times must be ordered and within trial time.

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

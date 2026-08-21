import type { CodeFixGuideEntry } from "./types";

export const QUALITY_FIX_GUIDES: Record<string, CodeFixGuideEntry> = {
  "client-ai-sdk": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Move any long-lived provider API key behind a server route or action; never ship it in the bundle, a public environment variable, or localStorage. If the provider documents a browser flow, mint a short-lived, least-privilege ephemeral token on the server and keep authorization, rate limits, and spend controls at the server boundary. Inspect the production bundle and Network tab to confirm no long-lived secret remains.",
    ],
  },
  "client-db-access": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "Move server-only database drivers and ORMs (Prisma, Knex, `pg`, MySQL clients, raw SQL) into a route, server action, or backend service, and confirm authorization is enforced before the query. Browser-designed clients such as Supabase or Firebase can be intentional when they use publishable credentials and enforce RLS or security rules, so review those policies instead of relocating them. Check production client chunks for drivers or credentials afterward.",
    ],
  },
  "config-secret": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Classify the matched value without copying it into logs or chat: genuine credential, documented public identifier, unmistakably fake fixture, or revoked value. If a real credential may have left its intended machine, revoke or rotate it before cleanup, then replace the literal with the tool's supported environment substitution, OS keychain, or secret-manager flow and confirm the old value no longer works.",
    ],
  },
  "critical-path-no-test": {
    effort: "involved",
    effortMinutes: 45,
    default: [
      "Write at least one happy-path and one main-failure-case test for each critical path: authentication, payment processing, data mutations, and authorization. Start with integration tests over the full request-response cycle and add them to the required CI gate; tests do not prove a path is defect-free, but they make its intended behavior executable and catch repeat regressions.",
    ],
  },
  "god-module": {
    effort: "involved",
    effortMinutes: 45,
    default: [
      "Map the module's public API, invariants, and failure boundaries first; size and marker counts are review signals, not proof every responsibility needs its own module. Where unrelated policy is coupled, extract one cohesive concern at a time behind a narrow typed interface, use characterization tests before and after each extraction, and judge success by clearer ownership rather than an arbitrary line target.",
    ],
  },
  "god-route": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Map the route's authentication, authorization, validation, transaction, and failure handling before moving code; a cohesive orchestration handler can be valid. Where policy is genuinely entangled, extract the smallest cohesive operation behind a typed boundary, keep security checks visible, and rerun route-level tests for access, invalid input, success, and failure paths to confirm equivalent behavior.",
    ],
  },
  "oversized-module": {
    effort: "moderate",
    effortMinutes: 25,
    default: [
      "Confirm the file is authored application code rather than generated or vendor output; size alone does not require a split. Where unrelated concerns are coupled, extract cohesive modules behind narrow typed interfaces, keep code that shares one transaction or failure boundary together, and use characterization tests to verify behavior stays equivalent.",
    ],
  },
  "error-boundary-missing": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Inventory the framework's existing route and root error surfaces first, then add boundaries only around useful recovery zones such as the application shell or an independently recoverable route. React boundaries do not catch event-handler errors, most asynchronous callbacks, or server failures, so keep explicit handling there, show a safe fallback with a retry path, and report scrubbed exceptions without tokens or personal data.",
    ],
  },
  "error-reporting-missing": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Map which browser, server, worker, and background-job failures are already captured by platform logs and existing wrappers before adding a vendor SDK. For uncovered boundaries, choose a reporting path that meets privacy, retention, and cost requirements, scrub credentials and personal data before export, then trigger controlled failures in staging to confirm events arrive once with usable context.",
    ],
  },
  "external-call-retry": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Inventory retries already performed by the SDK, gateway, messaging layer, and caller first; a documented fail-fast policy is valid. Where retries are justified, retry only transient failures (eligible network errors, 408, 429, provider-documented 5xx) with a small attempt cap, exponential backoff with jitter, and an overall deadline; note `fetch()` resolves on HTTP errors, so a bare retry wrapper will not retry a 503. Retry non-idempotent operations only with an idempotency key.",
    ],
  },
  "external-call-timeout": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Inventory the effective SDK, client, proxy, and platform deadlines, then set an explicit per-attempt deadline where the budget is absent or too broad (an AbortSignal for fetch, timeout options for axios). Derive the value from the caller's latency budget rather than one universal number, propagate cancellation, and return a controlled result instead of letting the request occupy resources indefinitely.",
    ],
  },
  "healthcheck-missing": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Add a cheap liveness endpoint such as `GET /live` that answers only whether the process can serve requests, with no database or dependency queries, plus a separate readiness endpoint such as `GET /ready` that runs bounded dependency checks and returns 503 while unavailable. Point the platform's probes at the matching endpoints and keep responses free of versions, credentials, and stack traces.",
    ],
  },
  "recovery-runbook-missing": {
    effort: "moderate",
    effortMinutes: 25,
    default: [
      "Link approved private runbooks and provider procedures from an accessible operational index rather than duplicating instructions that will drift. For credible failure modes, document detection signals, ownership, escalation, provider-supported recovery steps, and verification, referencing access roles and secret-manager locations without embedding credential values, then validate with a tabletop exercise run by someone other than the author.",
    ],
  },
  "structured-logging-missing": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Check whether the runtime, framework, or hosting platform already structures standard output before choosing a library. Then add one server logging boundary with stable event names, severity, timestamps, and request or trace correlation, redacting credentials, cookies, and personal data by default, and migrate security- and failure-relevant logs first rather than mechanically replacing every console call.",
    ],
  },
  "job-visibility-missing": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Review the visibility your background-job platform or provider already supplies, then classify jobs by user impact and whether durable per-job state is actually required. For important durable work, expose a correlation-safe job identifier with meaningful states, attempts, timestamps, and terminal failure; lightweight best-effort work may need only completion and failure metrics. Confirm stuck work becomes visible within the promised response window.",
    ],
  },
  "ci-workflow-missing": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Add a checked-in CI workflow for the repo's host (`.github/workflows/quality.yml` for GitHub Actions, `.gitlab-ci.yml` for GitLab). Keep it small and real: checkout, install dependencies from the lockfile, run the production build plus at least one existing lint, typecheck, or test command, then push a branch to confirm it runs from a clean checkout.",
    ],
  },
  "ci-quality-gate-missing": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Add the repo's real quality commands (build, lint, typecheck, tests, or an existing quality script) to the CI job that already runs on pull requests or pushes. Verify by introducing a harmless failing check on a branch, confirming CI blocks it, then reverting.",
    ],
  },
  "build-script-missing": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Add a production build script to the primary package manifest using the framework-native command such as `next build`, `vite build`, or `astro build`. Run it from a clean install to expose missing environment or compile-time assumptions, then wire the same script into CI and deploy settings.",
    ],
  },
  "ci-only-builds": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Add at least one lightweight quality command beyond the production build, such as lint, typecheck, or a focused test for the riskiest route or flow. Expose it as a package script, run it in CI before or alongside the build, and confirm CI fails when the command finds a real error.",
    ],
  },
  "pre-commit-hooks-missing": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Choose a repo-managed hook tool such as Lefthook, Husky with lint-staged, pre-commit, or simple-git-hooks, and commit its config so teammates share the guardrail. Run fast checks (formatter or lint-staged on touched files) before commit, keep slow builds and broad test suites in pre-push or CI, and confirm the hook runs by making a harmless staged change.",
    ],
  },
  "pre-commit-hooks-weak": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Open the checked-in hook config; if it only prints text, installs tooling, or does bookkeeping, it is not protecting the repo. Wire it to the fastest useful quality command, such as lint-staged for touched files or a formatter check, and confirm a deliberately failing staged change blocks the commit or push.",
    ],
  },
  "runtime-version-eol": {
    effort: "moderate",
    effortMinutes: 25,
    default: [
      "Check the runtime's official support schedule and determine what the flagged declaration controls: a version-manager file selects a tool version, while `engines.node`, `requires-python`, or Composer `require.php` usually declare compatibility and do not prove the deployed runtime. Update every environment intended to match, run the build and tests on the newly supported line, and treat a raised library compatibility floor as a release-impacting change.",
    ],
  },
  "deploy-rollback-plan-missing": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Add a short rollback note near your deploy docs naming the provider and the exact screen or command used to find the last known-good release. Document when rollback is unsafe, especially after database migrations or one-way side effects, and have someone follow the note against staging to confirm they can restore a known-good release without extra context.",
    ],
  },
  "backup-restore-plan-missing": {
    effort: "moderate",
    effortMinutes: 25,
    default: [
      "Document or link the approved backup provider, data scope, schedule, retention, encryption and access model, recovery objectives, and provider-supported restore path, referencing access roles rather than credential values. Then have an authorized teammate restore a recent backup to an isolated non-production target, measure data age and recovery time, and record the result.",
    ],
  },
  "jsx-inline-style-density": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Keep genuinely dynamic values inline, but move repeated static visual styles out of `style={{ ... }}` into classes, CSS modules, component variants, or a shared wrapper. Start with the dominant repeated pattern in the flagged component rather than rewriting the whole file, then re-render and compare the main states to confirm layout and spacing are unchanged.",
    ],
  },
  "direct-url-dependency": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Prefer an authenticated registry release when the publisher provides one, verifying name, owner, repository, and integrity before switching sources. If a Git dependency is necessary, pin the manifest and lockfile to an immutable full commit SHA rather than a branch or movable tag, review that commit and its install/build scripts, and let update automation propose reviewed SHA changes.",
    ],
  },
  "lockfile-missing": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Generate a lockfile with your package manager's install command (`npm install`, `yarn install`, or `pnpm install`), commit it, and keep it out of `.gitignore`. Make CI use the matching frozen command (`npm ci`, `pnpm install --frozen-lockfile`, or equivalent) so manifest drift fails visibly.",
    ],
  },
  "unpinned-github-action": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Replace each third-party action's tag or branch reference with the full 40-character commit SHA of the release you trust (`uses: owner/action@<commit-sha> # v4`), keeping the human-readable version in a comment. Enable Dependabot or Renovate for the `github-actions` ecosystem so pins get reviewed updates; prioritize third-party actions since they run with your workflow's permissions and secrets.",
    ],
  },
  "workflow-script-injection": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      'Move each attacker-controllable expression such as `${{ github.event.* }}` or `${{ github.head_ref }}` out of the `run:` script and into the step `env:` block, then reference it as quoted shell data (`echo "$TITLE"`) so the shell treats the value as text, never as commands. PR titles and bodies, issue and comment bodies, branch names, and commit messages are all attacker-controlled.',
    ],
  },
  "npmrc-committed-token": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Determine whether the `.npmrc` is tracked or shared and whether the literal is a genuine credential, without printing it. If it was committed, shared, or copied into CI, revoke or rotate it first, then replace the literal with the registry's supported environment substitution such as `//registry.npmjs.org/:_authToken=${NPM_TOKEN}` and keep the variable in the local or CI secret store. Confirm the exposed old value no longer authenticates.",
    ],
  },
  "dockerfile-unpinned-base": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Replace tagless and `:latest` FROM references with a specific version tag such as `FROM node:22-bookworm-slim`, or pin to a digest (`@sha256:...`) for full immutability. Let Dependabot or Renovate raise pull requests for the `docker` ecosystem so pinned bases get reviewed updates instead of silent ones.",
    ],
  },
  "remote-pipe-to-shell": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Split the pipe: download the script to a file first (`curl -fsSL <url> -o install.sh`), verify it against a checksum committed in your repository or the publisher's signature, then execute it. Prefer a package manager or digest-pinned container image when one exists; these improve integrity checking but do not make an untrusted publisher safe automatically.",
    ],
  },
  "workflow-pr-target-checkout": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Never combine a privileged trigger (`pull_request_target` or a privileged `workflow_run`) with a checkout of the PR head; executing that checkout can give attacker-controlled code the job's secrets and token permissions. If the job only builds, lints, or tests the contribution, use `pull_request` with minimal read-only `permissions` and no injected secrets; keep any privileged follow-up job separate and confirm privileged jobs check out only the base branch.",
    ],
  },
  "lockfile-integrity-weak": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Review the affected entries: a missing `integrity` value removes npm's recorded content check for that registry tarball, and SHA-1 has weaker collision resistance than SHA-512 and should be refreshed when the registry supplies stronger metadata. On a disposable branch, refresh only the affected resolution rather than deleting the whole lockfile, review the diff, and if weak entries keep returning, check whether a private registry or proxy is omitting integrity metadata.",
    ],
  },
  "nextconfig-errors-ignored": {
    effort: "moderate",
    effortMinutes: 30,
    default: [
      "Prefer removing `typescript.ignoreBuildErrors` from next.config and fixing real errors; if a required `tsc --noEmit` gate intentionally runs before deployment, document and test that ordering instead. On Next.js 16+ the `eslint` option and `next lint` were removed, so delete the obsolete block and run ESLint through the supported CLI; on older releases, remove `ignoreDuringBuilds` unless a separate required ESLint gate is proven to run.",
    ],
  },
  "release-age-policy-missing": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Pin a supporting package-manager version, then enable its native release-age gate: pnpm 10.16+ uses `minimumReleaseAge: 1440` (minutes) in `pnpm-workspace.yaml`, npm 11.10+ uses `min-release-age=1` (days) in `.npmrc`, Bun 1.3+ uses `minimumReleaseAge` (seconds) in `bunfig.toml`, and Yarn 4.12+ uses `npmMinimalAgeGate: 1d` in `.yarnrc.yml`. Choose a window that balances review time with urgent fixes; Renovate or Dependabot cooldowns defer only bot-generated updates, not manual or CI resolution.",
    ],
  },
  "unbounded-dependency-range": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Replace each `*`, `x`, or `latest` spec in package.json `dependencies` with a bounded range pinned to the version you actually use, such as `^1.4.0` or `~1.4.0`. Run the package manager's install so the lockfile records the resolution, commit both files, and review upgrades deliberately with `npm outdated` or your package manager's equivalent.",
    ],
  },
  "workflow-write-all-permissions": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Replace `permissions: write-all` with least privilege: set a restrictive default at the top of the file such as `permissions: { contents: read }`, then grant specific write scopes only on the jobs that need them (for example `contents: write` on a release job). Trigger the workflow once after tightening it; the Actions log names the exact permission a denied step needs.",
    ],
  },
  "tsconfig-strict-off": {
    effort: "moderate",
    effortMinutes: 30,
    default: [
      "Run `tsc --showConfig -p <flagged-config>` to resolve `extends` and confirm which build or package consumes the file; a tooling or migration config may intentionally differ from the production target. If strict mode should apply, enable `strict` (or remove the narrower `noImplicitAny: false` override), fix errors with accurate types and narrowing rather than broad `any` or `@ts-ignore`, and re-check the effective config for the application target.",
    ],
  },
  "lockfile-mismatch": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Run the package manager's frozen install first to reproduce the mismatch, then decide whether the manifest change or the lockfile is the intended source change; do not delete the lockfile and silently re-resolve the whole dependency graph. Reconcile with the repository's pinned package-manager version, review the diffs for unrelated changes, and commit the manifest and lockfile together with a matching frozen CI command.",
    ],
  },
  "registry-host-mismatch": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Build the approved registry map from `.npmrc`, workspace and CI configuration, and scoped packages; an intentional private registry, caching proxy, or mirror is valid, while an unexplained host change is the review signal. Treat lookalike or unapproved public hosts as a possible dependency-confusion or tampering event and investigate before reinstalling, then pin the intended registries in checked-in configuration and regenerate only the affected entries.",
    ],
  },
  "undeclared-package": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Resolve what the import is before installing anything: confirm it is not a typo, workspace package, path alias, framework-provided virtual module, runtime built-in, or generated file. If application code genuinely relies on a registry package, declare it in the appropriate manifest section instead of relying on hoisting or a transitive dependency, then verify with a clean frozen install plus the build, typecheck, and tests.",
    ],
  },
  "unused-dependency": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Confirm the package is genuinely unused before removing it: search source imports plus configuration, package scripts, framework plugins, type-only use, dynamic imports, and command-line use a static scan may miss. If unused, remove it with the project's package manager so the manifest and lockfile change together; if used, place it in the manifest section matching how consumers obtain it, then verify from a clean install, build, and tests.",
    ],
  },
  "empty-catch-blocks": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "For each empty `catch` block, decide the right action: log the error with context, report it to your error tracker, rethrow, or return a safe error response. Where the error truly does not matter, add a brief comment explaining why, so intentional suppression is distinguishable from forgotten handling, then trigger an error condition and confirm it now surfaces in logs or reporting.",
    ],
  },
  "console-log-error-handling": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Replace `console.log(e)` with the project's structured logger or error reporter and a stable error code, including only safe diagnostic context such as request or trace ID and route; never log credentials, raw tokens, or request bodies. Add a user-facing error response where the error affects the request, then trigger an error in development and confirm it reaches your reporting system with enough context to debug.",
    ],
  },
  "typescript-any-abuse": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "Replace the most-used `any` values with specific types first, tracing what each value actually is. Where the type is genuinely unknown at compile time, use `unknown` instead of `any` so runtime checks are forced before use, then run `npx tsc --noEmit --strict` and work through the remaining errors.",
    ],
  },
  "no-automated-tests": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "Install a test runner for your stack (`npm install -D vitest` for JS/TS, `pip install pytest` for Python, or `cargo test` for Rust), add a `test` script, and write one test for your most critical route or data flow, starting with the unhappy path such as a failing database or malformed input. Run it, confirm it passes, and add it to CI so it runs on every push.",
    ],
  },
  "linter-missing": {
    effort: "moderate",
    effortMinutes: 10,
    default: [
      "Install a linter for your stack (`npm init @eslint/config` or `npx @biomejs/biome init` for JS/TS, `ruff` for Python, `cargo clippy` for Rust), review the initial findings, fix genuine bugs, suppress false positives, and configure rules that do not fit the project. Add a `lint` script and run it in CI so lint errors block merges.",
    ],
  },
  "placeholder-density": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "Search the codebase for TODO, FIXME, HACK, CHANGEME, and PLACEHOLDER markers and categorize each as a critical gap, a nice-to-have, or stale. Implement critical gaps such as auth, error handling, and validation now, convert nice-to-haves into tracked issues, remove stale markers, and confirm none of the remainder sit in critical code paths.",
    ],
  },
  "no-pagination": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Trace the query through wrappers and framework defaults and confirm the collection can actually grow beyond a safe response; if it is inherently bounded, document that invariant instead of adding ceremonial pagination. Where a bound is needed, enforce a server-side maximum with validated input and deterministic ordering, choose cursor pagination for large or changing datasets, and test edge-case page values against a dataset larger than the maximum page.",
    ],
  },
  "duplicate-utility-deps": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Pick one library to keep based on bundle size, maintenance status, and existing usage (search imports from each duplicate), migrate all usage to it, then remove the others with `npm uninstall <package>` and run the test suite and build to confirm nothing broke.",
    ],
  },
};

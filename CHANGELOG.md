# Changelog

Notable user-facing changes to SiteCMD will be recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
releases use [Semantic Versioning](https://semver.org/).

This changelog begins with SiteCMD's first public release. Private development
tags and their generated artifacts are intentionally not carried into the
public repository history.

## [Unreleased]

### Changed

- The Issues list drops the web/code scan filter. One category filter now covers
  findings from both scanners, a search box narrows the list by title, and the
  list pages twenty issues at a time.
- The Issues list and Code Scan results share one pager with numbered page links.
  Previous and Next sit beside the numbers and are hidden at the ends rather than
  shown as dead controls.

### Fixed

- Adding a project folder that runs on DDEV, Lando, Docksal, or a `.test`
  hostname now labels the detected URL as Local instead of listing it as
  another production site.
- Code findings in the Issues list now name their category, such as Database or
  Architecture, instead of every row reading "Code".
- Issue history no longer claims an issue regressed after a deploy when the
  issue had never passed. A regression now requires an earlier verified pass.

## [1.1.1] - 2026-08-31

### Changed

- Fix guides and MCP scan comparisons now use plain words instead of
  emoji, so status and severity read the same everywhere they are quoted.
- The hosted service is now named SiteCMD Connect, replacing the retired
  "Founder Beta" branding in the app and documentation.
- New and resolved counts in scan summaries now come from exact issue
  lifecycle tracking across the whole run, instead of per-check estimates.

### Fixed

- A scan that cannot finish everything it was asked to run now reports as
  partially complete and names what fell short, instead of presenting a partial
  run as a clean one. New, resolved, and regression counts are withheld from a
  partial run rather than compared against full history.
- Scheduled scan notifications no longer report a regression when the two runs
  did not cover the same ground. A run whose in-app browser or accessibility
  layer did not execute is compared only against runs with the same coverage.
- The scan history limit set in Settings now applies to scans started from the
  app, which previously kept the built-in default instead.
- Automatic dependency refreshes no longer record an update as applied when
  verification did not observe it, and previously recorded inferred entries
  are removed.
- The MCP server now reports a clear health error when the desktop database
  schema is newer than it supports, instead of failing unpredictably.

## [1.1.0] - 2026-08-25

### Added

- The MCP server can now drive the whole fix loop: `start_fix` opens a fix
  attempt, `run_scan` triggers a scan, and a desktop heartbeat tells the agent
  whether the app is running. `request_scan` is now `how_to_rescan` (the old
  name still works).
- The CLI gained unified gate flags across `audit`, `scan`, `check`, and
  `gate`, SARIF output for code-scanning integrations, and audit baselines.
- A false-positive report form and accuracy labels, so a wrong finding can be
  reported from the issue itself.
- Every release now publishes signed checksums and build provenance: a
  minisign-signed `SHA256SUMS` beside the artifacts and on the GitHub
  Release, with verification steps in the README.
- The first run walks through connecting an AI editor, with manual MCP setup
  instructions for every supported editor.
- The Issues page explains how each issue affects the SiteCMD Score.

### Changed

- Backend errors shown in the app are now plain-language messages with a next
  step, instead of raw error text.
- Fix guides and first screens open with plain-English leads.
- Dialogs, toasts, and navigation follow accessibility expectations: correct
  focus handling, screen-reader announcements, visible focus rings, and
  reduced-motion support across the app.

### Fixed

- The installer script refuses truncated downloads and version downgrades,
  and compares release versions strictly.
- Scan traffic is harder to abuse: the analyzer blocks private-network
  subresources, API responses are size-bounded, git metadata is read with
  hostile repository configuration neutralized, and credentials that never
  migrated to the OS keychain are refused instead of silently used.

## [1.0.0] - 2026-08-21

### Added

- Initial public release of SiteCMD. The desktop app scans websites and linked
  codebases for security, performance, SEO, accessibility, compliance, and
  configuration issues, ranks them by real risk, and hands the fix to the
  editor or coding agent you already work in.
- The complete local workbench is free: scanning, scan history, issue
  correlation, reports, and fix guidance all run on this machine, and nothing
  leaves it unless a site is explicitly connected.
- The connected service enters founder beta: hosted scheduled scans, alert
  email and webhooks, and shareable reports for connected sites, comped for
  the beta cohort.
- An MCP server so AI editors can read scan results and fix briefs directly.
- A CLI (`sitecmd`) for scanning and CI quality gates.

# SiteCMD

Desktop website health scanner and command center. SiteCMD scans websites and
linked codebases for security, performance, SEO, accessibility, compliance, and
configuration issues, ranks them by real risk, and hands the fix to the editor
or coding agent you already work in.

![SiteCMD desktop dashboard showing issues, updates, score, alerts, traffic, search, and deploy signals](https://sitecmd.com/images/features-hero-poster.png)

## What ships

| Surface           | Purpose                                                                                                  | Account required    |
| ----------------- | -------------------------------------------------------------------------------------------------------- | ------------------- |
| Desktop app       | Local website and source scanning, project history, correlations, reports, and fix guidance              | No                  |
| `sitecmd` CLI     | Local Code Scan audits, machine-readable reports, and connected CI gates                                 | No for local audits |
| MCP server        | Gives supported AI coding tools local findings and fix briefs, then hands attempts back for verification | No                  |
| Connected service | Hosted scheduled scans, deploy verification, alerts, shared reports, and baseline-aware CI decisions     | Yes                 |

The desktop app, CLI, and MCP server in this repository are the complete local
product. The connected service is a separate hosted product; local scan detail
is not withheld to create the paid tier.

## Install and try it

Download the desktop app from [sitecmd.com/download](https://sitecmd.com/download).
Release builds support macOS 11 or later with Safari/WebKit 16.2 or later
(Apple silicon and Intel through one universal app), Windows 10 or later on
x86_64 with WebView2 111 or later, and Ubuntu 22.04 or a compatible x86_64
Linux distribution with WebKitGTK 2.40 or later as an AppImage.

The installed `sitecmd` CLI can run the source audit directly. The installer is
[maintained in this repository](install.sh), authenticates the archive with the
public updater key, and verifies its SHA-256 checksum and reported version.

```bash
curl -fsSL https://sitecmd.com/install.sh | sh
sitecmd audit .
sitecmd audit . --format github --fail-on high
```

To inspect the installer before running it:

```bash
curl -fsSLo sitecmd-install.sh https://sitecmd.com/install.sh
less sitecmd-install.sh
sh sitecmd-install.sh
```

The installer supports macOS and Linux, requires
[Minisign](https://jedisct1.github.io/minisign/), verifies the release checksum
and signature before installing, and accepts `SITECMD_VERSION` for a pinned
release. Windows users can use the signed zip linked from the
[CLI documentation](https://sitecmd.com/docs/cli).

See [Get value in five minutes](docs/product/get-value-in-5-minutes.md) for the
shortest desktop walkthrough, or [the MCP server README](apps/mcp-server/README.md)
to put the same findings inside an AI coding tool.

## Verify your download

Releases published by the release workflow carry a Sigstore build-provenance
attestation; verify one with
`gh attestation verify <file> --repo brambleworks/SiteCMD`. Releases
back-filled by hand before the workflow change carry none.

## Where your data lives

Everything in this repository runs on your machine. Source code and raw file
paths stay there. Integration credentials are stored locally and sent only to
the provider they authenticate, never to SiteCMD. Scan findings stay local
unless you explicitly connect a site to the connected service, which is off
until you set it up.

That claim is checkable rather than asserted, which is the reason this
repository is public:

- The filesystem and network boundaries are in this source. What the scanner
  requests, what the code audit reads, and where either can send anything are
  all here to be read, and [sitecmd.com/trust](https://sitecmd.com/trust)
  enumerates every outbound call by name.
- The sync payload builder is in this source too, and the app and CLI render
  its exact output before anything is sent. What you read in the payload
  inspector is the bytes, not a description of them.
- Site operators reading SiteCMD's identity out of an access log get the same
  treatment at [sitecmd.com/scanner](https://sitecmd.com/scanner): every kind
  of request, why some look hostile, and how to block them.

## What is sold, and what is not

The client in this repository is the complete local product. The paid product
is the connected service: hosted scans between deploys, deploy-anchored
verification, baseline-aware regression alerting, and the CI merge gate's
server side. The line is not how much of the local product you get; it is
whether SiteCMD's servers have to exist for the capability to exist at all.
Code that runs on your machine is here under Apache-2.0. Code that runs on
SiteCMD infrastructure is not part of this repository.

## Repo map

This repository is a pnpm monorepo. The root contains orchestration, CI, shared
tooling, and repository documentation; deployable code lives under `apps/`.

```txt
apps/
  desktop/            Tauri v2 desktop app: React frontend + Rust backend
  mcp-server/         MCP server package
.github/actions/
  setup-sitecmd/       GitHub Action: verify and install an exact CLI release
  sitecmd-gate/       GitHub Action: fail a PR on findings new against the
                      connected baseline
docs/                 Current engineering, product, QA, and operations docs
tools/                Repository tooling and maintained benchmarks
```

The marketing site, the public scanner page, and the Cloudflare Workers live in
the separate SiteCMD-Web repository. `product-facts.json` publishes the values
that side has to restate accurately: check counts, the commercial boundary and
surface status, the Sentry ingest host, and the license-activation deep-link
scheme.
Regenerate it with `pnpm facts:generate` after changing any of their sources.

## Prerequisites

Toolchain versions are pinned in files rather than repeated here. External
utilities link to their installation instructions:

- Rust, on the toolchain `apps/desktop/src-tauri/rust-toolchain.toml` pins.
  rustup applies it automatically to any cargo work under that directory;
  local dev, `verify:push`, and the release build all read the same pin.
- Rust 1.89.0, installed with `rustup toolchain install 1.89.0`, for the MSRV
  checks mirrored by `verify:push` and CI.
- Node.js, on the version `.nvmrc` pins. CI workflows read the same file via
  `node-version-file`.
- pnpm, on the version the `packageManager` field in `package.json` pins
  (`corepack enable` makes that automatic).
- [Tauri's system dependencies for your OS](https://v2.tauri.app/start/prerequisites/),
  including the platform webview and native build tools.
- The [Gitleaks CLI](https://github.com/gitleaks/gitleaks#installing), required
  by the pre-push and publication-history secret scans.
- [cargo-audit](https://github.com/rustsec/rustsec/tree/master/cargo-audit),
  [cargo-deny](https://embarkstudios.github.io/cargo-deny/cli/index.html), and
  [cargo-nextest](https://nexte.st/docs/installation/pre-built-binaries/),
  required by the dependency and Rust test tiers in `verify:push`.
- [actionlint](https://github.com/rhysd/actionlint), installed with
  `go install github.com/rhysd/actionlint/cmd/actionlint@v1.7.12`, for the
  workflow syntax tier in `verify:push`.

## Development

```bash
pnpm install
pnpm tauri:dev
```

Common root commands delegate into the right workspace:

```bash
pnpm dev
pnpm build
pnpm test
pnpm test:desktop
pnpm test:mcp
pnpm e2e
pnpm quality:mcp
pnpm sitecmd -- audit .
```

Direct workspace examples:

```bash
pnpm --filter @sitecmd/desktop run tauri:dev
pnpm --filter sitecmd-mcp run test
```

The desktop app and the MCP server each document themselves through their own
`AGENTS.md`. Start there before changing either.

## Desktop app

The desktop app lives in `apps/desktop/`.

```txt
apps/desktop/src/          React frontend
apps/desktop/src-tauri/    Rust backend, Tauri config, capabilities, CLI
apps/desktop/e2e/          Playwright smoke tests
```

Rust checks can be run directly with the moved manifest path:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
```

## Code Scan audit

```bash
pnpm sitecmd -- audit .
pnpm sitecmd -- audit . --format review --output guardrails-review.md
pnpm sitecmd -- audit . --format github --fail-on high
pnpm sitecmd -- audit . --inspect-local-databases
```

- Local Code Scan is free and needs no account, license, desktop database, or
  connected-service credential.
- The installed release runs as `sitecmd audit`; `pnpm sitecmd -- audit` is the
  source-checkout wrapper for contributors.
- Ordinary audits do not read local dotenv values or open database files or
  connections. `--inspect-local-databases` opts that run into local dotenv
  target discovery and read-only inspection of schema and migration metadata,
  never application table rows. SQLite targets must be inside the audited
  project; every PostgreSQL host must resolve to loopback or a local Unix
  socket. Remote targets are rejected.
- CI guardrails live in `.github/workflows/app-guardrails.yml`.

### Acknowledging Code Scan findings

Commit reviewed suppressions in `.sitecmd/config.json`. A suppression may match
an exact canonical rule, a gitignore-style project-relative path, an occurrence
fingerprint from JSON output, or a combination of those fields. Every entry
requires a reason; `expires` is optional and uses `YYYY-MM-DD`.

```json
{
  "version": 1,
  "url": "https://example.com",
  "name": "Example",
  "code_scan": {
    "suppressions": [
      {
        "match": {
          "path": "examples/**",
          "rule": "code_scan.tls-verification-disabled"
        },
        "reason": "The insecure client is an instructional fixture.",
        "expires": "2026-12-31"
      }
    ]
  }
}
```

Suppression counts remain visible in every report, and ignored findings do not
count toward `--fail-on`. JSON output includes active-finding fingerprints,
full `ignoredFindings` records, and the state of every configured suppression.
Unmatched and expired entries are reported as stale policy so obsolete
acknowledgements can be removed. The same policy is applied before `sitecmd
gate` or a connected CI submission builds its code snapshot.

## The CI gate

`sitecmd gate` audits the checkout in front of it and asks the connected
service which findings are new against the site's baseline, and only those: a
repository that has carried a finding for a year does not start failing because
someone touched an unrelated file. The
[GitHub Action](.github/actions/sitecmd-gate/README.md) wraps it for pull
requests. Both need a site connected to the connected service.

## Documentation

- [Documentation index](docs/README.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [Support](SUPPORT.md)
- [Governance](GOVERNANCE.md)

Repository documentation describes current behavior and accepted decisions.
Generated plans, review transcripts, design-tool exports, browser captures, and
other session artifacts are not committed.

## Contributing

Issues and discussion are open. Unsolicited code contributions are not accepted
yet, and pull requests are limited to invited collaborators while the public
maintenance process is established; [CONTRIBUTING](CONTRIBUTING.md) has the
current policy and the local quality gates.

## License

Brambleworks-authored source is licensed under the
[Apache License 2.0](LICENSE). Bundled third-party material retains its own
license; see [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES).

SiteCMD names and logos are trademarks of Brambleworks LLC. The software
license does not grant trademark rights.

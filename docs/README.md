# SiteCMD Documentation

This directory contains maintained engineering notes, product references, QA runbooks, and operational guides.

Use `docs/` only for information that should be treated as current project truth. Generated plans, review transcripts, one-off audits, acceptance-test results, design exports, and other session artifacts stay outside Git. Promote a durable conclusion into the appropriate document instead of preserving the working transcript.

## Sections

- `engineering/` - architecture, tooling, observability, performance, Tauri, naming conventions, and the connected-service implementation specifications (protocol and state, hosted scanner, alert delivery, maintained surfaces). The business strategy records those specifications fulfill (the publication decision, the connected-service architecture, the commercial terms, and the paid intelligence architecture) are maintained privately in the SiteCMD-Web repository and are named here in plain text only.
- `operations/` - release and launch-smoke runbooks
- `product/` - product behavior, scanner accuracy, and onboarding
- `qa/` - repeatable manual test procedures and review templates

## High-Signal Entry Points

- [Connected service implementation specifications](engineering/connected-service/connected-protocol-spec.md) (protocol and state, hosted scanner, alert delivery, and maintained surfaces)
- [Maintained-surface matrix](engineering/connected-service/maintained-surfaces.md)
- [Entitlement threat model](engineering/entitlement-threat-model.md)
- [Privileged broker threat model](engineering/privileged-broker-threat-model.md)
- [Public repository and release security](engineering/repository-release-security-spec.md)
- [Releasing the desktop app](operations/releasing.md)
- [Tauri command and capability guide](engineering/tauri.md)
- [Unified scan architecture](engineering/unified-scan-architecture.md)
- [Issue and alert architecture](engineering/issue-and-alert-architecture.md)
- [Manual testing runbook](qa/manual-testing-runbook.md)
- [Acceptance review template](qa/acceptance-review-template.md)

## Historical procedures

- [Public repository cutover](operations/publication-checklist.md) (historical: executed 2026-08-21 when the repository became public; kept as the record of the in-place rewrite and the settings it restored)

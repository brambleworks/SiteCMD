# Changelog

Notable user-facing changes to SiteCMD will be recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
releases use [Semantic Versioning](https://semver.org/).

This changelog begins with SiteCMD's first public release. Private development
tags and their generated artifacts are intentionally not carried into the
public repository history.

## [Unreleased]

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

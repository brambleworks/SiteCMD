# Scanner Accuracy Log

Use this as the single place to track scanner trust issues across the acceptance suite.

This file is for launch-level accuracy work, not every small thought from a manual session. If a finding makes SiteCMD feel noisy, misleading, or untrustworthy on a real acceptance project, record it here.

## Status Values

- `open` - discovered but not fixed yet
- `fixed-awaiting-recheck` - code changed, but the acceptance project that found it has not been re-run yet
- `closed` - rechecked on the project that found it and no longer a trust issue

## Entry Types

- `false-positive`
- `false-negative`
- `weak-priority`
- `weak-copy`
- `verification-gap`

## Log

| Date       | Project       | Type             | Surface    | Issue / Check                                                                                                                                                    | Summary                                                                                                                                                                  | Status   | Regression Coverage                                                                                                                                                                                                                                           | Notes                                                                                                                                       |
| ---------- | ------------- | ---------------- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-04-14 | `sitecmd.com` | `false-positive` | `web scan` | `seo.robots_txt`, `seo.sitemap`, `config.sitemap_in_robots`, `seo.llms_txt`, `seo.broken_links`                                                                  | Local preview probes dropped the explicit `:4324` port and did not look for `sitemap-index.xml`, so SiteCMD reported discovery/config issues on a healthy preview build. | `closed` | `origin_with_port_preserves_non_default_localhost_ports`, `sitemap_candidate_urls_include_hyphenated_index_paths`, `sitecmd_landing_preview_golden_slice_matches_expected_statuses`, `sitecmd_landing_slice_score_recovers_when_preview_discovery_is_healthy` | Rechecked on the built localhost preview after the scanner fix pass.                                                                        |
| 2026-04-14 | `sitecmd.com` | `false-positive` | `web scan` | `seo.noindex`, metadata extraction                                                                                                                               | Meta parsing was reading the wrong nearby `content=` attributes and visible page copy, which produced bogus metadata and noindex conclusions.                            | `closed` | `test_meta_description_ignores_neighboring_viewport_content`, `test_noindex_check_does_not_trigger_from_visible_copy`                                                                                                                                         | Rechecked on the project after the parser fix.                                                                                              |
| 2026-04-14 | `sitecmd.com` | `false-positive` | `web scan` | localhost preview server noise (`security.headers.*`, `security.server_info`, `security.insecure_form`, `performance.compression`, preview-only canonical noise) | Local preview-server behavior was being treated like a production deployment, which flooded preview scans with header/server findings users had to mentally ignore.      | `closed` | `test_headers_localhost_preview_are_skipped`, `test_insecure_form_localhost_http_skips_preview_server`, `test_server_info_localhost_preview_is_skipped`, `localhost_preview_canonical_mismatch_is_skipped_when_pointing_to_production`                        | Rechecked on localhost preview. Remaining security findings on that pass looked real or were intentionally skipped as environment-specific. |

## Rules

When an entry lands here:

1. Keep it short and specific.
2. Name the exact issue or check if possible.
3. Note which acceptance project exposed it.
4. Link the regression test or code path once a fix exists.
5. Do not close it until the originating project has been re-run.

## What Belongs Here

- A scanner issue that is factually wrong on a launch project
- A missing issue that should obviously have been caught
- An issue that is technically true but clearly ranked too high or too low
- Guidance that is so vague the user still would not know what to change
- Verification that does not actually prove whether a fix worked

## What Does Not Belong Here

- One-off product polish ideas that do not undermine trust
- Small copy tweaks that do not affect actionability
- Bugs unrelated to scan accuracy or fix-loop trust

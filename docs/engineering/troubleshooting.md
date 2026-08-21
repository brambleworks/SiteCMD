# Troubleshooting

Use this when something feels off. The goal is to get you back to a trustworthy working state quickly.

## Scans are not updating

Start with the simple checks:

- make sure you are looking at the right project and environment
- rerun the scan instead of assuming the previous result refreshed
- check whether the URL you scanned matches the environment URL in the project

If the app still looks stale:

- reopen `Issues` or `Dashboard`
- restart the app once
- rerun the scan and confirm a new result appears in `Activity`

If you are using CLI artifacts, make sure the project was imported from the same repo and URL you are expecting.

## Local vs production environment looks wrong

SiteCMD treats localhost-style URLs as local environments and tries to infer obvious dev or preview URLs when you add a project, but you can always override the label before saving.

If a local preview is showing up as production:

- open the project
- check the environment URL and label
- check whether the wrong label was saved on the primary URL during project setup
- confirm you did not save a localhost preview as the primary production URL

For local work, prefer URLs like:

- `http://127.0.0.1:3000`
- `http://localhost:4321`

For deploy previews, save the preview URL as its own environment instead of replacing the real production URL. If you use Quick Scan first, confirm the environment label before tracking the site as a real project.

## Integration setup is not working

Open `Integrations` and confirm the service is actually connected for the current project.

Then check:

- the integration is enabled
- the account or token has access to the right project or site
- the service itself has fresh data to return

If the data still does not appear:

- disconnect and reconnect the integration
- refresh the page that depends on that data, such as `Dashboard`, `Deploys`, or `Traffic`

## Local database state looks wrong

If the app suddenly looks inconsistent across pages, treat it like a local state problem first, not a content problem.

Common signs:

- a project appears imported in one surface but not another
- scans look stale after a restart
- timeline or Issues looks out of sync with the most recent run

Use this order:

1. Restart the app once.
2. Reopen the project and rerun the scan you care about.
3. If you have a healthy backup, use the import or restore path from Settings instead of trying to hand-edit local files.
4. Reconnect integrations only after the project and scan history look sane again.

If a restore succeeds but the app still feels stale, restart once more so every surface is reading the same current state.

## CLI or MCP output looks incomplete

Every local CLI and MCP capability is free and complete: no scan, issue detail, fix guidance, history, or correlation output is tier-gated, so missing data is a state problem, never a licensing one.

- rerun the scan the output is derived from; most gaps are a stale or missing local scan
- for the MCP server, make sure it is reading the correct database path, and restart the AI tool or MCP server after changing database overrides
- if you override the DB path, verify `SITECMD_DB_PATH` points at the same SiteCMD database the desktop app is using

Connected-service and catalog credentials are the only things a license entitles; manage those from Settings in the desktop app.

## Basic recovery path

If you are not sure what state the app is in, use this reset order:

1. Confirm the project URL and environment are correct.
2. Rerun the scan you care about.
3. Reopen `Issues` and `Activity`.
4. Restart the app if the UI still looks stale.
5. Reconnect the affected integration if the problem is data-related.

That should get most local-state problems back to a known good baseline without risky cleanup steps.

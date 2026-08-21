# Get Value In 5 Minutes

Use this when you want the fastest path from install to a real fix.

## 1. Add one real project

Pick a site you actually care about.

- In the desktop app, click `Add Project`.
- Add the main URL you want SiteCMD to open first, and make sure the environment label matches what it really is.
- Use `Production` for the real live site, `Staging` for preview deploys, and `Local` for localhost or tunnel URLs.
- If you have the repo locally, link the folder too.

If you already used Quick Scan, still save the site as a real project. Quick Scan is a preview. The tracked project is what powers Issues, Activity, verification, and follow-up work.

## 2. Run the first Full Scan

Start with a Full Scan from the project home screen.

That gives SiteCMD the baseline it needs for:

- the `Dashboard`
- the `Issues` page
- score history
- activity tracking
- verification after a fix

If you linked a project folder, the Full Scan runs the live-site Web Scan first and then runs Code Scan against the local project. If no folder is linked yet, SiteCMD still creates a live-site baseline and should make the missing project-folder step obvious.

## 3. Start from Dashboard

After the first Full Scan, SiteCMD should land on `Dashboard` with enough context to understand what happened and what to do next.

The Dashboard should answer three questions quickly:

- what changed
- what needs attention first
- where should I go next

From there, open the highest-priority `Issues` or `Updates` card. Start with one item that is:

- `critical` or `high`
- clearly explained
- easy to verify after the fix

## 4. Fix one thing all the way through

Open the issue and look for:

- the plain-language summary
- why it matters
- likely fix direction
- verification guidance

Do not try to clear the whole list on day one. Fix one meaningful issue end to end so you can trust the loop.

## 5. Verify the fix

Run the suggested verification flow or rerun the scan.

You should be able to tell, without guessing:

- whether the issue is gone
- whether the score moved
- whether anything regressed

If verification is vague, treat that as product feedback. SiteCMD should make that answer obvious.

## 6. Then add depth

Once the first Full Scan loop feels solid, add the next layer that matches how you work:

- `Integrations` if you want uptime, analytics, deploys, or search signals in the app
- `Updates` if you want dependency drift and security patches surfaced alongside scan work

## What success looks like

You are getting value if, inside a few minutes, SiteCMD helps you:

- find a real issue
- understand why it matters
- make the fix
- verify that it actually worked

That is the core promise. Everything else should support that loop.

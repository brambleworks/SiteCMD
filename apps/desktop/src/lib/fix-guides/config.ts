import type { FixGuideEntry } from "./types";

export const CONFIG_FIX_GUIDES: Record<string, FixGuideEntry> = {
  "config.favicon": {
    effort: "quick",
    effortMinutes: 3,
    lead: "This site has no working icon, so browser tabs and bookmarks show a blank or broken image instead of your brand mark.",
    default: [
      'Choose a simple brand mark that stays recognizable at favicon sizes, declare each intended icon with an accurate URL, type, and size, for example `<link rel="icon" href="/favicon.svg" type="image/svg+xml">`, and remove stale or conflicting declarations. Fetch the deployed icons and verify status, Content-Type, and appearance in light and dark browser tabs, allowing for aggressive favicon caching when validating a replacement.',
    ],
  },
  "config.localhost_refs": {
    effort: "quick",
    effortMinutes: 5,
    lead: "Your local preview output contains loopback addresses that a production build may replace before it ships.",
    default: [
      "Inspect the exact reference in the served code and identify who resolves it; a loopback address can be correct for an intentional local companion service or a documented development-only path. If the browser should call the deployed origin, switch to a relative or canonical same-origin route, or inject a validated public endpoint with no silent localhost fallback, then exercise the feature from a different device to confirm every destination matches the intended topology.",
    ],
  },
  "config.responsive_design": {
    effort: "involved",
    effortMinutes: 30,
    lead: "This page breaks or becomes hard to use at narrow screen widths, which hurts the experience for anyone on a phone.",
    default: [
      'Add `<meta name="viewport" content="width=device-width, initial-scale=1">` if missing, without disabling zoom, then resize from the narrowest supported viewport through wide desktop and fix breaks with fluid sizing, wrapping, and grid/flex behavior at content-driven breakpoints. Do not hide navigation or content as a generic mobile fix, avoid horizontal scrolling except where the content requires it, and verify with zoom, large text, and orientation changes that nothing is clipped or unreachable.',
    ],
  },
  "config.analytics": {
    effort: "quick",
    effortMinutes: 5,
    lead: "This site has no analytics configured, so there is no way to see how visitors actually find or use it.",
    default: [
      "Decide first whether the product needs analytics; running none can be a deliberate privacy decision, so mark the finding reviewed when that is intentional. If you add a provider, measure only the pages and events you need, exclude secrets, form contents, and sensitive URL parameters, apply the consent and disclosure rules your jurisdictions require, and verify with browser Network tools that disabled or opted-out states send nothing unintended.",
    ],
  },
  "config.custom_404": {
    effort: "quick",
    effortMinutes: 5,
    lead: "Visiting an unknown page on this site does not show a proper not-found page, leaving a lost visitor with no way back.",
    default: [
      "Create a not-found page that uses the site's normal shell, states the resource was not found, and offers recovery actions such as navigation or search, without exposing stack traces. Configure the router, server, or CDN so a genuinely missing URL returns an HTTP 404 status with that body rather than a 200 catch-all or home-page redirect, then test unknown paths both by direct navigation and after client-side navigation.",
    ],
  },
  "config.www_redirect": {
    effort: "quick",
    effortMinutes: 5,
    lead: "Both the www and plain versions of your domain serve content on their own instead of one leading to the other.",
    default: [
      "Serving both hosts can be intentional, but duplicate public content needs one coherent strategy: pick the public host from the site's existing links, certificates, cookies, and search configuration, then redirect the alternate known hostname at the edge or web server to that fixed destination, preserving path and query. Never build the destination from an unvalidated `Host` header. Request both hosts over HTTP and HTTPS to confirm a single hop, correct 301/308 semantics, and certificate coverage.",
    ],
  },
  "config.deprecated_html": {
    effort: "quick",
    effortMinutes: 5,
    lead: "This page uses outdated HTML elements that browsers can render inconsistently and that no longer match modern styling.",
    default: [
      "Open each occurrence the issue lists and replace presentation-only elements such as `center`, `font`, and `big` with semantic HTML plus CSS, preserving meaning rather than making a mechanical substitution (`<s>` for outdated information, `<del>` for an actual deletion). Remove `blink`/`marquee` effects unless motion is genuinely necessary, and redesign obsolete framesets as ordinary documents instead of assuming `frame` maps to `<iframe>`. A validator helps review but does not guarantee complete coverage.",
    ],
  },
  "config.console_logs": {
    effort: "quick",
    effortMinutes: 5,
    lead: "Debugging messages are visible in the live site's browser console, which can expose internal details to any visitor.",
    default: [
      "Determine which of the flagged console statements execute in the production path; some can be environment-gated, third-party, or intentional diagnostics, so do not delete every console method by default. Remove obsolete debugging and any output containing credentials, tokens, personal data, or internal state, route real failures through existing structured monitoring, and inspect a production build to confirm sensitive output is gone while useful diagnostics remain.",
    ],
  },
  "config.debug_mode": {
    effort: "quick",
    effortMinutes: 5,
    lead: "The live site appears to be running with debug mode on, which can expose stack traces or internal state to visitors.",
    default: [
      "Search the production environment for debug flags such as `DEBUG=true`, `NODE_ENV=development`, framework-specific debug settings, or CMS admin debug toggles, and disable them. Check the live site for visible debug UI: error stack traces, debug panels, development toolbars, or verbose errors shown to users. Prefer failing production startup when a debug flag is enabled over logging environment values into a client bundle or shared logs.",
    ],
  },
  "config.dev_dependencies": {
    effort: "quick",
    effortMinutes: 5,
    lead: "A development tool or route that was never meant for the public may still be reachable on the live site.",
    default: [
      "Check whether development tools are reachable in production by trying routes like `/phpinfo.php`, `/_profiler`, `/graphiql`, and `/playground`, and remove any debug routes, test/fixture endpoints, or development-only middleware. Keep dev dependencies in the trusted build stage: the final runtime artifact should contain only the compiled output and the runtime dependencies it needs, verified by inspecting the artifact itself. An exposed debug route is higher risk than a merely present test library.",
    ],
  },
  "config.placeholder_content": {
    effort: "quick",
    effortMinutes: 5,
    lead: "This page still shows placeholder text or stock imagery, which reads as an unfinished site to anyone who visits.",
    default: [
      "Search the site's visible text for markers like 'Lorem ipsum', 'TODO', 'example.com', and 'John Doe', and check images for placeholder services or stock watermarks. Replace them with real content, or move unfinished pages to a draft/unpublished state instead of publishing placeholders. If a placeholder page was publicly crawlable, search systems may retain it until recrawl, so verify the deployed response rather than expecting a specific ranking effect.",
    ],
  },
  "config.print_stylesheet": {
    effort: "moderate",
    effortMinutes: 10,
    lead: "Printing this page can produce clipped content or unreadable colors instead of a clean, usable printout.",
    default: [
      "If users need to print or save this page as PDF, open print preview and identify actual failures such as clipped content, unreadable colors, or broken page breaks; mark the finding reviewed when printing is intentionally out of scope. Fix real problems with targeted `@media print` rules that hide only interactions with no printed value, preserve required legal and context content, and control page breaks, then re-check print preview and print-to-PDF while confirming the on-screen stylesheet is unchanged.",
    ],
  },
  "config.sitemap_in_robots": {
    effort: "quick",
    effortMinutes: 2,
    lead: "Your robots.txt file does not point to a sitemap, missing an easy hint that helps search engines find your pages.",
    default: [
      "Treat a `Sitemap:` line as an optional discovery hint, not an indexing requirement. If the site has a canonical public sitemap crawlers should discover here, add its absolute URL to robots.txt on its own line, then fetch both deployed files to confirm status, body, and that only canonical indexable URLs are advertised. Search-console submission also provides discovery, so mark the finding reviewed when the omission is deliberate.",
    ],
  },
  "config.todo_comments": {
    effort: "quick",
    effortMinutes: 5,
    lead: "Leftover developer notes are visible in the deployed code, which can hint at unfinished work or expose sensitive detail.",
    default: [
      "Inspect each surfaced marker in the deployed code; a TODO can be harmless maintainer context, a required license comment, or third-party code, so the marker alone does not prove an unfinished feature. Remove comments that expose credentials, sensitive endpoints, or exploitable detail, and if comments should be stripped, configure the existing production minifier with explicit license behavior and inspect the emitted artifacts, recording any real outstanding work in the team's issue system.",
    ],
  },
  "config.trailing_slash": {
    effort: "quick",
    effortMinutes: 5,
    lead: "A page on your site is reachable both with and without a trailing slash, and both may count as separate duplicate pages.",
    default: [
      "Request the flagged paths in both `/path` and `/path/` forms and compare status, final URL, body, and canonical metadata; source-link differences alone do not prove duplicate public pages. If both variants expose unintended duplicates, adopt the convention your framework or host already supports, add one fixed canonical redirect for the other form, update internal links and sitemap entries, and re-test. If both forms are intentional or already normalize to one canonical response, document that instead of changing routing.",
    ],
  },
  "config.web_manifest": {
    effort: "quick",
    effortMinutes: 15,
    lead: "This site declares a web app manifest, but the file is missing, broken, or does not match how the site behaves.",
    default: [
      "Decide first whether the product intends an installable experience; if not, remove the accidental manifest declaration and mark the finding resolved, because a normal website does not need to become a PWA to satisfy this check. If installation is intended, fetch the declared manifest URL, confirm valid JSON rather than a catch-all HTML shell, keep name, scope, start URL, icons, and colors truthful to actual app behavior, and test install and launch in representative browsers.",
    ],
  },
};

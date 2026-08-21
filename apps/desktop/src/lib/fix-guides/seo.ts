import type { FixGuideEntry } from "./types";

export const SEO_FIX_GUIDES: Record<string, FixGuideEntry> = {
  "seo.title": {
    effort: "quick",
    effortMinutes: 3,
    default: [
      "Add a unique, descriptive `<title>` in `<head>` that matches the page's visible primary heading and actual purpose, with the differentiating topic near the start. Search snippets are truncated by rendered width and sometimes rewritten, so preview representative titles and remove filler rather than forcing a fixed character count. Check the rendered server HTML so indexable pages get distinct, non-duplicate titles.",
    ],
  },
  "seo.meta_description": {
    effort: "quick",
    effortMinutes: 3,
    default: [
      'Add a concise, accurate `<meta name="description">` summarizing what the page offers, in natural language without keyword stuffing or claims absent from the visible content. Treat 150-160 characters as a rough drafting range rather than a pass/fail rule, since search engines may rewrite the text. Give important indexable pages distinct descriptions and omit low-value boilerplate instead of duplicating it site-wide.',
    ],
  },
  "seo.canonical": {
    effort: "quick",
    effortMinutes: 3,
    default: [
      'Add one `<link rel="canonical">` in `<head>` pointing to a single absolute, crawlable, final HTTPS URL with the preferred host and path; a self-referencing canonical is usually appropriate for a standalone indexable page. Keep internal links, redirects, sitemap entries, and hreflang consistent with it, and treat it as a consolidation hint for genuinely duplicate content, not a redirect or indexing command.',
    ],
  },
  "seo.open_graph": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Add page-specific `og:title`, `og:description`, `og:image`, `og:url`, and `og:type` meta tags that match the visible page, with an absolute, publicly fetchable image URL in the dimensions and format supported by the platforms that matter. Fetch the deployed HTML and image as a logged-out client, then verify previews with current platform debuggers and refresh their caches where supported.",
    ],
  },
  "seo.structured_data": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "First identify a current consumer or search feature that is useful for this exact page; structured data is optional, and generic markup added solely to clear the issue creates noise. If a supported use exists, add JSON-LD that accurately describes visible content, keep every value truthful and synchronized with the production page, and validate with Schema.org tooling. Valid markup enables consideration; it does not guarantee a search treatment.",
    ],
  },
  "seo.structured_data.invalid": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      'Find the failing `<script type="application/ld+json">` block at the position and line/column the issue reports, then fix the syntax: JSON-LD must be strict JSON with double-quoted keys and strings, no trailing commas, no comments, and no JavaScript expressions. Validate the corrected block with a strict JSON parser and https://validator.schema.org, then re-scan; a consumer may ignore a block it cannot parse.',
    ],
  },
  "seo.structured_data.incomplete": {
    effort: "quick",
    effortMinutes: 15,
    default: [
      "First confirm which consumer profile is relevant, then add only applicable properties from the issue list with values that are true, represented by the visible page, and kept synchronized with bylines, dates, and prices. Check the target consumer's current documentation for feature-specific required and recommended properties, then re-validate; completeness does not guarantee a rich result or citation.",
    ],
  },
  "seo.robots_txt": {
    effort: "quick",
    effortMinutes: 3,
    default: [
      "Fetch the deployed `/robots.txt` and confirm whether the root-wide `Disallow` policy is intentional; a missing robots.txt is optional and normally means no crawl restrictions, so do not create one solely to clear this finding. If the site should be crawled, remove the wildcard disallow or narrow it to the exact low-value or unsafe-to-crawl paths, and treat robots rules as public crawl guidance, not authentication or guaranteed de-indexing.",
    ],
  },
  "seo.sitemap": {
    effort: "moderate",
    effortMinutes: 10,
    default: [
      "Create a valid XML sitemap at a stable public URL, generated from the same routing/content source as the site, listing canonical absolute URLs and excluding redirects, errors, blocked/noindex pages, duplicates, and non-canonical variants. Advertise it from robots.txt, submit it to the search consoles the product uses, and fetch the deployed URL to confirm real XML rather than a catch-all HTML shell; submission helps discovery but does not guarantee indexing.",
    ],
  },
  "seo.headings": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Use headings to label the page's real sections, choosing levels from content structure so subsections nest beneath their parents, and use CSS for visual styling. Multiple h1s and skipped levels are not automatically failures, but a single clear top-level heading is often easier to maintain, and unexpected jumps often reveal that visual size chose the tag; inspect the rendered outline with an accessibility tree.",
    ],
  },
  "seo.noindex": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "First decide whether this exact URL is meant to appear in search; account, duplicate, staging, filtered, and private-workflow pages often use noindex deliberately, so a detected directive is not automatically a defect. If the page should be indexed, remove only the unintended `noindex` token from every robots meta tag and `X-Robots-Tag` source, keep the URL crawlable, and verify in the relevant search console; removal permits consideration but does not guarantee indexing.",
    ],
  },
  "seo.viewport": {
    effort: "quick",
    effortMinutes: 2,
    default: [
      'Add `<meta name="viewport" content="width=device-width, initial-scale=1">` in `<head>` and avoid zoom restrictions like `maximum-scale` or `user-scalable=no`, which can create an accessibility barrier. The tag only gives mobile browsers a viewport baseline, so test rendered pages at narrow widths, high zoom, and large text rather than inferring responsiveness from it.',
    ],
  },
  "seo.og_image_relative": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Replace the relative `og:image` or `og:url` value with a full absolute HTTPS URL including scheme and host; some consumers resolve relative values while others reject or misresolve them, and protocol-relative URLs inherit a context the crawler may not share. Inspect the rendered production tag rather than assuming framework metadata made it absolute, then fetch the page and image logged out and refresh cached previews with current social debuggers.",
    ],
  },
  "seo.og_image_status": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Start from the observed status and Content-Type: a 404 or 410 is direct missing-at-probe-time evidence, while another status, timeout, or access response is a review state, not proof that every social crawler sees a broken image. Fetch the exact deployed `og:image` URL with GET while logged out, correct only what the evidence supports (missing asset, HTML catch-all, unintended authentication, or a narrowly verified crawler/CDN rule), then re-check each platform with its preview or rescrape tooling since previews are cached.",
    ],
  },
  "seo.temporary_redirect": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "SiteCMD flags a 302, 303, or 307 only when that hop changes HTTP to HTTPS or swaps the www/apex host; not every temporary redirect should become permanent. Trace which layer owns the response, and if the normalization is intentionally permanent, switch it to 301 or 308 using current method/body semantics while preserving temporary statuses for authentication, consent, locale, or outage flows. Test the deployed chain for one direct hop to the intended final response with no loop.",
    ],
  },
  "seo.charset": {
    effort: "quick",
    effortMinutes: 2,
    default: [
      'Declare the encoding with `<meta charset="utf-8">` placed first inside `<head>`, keeping it within the first 1024 bytes so browsers do not need to guess, or declare it on the response header as `Content-Type: text/html; charset=utf-8`, which alone satisfies the requirement. Reload a page containing accented characters or curly quotes and confirm they render correctly.',
    ],
  },
  "seo.twitter_cards": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "First decide whether X/Twitter presentation matters for this page: the finding means no explicit `twitter:card` type was observed, and complete Open Graph fields supply some fallback values but do not request a particular card layout. When a card is in scope, add a supported type such as `summary_large_image` with truthful page-specific title, description, alt text, and an absolute image URL meeting current platform limits, then verify the crawler can fetch the page and image and test with current preview tooling.",
    ],
  },
  "seo.hreflang": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      'First confirm the URLs are localized equivalents; a single-language page needs no hreflang. Choose one consistently managed channel, emit `rel="alternate"` links with consumer-supported BCP 47 values and fully qualified canonical final URLs, include the current page under its own language value, and add the return annotations the target search engine requires. Treat `x-default` as optional; add it only when a real unmatched-language destination exists.',
    ],
  },
  "seo.duplicate_meta": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Use the issue evidence to distinguish the two cases: multiple title/description declarations on the same page, or the same effective value repeated across scanned pages. Consolidate same-page conflicts into one authoritative metadata path, and across pages give genuinely distinct indexable content accurate page-specific titles and descriptions while preserving intentional repetition for equivalent, paginated, localized, or application states.",
    ],
  },
  "seo.duplicate_h1": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Review the surfaced URLs and confirm they actually have different primary topics; SiteCMD compares only the first non-empty H1 in each initial HTML response. Give genuinely distinct pages accurate visible page-level headings by tracing shared layout/CMS defaults, but keep a repeated H1 when pages intentionally present the same topic or equivalent content; do not rewrite headings solely to manufacture uniqueness.",
    ],
  },
  "seo.url_structure": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Choose stable, readable URLs that reflect the content, but do not rename a working URL solely because it contains an underscore, uppercase character, identifier, or necessary parameter. Remove session identifiers and sensitive tracking parameters from public links; when restructuring, map old URLs to 301/308 only where a genuine replacement exists, return 404 or 410 otherwise, and keep internal links, sitemap entries, and redirects consistent.",
    ],
  },
  "seo.broken_links": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Re-request each surfaced internal URL with a normal GET through the deployed host; timeouts, transient 5xx responses, authentication, rate limits, and bot controls need a different fix from a permanently missing page. Update or remove wrong links, add a 301/308 only when the content has a genuine replacement, keep a truthful 404/410 when none exists, and fix shared templates and components first.",
    ],
  },
  "seo.broken_external_links": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Verify each surfaced external URL with a normal browser or GET from a representative network, distinguishing a persistent 404/410 or domain failure from a transient timeout, geo/bot block, or a server that rejects automated requests; re-check transient failures after a delay. For persistently unavailable destinations, use an authoritative updated URL, replace the citation with another primary source, or remove the link, linking to an archive only when appropriate and labeled as archived.",
    ],
  },
  "seo.thin_content": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "Review the rendered primary content in its language and page type first: whitespace word counts are unreliable for CJK and other scripts, and a short contact, login, calculator, or product page can completely satisfy its purpose, so the count is a triage signal, not a quality verdict. Where the page should answer an informational query, add original material that resolves the visitor's task rather than padding to a threshold, and consolidate or noindex true near-duplicate pages when that matches search intent.",
    ],
  },
  "seo.canonical_mismatch": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Resolve the rendered canonical against the final response URL and compare normalized scheme, host, path case, trailing slash, encoding, and meaningful query parameters. Fix only an unintended mismatch: a self-canonical usually suits standalone content, an intentional cross-page canonical should point directly to a crawlable equivalent, and pagination or localized pages should not collapse unless the target truly represents them. Canonical remains a hint, not a redirect or access control.",
    ],
  },
  "seo.meta_conflicts": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      'Identify exactly which template, plugin, proxy, or CDN emits each rendered `<meta name="robots">` value and the final `X-Robots-Tag` header, since crawlers generally combine directives and the most restrictive result can win. Choose the intended indexing, snippet, and preview behavior for this page type, keep one authoritative configuration where practical, and verify the final deployed HTML and header values on success, redirect, and error responses.',
    ],
  },
  "seo.page_speed_hints": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      'Treat this as a source-order candidate review, not a Core Web Vitals measurement: SiteCMD saw a first non-lazy `<img>` without `fetchpriority=high`, which does not establish that it is the rendered LCP element. Capture a performance trace first, and only when measurement confirms an above-the-fold image is the late-scheduled LCP resource, avoid lazy-loading it and add `fetchpriority="high"` to the correct `<img>`, then re-measure; unnecessary high priority can starve other resources.',
    ],
  },
  "seo.llms_txt": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "If AI citation or docs discovery matters for this site, create an optional `/llms.txt` file at the site root with the site name, what it covers, the pages that matter most, and preferred citation or usage guidance. Keep it short, factual, and consistent with the public site, verify it is accessible at `yourdomain.com/llms.txt`, and treat it as an emerging convention rather than a guaranteed ranking or citation control that every tool will read.",
    ],
  },
  "seo.ai_crawler_blocking": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Review every surfaced user-agent token against the provider's current official documentation, separating search/discovery indexing, user-triggered retrieval, model training and data-use controls, and advertising validation, since similarly named tokens can have different effects. Allow only products the organization intentionally supports and preserve blocks that match legal, licensing, privacy, and commercial policy; do not bulk-enable all AI-labelled crawlers merely to clear the finding, and remember a robots rule is public crawl guidance, not authentication.",
    ],
  },
  "seo.sitemap_freshness": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Emit `<lastmod>` in W3C datetime format only when the source system knows a meaningful modification time for that URL's primary content, generated from the CMS record or a page-specific content dependency; do not stamp every URL with the build time or use a commit that changed unrelated code. Omit the value when no trustworthy date exists, and confirm rebuilds change only URLs whose content meaningfully changed.",
    ],
  },
  "seo.citation_meta": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "First confirm this URL is an authored article, research paper, or other work where attribution is relevant; product, account, utility, and ordinary landing pages do not need citation metadata. For editorial content, show a truthful byline connected to suitable metadata such as Article JSON-LD with an `author`; for academic or research content consumed by systems that document Highwire-style metadata, add the fields that system expects, such as `citation_author` and `citation_title`, without assuming they are universal ranking signals.",
    ],
  },
  "seo.content_freshness": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "First confirm publication or modification dates are meaningful for this content type; do not add a synthetic date to evergreen landing pages or update it on every deployment. For an article, expose the truthful publication date visibly and, when useful, in Article JSON-LD or `article:published_time`, add a modified date only after a meaningful content change, and treat the `Last-Modified` header as an HTTP timestamp, not proof of an editorial update.",
    ],
  },
  "seo.organization_identity": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "First decide whether Organization, Person, or another entity accurately represents the site or publisher, then put one authoritative JSON-LD identity node with a stable `@id` on the homepage or shared graph and reference it from page-specific markup instead of duplicating inconsistent identities. Include only true public properties the entity supports, such as official name, canonical URL, logo, and `sameAs` profiles the organization controls; identity markup does not guarantee recognition or ranking.",
    ],
  },
  "seo.faq_schema": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Do not add question-oriented markup solely to obtain an assumed search treatment; check the target consumer's current supported-feature documentation first, since feature lists and eligibility change. Keep `FAQPage` or `HowTo` only when it truthfully describes visible content and serves a documented use, and note that `QAPage` is intended for a page centered on one question where users submit answers, not an authored FAQ; markup presence does not establish eligibility, a rich result, or ranking.",
    ],
  },
  "seo.semantic_html": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Inspect the initial HTML evidence and the runtime DOM before changing markup; `<main>` and `<article>` are optional, and the source check does not evaluate existing landmark roles or client-rendered structure. Where the page has a distinct primary content region, expose one visible main landmark, use `<article>` only for self-contained content that could stand independently, match `<nav>`, `<header>`, `<footer>`, `<section>`, and `<aside>` to their real semantics, and reinspect the rendered accessibility tree.",
    ],
  },
  "seo.source_citations": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Identify factual claims a reader would reasonably need to verify and cite the strongest available primary source near each claim: official documentation or data, standards, legislation, or original research. A page that makes no externally verifiable claim needs no outbound citations; use descriptive link text, check each source's date, methodology, and stability, and do not add links merely to hit a count.",
    ],
  },
  "seo.js_only_content": {
    effort: "involved",
    effortMinutes: 60,
    default: [
      "Fetch the deployed URL while logged out and compare the raw response HTML with the rendered DOM; a mount container and a small source-text estimate do not prove the page is empty, and an intentionally client-only application may not need changes. If important public content is genuinely absent from the response, use the installed framework's current server-rendering, static-generation, or targeted prerendering path for that route, then re-fetch with JavaScript disabled to confirm content and metadata appear and hydration still behaves.",
    ],
  },
  "seo.orphan_pages": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Treat the result as a bounded scanned graph, not proof a page is globally orphaned; SiteCMD only considers initial-HTML links between scanned pages and cannot see rendered navigation, authenticated states, or direct visits. Verify with a complete production crawl, then link useful discoverable pages from navigation, an index page, or related content where people would actually look, and keep deliberate landing or workflow routes unlisted, handling their sitemap and indexability separately.",
    ],
  },
  "seo.noindex_in_sitemap": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Review the contradiction: the scanned response carried `noindex` while the URL appeared in the sitemap, which can be an accidental exclusion, stale sitemap output, or a deliberate exception. If the URL should be indexable, remove every unintended noindex source across templates, plugins, origin headers, and edge rules and keep it crawlable; if exclusion is intentional, remove the URL from index-oriented sitemap generation. Then regenerate and fetch the deployed sitemap and page to confirm they agree; neither state makes a page private or guarantees an indexing outcome.",
    ],
  },
  "seo.canonical_loop": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Confirm the URLs are duplicate or near-duplicate representations, choose the crawlable final URL that best represents the content, and point each duplicate's canonical directly at that representative rather than through another canonical; a standalone representative commonly self-canonicalizes. Trace shared templates, request-host logic, rewrites, framework metadata, and edge headers so the cycle or chain is fixed at its source, and do not consolidate localized, paginated, or distinct content unless the target truly represents it.",
    ],
  },
  "seo.hreflang_reciprocity": {
    effort: "moderate",
    effortMinutes: 25,
    default: [
      "Confirm each surfaced source/target pair represents localized equivalents; a return annotation may legitimately be delivered through HTTP headers or a sitemap that the page check does not compare. For pairs the target search engine should process, add the missing return relationship and a language-specific self-reference through one consistently managed channel, generated from one shared locale map, using consumer-supported BCP 47 values and fully qualified canonical final URLs; a documented partial bidirectional set can be intentional.",
    ],
  },
};

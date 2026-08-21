import type { FixGuideEntry } from "./types";

export const PERFORMANCE_FIX_GUIDES: Record<string, FixGuideEntry> = {
  "performance.lcp": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "SiteCMD records one lab navigation, so confirm the slowdown across representative runs, then use a browser Performance trace to identify the actual Largest Contentful Paint element. Split LCP into server response, resource discovery/load delay, download, and render delay and optimize the dominant segment; for an image LCP, serve a correctly sized compressed image that is discoverable in the initial HTML and not lazy-loaded, and for text, prioritize the required CSS and fonts.",
    ],
  },
  "performance.cls": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Record a Performance trace with layout-shift regions enabled and reproduce the largest shift cluster before changing layout code. Reserve space for images, video, ads, embeds, and late-loading components with intrinsic dimensions, `aspect-ratio`, or stable placeholders; avoid inserting banners or consent UI above rendered content, use transform/opacity for motion, and choose font-loading behavior that limits text reflow.",
    ],
  },
  "performance.fcp": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Use a browser Performance trace to determine what keeps the page blank before First Contentful Paint; FCP is a supporting loading metric, not one of the three Core Web Vitals. Check the document response, render-blocking stylesheets, synchronous scripts, fonts, and client-only rendering, fix the measured blocker rather than applying every optimization at once, and deliver the minimum CSS and content needed for the first visible render early.",
    ],
  },
  "performance.long_task_blocking": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "Treat this as SiteCMD's observed post-FCP long-task sample, not Lighthouse Total Blocking Time; the 200/600 ms bands are review heuristics, so reproduce it in a browser Performance trace first. Then open the longest main-thread tasks, trace them to application code, hydration, third-party scripts, parsing, or rendering, and reduce, split, defer, or move that work to a worker, starting with the largest task.",
    ],
  },
  "performance.images": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Treat this aggregate finding as a pointer to the specific image evidence. Inspect the rendered image, selected `currentSrc`, viewport position, dimensions, and transferred bytes first, because source markup alone cannot establish those facts. Then apply only the matching remediation: lazy-load images that begin sufficiently off-screen, reserve the correct aspect ratio, or compare encodings at equivalent visual quality, keeping eager loading for a measured LCP image.",
    ],
  },
  "performance.cache": {
    effort: "moderate",
    effortMinutes: 10,
    default: [
      "Classify each response by whether its URL is content-versioned, public or personalized, and how quickly it must become fresh. Reserve `public, max-age=31536000, immutable` for assets whose URL changes whenever bytes change; give mutable assets and HTML a freshness/revalidation policy, and never cache personalized or authorization-dependent content in a shared cache without a complete safe key and variation design.",
    ],
  },
  "performance.asset_caching": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Confirm the build never overwrites the fingerprinted-looking URL with different bytes; only content-versioned assets are safe for a long immutable cache. For verified immutable assets, serve `Cache-Control: public, max-age=31536000, immutable` at the CDN or static-file layer scoped to those paths, and keep HTML and any mutable URL on a short `max-age` with revalidation.",
    ],
  },
  "performance.images.lazy": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      'Treat the finding as a candidate list and check actual image positions across responsive breakpoints. Add native `loading="lazy"` only to images that begin well outside the initial viewport, keep the LCP image and near-viewport images eager, and set width/height or an aspect ratio so lazy loading does not introduce layout shift.',
    ],
  },
  "performance.images.format": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      'Encode representative images as WebP and AVIF and compare bytes plus visual quality with the original; modern formats are not always smaller for tiny, flat-color, already optimized, animated, or transparency-heavy assets, so keep the best measured variant. Serve the winner with a supported fallback, for example a `<picture>` element with a `type="image/webp"` source, and verify Content-Type and transfer bytes on the deployed page.',
    ],
  },
  "performance.images.dimensions": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Give each flagged `<img>` its intrinsic aspect ratio with numeric `width` and `height` attributes, or a CSS `aspect-ratio` or stable container when intrinsic dimensions are not available. Let responsive CSS change the rendered size while preserving the ratio, then reload with the image cache disabled and confirm the browser reserves the slot without distortion or a late shift.",
    ],
  },
  "performance.render_blocking": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Record a production Performance trace to identify which stylesheets or parser-blocking scripts actually delay the first useful render; the presence of a head resource alone does not prove it should be deferred. Use `defer` or modules for scripts that can run after parsing, remove unused CSS before considering inlining, keep truly synchronous bootstraps synchronous, and re-test slow-network and no-JavaScript behavior for flashes or missing styles.",
    ],
  },
  "performance.compression": {
    effort: "moderate",
    effortMinutes: 10,
    default: [
      "Confirm the finding on a representative text response with `Accept-Encoding: br, gzip`, then enable Brotli and/or gzip for appropriate textual content types at the authoritative layer (origin, reverse proxy, or CDN). Do not compress already compressed media, add `Vary: Accept-Encoding` where the cache does not handle variants automatically, consider compression side-channel risk for responses that mix secrets with attacker-controlled bytes, and verify the deployed URL with and without `Accept-Encoding`.",
    ],
  },
  "performance.fonts": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Inspect the production font requests and text-render timeline first. Choose `font-display` per product behavior (`swap`, `fallback`, or `optional` are common tradeoffs), subset to the characters and weights actually used, and preload only a font file proven necessary for above-the-fold text; there is no universal file count, so measure transfer, render timing, and CLS across representative routes.",
    ],
  },
  "performance.page_weight": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Start from the exact fetched HTML document size in the evidence and profile what contributes bytes: rendered content, serialized state, repeated markup, inline styles/scripts/SVG/data URIs, or debug output; the check does not attribute the cause. Remove unintended duplication and debug data first, defer or paginate a section only when it is genuinely non-critical, and treat the 1 MB and 3 MB bands as review thresholds, not universal product budgets.",
    ],
  },
  "performance.asset_weight": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Read the coverage fields first: SiteCMD measures a bounded asset sample and the largest responsive candidate per group, not one browser navigation. Record a production trace, rank resources by actual critical-path and transferred bytes plus CPU cost, then remove unused resources, right-size and recompress images, reduce shipped code, and enable suitable text compression. Treat the 2.5 MB and 5 MB sampled-sum bands as review thresholds rather than universal page budgets.",
    ],
  },
  "performance.broken_images": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Check each sampled candidate's HTTP outcome against what the browser actually selects at representative viewports; an inconclusive probe is not a confirmed failure. For a selected 4xx/5xx response, restore the intended file, correct routing or the case-sensitive path, or remove the stale reference, and do not make a private image public merely to clear an unauthenticated probe.",
    ],
  },
  "performance.images.heavy": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Inspect the browser's selected candidate and rendered dimensions at representative breakpoints; SiteCMD may measure the largest srcset candidate, so a response over 300 KB is a review threshold rather than proof that every visitor downloads it. If the selected candidate is oversized for its rendered use, generate appropriate widths with correct `srcset`/`sizes` and compare supported encodings at equivalent visual quality; do not reduce quality solely to cross the band.",
    ],
  },
  "performance.http2": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Check the protocol negotiated at the deployed public hostname, for example `curl -sS -o /dev/null -w '%{http_version}\n' https://yourdomain.com`; a result of `2` or `3` is a modern multiplexed transport. If needed, enable HTTP/2 at the authoritative browser-facing server or CDN, verifying certificate and ALPN configuration together and accounting for any CDN or proxy in front of the origin.",
    ],
  },
  "performance.dom_size": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "Reproduce the measured DOM count in the rendered state that triggered the finding; count alone is a heuristic, not proof of a user-visible slowdown. For a measured long-list problem, paginate, incrementally render, or virtualize while preserving keyboard navigation, focus, and screen-reader semantics, and remove wrappers only when they have no semantic, styling, containment, or event purpose.",
    ],
  },
  "performance.third_party": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Use the surfaced src values and domains as an inventory and record a production trace; the tag count is a heuristic and does not measure transfer, execution, privacy, or user value. For each vendor, document purpose, consent/legal basis where applicable, and measured cost, remove stale or duplicate integrations, and use `async`, `defer`, delayed loading, or an interaction boundary only when the script's dependency order and required timing allow it.",
    ],
  },
  "performance.preconnect": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Use a production waterfall to find cross-origin connections needed for an early critical resource but discovered late; a missing preconnect is only an opportunity. Add a hint for the smallest proven set, with `crossorigin` when the eventual fetch uses CORS credentials mode, and remove hints that are unused or do not improve the critical path; there is no universal maximum that fits every page and browser.",
    ],
  },
  "performance.unminified": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Inspect each inline block listed in the evidence; the whitespace heuristic identifies large formatted `<script>` and `<style>` blocks and does not prove a production build is misconfigured. Trace the block to its source or generator, remove unused content first, and minify only when the measured savings justify the build and debugging tradeoffs, verifying the installed build tool's production settings rather than copying a framework-specific switch blindly.",
    ],
  },
  "performance.redirect_chain": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Reproduce the exact chain in the evidence with a browser Network trace or a GET-following client such as `curl -sS -L -D - -o /dev/null https://yourdomain.com`. Break a loop at the first rule pointing back to an earlier URL; otherwise collapse only unnecessary hops, update controlled internal links and canonicals to the intended final URL, and preserve deliberate authentication, consent, or method-sensitive transitions.",
    ],
  },
  "performance.inline_css": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Treat the evidence as a document-level size signal: the check totals `<style>` content in the fetched HTML and does not identify unused selectors or measure render delay. Run CSS coverage to separate critical, route-specific, shared, and dead rules; remove unused and duplicated output first, keep a measured amount of critical CSS inline when it improves initial rendering, and move reusable rules to cacheable stylesheets only when loading order and CSP remain correct.",
    ],
  },
  "performance.http_requests": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "This is retained for findings created by older SiteCMD versions; request count alone is not a defect under HTTP/2/3. Re-measure the deployed page, rank resources by critical-path latency, transfer, and main-thread work, remove duplicate or unused resources and repeated third-party tags first, and bundle only when it shortens the critical dependency chain without hurting route-level caching or parallel loading.",
    ],
  },
  "performance.ttfb": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "Confirm the single SiteCMD sample with several requests under comparable conditions, for example `curl -o /dev/null -s -w 'TTFB: %{time_starttransfer}s\\n' https://yourdomain.com`. Trace the measured request through CDN/cache status, origin processing, database queries, and dependency calls and fix the segment that actually consumes time; cache only public or safely keyed responses and never share personalized HTML across users.",
    ],
  },
  "performance.tbt": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "This finding is PageSpeed Insights' Lighthouse Total Blocking Time for the named lab run: a lab diagnostic, not INP and not field data. Repeat the run, record a local Performance trace under comparable throttling, attribute the largest tasks between First Contentful Paint and Time to Interactive to application code, hydration, or third-party scripts, then remove, split, defer, or move that work to a worker and confirm the freeze does not move to the first click.",
    ],
  },
};

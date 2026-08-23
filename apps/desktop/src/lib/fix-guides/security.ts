import type { FixGuideEntry } from "./types";

export const SECURITY_FIX_GUIDES: Record<string, FixGuideEntry> = {
  "security.csp": {
    effort: "involved",
    effortMinutes: 30,
    lead: "Nothing tells the browser which scripts belong to your site, so an injected script would run as if it were yours.",
    default: [
      "Inventory the scripts, styles, frames, fonts, images, workers, forms, and API origins the deployed product intentionally uses, then build a least-privilege policy from that list, using per-response nonces or stable hashes for required inline scripts instead of `unsafe-inline` or wildcards.",
      "Deploy the candidate as `Content-Security-Policy-Report-Only` with a monitored reporting endpoint, exercise representative routes and authenticated states while separating application traffic from extension noise, and enforce the reviewed policy only after coverage is representative and required resources work.",
    ],
  },
  "security.hsts": {
    effort: "quick",
    effortMinutes: 5,
    lead: "Browsers are not told to always reach your site over https, leaving an opening for the connection to be downgraded.",
    default: [
      "Confirm every host, route, and certificate works over HTTPS, then enable `Strict-Transport-Security` at one authoritative layer with a short `max-age` (for example 300) and raise it gradually as monitoring stays clean. Treat `includeSubDomains` and preload as separate, hard-to-reverse decisions that need a full subdomain inventory first.",
    ],
  },
  "security.x_frame_options": {
    effort: "quick",
    effortMinutes: 3,
    lead: "Another website could load your page inside a hidden frame and trick visitors into clicking things unintentionally.",
    default: [
      "Decide which origins, if any, may frame each sensitive page, then prefer CSP `frame-ancestors 'none'` or an exact origin list, adding `X-Frame-Options: DENY` or `SAMEORIGIN` as legacy defense when compatible. Keep account and admin flows unframeable, avoid `SAMEORIGIN` for widgets partners must embed cross-origin, and test a hostile iframe plus every intended embedding origin.",
    ],
  },
  "security.x_content_type_options": {
    effort: "quick",
    effortMinutes: 2,
    lead: "Browsers are left free to guess a file's type, which lets a malicious upload be reinterpreted and run as a script.",
    default: [
      "Add `X-Content-Type-Options: nosniff` to responses. This is a low-risk header for correctly served assets; it tells browsers not to guess file types, preventing MIME-sniffing attacks.",
    ],
  },
  "security.referrer_policy": {
    effort: "quick",
    effortMinutes: 2,
    lead: "Visiting links from your site can leak the full address of the page a visitor was just looking at.",
    default: [
      "Add `Referrer-Policy: strict-origin-when-cross-origin`. Modern browsers commonly default to this value, but an explicit policy makes the intended behavior auditable and protects legacy or embedded contexts with different defaults; test any product flow that intentionally relies on a full cross-origin Referer.",
    ],
  },
  "security.permissions_policy": {
    effort: "quick",
    effortMinutes: 3,
    lead: "The page never tells the browser to block camera, microphone, and location access, so embedded content could request them.",
    default: [
      "Add `Permissions-Policy: camera=(), microphone=(), geolocation=()`; the empty allowlist disables each feature for the document and its child frames. If a feature is required, grant it only to the top-level origin or an exact iframe origin with a matching `allow` attribute, and remember this header constrains document and frame delegation rather than isolating a third-party script running in your page.",
    ],
  },
  "security.https": {
    effort: "moderate",
    effortMinutes: 10,
    lead: "Your site still answers over plain http://, so visitors can land on an unencrypted page.",
    default: [
      "Terminate TLS with a valid certificate for every public hostname, then redirect the HTTP listener to the canonical HTTPS origin with a 301 or 308 and no chain through alternate hosts. Build the redirect from configured canonical hosts, never from an untrusted Host header, and verify alternate hostnames, IPv6, query and encoded paths, and any allowed non-GET requests before adding HSTS.",
    ],
  },
  "security.mixed_content": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "A secure page is still pulling in some images, scripts, or styles over plain http, weakening the encryption visitors expect.",
    default: [
      "Reproduce the affected route with browser Network and Console tools to confirm each observed HTTP resource is actually requested. For each one, verify the same asset works over HTTPS before updating the authoritative template, stylesheet, CMS field, or configuration; if the origin is HTTP-only, self-host or replace it, and swap localhost or loopback references for the intended deployed endpoint.",
    ],
  },
  "security.ssl": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "The certificate serving this site has a problem, such as expiring soon or not matching the domain, which can block visitors.",
    default: [
      "Inspect the certificate actually served for every affected public hostname and edge, and distinguish an expired or name-mismatched certificate from an incomplete chain, intentional private PKI, stale edge, or split DNS; correct only the condition the evidence supports. Renew or replace through the deployment's supported workflow, confirm the new certificate is the one publicly served, and monitor the deployed certificate before expiry.",
    ],
  },
  "security.exposed_files": {
    effort: "moderate",
    effortMinutes: 10,
    lead: "A sensitive file such as an environment or backup file may be reachable at a public URL, which can expose real credentials.",
    default: [
      "Inspect the exact URL, final status, Content-Type, and response body from the finding; a soft-404 SPA page differs from served `.env`, repository, credential, or backup contents. Remove sensitive files from the publish artifact, add origin/CDN rules denying dotfiles, VCS directories, and backups while preserving `/.well-known/`, and if real credentials were served, rotate them and verify with direct GET requests.",
    ],
  },
  "security.cookies": {
    effort: "quick",
    effortMinutes: 5,
    lead: "A cookie on your site is missing protections like Secure or HttpOnly, making it easier to steal or intercept.",
    default: [
      "Classify the surfaced cookie first: authentication and session cookies should normally be Secure and HttpOnly with the narrowest viable Domain/Path, a bounded lifetime, and a `__Host-`/`__Secure-` prefix where compatible, while a cookie intentionally read by JavaScript cannot be HttpOnly and should not carry a reusable session secret. Choose SameSite from real same-site, OAuth, and embedded flows, then inspect production `Set-Cookie` headers and test login, logout, and cross-site behavior.",
    ],
  },
  "security.cors": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "Your site's cross-origin sharing settings may let any website read responses meant only for your own pages.",
    default: [
      "Decide whether the resource is intentionally readable by any website: `Access-Control-Allow-Origin: *` is valid for public non-credentialed content, while sensitive or credentialed APIs need an exact origin allowlist plus normal authentication and authorization. When selecting an origin dynamically, compare the full serialized origin (scheme, host, port), emit it only on an exact match, add `Vary: Origin`, and never use substring, regex, or blind reflection checks.",
    ],
  },
  "security.cors_reflection": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "Your server copies whatever website asks into its access-control header, so it ends up trusting every site that asks.",
    default: [
      "Find the code that copies the request `Origin` header into `Access-Control-Allow-Origin` and replace the reflection with an exact allowlist match, echoing an origin only when it is one of your own trusted origins. Never reflect an unvalidated origin alongside `Access-Control-Allow-Credentials: true`; if the content is genuinely public and non-credentialed, prefer a static `Access-Control-Allow-Origin: *` instead.",
    ],
  },
  "security.open_redirect": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "A link on your site can be tricked into sending visitors to an outside destination while it still looks like your own domain.",
    default: [
      "Prefer a server-owned destination key or relative application path over a caller-supplied URL. Otherwise parse the target once against a configured canonical base, require the normalized origin (scheme, hostname, and port) to match an exact allowlist, reject scheme-relative, backslash, userinfo, control-character, and non-http(s) input, and fall back to a fixed safe page when validation fails.",
    ],
  },
  "security.directory_listing": {
    effort: "quick",
    effortMinutes: 5,
    lead: "Visiting certain folders on your site shows a raw file listing instead of a page, revealing files never meant to be public.",
    default: [
      "Disable automatic directory indexes at the web server: `autoindex off;` for Nginx or `Options -Indexes` in an Apache `.htaccess`. Verify by visiting a directory URL without an index file, such as `yourdomain.com/images/`; you should get a 403 or 404, not a file listing.",
    ],
  },
  "security.sri": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "Scripts loaded from another site have no integrity check, so a compromised host there could silently change what runs on your page.",
    default: [
      "Add SRI only where the URL identifies stable, versioned bytes and, for a cross-origin resource, the server supports the CORS request SRI requires; a rolling vendor URL whose bytes change in place cannot be pinned safely with one hash. For a stable resource, generate a SHA-384 digest of the exact production bytes, add `integrity` with the appropriate cross-origin mode, and test the real page, because a mismatched hash blocks the resource rather than falling back to unverified bytes.",
    ],
  },
  "security.server_info": {
    effort: "quick",
    effortMinutes: 5,
    lead: "Your server's responses reveal the exact software and version it runs, giving attackers a head start on known weaknesses.",
    default: [
      "Remove unnecessary product and version detail at the component that adds it, for example Express `app.disable('x-powered-by')`, PHP `expose_php=Off`, or reduced server tokens. Treat this as low-impact hardening, not a substitute for patching, also review error pages and debug endpoints where precise versions are more actionable, and verify representative success and error responses afterward.",
    ],
  },
  "security.env_leak": {
    effort: "quick",
    effortMinutes: 5,
    lead: "A path that would normally serve your environment configuration file responds publicly, which can hand out real secrets.",
    default: [
      "Fetch the exact path with GET and inspect the final body; a 200 catch-all HTML page is not an environment-file disclosure, while assignment-style configuration from the real file confirms exposure. Remove environment files from the publish artifact, deny dotfiles at the origin/CDN while preserving `/.well-known/`, and if actual credentials were served, revoke or rotate each value and verify the URL now returns a real 404 or 403.",
    ],
  },
  "security.source_maps": {
    effort: "quick",
    effortMinutes: 3,
    lead: "Your production build ships source maps that let anyone reconstruct your original code, internal paths, and any embedded secrets.",
    default: [
      "Decide whether public source maps match the product's transparency and threat model; a map is not automatically a vulnerability, but it can reveal original source, internal paths, embedded endpoints, and accidentally bundled secrets. If browser access is unnecessary, generate hidden maps uploaded to an access-controlled monitoring service and exclude `.map` files plus public `sourceMappingURL` comments; search mapped sources for credentials and rotate anything truly exposed.",
    ],
  },
  "security.insecure_form": {
    effort: "quick",
    effortMinutes: 5,
    lead: "A form on your site submits its data over plain http, so anything a visitor types can be read by anyone on the network.",
    default: [
      "Find every `<form>` whose `action` uses `http://` and change it to `https://` or a relative path that inherits the page protocol. If a form submits to a third-party service, verify that service supports HTTPS, then confirm in the DevTools Network tab that submissions go over HTTPS.",
    ],
  },
  "security.form_action_hijack": {
    effort: "moderate",
    effortMinutes: 10,
    lead: "A form on your site sends what people type to another website, so make sure that destination is one you chose.",
    default: [
      "Audit each surfaced form action and its purpose; cross-origin submission can be intentional for a payment, identity, support, or email provider, and the risk is an unexpected, lookalike, or user-controlled destination handling sensitive fields. Verify legitimate third-party destinations match the official service URL exactly, trace any JavaScript-set action to confirm it cannot be manipulated by user input, and restrict submissions with a CSP `form-action` directive.",
    ],
  },
  "security.vibe.client_auth": {
    effort: "involved",
    effortMinutes: 45,
    lead: "A page checks who is logged in only in the browser, so anyone can skip that check and call the real data directly.",
    default: [
      "Identify authentication checks that run only in client-side JavaScript and move verification to the server: every protected API endpoint must validate the session or token server-side before returning data, because an attacker can bypass any JavaScript check. Keep client-side checks for UX only, and test by calling protected endpoints directly without credentials to confirm they return 401, not data.",
    ],
  },
  "security.vibe.csrf": {
    effort: "moderate",
    effortMinutes: 20,
    lead: "A request that changes data on your site has no protection against being triggered from another website a visitor has open.",
    default: [
      "Confirm the route uses ambient cookie or session authentication and changes state; bearer-token-only APIs have a different CSRF boundary, and GET must stay side-effect free. Use the framework's maintained synchronizer-token or signed double-submit mechanism and validate it before the side effect, strictly compare the Origin header with the configured public origin, and treat SameSite as defense in depth rather than complete CSRF protection.",
    ],
  },
  "security.vibe.env_exposure": {
    effort: "quick",
    effortMinutes: 5,
    lead: "Code that reads a server-only environment value shows up in what ships to the browser, which can leak it to every visitor.",
    default: [
      "Locate the exact match in the fetched page and local build before acting; a literal `process.env.X` reference is not a leaked value, and a secret-named match can be documentation or a fixture. Move confirmed privileged credentials to an authorized server boundary, apply the provider's supported restrictions to intentionally public browser values, and start rotation only after confirming a genuine secret reached a public artifact.",
    ],
  },
  "security.vibe.exposed_keys": {
    effort: "quick",
    effortMinutes: 10,
    lead: "A string that looks like a privileged credential turned up in public code, and if it is real, anyone could use it as you.",
    default: [
      "Identify the value through the provider's own console or trusted local tooling without copying the complete value into logs, tickets, or a public token decoder; a format match does not prove ownership, activity, or privilege. Keep privileged credentials such as Stripe `sk_`/`rk_` and service-role tokens behind a trusted server boundary, and for a genuine exposed secret, revoke or rotate it first, remove it from current and historical artifacts, and confirm the old value no longer works.",
    ],
  },
  "security.vibe.exposed_keys.public": {
    effort: "quick",
    effortMinutes: 10,
    lead: "A key that looks meant for public browser use appeared in your code, but its scope and restrictions still need to be checked.",
    default: [
      "Classify the value before acting: Stripe `pk_` publishable keys are designed for browser delivery, Google/Firebase `AIza` keys are commonly client-visible but need the provider's supported API and application restrictions, and a generic JWT may be an intended public-role token or a privileged credential. Rotate only what classification confirms was unintentionally public; do not rotate an intentionally public identifier merely to clear the finding.",
    ],
  },
  "security.vibe.hardcoded_secrets": {
    effort: "moderate",
    effortMinutes: 15,
    lead: "A value that looks like a real credential is written directly into the source code instead of coming from a secret store.",
    default: [
      "Review the exact matched value and context; variable names, public identifiers, fixtures, and placeholders can resemble secrets, so validate a real credential with the issuing service before incident actions. For a real credential, revoke or rotate it first, replace the literal with a server-side secret-store or environment reference, and search repository history, artifacts, and logs for the old value to verify it no longer authenticates.",
    ],
  },
  "security.security_txt": {
    effort: "quick",
    effortMinutes: 10,
    lead: "There is no security.txt file, so a researcher who finds a vulnerability has no defined way to report it to you.",
    default: [
      "Serve a UTF-8 plain-text file at `/.well-known/security.txt` over HTTPS with `Content-Type: text/plain; charset=utf-8`, containing at least one syntactically valid `Contact:` URI and exactly one RFC 3339 `Expires:` timestamp less than a year ahead. Verify every advertised contact is still controlled and monitored, add a renewal reminder, and fetch the deployed URL to confirm it is not a catch-all HTML page.",
    ],
  },
  "security.cross_origin": {
    effort: "quick",
    effortMinutes: 10,
    lead: "Your site's browsing window can still be reached and observed by other tabs and popups it did not intentionally open.",
    default: [
      "Inventory cross-origin opener relationships, OAuth and payment popups, and postMessage flows first, because COOP changes browsing-context groups and there is no universal header value for every site. Use `same-origin` for strong isolation where compatible, or `same-origin-allow-popups` when the page must retain certain popups it opens, then test every popup, close/return flow, and authentication path in both directions on success and error pages.",
    ],
  },
  "security.email_exposure": {
    effort: "quick",
    effortMinutes: 15,
    lead: "Email addresses are written plainly on your pages, making them easy targets for spam bots that harvest visible addresses.",
    default: [
      "Decide which exposed addresses actually need to be public and route the rest through a contact form protected with abuse controls. For an address that must stay visible, keep it accessible with a normal mail link; JavaScript obfuscation is not a dependable anti-harvesting control and can harm no-script users and assistive technology. Prefer a monitored role address that can be filtered or rotated over a personal inbox.",
    ],
  },
  "security.dns.spf": {
    effort: "quick",
    effortMinutes: 10,
    lead: "Your domain has no record listing which mail servers may send email as you, making it easier for attackers to spoof your address.",
    default: [
      "Publish exactly one TXT record at the apex starting with `v=spf1`, naming every service that sends mail for the domain (for example `include:_spf.google.com`), and end with `-all` or `~all`, never `+all`. Keep the record under 10 DNS-querying terms, never add a second `v=spf1` record, and if the domain never sends mail, publish `v=spf1 -all`.",
    ],
  },
  "security.dns.dmarc": {
    effort: "moderate",
    effortMinutes: 25,
    lead: "Nothing tells mail providers what to do with email that fails to prove it came from you, so spoofed mail can reach inboxes.",
    default: [
      "Publish a TXT record at `_dmarc.yourdomain.com` starting in monitoring mode, for example `v=DMARC1; p=none; rua=mailto:dmarc-reports@yourdomain.com`, then review the aggregate reports for a couple of weeks to find legitimate senders that fail SPF or DKIM alignment.",
      "Fix any legitimate sender that fails alignment, then graduate to `p=quarantine` (optionally ramping with `pct`) and finally `p=reject` once reports stay clean. Rejection reduces exact-domain spoofing; it does not stop display-name impersonation or lookalike domains.",
    ],
  },
  "security.dns.dkim": {
    effort: "quick",
    effortMinutes: 15,
    lead: "Your outgoing mail is not being signed, so recipients have no way to confirm it truly came from your domain unaltered.",
    default: [
      "Enable DKIM signing in each provider that actually sends your mail (it is a provider setting, not something you invent yourself), then publish the CNAME or TXT record it gives you at `<selector>._domainkey.yourdomain.com`. Validate a real delivered message afterward; DNS key publication alone does not prove outbound mail is signed or that its signing domain aligns for DMARC.",
    ],
  },
  "security.dns.mx": {
    effort: "quick",
    effortMinutes: 10,
    lead: "Your domain's mail-receiving setup is not explicit, which can leave senders retrying for days instead of getting a clear answer.",
    default: [
      "This check passes either way; it exists to make the domain's mail posture explicit. If the domain should receive mail but has no MX records, add your provider's records; if it should not, publish a null MX (`yourdomain.com. MX 0 .`) so senders get an immediate bounce instead of retrying for days.",
    ],
  },
  "security.dns.dnssec": {
    effort: "moderate",
    effortMinutes: 20,
    lead: "Your domain's DNS responses are not cryptographically signed, so they could in theory be forged in transit.",
    default: [
      "DNSSEC is optional hardening most sites skip; treat it as a nice-to-have, not an emergency. Enable signing at your DNS host first, add the resulting DS record at your registrar to link the zone into the chain of trust, then verify with `dig +dnssec` or an online DNSSEC analyzer. Plan any later DNS-provider migration as a documented key/DS rollover so the zone never fails validation.",
    ],
  },
  "security.dns.caa": {
    effort: "quick",
    effortMinutes: 10,
    lead: "Nothing on your domain restricts which certificate authorities may issue certificates for it, widening who could impersonate you.",
    default: [
      "Identify every certificate authority that actually issues for the domain, including host- or CDN-managed certificates, then publish one `CAA 0 issue` record per allowed CA at the apex, with `issuewild` handling if you use wildcards. Include every CA your certificate automation may use before rollout; CAA is an issuance-time request to compliant public CAs, and omitting a required issuer can make a later renewal fail.",
    ],
  },
  "security.dns.dangling_cname": {
    effort: "quick",
    effortMinutes: 15,
    lead: "A subdomain points to a third-party service that is no longer set up there, and someone else could claim it and take over.",
    default: [
      "Confirm the chain with `dig` first; an empty address result confirms the availability problem but not that another account can claim the target. If www is no longer used, delete the CNAME record; if it should work, re-point it at the current live hostname, and for shared-platform targets use the provider's own domain-claim documentation to determine whether the identifier is claimable before labeling it a takeover.",
    ],
  },
  "security.domain_expiry": {
    effort: "quick",
    effortMinutes: 10,
    lead: "Your domain's registration is approaching or past its expiration date, which risks losing the domain and everything on it.",
    default: [
      "Read the reported RDAP expiration date and urgency tier first; a date within 90 days is a planning warning, not proof that auto-renew failed, and even a past date needs confirmation because renewal publication can lag. Confirm status and renewal intent in the authoritative registrar account, renew immediately if needed, verify auto-renew and the payment path, and keep registrar access recoverable independently of the domain being protected.",
    ],
  },
  "security.vulnerable_libraries": {
    effort: "moderate",
    effortMinutes: 25,
    lead: "A library loaded on your site has a version with a publicly known vulnerability that attackers can look up and exploit.",
    default: [
      "Confirm the library and served version from the exact asset (signatures can be wrong after bundling) and read the primary advisory for affected ranges and whether the vulnerable code path is reachable in this site. Upgrade to a compatible fixed release or remove the library, then re-scan the deployed asset; if no fix exists, remove or replace the vulnerable feature and document a temporary compensating control with an owner and deadline.",
    ],
  },
};

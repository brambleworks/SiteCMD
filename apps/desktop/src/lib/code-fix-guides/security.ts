import type { CodeFixGuideEntry } from "./types";

export const SECURITY_FIX_GUIDES: Record<string, CodeFixGuideEntry> = {
  "client-auth-without-server-enforcement": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "Confirm whether the client auth check protects only navigation or corresponds to server data or mutations; a public-only backend may need documentation, not new middleware. Otherwise verify the session or token with a maintained server verifier at every protected boundary, authorize role, ownership, and tenant separately, and test protected operations directly without the UI.",
    ],
  },
  "cors-credentials-wildcard": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Choose one valid CORS mode: for credentialed requests, replace `origin: '*'` with an exact origin allowlist; for a public API that needs `*`, disable credentials. When selecting an origin dynamically, return it only on an exact allowlist match and emit `Vary: Origin`, then verify in a browser that an untrusted origin receives no CORS permission.",
    ],
  },
  "csrf-missing": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Protect cookie- or session-authenticated POST, PUT, PATCH, and DELETE requests with the framework's maintained synchronizer-token or signed double-submit protection, and keep GET free of side effects; SameSite cookies are defense in depth, not a substitute for validating the request. Verify cross-origin mutations with a missing or invalid token fail before any side effect.",
    ],
  },
  "jwt-decode-without-verify": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Verify the JWT signature with a maintained library and trusted key source before using any claim for identity or authorization; decoding alone only parses attacker-supplied bytes. Configure an explicit algorithm allowlist, expected issuer and audience, and expiry validation, and keep decode-only use limited to non-authoritative UI hints.",
    ],
  },
  "oauth-callback-pkce": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Generate a random 43-128 character `code_verifier` before the OAuth redirect and store it in the session, send its SHA-256 hash as `code_challenge` with `code_challenge_method=S256` in the authorization request, then include the stored verifier when exchanging the authorization code for tokens so the provider can match it to the original challenge.",
    ],
  },
  "oauth-callback-state": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Generate a cryptographically random, single-use state value with at least 128 bits of entropy per authorization attempt, bind it to the initiating browser session, and require an exact match with the unexpired stored value before exchanging the code, consuming it atomically. Test missing, altered, replayed, and cross-session callbacks; PKCE protects the authorization code and does not by itself replace this binding in every flow.",
    ],
  },
  "one-time-token-no-expiry": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Add an `expires_at` column set to a short window when creating the token: 15 minutes for password resets, 24-72 hours for invites. Reject tokens whose expiry has passed during validation, for example `WHERE token_hash = ? AND expires_at > NOW()`, and add a periodic cleanup job that deletes expired rows.",
    ],
  },
  "one-time-token-no-single-use": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Add a `used_at` column and claim the token with one conditional mutation such as `UPDATE tokens SET used_at = NOW() WHERE token_hash = ? AND used_at IS NULL AND expires_at > NOW() RETURNING id`, requiring exactly one returned row; a separate SELECT then UPDATE can let concurrent requests redeem the same token. Keep the claim and the protected action in the same transaction and test two simultaneous redemptions.",
    ],
  },
  "one-time-token-raw-lookup": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Store only `SHA-256(rawToken)` (or an HMAC with a server-side secret) and send the raw token once to the user; fast hashing is appropriate for an unguessable token, while passwords require a slow password hash. Hash the presented token before a parameterized lookup and keep expiry plus single-use redemption atomic; this limits token theft after a database disclosure but does not remove tokens already leaked through URLs or logs.",
    ],
  },
  "open-redirect": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Prefer a server-side destination key or a relative application path. Otherwise parse the input once against a trusted base URL, reject scheme-relative input, credentials, and any scheme other than http/https, enforce the exact origin after normalization, and test encoded and parser-confusion cases such as `//evil.example` and `javascript:` before redirecting.",
    ],
  },
  "raw-sql-unsafe": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Trace the matched value into the executed query and confirm whether a driver binding or trusted wrapper already separates data from SQL structure. Where values are interpolated, replace them with the driver's bound-parameter API, and map identifier choices such as table, column, or sort direction through a strict server-owned allowlist because they usually cannot be parameterized.",
    ],
  },
  "sensitive-auth": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Trace the effective route through the proxy, middleware, route groups, and imported wrappers to document the exact control that authenticates this handler; if none exists, verify a server-side session or credential before any sensitive read or side effect, then enforce the action's authorization policy as a separate default-deny decision. Test the route as anonymous, expired, wrong-tenant, and low-privilege users.",
    ],
  },
  "sensitive-authz": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "Trace middleware, called services, and database row policies to determine whether authorization already occurs outside this file; authentication alone is not authorization. If a gap remains, enforce a default-deny policy using identity and tenant context from a verified server session, keep ownership checks close to the data access, and test wrong-owner, wrong-tenant, and privileged identities across list, detail, mutation, and export paths.",
    ],
  },
  "session-cookie-flags": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Set `Secure` and `HttpOnly` on the session cookie, plus `SameSite=Lax` or `Strict` when the product's cross-site flows allow it; SameSite reduces cross-site request exposure but is defense in depth, not complete CSRF protection. Verify the attributes on the production `Set-Cookie` header and confirm the chosen SameSite value still supports required OAuth or embedded flows.",
    ],
  },
  "unsafe-html": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Remove raw HTML rendering when structured components or text are sufficient. When the feature genuinely accepts HTML, including rendered Markdown, sanitize at the final rendering boundary with a maintained allowlist sanitizer such as DOMPurify, `sanitize-html`, or `rehype-sanitize`, and test event handlers, `javascript:` URLs, and malformed markup.",
    ],
  },
  "upload-key-scope": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Generate an opaque, cryptographically random object key server-side instead of using the untrusted filename, store the authenticated owner or tenant as authoritative metadata, and authorize every read, write, overwrite, list, and delete through the bucket policy or download endpoint; a user-prefixed path does not enforce access by itself. Test direct and guessed object URLs as two different users or tenants.",
    ],
  },
  "upload-validation": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Enforce the exact formats and size limits the feature needs server-side while streaming, treating the declared Content-Type and extension only as hints; inspect magic bytes and fully parse the file with a maintained library where feasible. Store uploads outside executable and static roots under opaque server-generated keys, serve them with an explicit safe Content-Type, and authorize every object operation.",
    ],
  },
  "env-drift": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Review the exact key-set differences between environment-parallel files and classify each key as shared and required, environment-specific, platform-injected, optional, or obsolete; different key sets are not automatically wrong. Align genuinely shared required keys, remove obsolete entries, and document intentional per-environment differences without copying secret values into source control.",
    ],
  },
  "env-example-incomplete": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "For each source-referenced key missing from the example, determine whether it is developer-supplied, platform-injected, optional, or obsolete before changing the template. Add developer-supplied keys with clearly fake placeholders and a short required/optional note, never live credentials or production values, then confirm a clean checkout can configure the documented workflow from the example.",
    ],
  },
  "env-example-missing": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Create a `.env.example` (or `.env.sample`/`.env.template`) listing developer-supplied configuration with clearly fake placeholders, marking each entry required, optional, or defaulted; platform-injected variables do not need to be copied in. Never include live credentials or production values, and keep real local secret files such as `.env.local` out of version control.",
    ],
  },
  "suspicious-manifest-package": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Do not assume the package is malicious from spelling alone; compare the exact package.json name with the intended library and verify the registry page, publisher, repository link, and release history, reviewing lifecycle scripts in an isolated environment. If it is a typo, install the intended official package and regenerate the lockfile; if the near-match is legitimate, document the provenance decision.",
    ],
  },
  "suspicious-package": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Check the exact import spelling against the library the code intends to use; this finding is based on string similarity and does not prove the near-match package exists or is malicious. Because the import is undeclared, run a clean type-check or build first to rule out a local alias or framework-provided module, and if a registry dependency is genuinely needed, verify its publisher and provenance before installing the intended exact package.",
    ],
  },
  "public-endpoint-rate-limit": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Identify each anonymous or abuse-sensitive operation and choose controls from its cost and abuse mode rather than copying one requests-per-window number across login, reset, search, and uploads. Use layered keys such as account or identifier plus network signals, an atomic shared store across instances, and a 429 response with `Retry-After`; test bursts, distributed IPs, and multi-instance deployments.",
    ],
  },
  "request-validation": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Validate input at the start of every route handler with a schema library such as Zod before any business logic runs, returning 400 with specific field errors for invalid input. Cover string length limits, email format, numeric ranges, required fields, and enum values, and validate params and query as well as the body; never trust client input.",
    ],
  },
  "user-controlled-fetch": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Prefer accepting a destination ID and mapping it to a server-owned base URL. If arbitrary URLs are required, allow only the necessary scheme, compare the normalized hostname with an exact allowlist, resolve DNS and reject loopback, private, link-local, and cloud-metadata destinations, and disable redirects or re-run the full policy on every hop. Test rebinding, IPv6 forms, and redirects to internal addresses.",
    ],
  },
  "webhook-idempotency": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Use the provider plus its stable event ID as an idempotency key backed by a database unique constraint, claiming each event with one atomic insert or upsert; a SELECT-then-INSERT check races under concurrent deliveries. Return the provider's expected success response for an already completed event, give external side effects their own idempotency key, and test two simultaneous deliveries.",
    ],
  },
  "webhook-signature": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Verify the provider's signature against the exact raw request bytes before parsing, using the provider SDK or constant-time comparison, with the secret in server-side secret storage. Validate the signed timestamp with a narrow replay tolerance where the scheme provides one, and reject missing, stale, wrong-key, and wrong-body signatures before any side effect; a valid signature does not stop replay by itself.",
    ],
  },
  "stripe-checkout-idempotency": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Create a server-owned order or checkout-attempt record and use its immutable ID as the Stripe idempotency key for one logical session creation; do not derive the key from mutable cart contents or let the client choose it. Persist and reuse the returned session ID while the attempt is valid, create a new attempt when price or cart state intentionally changes, and treat a disabled loading button as UX, not the correctness boundary.",
    ],
  },
  "stripe-user-controlled-price": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Accept only a narrow product or plan choice from the client and map it to a server-owned catalog entry; do not forward a client-sent Price ID, amount, or currency merely because it is schema-valid. Validate the mapped product, currency, and eligibility before creating Checkout, and derive fulfillment from trusted server catalog data plus the signature-verified Stripe event, not client metadata alone.",
    ],
  },
  "stripe-user-controlled-redirect": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Build `success_url` and `cancel_url` from a server-owned canonical origin and an allowlisted route key or relative path rather than forwarding an arbitrary client URL. Parse and normalize any permitted dynamic value once, requiring the intended scheme and exact host and rejecting credentials, protocol-relative forms, and non-web schemes; verify in Stripe test mode that accepted return URLs resolve to the canonical origin.",
    ],
  },
  "tenant-scope-missing": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "Derive the active tenant from a verified server-side membership or session, never from a request body or unverified token claim, and scope reads, writes, joins, caches, background jobs, and exports to that tenant. Enforce the boundary in a shared repository or query layer with database RLS as defense in depth where supported, and test Tenant A versus Tenant B across list, detail, update, delete, and guessed IDs.",
    ],
  },
  "tls-verification-disabled": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Confirm the matched setting is active in a production-capable path rather than a comment, fixture, or isolated test configuration, then remove the verification override. If the target uses a self-signed or internal CA certificate, trust that CA instead, for example `NODE_EXTRA_CA_CERTS` in Node or `REQUESTS_CA_BUNDLE` in Python, and re-run the call to confirm the valid endpoint succeeds while untrusted certificates fail.",
    ],
  },
  "cors-origin-reflection": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Check the effective middleware scope and authentication first; reflected credentialed CORS is exploitable when an untrusted origin can make the browser attach credentials and read a sensitive response, so the static match alone does not prove every condition. For private APIs, compare the parsed Origin with an exact allowlist, return it only on a match, and add `Vary: Origin`; for an intentionally public API, disable credentialed CORS and use `*`.",
    ],
  },
  "framework-debug-enabled": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Confirm which configuration the deployment actually selects; a production-looking file in source is evidence to review, not proof it is active. Turn verbose debug off in production, parse and validate environment booleans instead of relying on truthy strings, add a production startup assertion that fails closed, and verify in a staging deployment that an unhandled error returns a generic public response.",
    ],
  },
  "hardcoded-secret": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "First verify the matched value is a live or plausibly usable credential rather than a public identifier, test fixture, or placeholder. If it is real, revoke or rotate it at the issuing service before treating the code change as remediation, then load the replacement from a server-side secret store or injected environment and search repository history, logs, and build artifacts for the old value.",
    ],
  },
  "hardcoded-localhost-url": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Confirm the intended deployment topology first; loopback can be correct for a co-located sidecar, emulator, or explicitly local-only path, and an intentional design should be documented and marked reviewed. If the destination varies by environment, read it from validated server-only configuration with a localhost default only behind an explicit development-mode branch, and fail startup when required configuration is missing.",
    ],
  },
  "client-env-secret": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "First check whether the referencing module reaches a production client bundle; a secret-shaped name can hold a public identifier or placeholder, and intentional browser exposure can be marked not applicable. If a live privileged value reached a client asset, revoke and rotate it first, move the replacement to a server-only secret store, and put the privileged operation behind an authenticated server boundary; renaming alone is insufficient.",
    ],
  },
  "gitignore-missing": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Create a `.gitignore` in the project root covering at minimum `.env`, `.env.local`, `.env.*.local`, `node_modules/`, `dist/`, `build/`, `.DS_Store`, and `*.log`, plus your framework's output directories. Run `git status` to verify sensitive files no longer appear as trackable, and remove any already-committed ones with `git rm --cached`.",
    ],
  },
  "gitignore-missing-env": {
    effort: "quick",
    effortMinutes: 2,
    default: [
      "Add `.env`, `.env.local`, and `.env.*.local` to `.gitignore`. If .env files were already committed, remove them from tracking with `git rm --cached .env`, rotate any credentials exposed in the git history, and verify with `git status` that they no longer appear as tracked.",
    ],
  },
  "eval-exec-injection": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Remove runtime code evaluation from the request path: use `JSON.parse` for JSON, a fixed dispatch table for named operations, or a deliberately small parser for the needed expression grammar. Do not treat an in-process evaluator or a 'sandbox' package as a security boundary; if arbitrary customer code is a real requirement, run it in a separately isolated, unprivileged runtime with no ambient secrets and strict resource limits.",
    ],
  },
  "shell-injection": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Replace string-based shell execution with a fixed server-owned executable and an argument array with shell mode disabled, using `--` when the command supports it; avoiding the shell does not prevent option injection, so allowlist command-specific values. If shell syntax is genuinely required, keep the entire script server-owned and pass untrusted data as positional arguments or environment variables, never by concatenating it into shell source.",
    ],
  },
  "php-file-inclusion": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Trace whether a raw request value can select the `include` or `require` target; if so, replace it with a server-owned map such as `$pages = ['home' => __DIR__.'/home.php']` and reject unknown keys rather than deriving executable paths from input. Keep `allow_url_include=0` as defense in depth where the deployment supports it, and verify with inert canary files inside and outside an isolated fixture root instead of probing real system files.",
    ],
  },
  "php-object-injection": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "For untrusted structured input, migrate from `unserialize` to schema-validated JSON, which does not instantiate PHP classes. If a legacy payload must remain, authenticate it before deserialization and pass `['allowed_classes' => false]`; a class allowlist is not proof the permitted magic methods are safe, and a signature checked after `unserialize` is too late.",
    ],
  },
  "php-dynamic-command": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Trace the matched call first; a request value found only inside `escapeshellarg` or `escapeshellcmd` is a lower-severity review because escaping does not constrain leading options, paths, or URLs. Prefer a native PHP library; otherwise run a fixed server-owned executable with an argument-array invocation that bypasses shell parsing where the runtime API permits, plus allowlists, timeouts, output limits, and least privilege.",
    ],
  },
  "python-command-injection": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Trace the process call first; a request value found only inside `shlex.quote` is a lower-severity review because POSIX quoting preserves one argument boundary but does not constrain leading options, paths, or URLs. Use `subprocess.run` or `Popen` with a fixed server-owned executable, an argument list, and `shell=False`, allowlist command-specific values, and add timeouts, output limits, and least-privilege execution.",
    ],
  },
  "php-code-execution": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "If request-derived text reaches an active evaluator, replace it with schema parsing or a server-owned callable map; replace `create_function` with a normal closure and do not treat a character blacklist as a PHP sandbox. For a `preg_replace` pattern with the legacy `/e` modifier, migrate to a fixed `preg_replace_callback`, and verify with inert markers and an evaluator spy rather than real side effects.",
    ],
  },
  "php-path-traversal": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Prefer mapping an opaque object id to a server-owned path or a small allowlist; a basename wrapper only removes directory components and does not authorize which file inside the directory may be accessed. Otherwise canonicalize the fixed base and target and compare path components rather than a bare string prefix, handle symlinks and write targets explicitly, and test with canary files inside and outside an isolated temporary root.",
    ],
  },
  "python-open-redirect": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Prefer named routes or a fixed allowlist of application paths instead of echoing a raw return URL; if an imported helper already constrains the route, document and test that boundary. In Django, use `url_has_allowed_host_and_scheme` with the intended hosts and HTTPS requirement; a `netloc` check alone can miss scheme-relative, backslash, user-info, and encoding edge cases, so test absolute, scheme-relative, encoded, and backslash variants.",
    ],
  },
  "python-path-traversal": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Prefer an opaque server-side object id or a maintained bounded-serving helper with a fixed trusted root; basename or `secure_filename` removes path syntax but does not authorize which in-root file a caller may access. For direct access, resolve base and target with `pathlib`, require component-aware containment such as `is_relative_to` rather than a string-prefix check, and test with canaries in an isolated temporary root.",
    ],
  },
  "js-command-injection": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "If the request value can select command text or the executable, replace `exec`/`execSync` with `execFile` or `spawn` using a fixed server-owned executable, an argument array, and `shell: false`. If shell features are required, keep the shell source fixed and pass untrusted values as arguments or environment data; allowlist values, add timeouts and output caps, and test metacharacters and leading options against a mocked invocation.",
    ],
  },
  "python-unsafe-deserialization": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "For untrusted structured input, migrate to schema-validated JSON; do not use pickle or marshal as an untrusted interchange format. For YAML, use `yaml.safe_load` or an explicitly safe loader with size and depth limits, and if a trusted legacy pickle must remain, authenticate and bind it to context and expiry before deserialization; a signature checked after loading is too late.",
    ],
  },
  "python-code-execution": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Confirm the matched value reaches the built-in `eval` or `exec` on a reachable path, then replace dynamic evaluation with schema parsing: `ast.literal_eval` for literal-only Python syntax, `json.loads` for JSON, or a lookup dict keyed by an allowlist for dynamic dispatch. Removing `__builtins__` or an in-process 'sandbox' is not a security boundary; arbitrary customer code requires separate OS or container isolation with strict resource limits.",
    ],
  },
  "python-sql-injection": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Confirm no wrapper already parameterizes the matched call, then keep query structure constant with driver placeholders, for example `cursor.execute('SELECT * FROM users WHERE id = %s', [request.args['id']])`. Map dynamic table or column choices to server-owned identifiers or use the driver's identifier-composition API such as psycopg2 `sql.Identifier`; do not interpolate identifier strings.",
    ],
  },
  "python-template-injection": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Keep template source server-owned and pass request data only as context variables, for example `render_template_string(TEMPLATE, name=request.args['name'])`; if users choose among templates, map a small key to fixed template names. For customer-authored templates, Jinja's SandboxedEnvironment is not a complete boundary by itself; render in a separate least-privilege process with no secrets or network and strict resource limits.",
    ],
  },
  "localstorage-auth-token": {
    effort: "involved",
    effortMinutes: 30,
    default: [
      "Pick the architecture first: for a same-site web app, prefer a server-managed opaque session in a Secure, HttpOnly cookie, and because a cookie is sent automatically add CSRF protection, rotation, and revocation rather than copying a JWT into a cookie. Stop persisting long-lived bearer tokens in localStorage, clear and rotate previously exposed tokens, and keep any browser access token short-lived and in memory only when the architecture requires it.",
    ],
  },
  "plaintext-password": {
    effort: "moderate",
    effortMinutes: 15,
    default: [
      "Hash passwords with the framework's maintained password API or a reviewed library: prefer Argon2id where supported; bcrypt, scrypt, or PBKDF2 can be appropriate with parameters benchmarked on production-class hardware, not one universal work factor. Use the library's salt and verification APIs, rate-limit attempts, and plan migration of existing plaintext credentials, forcing resets when safe rehash-on-login is not possible.",
    ],
  },
  "weak-default-credential": {
    effort: "quick",
    effortMinutes: 5,
    default: [
      "Delete the placeholder fallback (for example `process.env.SECRET || 'changeme'`) so the value can only come from the environment, and fail fast at startup when the variable is missing instead of running on a guessable default. Generate a strong random value per environment, for example `openssl rand -base64 32`, store it in your secret manager, and confirm the app errors clearly when the variable is unset.",
    ],
  },
};

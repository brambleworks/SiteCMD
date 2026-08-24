const SECURITY_FOCUS_META = {
  "sec.https": {
    label: "HTTPS and mixed content",
    patterns: ["mixed_content", "https_enforcement", "https"],
  },
  "sec.ssl_expiry": {
    label: "SSL certificate validity",
    patterns: ["ssl.validity", "ssl"],
  },
  "sec.headers": {
    label: "Content Security Policy (CSP) header",
    patterns: ["headers.csp", "csp", "content security policy"],
    watchImpact: "This could affect security headers, hardening, or exposed infrastructure.",
  },
  "sec.hsts": {
    label: "HTTPS-only (HSTS) header",
    patterns: ["headers.hsts", "hsts"],
  },
  "sec.cors": {
    label: "CORS and API boundary hardening",
    patterns: [
      "security.cors",
      "cors",
      "access-control-allow-origin",
      "cross-origin",
      "cross origin",
    ],
    watchImpact: "This could affect cross-origin access, API credentials, or proxy configuration.",
  },
  "sec.auth": {
    label: "Auth enforcement",
    patterns: [
      "security.vibe.client_auth",
      "client_auth",
      "authorization",
      "access control",
      "server-side auth",
      "client-side auth",
    ],
    watchImpact:
      "This could affect route protection, authorization, or server-side auth enforcement.",
  },
  "sec.cookies": {
    label: "Cookie and session security",
    patterns: [
      "security.cookies",
      "cookies_",
      "set-cookie",
      "samesite",
      "httponly",
      "csrf",
      "session",
    ],
    watchImpact: "This could affect cookie security, CSRF protection, or session handling.",
  },
  "sec.exposed_files": {
    label: "exposed files",
    patterns: ["exposed", "dotfile", ".env", ".git"],
    watchImpact: "This could affect exposed files, secrets, or infrastructure metadata.",
  },
} as const;

type SecurityFocus = keyof typeof SECURITY_FOCUS_META;

export function getSecurityFocusLabel(focus: string | null | undefined): string | null {
  if (!focus) return null;
  return SECURITY_FOCUS_META[focus as SecurityFocus]?.label ?? null;
}

function getSecurityFocusPatterns(focus: string | null | undefined): readonly string[] | null {
  if (!focus) return null;
  return SECURITY_FOCUS_META[focus as SecurityFocus]?.patterns ?? null;
}

export function matchesSecurityFocusText(
  haystack: string,
  focus: string | null | undefined,
): boolean {
  if (!focus) return false;
  const normalizedHaystack = haystack.toLowerCase();
  const patterns = getSecurityFocusPatterns(focus);
  if (!patterns) return normalizedHaystack.includes(focus.toLowerCase());
  return patterns.some((pattern) => normalizedHaystack.includes(pattern.toLowerCase()));
}

export function inferSecurityFocusFromText(haystack: string): string | null {
  const focus = Object.keys(SECURITY_FOCUS_META).find((candidate) =>
    matchesSecurityFocusText(haystack, candidate),
  );
  return focus ?? null;
}

export function getSecurityWatchImpactSentence(focus: string | null | undefined): string {
  const meta = SECURITY_FOCUS_META[focus as SecurityFocus];
  if (meta && "watchImpact" in meta) {
    return meta.watchImpact;
  }
  return "This could affect security headers, hardening, or exposed infrastructure.";
}

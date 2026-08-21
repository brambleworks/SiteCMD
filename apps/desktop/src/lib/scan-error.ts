// Maps backend scan errors to stable UI recovery codes.
import { SUPPORT_EMAIL } from "@/lib/support";

type ScanErrorCode =
  | "invalid_url"
  | "dns"
  | "tls_cert"
  | "connection_refused"
  | "timeout"
  | "cancelled"
  | "network"
  | "scan_failed"
  | "unknown";

export interface ScanErrorPayload {
  code: ScanErrorCode;
  message: string;
}

export function parseScanError(raw: unknown): ScanErrorPayload {
  const message = raw instanceof Error ? raw.message : typeof raw === "string" ? raw : "";
  const lowered = message.toLowerCase();
  if (!message) return { code: "unknown", message: "" };

  if (lowered.startsWith("invalid url") || lowered.includes("invalid url:")) {
    return { code: "invalid_url", message };
  }
  if (lowered.includes("cancelled") || lowered.includes("canceled")) {
    return { code: "cancelled", message };
  }
  // Match timeouts before their overlapping network-error signals.
  if (lowered.includes("timed out") || lowered.includes("timeout")) {
    return { code: "timeout", message };
  }
  if (
    lowered.includes("dns") ||
    lowered.includes("could not resolve") ||
    lowered.includes("name resolution") ||
    lowered.includes("name or service not known") ||
    lowered.includes("no such host")
  ) {
    return { code: "dns", message };
  }
  if (
    lowered.includes("certificate") ||
    lowered.includes("tls") ||
    lowered.includes("ssl") ||
    lowered.includes("self-signed")
  ) {
    return { code: "tls_cert", message };
  }
  if (
    lowered.includes("connection refused") ||
    lowered.includes("connection reset") ||
    lowered.includes("connection closed")
  ) {
    return { code: "connection_refused", message };
  }
  if (lowered.includes("network error") || lowered.includes("failed to fetch")) {
    return { code: "network", message };
  }
  if (lowered.startsWith("scan error") || lowered.startsWith("scan failed")) {
    return { code: "scan_failed", message };
  }
  return { code: "unknown", message };
}

export interface FormattedScanError {
  title: string;
  body: string;
}

export function formatScanError(payload: ScanErrorPayload): FormattedScanError {
  switch (payload.code) {
    case "invalid_url":
      return {
        title: "URL isn't valid",
        body: "Double-check the site URL in your project settings. It needs the full https://… form.",
      };
    case "dns":
      return {
        title: "Couldn't resolve the site's address",
        body: "DNS lookup failed. Check that the domain is spelled correctly and currently registered, then try again.",
      };
    case "tls_cert":
      return {
        title: "The site's TLS certificate is invalid",
        body: "The HTTPS certificate failed verification (expired, self-signed, or hostname mismatch). Fix the cert on the site, or scan with a different URL.",
      };
    case "connection_refused":
      return {
        title: "The site refused the connection",
        body: "The server is reachable on DNS but isn't accepting connections on HTTPS. Check whether the site is running and the firewall allows traffic from your machine.",
      };
    case "timeout":
      return {
        title: "The scan timed out",
        body: "The site didn't respond in time. Bump the scan timeout in Settings if the site is normally slow to first byte, or try again in a minute.",
      };
    case "cancelled":
      return {
        title: "Scan cancelled",
        body: "You stopped the scan before it finished. Re-run it when you're ready.",
      };
    case "network":
      return {
        title: "Network error during scan",
        body: "Couldn't reach the site. Check your internet connection and the site's availability, then try again.",
      };
    case "scan_failed":
      return {
        title: "Scan failed",
        body:
          payload.message ||
          `Something went wrong running the scan. Try again, or contact ${SUPPORT_EMAIL} if it keeps failing.`,
      };
    case "unknown":
    default:
      return {
        title: "Scan failed",
        body: payload.message || "Something went wrong running the scan. Try again.",
      };
  }
}

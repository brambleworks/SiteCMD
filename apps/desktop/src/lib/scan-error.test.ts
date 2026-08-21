import { describe, expect, it } from "vitest";
import { formatScanError, parseScanError } from "./scan-error";

describe("parseScanError", () => {
  it("classifies an InvalidUrl error", () => {
    expect(parseScanError("Invalid URL: not-a-url").code).toBe("invalid_url");
  });

  it("classifies DNS failures", () => {
    expect(parseScanError("Failed to fetch https://nope.example: dns error").code).toBe("dns");
    expect(parseScanError("Could not resolve host nope.example").code).toBe("dns");
    expect(parseScanError("name or service not known").code).toBe("dns");
  });

  it("classifies TLS certificate errors", () => {
    expect(parseScanError("Failed to fetch: self-signed certificate in chain").code).toBe(
      "tls_cert",
    );
    expect(parseScanError("Network error: tls handshake failed").code).toBe("tls_cert");
  });

  it("classifies connection refused / reset", () => {
    expect(parseScanError("Network error: connection refused").code).toBe("connection_refused");
    expect(parseScanError("Network error: connection reset by peer").code).toBe(
      "connection_refused",
    );
  });

  it("classifies timeouts (and prefers timeout over generic network)", () => {
    expect(parseScanError("Timed out reading response body from https://x.example").code).toBe(
      "timeout",
    );
    expect(parseScanError("Network error: timeout after 30s").code).toBe("timeout");
  });

  it("classifies cancelled", () => {
    expect(parseScanError("Scan cancelled").code).toBe("cancelled");
  });

  it("classifies a legacy daily-limit string as a generic network error", () => {
    const raw = "Network error: Daily scan limit reached (3/3).";
    expect(parseScanError(raw).code).toBe("network");
    expect(formatScanError(parseScanError(raw)).title).toBe("Network error during scan");
  });

  it("falls back to 'network' for generic network errors", () => {
    expect(parseScanError("Network error: failed to fetch https://x.example: misc").code).toBe(
      "network",
    );
  });

  it("returns 'unknown' with empty message for empty input", () => {
    expect(parseScanError("")).toEqual({ code: "unknown", message: "" });
  });
});

describe("formatScanError", () => {
  it("renders distinct title/body per code", () => {
    const codes = [
      "invalid_url",
      "dns",
      "tls_cert",
      "connection_refused",
      "timeout",
      "cancelled",
      "network",
      "scan_failed",
      "unknown",
    ] as const;
    const titles = new Set<string>();
    for (const code of codes) {
      titles.add(formatScanError({ code, message: "" }).title);
    }
    expect(titles.size).toBeGreaterThanOrEqual(7);
  });

  it("DNS guidance mentions resolving the address", () => {
    const out = formatScanError(parseScanError("Could not resolve host"));
    expect(out.title).toMatch(/resolve|address/i);
    expect(out.body).toMatch(/DNS|domain|registered/i);
  });

  it("TLS guidance points at the certificate", () => {
    const out = formatScanError(parseScanError("self-signed certificate"));
    expect(out.body).toMatch(/certificate|HTTPS/i);
  });

  it("timeout guidance suggests bumping the scan timeout", () => {
    const out = formatScanError(parseScanError("Timed out"));
    expect(out.body).toMatch(/timeout|try again/i);
  });
});

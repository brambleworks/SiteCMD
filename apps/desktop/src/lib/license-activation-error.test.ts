import { describe, expect, it } from "vitest";
import {
  activationFailureAction,
  formatLicenseActivationError,
  parseLicenseActivationError,
} from "./license-activation-error";

describe("activationFailureAction", () => {
  it("treats the backend's own 'unknown' as no verdict, not as a failure", () => {
    expect(activationFailureAction('{"code":"unknown"}')).toEqual({ announce: "pending" });
    expect(activationFailureAction(new Error('{"code":"unknown"}'))).toEqual({
      announce: "pending",
    });
  });

  it("still reports a conclusive refusal as a failure", () => {
    expect(activationFailureAction('{"code":"not_found"}')).toEqual({
      announce: "failure",
      payload: { code: "not_found" },
    });
    expect(activationFailureAction('{"code":"provider_refused","message":"refunded"}')).toEqual({
      announce: "failure",
      payload: { code: "provider_refused", message: "refunded" },
    });
  });

  it("reports a code this build does not recognize as a failure, not as pending", () => {
    expect(activationFailureAction('{"code":"minted_on_a_tuesday"}')).toEqual({
      announce: "failure",
      payload: { code: "unknown" },
    });
  });

  it("says nothing when the user declined", () => {
    expect(activationFailureAction('{"code":"cancelled"}')).toEqual({ announce: "nothing" });
  });

  it("treats an unstructured rejection as no verdict", () => {
    // Not the backend's answer at all: a bridge window that never became
    // ready, a token that could not be issued. The command did not run.
    expect(activationFailureAction(new Error("bridge window is not available"))).toEqual({
      announce: "pending",
    });
    expect(activationFailureAction("plain sentence")).toEqual({ announce: "pending" });
    expect(activationFailureAction(undefined)).toEqual({ announce: "pending" });
  });

  it("treats our own client deadline as no verdict", () => {
    const timeout = Object.assign(new Error("That action took too long to finish."), {
      command: "activate_license",
      scope: "data-admin",
      timeoutMs: 180_000,
    });
    expect(activationFailureAction(timeout)).toEqual({ announce: "pending" });
  });
});

describe("parseLicenseActivationError", () => {
  it("parses a typed payload from the Rust JSON error string", () => {
    const result = parseLicenseActivationError('{"code":"not_found"}');
    expect(result).toEqual({ code: "not_found" });
  });

  it("includes the payload message when present", () => {
    const result = parseLicenseActivationError('{"code":"network","message":"timeout"}');
    expect(result).toEqual({ code: "network", message: "timeout" });
  });

  it("falls back to 'unknown' when the code is not in the known set", () => {
    expect(parseLicenseActivationError('{"code":"future_code"}')).toEqual({ code: "unknown" });
  });

  it("returns 'unknown' with the raw message when parsing fails entirely", () => {
    const result = parseLicenseActivationError("legacy plain-text error");
    expect(result.code).toBe("unknown");
    expect(result.message).toBe("legacy plain-text error");
  });

  it("accepts Error instances and reads .message", () => {
    const result = parseLicenseActivationError(new Error('{"code":"limit_reached"}'));
    expect(result).toEqual({ code: "limit_reached" });
  });
});

describe("formatLicenseActivationError", () => {
  it("renders a distinct message for each known code", () => {
    const codes = [
      "key_required",
      "invalid_key",
      "not_found",
      "store_mismatch",
      "limit_reached",
      "expired",
      "variant_unknown",
      "server_error",
      "network",
      "missing_instance_id",
      "unknown",
    ] as const;
    const messages = new Set<string>();
    for (const code of codes) messages.add(formatLicenseActivationError({ code }));
    expect(messages.size).toBe(codes.length);
  });

  it("never echoes a raw upstream message back to the user", () => {
    const rendered = formatLicenseActivationError({
      code: "not_found",
      message: "raw LemonSqueezy body: license_key xyz not in store 99",
    });
    expect(rendered).not.toContain("xyz");
    expect(rendered).not.toContain("store 99");
    expect(rendered).not.toContain("raw");
  });

  it("limit_reached message offers a recovery path the user can take", () => {
    const rendered = formatLicenseActivationError({ code: "limit_reached" });
    expect(rendered).toMatch(/deactivate|email hello@sitecmd\.com/i);
  });

  it("expired message names the subscription and the renewal path, not a retry", () => {
    const rendered = formatLicenseActivationError({ code: "expired" });
    expect(rendered).toMatch(/expired/i);
    expect(rendered).toMatch(/renew|billing|subscription/i);
    expect(rendered).not.toMatch(/try again/i);
  });

  it("network message points at the user's connection, not a server bug", () => {
    const rendered = formatLicenseActivationError({ code: "network" });
    expect(rendered).toMatch(/connection|reach the license server/i);
  });
});

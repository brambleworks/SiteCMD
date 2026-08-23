import { describe, expect, it } from "vitest";
import { userFacingError } from "./user-facing-error";

const FALLBACK = "Try again in a moment.";

describe("userFacingError", () => {
  it("uses the rejection text as one capitalized sentence", () => {
    expect(userFacingError("could not read ~/.cursor/mcp.json", FALLBACK)).toBe(
      "Could not read ~/.cursor/mcp.json.",
    );
    expect(userFacingError(new Error("Timed out after 30s."), FALLBACK)).toBe(
      "Timed out after 30s.",
    );
  });

  it("drops transport prefixes that mean nothing to a person", () => {
    expect(userFacingError("Error: connection refused", FALLBACK)).toBe("Connection refused.");
    expect(userFacingError("invoke error: no such project", FALLBACK)).toBe("No such project.");
    expect(userFacingError("Error: invoke error: no such project", FALLBACK)).toBe(
      "No such project.",
    );
    expect(
      userFacingError(new Error("error: tauri error: command error: disk is full"), FALLBACK),
    ).toBe("Disk is full.");
  });

  it("falls back when the rejection carries no words", () => {
    expect(userFacingError(null, FALLBACK)).toBe(FALLBACK);
    expect(userFacingError(undefined, FALLBACK)).toBe(FALLBACK);
    expect(userFacingError("", FALLBACK)).toBe(FALLBACK);
    expect(userFacingError({}, FALLBACK)).toBe(FALLBACK);
  });

  it("caps runaway backend text at 240 characters", () => {
    const long = "x".repeat(400);
    const result = userFacingError(long, FALLBACK);
    expect(result.length).toBeLessThanOrEqual(240);
    expect(result.endsWith("...")).toBe(true);
  });
});

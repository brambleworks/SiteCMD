import { describe, expect, it } from "vitest";
import { errorMessage } from "./error-message";

describe("errorMessage", () => {
  it("returns an Error's message", () => {
    expect(errorMessage(new Error("boom"))).toBe("boom");
  });

  it("passes strings through unchanged", () => {
    expect(errorMessage("plain failure")).toBe("plain failure");
  });

  it("collapses null and undefined to an empty string so callers can `|| fallback`", () => {
    expect(errorMessage(null)).toBe("");
    expect(errorMessage(undefined)).toBe("");
    expect(errorMessage(null) || "Something went wrong").toBe("Something went wrong");
  });

  it("stringifies other primitive values", () => {
    expect(errorMessage(42)).toBe("42");
    expect(errorMessage(false)).toBe("false");
  });
});

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import {
  clearFirstScanCompletedForTests,
  markFirstScanCompleted,
  readHasCompletedFirstScan,
} from "./onboarding-flags";

describe("onboarding flags: first scan completed", () => {
  beforeEach(() => {
    clearFirstScanCompletedForTests();
  });

  afterEach(() => {
    clearFirstScanCompletedForTests();
  });

  it("defaults to false on a fresh install", () => {
    expect(readHasCompletedFirstScan()).toBe(false);
  });

  it("flips to true after the first completion is marked", () => {
    markFirstScanCompleted();
    expect(readHasCompletedFirstScan()).toBe(true);
  });

  it("is idempotent: marking twice keeps it true and does not throw", () => {
    markFirstScanCompleted();
    markFirstScanCompleted();
    expect(readHasCompletedFirstScan()).toBe(true);
  });

  it("dispatches a change event so subscribers can re-read", () => {
    let observed = 0;
    const listener = () => {
      observed += 1;
    };
    window.addEventListener("sitecmd:onboarding-flags-changed", listener);
    try {
      markFirstScanCompleted();
      expect(observed).toBe(1);
    } finally {
      window.removeEventListener("sitecmd:onboarding-flags-changed", listener);
    }
  });
});

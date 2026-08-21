import { beforeEach, describe, expect, it } from "vitest";

import {
  persistProjectSelection,
  readStoredProjectSelection,
  normalizeStoredProjectSelectionUrl,
} from "./project-selection-state";

describe("project-selection-state", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("stores and restores a normalized project selection", () => {
    persistProjectSelection(7, "https://example.com/");

    expect(readStoredProjectSelection()).toEqual({
      projectId: 7,
      envUrl: "https://example.com",
    });
  });

  it("ignores corrupted or non-object stored selection state", () => {
    for (const storedValue of ["not json", "null", JSON.stringify(["https://example.com"])]) {
      window.localStorage.setItem("sitecmd_project_selection_v1", storedValue);

      expect(readStoredProjectSelection()).toBeNull();
    }
  });

  it("ignores records without a numeric project id", () => {
    window.localStorage.setItem(
      "sitecmd_project_selection_v1",
      JSON.stringify({ projectId: "7", envUrl: "https://example.com" }),
    );

    expect(readStoredProjectSelection()).toBeNull();
  });

  it("ignores records without a positive integer project id", () => {
    for (const projectId of [0, -1, 1.5]) {
      window.localStorage.setItem(
        "sitecmd_project_selection_v1",
        JSON.stringify({ projectId, envUrl: "https://example.com" }),
      );

      expect(readStoredProjectSelection()).toBeNull();
    }
  });

  it("drops unsafe stored environment URLs but preserves the valid project id", () => {
    window.localStorage.setItem(
      "sitecmd_project_selection_v1",
      JSON.stringify({ projectId: 7, envUrl: "https://user:token@example.com" }),
    );

    expect(readStoredProjectSelection()).toEqual({
      projectId: 7,
      envUrl: null,
    });
  });

  it("normalizes stored project URLs", () => {
    expect(normalizeStoredProjectSelectionUrl("https://example.com/")).toBe("https://example.com");
    expect(normalizeStoredProjectSelectionUrl("http://localhost:4321/")).toBe(
      "http://localhost:4321",
    );
    expect(normalizeStoredProjectSelectionUrl("javascript:alert(1)")).toBeNull();
    expect(normalizeStoredProjectSelectionUrl("https://user:token@example.com")).toBeNull();
    expect(normalizeStoredProjectSelectionUrl(null)).toBeNull();
  });

  it("clears stored selection when asked to persist an invalid project id", () => {
    persistProjectSelection(7, "https://example.com");
    persistProjectSelection(0, "https://example.com");

    expect(window.localStorage.getItem("sitecmd_project_selection_v1")).toBeNull();
    expect(readStoredProjectSelection()).toBeNull();
  });
});

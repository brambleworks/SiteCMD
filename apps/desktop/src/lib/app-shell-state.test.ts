import { beforeEach, describe, expect, it } from "vitest";

import {
  clearPersistedShellPage,
  readPersistedShellPage,
  writePersistedShellPage,
} from "./app-shell-state";

describe("app-shell-state", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("stores and restores a known page", () => {
    writePersistedShellPage("updates");
    expect(readPersistedShellPage()).toBe("updates");
  });

  it("ignores unknown stored pages", () => {
    window.localStorage.setItem("sitecmd_shell_state_v1", JSON.stringify({ page: "nope" }));

    expect(readPersistedShellPage()).toBeNull();
  });

  it("ignores corrupted or non-object stored page state", () => {
    for (const storedValue of ["not json", "null", JSON.stringify(["dashboard"])]) {
      window.localStorage.setItem("sitecmd_shell_state_v1", storedValue);

      expect(readPersistedShellPage()).toBeNull();
    }
  });

  it("clears persisted state", () => {
    writePersistedShellPage("sites");
    clearPersistedShellPage();

    expect(readPersistedShellPage()).toBeNull();
  });

  it("restores Overview launches back into the project dashboard", () => {
    window.localStorage.setItem("sitecmd_shell_state_v1", JSON.stringify({ page: "sites" }));

    expect(readPersistedShellPage()).toBe("dashboard");
  });

  it("migrates the retired Today page to the project dashboard", () => {
    window.localStorage.setItem("sitecmd_shell_state_v1", JSON.stringify({ page: "today" }));

    expect(readPersistedShellPage()).toBe("dashboard");
  });

  it("stores Overview as dashboard so launches open on the active project", () => {
    writePersistedShellPage("sites");

    expect(window.localStorage.getItem("sitecmd_shell_state_v1")).toBe(
      JSON.stringify({ page: "dashboard" }),
    );
  });
});

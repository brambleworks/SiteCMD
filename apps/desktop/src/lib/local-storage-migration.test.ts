import { describe, expect, it } from "vitest";

import {
  migrateLegacyValue,
  readCurrentOrLegacyValue,
  writeCurrentValue,
} from "./local-storage-migration";

describe("local-storage migration helpers", () => {
  it("prefers the current key over the legacy key", () => {
    const storage = new Map<string, string>([
      ["sitecmd-theme", "dark"],
      ["sitehealthkit-theme", "light"],
    ]);

    const value = readCurrentOrLegacyValue(
      {
        getItem: (key) => storage.get(key) ?? null,
        setItem: (key, value) => void storage.set(key, value),
        removeItem: (key) => void storage.delete(key),
      },
      "sitecmd-theme",
      "sitehealthkit-theme",
    );

    expect(value).toBe("dark");
  });

  it("migrates the legacy value into the current key", () => {
    const storage = new Map<string, string>([["sitehealthkit-theme", "system"]]);

    const value = migrateLegacyValue(
      {
        getItem: (key) => storage.get(key) ?? null,
        setItem: (key, newValue) => void storage.set(key, newValue),
        removeItem: (key) => void storage.delete(key),
      },
      "sitecmd-theme",
      "sitehealthkit-theme",
    );

    expect(value).toBe("system");
    expect(storage.get("sitecmd-theme")).toBe("system");
    expect(storage.has("sitehealthkit-theme")).toBe(false);
  });

  it("writes the current key and clears the legacy key", () => {
    const storage = new Map<string, string>([["sitehealthkit-scan-prefs", '{"timeout":30}']]);

    writeCurrentValue(
      {
        getItem: (key) => storage.get(key) ?? null,
        setItem: (key, value) => void storage.set(key, value),
        removeItem: (key) => void storage.delete(key),
      },
      "sitecmd-scan-prefs",
      "sitehealthkit-scan-prefs",
      '{"timeout":45}',
    );

    expect(storage.get("sitecmd-scan-prefs")).toBe('{"timeout":45}');
    expect(storage.has("sitehealthkit-scan-prefs")).toBe(false);
  });
});

import { describe, expect, it } from "vitest";
import { isTestSourceFile } from "./test-file-classification.mjs";

describe("isTestSourceFile", () => {
  it.each([
    "apps/desktop/src/components/Panel.test.tsx",
    "apps/desktop/src/components/Panel.behavior.test.tsx",
    "apps/desktop/src/components/Panel.spec.ts",
    "apps/desktop/src-tauri/src/db/tests.rs",
    "apps/desktop/src-tauri/src/db/connected_bootstrap_tests.rs",
    "apps/desktop/src-tauri/src/db/migration_test.rs",
    "apps/desktop/src-tauri/src/db/test_helpers.rs",
    "apps/desktop/src-tauri/src/core/tests/parser.rs",
    "apps\\desktop\\src-tauri\\src\\db\\connected_bootstrap_tests.rs",
  ])("recognizes %s", (path) => {
    expect(isTestSourceFile(path)).toBe(true);
  });

  it.each([
    "apps/desktop/src/components/Panel.tsx",
    "apps/desktop/src-tauri/src/db/connected_bootstrap.rs",
    "apps/desktop/src-tauri/src/core/testing.rs",
  ])("keeps %s in production metrics", (path) => {
    expect(isTestSourceFile(path)).toBe(false);
  });
});

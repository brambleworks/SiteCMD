import { defineConfig, devices } from "@playwright/test";

const e2eHost = "127.0.0.1";
// verify-push provides a dedicated port; standalone runs default to Vite's port.
const e2ePort = Number(process.env.SITECMD_E2E_PORT ?? 5173);
const e2eBaseUrl = `http://${e2eHost}:${e2ePort}`;
// Reuse the push gate's build; CI builds its own preview and local runs use Vite.
const isVerifyPush = Boolean(process.env.SITECMD_VERIFY_PUSH);
const webServerCommand = isVerifyPush
  ? `pnpm exec vite preview --host ${e2eHost} --port ${e2ePort}`
  : process.env.CI
    ? `pnpm build && pnpm exec vite preview --host ${e2eHost} --port ${e2ePort}`
    : `pnpm exec vite --host ${e2eHost} --port ${e2ePort}`;

// Browser smoke tests run against Vite with Tauri IPC installed by the fixture stub.
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  // Serial execution avoids cold-transform contention in local Vite runs.
  workers: 1,
  reporter: process.env.CI ? [["github"], ["list"]] : "list",
  timeout: 30_000,
  expect: {
    // Allow cold local Vite transforms the same budget as navigation waits.
    timeout: 15_000,
  },
  use: {
    baseURL: e2eBaseUrl,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  webServer: {
    command: webServerCommand,
    url: e2eBaseUrl,
    reuseExistingServer: false,
    timeout: 180_000,
    stdout: "pipe",
    stderr: "pipe",
  },
});

import { describe, expect, it } from "vitest";

import {
  attachRiskAcknowledged,
  buildAttachPreflightFailureMessage,
  isHealthyDevServerResponse,
  waitForHealthyDevServer,
} from "./tauri-attach-lib.mjs";

describe("tauri attach preflight", () => {
  it("requires an explicit acknowledgement before attaching a privileged renderer", () => {
    expect(attachRiskAcknowledged({})).toBe(false);
    expect(attachRiskAcknowledged({ SITECMD_ALLOW_PRIVILEGED_ATTACH: "0" })).toBe(false);
    expect(attachRiskAcknowledged({ SITECMD_ALLOW_PRIVILEGED_ATTACH: "1" })).toBe(true);
  });

  it("recognizes a healthy Vite dev response", () => {
    expect(
      isHealthyDevServerResponse({
        ok: true,
        contentType: "text/html; charset=utf-8",
        body: `
        <!doctype html>
        <html>
          <body>
            <div id="root"></div>
            <script type="module" src="/@vite/client"></script>
            <script type="module" src="/src/main.tsx"></script>
          </body>
        </html>
      `,
      }),
    ).toBe(true);
  });

  it("accepts equivalent shell markers when the root id uses single quotes", () => {
    expect(
      isHealthyDevServerResponse({
        ok: true,
        contentType: "text/html; charset=utf-8",
        body: `
        <!doctype html>
        <html>
          <body>
            <div id='root'></div>
            <script type="module" src="/@vite/client"></script>
            <script type="module" src="/src/main.ts"></script>
          </body>
        </html>
      `,
      }),
    ).toBe(true);
  });

  it("rejects HTML that is missing the real app shell markers", () => {
    expect(
      isHealthyDevServerResponse({
        ok: true,
        contentType: "text/html",
        body: '<html><body><div id="root"></div><p>Still booting</p></body></html>',
      }),
    ).toBe(false);
  });

  it("waits for the server to become healthy", async () => {
    let requestCount = 0;
    const fetchResponse = async () => {
      requestCount += 1;
      if (requestCount < 2) {
        return {
          ok: true,
          contentType: "text/html; charset=utf-8",
          body: '<html><body><div id="root"></div><p>warming up</p></body></html>',
        };
      }
      return {
        ok: true,
        contentType: "text/html; charset=utf-8",
        body: '<html><body><div id="root"></div><script type="module" src="/@vite/client"></script><script type="module" src="/src/main.tsx"></script></body></html>',
      };
    };

    await expect(
      waitForHealthyDevServer("http://127.0.0.1:5173", {
        timeoutMs: 2000,
        pollMs: 25,
        fetchResponse,
      }),
    ).resolves.toBe(true);
    expect(requestCount).toBe(2);
  });

  it("times out cleanly when the dev server never becomes healthy", async () => {
    const fetchResponse = async () => ({
      ok: true,
      contentType: "text/html; charset=utf-8",
      body: '<html><body><div id="root"></div><p>no vite client yet</p></body></html>',
    });

    await expect(
      waitForHealthyDevServer("http://127.0.0.1:5173", {
        timeoutMs: 150,
        pollMs: 20,
        fetchResponse,
      }),
    ).resolves.toBe(false);
  });

  it("builds a useful failure message", () => {
    expect(buildAttachPreflightFailureMessage("http://127.0.0.1:5173")).toEqual([
      "SiteCMD attach preflight failed: http://127.0.0.1:5173 is not serving a healthy Vite app yet.",
      "Start the web app first with `pnpm dev` or `pnpm dev:tauri`, wait for the Vite URL to load, then run `pnpm tauri:dev:attach`.",
      "If you still get a white window, close any older SiteCMD instance before attaching again.",
    ]);
  });
});

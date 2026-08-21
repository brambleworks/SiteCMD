/** Collects page and console errors, including unstubbed Tauri IPC calls. */
import type { Page } from "@playwright/test";

// Ignore HMR websocket failures caused by Playwright's floating dev port.
const isViteWebsocketNoise = (text: string) =>
  text.includes("WebSocket closed without opened") ||
  (text.includes("WebSocket connection") && text.includes("Unexpected response code: 400")) ||
  text.includes("[vite] failed to connect to websocket");

export function collectConsoleErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on("pageerror", (error) => {
    if (isViteWebsocketNoise(error.message)) return;
    errors.push(`pageerror: ${error.message}`);
  });
  page.on("console", (msg) => {
    if (msg.type() !== "error") return;
    const text = msg.text();
    if (text.includes("Download the React DevTools")) return;
    if (isViteWebsocketNoise(text)) return;
    errors.push(`console.error: ${text}`);
  });
  return errors;
}

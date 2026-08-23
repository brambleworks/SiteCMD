import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { DESKTOP_HEARTBEAT_STALE_MS } from "../dist/heartbeat.js";

/** Reads `NAME: i64 = <expr>;` out of a Rust const declaration and evaluates a bare integer or `a * b` product. */
function evalRustIntConst(source, name) {
  const match = source.match(new RegExp(`\\b${name}\\s*:\\s*i64\\s*=\\s*([^;]+);`));
  assert.ok(match, `${name} not found in constants.rs`);
  const expr = match[1].trim();
  const literal = expr.match(/^\d+$/);
  if (literal) return Number(literal[0]);
  const product = expr.match(/^(\d+)\s*\*\s*(\d+)$/);
  assert.ok(product, `Cannot evaluate constants.rs expression for ${name}: ${expr}`);
  return Number(product[1]) * Number(product[2]);
}

test("the MCP heartbeat stale window is pinned to the desktop's DESKTOP_HEARTBEAT_STALE_MS", async () => {
  const constantsSource = await readFile(
    new URL("../../desktop/src-tauri/src/constants.rs", import.meta.url),
    "utf8",
  );
  const desktopValue = evalRustIntConst(constantsSource, "DESKTOP_HEARTBEAT_STALE_MS");
  assert.equal(
    DESKTOP_HEARTBEAT_STALE_MS,
    desktopValue,
    "any reader of the heartbeat file must apply the same staleness window as the desktop",
  );
});

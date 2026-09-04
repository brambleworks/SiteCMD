import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";
import { downloadVerified } from "./vm-runtime.mjs";

test("runtime downloads verify the pinned checksum before creating an installable artifact", async (t) => {
  const root = mkdtempSync(path.join(tmpdir(), "sitecmd-vm-download-test-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const body = "owned test artifact";
  const artifact = {
    url: "https://example.com/runtime",
    sha256: createHash("sha256").update(body).digest("hex"),
  };
  const output = path.join(root, "runtime.tgz");
  await assert.rejects(
    downloadVerified(artifact, output, async () => new Response("tampered")),
    /checksum/,
  );
  assert.equal(existsSync(output), false);
  await downloadVerified(artifact, output, async () => new Response(body));
  assert.equal(readFileSync(output, "utf8"), body);
  await assert.rejects(
    downloadVerified(artifact, output, async () => new Response(body)),
    /EEXIST/,
  );
});

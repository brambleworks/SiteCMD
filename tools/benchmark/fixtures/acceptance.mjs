import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";

const { allowOrigin } = await import(pathToFileURL(process.argv[2]).href);
assert.equal(allowOrigin("https://untrusted.example"), null);
assert.equal(allowOrigin("https://example.com.attacker.example"), null);
console.log("Acceptance: untrusted origins are rejected");

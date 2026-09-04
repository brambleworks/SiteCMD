import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";

const { allowOrigin } = await import(pathToFileURL(process.argv[2]).href);
assert.equal(allowOrigin("https://example.com"), "https://example.com");
console.log("Regression: the allowed origin still works");

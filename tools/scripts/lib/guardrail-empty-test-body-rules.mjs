const RUST_ROOTS = ["apps/desktop/src-tauri/src", "apps/desktop/src-tauri/crates"];

// #[test] or #[tokio::test] (tolerating trailing args like (flavor = "...")).
const TEST_ATTR_RE = /#\[\s*(?:tokio::)?test\b[^\]]*\]/g;
// A test fn signature through its opening brace: `fn name(...) [-> Ret] {`.
const FN_SIG_RE = /\bfn\s+(\w+)\s*\([^)]*\)\s*(?:->\s*[^{;]+?)?\{/;

function extractBraceBody(src, openIdx) {
  let depth = 0;
  for (let i = openIdx; i < src.length; i += 1) {
    const c = src[i];
    if (c === "{") depth += 1;
    else if (c === "}") {
      depth -= 1;
      if (depth === 0) return src.slice(openIdx + 1, i);
    }
  }
  return null;
}

function bodyIsEmpty(body) {
  // Remove comments; code before a string-contained `//` still keeps the body nonempty.
  const withoutBlock = body.replace(/\/\*[\s\S]*?\*\//g, "");
  const withoutLine = withoutBlock
    .split("\n")
    .map((line) => {
      const idx = line.indexOf("//");
      return idx === -1 ? line : line.slice(0, idx);
    })
    .join("");
  return withoutLine.trim() === "";
}

export function emptyTestBodyFailures(read, listFiles) {
  const failures = [];
  const rustFiles = RUST_ROOTS.flatMap((root) => listFiles(root, (file) => file.endsWith(".rs")));
  for (const file of rustFiles) {
    const src = read(file);
    TEST_ATTR_RE.lastIndex = 0;
    let attr;
    while ((attr = TEST_ATTR_RE.exec(src)) !== null) {
      const sig = FN_SIG_RE.exec(src.slice(attr.index));
      if (!sig) continue;
      const openIdx = attr.index + sig.index + sig[0].length - 1;
      const body = extractBraceBody(src, openIdx);
      if (body === null || !bodyIsEmpty(body)) continue;
      const lineNo = src.slice(0, attr.index).split("\n").length;
      failures.push(
        `${file}:${lineNo} test \`${sig[1]}\` has an empty body (only whitespace/comments). An assertionless test is false coverage - give it real assertions or delete it (audit F27).`,
      );
    }
  }
  return failures;
}

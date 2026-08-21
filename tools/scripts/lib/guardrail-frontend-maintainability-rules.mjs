import { stripComments } from "./guardrail-source-text.mjs";
import { lineNumberFor } from "./guardrail-text-utils.mjs";

export function frontendMaintainabilityFailures(read, listFiles, sourceFiles, lineBudgetOverrides) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const frontendRuntimeFiles = listFiles(
    "apps/desktop/src",
    (file) =>
      /\.(ts|tsx)$/.test(file) &&
      !/(\.test|\.spec|\.behavior|\.render)\.(ts|tsx)$/.test(file) &&
      file !== "apps/desktop/src/lib/tauri-invoke.ts",
  );

  const unsafeBlankOpens = [];
  for (const file of frontendRuntimeFiles) {
    const source = read(file);
    for (const match of source.matchAll(/window\.open\s*\(([\s\S]*?)\)/g)) {
      const args = match[1];
      if (
        /["']_blank["']/.test(args) &&
        (!/\bnoopener\b/.test(args) || !/\bnoreferrer\b/.test(args))
      ) {
        unsafeBlankOpens.push(`${file}:${lineNumberFor(source, match.index ?? 0)}`);
      }
    }
  }
  check(
    unsafeBlankOpens.length === 0,
    `window.open(..., "_blank") must include noopener,noreferrer: ${unsafeBlankOpens.join(", ")}`,
  );

  const settingsButtonOffenders = [];
  const settingsButtonFiles = listFiles(
    "apps/desktop/src/components/settings",
    (file) => /\.tsx$/.test(file) && !/(\.test|\.spec|\.behavior|\.render)\.tsx$/.test(file),
  );
  for (const file of settingsButtonFiles) {
    const source = read(file);
    for (const match of source.matchAll(/<button\b([\s\S]*?)>/g)) {
      const attributes = match[1];
      const classMatch = attributes.match(/className\s*=\s*(?:`([^`]*)`|"([^"]*)")/);
      if (!classMatch) continue;
      const className = classMatch[1] ?? classMatch[2] ?? "";
      if (
        /(bg-blue-500\/|bg-primary\/|hover:bg-(?:red|blue|primary|destructive)-?500?\/|bg-red-500\/)/.test(
          className,
        )
      ) {
        settingsButtonOffenders.push(`${file}:${lineNumberFor(source, match.index ?? 0)}`);
      }
    }
  }
  check(
    settingsButtonOffenders.length === 0,
    `Settings panels must use <Button> from @/components/ui/button instead of bare <button> with bespoke bg/hover utilities: ${settingsButtonOffenders.join(", ")}`,
  );

  const inlineStyleScopedFiles = listFiles(
    "apps/desktop/src/components/settings",
    (file) => /\.tsx$/.test(file) && !/(\.test|\.spec|\.behavior|\.render)\.tsx$/.test(file),
  );
  const forbiddenInlineStyleKeys =
    /\b(?:background(?:Color)?|border(?:Color|Top|Right|Bottom|Left)?|color|gridTemplateColumns|maxHeight)\s*:/;
  const inlineStyleOffenders = [];
  for (const file of inlineStyleScopedFiles) {
    const source = read(file);
    const lines = source.split(/\r\n|\r|\n/);
    let runningOffset = 0;
    for (let index = 0; index < lines.length; index += 1) {
      const line = lines[index];
      const styleIndex = line.indexOf("style={{");
      if (styleIndex !== -1 && !line.includes("// allow-inline-style")) {
        const slice = source.slice(runningOffset + styleIndex, runningOffset + styleIndex + 600);
        const close = slice.indexOf("}}");
        const block = close === -1 ? slice : slice.slice(0, close);
        if (forbiddenInlineStyleKeys.test(block)) {
          inlineStyleOffenders.push(`${file}:${index + 1}`);
        }
      }
      runningOffset += line.length + 1;
    }
  }
  check(
    inlineStyleOffenders.length === 0,
    `Inline style={{...}} with background/border/color/gridTemplateColumns/maxHeight is not allowed in gates/ or settings/ component trees. Use design tokens, the ProgressBar primitive, or a static class map. Add a same-line // allow-inline-style marker if absolutely necessary. Offenders: ${inlineStyleOffenders.join(", ")}`,
  );

  const issuesFlowSpec = read("apps/desktop/e2e/issues-flow.spec.ts");
  const issuesFlowCode = stripComments(issuesFlowSpec, "apps/desktop/e2e/issues-flow.spec.ts");
  check(
    /const\s+scoreTile\s*=\s*page\.getByRole\(\s*"button"\s*\)\.filter\(\{\s*hasText:\s*"SiteCMD Score"\s*\}\)\.first\(\s*\)/s.test(
      issuesFlowCode,
    ) &&
      /expect\(\s*scoreTile\s*\)\.toContainText\(\s*String\(\s*SEEDED_SCORE\s*\)\s*\)/s.test(
        issuesFlowCode,
      ),
    "apps/desktop/e2e/issues-flow.spec.ts must assert SEEDED_SCORE inside the SiteCMD Score tile so a blank-but-titled dashboard cannot pass.",
  );

  const markdownSource = read("apps/desktop/src/components/ui/markdown.tsx");
  check(
    /languages\s*:\s*LANGUAGES/.test(markdownSource) &&
      /highlight\.js\/lib\/languages\//.test(markdownSource),
    "apps/desktop/src/components/ui/markdown.tsx must register a curated highlight.js subset (`languages: LANGUAGES`) instead of falling back to lowlight's default `common` set.",
  );

  const oversizeFiles = [];
  for (const file of sourceFiles) {
    if (/(\.test|\.spec|\.behavior|\.render)\.(ts|tsx)$/.test(file)) continue;
    const lineCount = read(file).split("\n").length;
    const maxLines = lineBudgetOverrides.get(file) ?? (file.endsWith(".css") ? 1200 : 900);
    if (lineCount > maxLines) {
      oversizeFiles.push(`${file} has ${lineCount} lines (budget ${maxLines})`);
    }
  }
  check(
    oversizeFiles.length === 0,
    `Source files exceeded maintainability line budgets. Split large modules or add a temporary ratchet with a lower target: ${oversizeFiles.join(", ")}`,
  );

  return failures;
}

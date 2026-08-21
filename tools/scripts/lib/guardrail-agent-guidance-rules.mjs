const AGENT_GUIDANCE_SURFACES = ["apps/desktop", "apps/desktop/src-tauri", "apps/mcp-server"];

const STALE_STYLING_ROUTE_RE =
  /index\.css[^\n]{0,40}@layer components|@layer components[^\n]{0,40}index\.css|with CVA variants/;
const MAX_PROSE_LINE_LENGTH = 160;

function longProseLines(file, source) {
  const failures = [];
  let inFence = false;
  for (const [index, line] of source.split("\n").entries()) {
    if (line.trimStart().startsWith("```")) {
      inFence = !inFence;
      continue;
    }
    if (inFence || line.trimStart().startsWith("|")) continue;
    if (line.length > MAX_PROSE_LINE_LENGTH) failures.push(`${file}:${index + 1}`);
  }
  return failures;
}

export function agentGuidanceFailures(read, exists, listFiles) {
  const failures = [];

  const missingGuides = AGENT_GUIDANCE_SURFACES.flatMap((dir) =>
    ["AGENTS.md", "CLAUDE.md"].map((name) => `${dir}/${name}`).filter((file) => !exists(file)),
  );
  if (missingGuides.length > 0) {
    failures.push(
      `Every app surface needs an AGENTS.md and a CLAUDE.md pointer so directory-local rules travel with the code: ${missingGuides.join(", ")}`,
    );
  }

  const staleStylingGuides = ["CLAUDE.md", "AGENTS.md"]
    .concat(listFiles("apps", (file) => /(^|\/)(CLAUDE|AGENTS)\.md$/i.test(file)))
    .filter(exists)
    .filter((file) => STALE_STYLING_ROUTE_RE.test(read(file)));
  if (staleStylingGuides.length > 0) {
    failures.push(
      `Agent guidance must point at src/styles/*.css partials (see COMPONENT_GUIDE.md), not index.css @layer components, and must not describe Button as CVA-based: ${staleStylingGuides.join(", ")}`,
    );
  }

  const guidanceFiles = ["AGENTS.md", ...listFiles("apps", (file) => file.endsWith("/AGENTS.md"))]
    .filter(exists)
    .flatMap((file) => longProseLines(file, read(file)));
  if (guidanceFiles.length > 0) {
    failures.push(
      `AGENTS.md prose lines must stay within ${MAX_PROSE_LINE_LENGTH} characters; wrap guidance instead of creating narrative walls: ${guidanceFiles.join(", ")}`,
    );
  }

  return failures;
}

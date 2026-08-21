export interface BatchFixItem {
  kind: "web" | "code";
  title: string;
  severity: string;
  category: string;
  description: string;
  fixHint: string | null;
  filePath: string | null;
}

function buildStackLine(detectedStack?: Record<string, unknown> | null): string {
  if (!detectedStack || Object.keys(detectedStack).length === 0) return "";
  return `Detected stack: ${JSON.stringify(detectedStack, null, 2)}`;
}

export function buildBatchFixPrompt(
  items: BatchFixItem[],
  options: {
    url?: string;
    detectedStack?: Record<string, unknown> | null;
  },
): string {
  const stackLine = buildStackLine(options.detectedStack);
  const webItems = items.filter((i) => i.kind === "web");
  const codeItems = items.filter((i) => i.kind === "code");

  const lines: string[] = [
    "# Fix multiple issues in one pass",
    "",
    `Fix the following ${items.length} issue${items.length === 1 ? "" : "s"}${options.url ? ` on ${options.url}` : ""}.`,
    "Tackle them in the order listed (highest impact first).",
  ];
  if (stackLine) lines.push(stackLine);
  lines.push("");

  if (webItems.length > 0) {
    lines.push(`## Web Scan Issues (${webItems.length})`, "");
    for (let i = 0; i < webItems.length; i++) {
      const item = webItems[i];
      lines.push(
        `### ${i + 1}. ${item.title} (${item.severity})`,
        `Category: ${item.category}`,
        item.description,
      );
      if (item.fixHint) lines.push(`Fix direction: ${item.fixHint}`);
      lines.push("");
    }
  }

  if (codeItems.length > 0) {
    lines.push(`## Code Scan Issues (${codeItems.length})`, "");
    for (let i = 0; i < codeItems.length; i++) {
      const item = codeItems[i];
      lines.push(`### ${i + 1}. ${item.title} (${item.severity})`, `Category: ${item.category}`);
      if (item.filePath) lines.push(`File: ${item.filePath}`);
      lines.push(item.description);
      if (item.fixHint) lines.push(`Fix direction: ${item.fixHint}`);
      lines.push("");
    }
  }

  lines.push(
    "---",
    "",
    "For each issue, provide:",
    "1. The exact file(s) to change",
    "2. The minimal code or config change needed",
    "3. How to verify the fix worked",
    "",
    "Keep each fix scoped and independent so they can be applied without conflicts.",
  );

  return lines.join("\n");
}

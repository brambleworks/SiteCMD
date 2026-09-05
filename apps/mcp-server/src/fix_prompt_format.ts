import { formatCausalityBlock } from "./causal_graph.js";
import type { FixPromptRow } from "./db.js";
import { quoteUntrustedText } from "./untrusted.js";

const MAX_BODY_CHARS = 60000;

/** Bound the escaped batch, including metadata and causal context. */
export function formatFixPromptBatch(prompts: FixPromptRow[], activeCheckIds: Set<string>) {
  const entries: string[] = [];
  let remaining = MAX_BODY_CHARS;
  let shortened = false;
  for (const prompt of prompts) {
    const causal = formatCausalityBlock(prompt.check_id, activeCheckIds);
    const blocks = [
      `## ${quoteUntrustedText(prompt.title, 500)}`,
      `**Severity:** ${quoteUntrustedText(prompt.severity, 100)} | **Category:** ${quoteUntrustedText(prompt.category, 100)} | **Check:** ${quoteUntrustedText(prompt.check_id, 200)}`,
    ];
    if (causal) blocks.push("", causal);
    blocks.push("", quoteUntrustedText(prompt.fix_prompt, 20000), "", "---");
    const entry = blocks.join("\n");
    if (entry.length > remaining) {
      entries.push(`${entry.slice(0, remaining - 4)}\n...`);
      shortened = true;
      break;
    }
    entries.push(entry);
    remaining -= entry.length + 2;
    if (remaining <= 4) break;
  }
  return {
    body: entries.join("\n\n"),
    count: entries.length,
    shortened: shortened || entries.length < prompts.length,
  };
}

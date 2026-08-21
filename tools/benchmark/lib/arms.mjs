import { categoryCounts } from "./scanner.mjs";

const SHARED_CLOSER = [
  "",
  "Constraints:",
  "- Make real source edits; do not just describe changes.",
  "- Do not delete features or tests to make problems disappear.",
  "- Keep the project building and its existing tests passing.",
  "- When finished, briefly summarize what you changed.",
].join("\n");

export function blindPrompt() {
  return [
    "You are working in a code repository.",
    "",
    "Audit this repository for code-quality, security, and maintainability",
    "problems, then fix the most significant ones you find.",
    SHARED_CLOSER,
  ].join("\n");
}

export function categoriesPrompt(baseline) {
  const counts = categoryCounts(baseline);
  const lines = Object.entries(counts)
    .sort((a, b) => b[1] - a[1])
    .map(([cat, n]) => `  - ${cat}: ${n}`);
  return [
    "You are working in a code repository.",
    "",
    `An automated code scanner found ${baseline.issueCount} issues in this`,
    "repository, broken down by category as follows:",
    ...lines,
    "",
    "Find each of these issues in the source and fix them.",
    SHARED_CLOSER,
  ].join("\n");
}

export function briefPrompt(reviewText) {
  return [
    "You are working in a code repository.",
    "",
    "An automated code scanner produced the review below. It lists specific",
    "issues with their file, line, evidence, and a suggested fix. Work through",
    "the report and fix each issue at the location given.",
    "",
    "----- BEGIN SCANNER REVIEW -----",
    reviewText.trim(),
    "----- END SCANNER REVIEW -----",
    SHARED_CLOSER,
  ].join("\n");
}

/** Build the next-round prompt while varying only the selected context arm. */
export function continuationPrompt(arm, { remaining, counts, reviewText }) {
  const head = [
    "You previously worked in this repository.",
    `A re-scan of your changes still finds ${remaining} unresolved issue(s).`,
    "",
  ];
  switch (arm) {
    case "blind":
      return [...head, "Keep auditing and fixing the remaining problems.", SHARED_CLOSER].join(
        "\n",
      );
    case "categories": {
      const lines = Object.entries(counts || {})
        .sort((a, b) => b[1] - a[1])
        .map(([cat, n]) => `  - ${cat}: ${n}`);
      return [
        ...head,
        "The remaining issues break down by category as follows:",
        ...lines,
        "",
        "Find and fix each of them.",
        SHARED_CLOSER,
      ].join("\n");
    }
    case "brief":
      return [
        ...head,
        "Updated scanner report of what still remains:",
        "",
        "----- BEGIN SCANNER REVIEW -----",
        (reviewText || "").trim(),
        "----- END SCANNER REVIEW -----",
        "",
        "Fix each remaining issue at the location given.",
        SHARED_CLOSER,
      ].join("\n");
    default:
      throw new Error(`unknown arm: ${arm}`);
  }
}

export function buildPrompt(arm, { baseline, reviewText }) {
  switch (arm) {
    case "blind":
      return blindPrompt();
    case "categories":
      return categoriesPrompt(baseline);
    case "brief":
      return briefPrompt(reviewText);
    default:
      throw new Error(`unknown arm: ${arm}`);
  }
}

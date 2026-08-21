import type { CodeFixGuideEntry } from "./types";

export const SCAFFOLDING_FIX_GUIDES: Record<string, CodeFixGuideEntry> = {
  "agent-instructions-stub": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Expand the instruction file so an agent is useful immediately: describe the stack and where the important code lives, explain how to run the app locally, list the exact build, test, lint, and typecheck commands an agent should use to verify its own work, and write down the hard rules, such as conventions to follow and files that must never be touched.",
    ],
  },
  "agent-instructions-fragmented": {
    effort: "quick",
    effortMinutes: 10,
    default: [
      "Choose one canonical instruction file (AGENTS.md is the emerging cross-tool convention), move the real guidance into it, and replace the other files with short pointers to it. Verify each remaining file either defers to the canonical file or is deliberately tool-specific.",
    ],
  },
  "agent-instructions-secret": {
    effort: "quick",
    effortMinutes: 15,
    default: [
      "Classify the match first, without printing it: public identifiers, placeholders, checksums, revoked values, and test fixtures can resemble keys. If it is not a live credential, document why it cannot authenticate and mark the finding reviewed.",
      "If it is a live credential with confirmed exposure, revoke or rotate it before relying on a file edit, then replace the literal with an environment-variable reference or the tool's supported credential store. Search repository history, logs, and agent transcripts under your organization's incident policy, and verify the old credential fails.",
    ],
  },
  "agent-instructions-legacy-format": {
    effort: "moderate",
    effortMinutes: 20,
    default: [
      "Split the guidance into focused files in the editor's rules directory, scoping each rule to where it applies (for example, frontend conventions versus backend commands) so the editor only loads what is relevant. Then remove the legacy single file, or leave a short pointer if you still support an older version of the tool.",
    ],
  },
};

/** Causal graph helpers over JSON generated from the Rust source of truth. */

import { readFileSync } from "fs";
import { dirname, join } from "path";
import { fileURLToPath } from "url";
import { severityRank } from "./severity.js";

type Confidence = "high" | "medium" | "low";

export interface CausalLink {
  cause: string;
  effect: string;
  confidence: Confidence;
}

interface CausalReference {
  check_id: string;
  confidence: Confidence;
}

const __dirname = dirname(fileURLToPath(import.meta.url));
export const CAUSAL_LINKS: readonly CausalLink[] = parseCausalGraph(readGraphJson());

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function parseConfidence(value: unknown): Confidence | null {
  return value === "high" || value === "medium" || value === "low" ? value : null;
}

function parseCausalLink(value: unknown): CausalLink | null {
  if (!isRecord(value)) return null;
  if (typeof value.cause !== "string" || typeof value.effect !== "string") return null;
  const cause = value.cause.trim();
  const effect = value.effect.trim();
  const confidence = parseConfidence(value.confidence);
  if (!cause || !effect || !confidence) return null;
  return { cause, effect, confidence };
}

export function parseCausalGraph(value: unknown): readonly CausalLink[] {
  if (!isRecord(value) || !Array.isArray(value.links)) {
    throw new Error("Generated causal graph is missing a links array");
  }
  return value.links.map((link, index) => {
    const parsed = parseCausalLink(link);
    if (!parsed) {
      throw new Error(`Generated causal graph contains an invalid link at index ${index}`);
    }
    return parsed;
  });
}

function readGraphJson(): unknown {
  const graphPath = join(__dirname, "causal_graph.json");
  try {
    return JSON.parse(readFileSync(graphPath, "utf8")) as unknown;
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(`Unable to load generated causal graph at ${graphPath}: ${detail}`, {
      cause: error,
    });
  }
}

/** Return only observed, active causes of `checkId`. */
export function getCausesOf(checkId: string, activeCheckIds: Set<string>): CausalReference[] {
  return CAUSAL_LINKS.filter((l) => l.effect === checkId && activeCheckIds.has(l.cause)).map(
    (l) => ({
      check_id: l.cause,
      confidence: l.confidence,
    }),
  );
}

/** Return active effects of a check. */
export function getEffectsOf(checkId: string, activeCheckIds: Set<string>): CausalReference[] {
  return CAUSAL_LINKS.filter((l) => l.cause === checkId && activeCheckIds.has(l.effect)).map(
    (l) => ({
      check_id: l.effect,
      confidence: l.confidence,
    }),
  );
}

/** Rank causes by the highest severity they can affect without mutating input. */
export function rankWithCausalReach<T extends { check_id: string; severity: string }>(
  issues: readonly T[],
  activeCheckIds: Set<string>,
): T[] {
  const effectiveRank = (issue: T): number => {
    const selfRank = severityRank(issue.severity);
    const effects = getEffectsOf(issue.check_id, activeCheckIds);
    let best = selfRank;
    for (const eff of effects) {
      const match = issues.find((i) => i.check_id === eff.check_id);
      if (!match) continue;
      const effRank = severityRank(match.severity);
      if (effRank < best) best = effRank;
    }
    return best;
  };

  return [...issues].sort((a, b) => {
    const rankDelta = effectiveRank(a) - effectiveRank(b);
    if (rankDelta !== 0) return rankDelta;
    const sevDelta = severityRank(a.severity) - severityRank(b.severity);
    if (sevDelta !== 0) return sevDelta;
    return a.check_id.localeCompare(b.check_id);
  });
}

function formatReferences(refs: CausalReference[]): string {
  return refs
    .map((r) => (r.confidence === "high" ? `${r.check_id} (high confidence)` : r.check_id))
    .join(", ");
}

/** Build causal markdown, or an empty string when no active relation exists. */
export function formatCausalityBlock(checkId: string, activeCheckIds: Set<string>): string {
  const causes = getCausesOf(checkId, activeCheckIds);
  const effects = getEffectsOf(checkId, activeCheckIds);
  if (causes.length === 0 && effects.length === 0) return "";

  const lines: string[] = [];
  if (effects.length > 0) {
    lines.push(`**Root cause hint:** Fixing this may also resolve: ${formatReferences(effects)}.`);
  }
  if (causes.length > 0) {
    lines.push(
      `**Likely caused by:** ${formatReferences(causes)}. Consider fixing that first - this may auto-resolve.`,
    );
  }
  return lines.join("\n");
}

import { resolveFixGuide, type ResolvedGuide } from "./commands/catalog";

import type { CodeFixGuide } from "./code-fix-guides";
import type { FixGuide } from "./fix-guides";

// Resolve catalog guidance first, then the bundled baseline. Normalize emitted
// check ids because catalog entries use canonical guide keys.

let webGuideModulePromise: Promise<typeof import("./fix-guides")> | null = null;
let codeGuideModulePromise: Promise<typeof import("./code-fix-guides")> | null = null;

function loadWebGuideModule() {
  if (!webGuideModulePromise) {
    webGuideModulePromise = import("./fix-guides");
  }
  return webGuideModulePromise;
}

function loadCodeGuideModule() {
  if (!codeGuideModulePromise) {
    codeGuideModulePromise = import("./code-fix-guides");
  }
  return codeGuideModulePromise;
}

// Match display names such as "Next.js (likely)" to canonical corpus keys.
function candidateKeys(value: string): string[] {
  const lowered = value
    .toLowerCase()
    .replace(/\s*\(likely\)\s*$/, "")
    .trim();
  const canonical = lowered.replace(/\.js$/, "");
  return canonical === lowered || canonical.length === 0 ? [lowered] : [canonical, lowered];
}

// Preserve framework, CMS, then CDN precedence for catalog variants.
function variantCandidates(detectedStack?: Record<string, unknown> | null): string[] {
  const keys = ["framework", "cms", "cdn"]
    .map((key) => detectedStack?.[key])
    .filter((value): value is string => typeof value === "string")
    .flatMap(candidateKeys);
  return [...new Set(keys)];
}

// Keep a guide's effort estimate coupled to its replacement steps.
function mergeResolved<T extends { effort: string; effortMinutes: number; steps: string[] }>(
  bundled: T | null,
  resolved: ResolvedGuide,
): T | null {
  const effort = resolved.effort ?? bundled?.effort;
  const effortMinutes = resolved.effortMinutes ?? bundled?.effortMinutes;
  if (effort === undefined || effortMinutes === undefined) return null;
  return { ...(bundled ?? {}), effort, effortMinutes, steps: resolved.steps } as T;
}

// Retain bundled guidance when catalog resolution fails.
async function resolveWithCatalog(
  checkId: string,
  candidates: string[],
  bundledSteps: string[],
): Promise<ResolvedGuide | null> {
  try {
    return await resolveFixGuide({
      bundled: bundledSteps,
      checkId,
      variantCandidates: candidates,
    });
  } catch {
    return bundledSteps.length > 0 ? { source: "bundled", steps: bundledSteps } : null;
  }
}

/** Returns baseline guidance without consulting potentially stale catalog data. */
export async function loadWebBaseline(checkId: string): Promise<FixGuide | null> {
  const { getFixGuide } = await loadWebGuideModule();
  return getFixGuide(checkId);
}

/** Baseline-only counterpart of {@link loadCodeFixGuide}; see {@link loadWebBaseline}. */
export async function loadCodeBaseline(checkId: string): Promise<CodeFixGuide | null> {
  const { getCodeFixGuide } = await loadCodeGuideModule();
  return getCodeFixGuide(checkId);
}

export async function loadWebFixGuide(
  checkId: string,
  detectedStack?: Record<string, unknown> | null,
): Promise<FixGuide | null> {
  const { getFixGuide, normalizeFixGuideKey } = await loadWebGuideModule();
  const bundled = getFixGuide(checkId);
  // A check with no baseline can still have a catalog guide, so the lookup
  // runs either way, keyed the way the corpus is authored.
  const resolved = await resolveWithCatalog(
    normalizeFixGuideKey(checkId),
    variantCandidates(detectedStack),
    bundled?.steps ?? [],
  );
  if (!resolved) return null;
  return mergeResolved(bundled, resolved);
}

export async function loadCodeFixGuide(
  checkId: string,
  framework?: string | null,
): Promise<CodeFixGuide | null> {
  const { getCodeFixGuide } = await loadCodeGuideModule();
  const bundled = getCodeFixGuide(checkId);
  const resolved = await resolveWithCatalog(
    checkId,
    framework ? candidateKeys(framework) : [],
    bundled?.steps ?? [],
  );
  if (!resolved) return null;
  return mergeResolved(bundled, resolved);
}

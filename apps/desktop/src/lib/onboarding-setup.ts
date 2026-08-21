export type OnboardingSetupStep = "baseline-review" | "code-scan" | "integrations" | "updates";

const VALID_STEPS: ReadonlySet<string> = new Set<OnboardingSetupStep>([
  "baseline-review",
  "code-scan",
  "integrations",
  "updates",
]);

function getOnboardingSetupKey(projectId: number) {
  return `sitecmd_setup_pending:${projectId}`;
}

function normalizeSetupSteps(raw: unknown): OnboardingSetupStep[] {
  if (Array.isArray(raw)) {
    return raw.filter(
      (step): step is OnboardingSetupStep => typeof step === "string" && VALID_STEPS.has(step),
    );
  }
  if (typeof raw === "string" && VALID_STEPS.has(raw)) {
    return [raw as OnboardingSetupStep];
  }
  return [];
}

export function readOnboardingSetupSteps(projectId: number): OnboardingSetupStep[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem(getOnboardingSetupKey(projectId));
    if (!raw) return [];
    return normalizeSetupSteps(JSON.parse(raw));
  } catch {
    return [];
  }
}

export function writeOnboardingSetupSteps(projectId: number, steps: OnboardingSetupStep[]) {
  if (typeof window === "undefined") return;
  const normalized = normalizeSetupSteps(steps);
  const key = getOnboardingSetupKey(projectId);
  try {
    if (normalized.length === 0) {
      window.localStorage.removeItem(key);
      return;
    }
    window.localStorage.setItem(key, JSON.stringify(normalized));
  } catch {
    // best effort
  }
}

export function removeOnboardingSetupStep(
  projectId: number,
  step: OnboardingSetupStep,
): OnboardingSetupStep[] {
  const filtered = readOnboardingSetupSteps(projectId).filter((entry) => entry !== step);
  writeOnboardingSetupSteps(projectId, filtered);
  return filtered;
}

export function consumeOnboardingSetupStepForTarget(
  projectId: number,
  target: string,
): OnboardingSetupStep[] {
  const normalizedTarget = target === "settings:integrations" ? "integrations" : target;
  if (normalizedTarget !== "integrations" && normalizedTarget !== "updates") {
    return readOnboardingSetupSteps(projectId);
  }
  return removeOnboardingSetupStep(projectId, normalizedTarget);
}

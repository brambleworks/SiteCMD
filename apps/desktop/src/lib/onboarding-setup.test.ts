import { beforeEach, describe, expect, it } from "vitest";

import {
  consumeOnboardingSetupStepForTarget,
  readOnboardingSetupSteps,
  removeOnboardingSetupStep,
  writeOnboardingSetupSteps,
} from "./onboarding-setup";

describe("onboarding setup helpers", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("writes and reads normalized setup steps", () => {
    writeOnboardingSetupSteps(7, ["baseline-review", "code-scan", "integrations"]);

    expect(readOnboardingSetupSteps(7)).toEqual(["baseline-review", "code-scan", "integrations"]);
  });

  it("removes a single setup step", () => {
    writeOnboardingSetupSteps(7, ["code-scan", "updates"]);

    expect(removeOnboardingSetupStep(7, "code-scan")).toEqual(["updates"]);
    expect(readOnboardingSetupSteps(7)).toEqual(["updates"]);
  });

  it("consumes the matching integrations target alias", () => {
    writeOnboardingSetupSteps(7, ["integrations", "updates"]);

    expect(consumeOnboardingSetupStepForTarget(7, "settings:integrations")).toEqual(["updates"]);
    expect(readOnboardingSetupSteps(7)).toEqual(["updates"]);
  });

  it("leaves unrelated targets untouched", () => {
    writeOnboardingSetupSteps(7, ["code-scan", "updates"]);

    expect(consumeOnboardingSetupStepForTarget(7, "dashboard")).toEqual(["code-scan", "updates"]);
    expect(readOnboardingSetupSteps(7)).toEqual(["code-scan", "updates"]);
  });
});

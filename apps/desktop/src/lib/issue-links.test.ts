import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CheckResult, IntegrationConfig } from "@/generated/ipc-bindings";
import { enabledTrackerProviders, sendIssueToTracker } from "./issue-links";

const createIssueLinkMock = vi.fn();

vi.mock("@/lib/commands", () => ({
  createIssueLink: (...args: unknown[]) => createIssueLinkMock(...args),
}));

const ISSUE: CheckResult = {
  checkId: "security.csp",
  category: "security",
  title: "Content Security Policy is missing",
  description: "Responses do not include a CSP header.",
  status: "fail",
  severity: "high",
  fixPrompt: null,
  manualFix: null,
  rawData: null,
  confidence: "high",
};

function config(overrides: Partial<IntegrationConfig>): IntegrationConfig {
  return {
    integrationType: "github",
    apiKey: null,
    siteId: null,
    extra: null,
    enabled: true,
    ...overrides,
  };
}

describe("enabledTrackerProviders", () => {
  it("returns only enabled tracker integrations, github before jira", () => {
    const providers = enabledTrackerProviders([
      config({ integrationType: "jira" }),
      config({ integrationType: "github" }),
      config({ integrationType: "plausible" }),
    ]);
    expect(providers).toEqual(["github", "jira"]);
  });

  it("skips disabled trackers and non-tracker integrations", () => {
    const providers = enabledTrackerProviders([
      config({ integrationType: "github", enabled: false }),
      config({ integrationType: "cloudflare" }),
      config({ integrationType: "jira" }),
    ]);
    expect(providers).toEqual(["jira"]);
  });

  it("returns an empty list when nothing is configured", () => {
    expect(enabledTrackerProviders([])).toEqual([]);
  });
});

describe("sendIssueToTracker", () => {
  beforeEach(() => {
    createIssueLinkMock.mockReset();
  });

  it("sends only immutable finding identifiers and score impact", async () => {
    createIssueLinkMock.mockResolvedValue({ id: 1 });

    await sendIssueToTracker({
      projectId: 7,
      scanId: 42,
      provider: "github",
      issue: ISSUE,
      estimatedImpact: 4,
    });

    expect(createIssueLinkMock).toHaveBeenCalledWith({
      projectId: 7,
      checkId: "security.csp",
      scanId: 42,
      provider: "github",
      estimatedImpact: 4,
    });
  });

  it("clamps the estimated impact to a whole non-negative number", async () => {
    createIssueLinkMock.mockResolvedValue({ id: 1 });

    await sendIssueToTracker({
      projectId: 7,
      scanId: 42,
      provider: "github",
      issue: ISSUE,
      estimatedImpact: -2.6,
    });

    expect(createIssueLinkMock.mock.calls[0][0].estimatedImpact).toBe(0);
  });
});

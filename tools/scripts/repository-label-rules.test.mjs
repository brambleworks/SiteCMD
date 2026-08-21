import { describe, expect, it } from "vitest";

import {
  liveRepositoryLabelFailures,
  repositoryLabelFailures,
} from "./lib/repository-label-rules.mjs";

const renovate = JSON.stringify({
  labels: ["dependencies"],
  vulnerabilityAlerts: { labels: ["security"] },
});
const dependabot = `updates:
  - package-ecosystem: github-actions
    labels:
      - dependencies
      - github-actions
`;
const issueTemplate = `name: Bug report
labels:
  - bug
  - "security # review" # quoted label
body: []
`;

describe("repository label contract", () => {
  it("accepts every label referenced by repository automation", () => {
    const contract = {
      labels: ["bug", "dependencies", "github-actions", "security", "security # review"].map(
        (name) => ({ name }),
      ),
    };

    expect(repositoryLabelFailures(contract, renovate, [dependabot, issueTemplate])).toEqual([]);
  });

  it("reports an automation label missing from the contract", () => {
    const contract = {
      labels: ["bug", "dependencies", "github-actions", "security # review"].map((name) => ({
        name,
      })),
    };

    expect(repositoryLabelFailures(contract, renovate, [dependabot, issueTemplate])).toEqual([
      'Repository label contract is missing "security".',
    ]);
  });

  it("reports a contracted label missing from GitHub", () => {
    const contract = {
      labels: [{ name: "dependencies" }, { name: "security" }],
    };

    expect(liveRepositoryLabelFailures(contract, [{ name: "dependencies" }])).toEqual([
      'GitHub repository is missing the contracted "security" label.',
    ]);
  });
});

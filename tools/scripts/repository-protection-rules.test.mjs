import { describe, expect, it } from "vitest";

import {
  liveRepositoryProtectionFailures,
  requiredCheckWorkflowFailures,
} from "./lib/repository-protection-rules.mjs";

const contract = {
  repository: "brambleworks/SiteCMD",
  privateVulnerabilityReporting: true,
  securityAndAnalysis: {
    secret_scanning: "enabled",
    secret_scanning_push_protection: "enabled",
    secret_scanning_non_provider_patterns: "enabled",
  },
  branchRuleset: {
    name: "protect-main",
    ruleTypes: ["deletion", "non_fast_forward", "required_linear_history", "pull_request"],
    requiredStatusChecks: ["Repository guardrails", "Analyze rust"],
  },
  tagRuleset: {
    name: "protect-release-tags",
    ruleTypes: ["deletion", "non_fast_forward", "update"],
  },
};

const workflows = {
  ".github/workflows/repository-guardrails.yml": [
    "name: repository-guardrails",
    "",
    "on:",
    "  pull_request:",
    "    types: [opened, synchronize]",
    "  merge_group:",
    "    types: [checks_requested]",
    "",
    "jobs:",
    "  check:",
    "    name: Repository guardrails",
    "    runs-on: ubuntu-latest",
    "",
  ].join("\n"),
  ".github/workflows/codeql.yml": [
    "name: CodeQL",
    "",
    "on:",
    "  pull_request:",
    "  merge_group:",
    "    types: [checks_requested]",
    "",
    "jobs:",
    "  analyze:",
    "    name: Analyze ${{ matrix.language }}",
    "    strategy:",
    "      matrix:",
    "        language:",
    "          - javascript-typescript",
    "          - rust",
    "",
  ].join("\n"),
  ".github/workflows/rust-tests.yml": [
    "name: rust-tests",
    "",
    "on:",
    "  pull_request:",
    "    paths:",
    '      - "apps/desktop/src-tauri/**"',
    "  merge_group:",
    "    types: [checks_requested]",
    "",
    "jobs:",
    "  test:",
    "    name: cargo nextest run",
    "",
  ].join("\n"),
};
const read = (file) => workflows[file];
const listFiles = (dir, predicate) =>
  Object.keys(workflows).filter((file) => file.startsWith(`${dir}/`) && predicate(file));

const liveClean = () => ({
  privateVulnerabilityReporting: true,
  securityAndAnalysis: {
    secret_scanning: { status: "enabled" },
    secret_scanning_push_protection: { status: "enabled" },
    secret_scanning_non_provider_patterns: { status: "enabled" },
  },
  rulesets: [
    {
      name: "protect-main",
      enforcement: "active",
      bypass_actors: [],
      rules: [
        { type: "deletion" },
        { type: "non_fast_forward" },
        { type: "required_linear_history" },
        { type: "pull_request" },
        {
          type: "required_status_checks",
          parameters: {
            required_status_checks: [
              { context: "Repository guardrails" },
              { context: "Analyze rust" },
            ],
          },
        },
      ],
    },
    {
      name: "protect-release-tags",
      enforcement: "active",
      bypass_actors: [],
      rules: [{ type: "deletion" }, { type: "non_fast_forward" }, { type: "update" }],
    },
  ],
});

describe("requiredCheckWorkflowFailures", () => {
  it("accepts checks that every pull request reports", () => {
    expect(requiredCheckWorkflowFailures(contract, read, listFiles)).toEqual([]);
  });

  it("rejects a check no workflow job produces", () => {
    const failures = requiredCheckWorkflowFailures(
      {
        ...contract,
        branchRuleset: { ...contract.branchRuleset, requiredStatusChecks: ["Frontend gates"] },
      },
      read,
      listFiles,
    );
    expect(failures.join("\n")).toContain('"Frontend gates" names no job');
  });

  it("rejects a check from a path-filtered workflow", () => {
    const failures = requiredCheckWorkflowFailures(
      {
        ...contract,
        branchRuleset: { ...contract.branchRuleset, requiredStatusChecks: ["cargo nextest run"] },
      },
      read,
      listFiles,
    );
    expect(failures.join("\n")).toContain("path-filtered workflow");
  });
});

describe("liveRepositoryProtectionFailures", () => {
  it("accepts the configured repository", () => {
    expect(liveRepositoryProtectionFailures(contract, liveClean())).toEqual([]);
  });

  it("reports private vulnerability reporting switched off", () => {
    const live = liveClean();
    live.privateVulnerabilityReporting = false;
    expect(liveRepositoryProtectionFailures(contract, live).join("\n")).toContain(
      "Private vulnerability reporting is disabled",
    );
  });

  it("reports a secret-scanning setting that drifted", () => {
    const live = liveClean();
    live.securityAndAnalysis.secret_scanning_push_protection.status = "disabled";
    expect(liveRepositoryProtectionFailures(contract, live).join("\n")).toContain(
      "secret_scanning_push_protection is disabled",
    );
  });

  it("reports a missing ruleset, a missing rule, a bypass actor, and a dropped check", () => {
    const live = liveClean();
    live.rulesets[0].rules = live.rulesets[0].rules.filter((rule) => rule.type !== "deletion");
    live.rulesets[0].bypass_actors = [{ actor_id: 5, actor_type: "RepositoryRole" }];
    live.rulesets[0].rules[3].parameters.required_status_checks.pop();
    live.rulesets.pop();
    const failures = liveRepositoryProtectionFailures(contract, live).join("\n");
    expect(failures).toContain('ruleset "protect-release-tags" does not exist');
    expect(failures).toContain('ruleset "protect-main" is missing the deletion rule');
    expect(failures).toContain("grants bypass actors");
    expect(failures).toContain('does not require "Analyze rust"');
  });
});

// Static half: every required check must come from a workflow that runs on
// every pull request, or GitHub waits forever on pull requests outside its
// paths filter. Live half: the settings SECURITY.md and the docs promise.

function pullRequestTriggerBody(source) {
  const trigger = /^ {2}pull_request:(.*)$/m.exec(source);
  if (!trigger) return null;
  const rest = source.slice(trigger.index + trigger[0].length);
  return rest.split(/^ {0,2}[a-z_-]+:/m)[0];
}

function runsOnEveryPullRequest(source) {
  const body = pullRequestTriggerBody(source);
  return body !== null && !/^ {4}paths(?:-ignore)?:/m.test(body);
}

function jobNames(source) {
  return [...source.matchAll(/^ {4}name: (.+)$/gm)].map((match) => match[1].trim());
}

function matrixValues(source, key) {
  const start = source.search(new RegExp(`^ {8}${key}:\\s*$`, "m"));
  if (start === -1) return [];
  const values = [];
  for (const line of source.slice(start).split("\n").slice(1)) {
    const item = /^ {10}- (.+)$/.exec(line);
    if (!item) break;
    values.push(item[1].trim());
  }
  return values;
}

function jobNameMatches(template, context, source) {
  if (template === context) return true;
  const matrix = /^(.*)\$\{\{ matrix\.([a-z_]+) \}\}(.*)$/.exec(template);
  if (!matrix) return false;
  const [, prefix, key, suffix] = matrix;
  return matrixValues(source, key).some((value) => `${prefix}${value}${suffix}` === context);
}

export function requiredCheckWorkflowFailures(contract, read, listFiles) {
  const failures = [];
  const workflows = listFiles(".github/workflows", (file) => /\.ya?ml$/.test(file)).map((file) => ({
    file,
    source: read(file),
  }));
  for (const context of contract.branchRuleset.requiredStatusChecks) {
    const owners = workflows.filter(({ source }) =>
      jobNames(source).some((name) => jobNameMatches(name, context, source)),
    );
    if (owners.length === 0) {
      failures.push(
        `required check "${context}" names no job in .github/workflows; GitHub would wait for it forever.`,
      );
      continue;
    }
    if (!owners.some(({ source }) => runsOnEveryPullRequest(source))) {
      failures.push(
        `required check "${context}" comes from a path-filtered workflow (${owners
          .map((owner) => owner.file)
          .join(", ")}); a pull request outside those paths never reports it and cannot merge.`,
      );
    }
  }
  return failures;
}

export function liveRepositoryProtectionFailures(contract, live) {
  const failures = [];
  if (live.privateVulnerabilityReporting !== true) {
    failures.push(
      "Private vulnerability reporting is disabled; SECURITY.md and the issue chooser route reporters to it.",
    );
  }
  for (const [setting, expected] of Object.entries(contract.securityAndAnalysis)) {
    const actual = live.securityAndAnalysis?.[setting]?.status ?? "missing";
    if (actual !== expected) {
      failures.push(`security_and_analysis.${setting} is ${actual}; expected ${expected}.`);
    }
  }
  for (const expected of [contract.branchRuleset, contract.tagRuleset]) {
    const ruleset = live.rulesets.find((candidate) => candidate.name === expected.name);
    if (!ruleset) {
      failures.push(`ruleset "${expected.name}" does not exist.`);
      continue;
    }
    if (ruleset.enforcement !== "active") {
      failures.push(`ruleset "${expected.name}" is ${ruleset.enforcement}, not active.`);
    }
    if ((ruleset.bypass_actors ?? []).length > 0) {
      failures.push(
        `ruleset "${expected.name}" grants bypass actors; administrators must be included.`,
      );
    }
    const types = new Set((ruleset.rules ?? []).map((rule) => rule.type));
    for (const type of expected.ruleTypes) {
      if (!types.has(type))
        failures.push(`ruleset "${expected.name}" is missing the ${type} rule.`);
    }
    if (expected.requiredStatusChecks) {
      const rule = (ruleset.rules ?? []).find(
        (candidate) => candidate.type === "required_status_checks",
      );
      const contexts = new Set(
        (rule?.parameters?.required_status_checks ?? []).map((check) => check.context),
      );
      for (const context of expected.requiredStatusChecks) {
        if (!contexts.has(context)) {
          failures.push(`ruleset "${expected.name}" does not require "${context}".`);
        }
      }
    }
  }
  return failures;
}

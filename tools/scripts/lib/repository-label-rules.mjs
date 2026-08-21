function renovateLabels(value, found = new Set()) {
  if (Array.isArray(value)) {
    for (const item of value) renovateLabels(item, found);
    return found;
  }
  if (!value || typeof value !== "object") return found;
  for (const [key, item] of Object.entries(value)) {
    if (key === "labels" && Array.isArray(item)) {
      for (const label of item) {
        if (typeof label === "string") found.add(label);
      }
    }
    renovateLabels(item, found);
  }
  return found;
}

function yamlListScalar(line) {
  const item = line.trimStart();
  if (item[0] !== "-" || (item[1] !== " " && item[1] !== "\t")) return null;

  let quote = null;
  let end = item.length;
  for (let index = 1; index < item.length; index += 1) {
    const character = item[index];
    if (quote) {
      if (character === quote && item[index - 1] !== "\\") quote = null;
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
    } else if (character === "#" && (item[index - 1] === " " || item[index - 1] === "\t")) {
      end = index;
      break;
    }
  }

  const scalar = item.slice(1, end).trim();
  if (scalar.length >= 2 && scalar[0] === scalar.at(-1) && /["']/.test(scalar[0])) {
    return scalar.slice(1, -1).trim();
  }
  return scalar || null;
}

function yamlLabels(source) {
  const found = new Set();
  const lines = source.split(/\r?\n/);
  for (let index = 0; index < lines.length; index += 1) {
    const match = /^(\s*)labels:\s*$/.exec(lines[index]);
    if (!match) continue;
    const parentIndent = match[1].length;
    for (index += 1; index < lines.length; index += 1) {
      const line = lines[index];
      if (line.trim() === "") continue;
      const indent = /^\s*/.exec(line)[0].length;
      if (indent <= parentIndent) {
        index -= 1;
        break;
      }
      const item = yamlListScalar(line);
      if (item) found.add(item);
    }
  }
  return found;
}

export function repositoryLabelFailures(contract, renovateSource, yamlSources) {
  const failures = [];
  const labels = Array.isArray(contract?.labels) ? contract.labels : [];
  const contracted = new Set();
  for (const label of labels) {
    const name = typeof label?.name === "string" ? label.name.trim() : "";
    if (!name) {
      failures.push("Repository label contract contains an entry without a name.");
    } else if (contracted.has(name)) {
      failures.push(`Repository label contract repeats "${name}".`);
    } else {
      contracted.add(name);
    }
  }

  let renovate;
  try {
    renovate = JSON.parse(renovateSource);
  } catch (error) {
    return [...failures, `renovate.json is not valid JSON: ${error.message}`];
  }
  const configured = renovateLabels(renovate);
  for (const source of yamlSources) {
    for (const label of yamlLabels(source)) configured.add(label);
  }
  for (const label of [...configured].sort()) {
    if (!contracted.has(label)) {
      failures.push(`Repository label contract is missing "${label}".`);
    }
  }
  return failures;
}

export function liveRepositoryLabelFailures(contract, liveLabels) {
  const failures = [];
  const live = new Map(liveLabels.map((label) => [label.name.toLowerCase(), label]));
  for (const expected of contract.labels ?? []) {
    const actual = live.get(expected.name.toLowerCase());
    if (!actual) {
      failures.push(`GitHub repository is missing the contracted "${expected.name}" label.`);
      continue;
    }
    if (expected.color && actual.color.toLowerCase() !== expected.color.toLowerCase()) {
      failures.push(
        `GitHub label "${expected.name}" has color ${actual.color}; expected ${expected.color}.`,
      );
    }
    if (expected.description !== undefined && actual.description !== expected.description) {
      failures.push(
        `GitHub label "${expected.name}" has a different description from the contract.`,
      );
    }
  }
  return failures;
}

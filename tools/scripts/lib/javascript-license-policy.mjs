function tokenizeLicenseExpression(expression) {
  const tokens = [];
  const tokenPattern = /\s*(\(|\)|AND\b|OR\b|WITH\b|[A-Za-z0-9][A-Za-z0-9.+-]*)/gy;
  let offset = 0;

  while (offset < expression.length) {
    tokenPattern.lastIndex = offset;
    const match = tokenPattern.exec(expression);
    if (!match) {
      if (expression.slice(offset).trim() === "") break;
      throw new Error(
        `unexpected token near ${JSON.stringify(expression.slice(offset, offset + 20))}`,
      );
    }
    tokens.push(match[1]);
    offset = tokenPattern.lastIndex;
  }

  if (tokens.length === 0) throw new Error("empty expression");
  return tokens;
}

function parseLicenseExpression(expression) {
  const tokens = tokenizeLicenseExpression(expression);
  let cursor = 0;

  const peek = () => tokens[cursor];
  const take = () => tokens[cursor++];

  const parsePrimary = () => {
    if (peek() === "(") {
      take();
      const node = parseOr();
      if (take() !== ")") throw new Error("missing closing parenthesis");
      return node;
    }
    const token = take();
    if (!token || [")", "AND", "OR", "WITH"].includes(token)) {
      throw new Error(`expected a license identifier, found ${token ?? "end of expression"}`);
    }
    return { kind: "license", value: token };
  };

  const parseWith = () => {
    const node = parsePrimary();
    if (peek() !== "WITH") return node;
    take();
    if (node.kind !== "license") throw new Error("WITH must follow one license identifier");
    const exception = take();
    if (!exception || ["(", ")", "AND", "OR", "WITH"].includes(exception)) {
      throw new Error("WITH must name a license exception");
    }
    return { kind: "with", license: node.value, exception };
  };

  const parseAnd = () => {
    let node = parseWith();
    while (peek() === "AND") {
      take();
      node = { kind: "and", left: node, right: parseWith() };
    }
    return node;
  };

  function parseOr() {
    let node = parseAnd();
    while (peek() === "OR") {
      take();
      node = { kind: "or", left: node, right: parseAnd() };
    }
    return node;
  }

  const root = parseOr();
  if (cursor !== tokens.length) throw new Error(`unexpected token ${JSON.stringify(peek())}`);
  return root;
}

function nodeIsAllowed(node, allowedLicenses) {
  switch (node.kind) {
    case "license":
      return allowedLicenses.has(node.value);
    case "with":
      return allowedLicenses.has(`${node.license} WITH ${node.exception}`);
    case "and":
      return (
        nodeIsAllowed(node.left, allowedLicenses) && nodeIsAllowed(node.right, allowedLicenses)
      );
    case "or":
      return (
        nodeIsAllowed(node.left, allowedLicenses) || nodeIsAllowed(node.right, allowedLicenses)
      );
    default:
      return false;
  }
}

export function allowedLicensesFromCargoDeny(source) {
  const sectionStart = source.search(/^\[licenses\]\s*$/m);
  if (sectionStart === -1) throw new Error("deny.toml has no [licenses] section");
  const sectionTail = source.slice(sectionStart + "[licenses]".length);
  const nextSection = sectionTail.search(/^\[/m);
  const section = nextSection === -1 ? sectionTail : sectionTail.slice(0, nextSection);
  const allowBlock = /^allow\s*=\s*\[([\s\S]*?)^\s*\]/m.exec(section)?.[1];
  if (!allowBlock) throw new Error("deny.toml [licenses] has no allow list");
  const allowed = new Set([...allowBlock.matchAll(/"([^"]+)"/g)].map((match) => match[1]));
  if (allowed.size === 0) throw new Error("deny.toml license allow list is empty");
  return allowed;
}

export function licenseExpressionIsAllowed(expression, allowedLicenses) {
  return nodeIsAllowed(parseLicenseExpression(expression), allowedLicenses);
}

export function javascriptLicenseFailures(inventory, allowedLicenses) {
  if (!Array.isArray(inventory?.packages)) {
    return ["THIRD_PARTY_DEPENDENCIES.json has no packages array"];
  }
  const packages = inventory.packages.filter((entry) => entry?.ecosystem === "npm");
  if (packages.length === 0) return ["The dependency inventory contains no npm packages"];

  const failures = [];
  for (const entry of packages) {
    const identity = `${entry.name ?? "unknown"}@${entry.version ?? "unknown"}`;
    if (typeof entry.license !== "string" || entry.license.trim() === "") {
      failures.push(`${identity}: missing SPDX license expression`);
      continue;
    }
    try {
      if (!licenseExpressionIsAllowed(entry.license, allowedLicenses)) {
        failures.push(
          `${identity}: disallowed license expression ${JSON.stringify(entry.license)}`,
        );
      }
    } catch (error) {
      failures.push(
        `${identity}: invalid license expression ${JSON.stringify(entry.license)} (${error})`,
      );
    }
  }
  return failures.sort();
}

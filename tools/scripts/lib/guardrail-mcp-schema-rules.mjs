const SNAPSHOT = "apps/desktop/src-tauri/src/db/schema_snapshot.sql";
const MCP_REQUIRED_QUERY_FILES = [
  "apps/mcp-server/src/db.ts",
  "apps/mcp-server/src/db_correlation.ts",
  "apps/mcp-server/src/db_fix_attempts.ts",
];
const MCP_CONNECTION_FILE = "apps/mcp-server/src/db_connection.ts";
const MCP_FIX_ATTEMPTS_FILE = "apps/mcp-server/src/db_fix_attempts.ts";
const MCP_AGENT_REQUESTS_FILE = "apps/mcp-server/src/db_agent_requests.ts";
const MCP_SCHEMA_VERSION_FILE = "apps/mcp-server/src/schema_version.ts";

// Tokens that legally appear bare inside MCP SQL without naming a column.
const SQL_KEYWORDS = new Set(
  (
    "select from where and or not null is in as on join left inner outer order by " +
    "group having limit offset desc asc case when then else end distinct like exists " +
    "coalesce count sum min max insert into values update set delete union all " +
    "conflict do nothing excluded between pragma"
  ).split(" "),
);

function splitTopLevelDefinitions(body) {
  const definitions = [];
  let current = "";
  let depth = 0;
  let quote = null;
  let inLineComment = false;

  for (let index = 0; index < body.length; index += 1) {
    const character = body[index];
    const next = body[index + 1];

    if (inLineComment) {
      current += character;
      if (character === "\n") inLineComment = false;
      continue;
    }
    if (quote) {
      current += character;
      if (character === quote) {
        if (next === quote) {
          current += next;
          index += 1;
        } else {
          quote = null;
        }
      }
      continue;
    }
    if (character === "-" && next === "-") {
      current += character + next;
      index += 1;
      inLineComment = true;
      continue;
    }
    if (character === "'" || character === '"' || character === "`") {
      quote = character;
      current += character;
      continue;
    }
    if (character === "(") {
      depth += 1;
      current += character;
      continue;
    }
    if (character === ")") {
      depth = Math.max(0, depth - 1);
      current += character;
      continue;
    }
    if (character === "," && depth === 0) {
      definitions.push(current);
      current = "";
      continue;
    }
    current += character;
  }
  definitions.push(current);
  return definitions;
}

export function parseSnapshotTables(snapshotSql) {
  const tables = new Map();
  // SQLite may place an ALTER TABLE column and closing parenthesis on one line,
  // so the parser cannot require a newline before `);`.
  const tableRe = /CREATE TABLE ["`]?(\w+)["`]?\s*\(([\s\S]*?)\);/g;
  for (const match of snapshotSql.matchAll(tableRe)) {
    const [, name, body] = match;
    const columns = new Set();
    for (const definition of splitTopLevelDefinitions(body)) {
      const normalized = definition.replace(/--[^\n]*/g, "").trim();
      const first = normalized.match(/^["`]?([A-Za-z_]\w*)["`]?/)?.[1];
      if (!first) continue;
      if (/^(FOREIGN|PRIMARY|UNIQUE|CHECK|CONSTRAINT)$/i.test(first)) continue;
      columns.add(first);
    }
    tables.set(name, columns);
  }
  return tables;
}

// Match uppercase SQL verbs so prose in template literals is excluded.
function extractSqlLiterals(source) {
  const literals = [];
  for (const match of source.matchAll(/`([^`]*)`/gs)) {
    const text = match[1];
    if (/\b(SELECT|INSERT INTO|UPDATE|DELETE FROM)\b/.test(text) || /\bFROM \w+/.test(text)) {
      literals.push(text);
    }
  }
  return literals;
}

function checkSqlLiteral(sourcePath, sql, tables, failures) {
  const fail = (message) =>
    failures.push(`${sourcePath} SQL parity: ${message}\n  in: ${oneLine(sql)}`);

  // Interpolations may only appear in value positions (placeholder lists).
  if (/\b(FROM|JOIN|INTO|UPDATE)\s*\$\{/i.test(sql)) {
    fail("cannot statically resolve a table name built from an interpolation");
    return;
  }
  let text = sql.replace(/\$\{[^}]*\}/g, " __interp__ ");

  // Uppercase clauses keep lowercase prose from binding fake tables.
  const scope = new Map();
  const tableRe =
    /\b(?:FROM|JOIN|INSERT INTO|UPDATE)\s+(\w+)(?:\s+(?!ON\b|WHERE\b|SET\b|JOIN\b|LEFT\b|ORDER\b|GROUP\b|VALUES\b|AS\b)(\w+))?/g;
  for (const match of text.matchAll(tableRe)) {
    const [, table, alias] = match;
    if (!tables.has(table)) {
      fail(`table "${table}" does not exist in ${SNAPSHOT}`);
      return;
    }
    scope.set(table, table);
    if (alias) scope.set(alias, table);
  }

  for (const match of text.matchAll(/\b(\w+)\.(\w+)\b/g)) {
    const [, qualifier, column] = match;
    if (qualifier === "excluded") continue; // upsert pseudo-table
    const table = scope.get(qualifier);
    if (!table) {
      if (scope.size === 0) {
        // Statement fragment (built via string concatenation): no FROM clause
        // to bind against, so require the column to exist somewhere.
        if (![...tables.values()].some((cols) => cols.has(column))) {
          fail(`column "${qualifier}.${column}" does not exist in any table in ${SNAPSHOT}`);
        }
        continue;
      }
      fail(`cannot resolve table alias "${qualifier}" (of "${qualifier}.${column}")`);
      continue;
    }
    if (!tables.get(table).has(column)) {
      fail(`column "${column}" does not exist on "${table}" in ${SNAPSHOT}`);
    }
  }

  if (scope.size > 0) {
    const scopeTables = new Set(scope.values());
    const outputAliases = new Set([...text.matchAll(/\bAS\s+(\w+)/gi)].map((match) => match[1]));
    const pad = (span) => " ".repeat(span.length); // keep indices stable
    const bare = text
      .replace(/'[^']*'/g, pad) // SQL string literals
      .replace(/\b\w+\.\w+\b/g, pad) // qualified refs already checked
      .replace(/\bAS\s+\w+\b/gi, pad); // the alias declaration itself
    const aliasZoneStart = bare.search(/\b(GROUP BY|HAVING|ORDER BY)\b/i);
    for (const match of bare.matchAll(/\b([a-z_][a-z0-9_]*)\b/gi)) {
      const token = match[1];
      if (SQL_KEYWORDS.has(token.toLowerCase())) continue;
      if (token === "__interp__") continue;
      if (scope.has(token)) continue; // table name or alias
      if (outputAliases.has(token) && aliasZoneStart !== -1 && match.index >= aliasZoneStart) {
        continue;
      }
      if ([...scopeTables].some((table) => tables.get(table).has(token))) continue;
      fail(`bare identifier "${token}" is not a column on ${[...scopeTables].join("/")}`);
    }
  }
}

function oneLine(sql) {
  return sql.replace(/\s+/g, " ").trim().slice(0, 120);
}

export function mcpSchemaParityFailures(read, listFiles) {
  const failures = [];
  const tables = parseSnapshotTables(read(SNAPSHOT));
  if (tables.size < 10) {
    failures.push(
      `${SNAPSHOT} parsed to only ${tables.size} tables - the snapshot format changed; update guardrail-mcp-schema-rules.mjs.`,
    );
    return failures;
  }
  const dbModulePrefix = "apps/mcp-server/src/db";
  const sourcePaths = listFiles("apps/mcp-server/src", (sourcePath) => {
    if (sourcePath === MCP_SCHEMA_VERSION_FILE) return true;
    if (!sourcePath.startsWith(dbModulePrefix) || !sourcePath.endsWith(".ts")) return false;
    const moduleSuffix = sourcePath.slice(dbModulePrefix.length, -3);
    return (
      moduleSuffix === "" ||
      (moduleSuffix.startsWith("_") &&
        [...moduleSuffix.slice(1)].every(
          (character) => character === "_" || (character >= "a" && character <= "z"),
        ))
    );
  }).sort();
  for (const requiredPath of MCP_REQUIRED_QUERY_FILES) {
    if (!sourcePaths.includes(requiredPath)) {
      failures.push(
        `${requiredPath} is missing from the MCP database-module inventory; update guardrail-mcp-schema-rules.mjs if the query boundary moved.`,
      );
    }
  }

  let literalCount = 0;
  for (const sourcePath of sourcePaths) {
    const source = read(sourcePath);
    if (
      sourcePath !== MCP_CONNECTION_FILE &&
      sourcePath !== MCP_FIX_ATTEMPTS_FILE &&
      sourcePath !== MCP_AGENT_REQUESTS_FILE &&
      /\bgetDbWrite\b/.test(source)
    ) {
      failures.push(
        `${sourcePath} imports or exposes the write-capable MCP connection; only ${MCP_FIX_ATTEMPTS_FILE} and ${MCP_AGENT_REQUESTS_FILE} may use it.`,
      );
    }

    const literals = extractSqlLiterals(source);
    literalCount += literals.length;
    for (const sql of literals) {
      for (const mutation of sql.matchAll(/\b(?:INSERT INTO|UPDATE|DELETE FROM)\s+(\w+)/g)) {
        const table = mutation[1];
        const allowed =
          (sourcePath === MCP_FIX_ATTEMPTS_FILE && table === "fix_attempts") ||
          (sourcePath === MCP_AGENT_REQUESTS_FILE &&
            table === "agent_requests" &&
            /INSERT INTO agent_requests/.test(sql));
        if (!allowed) {
          failures.push(
            `${sourcePath} mutates "${table}"; the MCP write boundary permits guarded updates to existing fix_attempts rows and inserts into agent_requests only.`,
          );
        }
      }
      checkSqlLiteral(sourcePath, sql, tables, failures);
    }
  }
  if (literalCount < 10) {
    failures.push(
      `MCP database modules yielded only ${literalCount} SQL literals - the extraction heuristic or module inventory broke; update guardrail-mcp-schema-rules.mjs.`,
    );
  }
  return failures;
}

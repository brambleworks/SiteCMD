// Non-blocking accessors and Arc plumbing on the `db` receiver.
const RECEIVER_ALLOWLIST = new Set(["path", "inner", "clone", "as_ref"]);

const COMMAND_ATTRIBUTE = "#[tauri::command]";

// Flat alternatives avoid nested quantifiers while skipping stacked attributes.
const FN_SIGNATURE = /\basync\s+fn\s+([A-Za-z0-9_]+)|\bfn\s+([A-Za-z0-9_]+)/;

// Return the balanced function body at or after an offset.
function matchBraceBody(source, from) {
  const open = source.indexOf("{", from);
  if (open === -1) return null;
  let depth = 0;
  for (let i = open; i < source.length; i += 1) {
    const ch = source[i];
    if (ch === "{") depth += 1;
    else if (ch === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(open, i + 1);
    }
  }
  return null;
}

/** Every `async fn` bearing #[tauri::command] in one Rust source file. */
function asyncCommandBodies(source) {
  const commands = [];
  let cursor = source.indexOf(COMMAND_ATTRIBUTE);
  while (cursor !== -1) {
    const rest = source.slice(cursor + COMMAND_ATTRIBUTE.length);
    const signature = FN_SIGNATURE.exec(rest);
    if (signature) {
      const bodyStart = cursor + COMMAND_ATTRIBUTE.length + signature.index;
      const body = matchBraceBody(source, bodyStart);
      const asyncName = signature[1];
      if (body !== null && asyncName) {
        commands.push({ name: asyncName, body });
      }
    }
    cursor = source.indexOf(COMMAND_ATTRIBUTE, cursor + COMMAND_ATTRIBUTE.length);
  }
  return commands;
}

/** Mark argument ranges that execute on blocking threads. */
function blockingWrapperMask(body) {
  const mask = new Array(body.length).fill(false);
  const wrapper = /\b(?:run_blocking|spawn_blocking)\s*\(/g;
  let call;
  while ((call = wrapper.exec(body)) !== null) {
    const open = call.index + call[0].length - 1;
    let depth = 0;
    for (let i = open; i < body.length; i += 1) {
      if (body[i] === "(") depth += 1;
      else if (body[i] === ")") {
        depth -= 1;
        if (depth === 0) {
          for (let j = open; j <= i; j += 1) mask[j] = true;
          break;
        }
      }
    }
  }
  return mask;
}

/** Find direct DB calls outside blocking wrappers. */
function directDbCalls(body) {
  const mask = blockingWrapperMask(body);
  const methods = [];
  // Match only the bare `db` binding across rustfmt line breaks.
  const call = /(?<![.\w])db\s*\.\s*([A-Za-z0-9_]+)\s*\(/g;
  let match;
  while ((match = call.exec(body)) !== null) {
    if (mask[match.index]) continue;
    const method = match[1];
    if (RECEIVER_ALLOWLIST.has(method)) continue;
    methods.push(method);
  }
  return methods;
}

// Every directory whose Rust sources define #[tauri::command] handlers.
const COMMAND_SOURCE_DIRS = [
  "apps/desktop/src-tauri/src/commands",
  "apps/desktop/src-tauri/src/licensing/commands",
];

/**
 * @param {(file: string) => string} read
 * @param {(dir: string, predicate: (file: string) => boolean) => string[]} listFiles
 */
export function asyncCommandDbBlockingFailures(read, listFiles) {
  const files = COMMAND_SOURCE_DIRS.flatMap((dir) =>
    listFiles(dir, (file) => file.endsWith(".rs")),
  );
  const failures = [];
  for (const file of files) {
    const source = read(file);
    if (!source.includes(COMMAND_ATTRIBUTE)) continue;
    for (const command of asyncCommandBodies(source)) {
      for (const method of directDbCalls(command.body)) {
        failures.push(
          `${file} - async command ${command.name} calls db.${method}(...) inline on the async runtime; Database methods block the calling thread, so route the call through run_blocking(...) (see commands/mod.rs).`,
        );
      }
    }
  }
  return failures;
}

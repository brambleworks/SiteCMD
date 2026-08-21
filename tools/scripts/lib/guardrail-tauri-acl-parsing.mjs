const CAPABILITIES_DIR = "apps/desktop/src-tauri/capabilities";
const PERMISSIONS_DIR = "apps/desktop/src-tauri/permissions";

// Return null instead of treating an unparseable handler block as empty.
export function handlerCommandNames(read) {
  const handlerBlock = read("apps/desktop/src-tauri/src/lib.rs")
    .split("tauri::generate_handler![")
    .at(1)
    ?.split("])")
    .at(0);
  if (!handlerBlock) return null;
  // Commas separate entries; a trailing comma is optional Rust syntax.
  return new Set(
    stripRustComments(handlerBlock)
      .split(",")
      .map((entry) => entry.split("::").at(-1).trim())
      .filter((entry) => /^[a-z][a-z0-9_]*$/.test(entry)),
  );
}

// Commented-out handlers are not part of the runtime surface.
function stripRustComments(source) {
  return source.replace(/\/\*[\s\S]*?\*\//g, " ").replace(/\/\/[^\n]*/g, "");
}

// Resolve direct grants and local permission sets. Null indicates unreadable input.
export function grantedCommandNames(read, listFiles, mainWindowOnly = false) {
  const granted = new Set();
  // Tauri deny rules override allow rules.
  const denied = new Set();
  const addEntry = (entry) => {
    const allow = /^allow-([a-z][a-z0-9-]*)$/.exec(entry);
    if (allow) granted.add(allow[1].replaceAll("-", "_"));
    const deny = /^deny-([a-z][a-z0-9-]*)$/.exec(entry);
    if (deny) denied.add(deny[1].replaceAll("-", "_"));
  };
  let capabilityFiles;
  try {
    capabilityFiles = listFiles(CAPABILITIES_DIR, (file) => file.endsWith(".json"));
  } catch {
    return null;
  }
  for (const file of capabilityFiles) {
    let parsed;
    try {
      parsed = JSON.parse(read(file));
    } catch {
      return null;
    }
    // An absent window list means all windows; named lists must reach main.
    if (mainWindowOnly && !reachesMainWindow(parsed)) continue;
    for (const entry of Array.isArray(parsed.permissions) ? parsed.permissions : []) {
      if (typeof entry !== "string" || entry.includes(":")) continue;
      if (entry.startsWith("allow-") || entry.startsWith("deny-")) {
        addEntry(entry);
        continue;
      }
      let setSource;
      try {
        setSource = read(`${PERMISSIONS_DIR}/${entry}.toml`);
      } catch {
        return null;
      }
      // Ignore permission names in TOML comments.
      for (const match of stripTomlComments(setSource).matchAll(/"((?:allow|deny)-[a-z0-9-]+)"/g)) {
        addEntry(match[1]);
      }
    }
  }
  for (const command of denied) granted.delete(command);
  return granted;
}

/** Whether a capability can reach the platform-independent main webview. */
function reachesMainWindow(capability) {
  if (Array.isArray(capability.platforms) && capability.platforms.length > 0) return false;
  if (Array.isArray(capability.webviews) && capability.webviews.length > 0) {
    return capability.webviews.some(matchesMainWindow);
  }
  if (Array.isArray(capability.windows)) return capability.windows.some(matchesMainWindow);
  return true;
}

/** Return broker entrypoints from the frontend routing table. */
export function brokerEntrypointCommands(read) {
  const names = new Set();
  const source = read("apps/desktop/src/lib/tauri-invoke.ts");
  // Match brackets because the map may close with a TypeScript assertion.
  const table = mapLiteralBody(source, "PRIVILEGED_BROKER_COMMANDS");
  for (const match of table.matchAll(/\[\s*"[a-z0-9_]+"\s*,\s*"([a-z0-9_]+)"\s*\]/g)) {
    names.add(match[1]);
  }
  return names;
}

/** The text between `new Map([` and its matching `]`, for the named constant. */
function mapLiteralBody(source, constantName) {
  const declaration = source.indexOf(constantName);
  if (declaration < 0) return "";
  const open = source.indexOf("[", source.indexOf("new Map(", declaration));
  if (open < 0) return "";
  let depth = 0;
  for (let index = open; index < source.length; index++) {
    if (source[index] === "[") depth++;
    else if (source[index] === "]") {
      depth--;
      if (depth === 0) return source.slice(open + 1, index);
    }
  }
  return "";
}

/** Whether a Tauri window selector includes `main`. */
function matchesMainWindow(selector) {
  if (typeof selector !== "string") return false;
  if (selector === "*") return true;
  if (selector.endsWith("*")) return "main".startsWith(selector.slice(0, -1));
  return selector === "main";
}

/** Drop `#` comments, honouring quoted `#` so a grant is never truncated. */
function stripTomlComments(source) {
  return source
    .split("\n")
    .map((line) => {
      let quoted = false;
      for (let index = 0; index < line.length; index += 1) {
        if (line[index] === '"') quoted = !quoted;
        else if (line[index] === "#" && !quoted) return line.slice(0, index);
      }
      return line;
    })
    .join("\n");
}

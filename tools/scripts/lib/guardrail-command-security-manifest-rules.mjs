import { registeredAppCommands } from "./guardrail-invoke-acl-rules.mjs";

export function functionBodyContains(source, fnName, needle) {
  const escapedName = fnName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const signatures = new RegExp(
    `(?:(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn|(?:export\\s+)?(?:async\\s+)?function)\\s+${escapedName}\\b`,
    "g",
  );
  for (const signature of source.matchAll(signatures)) {
    const tailStart = signature.index + signature[0].length;
    const boundary = source.slice(tailStart).search(/[;{]/);
    if (boundary === -1 || source[tailStart + boundary] === ";") continue;
    const bodyStart = tailStart + boundary;
    let depth = 0;
    for (let cursor = bodyStart; cursor < source.length; cursor += 1) {
      const ch = source[cursor];
      if (ch === "{") {
        depth += 1;
      } else if (ch === "}") {
        depth -= 1;
        if (depth === 0) {
          if (source.slice(bodyStart, cursor).includes(needle)) return true;
          break;
        }
      }
    }
  }
  return false;
}

export function commandsAllowFromToml(read, relativePath) {
  const commands = [];
  let inAllowBlock = false;
  for (const line of read(relativePath)
    .split("\n")
    .map((value) => value.trim())) {
    const inlineAllow = line.match(/^commands\.allow\s*=\s*\[(.*)\]$/);
    if (inlineAllow) {
      for (const match of inlineAllow[1].matchAll(/"([a-z0-9_]+)"/g)) {
        commands.push(match[1]);
      }
      continue;
    }
    if (line.startsWith("commands.allow = [")) {
      inAllowBlock = true;
      continue;
    }
    if (inAllowBlock && line === "]") {
      inAllowBlock = false;
      continue;
    }
    if (inAllowBlock) {
      const command = line.replace(/,$/, "").replace(/^"|"$/g, "");
      if (command) commands.push(command);
    }
  }
  return commands;
}

function commandsFromGeneratedToml(read, relativePath) {
  const commands = new Set();
  for (const line of read(relativePath)
    .split("\n")
    .map((value) => value.trim())) {
    const inlineCommands = line.match(/^commands\.(?:allow|deny)\s*=\s*\[(.*)\]$/);
    if (!inlineCommands) continue;
    for (const match of inlineCommands[1].matchAll(/"([a-z0-9_]+)"/g)) {
      commands.add(match[1]);
    }
  }
  return [...commands];
}

export function commandSecurityManifestFailures(read, readJson, listFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };
  const manifest = readJson("apps/desktop/src-tauri/permissions/command-security.json");
  const invalidConfirmations = manifest.nativeConfirmedCommands.filter(
    (entry) =>
      typeof entry.command !== "string" ||
      typeof entry.source !== "string" ||
      (entry.confirmationPath === undefined
        ? typeof entry.requires !== "string"
        : !Array.isArray(entry.confirmationPath) ||
          entry.confirmationPath.length === 0 ||
          entry.confirmationPath.some(
            (step) => typeof step?.function !== "string" || typeof step?.requires !== "string",
          )),
  );
  check(
    invalidConfirmations.length === 0,
    `Delegated native confirmation paths must be non-empty function/require chains: ${invalidConfirmations.map((entry) => entry.command).join(", ")}`,
  );

  const confirmationChecks = manifest.nativeConfirmedCommands.flatMap((entry) => {
    const steps =
      Array.isArray(entry.confirmationPath) && entry.confirmationPath.length > 0
        ? entry.confirmationPath
        : [{ function: entry.confirmationFunction ?? entry.command, requires: entry.requires }];
    return steps
      .filter((step) => typeof step.function === "string" && typeof step.requires === "string")
      .map((step) => [entry.source, step.function, step.requires, entry.command]);
  });
  const missingConfirmations = confirmationChecks
    .filter(([file, fnName, needle]) => !functionBodyContains(read(file), fnName, needle))
    .map(([file, , , command]) => `${file}::${command}`);
  check(
    missingConfirmations.length === 0,
    `High-risk IPC commands must require native confirmation before destructive, sensitive, or executable work: ${missingConfirmations.join(", ")}`,
  );

  const confirmedNames = new Set(manifest.nativeConfirmedCommands.map((entry) => entry.command));
  const dataAdminCommands = commandsAllowFromToml(
    read,
    "apps/desktop/src-tauri/permissions/data_admin.toml",
  );
  const unconfirmedDataAdmin = dataAdminCommands.filter((command) => !confirmedNames.has(command));
  check(
    unconfirmedDataAdmin.length === 0,
    `Data-admin IPC commands must be listed in command-security.json nativeConfirmedCommands: ${unconfirmedDataAdmin.join(", ")}`,
  );

  const sensitiveBrokerFiles = [
    "apps/desktop/src-tauri/permissions/data_admin.toml",
    "apps/desktop/src-tauri/permissions/filesystem_export.toml",
    "apps/desktop/src-tauri/permissions/project_execution.toml",
  ];
  const unconfirmedSensitive = sensitiveBrokerFiles
    .flatMap((file) => commandsAllowFromToml(read, file))
    .filter((command) => !confirmedNames.has(command));
  check(
    unconfirmedSensitive.length === 0,
    `Sensitive privileged token broker commands must require native confirmation before token-backed execution: ${unconfirmedSensitive.join(", ")}`,
  );

  const elevatedAclCommands = [
    "apps/desktop/src-tauri/permissions/data_admin.toml",
    "apps/desktop/src-tauri/permissions/external_connectors.toml",
    "apps/desktop/src-tauri/permissions/filesystem_access.toml",
    "apps/desktop/src-tauri/permissions/filesystem_export.toml",
    "apps/desktop/src-tauri/permissions/project_execution.toml",
  ].flatMap((file) => commandsAllowFromToml(read, file));
  const elevatedNames = new Set(manifest.elevatedCommands);
  const missingElevated = elevatedAclCommands.filter((command) => !elevatedNames.has(command));
  const elevatedAclNames = new Set(elevatedAclCommands);
  const staleElevated = manifest.elevatedCommands.filter(
    (command) => !elevatedAclNames.has(command),
  );
  check(
    missingElevated.length === 0,
    `Elevated ACL commands must be listed in command-security.json elevatedCommands: ${missingElevated.join(", ")}`,
  );
  check(
    staleElevated.length === 0,
    `command-security.json elevatedCommands contains stale commands not present in elevated ACL: ${staleElevated.join(", ")}`,
  );

  const registeredCommands = registeredAppCommands(read);
  const staleGeneratedPermissions = listFiles(
    "apps/desktop/src-tauri/permissions/autogenerated",
    (file) => file.endsWith(".toml"),
  )
    .map((file) => ({
      file,
      staleCommands: commandsFromGeneratedToml(read, file).filter(
        (command) => !registeredCommands.has(command),
      ),
    }))
    .filter((entry) => entry.staleCommands.length > 0);
  check(
    staleGeneratedPermissions.length === 0,
    `Autogenerated Tauri permission files must only reference registered commands; remove stale generated files: ${staleGeneratedPermissions.map((entry) => `${entry.file} (${entry.staleCommands.join(", ")})`).join(", ")}`,
  );
  return failures;
}

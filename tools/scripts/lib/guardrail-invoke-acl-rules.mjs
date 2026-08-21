import { findInvokeCalls, lineNumberFor } from "./guardrail-text-utils.mjs";
import {
  brokerEntrypointCommands,
  grantedCommandNames,
  handlerCommandNames,
} from "./guardrail-tauri-acl-parsing.mjs";

const COMMANDS_DIR = "apps/desktop/src/lib/commands";

const BROKER_PERMISSION_FILES = [
  "apps/desktop/src-tauri/permissions/data_admin.toml",
  "apps/desktop/src-tauri/permissions/external_connectors.toml",
  "apps/desktop/src-tauri/permissions/filesystem_access.toml",
  "apps/desktop/src-tauri/permissions/filesystem_export.toml",
  "apps/desktop/src-tauri/permissions/project_execution.toml",
];

function appCommandNames(read) {
  const buildSource = read("apps/desktop/src-tauri/build.rs");
  const appBlock = buildSource.split("APP_COMMANDS: &[&str] = &[").at(1)?.split("];").at(0);
  return {
    parsed: Boolean(appBlock),
    names: new Set(Array.from(appBlock?.matchAll(/"([a-z0-9_]+)"/g) ?? [], (match) => match[1])),
  };
}

/** Return commands registered by `build.rs`. */
export function registeredAppCommands(read) {
  return appCommandNames(read).names;
}

/** Return commands admitted only through privileged brokers. */
export function brokerOnlyCommands(read) {
  const names = new Set();
  for (const file of BROKER_PERMISSION_FILES) {
    const allowBlocks = read(file).matchAll(/commands\.allow\s*=\s*\[([^\]]*)\]/g);
    for (const block of allowBlocks) {
      for (const match of block[1].matchAll(/"([a-z0-9_]+)"/g)) {
        names.add(match[1]);
      }
    }
  }
  return names;
}

/** Report broker-only commands exposed through direct IPC. */
export function brokerOnlyRegistrationFailures(read) {
  const failures = [];
  const brokerOnly = brokerOnlyCommands(read);
  if (brokerOnly.size === 0) {
    failures.push("Could not parse broker-only commands from the elevated permission files.");
    return failures;
  }
  const { parsed, names: registered } = appCommandNames(read);
  if (!parsed) {
    failures.push("Could not parse apps/desktop/src-tauri/build.rs APP_COMMANDS.");
    return failures;
  }
  const handlerNames = handlerCommandNames(read);
  if (!handlerNames) {
    failures.push("Could not parse apps/desktop/src-tauri/src/lib.rs generate_handler! block.");
    return failures;
  }

  const inManifest = [...brokerOnly].filter((command) => registered.has(command));
  const inHandler = [...brokerOnly].filter((command) => handlerNames.has(command));
  if (inManifest.length > 0) {
    failures.push(
      `Broker-only privileged commands must stay out of build.rs APP_COMMANDS; route them through their run_* broker: ${inManifest.join(", ")}`,
    );
  }
  if (inHandler.length > 0) {
    failures.push(
      `Broker-only privileged commands must stay out of lib.rs generate_handler!; route them through their run_* broker: ${inHandler.join(", ")}`,
    );
  }
  return failures;
}

/** Report registered IPC commands missing an applicable capability grant. */
export function ungrantedIpcCommandFailures(read, listFiles) {
  const failures = [];
  const handlerNames = handlerCommandNames(read);
  if (!handlerNames) {
    failures.push("Could not parse apps/desktop/src-tauri/src/lib.rs generate_handler! block.");
    return failures;
  }
  const granted = grantedCommandNames(read, listFiles);
  const grantedToMain = grantedCommandNames(read, listFiles, true);
  if (granted === null || grantedToMain === null) {
    failures.push("Could not read the desktop capabilities and permission sets.");
    return failures;
  }

  const ungranted = [...handlerNames].filter((command) => !granted.has(command));
  if (ungranted.length > 0) {
    failures.push(
      `Tauri IPC commands that no capability grants are denied at runtime; add allow-<command> to a capability or a permission set it references: ${ungranted.join(", ")}`,
    );
  }

  const { parsed, names: frontendCommands } = appCommandNames(read);
  if (!parsed) {
    failures.push("Could not parse apps/desktop/src-tauri/build.rs APP_COMMANDS.");
    return failures;
  }
  const unregistered = [...frontendCommands].filter((command) => !handlerNames.has(command));
  if (unregistered.length > 0) {
    failures.push(
      `Tauri IPC commands in build.rs APP_COMMANDS but absent from lib.rs generate_handler!, so nothing answers them: ${unregistered.join(", ")}`,
    );
  }

  const brokerEntrypoints = brokerEntrypointCommands(read);
  const mainDenied = [...frontendCommands].filter(
    (command) =>
      handlerNames.has(command) && !grantedToMain.has(command) && !brokerEntrypoints.has(command),
  );
  if (mainDenied.length > 0) {
    failures.push(
      `Tauri IPC commands the frontend invokes that no capability grants to the main window: ${mainDenied.join(", ")}`,
    );
  }
  return failures;
}

export function invokeAclFailures(read, listFiles) {
  const failures = [];
  const { parsed, names: registeredCommands } = appCommandNames(read);
  if (!parsed) {
    failures.push("Could not parse apps/desktop/src-tauri/build.rs APP_COMMANDS.");
    return failures;
  }
  // Wrapper literals can name commands rerouted through privileged brokers.
  const brokerRoutedCommands = brokerOnlyCommands(read);

  const missingInvokeCommands = [];
  const dynamicInvokeCalls = [];

  // The wrapper layer owns its single dynamic transport call.
  const frontendRuntimeFiles = listFiles(
    "apps/desktop/src",
    (file) =>
      /\.(ts|tsx)$/.test(file) &&
      !/(\.test|\.spec|\.behavior|\.render)\.(ts|tsx)$/.test(file) &&
      file !== "apps/desktop/src/lib/tauri-invoke.ts" &&
      !file.startsWith(`${COMMANDS_DIR}/`),
  );
  for (const file of frontendRuntimeFiles) {
    const source = read(file);
    for (const call of findInvokeCalls(source)) {
      const literal = call.arg.trim().match(/^["']([a-z0-9_]+)["']$/);
      const line = lineNumberFor(source, call.index);
      if (!literal) {
        dynamicInvokeCalls.push(`${file}:${line}`);
        continue;
      }
      if (!registeredCommands.has(literal[1])) {
        missingInvokeCommands.push(`${file}:${line} invokes ${literal[1]}`);
      }
    }
  }

  const wrapperFiles = listFiles(
    COMMANDS_DIR,
    (file) => file.endsWith(".ts") && !file.endsWith("/index.ts") && !file.endsWith("/invoke.ts"),
  );
  for (const file of wrapperFiles) {
    const source = read(file);
    for (const match of source.matchAll(/\bcommand\s*(?:<[^(;]*?>)?\s*\(\s*"([a-z0-9_]+)"/g)) {
      if (!registeredCommands.has(match[1]) && !brokerRoutedCommands.has(match[1])) {
        missingInvokeCommands.push(`${file} wraps ${match[1]}`);
      }
    }
  }

  if (missingInvokeCommands.length > 0) {
    failures.push(
      `Frontend invokes commands missing from apps/desktop/src-tauri/build.rs APP_COMMANDS and the privileged broker allowlists: ${missingInvokeCommands.join(", ")}`,
    );
  }
  if (dynamicInvokeCalls.length > 0) {
    failures.push(
      `Frontend Tauri invokes must use literal command names so ACL drift can be checked: ${dynamicInvokeCalls.join(", ")}`,
    );
  }
  failures.push(...brokerOnlyRegistrationFailures(read));
  failures.push(...ungrantedIpcCommandFailures(read, listFiles));
  return failures;
}

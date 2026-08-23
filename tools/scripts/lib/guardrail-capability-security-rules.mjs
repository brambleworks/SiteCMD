import { commandsAllowFromToml } from "./guardrail-command-security-manifest-rules.mjs";
import { privilegedTokenIssuerFailures } from "./guardrail-privileged-token-rules.mjs";

export function capabilitySecurityFailures(read, readJson, exists, listFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const commandSecurityManifest = readJson(
    "apps/desktop/src-tauri/permissions/command-security.json",
  );
  const elevatedTauriPermissions = commandSecurityManifest.elevatedPermissions;
  const defaultCapability = readJson("apps/desktop/src-tauri/capabilities/default.json");
  const capabilityFiles = listFiles("apps/desktop/src-tauri/capabilities", (file) =>
    file.endsWith(".json"),
  );
  const capabilities = capabilityFiles.map((file) => ({ file, data: readJson(file) }));
  const disallowedDefaultCapabilityPermissions = [
    "core:window:allow-create",
    "core:webview:allow-create-webview-window",
  ];
  const defaultWindowCreatePermissions = defaultCapability.permissions.filter((permission) =>
    disallowedDefaultCapabilityPermissions.includes(permission),
  );
  check(
    defaultWindowCreatePermissions.length === 0,
    `Default Tauri capability must not allow renderer-created windows/webviews: ${defaultWindowCreatePermissions.join(", ")}`,
  );
  const defaultElevatedPermissions = defaultCapability.permissions.filter((permission) =>
    elevatedTauriPermissions.includes(permission),
  );
  check(
    defaultElevatedPermissions.length === 0,
    `Default Tauri capability must not include elevated permissions: ${defaultElevatedPermissions.join(", ")}`,
  );
  const forbiddenMainWindowPluginPermissions =
    commandSecurityManifest.forbiddenMainWindowPluginPermissions ?? [];
  check(
    Array.isArray(forbiddenMainWindowPluginPermissions) &&
      forbiddenMainWindowPluginPermissions.includes("keyring:default") &&
      forbiddenMainWindowPluginPermissions.includes("updater:default") &&
      forbiddenMainWindowPluginPermissions.includes("updater:allow-download-and-install"),
    "command-security.json must list forbidden main-renderer plugin permissions for keyring secrets and install-capable updater actions.",
  );
  const forbiddenPluginPermissionsMountedOnMain = capabilities
    .filter((capability) => capability.data.windows?.includes("main"))
    .flatMap((capability) =>
      (capability.data.permissions ?? [])
        .filter((permission) => forbiddenMainWindowPluginPermissions.includes(permission))
        .map((permission) => `${capability.file}:${permission}`),
    );
  check(
    forbiddenPluginPermissionsMountedOnMain.length === 0,
    `Main renderer capabilities must not grant keyring secret access or install-capable updater permissions: ${forbiddenPluginPermissionsMountedOnMain.join(", ")}`,
  );
  check(
    !exists("apps/desktop/src-tauri/capabilities/sensitive.json"),
    "Do not reintroduce a bundled sensitive.json capability; keep elevated command groups split by feature.",
  );
  for (const permission of elevatedTauriPermissions) {
    const matchingCapabilities = capabilities.filter((capability) =>
      capability.data.permissions?.includes(permission),
    );
    check(
      matchingCapabilities.length === 0,
      `No Tauri capability may grant the elevated permission set ${permission}; the broker dispatch is the only callable path. Found grants in: ${matchingCapabilities.map((capability) => capability.file).join(", ")}`,
    );
  }
  const bundledElevatedCapabilities = capabilities
    .map((capability) => ({
      file: capability.file,
      permissions:
        capability.data.permissions?.filter((permission) =>
          elevatedTauriPermissions.includes(permission),
        ) ?? [],
    }))
    .filter((capability) => capability.permissions.length > 1);
  check(
    bundledElevatedCapabilities.length === 0,
    `Elevated Tauri permissions must stay split by feature, not bundled into broad capabilities: ${bundledElevatedCapabilities.map((capability) => `${capability.file} (${capability.permissions.join(", ")})`).join(", ")}`,
  );
  const mainWindowElevatedCapabilityPolicy =
    commandSecurityManifest.mainWindowElevatedCapabilities ?? {};
  const mainMountedElevatedCapabilities = capabilities
    .flatMap((capability) =>
      (capability.data.permissions ?? [])
        .filter((permission) => elevatedTauriPermissions.includes(permission))
        .map((permission) => ({
          file: capability.file,
          permission,
          windows: capability.data.windows,
        })),
    )
    .filter((capability) => capability.windows?.includes("main"));
  const mainWindowElevatedCapabilityBudget =
    commandSecurityManifest.mainWindowElevatedCapabilityBudget ?? {};
  check(
    Number.isInteger(mainWindowElevatedCapabilityBudget.max) &&
      mainMountedElevatedCapabilities.length <= mainWindowElevatedCapabilityBudget.max &&
      mainWindowElevatedCapabilityBudget.target === 0 &&
      typeof mainWindowElevatedCapabilityBudget.targetPlan === "string" &&
      mainWindowElevatedCapabilityBudget.targetPlan.length >= 120,
    "command-security.json must cap temporary main-window elevated capabilities and document the target-zero split plan.",
  );
  for (const capability of mainMountedElevatedCapabilities) {
    const policy = mainWindowElevatedCapabilityPolicy[capability.permission];
    check(
      policy?.capability === capability.file &&
        typeof policy?.rationale === "string" &&
        policy.rationale.length >= 80 &&
        typeof policy?.nativeConfirmationPolicy === "string" &&
        policy.nativeConfirmationPolicy.length >= 40 &&
        policy.temporaryMainWindowGrant === true &&
        typeof policy?.splitPlan === "string" &&
        policy.splitPlan.length >= 100,
      `Main-window elevated capability ${capability.permission} in ${capability.file} must be documented in command-security.json with a rationale, confirmation policy, temporary grant marker, and split plan.`,
    );
  }
  const staleMainWindowElevatedPolicies = Object.entries(mainWindowElevatedCapabilityPolicy)
    .filter(
      ([permission, policy]) =>
        !mainMountedElevatedCapabilities.some(
          (capability) =>
            capability.permission === permission && capability.file === policy.capability,
        ),
    )
    .map(([permission]) => permission);
  check(
    staleMainWindowElevatedPolicies.length === 0,
    `command-security.json mainWindowElevatedCapabilities contains stale entries: ${staleMainWindowElevatedPolicies.join(", ")}`,
  );
  const brokeredMainWindowCommands = commandSecurityManifest.brokeredMainWindowCommands ?? {};
  const tauriInvokeSource = read("apps/desktop/src/lib/tauri-invoke.ts");
  const privilegedCommandBridgeSource = read("apps/desktop/src/lib/privileged-command-bridge.ts");
  const tauriDesktopLibSource = read("apps/desktop/src-tauri/src/lib.rs");
  const defaultPermissionSource = read("apps/desktop/src-tauri/permissions/default.toml");
  const brokeredPermissionCommandFiles = {
    "sitecmd-data-admin": "apps/desktop/src-tauri/permissions/data_admin.toml",
    "sitecmd-external-connectors": "apps/desktop/src-tauri/permissions/external_connectors.toml",
    "sitecmd-filesystem-access": "apps/desktop/src-tauri/permissions/filesystem_access.toml",
    "sitecmd-filesystem-export": "apps/desktop/src-tauri/permissions/filesystem_export.toml",
    "sitecmd-project-execution": "apps/desktop/src-tauri/permissions/project_execution.toml",
  };
  const brokeredPermissionRustScopes = {
    "sitecmd-data-admin": "DATA_ADMIN_COMMANDS",
    "sitecmd-external-connectors": "EXTERNAL_CONNECTOR_COMMANDS",
    "sitecmd-filesystem-access": "FILESYSTEM_ACCESS_COMMANDS",
    "sitecmd-filesystem-export": "FILESYSTEM_EXPORT_COMMANDS",
    "sitecmd-project-execution": "PROJECT_EXECUTION_COMMANDS",
  };
  const privilegedBridgeWindowPermissions = new Set([
    "sitecmd-data-admin",
    "sitecmd-external-connectors",
    "sitecmd-filesystem-access",
    "sitecmd-filesystem-export",
    "sitecmd-project-execution",
  ]);
  const brokeredDirectCommands = Object.values(brokeredPermissionCommandFiles).flatMap((file) =>
    commandsAllowFromToml(read, file),
  );
  // Limit dispatch matching to dispatcher modules, excluding confirmation copy.
  const privilegedBrokerRoot = "apps/desktop/src-tauri/src/commands/privileged_command_broker";
  const privilegedBrokerDispatchFiles = [
    `${privilegedBrokerRoot}/data_admin.rs`,
    `${privilegedBrokerRoot}/external_connectors.rs`,
    ...listFiles(`${privilegedBrokerRoot}/external_connectors`, (file) => file.endsWith(".rs")),
    `${privilegedBrokerRoot}/filesystem_access.rs`,
    `${privilegedBrokerRoot}/filesystem_export.rs`,
    `${privilegedBrokerRoot}/project_execution.rs`,
  ];
  const privilegedBrokerDispatchSource = privilegedBrokerDispatchFiles
    .map((file) => read(file))
    .join("\n");
  const privilegedBrokerSource = [
    `${privilegedBrokerRoot}/mod.rs`,
    ...privilegedBrokerDispatchFiles,
    `${privilegedBrokerRoot}/token_state.rs`,
    `${privilegedBrokerRoot}/tests.rs`,
  ]
    .map((file) => read(file))
    .join("\n");
  function rustStringArrayConst(source, constName) {
    const match = new RegExp(
      `const\\s+${constName}\\s*:\\s*&\\[&str\\]\\s*=\\s*&\\[(.*?)\\];`,
      "s",
    ).exec(source);
    if (!match) return [];
    return Array.from(match[1].matchAll(/"([^"]+)"/g), (item) => item[1]);
  }
  const brokerMatchCommands = new Set(
    Array.from(
      privilegedBrokerDispatchSource.matchAll(/^\s*"([a-z0-9_]+)"\s*=>/gm),
      (match) => match[1],
    ),
  );
  const privilegedBrokerCommandNames = new Set(
    Object.values(brokeredMainWindowCommands)
      .map((policy) => policy?.brokerCommand)
      .filter(Boolean),
  );
  for (const brokerCommand of privilegedBrokerCommandNames) {
    brokerMatchCommands.delete(brokerCommand);
  }
  const missingBrokerMatchArms = brokeredDirectCommands.filter(
    (command) => !brokerMatchCommands.has(command),
  );
  const staleBrokerMatchArms = Array.from(brokerMatchCommands).filter(
    (command) => !brokeredDirectCommands.includes(command),
  );
  const elevatedCapabilitiesMountedOnMain = capabilities
    .filter((capability) => capability.data.windows?.includes("main"))
    .flatMap((capability) =>
      (capability.data.permissions ?? []).filter((permission) =>
        elevatedTauriPermissions.includes(permission),
      ),
    );
  check(
    elevatedCapabilitiesMountedOnMain.length === 0,
    `Elevated Tauri capabilities must not mount on the main window: ${elevatedCapabilitiesMountedOnMain.join(", ")}`,
  );
  const brokeredPolicyFailures = Object.entries(brokeredPermissionCommandFiles)
    .filter(([permission, permissionFile]) => {
      const policy = brokeredMainWindowCommands[permission];
      const directCommands = commandsAllowFromToml(read, permissionFile);
      const brokerCommand = policy?.brokerCommand;
      const brokerPermission = brokerCommand ? `allow-${brokerCommand.replaceAll("_", "-")}` : "";
      const brokerPermissionMountedOnMain = defaultPermissionSource.includes(brokerPermission);
      const capabilityPermissions =
        typeof policy?.capability === "string"
          ? (readJson(policy.capability).permissions ?? [])
          : [];
      const usesPrivilegedBridge = privilegedBridgeWindowPermissions.has(permission);
      const expectedBrokerPlacement = usesPrivilegedBridge
        ? policy?.mainWindowBrokerGrant === false &&
          typeof policy?.privilegedBridge === "string" &&
          policy.privilegedBridge === "apps/desktop/src/lib/privileged-command-bridge.ts" &&
          typeof policy?.bridgeWindow === "string" &&
          capabilityPermissions.includes("core:event:default") &&
          capabilityPermissions.includes(brokerPermission) &&
          !brokerPermissionMountedOnMain &&
          privilegedCommandBridgeSource.includes(`"${policy.bridgeWindow}"`) &&
          privilegedCommandBridgeSource.includes(`"${brokerCommand}"`) &&
          !privilegedCommandBridgeSource.includes("new WebviewWindow(") &&
          tauriDesktopLibSource.includes("create_privileged_bridge_windows") &&
          tauriDesktopLibSource.includes("create_privileged_bridge_windows(app)?") &&
          tauriDesktopLibSource.includes("WebviewWindowBuilder::new") &&
          tauriDesktopLibSource.includes(`"${policy.bridgeWindow}"`) &&
          tauriInvokeSource.includes("invokeThroughPrivilegedBridge")
        : policy?.mainWindowBrokerGrant === true && brokerPermissionMountedOnMain;
      return !(
        typeof brokerCommand === "string" &&
        brokerCommand !== "run_privileged_command" &&
        policy?.directMainWindowGrant === false &&
        policy?.frontendBroker === "apps/desktop/src/lib/tauri-invoke.ts" &&
        expectedBrokerPlacement &&
        tauriInvokeSource.includes(`"${brokerCommand}"`) &&
        directCommands.every((command) => tauriInvokeSource.includes(`"${command}"`))
      );
    })
    .map(([permission]) => permission);
  const brokeredScopeFailures = Object.entries(brokeredPermissionCommandFiles)
    .filter(([permission, permissionFile]) => {
      const expected = commandsAllowFromToml(read, permissionFile).sort();
      const actual = rustStringArrayConst(
        privilegedBrokerSource,
        brokeredPermissionRustScopes[permission],
      ).sort();
      return expected.join("\n") !== actual.join("\n");
    })
    .map(([permission]) => permission);
  check(
    brokeredPolicyFailures.length === 0 &&
      !defaultPermissionSource.includes("allow-run-privileged-command") &&
      !defaultPermissionSource.includes("allow-run-data-admin-command") &&
      !defaultPermissionSource.includes("allow-run-external-connector-command") &&
      !defaultPermissionSource.includes("allow-run-filesystem-access-command") &&
      !defaultPermissionSource.includes("allow-run-filesystem-export-command") &&
      !defaultPermissionSource.includes("allow-run-project-execution-command") &&
      !tauriInvokeSource.includes('"run_privileged_command"'),
    `Main-window elevated access must keep every elevated broker off main and route them through privileged bridge windows. Broken policies: ${brokeredPolicyFailures.join(", ")}`,
  );
  check(
    brokeredScopeFailures.length === 0,
    `Feature-scoped privileged broker command lists must exactly match their elevated permission files. Broken scopes: ${brokeredScopeFailures.join(", ")}`,
  );
  check(
    missingBrokerMatchArms.length === 0 && staleBrokerMatchArms.length === 0,
    `Privileged broker match arms must exactly cover every brokered elevated permission. Missing: ${missingBrokerMatchArms.join(", ")}. Stale: ${staleBrokerMatchArms.join(", ")}.`,
  );
  const tauriInvokeTests = read("apps/desktop/src/lib/tauri-invoke.test.ts");
  check(
    privilegedCommandBridgeSource.includes("PRIVILEGED_BRIDGE_PING_EVENT") &&
      privilegedCommandBridgeSource.includes("pingPrivilegedBridge") &&
      privilegedCommandBridgeSource.includes("waitForPrivilegedBridge") &&
      privilegedCommandBridgeSource.includes("await waitForPrivilegedBridge(scope)") &&
      privilegedCommandBridgeSource.includes("pongEventName") &&
      tauriInvokeTests.includes(
        "pings privileged bridge windows before sending command requests",
      ) &&
      tauriInvokeTests.includes(
        "cleans up privileged command listeners when command dispatch fails",
      ),
    "Privileged bridge windows must use a ping/ack readiness handshake and cleanup regression tests before privileged command dispatch.",
  );
  check(
    privilegedCommandBridgeSource.includes("scope: PrivilegedBridgeScope;") &&
      privilegedCommandBridgeSource.includes("function privilegedCommandEventName") &&
      privilegedCommandBridgeSource.includes("`${PRIVILEGED_BRIDGE_EVENT}/${scope}`") &&
      privilegedCommandBridgeSource.includes("if (request.scope !== scope) return;") &&
      tauriInvokeTests.includes("sitecmd://privileged-command/external-connectors") &&
      tauriInvokeTests.includes(
        "bridge windows ignore privileged command events for a different bridge scope",
      ),
    "Privileged bridge command events must be scope-specific and ignored by bridge windows for other command families.",
  );
  failures.push(...privilegedTokenIssuerFailures(read));
  check(
    privilegedCommandBridgeSource.includes("args: Record<string, unknown>;") &&
      privilegedCommandBridgeSource.includes(
        "issuePrivilegedCommandToken(brokerCommand, command, commandArgs)",
      ) &&
      privilegedCommandBridgeSource.includes("args,") &&
      privilegedBrokerSource.includes("args_signature") &&
      privilegedBrokerSource.includes("canonical_json_value") &&
      privilegedBrokerSource.includes("Sha256::digest") &&
      privilegedBrokerSource.includes("tokens.consume(") &&
      privilegedBrokerSource.includes("request.token.as_deref()") &&
      privilegedBrokerSource.includes("broker_command") &&
      privilegedBrokerSource.includes("&request.args") &&
      privilegedBrokerSource.includes("privileged_command_tokens_are_bound_to_argument_payload") &&
      privilegedBrokerSource.includes(
        "privileged_command_token_argument_binding_is_stable_for_object_key_order",
      ) &&
      privilegedBrokerSource.includes(
        "privileged_command_token_argument_signature_does_not_store_raw_payload",
      ) &&
      tauriInvokeTests.includes("args: { projectId: 7 }") &&
      tauriInvokeTests.includes('args: { command: "pnpm install" }'),
    "Privileged command tokens must be bound to the exact argument payload and covered by frontend/Rust regression tests.",
  );

  return failures;
}

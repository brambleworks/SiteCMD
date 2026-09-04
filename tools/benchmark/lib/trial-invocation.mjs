export const agentVersions = { codex: "0.153.0-alpha.5", claude: "2.1.260" };
export const reasoning = "high";

export function trialInvocation({ agent, arm, workspace, socket, proxy }) {
  if (!["codex", "claude"].includes(agent) || !["normal", "report", "mcp"].includes(arm))
    throw new Error("Unsupported benchmark agent or arm");
  if (agent === "codex") {
    const config = [
      'forced_login_method="chatgpt"',
      'approval_policy="never"',
      'history.persistence="none"',
      'model_reasoning_effort="high"',
      'web_search="disabled"',
      "features.multi_agent=false",
      "features.memories=false",
      "features.hooks=false",
      "features.apps=false",
      'default_permissions="benchmark"',
      `permissions.benchmark={extends=":workspace",filesystem={"/home/runner"="deny","/opt/sitecmd-benchmark/products"="deny",${JSON.stringify(workspace)}="write"},network={unix_sockets={${JSON.stringify(socket)}="allow"}}}`,
      ...(arm === "mcp"
        ? [
            'mcp_servers.sitecmd.command="node"',
            `mcp_servers.sitecmd.args=${JSON.stringify([proxy, socket])}`,
            "mcp_servers.sitecmd.required=true",
            "mcp_servers.sitecmd.tool_timeout_sec=120",
          ]
        : []),
    ];
    return {
      command: "codex",
      args: [
        "exec",
        "--strict-config",
        "--ignore-user-config",
        "--ignore-rules",
        "--ephemeral",
        "--skip-git-repo-check",
        "--json",
        "--color",
        "never",
        "--model",
        "gpt-5.6-sol",
        "--cd",
        workspace,
        ...config.flatMap((value) => ["-c", value]),
        "-",
      ],
      env: {},
    };
  }
  const mcpServers = arm === "mcp" ? { sitecmd: { command: "node", args: [proxy, socket] } } : {};
  const settings = {
    sandbox: {
      enabled: true,
      failIfUnavailable: true,
      autoAllowBashIfSandboxed: true,
      allowUnsandboxedCommands: false,
      filesystem: {
        denyRead: ["/home/runner", "/opt/sitecmd-benchmark/products"],
        allowRead: [workspace],
      },
      network: { allowedDomains: [], strictAllowlist: true, allowUnixSockets: [socket] },
    },
    permissions: {
      deny: [
        "Read(//home/runner/**)",
        "Read(//opt/sitecmd-benchmark/products/**)",
        "Agent",
        "Task",
        "WebFetch",
        "WebSearch",
      ],
    },
  };
  return {
    command: "claude",
    args: [
      "--print",
      "--verbose",
      "--output-format",
      "stream-json",
      "--no-session-persistence",
      "--disable-slash-commands",
      "--no-chrome",
      "--setting-sources",
      "",
      "--strict-mcp-config",
      "--mcp-config",
      JSON.stringify({ mcpServers }),
      "--settings",
      JSON.stringify(settings),
      "--permission-mode",
      "dontAsk",
      "--tools",
      "Bash,Read,Edit,Write,Glob,Grep",
      "--disallowedTools",
      "Agent,Task,WebFetch,WebSearch",
      "--model",
      "claude-opus-5",
      "--effort",
      reasoning,
    ],
    env: {
      DISABLE_AUTOUPDATER: "1",
      CLAUDE_CODE_DISABLE_AUTO_MEMORY: "1",
      CLAUDE_CODE_SKIP_PROMPT_HISTORY: "1",
      CLAUDE_CODE_SUBPROCESS_ENV_SCRUB: "1",
    },
  };
}

import { initializeMcp, openMcp } from "./mcp-session.mjs";

export const trialUrl = "http://localhost:4173";

export async function prepareProject(desktop, mounted, item, product, configuration, arm, log) {
  const projectId = await desktop.invoke("add_project", {
    name: item.id,
    path: mounted.path,
    framework: null,
    urls: [{ url: trialUrl, environment: "local", source: "benchmark" }],
  });
  const mcp = openMcp(product.mcp, desktop.database, (event) =>
    log(arm === "mcp" ? "mcp.jsonl" : "setup.jsonl", event),
  );
  try {
    await initializeMcp(mcp, configuration.agent);
    const scan = await mcp.call("run_scan", {
      project_id: projectId,
      url: trialUrl,
      scope: "code",
      wait: true,
    });
    if (
      scan.isError ||
      !/complete: execution #\d+ \(complete\)/.test(scan.content?.[0]?.text ?? "")
    )
      throw new Error(`Desktop scan did not complete: ${JSON.stringify(scan)}`);
    let handoff = "";
    if (arm === "mcp" && item.kind === "repair") {
      const result = await mcp.call("start_fix", {
        project_id: projectId,
        url: trialUrl,
        check_id: `code_scan.${item.rule}`,
        wait: true,
      });
      if (result.isError || !/Fix attempt #\d+ is briefed/.test(result.content?.[0]?.text ?? ""))
        throw new Error(`SiteCMD could not prepare the repair: ${JSON.stringify(result)}`);
      handoff = result.content[0].text;
    }
    return { projectId, handoff };
  } finally {
    mcp.close();
  }
}

import { createServer } from "node:http";
import { chmodSync, chownSync } from "node:fs";

export async function createTrialBridge({ socket, arm, mcp, submit, owner }) {
  let busy = false;
  const server = createServer(async (request, response) => {
    const reply = (code, value) => {
      response.writeHead(code, { "Content-Type": "application/json" });
      response.end(JSON.stringify(value));
    };
    if (busy) {
      reply(429, { error: "One benchmark request at a time" });
      return;
    }
    busy = true;
    try {
      if (request.method !== "POST") throw new Error("POST required");
      const chunks = [];
      let size = 0;
      for await (const chunk of request) {
        size += chunk.length;
        if (size > 1024 * 1024) throw new Error("Request exceeds 1 MiB");
        chunks.push(chunk);
      }
      const body = JSON.parse(Buffer.concat(chunks));
      if (request.url === "/submit") {
        reply(200, await submit(body.summary, "explicit"));
      } else if (request.url === "/mcp" && arm === "mcp") {
        if (body.jsonrpc !== "2.0" || typeof body.method !== "string")
          throw new Error("Invalid MCP message");
        if (body.method === "tools/call" && body.params?.name === "request_verification")
          await submit(
            body.params.arguments?.summary,
            "verification",
            body.params.arguments?.attempt_id,
          );
        if (Object.hasOwn(body, "id")) reply(200, await mcp.request(body.method, body.params));
        else {
          mcp.notify(body.method, body.params);
          reply(200, {});
        }
      } else throw new Error("This workflow does not expose that endpoint");
    } catch (error) {
      reply(400, { error: error.message });
    } finally {
      busy = false;
    }
  });
  server.requestTimeout = 150000;
  server.headersTimeout = 5000;
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(socket, resolve);
  });
  if (owner) chownSync(socket, owner.uid, owner.gid);
  chmodSync(socket, 0o600);
  return {
    close: () =>
      new Promise((resolve) => {
        server.closeAllConnections();
        server.close(resolve);
      }),
  };
}

import { createInterface } from "node:readline";
import { bridgeRequest } from "./bridge-client.mjs";

const channel = process.argv[2];
const lines = createInterface({ input: process.stdin });
for await (const line of lines) {
  let message;
  try {
    if (line.length > 1024 * 1024) throw new Error("MCP request too large");
    message = JSON.parse(line);
    const result = await bridgeRequest(channel, "/mcp", message);
    if (Object.hasOwn(message, "id"))
      process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id: message.id, result })}\n`);
  } catch (error) {
    if (message && Object.hasOwn(message, "id"))
      process.stdout.write(
        `${JSON.stringify({ jsonrpc: "2.0", id: message.id, error: { code: -32603, message: error.message } })}\n`,
      );
    else process.stderr.write(`${error.message}\n`);
  }
}

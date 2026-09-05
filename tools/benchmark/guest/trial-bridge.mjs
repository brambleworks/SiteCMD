import { readdirSync } from "node:fs";
import { createChannel } from "./trial-channel.mjs";
import {
  publishMessage,
  readMessage,
  removeMessage,
  requestLimit,
  responseLimit,
} from "./bridge-files.mjs";

export async function createTrialBridge({ channel, arm, mcp, submit, owner, onError }) {
  const storage = createChannel(channel, owner);
  const seen = new Set();
  let active, failure;
  const dispatch = async ({ route, body }) => {
    if (route === "/submit") return await submit(body?.summary, "explicit");
    if (route !== "/mcp" || arm !== "mcp")
      throw new Error("This workflow does not expose that endpoint");
    if (body?.jsonrpc !== "2.0" || typeof body.method !== "string")
      throw new Error("Invalid MCP message");
    if (body.method === "tools/call" && body.params?.name === "request_verification")
      await submit(
        body.params.arguments?.summary,
        "verification",
        body.params.arguments?.attempt_id,
      );
    if (Object.hasOwn(body, "id")) return await mcp.request(body.method, body.params);
    mcp.notify(body.method, body.params);
    return {};
  };
  const poll = async () => {
    const names = readdirSync(`${channel}/requests`);
    if (names.length > 128) throw new Error("Benchmark request queue limit reached");
    for (const name of names.sort()) {
      if (!/^[a-f0-9]{32}\.json$/.test(name)) continue;
      const request = `${channel}/requests/${name}`;
      const id = name.slice(0, -5);
      if (seen.has(id)) {
        removeMessage(request);
        continue;
      }
      if (seen.size >= 256) throw new Error("Benchmark request count limit reached");
      seen.add(id);
      let response;
      try {
        const message = readMessage(request, requestLimit, owner?.uid);
        if (message?.version !== 1 || message.id !== id)
          throw new Error("Invalid benchmark request identity");
        response = { version: 1, id, ok: true, value: await dispatch(message) };
        if (Buffer.byteLength(JSON.stringify(response)) > responseLimit)
          throw new Error("Benchmark response exceeds the byte limit");
      } catch (error) {
        response = { version: 1, id, ok: false, error: String(error.message).slice(0, 2000) };
      }
      const target = `${channel}/responses/${name}`;
      publishMessage(target, response, responseLimit, 0o640, owner && { uid: 0, gid: owner.gid });
      removeMessage(request);
    }
  };
  const timer = setInterval(() => {
    if (active || failure) return;
    active = poll()
      .catch((error) => {
        failure = error;
        onError?.(error);
      })
      .finally(() => {
        active = null;
      });
  }, 25);
  return {
    close: async () => {
      clearInterval(timer);
      await active;
      storage.close();
      if (failure) throw failure;
    },
  };
}

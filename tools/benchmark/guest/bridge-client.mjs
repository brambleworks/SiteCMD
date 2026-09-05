import { randomBytes } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";
import {
  bridgeTimeoutMs,
  publishMessage,
  readMessage,
  removeMessage,
  requestLimit,
  responseLimit,
} from "./bridge-files.mjs";

export async function bridgeRequest(channel, route, body, { timeoutMs = bridgeTimeoutMs } = {}) {
  const id = randomBytes(16).toString("hex");
  const request = `${channel}/requests/${id}.json`;
  const response = `${channel}/responses/${id}.json`;
  publishMessage(request, { version: 1, id, route, body }, requestLimit, 0o600);
  try {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      let message;
      try {
        message = readMessage(response, responseLimit);
      } catch (error) {
        if (error.code !== "ENOENT") throw error;
        await delay(25);
        continue;
      }
      if (message?.version !== 1 || message.id !== id || typeof message.ok !== "boolean")
        throw new Error("Invalid benchmark response identity");
      if (!message.ok) throw new Error(message.error);
      return message.value;
    }
    throw new Error("Benchmark bridge timed out");
  } finally {
    removeMessage(request);
  }
}

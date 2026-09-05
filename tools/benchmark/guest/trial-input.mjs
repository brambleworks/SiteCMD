import { StringDecoder } from "node:string_decoder";

export function trialInput({ agent, stdin, prompt, initialized, fail }) {
  if (agent !== "claude") {
    stdin.end(prompt);
    return { write() {} };
  }
  const requestId = "benchmark-initialize";
  const decoder = new StringDecoder("utf8");
  let pending = "";
  let ready = false;
  stdin.write(
    JSON.stringify({
      type: "control_request",
      request_id: requestId,
      request: { subtype: "initialize" },
    }) + "\n",
  );
  return {
    write(chunk) {
      if (ready) return;
      pending += decoder.write(chunk);
      const lines = pending.split("\n");
      pending = lines.pop();
      for (const line of lines) {
        let event;
        try {
          event = JSON.parse(line);
        } catch {
          continue;
        }
        if (event.type !== "control_response" || event.response?.request_id !== requestId) continue;
        ready = true;
        try {
          if (event.response.subtype !== "success")
            throw new Error(event.response.error || "Client initialization failed");
          initialized();
          stdin.end(
            JSON.stringify({
              type: "user",
              message: { role: "user", content: prompt },
              parent_tool_use_id: null,
              session_id: "",
            }) + "\n",
          );
        } catch (error) {
          fail(error.message);
        }
        return;
      }
    },
  };
}

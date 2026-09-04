import { StringDecoder } from "node:string_decoder";

export function watchProviderEvents(requestedModel, stop) {
  const decoder = new StringDecoder("utf8");
  const observed = new Set();
  let pending = "";
  const consume = (line) => {
    let event;
    try {
      event = JSON.parse(line);
    } catch {
      return;
    }
    const models = [
      ...(event.type === "system" && event.subtype === "init" ? [event.model] : []),
      ...(event.type === "assistant" ? [event.message?.model] : []),
      ...(event.type === "result" ? Object.keys(event.modelUsage ?? {}) : []),
      ...(["thread.started", "turn.started", "turn.completed"].includes(event.type)
        ? [event.model]
        : []),
    ].filter((model) => typeof model === "string" && model.length > 0);
    for (const model of models) {
      observed.add(model);
      if (model !== requestedModel)
        stop(`Provider model differs from the frozen request: ${model}`);
    }
    if (
      (event.type === "rate_limit_event" && event.rate_limit_info?.status === "rejected") ||
      (event.error && /rate[_ -]?limit|quota|billing|credit/i.test(JSON.stringify(event.error)))
    )
      stop("Provider rate limit or billing error; batch paused");
  };
  return {
    write(chunk) {
      pending += decoder.write(chunk);
      const lines = pending.split("\n");
      pending = lines.pop();
      lines.forEach(consume);
    },
    end() {
      consume(pending + decoder.end());
      pending = "";
    },
    models: () => [...observed].sort(),
  };
}

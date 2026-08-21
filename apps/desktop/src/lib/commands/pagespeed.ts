import { command } from "./invoke";
import type { PageSpeedReport, SslProbeResult } from "@/generated/ipc-bindings";

export function getPagespeedReport(args: {
  url: string;
  strategy: string;
}): Promise<PageSpeedReport> {
  return command<PageSpeedReport>("get_pagespeed_report", args);
}

export function setPagespeedApiKey(args: { key: string }): Promise<void> {
  return command<void>("set_pagespeed_api_key", args);
}

export function pagespeedApiKeyIsSet(): Promise<boolean> {
  return command<boolean>("pagespeed_api_key_is_set");
}

export function checkSsl(args: { url: string }): Promise<SslProbeResult> {
  return command<SslProbeResult>("check_ssl", args);
}

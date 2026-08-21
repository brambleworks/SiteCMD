import { sendTelemetryRequest, type TelemetryRequest } from "@/lib/commands";

export type TelemetryTransport = (
  endpoint: string,
  body: string,
  init?: Pick<RequestInit, "headers">,
) => Promise<Response>;

type UsageRegisterRequest = Extract<TelemetryRequest, { kind: "usageRegister" }>;
type UsageEventsRequest = Extract<TelemetryRequest, { kind: "usageEvents" }>;
type UsageDeleteRequest = Extract<TelemetryRequest, { kind: "usageDelete" }>;

/** Adapt queued usage calls without sending the production endpoint over IPC. */
export const tauriTelemetryTransport: TelemetryTransport = async (endpoint, body, init) => {
  const url = new URL(endpoint);
  if (url.protocol !== "https:" || url.host !== "telemetry.sitecmd.com" || url.search || url.hash) {
    throw new Error("Telemetry endpoint is not allowed");
  }
  const parsed = JSON.parse(body) as unknown;
  let args: TelemetryRequest;
  if (url.pathname === "/v1/register") {
    args = {
      kind: "usageRegister",
      body: parsed as UsageRegisterRequest["body"],
    };
  } else if (url.pathname === "/v1/events") {
    const authorization = new Headers(init?.headers).get("Authorization");
    if (!authorization) throw new Error("Usage telemetry authorization is required");
    args = {
      kind: "usageEvents",
      body: parsed as UsageEventsRequest["body"],
      authorization,
    };
  } else if (url.pathname === "/v1/delete") {
    args = {
      kind: "usageDelete",
      body: parsed as UsageDeleteRequest["body"],
    };
  } else {
    throw new Error("Telemetry operation is not allowed");
  }
  const response = await sendTelemetryRequest({ args });
  return new Response(response.body || null, {
    status: response.status,
    headers: response.headers,
  });
};

import { command } from "./invoke";

export interface TelemetryHttpResponse {
  status: number;
  body: string;
  headers: Record<string, string>;
}

export interface BackendTelemetryConsent {
  usageAnalytics: boolean;
  crashReports: boolean;
  consentVersion: number;
  updatedAt: string | null;
}

export type TelemetryRequest =
  | {
      kind: "usageRegister";
      body: { subjectId: string; deleteProofHash: string };
    }
  | {
      kind: "usageEvents";
      body: { events: unknown[] };
      authorization: string;
    }
  | {
      kind: "usageDelete";
      body: { subjectId: string; deleteSecret: string };
    }
  | {
      kind: "crashReport";
      report: {
        name: "frontend_error" | "tauri_command_failed" | "startup_error";
        message: string;
        stack: string | null;
        properties: Record<string, string | number | boolean | null>;
        appVersion: string;
        buildChannel: string;
      };
    };

export function getTelemetryConsent(): Promise<BackendTelemetryConsent> {
  return command<BackendTelemetryConsent>("get_telemetry_consent");
}

export function setBackendTelemetryConsent(args: {
  args: { usageAnalytics: boolean; crashReports: boolean };
}): Promise<BackendTelemetryConsent> {
  return command<BackendTelemetryConsent>("set_telemetry_consent", args);
}

export function sendTelemetryRequest(args: {
  args: TelemetryRequest;
}): Promise<TelemetryHttpResponse> {
  return command<TelemetryHttpResponse>("send_telemetry_request", args);
}

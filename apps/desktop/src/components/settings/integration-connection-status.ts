import type { IntegrationData } from "./integration-services";

const LIVE_VERIFIED_SERVICE_TYPES = new Set<string>([
  "plausible",
  "cloudflare",
  "uptimerobot",
  "googleanalytics",
  "googlesearchconsole",
  "bingwebmaster",
]);

export function isIntegrationActive(
  type: string,
  configured: boolean,
  liveData: IntegrationData | undefined,
) {
  if (!configured) return false;
  if (!liveData) return true;
  if (!LIVE_VERIFIED_SERVICE_TYPES.has(type)) return true;
  return !liveData.error;
}

export function hasSetupError(
  type: string,
  configured: boolean,
  liveData: IntegrationData | undefined,
) {
  return Boolean(configured && LIVE_VERIFIED_SERVICE_TYPES.has(type) && liveData?.error);
}

import { GOOGLE_SERVICES, SERVICES } from "@/components/settings/integration-services";

export function getServiceName(type: string): string {
  const api = SERVICES.find((s) => s.type === type);
  if (api) return api.name;
  const google = GOOGLE_SERVICES.find((s) => s.type === type);
  if (google) return google.name;
  return type;
}

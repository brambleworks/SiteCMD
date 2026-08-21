import { formatSeverityToneClass } from "@/lib/severity";

export function getSeverityConfig(severity: string) {
  return { color: formatSeverityToneClass(severity) };
}

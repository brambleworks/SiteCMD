export type AnalyticsPeriod = "day" | "7d" | "30d";

export const ANALYTICS_PERIODS: { value: AnalyticsPeriod; label: string }[] = [
  { value: "day", label: "Last 24h" },
  { value: "7d", label: "7 Days" },
  { value: "30d", label: "30 Days" },
];

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
}

const COUNTRIES: Record<string, string> = {
  US: "United States",
  GB: "United Kingdom",
  DE: "Germany",
  FR: "France",
  CA: "Canada",
  AU: "Australia",
  NL: "Netherlands",
  IN: "India",
  BR: "Brazil",
  JP: "Japan",
  IT: "Italy",
  ES: "Spain",
  SE: "Sweden",
  PL: "Poland",
  MX: "Mexico",
  KR: "South Korea",
  CN: "China",
  CH: "Switzerland",
  AT: "Austria",
  BE: "Belgium",
  DK: "Denmark",
  NO: "Norway",
  FI: "Finland",
  IE: "Ireland",
  NZ: "New Zealand",
  PT: "Portugal",
  CZ: "Czech Republic",
  RO: "Romania",
  PH: "Philippines",
  TH: "Thailand",
  ZA: "South Africa",
  SG: "Singapore",
  HK: "Hong Kong",
  TW: "Taiwan",
  IL: "Israel",
  AR: "Argentina",
  TR: "Turkey",
  RU: "Russia",
  UA: "Ukraine",
  PK: "Pakistan",
  ID: "Indonesia",
};

export function countryName(code: string): string {
  return COUNTRIES[code] || code;
}

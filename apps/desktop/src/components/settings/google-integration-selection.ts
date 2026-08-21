export type GoogleIntegrationType = "googleanalytics" | "googlesearchconsole";

export type GooglePickerData = {
  ga4_properties: Array<{ property_id: string; display_name: string; account_name: string }>;
  gsc_sites: Array<{ site_url: string; permission: string }>;
  ga4_error?: string | null;
  gsc_error?: string | null;
  /** Services reconnected by refreshed backend credentials. */
  auto_saved?: string[];
};

function normalizeHost(host: string) {
  return host
    .trim()
    .toLowerCase()
    .replace(/^www\./, "");
}

export function isGoogleIntegrationType(
  type: string | null | undefined,
): type is GoogleIntegrationType {
  return type === "googleanalytics" || type === "googlesearchconsole";
}

export function googleIntegrationLabel(type: GoogleIntegrationType) {
  return type === "googleanalytics" ? "Google Analytics" : "Search Console";
}

function searchConsoleSiteMatchesProject(siteUrl: string, projectHost: string) {
  const normalizedProjectHost = normalizeHost(projectHost);
  if (!normalizedProjectHost) return false;

  if (siteUrl.startsWith("sc-domain:")) {
    const domain = normalizeHost(siteUrl.replace("sc-domain:", ""));
    return normalizedProjectHost === domain || normalizedProjectHost.endsWith(`.${domain}`);
  }

  try {
    const siteHost = normalizeHost(new URL(siteUrl).hostname);
    return siteHost === normalizedProjectHost || siteHost.endsWith(`.${normalizedProjectHost}`);
  } catch {
    return siteUrl.toLowerCase().includes(normalizedProjectHost);
  }
}

export function sortSearchConsoleSites(sites: GooglePickerData["gsc_sites"], projectHost: string) {
  return [...sites].sort((a, b) => {
    const aMatch = searchConsoleSiteMatchesProject(a.site_url, projectHost) ? 0 : 1;
    const bMatch = searchConsoleSiteMatchesProject(b.site_url, projectHost) ? 0 : 1;
    return aMatch - bMatch;
  });
}

export function filterGooglePickerData(
  data: GooglePickerData,
  type: GoogleIntegrationType | null,
): GooglePickerData {
  if (type === "googleanalytics") {
    return { ga4_properties: data.ga4_properties, gsc_sites: [], ga4_error: data.ga4_error };
  }
  if (type === "googlesearchconsole") {
    return { ga4_properties: [], gsc_sites: data.gsc_sites, gsc_error: data.gsc_error };
  }
  return data;
}

export function googleChoiceCount(data: GooglePickerData, type: GoogleIntegrationType) {
  return type === "googleanalytics" ? data.ga4_properties.length : data.gsc_sites.length;
}

export function pickPreferredGoogleChoice(
  data: GooglePickerData,
  type: GoogleIntegrationType,
  projectHost: string,
) {
  if (type === "googleanalytics") {
    return data.ga4_properties.length === 1 ? data.ga4_properties[0]?.property_id : null;
  }

  const matchingSites = data.gsc_sites.filter((site) =>
    searchConsoleSiteMatchesProject(site.site_url, projectHost),
  );
  if (matchingSites.length === 1) return matchingSites[0]?.site_url ?? null;
  if (data.gsc_sites.length === 1) return data.gsc_sites[0]?.site_url ?? null;
  return null;
}

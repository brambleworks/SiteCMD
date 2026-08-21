import { command } from "./invoke";

/** Where a resolved guide came from, so the UI can be honest about staleness. */
export interface ResolvedGuide {
  steps: string[];
  source: "catalog" | "bundled";
  /** Present only for a catalog result; a bundled caller already holds these. */
  effort?: "quick" | "moderate" | "involved";
  effortMinutes?: number;
  catalogVersion?: string;
}

export interface CatalogStatus {
  active: boolean;
  catalogVersion?: string;
  publishedAt?: string;
  /** Set when a pack exists on disk but cannot be read. Distinct from
   *  `active: false`, which means no pack has ever activated. */
  error?: string;
  /** Last credential-issuance refusal returned by activation. */
  credentialBlock?: {
    code: string;
    active?: number;
    cap?: number;
  };
  /** Distinguishes an unavailable compiled endpoint from a pending first download. */
  endpointConfigured: boolean;
}

export function getCatalogStatus(): Promise<CatalogStatus> {
  return command<CatalogStatus>("get_catalog_status", {});
}

/** Refreshes licensing and immediately retries catalog credentials. */
export function retryCatalogRefresh(): Promise<void> {
  return command<void>("retry_catalog_refresh", {});
}

/** Resolves catalog-first remediation using most-specific variants first. */
export function resolveFixGuide(args: {
  checkId: string;
  variantCandidates: string[];
  bundled: string[];
}): Promise<ResolvedGuide | null> {
  return command<ResolvedGuide | null>("resolve_fix_guide", args);
}

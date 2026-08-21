// Typed frontend surface for Rust license-activation errors.
import { isPrivilegedCommandTimeoutError } from "@/lib/privileged-command-bridge";
import { SUPPORT_EMAIL } from "@/lib/support";

type LicenseActivationErrorCode =
  | "key_required"
  | "invalid_key"
  | "not_found"
  | "store_mismatch"
  | "limit_reached"
  | "expired"
  | "variant_unknown"
  | "provider_refused"
  | "server_error"
  | "network"
  | "missing_instance_id"
  | "changed_during_activation"
  | "cancelled"
  | "incomplete"
  | "unknown";

export interface LicenseActivationErrorPayload {
  code: LicenseActivationErrorCode;
  message?: string;
}

const KNOWN_CODES: ReadonlySet<LicenseActivationErrorCode> = new Set<LicenseActivationErrorCode>([
  "key_required",
  "invalid_key",
  "not_found",
  "store_mismatch",
  "limit_reached",
  "expired",
  "variant_unknown",
  "provider_refused",
  "server_error",
  "network",
  "missing_instance_id",
  "changed_during_activation",
  "cancelled",
  "incomplete",
  "unknown",
]);

/** Return the backend verdict code, or null when the command never answered. */
function structuredCode(raw: unknown): string | null {
  const message = raw instanceof Error ? raw.message : typeof raw === "string" ? raw : "";
  if (!message) return null;
  try {
    const parsed = JSON.parse(message) as unknown;
    if (typeof parsed !== "object" || parsed === null) return null;
    const code = (parsed as Record<string, unknown>).code;
    return typeof code === "string" ? code : null;
  } catch {
    return null;
  }
}

export function parseLicenseActivationError(raw: unknown): LicenseActivationErrorPayload {
  const message = raw instanceof Error ? raw.message : typeof raw === "string" ? raw : "";
  if (message) {
    try {
      const parsed = JSON.parse(message) as Record<string, unknown>;
      if (parsed && typeof parsed === "object" && typeof parsed.code === "string") {
        const code = (
          KNOWN_CODES.has(parsed.code as LicenseActivationErrorCode) ? parsed.code : "unknown"
        ) as LicenseActivationErrorCode;
        const payload: LicenseActivationErrorPayload = { code };
        if (typeof parsed.message === "string" && parsed.message.length > 0) {
          payload.message = parsed.message;
        }
        return payload;
      }
    } catch {
      // Fall through to substring matching below.
    }
  }
  // Preserve legacy unstructured text for support diagnostics.
  return { code: "unknown", message: message || undefined };
}

export function formatLicenseActivationError(payload: LicenseActivationErrorPayload): string {
  switch (payload.code) {
    case "key_required":
      return "Enter the license key from your purchase email.";
    case "invalid_key":
      return "That doesn't look like a valid license key. Double-check the format and try again.";
    case "not_found":
      return "We couldn't find that license key. Check the spelling, or paste it directly from your purchase email.";
    case "store_mismatch":
      return `That license key was issued for a different SiteCMD build. Email ${SUPPORT_EMAIL} and we'll sort it out.`;
    case "limit_reached":
      return `This license has already been activated on the maximum number of machines. Deactivate one of them from Settings or email ${SUPPORT_EMAIL}.`;
    case "expired":
      return `This license key has expired because its subscription ended. Renew or restart your subscription from the billing portal, or email ${SUPPORT_EMAIL}.`;
    case "variant_unknown":
      return `Your license key is valid but doesn't match a tier this build of SiteCMD recognizes. Please update SiteCMD or email ${SUPPORT_EMAIL}.`;
    case "provider_refused":
      return payload.message
        ? `The license provider refused this key: ${payload.message}`
        : `The license provider refused this key. Email ${SUPPORT_EMAIL} and we'll sort it out.`;
    case "server_error":
      return `The LemonSqueezy license server returned an error. Try again in a minute, or email ${SUPPORT_EMAIL}.`;
    case "network":
      return "Couldn't reach the license server. Check your internet connection and try again.";
    case "missing_instance_id":
      return `Activation completed without a usable instance id. Please try again, or email ${SUPPORT_EMAIL} if it keeps happening.`;
    case "changed_during_activation":
      return "The installed license changed while this activation was in progress, so nothing was replaced. Check Settings and try again if this key is still the one you want.";
    case "cancelled":
      return "Activation cancelled. Your current license is unchanged.";
    case "incomplete":
      return "Activation stopped before it could complete. Try again.";
    case "unknown":
    default:
      return `Couldn't activate that license right now. Try again, or email ${SUPPORT_EMAIL} if it keeps failing.`;
  }
}

/** Classify failures without inferring an outcome from stored license state. */
export type ActivationFailureAction =
  | { announce: "nothing" }
  | { announce: "failure"; payload: LicenseActivationErrorPayload }
  | { announce: "pending" };

export function activationFailureAction(raw: unknown): ActivationFailureAction {
  if (isPrivilegedCommandTimeoutError(raw)) return { announce: "pending" };
  const code = structuredCode(raw);
  if (code === null) return { announce: "pending" };
  if (code === "cancelled") return { announce: "nothing" };
  // `unknown` means the backend could not determine the outcome.
  if (code === "unknown") return { announce: "pending" };
  return { announce: "failure", payload: parseLicenseActivationError(raw) };
}

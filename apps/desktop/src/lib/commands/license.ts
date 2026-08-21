import { command } from "./invoke";
import type { LicenseInfo } from "@/generated/ipc-bindings";

export function activateLicense(args: { key: string }): Promise<LicenseInfo> {
  return command<LicenseInfo>("activate_license", args);
}

/** Confirm activation links before installing an externally supplied key. */
export function confirmLinkLicenseActivation(): Promise<boolean> {
  return command<boolean>("confirm_link_license_activation", {});
}

/** Set `force` only for an explicit live refresh. */
export function validateLicense(options?: { force?: boolean }): Promise<LicenseInfo> {
  return options?.force
    ? command<LicenseInfo>("validate_license", { force: true })
    : command<LicenseInfo>("validate_license");
}

export function deactivateLicense(): Promise<void> {
  return command<void>("deactivate_license");
}

export function getLicenseStatus(): Promise<LicenseInfo> {
  return command<LicenseInfo>("get_license_status");
}

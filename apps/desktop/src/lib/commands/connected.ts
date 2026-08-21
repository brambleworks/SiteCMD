import type {
  ConnectedAlertFeed,
  ConnectedAlertWebhook,
  ConnectedCiToken,
  ConnectedCreatedAlertWebhook,
  ConnectedCreatedProviderConnection,
  ConnectedDestination,
  ConnectedDestinationDeletion,
  ConnectedDestinationWrite,
  ConnectedErasureReceipt,
  ConnectedImportResult,
  ConnectedInspection,
  ConnectedKeyRotation,
  ConnectedNotificationSettings,
  ConnectedNotificationSettingsWrite,
  ConnectedProviderConnection,
  ConnectedProviderProject,
  ConnectedProviderVerification,
  ConnectedRecoveryAnswer,
  ConnectedRecoveryState,
  ConnectedReconnection,
  ConnectedRemoteState,
  ConnectedReportLink,
  ConnectedReportRevocation,
  ConnectedReportRow,
  ConnectedRotatedAlertWebhook,
  ConnectedRotatedWebhookSecret,
  ConnectedServiceActivation,
  ConnectedSiteChallenge,
  ConnectedSiteCredential,
  ConnectedStatus,
  ConnectedSyncResult,
  ConnectedVerification,
  ConnectedVerificationResend,
  ConnectedWebhookSecret,
  ConnectedWebhookTest,
} from "@/generated/ipc-bindings-connected";
import { command } from "./invoke";

export interface ConnectedScopeArgs {
  projectId: number;
  environmentScopeKey: string;
}

export function getConnectedStatus(args: ConnectedScopeArgs): Promise<ConnectedStatus> {
  return command<ConnectedStatus>("get_connected_status", args);
}

export function inspectConnectedSync(args: ConnectedScopeArgs): Promise<ConnectedInspection> {
  return command<ConnectedInspection>("inspect_connected_sync", args);
}

export function syncConnectedSite(args: ConnectedScopeArgs): Promise<ConnectedSyncResult> {
  return command<ConnectedSyncResult>("sync_connected_site", args);
}

export function importConnectedConnection(
  args: ConnectedScopeArgs & {
    encryptedExport: string;
    passphrase: string;
    installationToken: string;
  },
): Promise<ConnectedImportResult> {
  return command<ConnectedImportResult>("import_connected_connection", args);
}

export function exportConnectedConnection(
  args: ConnectedScopeArgs & { passphrase: string },
): Promise<string> {
  return command<string>("export_connected_connection", args);
}

export function unlinkConnectedSite(args: ConnectedScopeArgs): Promise<void> {
  return command<void>("unlink_connected_site", args);
}

/** Stop remote monitoring, then unlink the local binding. */
export function disconnectConnectedSite(args: ConnectedScopeArgs): Promise<void> {
  return command<void>("disconnect_connected_site", args);
}

/** Permanently erase remote state and return its one-time receipt. */
export function eraseConnectedSite(args: ConnectedScopeArgs): Promise<ConnectedErasureReceipt> {
  return command<ConnectedErasureReceipt>("erase_connected_site", args);
}

/** Exchange the stored license for installation credentials. */
export function activateConnectedService(): Promise<ConnectedServiceActivation> {
  return command<ConnectedServiceActivation>("activate_connected_service", {});
}

export function createConnectedSite(
  args: ConnectedScopeArgs & { url: string; installationToken: string },
): Promise<ConnectedSiteChallenge> {
  return command<ConnectedSiteChallenge>("create_connected_site", args);
}

export function verifyConnectedSite(
  args: ConnectedScopeArgs & { method: "dns_txt" | "well_known" },
): Promise<ConnectedVerification> {
  return command<ConnectedVerification>("verify_connected_site", args);
}

export function fetchConnectedSiteState(args: ConnectedScopeArgs): Promise<ConnectedRemoteState> {
  return command<ConnectedRemoteState>("fetch_connected_site_state", args);
}

export function mintConnectedCiToken(
  args: ConnectedScopeArgs & { repository: string; workflowRef: string; gitRef: string },
): Promise<ConnectedCiToken> {
  return command<ConnectedCiToken>("mint_connected_ci_token", args);
}

/** List site credentials, including tombstones. */
export function listConnectedSiteCredentials(
  args: ConnectedScopeArgs,
): Promise<ConnectedSiteCredential[]> {
  return command<ConnectedSiteCredential[]>("list_connected_site_credentials", args);
}

/** Mint a one-time-readable deploy webhook secret. */
export function mintConnectedWebhookSecret(
  args: ConnectedScopeArgs,
): Promise<ConnectedWebhookSecret> {
  return command<ConnectedWebhookSecret>("mint_connected_webhook_secret", args);
}

/** Rotate the webhook secret while the previous generation remains valid. */
export function rotateConnectedSiteCredential(
  args: ConnectedScopeArgs & { tokenId: string },
): Promise<ConnectedRotatedWebhookSecret> {
  return command<ConnectedRotatedWebhookSecret>("rotate_connected_site_credential", args);
}

/** Revoke either credential kind by its public handle. */
export function revokeConnectedSiteCredential(
  args: ConnectedScopeArgs & { tokenId: string },
): Promise<void> {
  return command<void>("revoke_connected_site_credential", args);
}

/** Resume a disconnected site and return any newly minted webhook secret. */
export function reconnectConnectedSite(args: ConnectedScopeArgs): Promise<ConnectedReconnection> {
  return command<ConnectedReconnection>("reconnect_connected_site", args);
}

/** Start provider OAuth and return the consent URL and scopes. */
export function createConnectedProviderConnection(args: {
  provider: "vercel" | "netlify";
}): Promise<ConnectedCreatedProviderConnection> {
  return command<ConnectedCreatedProviderConnection>("create_connected_provider_connection", args);
}

export function listConnectedProviderConnections(): Promise<ConnectedProviderConnection[]> {
  return command<ConnectedProviderConnection[]>("list_connected_provider_connections", {});
}

export function listConnectedProviderProjects(args: {
  connectionId: string;
}): Promise<ConnectedProviderProject[]> {
  return command<ConnectedProviderProject[]>("list_connected_provider_projects", args);
}

export function revokeConnectedProviderConnection(args: { connectionId: string }): Promise<void> {
  return command<void>("revoke_connected_provider_connection", args);
}

/** Verify ownership through a provider project and provision its deploy trigger. */
export function verifyConnectedSiteProvider(
  args: ConnectedScopeArgs & { connectionId: string; externalProjectId: string },
): Promise<ConnectedProviderVerification> {
  return command<ConnectedProviderVerification>("verify_connected_site_provider", args);
}

/** Start a fingerprint-key rotation completed by the next full-coverage sync. */
export function rotateConnectedFingerprintKey(
  args: ConnectedScopeArgs,
): Promise<ConnectedKeyRotation> {
  return command<ConnectedKeyRotation>("rotate_connected_fingerprint_key", args);
}

export function abortConnectedKeyRotation(args: ConnectedScopeArgs): Promise<void> {
  return command<void>("abort_connected_key_rotation", args);
}

/** Request or rejoin account recovery when no admin device remains. */
export function requestAccountRecovery(): Promise<ConnectedRecoveryState> {
  return command<ConnectedRecoveryState>("request_account_recovery", {});
}

export function getAccountRecovery(): Promise<ConnectedRecoveryAnswer> {
  return command<ConnectedRecoveryAnswer>("get_account_recovery", {});
}

/** Acknowledge that this machine displayed the recovery warning. */
export function acknowledgeAccountRecovery(): Promise<ConnectedRecoveryAnswer> {
  return command<ConnectedRecoveryAnswer>("acknowledge_account_recovery", {});
}

/** Cancel the pending recovery as an admin. */
export function cancelAccountRecovery(): Promise<void> {
  return command<void>("cancel_account_recovery", {});
}

/** Create a frozen report link returned only in this response. */
export function createConnectedReport(
  args: ConnectedScopeArgs & { includeRoutes: boolean; ttlDays: number },
): Promise<ConnectedReportLink> {
  return command<ConnectedReportLink>("create_connected_report", args);
}

export function listConnectedReports(args: ConnectedScopeArgs): Promise<ConnectedReportRow[]> {
  return command<ConnectedReportRow[]>("list_connected_reports", args);
}

export function revokeConnectedReport(
  args: ConnectedScopeArgs & { reportId: string },
): Promise<ConnectedReportRevocation> {
  return command<ConnectedReportRevocation>("revoke_connected_report", args);
}

export function listConnectedAlerts(args: ConnectedScopeArgs): Promise<ConnectedAlertFeed> {
  return command<ConnectedAlertFeed>("list_connected_alerts", args);
}

/** Register an alert webhook and return its one-time signing secret. */
export function createConnectedAlertWebhook(
  args: ConnectedScopeArgs & { url: string },
): Promise<ConnectedCreatedAlertWebhook> {
  return command<ConnectedCreatedAlertWebhook>("create_connected_alert_webhook", args);
}

export function listConnectedAlertWebhooks(
  args: ConnectedScopeArgs,
): Promise<ConnectedAlertWebhook[]> {
  return command<ConnectedAlertWebhook[]>("list_connected_alert_webhooks", args);
}

/** Test a webhook, re-enabling a disabled endpoint on success. */
export function testConnectedAlertWebhook(
  args: ConnectedScopeArgs & { webhookId: string },
): Promise<ConnectedWebhookTest> {
  return command<ConnectedWebhookTest>("test_connected_alert_webhook", args);
}

export function rotateConnectedAlertWebhook(
  args: ConnectedScopeArgs & { webhookId: string },
): Promise<ConnectedRotatedAlertWebhook> {
  return command<ConnectedRotatedAlertWebhook>("rotate_connected_alert_webhook", args);
}

export function deleteConnectedAlertWebhook(
  args: ConnectedScopeArgs & { webhookId: string },
): Promise<void> {
  return command<void>("delete_connected_alert_webhook", args);
}

/** Add an unverified email destination. */
export function createConnectedDestination(args: {
  address: string;
}): Promise<ConnectedDestination> {
  return command<ConnectedDestination>("create_connected_destination", args);
}

/** List destinations visible to this installation. */
export function listConnectedDestinations(): Promise<ConnectedDestination[]> {
  return command<ConnectedDestination[]>("list_connected_destinations", {});
}

/** Update destination policy at an observed revision. */
export function updateConnectedDestinationPolicy(args: {
  destinationId: string;
  revision: number;
  immediateDisabled: boolean;
  digestDisabled: boolean;
}): Promise<ConnectedDestinationWrite> {
  return command<ConnectedDestinationWrite>("update_connected_destination_policy", args);
}

/** Resend a destination's rate-limited confirmation email. */
export function resendConnectedDestinationVerification(args: {
  destinationId: string;
}): Promise<ConnectedVerificationResend> {
  return command<ConnectedVerificationResend>("resend_connected_destination_verification", args);
}

/** Delete an unused destination or return the sites that still use it. */
export function deleteConnectedDestination(args: {
  destinationId: string;
}): Promise<ConnectedDestinationDeletion> {
  return command<ConnectedDestinationDeletion>("delete_connected_destination", args);
}

/** Where this site's alerts go, and the controls that shape them. */
export function getConnectedNotificationSettings(
  args: ConnectedScopeArgs,
): Promise<ConnectedNotificationSettings> {
  return command<ConnectedNotificationSettings>("get_connected_notification_settings", args);
}

/** Replace alert routing under revision guard, returning current revision on conflict. */
export function putConnectedNotificationSettings(
  args: ConnectedScopeArgs & {
    revision: number;
    destinationId: string | null;
    mute: boolean;
    allQuietHeartbeat: boolean;
    severityFloor: string | null;
    digestCadence: string;
    contentMode: string;
  },
): Promise<ConnectedNotificationSettingsWrite> {
  return command<ConnectedNotificationSettingsWrite>("put_connected_notification_settings", args);
}

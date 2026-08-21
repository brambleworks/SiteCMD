import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type {
  ConnectedDestination,
  ConnectedNotificationSettings,
} from "@/generated/ipc-bindings-connected";
import { Button } from "@/components/ui/button";
import { useToast } from "@/hooks/useToast";
import {
  getConnectedNotificationSettings,
  listConnectedDestinations,
  putConnectedNotificationSettings,
} from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";

interface ConnectedNotificationSettingsSectionProps {
  projectId: number;
  environmentScopeKey: string;
}

const SEVERITY_FLOORS = [
  { label: "Everything", value: "" },
  { label: "Low and above", value: "low" },
  { label: "Medium and above", value: "medium" },
  { label: "High and above", value: "high" },
  { label: "Critical only", value: "critical" },
];

const DIGEST_CADENCES = [
  { label: "Weekly", value: "weekly" },
  { label: "Daily", value: "daily" },
  { label: "No digest", value: "off" },
];

const CONTENT_MODES = [
  {
    label: "Private: site alias, severity, cause, and counts; never routes, evidence, or code",
    value: "private",
  },
  {
    label: "Minimal: only that an alert exists and a link, with no site metadata",
    value: "minimal",
  },
];

// Make non-delivering destinations explicit in the option label.
function destinationOptionLabel(destination: ConnectedDestination): string {
  const name = destination.address ?? destination.destinationId;
  if (destination.verification !== "verified") {
    return `${name} (waiting for confirmation, receives nothing yet)`;
  }
  if (destination.suppressed) {
    return `${name} (suppressed, receives nothing until confirmed again)`;
  }
  return name;
}

// Preserve an unlisted selected value so the select cannot show a false default.
function unlistedDestinationLabel(
  destinationId: string,
  status: "pending" | "error" | "success",
): string {
  if (status === "pending") {
    return `${destinationId} (reading this account's addresses)`;
  }
  if (status === "error") {
    return `${destinationId} (address list could not be read)`;
  }
  return `${destinationId} (no longer on this account's address list)`;
}

interface AlertForm {
  allQuietHeartbeat: boolean;
  destinationId: string;
  mute: boolean;
  severityFloor: string;
  digestCadence: string;
  contentMode: string;
}

function formFor(settings: ConnectedNotificationSettings | undefined): AlertForm {
  return {
    allQuietHeartbeat: settings?.allQuietHeartbeat ?? false,
    contentMode: settings?.contentMode ?? "private",
    destinationId: settings?.destinationId ?? "",
    digestCadence: settings?.digestCadence ?? "weekly",
    mute: settings?.mute ?? false,
    severityFloor: settings?.severityFloor ?? "",
  };
}

/** Edits this site's revision-guarded alert settings as a full replacement. */
export function ConnectedNotificationSettingsSection({
  projectId,
  environmentScopeKey,
}: ConnectedNotificationSettingsSectionProps) {
  const toast = useToast();
  const queryClient = useQueryClient();
  const scope = { environmentScopeKey, projectId };
  const scopeKey = `${projectId}|${environmentScopeKey}`;
  const settingsKey = queryKeys.settings.connectedNotificationSettings(
    projectId,
    environmentScopeKey,
  );
  const destinationsKey = queryKeys.settings.connectedDestinations();
  const settingsQuery = useQuery({
    queryKey: settingsKey,
    queryFn: () => getConnectedNotificationSettings(scope),
  });
  const destinationsQuery = useQuery({
    queryKey: destinationsKey,
    queryFn: () => listConnectedDestinations(),
  });

  // Clearing edits returns the form to the service's authoritative state.
  const [edit, setEdit] = useState<AlertForm | null>(null);
  // Preserve the newest write revision so stale cache data cannot authorize a save.
  const [minimumRevision, setMinimumRevision] = useState<{
    scopeKey: string;
    value: number;
  } | null>(null);
  const [saving, setSaving] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);

  const settings = settingsQuery.data;
  const revision = settings?.revision ?? 0;
  const revisionIsCurrent =
    minimumRevision?.scopeKey !== scopeKey || revision >= minimumRevision.value;
  const form = edit ?? formFor(settings);
  const change = (patch: Partial<AlertForm>) => setEdit({ ...form, ...patch });

  const handleSave = async () => {
    setSaving(true);
    setRefusal(null);
    try {
      const outcome = await putConnectedNotificationSettings({
        ...scope,
        contentMode: form.contentMode,
        allQuietHeartbeat: form.allQuietHeartbeat,
        destinationId: form.destinationId === "" ? null : form.destinationId,
        digestCadence: form.digestCadence,
        mute: form.mute,
        revision,
        severityFloor: form.severityFloor === "" ? null : form.severityFloor,
      });
      setMinimumRevision({ scopeKey, value: outcome.revision });
      setEdit(null);
      await queryClient.invalidateQueries({ queryKey: settingsKey });
      if (!outcome.applied) {
        setRefusal(outcome.message);
        await queryClient.invalidateQueries({ queryKey: destinationsKey });
        return;
      }
      toast.success(
        "Alert settings saved",
        form.destinationId === ""
          ? "This site sends no alert email. Its findings still appear in the app."
          : "Changing the address emails the previous one to say alerts moved.",
      );
    } catch (error) {
      toast.error("Could not save the alert settings", String(error));
    } finally {
      setSaving(false);
    }
  };

  const destinations = destinationsQuery.data ?? [];
  // Disable destination edits until the authoritative address list loads.
  const destinationsKnown = destinationsQuery.isSuccess;
  const chosen = destinations.find(
    (destination) => destination.destinationId === form.destinationId,
  );
  const unlisted = form.destinationId !== "" && chosen == null;
  const unusable = chosen != null && (chosen.verification !== "verified" || chosen.suppressed);

  return (
    <section className="card card--spacious">
      <div className="settings-card-title-rule">
        <h2 className="settings-card-title">Alert Email</h2>
      </div>
      <p className="body-muted">
        Which address this site pages, and how loudly. A site with no address chosen sends no alert
        email at all, which is where every site starts; its findings are in the app either way.
        Addresses are added once for the whole account in Alert Email Addresses above.
      </p>
      {settingsQuery.isError ? (
        <p className="agent-handoff-error" role="alert">
          Alert settings could not load.
        </p>
      ) : null}
      {refusal ? (
        <p className="agent-handoff-error" role="alert">
          {refusal}
        </p>
      ) : null}
      <div className="stack-base connected-form">
        <label className="form-label" htmlFor="connected-alert-destination">
          Send this site's alerts to
        </label>
        <select
          id="connected-alert-destination"
          value={form.destinationId}
          onChange={(event) => change({ destinationId: event.target.value })}
          disabled={!destinationsKnown}
          className="field-control field-control--muted field-control--select">
          <option value="">No email alerts for this site</option>
          {unlisted ? (
            <option value={form.destinationId}>
              {unlistedDestinationLabel(form.destinationId, destinationsQuery.status)}
            </option>
          ) : null}
          {destinations.map((destination) => (
            <option key={destination.destinationId} value={destination.destinationId}>
              {destinationOptionLabel(destination)}
            </option>
          ))}
        </select>
        {destinationsQuery.isError ? (
          <p className="agent-handoff-error" role="alert">
            The account's alert addresses could not load, so this site's address cannot be changed
            here. Whatever it is already pointed at is still in force.
          </p>
        ) : null}
        {unusable ? (
          <p className="text-body-muted">
            That address has not confirmed yet, so this site would page nobody. Confirm it in Alert
            Email Addresses above first.
          </p>
        ) : null}
        <label className="rep-attr-label">
          <input
            type="checkbox"
            checked={form.mute}
            onChange={(event) => change({ mute: event.target.checked })}
            className="rep-checkbox"
          />
          <span className="text-13-muted">
            Mute this site: no alert email, while webhook deliveries continue
          </span>
        </label>
        <label className="form-label" htmlFor="connected-alert-floor">
          Email me about
        </label>
        <select
          id="connected-alert-floor"
          value={form.severityFloor}
          onChange={(event) => change({ severityFloor: event.target.value })}
          className="field-control field-control--muted field-control--select">
          {SEVERITY_FLOORS.map((floor) => (
            <option key={floor.value} value={floor.value}>
              {floor.label}
            </option>
          ))}
        </select>
        <label className="form-label" htmlFor="connected-alert-digest">
          Digest
        </label>
        <select
          id="connected-alert-digest"
          value={form.digestCadence}
          onChange={(event) => change({ digestCadence: event.target.value })}
          className="field-control field-control--muted field-control--select">
          {DIGEST_CADENCES.map((cadence) => (
            <option key={cadence.value} value={cadence.value}>
              {cadence.label}
            </option>
          ))}
        </select>
        <label className="rep-attr-label">
          <input
            type="checkbox"
            checked={form.allQuietHeartbeat}
            onChange={(event) => change({ allQuietHeartbeat: event.target.checked })}
            disabled={form.digestCadence === "off"}
            className="rep-checkbox"
          />
          <span className="text-13-muted">
            Send an all-quiet heartbeat when this digest has no findings or health warnings
          </span>
        </label>
        <label className="form-label" htmlFor="connected-alert-content">
          What the email may contain
        </label>
        <select
          id="connected-alert-content"
          value={form.contentMode}
          onChange={(event) => change({ contentMode: event.target.value })}
          className="field-control field-control--muted field-control--select">
          {CONTENT_MODES.map((mode) => (
            <option key={mode.value} value={mode.value}>
              {mode.label}
            </option>
          ))}
        </select>
        <Button
          onClick={() => void handleSave()}
          disabled={saving || !settingsQuery.isSuccess || !destinationsKnown || !revisionIsCurrent}>
          {saving ? "Saving..." : "Save Alert Settings"}
        </Button>
        {settings && settings.thresholdCount > 0 ? (
          <p className="text-body-muted">
            {settings.thresholdCount} measurement threshold
            {settings.thresholdCount === 1 ? " is" : "s are"} set on this site and kept as they are
            when you save here.
          </p>
        ) : null}
      </div>
    </section>
  );
}

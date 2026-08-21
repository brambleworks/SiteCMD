import { useState } from "react";
import { BarChart3, Bug, Eye, RotateCcw, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { DiagnosticLogButtons } from "./DiagnosticLogButtons";
import { useToast } from "@/hooks/useToast";
import {
  buildTelemetryPreview,
  deleteQueuedTelemetry,
  requestUploadedTelemetryDeletion,
  resetTelemetrySubject,
  setTelemetryConsent,
  useTelemetryConsent,
} from "@/lib/telemetry";

export function TelemetrySettingsSection() {
  const consent = useTelemetryConsent();
  const { success, error: showError, info } = useToast();
  const [showPreview, setShowPreview] = useState(false);
  const [saving, setSaving] = useState(false);
  const [deletingUploaded, setDeletingUploaded] = useState(false);

  const updateConsent = async (patch: Partial<typeof consent>) => {
    setSaving(true);
    try {
      await setTelemetryConsent({
        usageAnalytics: patch.usageAnalytics ?? consent.usageAnalytics,
        crashReports: patch.crashReports ?? consent.crashReports,
        promptStatus: "saved",
      });
      success("Privacy preferences saved", "SiteCMD will follow these choices immediately.");
    } catch (err) {
      showError("Could not save privacy preferences", String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleDeleteQueued = () => {
    deleteQueuedTelemetry();
    success("Unsent telemetry deleted", "No unsent telemetry remains on this device.");
  };

  const handleResetSubject = async () => {
    await resetTelemetrySubject();
    success("Anonymous ID reset", "Future telemetry will use a new anonymous identifier.");
  };

  const handleDeleteUploaded = async () => {
    setDeletingUploaded(true);
    try {
      const result = await requestUploadedTelemetryDeletion();
      if (result === "not_configured") {
        info(
          "Telemetry endpoint is not configured",
          "There is no hosted telemetry endpoint in this build, so only local unsent data can be deleted.",
        );
      } else {
        success(
          "Uploaded telemetry deletion requested",
          "The hosted telemetry store accepted the deletion request.",
        );
      }
    } catch (err) {
      showError("Could not request uploaded telemetry deletion", String(err));
    } finally {
      setDeletingUploaded(false);
    }
  };

  return (
    <div className="settings-section-stack">
      <section className="card card--spacious">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">Opt-In Telemetry</h2>
        </div>
        <p className="body-muted settings-card-intro">
          These controls are independent. SiteCMD never sends scan URLs, source code, project paths,
          credentials, raw logs, or page content.
        </p>
        <div className="stack-base">
          <TelemetrySettingsToggle
            icon={BarChart3}
            title="Usage analytics"
            body="Anonymous workflow events for understanding which features are used and where product flows fail."
            checked={consent.usageAnalytics}
            disabled={saving}
            onChange={(checked) => {
              void updateConsent({ usageAnalytics: checked });
            }}
          />
          <TelemetrySettingsToggle
            icon={Bug}
            title="Crash and error reports"
            body="Sanitized app errors sent to Sentry. Session replay, autocapture, broad tracing, and PII are disabled."
            checked={consent.crashReports}
            disabled={saving}
            onChange={(checked) => {
              void updateConsent({ crashReports: checked });
            }}
          />
        </div>
      </section>

      <section className="card card--spacious">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">Review and Control</h2>
        </div>
        <div className="stack-card">
          <div className="settings-control-row subtle-divider-bottom">
            <div>
              <p className="text-13-medium">Preview data</p>
              <p className="subtitle-xs">
                See the event shape and redaction rules before enabling anything.
              </p>
            </div>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => setShowPreview((current) => !current)}>
              <Eye className="icon-sm" />
              {showPreview ? "Hide" : "Preview"}
            </Button>
          </div>

          {showPreview ? (
            <pre className="telemetry-preview-box">{buildTelemetryPreview()}</pre>
          ) : null}

          <div className="settings-control-row subtle-divider-bottom">
            <div>
              <p className="text-13-medium">Reset anonymous ID</p>
              <p className="subtitle-xs">
                Generate a new random identifier for future opt-in telemetry.
              </p>
            </div>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => void handleResetSubject()}>
              <RotateCcw className="icon-sm" />
              Reset
            </Button>
          </div>

          <div className="settings-control-row subtle-divider-bottom">
            <div>
              <p className="text-13-medium">Delete unsent telemetry</p>
              <p className="subtitle-xs">Remove unsent usage events stored on this device.</p>
            </div>
            <Button type="button" variant="outline" size="sm" onClick={handleDeleteQueued}>
              <Trash2 className="icon-sm" />
              Delete Unsent
            </Button>
          </div>

          <div className="settings-control-row">
            <div>
              <p className="text-13-medium">Delete uploaded telemetry</p>
              <p className="subtitle-xs">
                Ask SiteCMD's hosted telemetry endpoint to delete data tied to this anonymous ID.
              </p>
            </div>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={deletingUploaded}
              onClick={() => void handleDeleteUploaded()}>
              <Trash2 className="icon-sm" />
              {deletingUploaded ? "Requesting..." : "Request Delete"}
            </Button>
          </div>
        </div>
      </section>

      <section className="card card--spacious">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">Diagnostic Logs</h2>
        </div>
        <div className="row-between">
          <div>
            <p className="text-13-medium">Copy recent app logs</p>
            <p className="subtitle-xs">
              Nothing is sent automatically. Copy the local log when support or a bug report needs
              context, and review it before sharing.
            </p>
          </div>
          <DiagnosticLogButtons />
        </div>
      </section>
    </div>
  );
}

function TelemetrySettingsToggle({
  icon: Icon,
  title,
  body,
  checked,
  disabled,
  onChange,
}: {
  icon: typeof BarChart3;
  title: string;
  body: string;
  checked: boolean;
  disabled: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="telemetry-toggle-row">
      <div className="telemetry-toggle-copy">
        <div className="icon-badge icon-badge--md icon-badge--muted">
          <Icon className="icon-md text-primary" aria-hidden="true" />
        </div>
        <div>
          <p className="text-13-medium">{title}</p>
          <p className="subtitle-xs">{body}</p>
        </div>
      </div>
      <Button
        type="button"
        unstyled
        className="toggle-switch"
        data-on={checked ? "true" : "false"}
        role="switch"
        aria-checked={checked}
        aria-label={title}
        disabled={disabled}
        onClick={() => onChange(!checked)}>
        <span className="toggle-switch-thumb" />
      </Button>
    </div>
  );
}

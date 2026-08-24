import { useState } from "react";
import { BarChart3, Bug, ShieldCheck } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { buildTelemetryPreview, setTelemetryConsent, useTelemetryConsent } from "@/lib/telemetry";
import { userFacingError } from "@/lib/user-facing-error";

export function TelemetryConsentPrompt() {
  const consent = useTelemetryConsent();
  // Reopened prompts preserve the user's existing consent choices.
  const [usageAnalytics, setUsageAnalytics] = useState(consent.usageAnalytics);
  const [crashReports, setCrashReports] = useState(consent.crashReports);
  const [saving, setSaving] = useState(false);
  const [showPreview, setShowPreview] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  if (consent.promptStatus === "saved") return null;

  const save = async (next: { usageAnalytics: boolean; crashReports: boolean }) => {
    setSaving(true);
    setSaveError(null);
    try {
      await setTelemetryConsent({ ...next, promptStatus: "saved" });
    } catch (error) {
      // A wordless rejection already gets one full sentence from the fallback;
      // prefixing it again would say "not saved" twice.
      const fallback = "Your choice was not saved. Try again.";
      const message = userFacingError(error, fallback);
      setSaveError(
        message === fallback ? fallback : `Couldn't save your telemetry choice. ${message}`,
      );
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog
      labelledBy="telemetry-title"
      onClose={() => undefined}
      dismissOnBackdrop={false}
      closeOnEscape={false}
      backdropClassName="dialog--blur"
      className="modal-panel telemetry-consent-panel">
      <div className="telemetry-consent-header">
        <div className="icon-badge icon-badge--lg icon-badge--primary-strong">
          <ShieldCheck className="icon-lg text-primary" aria-hidden="true" />
        </div>
        <div className="flex-fill">
          <p className="section-label-mid text-brand-accent">Your data stays yours</p>
          <h2 id="telemetry-title" className="telemetry-consent-title">
            Help improve SiteCMD
          </h2>
        </div>
      </div>

      <div className="telemetry-consent-body">
        <p className="body-muted text-relaxed">
          Both options stay off unless you choose to enable them. Scan URLs, source code, project
          paths, credentials, raw logs, and page content are never included either way.
        </p>

        <TelemetryToggleRow
          icon={BarChart3}
          title="Usage analytics"
          body="Off by default. Anonymous workflow events: scan started, scan completed, settings opened, issue guidance copied. Turn on if you want to help shape what we build next."
          checked={usageAnalytics}
          onChange={setUsageAnalytics}
        />
        <TelemetryToggleRow
          icon={Bug}
          title="Crash and error reports"
          body="Off by default. Sanitized frontend crashes and failed app commands through Sentry. No replay, no autocapture, and no broad tracing."
          checked={crashReports}
          onChange={setCrashReports}
        />

        {showPreview ? (
          <pre className="telemetry-preview-box">{buildTelemetryPreview()}</pre>
        ) : null}

        {saveError ? (
          <p
            role="alert"
            aria-live="polite"
            className="telemetry-consent-error body-muted text-relaxed">
            {saveError}
          </p>
        ) : null}
      </div>

      <div className="telemetry-consent-footer">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => setShowPreview((current) => !current)}>
          {showPreview ? "Hide Preview" : "Preview Data"}
        </Button>
        <div className="row">
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={saving}
            onClick={() => {
              void save({ usageAnalytics: false, crashReports: false });
            }}>
            Keep Off
          </Button>
          <Button
            type="button"
            size="sm"
            disabled={saving}
            onClick={() => {
              void save({ usageAnalytics, crashReports });
            }}>
            {saving ? "Saving..." : "Save Choices"}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}

function TelemetryToggleRow({
  icon: Icon,
  title,
  body,
  checked,
  onChange,
}: {
  icon: typeof BarChart3;
  title: string;
  body: string;
  checked: boolean;
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
        onClick={() => onChange(!checked)}>
        <span className="toggle-switch-thumb" />
      </Button>
    </div>
  );
}

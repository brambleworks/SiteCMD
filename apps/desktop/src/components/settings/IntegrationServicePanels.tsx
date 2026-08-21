import { Eye, EyeOff, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ExtLink } from "@/components/ui/external-link";
import { ServiceIcon } from "@/components/icons/ServiceIcon";
import type { IntegrationData } from "./integration-services";
import { JIRA_SERVICE } from "./integration-services";
import {
  CloudflareDataView,
  GenericIntegrationDataView,
  PlausibleDataView,
  UptimeRobotDataView,
} from "./IntegrationDataViews";

const ICON_BG: Record<string, string> = {
  plausible: "integration-tint--plausible",
  cloudflare: "integration-tint--cloudflare",
  uptimerobot: "integration-tint--uptimerobot",
  googleanalytics: "integration-tint--ga",
  googlesearchconsole: "integration-tint--gsc",
  bingwebmaster: "integration-tint--bing",
  github: "integration-tint--github",
  jira: "integration-tint--jira",
};

export interface JiraFormValue {
  instanceUrl: string;
  email: string;
  apiToken: string;
  projectKey: string;
  issueType: string;
}

interface ApiKeySetupService {
  keyLabel: string;
  setupSteps: readonly string[];
  setupUrl?: string;
  setupUrlLabel?: string;
  siteIdLabel?: string | null;
  siteIdPlaceholder?: string | null;
  siteIdHelp?: string | null;
}

export function IntegrationServiceIconBadge({ type }: { type: string }) {
  return (
    <div className={`integration-icon-badge ${ICON_BG[type] || "bg-muted"}`}>
      <ServiceIcon type={type} className="icon-lg" />
    </div>
  );
}

export function IntegrationLiveDataPanel({
  serviceType,
  liveData,
  loading,
  onRefresh,
  onDisconnect,
}: {
  serviceType: string;
  liveData?: IntegrationData;
  loading?: boolean;
  onRefresh: () => void;
  onDisconnect: () => void;
}) {
  return (
    <div className="subtle-divider-top integration-panel">
      {liveData && !liveData.error ? (
        <div className="text-body-muted integration-data">
          {serviceType === "plausible" ? <PlausibleDataView data={liveData.data} /> : null}
          {serviceType === "cloudflare" ? <CloudflareDataView data={liveData.data} /> : null}
          {serviceType === "uptimerobot" ? <UptimeRobotDataView data={liveData.data} /> : null}
          {serviceType !== "plausible" &&
          serviceType !== "cloudflare" &&
          serviceType !== "uptimerobot" ? (
            <GenericIntegrationDataView data={liveData.data} />
          ) : null}
        </div>
      ) : null}
      {loading ? (
        <div className="integration-loading text-body-muted text-muted-foreground">
          <Loader2 className="icon-xs animate-spin" /> Fetching…
        </div>
      ) : null}
      {liveData?.error ? (
        <div className="danger-callout-row">
          <p className="text-body integration-error-text">
            Live data could not load: {liveData.error}
          </p>
        </div>
      ) : null}
      <div className="integration-actions">
        <Button onClick={onRefresh} variant="outline" size="sm" className="btn--grow">
          Refresh data
        </Button>
        <Button
          onClick={onDisconnect}
          variant="outline"
          size="sm"
          className="btn--grow integration-disconnect-btn">
          Disconnect
        </Button>
      </div>
    </div>
  );
}

export function ApiKeyIntegrationSetup({
  service,
  apiKey,
  siteId,
  showKey,
  saving,
  submitLabel = "Connect",
  savingLabel = "Connecting...",
  onApiKeyChange,
  onSiteIdChange,
  onToggleShowKey,
  onSave,
  onCancel,
}: {
  service: ApiKeySetupService;
  apiKey: string;
  siteId: string;
  showKey: boolean;
  saving: boolean;
  submitLabel?: string;
  savingLabel?: string;
  onApiKeyChange: (value: string) => void;
  onSiteIdChange: (value: string) => void;
  onToggleShowKey: () => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="subtle-divider-top integration-panel">
      {service.setupUrl ? (
        <ExtLink href={service.setupUrl} className="text-body-muted text-primary integration-link">
          {service.setupUrlLabel || "Open setup page →"}
        </ExtLink>
      ) : null}
      <SetupSteps steps={service.setupSteps} />
      <div>
        <label className="section-label-mid-block">{service.keyLabel}</label>
        <div className="integration-secret-wrap">
          <input
            type={showKey ? "text" : "password"}
            value={apiKey}
            onChange={(event) => onApiKeyChange(event.target.value)}
            placeholder={`Paste ${service.keyLabel.toLowerCase()}`}
            className="field-control field-control--card integration-secret-input"
          />
          <SecretToggleButton show={showKey} label="API key" onToggle={onToggleShowKey} />
        </div>
      </div>
      {service.siteIdLabel ? (
        <div>
          <label className="section-label-mid-block">{service.siteIdLabel}</label>
          <input
            value={siteId}
            onChange={(event) => onSiteIdChange(event.target.value)}
            placeholder={service.siteIdPlaceholder || ""}
            className="input-ghost bg-card"
          />
          {service.siteIdHelp ? (
            <p className="text-body-muted integration-help">{service.siteIdHelp}</p>
          ) : null}
        </div>
      ) : null}
      <div className="integration-actions">
        <Button size="sm" onClick={onSave} disabled={!apiKey || saving} className="btn--grow">
          {saving ? savingLabel : submitLabel}
        </Button>
        <Button variant="outline" size="sm" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </div>
  );
}

export function JiraIntegrationForm({
  form,
  showKey,
  saving,
  submitLabel,
  savingLabel,
  onChange,
  onToggleShowKey,
  onSave,
  onCancel,
  onDisconnect,
}: {
  form: JiraFormValue;
  showKey: boolean;
  saving: boolean;
  submitLabel: string;
  savingLabel: string;
  onChange: (next: Partial<JiraFormValue>) => void;
  onToggleShowKey: () => void;
  onSave: () => void;
  onCancel?: () => void;
  onDisconnect?: () => void;
}) {
  return (
    <div className="subtle-divider-top integration-panel">
      <ExtLink
        href={JIRA_SERVICE.setupUrl}
        className="text-body-muted text-primary integration-link">
        {JIRA_SERVICE.setupUrlLabel} →
      </ExtLink>
      <SetupSteps steps={JIRA_SERVICE.setupSteps} />
      <div>
        <label className="section-label-mid-block">Instance URL</label>
        <input
          value={form.instanceUrl}
          onChange={(event) => onChange({ instanceUrl: event.target.value })}
          placeholder="yourcompany.atlassian.net"
          className="input-ghost bg-card"
        />
      </div>
      <div>
        <label className="section-label-mid-block">Atlassian Email</label>
        <input
          type="email"
          value={form.email}
          onChange={(event) => onChange({ email: event.target.value })}
          placeholder="you@example.com"
          className="input-ghost bg-card"
        />
      </div>
      <div>
        <label className="section-label-mid-block">API Token</label>
        <div className="integration-secret-wrap">
          <input
            type={showKey ? "text" : "password"}
            value={form.apiToken}
            onChange={(event) => onChange({ apiToken: event.target.value })}
            placeholder="Paste API token"
            className="field-control field-control--card integration-secret-input"
          />
          <SecretToggleButton show={showKey} label="token" onToggle={onToggleShowKey} />
        </div>
      </div>
      <div className="integration-form-grid">
        <div>
          <label className="section-label-mid-block">Project Key</label>
          <input
            value={form.projectKey}
            onChange={(event) => onChange({ projectKey: event.target.value.toUpperCase() })}
            placeholder="PROJ"
            className="input-ghost integration-input-mono bg-card"
          />
        </div>
        <div>
          <label className="section-label-mid-block">Issue Type</label>
          <select
            value={form.issueType}
            onChange={(event) => onChange({ issueType: event.target.value })}
            className="field-control field-control--card">
            <option value="Bug">Bug</option>
            <option value="Task">Task</option>
            <option value="Story">Story</option>
            <option value="Epic">Epic</option>
          </select>
        </div>
      </div>
      <div className="integration-actions">
        <Button
          onClick={onSave}
          disabled={saving}
          size="sm"
          className="btn--grow btn--bold integration-save-btn">
          {saving ? savingLabel : submitLabel}
        </Button>
        {onDisconnect ? (
          <Button
            onClick={onDisconnect}
            variant="outline"
            size="sm"
            className="btn--grow integration-disconnect-btn">
            Disconnect
          </Button>
        ) : null}
        {onCancel ? (
          <Button onClick={onCancel} variant="outline" size="sm">
            Cancel
          </Button>
        ) : null}
      </div>
    </div>
  );
}

function SetupSteps({ steps }: { steps: readonly string[] }) {
  return (
    <ol className="integration-steps">
      {steps.map((step, index) => (
        <li key={index} className="integration-step text-body-muted text-muted-foreground">
          <span className="integration-step-num text-muted-foreground">{index + 1}.</span>
          <span>{step}</span>
        </li>
      ))}
    </ol>
  );
}

function SecretToggleButton({
  show,
  label,
  onToggle,
}: {
  show: boolean;
  label: string;
  onToggle: () => void;
}) {
  return (
    <Button
      unstyled
      onClick={onToggle}
      aria-label={show ? `Hide ${label}` : `Show ${label}`}
      className="integration-secret-toggle text-muted-foreground">
      {show ? <EyeOff className="icon-sm" /> : <Eye className="icon-sm" />}
    </Button>
  );
}

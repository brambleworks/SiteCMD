import { useId, type ChangeEvent, type RefObject } from "react";
import { ChevronDown, FileText, LayoutGrid, Loader2, RotateCcw, Upload, X } from "lucide-react";
import { Button } from "@/components/ui/button";

import {
  getBrandingPreviewSrc,
  getReportCoverageBadges,
  PERIOD_OPTIONS,
  REPORT_LOGO_ACCEPT,
  SECTION_LABELS,
  type Branding,
  type ReportData,
  type SectionConfig,
} from "@/components/reports/reports-page-model";

interface ReportsSetupPanelProps {
  periodDays: number;
  reportTitle: string;
  onPeriodDaysChange: (periodDays: number) => void;
  onReportTitleChange: (title: string) => void;
}

export function ReportsSetupPanel({
  periodDays,
  reportTitle,
  onPeriodDaysChange,
  onReportTitleChange,
}: ReportsSetupPanelProps) {
  const reportTitleId = useId();
  return (
    <div className="rep-2col-grid">
      <div className="card card--muted card--spacious">
        <label className="section-label-block" htmlFor={reportTitleId}>
          Report Title
        </label>
        <input
          id={reportTitleId}
          type="text"
          value={reportTitle}
          onChange={(event) => onReportTitleChange(event.target.value)}
          placeholder="Site & Code Report"
          className="field-control field-control--card row-title-lg"
        />
      </div>
      <div className="card card--muted card--spacious">
        <p className="section-label-block">Report Period</p>
        <div className="row">
          {PERIOD_OPTIONS.map((option) => (
            <Button
              unstyled
              key={option.value}
              type="button"
              onClick={() => onPeriodDaysChange(option.value)}
              className={`rep-period-btn ghost-border ${
                periodDays === option.value ? "rep-period-btn--active" : "rep-period-btn--inactive"
              }`}>
              {option.label}
            </Button>
          ))}
        </div>
      </div>
    </div>
  );
}

interface ReportsCoveragePanelProps {
  hasLinkedFolder: boolean;
  sections: SectionConfig;
}

export function ReportsCoveragePanel({ hasLinkedFolder, sections }: ReportsCoveragePanelProps) {
  const coverageBadges = getReportCoverageBadges(sections, hasLinkedFolder);
  const coverageSummary =
    sections.code_scan && hasLinkedFolder
      ? "This export covers Web Scan issues and linked Code Scan results."
      : sections.code_scan
        ? "This export covers Web Scan issues now. Link a project folder to include Code Scan issues too."
        : "This export focuses on Web Scan issues for the selected period.";

  return (
    <div className="card card--muted card--spacious">
      <div className="rep-sk-head">
        <div>
          <p className="section-label-block">Report Coverage</p>
          <p className="text-relaxed rep-coverage-summary">{coverageSummary}</p>
        </div>
        {sections.code_scan && (
          <div
            className={`rep-coverage-tag ${hasLinkedFolder ? "text-primary" : "text-severity-medium"}`}>
            {hasLinkedFolder ? "Code Ready" : "Link Folder For Code"}
          </div>
        )}
      </div>
      <div className="rep-sk-badges">
        {coverageBadges.map((badge) => (
          <span
            key={badge.label}
            className={`text-meta rep-history-badge ${
              badge.tone === "primary"
                ? "text-primary"
                : badge.tone === "warning"
                  ? "text-severity-medium"
                  : "text-muted-foreground"
            }`}>
            {badge.label}
          </span>
        ))}
      </div>
      {sections.code_scan && !hasLinkedFolder && (
        <p className="text-body-muted rep-note">
          Code Scan is enabled in this report, but this project does not have a linked folder yet.
          Link the site folder to include database, AI, security, architecture, and operations
          issues.
        </p>
      )}
    </div>
  );
}

interface ReportsSnapshotPanelProps {
  hasLinkedFolder: boolean;
  reportSnapshot: ReportData | null;
  sections: SectionConfig;
  snapshotError?: string | null;
  // Show progress for initial loads and refetches over cached data.
  snapshotBusy: boolean;
  onRefreshSnapshot: () => void;
}

export function ReportsSnapshotPanel({
  hasLinkedFolder,
  reportSnapshot,
  sections,
  snapshotError,
  snapshotBusy,
  onRefreshSnapshot,
}: ReportsSnapshotPanelProps) {
  const reportWebScanSummary = reportSnapshot?.health ?? null;

  return (
    <div className="card card--muted card--spacious">
      <div className="rep-sk-head">
        <div>
          <p className="section-label-block">Latest Included Snapshot</p>
          <p className="text-body text-muted-foreground rep-snap-desc">
            Pulled from the latest data that will feed this report window, so you can see whether
            the export will include fresh Web Scan issues, linked code issues, and connected
            operational data before you generate it.
          </p>
        </div>
        <Button
          unstyled
          type="button"
          onClick={onRefreshSnapshot}
          disabled={snapshotBusy}
          className="btn-ghost-xs bg-card rep-refresh-btn">
          {snapshotBusy ? (
            <Loader2 className="icon-sm animate-spin" />
          ) : (
            <RotateCcw className="icon-sm" />
          )}
          Refresh
        </Button>
      </div>

      {snapshotError ? (
        <p className="text-body-muted rep-error" role="alert">
          {snapshotError} Use Refresh to try again.
        </p>
      ) : null}

      <div className="rep-sk-metric-grid">
        <div className="card">
          <p className="section-label-mid">Web Scan</p>
          {reportSnapshot && reportWebScanSummary ? (
            <>
              <div className="rep-score-row">
                <span className="rep-score-big">{reportWebScanSummary.currentScore}</span>
                <span className="text-body-muted rep-score-unit">/100</span>
              </div>
              <p className="text-body rep-snap-line">
                {reportWebScanSummary.issuesCritical} critical · {reportWebScanSummary.issuesHigh}{" "}
                high · {reportWebScanSummary.issuesTotal} total issues
              </p>
              <p className="text-body-muted rep-snap-note">
                {reportSnapshot.latestScanDate
                  ? `Latest Web Scan ${reportSnapshot.latestScanDate}.`
                  : "No recent Web Scan in this window yet."}
              </p>
            </>
          ) : (
            <p className="text-body-muted rep-note">Snapshot unavailable right now.</p>
          )}
        </div>

        <div className="card">
          <p className="section-label-mid">Code Scan</p>
          {!sections.code_scan ? (
            <p className="text-body-muted rep-note">Code Scan is turned off for this export.</p>
          ) : !hasLinkedFolder ? (
            <p className="text-body-muted rep-note">
              Link a project folder to include database, AI, security, architecture, and operations
              issues.
            </p>
          ) : reportSnapshot?.codeScan ? (
            <>
              <div className="rep-score-row">
                <span className="rep-score-big">{reportSnapshot.codeScan.currentScore}</span>
                <span className="text-body-muted rep-score-unit">/100</span>
              </div>
              <p className="text-body rep-snap-line">
                {reportSnapshot.codeScan.criticalCount} critical ·{" "}
                {reportSnapshot.codeScan.highCount} high · {reportSnapshot.codeScan.issueCount}{" "}
                total issues
              </p>
              <p className="text-body-muted rep-snap-note">
                Leading domain: {reportSnapshot.codeScan.topDomain || "Code Scan"} · checked{" "}
                {new Date(reportSnapshot.codeScan.checkedAt).toLocaleDateString(undefined, {
                  month: "short",
                  day: "numeric",
                  year: "numeric",
                })}
              </p>
              {reportSnapshot.codeScan.domainTrend && (
                <p className="text-body-muted rep-history-meta">
                  {reportSnapshot.codeScan.domainTrend}
                </p>
              )}
            </>
          ) : (
            <p className="text-body-muted rep-note">
              No Code Scan was found in the current report window yet. Run a Code Scan from Issues
              to include code issues here.
            </p>
          )}
        </div>

        <div className="card">
          <p className="section-label-mid">Connected Data</p>
          <div className="rep-connected-badges">
            {[
              {
                label: "Analytics",
                enabled: sections.analytics,
                ready: Boolean(reportSnapshot?.analytics),
              },
              {
                label: "Uptime",
                enabled: sections.uptime,
                ready: Boolean(reportSnapshot?.uptime),
              },
              {
                label: "Deployments",
                enabled: sections.deploys,
                ready: Boolean(reportSnapshot?.deploys),
              },
            ].map((item) => (
              <span
                key={item.label}
                className={`text-meta rep-history-badge ${
                  !item.enabled
                    ? "text-muted-foreground"
                    : item.ready
                      ? "text-score-excellent"
                      : "text-severity-medium"
                }`}>
                {item.label} {item.enabled ? (item.ready ? "ready" : "missing") : "off"}
              </span>
            ))}
          </div>
          <p className="text-body-muted rep-note">
            {reportSnapshot
              ? "These integrations and data sources will only appear when they have data in the selected period."
              : "Refresh the snapshot to confirm which connected data will render in this report."}
          </p>
        </div>
      </div>
    </div>
  );
}

interface ReportsBrandingPanelProps {
  branding: Branding;
  logoInputRef: RefObject<HTMLInputElement | null>;
  showBranding: boolean;
  onBrandingChange: (update: Partial<Branding>) => void;
  onLogoFileChange: (event: ChangeEvent<HTMLInputElement>) => void;
  onToggle: () => void;
}

export function ReportsBrandingPanel({
  branding,
  logoInputRef,
  showBranding,
  onBrandingChange,
  onLogoFileChange,
  onToggle,
}: ReportsBrandingPanelProps) {
  const logoPreviewSrc = getBrandingPreviewSrc(branding);
  const companyNameId = useId();
  const primaryColorId = useId();
  const footerTextId = useId();
  const clientNameId = useId();

  return (
    <div className="panel panel--flush panel--muted">
      <input
        ref={logoInputRef}
        type="file"
        accept={REPORT_LOGO_ACCEPT}
        className="rep-file-input"
        onChange={onLogoFileChange}
      />
      <Button unstyled type="button" onClick={onToggle} className="report-builder-row">
        <div className="row-loose">
          <FileText className="icon-md text-primary" />
          <p className="row-title-lg">White-Label Branding</p>
        </div>
        <ChevronDown
          className={`icon-md text-muted-foreground rep-chevron ${showBranding ? "rep-chevron--open" : ""}`}
        />
      </Button>
      {showBranding && (
        <div className="subtle-divider-top rep-panel-body">
          <div className="rep-panel-grid">
            <div>
              <label className="section-label-mid rep-field-label" htmlFor={companyNameId}>
                Company Name
              </label>
              <input
                id={companyNameId}
                type="text"
                value={branding.company_name}
                onChange={(event) => onBrandingChange({ company_name: event.target.value })}
                className="field-control field-control--card"
              />
            </div>
            <div>
              <label className="section-label-mid rep-field-label" htmlFor={primaryColorId}>
                Primary Color
              </label>
              <div className="row">
                <input
                  id={primaryColorId}
                  type="color"
                  value={branding.primary_color}
                  onChange={(event) => onBrandingChange({ primary_color: event.target.value })}
                  className="rep-color-input ghost-border"
                />
                <input
                  type="text"
                  value={branding.primary_color}
                  onChange={(event) => onBrandingChange({ primary_color: event.target.value })}
                  aria-label="Primary color hex value"
                  className="field-control field-control--card rep-color-text"
                />
              </div>
            </div>
          </div>
          <div>
            <label className="section-label-mid rep-field-label">Logo</label>
            {logoPreviewSrc ? (
              <div className="row-loose">
                <img src={logoPreviewSrc} alt="Logo" className="rep-logo-preview ghost-border" />
                <span className="subtitle-xs text-truncate flex-fill">
                  {branding.logo_name || branding.logo_path?.split("/").pop() || "Selected logo"}
                </span>
                <Button
                  variant="ghost"
                  size="icon"
                  type="button"
                  onClick={() =>
                    onBrandingChange({ logo_path: null, logo_data_url: null, logo_name: null })
                  }
                  aria-label="Remove logo"
                  className="rep-logo-remove text-muted-foreground">
                  <X className="icon-sm" />
                </Button>
              </div>
            ) : (
              <Button
                variant="outline"
                size="sm"
                type="button"
                onClick={() => logoInputRef.current?.click()}
                className="btn--gap-snug text-muted-foreground">
                <Upload className="icon-sm" /> Choose logo image
              </Button>
            )}
            <p className="text-meta rep-snap-note">
              PNG, JPG, WEBP, or GIF up to 2 MB. Logos stay in this session and are embedded into
              the generated report directly, so no local file path is sent to the backend.
            </p>
          </div>
          <div>
            <label className="section-label-mid rep-field-label" htmlFor={footerTextId}>
              Footer Text
            </label>
            <input
              id={footerTextId}
              type="text"
              value={branding.footer_text}
              onChange={(event) => onBrandingChange({ footer_text: event.target.value })}
              className="field-control field-control--card"
            />
          </div>
          <div>
            <label className="section-label-mid rep-field-label" htmlFor={clientNameId}>
              Client Name
            </label>
            <input
              id={clientNameId}
              type="text"
              value={branding.client_name || ""}
              onChange={(event) => onBrandingChange({ client_name: event.target.value || null })}
              placeholder="Leave empty to omit"
              className="field-control field-control--card"
            />
          </div>
          <label className="rep-attr-label">
            <input
              type="checkbox"
              checked={branding.hide_attribution}
              onChange={(event) => onBrandingChange({ hide_attribution: event.target.checked })}
              className="rep-checkbox"
            />
            <span className="text-13-muted">Hide "Generated by SiteCMD" attribution</span>
          </label>
        </div>
      )}
    </div>
  );
}

interface ReportsSectionsPanelProps {
  sections: SectionConfig;
  showSections: boolean;
  onSectionChange: (key: keyof SectionConfig, value: boolean) => void;
  onToggle: () => void;
}

export function ReportsSectionsPanel({
  sections,
  showSections,
  onSectionChange,
  onToggle,
}: ReportsSectionsPanelProps) {
  return (
    <div className="panel panel--flush panel--muted">
      <Button unstyled type="button" onClick={onToggle} className="report-builder-row">
        <div className="row-loose">
          <LayoutGrid className="icon-md text-primary" />
          <p className="row-title-lg">Report Sections</p>
        </div>
        <ChevronDown
          className={`icon-md text-muted-foreground rep-chevron ${showSections ? "rep-chevron--open" : ""}`}
        />
      </Button>
      {showSections && (
        <div className="builder-action-grid builder-action-grid--divided">
          {SECTION_LABELS.map(({ key, label, icon: Icon }) => {
            const checked = sections[key];
            return (
              <label
                key={key}
                className={`rep-section-row ghost-border ${checked ? "rep-section-row--on" : ""}`}>
                <Icon className="icon-muted no-shrink" />
                <span className="text-body-muted text-foreground flex-fill">{label}</span>
                <input
                  type="checkbox"
                  checked={checked}
                  onChange={(event) => onSectionChange(key, event.target.checked)}
                  className="rep-checkbox no-shrink"
                />
              </label>
            );
          })}
        </div>
      )}
    </div>
  );
}

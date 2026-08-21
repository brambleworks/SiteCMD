/* eslint-disable react-refresh/only-export-components -- test helpers are exported here. */

import { useState, useRef, useCallback, type ChangeEvent } from "react";
import { writeExportBytes, writeExportFile } from "@/lib/commands";
import { renderReportHtmlFromData } from "./report-commands";
import { HeaderActions } from "@/app/ShellHeader";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { FileText, Loader2 } from "lucide-react";
import { useToast } from "@/hooks/useToast";
import { SurfaceState } from "@/components/ui/surface-state";
import { useReportSnapshot } from "@/components/reports/useReportSnapshot";
import { useReportsHistory } from "@/components/reports/useReportsHistory";
import { Button } from "@/components/ui/button";
import {
  ReportsBrandingPanel,
  ReportsCoveragePanel,
  ReportsSectionsPanel,
  ReportsSetupPanel,
  ReportsSnapshotPanel,
} from "@/components/reports/ReportsBuilderPanels";
import {
  ReportsBuilderLoadingState,
  ReportsHistorySection,
  ReportsPreview,
} from "@/components/reports/ReportsPageSections";
import {
  isSupportedReportLogo,
  loadBranding,
  loadSections,
  parseHistoryBranding,
  parseHistorySections,
  readFileAsDataUrl,
  REPORT_LOGO_MAX_BYTES,
  saveBranding,
  saveSections,
  type Branding,
  type ReportHistoryEntry,
  type SectionConfig,
} from "@/components/reports/reports-page-model";

export {
  formatSavedReportDate,
  getHistoryCoverageBadges,
  getReportCoverageBadges,
  isSupportedReportLogo,
  parseHistoryBranding,
  parseHistorySections,
  parseHistorySummary,
  reportFormatLabel,
  toPersistedBranding,
  toReportBrandingPayload,
} from "@/components/reports/reports-page-model";
export type { ReportHistorySummary, SectionConfig } from "@/components/reports/reports-page-model";

interface ReportsPageProps {
  projectId: number | null;
  siteUrl: string;
  projectPath?: string | null;
}

export function ReportsPage({ projectId, siteUrl, projectPath }: ReportsPageProps) {
  return <ReportsBuilder projectId={projectId} siteUrl={siteUrl} projectPath={projectPath} />;
}

function ReportsBuilder({
  projectId,
  siteUrl,
  projectPath,
}: {
  projectId: number | null;
  siteUrl: string;
  projectPath?: string | null;
}) {
  const [periodDays, setPeriodDays] = useState(30);
  const [generating, setGenerating] = useState(false);
  const [previewHtml, setPreviewHtml] = useState<string | null>(null);
  const [branding, setBranding] = useState<Branding>(() => loadBranding(projectId));
  const [sections, setSections] = useState<SectionConfig>(() => loadSections(projectId));
  const [reportTitle, setReportTitle] = useState("Site & Code Report");
  const [showBranding, setShowBranding] = useState(true);
  const [showSections, setShowSections] = useState(true);
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const logoInputRef = useRef<HTMLInputElement>(null);
  const toast = useToast();
  const hasLinkedFolder = Boolean(projectPath);

  // Reload settings when the project changes, adjusting state during render
  // instead of via an effect.
  const [loadedProjectId, setLoadedProjectId] = useState(projectId);
  if (loadedProjectId !== projectId) {
    setLoadedProjectId(projectId);
    setBranding(loadBranding(projectId));
    setSections(loadSections(projectId));
  }

  const updateBranding = useCallback(
    (update: Partial<Branding>) => {
      setBranding((current) => {
        const next = { ...current, ...update };
        if (!saveBranding(projectId, next)) {
          toast.warning(
            "Branding saved for this session",
            "Logo settings could not be persisted after reload.",
          );
        }
        return next;
      });
    },
    [projectId, toast],
  );

  const updateSections = (key: keyof SectionConfig, value: boolean) => {
    const next = { ...sections, [key]: value };
    setSections(next);
    if (!saveSections(projectId, next)) {
      toast.warning(
        "Section settings saved for this session",
        "Report section preferences could not be persisted after reload.",
      );
    }
  };

  const {
    buildConfiguredReportData,
    ensureHistorySummary,
    loadSnapshot,
    reportSnapshot,
    snapshotError,
    snapshotLoading,
    snapshotRefreshing,
  } = useReportSnapshot({
    branding,
    periodDays,
    projectId,
    reportTitle,
    sections,
    siteUrl,
  });

  const {
    deleteHistoryReport,
    history,
    historyError,
    historyLoading,
    loadHistory,
    recordReportHistory,
  } = useReportsHistory({
    branding,
    ensureHistorySummary,
    periodDays,
    projectId,
    reportTitle,
    sections,
    siteUrl,
    toast,
  });

  const handleLogoFileChange = useCallback(
    async (event: ChangeEvent<HTMLInputElement>) => {
      const file = event.target.files?.[0];
      event.target.value = "";
      if (!file) return;
      if (!isSupportedReportLogo(file)) {
        toast.error("Unsupported logo", "Use a PNG, JPG, WEBP, or GIF logo image.");
        return;
      }
      if (file.size > REPORT_LOGO_MAX_BYTES) {
        toast.error("Logo too large", "Choose an image under 2 MB.");
        return;
      }
      try {
        const logoDataUrl = await readFileAsDataUrl(file);
        updateBranding({
          logo_path: null,
          logo_data_url: logoDataUrl,
          logo_name: file.name,
        });
      } catch (error) {
        toast.error("Logo upload failed", String(error));
      }
    },
    [toast, updateBranding],
  );

  const handleGenerate = async (mode: "preview" | "download") => {
    if (!projectId || !siteUrl) {
      toast.warning("No site selected", "Select a project with a URL to generate reports.");
      return;
    }
    setGenerating(true);
    try {
      const data = await buildConfiguredReportData();
      const html = await renderReportHtmlFromData({ data });
      if (mode === "preview") {
        setPreviewHtml(html);
        try {
          await recordReportHistory("preview", data);
        } catch {
          // Report history is best-effort.
        }
      } else {
        const safeName = siteUrl.replace(/https?:\/\//, "").replace(/[^a-zA-Z0-9.-]/g, "_");
        const filePath = await saveDialog({
          title: "Save Report",
          defaultPath: `${safeName}-report-${periodDays}d.html`,
          filters: [{ name: "HTML Report", extensions: ["html"] }],
        });
        if (filePath) {
          await writeExportFile({ path: filePath, content: html });
          try {
            await recordReportHistory("html", data);
          } catch {
            // Report history is best-effort.
          }
          toast.success("Report saved", `Saved to ${filePath.split("/").pop()}`);
        }
      }
    } catch (e) {
      toast.error("Report generation failed", String(e));
    } finally {
      setGenerating(false);
    }
  };

  const handleExportPDF = async () => {
    if (!projectId || !siteUrl) return;
    setGenerating(true);
    try {
      const data = await buildConfiguredReportData();

      const [{ pdf }, { ReportPDFDocument }] = await Promise.all([
        import("@/lib/react-pdf-browser"),
        import("./ReportPDF"),
      ]);

      const blob = await pdf(<ReportPDFDocument data={data} />).toBlob();
      const buffer = await blob.arrayBuffer();
      const bytes = Array.from(new Uint8Array(buffer));

      const safeName = siteUrl.replace(/https?:\/\//, "").replace(/[^a-zA-Z0-9.-]/g, "_");
      const filePath = await saveDialog({
        title: "Save PDF Report",
        defaultPath: `${safeName}-report-${periodDays}d.pdf`,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (filePath) {
        await writeExportBytes({ path: filePath, bytes });
        try {
          await recordReportHistory("pdf", data);
        } catch {
          // Report history is best-effort.
        }
        toast.success("PDF saved", `Saved to ${filePath.split("/").pop()}`);
      }
    } catch (e) {
      toast.error("PDF export failed", String(e));
    } finally {
      setGenerating(false);
    }
  };

  const handleRegenerateHistoryReport = useCallback(
    async (entry: ReportHistoryEntry) => {
      const savedSections = parseHistorySections(entry.sectionsJson);
      const savedBranding = parseHistoryBranding(entry.brandingJson, branding);
      setGenerating(true);
      try {
        const data = await buildConfiguredReportData({
          projectId: entry.projectId,
          siteUrl: entry.siteUrl,
          periodDays: entry.periodDays,
          branding: savedBranding,
          reportTitle: entry.reportTitle,
          sections: savedSections,
        });
        const html = await renderReportHtmlFromData({ data });
        setPreviewHtml(html);
      } catch (e) {
        toast.error("Regeneration failed", String(e));
      } finally {
        setGenerating(false);
      }
    },
    [branding, buildConfiguredReportData, toast],
  );

  if (!projectId || !siteUrl) {
    return (
      <SurfaceState
        kind="empty"
        icon={<FileText className="empty-state-icon" />}
        title="No site selected"
        description="Select a project with a live URL and SiteCMD will build the report from its latest scan, signals, and connected integrations."
        className="page-content"
      />
    );
  }

  if (snapshotLoading && !reportSnapshot) {
    return <ReportsBuilderLoadingState />;
  }

  if (previewHtml) {
    return (
      <ReportsPreview
        generating={generating}
        iframeRef={iframeRef}
        previewHtml={previewHtml}
        onExportPDF={handleExportPDF}
        onSaveHtml={() => handleGenerate("download")}
        onClose={() => setPreviewHtml(null)}
      />
    );
  }

  return (
    <div className="page-content stack-hero">
      <HeaderActions>
        <Button
          unstyled
          onClick={() => handleGenerate("preview")}
          disabled={generating}
          className="btn-action bg-primary text-primary-foreground">
          {generating ? (
            <Loader2 className="icon-md animate-spin" />
          ) : (
            <FileText className="icon-md" />
          )}
          Generate Report
        </Button>
      </HeaderActions>

      <ReportsSetupPanel
        periodDays={periodDays}
        reportTitle={reportTitle}
        onPeriodDaysChange={setPeriodDays}
        onReportTitleChange={setReportTitle}
      />

      <ReportsCoveragePanel hasLinkedFolder={hasLinkedFolder} sections={sections} />

      <ReportsSnapshotPanel
        hasLinkedFolder={hasLinkedFolder}
        reportSnapshot={reportSnapshot}
        sections={sections}
        snapshotError={snapshotError}
        snapshotBusy={snapshotLoading || snapshotRefreshing}
        onRefreshSnapshot={() => void loadSnapshot()}
      />

      <ReportsBrandingPanel
        branding={branding}
        logoInputRef={logoInputRef}
        showBranding={showBranding}
        onBrandingChange={updateBranding}
        onLogoFileChange={(event) => {
          void handleLogoFileChange(event);
        }}
        onToggle={() => setShowBranding((current) => !current)}
      />

      <ReportsSectionsPanel
        sections={sections}
        showSections={showSections}
        onSectionChange={updateSections}
        onToggle={() => setShowSections((current) => !current)}
      />

      <ReportsHistorySection
        history={history}
        error={historyError}
        loading={historyLoading}
        onDelete={deleteHistoryReport}
        onRegenerate={handleRegenerateHistoryReport}
        onRetry={() => void loadHistory()}
      />
    </div>
  );
}

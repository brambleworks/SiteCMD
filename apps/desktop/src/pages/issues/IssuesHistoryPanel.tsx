import { useHistoryContext } from "@/app/history-context";
import type { ScanConfigPreset } from "@/components/scan/ScanConfigOverlay";
import { ScanHistory } from "@/components/scan/ScanHistory";
import { SurfaceState } from "@/components/ui/surface-state";
import { IssuePanelSkeleton } from "@/components/issues/IssuePanelSkeleton";

export function IssuesHistoryPanel({
  projectId,
  url,
  openScanConfig,
}: {
  projectId: number;
  url: string;
  openScanConfig: (preset?: ScanConfigPreset) => void;
}) {
  const { executions, loadHistory, loading: historyLoading, historyError } = useHistoryContext();

  if (historyLoading) {
    return <IssuePanelSkeleton label="Loading scan history" className="panel-inset" rows={6} />;
  }

  if (historyError) {
    return (
      <SurfaceState
        kind="error"
        title="Scan history unavailable"
        description={historyError}
        primaryAction={{ label: "Retry", onClick: () => void loadHistory(url, projectId) }}
      />
    );
  }

  return (
    <ScanHistory
      executions={executions}
      onOpenScanConfig={() => openScanConfig({ scanType: "full" })}
    />
  );
}
